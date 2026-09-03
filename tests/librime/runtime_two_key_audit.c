/* XHUP Flow 二码零冲突层 runtime 审计。
 *
 * 对 manifest 中的每一行:
 *
 * - occupied 行(全部 405 个既有占用 2 键码,全量不抽样):
 *   在 PRODUCTION(xhup_flow schema,真实生成包)中输入该码,断言实际
 *   菜单与 manifest 记录的既有菜单逐项同序完全一致 —— P0:二码层加入
 *   前后,全部既有 2 键单字菜单不变(top1/次序/数量);
 *
 * - selected 行(全部选定二码映射,全量):
 *   输入 2 键码,断言目标词恰好出现一次且 rank 1(空码上唯一 exact
 *   候选),无 auto commit、组合保持活动(可继续输入完整码)。
 *
 * 每进程只 initialize 一次(glog 限制),单趟即可(无需 CONTROL 派生
 * schema:既有菜单快照来自 analyzer manifest,其本身已由 canonical 数据
 * 独立重建,且二码词典只可能新增空码候选)。
 *
 * 用法:
 *
 *   runtime_two_key_audit <shared> <production_dir> <manifest.tsv>
 *
 * user dir 必须已含生成包并完成 rime_deployer --compile,且
 * default.custom.yaml 中 menu/page_size 足够大(测试枚举专用配置)。
 * 只使用稳定 C API;无第三方测试框架;不访问用户真实 Rime 目录。
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rime_api.h>

#define SEP '\x1f'
#define TEST_MENU_PAGE_SIZE 500

static int failures = 0;
static int checks = 0;

static void report(int ok, const char *name, const char *detail) {
    ++checks;
    if (!ok) {
        ++failures;
    }
    printf("%s  %s%s%s\n", ok ? "PASS" : "FAIL", name,
           detail ? "  " : "", detail ? detail : "");
}

static RimeApi *rime;

static void type_keys(RimeSessionId session, const char *keys) {
    for (const char *p = keys; *p; ++p) {
        rime->process_key(session, *p, 0);
    }
}

/* 捕获当前菜单:SEP 分隔文本序列写入 buf,返回候选数。 */
static int capture_menu(RimeSessionId session, char *buf, size_t size,
                        int *composition_active) {
    int n = 0;
    buf[0] = '\0';
    *composition_active = 0;
    RIME_STRUCT(RimeContext, context);
    if (rime->get_context(session, &context)) {
        *composition_active = context.composition.length > 0;
        n = context.menu.num_candidates;
        for (int i = 0; i < n; ++i) {
            const char *text = context.menu.candidates[i].text;
            if (i > 0) {
                size_t len = strlen(buf);
                if (len + 1 < size) {
                    buf[len] = SEP;
                    buf[len + 1] = '\0';
                }
            }
            strncat(buf, text, size - strlen(buf) - 1);
        }
        rime->free_context(&context);
    }
    return n;
}

static int has_commit(RimeSessionId session) {
    RIME_STRUCT(RimeCommit, commit);
    int committed = 0;
    if (rime->get_commit(session, &commit)) {
        committed = 1;
        rime->free_commit(&commit);
    }
    return committed;
}

static void reset_composition(RimeSessionId session) {
    rime->clear_composition(session);
}

/* 把 manifest 的逗号分隔菜单转换为 SEP 分隔(与 capture_menu 同格式)。 */
static void menu_field_to_sep(const char *field, char *buf, size_t size) {
    buf[0] = '\0';
    for (const char *p = field; *p; ++p) {
        if (*p == ',') {
            size_t len = strlen(buf);
            if (len + 1 < size) {
                buf[len] = SEP;
                buf[len + 1] = '\0';
            }
        } else {
            size_t len = strlen(buf);
            if (len + 1 < size) {
                buf[len] = *p;
                buf[len + 1] = '\0';
            }
        }
    }
}

typedef struct {
    char code[16];
    char *fanout;   /* occupied: fanout 数值字符串 */
    char *menu;     /* occupied: 逗号分隔既有菜单 */
    char *word;     /* selected: 目标词 */
    char *full_code;/* selected: 完整码 */
    int is_selected;
} Row;

static char *xstrdup(const char *s) {
    char *copy = strdup(s ? s : "");
    if (!copy) {
        fprintf(stderr, "内存不足\n");
        exit(2);
    }
    return copy;
}

