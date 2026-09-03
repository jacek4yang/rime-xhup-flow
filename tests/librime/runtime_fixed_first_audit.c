/* XHUP Flow FIXED_FIRST 简码层全量 runtime A/B 审计。
 *
 * 对 manifest 中的每一条 production FIXED_FIRST 简码(全量,不抽样):
 *
 * CONTROL(xhup_flow_control schema,仅 primary translator)与
 * PRODUCTION(xhup_flow schema,双 translator)两个真实 librime
 * deployment 中输入同一 shortcut 码,捕获完整候选菜单,断言:
 *
 * - analyzer manifest 的 baseline 菜单 == CONTROL 实际菜单(逐项同序);
 * - CONTROL 候选数 == manifest baseline fanout;
 * - PRODUCTION 菜单 == CONTROL 菜单 + 末尾一个目标词(严格追加);
 * - 目标词在 PRODUCTION 菜单中恰好出现一次;
 * - 两侧均无 auto commit、组合保持活动。
 *
 * 三者一致即证明:加入第二 translator 前后真实 primary 候选菜单完全不变,
 * FIXED_FIRST 目标严格位于全部既有固定候选之后(rank = fanout + 1)。
 *
 * 用法:
 *   runtime_fixed_first_audit <shared_data_dir> <control_dir>
 *                             <production_dir> <manifest.tsv>
 *
 * 两个 user dir 必须已含生成包(control 含派生 control schema)并完成
 * rime_deployer --compile,且 default.custom.yaml 中 menu/page_size 足够大
 * (测试枚举专用配置,不属于 production schema)。
 * 只使用稳定 C API;无第三方测试框架;不访问用户真实 Rime 目录。
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <rime_api.h>

#define SEP '\x1f'

typedef struct {
    char code[16];
    char *word;
    int fanout;
    int expected_rank;
    char *baseline_menu; /* manifest 字段:逗号分隔 */
    char *control_menu;  /* CONTROL 实际:\x1f 分隔 */
    int control_len;
} Row;

static RimeApi *rime;
static int failures = 0;

/* 捕获当前菜单:\x1f 分隔文本序列写入 buf,返回候选数。 */
static int capture_menu(RimeSessionId session, char *buf, size_t size,
                        int *committed, int *composition_active) {
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
    *committed = 0;
    RIME_STRUCT(RimeCommit, commit);
    if (rime->get_commit(session, &commit)) {
        *committed = commit.text != NULL && commit.text[0] != '\0';
        rime->free_commit(&commit);
    }
    return n;
}

static void type_keys(RimeSessionId session, const char *keys) {
    for (const char *p = keys; *p; ++p) {
        rime->process_key(session, *p, 0);
    }
}

/* 初始化一个 deployment 并选择 schema;失败返回 0。 */
static int open_session(const char *shared, const char *user_dir,
                        const char *dist_name, const char *app_name,
                        const char *schema_id, RimeSessionId *out) {
    RIME_STRUCT(RimeTraits, traits);
    traits.shared_data_dir = shared;
    traits.user_data_dir = user_dir;
    traits.distribution_name = dist_name;
    traits.distribution_code_name = dist_name;
    traits.distribution_version = "0";
    traits.app_name = app_name;
    rime->setup(&traits);
    rime->initialize(&traits);
    if (rime->is_maintenance_mode && rime->is_maintenance_mode()) {
        rime->join_maintenance_thread();
    }
    RimeSessionId session = rime->create_session();
    if (!session || !rime->select_schema(session, schema_id)) {
        fprintf(stderr, "无法创建会话或选择 schema %s(%s)\n", schema_id,
                user_dir);
        rime->finalize();
        return 0;
    }
    *out = session;
    return 1;
}

/* 文本在 \x1f 分隔菜单中出现的次数。 */
static int count_occurrences(const char *menu, const char *word) {
    int count = 0;
    size_t len = strlen(word);
    const char *p = menu;
    while (*p) {
        const char *end = strchr(p, SEP);
        size_t item_len = end ? (size_t)(end - p) : strlen(p);
        if (item_len == len && strncmp(p, word, len) == 0) count++;
        if (!end) break;
        p = end + 1;
    }
    return count;
}