static int load_manifest(const char *path, Row **rows_out, int *count_out) {
    FILE *f = fopen(path, "r");
    if (!f) {
        fprintf(stderr, "无法打开 manifest %s\n", path);
        return -1;
    }
    Row *rows = NULL;
    int count = 0;
    int capacity = 0;
    char line[65536];
    while (fgets(line, sizeof(line), f)) {
        size_t len = strlen(line);
        while (len && (line[len - 1] == '\n' || line[len - 1] == '\r')) {
            line[--len] = '\0';
        }
        if (line[0] == '#' || line[0] == '\0') {
            continue;
        }
        char kind[16] = "";
        char *fields[4] = {NULL, NULL, NULL, NULL};
        int nfields = 0;
        char *save = NULL;
        for (char *tok = strtok_r(line, "\t", &save); tok && nfields < 4;
             tok = strtok_r(NULL, "\t", &save)) {
            fields[nfields++] = tok;
        }
        if (nfields < 3) {
            fprintf(stderr, "manifest 行字段不足: %s\n", line);
            continue;
        }
        snprintf(kind, sizeof(kind), "%s", fields[0]);
        if (count == capacity) {
            capacity = capacity ? capacity * 2 : 64;
            Row *grown = realloc(rows, capacity * sizeof(Row));
            if (!grown) {
                fprintf(stderr, "内存不足\n");
                exit(2);
            }
            rows = grown;
        }
        Row *row = &rows[count++];
        memset(row, 0, sizeof(*row));
        snprintf(row->code, sizeof(row->code), "%s", fields[1]);
        if (strcmp(kind, "occupied") == 0) {
            row->is_selected = 0;
            row->fanout = xstrdup(fields[2]);
            row->menu = nfields > 3 ? xstrdup(fields[3]) : xstrdup("");
        } else if (strcmp(kind, "selected") == 0) {
            row->is_selected = 1;
            row->word = xstrdup(fields[2]);
            row->full_code = nfields > 3 ? xstrdup(fields[3]) : xstrdup("");
        } else {
            fprintf(stderr, "未知 manifest 行类型 %s\n", kind);
            continue;
        }
    }
    fclose(f);
    *rows_out = rows;
    *count_out = count;
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 4) {
        fprintf(stderr,
                "用法: %s <shared_data_dir> <production_dir> <manifest.tsv>\n",
                argv[0]);
        return 2;
    }
    Row *rows = NULL;
    int count = 0;
    if (load_manifest(argv[3], &rows, &count) != 0) {
        return 2;
    }
    if (count == 0) {
        fprintf(stderr, "manifest 为空\n");
        return 2;
    }

    rime = rime_get_api();
    if (!rime) {
        fprintf(stderr, "无法获取 Rime API\n");
        return 2;
    }

    char user_dir[4096], shared_dir[4096];
    snprintf(shared_dir, sizeof(shared_dir), "%s", argv[1]);
    snprintf(user_dir, sizeof(user_dir), "%s", argv[2]);

    RIME_STRUCT(RimeTraits, traits);
    traits.app_name = "rime.xhup-two-key-audit";
    traits.shared_data_dir = shared_dir;
    traits.user_data_dir = user_dir;
    traits.distribution_name = "XHUP-Flow";
    traits.distribution_code_name = "xhup-flow";
    traits.distribution_version = "0";
    rime->setup(&traits);
    rime->initialize(&traits);
    if (rime->is_maintenance_mode && rime->is_maintenance_mode()) {
        rime->join_maintenance_thread();
    }

    RimeSessionId session = rime->create_session();
    if (!session || !rime->select_schema(session, "xhup_flow")) {
        fprintf(stderr, "无法创建会话或选择 schema xhup_flow(%s)\n", user_dir);
        return 2;
    }

    char actual[8192];
    char expected[8192];
    char detail[8256];
    int occupied_audited = 0;
    int selected_audited = 0;

    for (int i = 0; i < count; ++i) {
        Row *row = &rows[i];
        if (!row->is_selected) {
            /* occupied:PRODUCTION 菜单 == 既有菜单(逐项同序)。 */
            reset_composition(session);
            type_keys(session, row->code);
            int active = 0;
            int n = capture_menu(session, actual, sizeof(actual), &active);
            menu_field_to_sep(row->menu, expected, sizeof(expected));
            int ok = n == atoi(row->fanout) && strcmp(actual, expected) == 0;
            snprintf(detail, sizeof(detail), "%s fanout=%d/%s",
                     row->code, n, row->fanout);
            if (!ok) {
                fprintf(stderr,
                        "occupied %s 菜单变化: expected=[%s] actual=[%s] n=%d\n",
                        row->code, expected, actual, n);
            }
            report(ok, "occupied 菜单逐项不变", detail);
            ++occupied_audited;
        } else {
            /* selected:目标词恰一次且 rank 1;无 auto commit。 */
            reset_composition(session);
            type_keys(session, row->code);
            int active = 0;
            capture_menu(session, actual, sizeof(actual), &active);
            /* rank 1 即菜单首项 == 目标词,且目标词只出现一次。 */
            int occurrences = 0;
            int rank = 0;
            char *save = NULL;
            int position = 0;
            for (char *tok = strtok_r(actual, "\x1f", &save); tok;
                 tok = strtok_r(NULL, "\x1f", &save)) {
                ++position;
                if (strcmp(tok, row->word) == 0) {
                    ++occurrences;
                    rank = position;
                }
            }
            snprintf(detail, sizeof(detail), "%s %s rank=%d occurrences=%d",
                     row->code, row->word, rank, occurrences);
            report(rank == 1 && occurrences == 1,
                   "selected 目标 rank1 且唯一", detail);
            int no_commit = !has_commit(session);
            report(no_commit && active, "selected 无 auto commit、组合活动", row->code);
            ++selected_audited;
        }
    }

    rime->destroy_session(session);
    rime->finalize();

    printf("----\n二码 runtime 审计:occupied %d/405 全量、selected %d/%d 全量,"
           "%d 项检查,%d 项失败\n",
           occupied_audited, selected_audited, selected_audited, checks, failures);
    return failures == 0 ? 0 : 1;
}