int main(int argc, char **argv) {
    if (argc != 5) {
        fprintf(stderr,
                "用法: %s <shared_data_dir> <control_dir> <production_dir> "
                "<manifest.tsv>\n",
                argv[0]);
        return 2;
    }

    FILE *fp = fopen(argv[4], "r");
    if (!fp) {
        fprintf(stderr, "无法打开 manifest %s\n", argv[4]);
        return 2;
    }
    Row *rows = NULL;
    size_t count = 0, cap = 0;
    char line[4096];
    while (fgets(line, sizeof(line), fp)) {
        if (line[0] == '#') continue;
        char *nl = strchr(line, '\n');
        if (nl) *nl = '\0';
        if (count == cap) {
            cap = cap ? cap * 2 : 4096;
            rows = realloc(rows, cap * sizeof(Row));
            if (!rows) return 2;
        }
        Row *row = &rows[count];
        memset(row, 0, sizeof(*row));
        char *save = NULL;
        char *code = strtok_r(line, "\t", &save);
        char *word = strtok_r(NULL, "\t", &save);
        char *fanout = strtok_r(NULL, "\t", &save);
        char *expected_rank = strtok_r(NULL, "\t", &save);
        char *class_label = strtok_r(NULL, "\t", &save);
        char *menu = strtok_r(NULL, "\t", &save);
        if (!code || !word || !fanout || !expected_rank || !class_label || !menu) {
            fprintf(stderr, "manifest 行字段不足: %s\n", line);
            return 2;
        }
        snprintf(row->code, sizeof(row->code), "%s", code);
        row->word = strdup(word);
        row->fanout = atoi(fanout);
        row->expected_rank = atoi(expected_rank);
        /* manifest 的逗号分隔菜单换成内部 \x1f 分隔,便于逐项比较。 */
        row->baseline_menu = strdup(menu);
        for (char *p = row->baseline_menu; *p; ++p) {
            if (*p == ',') *p = SEP;
        }
        if (!row->word || !row->baseline_menu) return 2;
        count++;
    }
    fclose(fp);
    printf("manifest: %zu 条 production FIXED_FIRST 简码\n", count);

    rime = rime_get_api();
    if (!rime) {
        fprintf(stderr, "rime_get_api 失败\n");
        return 2;
    }

    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);

    /* ---- CONTROL 趟:验证 analyzer manifest 与真实 primary 菜单一致 ---- */
    RimeSessionId session;
    if (!open_session(argv[1], argv[2], "xhup-ff-audit-control",
                      "xhup.ff_audit_control", "xhup_flow_control", &session)) {
        return 2;
    }
    int control_mismatch = 0, control_fanout_mismatch = 0, control_commit = 0;
    char *buf = malloc(1 << 16);
    if (!buf) return 2;
    for (size_t i = 0; i < count; ++i) {
        Row *row = &rows[i];
        type_keys(session, row->code);
        int committed, active;
        int len = capture_menu(session, buf, 1 << 16, &committed, &active);
        if (strcmp(buf, row->baseline_menu) != 0) {
            if (control_mismatch < 10) {
                printf("FAIL  CONTROL %s: 菜单 [%s] != manifest [%s]\n", row->code,
                       buf, row->baseline_menu);
            }
            control_mismatch++;
        }
        if (len != row->fanout) {
            if (control_fanout_mismatch < 10) {
                printf("FAIL  CONTROL %s: 候选数 %d != fanout %d\n", row->code, len,
                       row->fanout);
            }
            control_fanout_mismatch++;
        }
        if (committed || !active) {
            if (control_commit < 10) {
                printf("FAIL  CONTROL %s: auto commit=%d active=%d\n", row->code,
                       committed, active);
            }
            control_commit++;
        }
        row->control_menu = strdup(buf);
        row->control_len = len;
        if (!row->control_menu) return 2;
        rime->clear_composition(session);
    }
    rime->destroy_session(session);
    rime->finalize();

    /* ---- PRODUCTION 趟:菜单 == CONTROL + 末尾目标词 ---- */
    if (!open_session(argv[1], argv[3], "xhup-ff-audit-production",
                      "xhup.ff_audit_production", "xhup_flow", &session)) {
        return 2;
    }
    int len_mismatch = 0, prefix_mismatch = 0, rank_mismatch = 0,
        top1_changes = 0, duplicates = 0, production_commit = 0;
    int *rank_hist = calloc(16, sizeof(int));
    if (!rank_hist) return 2;
    int max_rank = 0;
    for (size_t i = 0; i < count; ++i) {
        Row *row = &rows[i];
        type_keys(session, row->code);
        int committed, active;
        int len = capture_menu(session, buf, 1 << 16, &committed, &active);
        rime->clear_composition(session);

        /* 期望菜单 = control 菜单 + \x1f + 目标词。 */
        size_t need = strlen(row->control_menu) + strlen(row->word) + 2;
        char *expected = malloc(need);
        if (!expected) return 2;
        snprintf(expected, need, "%s%c%s", row->control_menu, SEP, row->word);

        if (len != row->control_len + 1) {
            if (len_mismatch < 10) {
                printf("FAIL  PROD %s(%s): 候选数 %d != control %d + 1\n",
                       row->code, row->word, len, row->control_len);
            }
            len_mismatch++;
        }
        /* 前缀逐项相等 + 末位为目标词:整体比较。 */
        if (strcmp(buf, expected) != 0) {
            /* 区分前缀变化与名次错误,用于定位。 */
            size_t control_len = strlen(row->control_menu);
            if (strncmp(buf, row->control_menu, control_len) != 0 ||
                buf[control_len] != SEP) {
                if (prefix_mismatch < 10) {
                    printf("FAIL  PROD %s(%s): 既有候选次序变化 [%s] ⊀ [%s]\n",
                           row->code, row->word, row->control_menu, buf);
                }
                prefix_mismatch++;
            } else {
                if (rank_mismatch < 10) {
                    printf("FAIL  PROD %s(%s): 目标不在 rank %d,菜单 [%s]\n",
                           row->code, row->word, row->expected_rank, buf);
                }
                rank_mismatch++;
            }
        } else {
            int rank = row->control_len + 1;
            if (rank < 16) rank_hist[rank]++;
            if (rank > max_rank) max_rank = rank;
        }
        free(expected);
        /* top1 独立断言:production 首项必须仍是 control 首项。 */
        {
            const char *sep0 = strchr(row->control_menu, SEP);
            size_t top1_len = sep0 ? (size_t)(sep0 - row->control_menu)
                                   : strlen(row->control_menu);
            if (row->control_len == 0 ||
                strncmp(buf, row->control_menu, top1_len) != 0 ||
                (buf[top1_len] != SEP && buf[top1_len] != '\0')) {
                if (top1_changes < 10) {
                    printf("FAIL  PROD %s(%s): top1 变化,菜单 [%s]\n", row->code,
                           row->word, buf);
                }
                top1_changes++;
            }
        }
        if (count_occurrences(buf, row->word) != 1) {
            if (duplicates < 10) {
                printf("FAIL  PROD %s(%s): 目标词出现次数 != 1,菜单 [%s]\n",
                       row->code, row->word, buf);
            }
            duplicates++;
        }
        if (committed || !active) {
            if (production_commit < 10) {
                printf("FAIL  PROD %s(%s): auto commit=%d active=%d\n", row->code,
                       row->word, committed, active);
            }
            production_commit++;
        }
    }
    rime->destroy_session(session);
    rime->finalize();

    clock_gettime(CLOCK_MONOTONIC, &end);
    double elapsed = (double)(end.tv_sec - start.tv_sec) +
                     (double)(end.tv_nsec - start.tv_nsec) / 1e9;

    failures = control_mismatch + control_fanout_mismatch + control_commit +
               len_mismatch + prefix_mismatch + rank_mismatch + top1_changes +
               duplicates + production_commit;
    printf("----\n");
    printf("FIXED_FIRST 全量 A/B 审计:%zu 条,耗时 %.2fs\n", count, elapsed);
    printf("  CONTROL menu != analyzer manifest: %d\n", control_mismatch);
    printf("  CONTROL fanout != analyzer fanout: %d\n", control_fanout_mismatch);
    printf("  CONTROL auto commit/组合异常:      %d\n", control_commit);
    printf("  PRODUCTION 候选数 != control + 1:  %d\n", len_mismatch);
    printf("  PRODUCTION 既有候选次序变化:       %d\n", prefix_mismatch);
    printf("  PRODUCTION 目标 rank != fanout+1:  %d\n", rank_mismatch);
    printf("  PRODUCTION top1 变化:              %d\n", top1_changes);
    printf("  PRODUCTION 目标词重复:             %d\n", duplicates);
    printf("  PRODUCTION auto commit/组合异常:   %d\n", production_commit);
    printf("  rank 分布(成功条目):");
    for (int r = 2; r < 16; ++r) {
        if (rank_hist[r]) printf(" rank%d=%d", r, rank_hist[r]);
    }
    printf("  max=%d\n", max_rank);
    if (failures == 0) {
        printf("全部 %zu 条 PASS:CONTROL == manifest,PRODUCTION 严格追加。\n",
               count);
    }
    return failures == 0 ? 0 : 1;
}
