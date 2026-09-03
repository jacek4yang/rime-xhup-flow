/* XHUP Flow Flow 引擎 runtime 审计(组句 / 学习 / 持久化 / 静态保护)。
 *
 * 模式(每进程一个,glog 单次 initialize 限制,多模式 = 多进程):
 *
 *   static-baseline <shared> <flow_dir> <static_dir> <manifest>
 *     —— 全静态等值审计:对 manifest 的每个静态 exact code,
 *        FLOW schema(干净 userdb)菜单必须与 STATIC schema 菜单完全相等
 *        (逐项同序;证明 Flow translator 在无学习数据时不可见)。
 *
 *   static-baseline-learned <shared> <flow_dir> <manifest>
 *     —— 学习后静态审计:对 manifest 的每个静态 exact code,
 *        学习过的 FLOW schema 菜单必须保持全部既有候选原次序、原 top1、
 *        无可见重复(动态候选只允许追加在静态组之后)。
 *
 *   learning <shared> <user_dir> <script>
 *     —— 学习会话:按脚本输入/上屏(如 "code<TAB>candidate_rank"),
 *        验证动态层重排(只允许在 Flow 层内部),并把学习后状态留在
 *        user_dir(供 restart 模式验证持久化)。
 *
 *   restart <shared> <user_dir> <checks>
 *     —— 重启验证:全新进程加载同一 user_dir,验证学习结果仍然存在
 *        (动态候选 / 学习短语 / 用户词典短语)。
 *
 *   sentence <shared> <dir> <sentences>
 *     —— 组句审计:每个句子条目(code, expected)在 FLOW schema 下
 *        产生目标句子候选;报告名次与分段。
 *
 * 静态保护不变量(全部模式强制):
 *   uij → [铈,鼫,时间];uijm → 时间 top1;uj/ujm 不含 时间;
 *   q → 去;wo → 我。任何模式下违反即 FAIL。
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <rime_api.h>

#define SEP '\x1f'
#define TEST_MENU_PAGE_SIZE 500
#define MAX_MENU 4096
#define MAX_LINE 65536

static int failures = 0;
static int checks = 0;

static void report(int ok, const char *name, const char *detail) {
    ++checks;
    if (!ok) {
        ++failures;
        /* 失败才打印完整细节,CI 日志保持可读(§82)。 */
        printf("%s  %s%s%s\n", ok ? "PASS" : "FAIL", name,
               detail ? "  " : "", detail ? detail : "");
    } else {
        printf("%s  %s\n", ok ? "PASS" : "FAIL", name);
    }
}

static RimeApi *rime;

static void type_keys(RimeSessionId session, const char *keys) {
    for (const char *p = keys; *p; ++p) {
        rime->process_key(session, *p, 0);
    }
}

/* 捕获当前菜单:SEP 分隔文本序列写入 buf,返回候选数。 */
static int capture_menu(RimeSessionId session, char *buf, size_t size) {
    int n = 0;
    buf[0] = '\0';
    RIME_STRUCT(RimeContext, context);
    if (rime->get_context(session, &context)) {
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
    (void)has_commit(session);
}

static int menu_contains(const char *menu, const char *target) {
    char needle[MAX_MENU];
    snprintf(needle, sizeof(needle), "%c%s%c", SEP, target, SEP);
    char padded[MAX_MENU + 2];
    snprintf(padded, sizeof(padded), "%c%s%c", SEP, menu, SEP);
    return strstr(padded, needle) != NULL;
}

static int menu_rank(const char *menu, const char *target) {
    int rank = 0;
    char copy[MAX_MENU];
    snprintf(copy, sizeof(copy), "%s", menu);
    char *save = NULL;
    int position = 0;
    for (char *tok = strtok_r(copy, "\x1f", &save); tok;
         tok = strtok_r(NULL, "\x1f", &save)) {
        ++position;
        if (strcmp(tok, target) == 0) {
            rank = position;
            break;
        }
    }
    return rank;
}

static int menu_count(const char *menu, const char *target) {
    int count = 0;
    char copy[MAX_MENU];
    snprintf(copy, sizeof(copy), "%s", menu);
    char *save = NULL;
    for (char *tok = strtok_r(copy, "\x1f", &save); tok;
         tok = strtok_r(NULL, "\x1f", &save)) {
        if (strcmp(tok, target) == 0) {
            ++count;
        }
    }
    return count;
}

/* 学习脚本断言 detail 的截断安全构造(code/text 有限长,menu 限 160)。 */
static void make_detail(char *detail, size_t size, const char *code,
                        const char *text, int rank, int count,
                        const char *menu) {
    snprintf(detail, size, "code=%.64s text=%.64s rank=%d count=%d menu=[%.160s]",
             code, text, rank, count, menu);
}

/* 初始化 deployment 并选择 schema;失败返回 0。 */
static int open_session(const char *shared, const char *user_dir,
                        const char *schema_id, RimeSessionId *out) {
    RIME_STRUCT(RimeTraits, traits);
    traits.app_name = "rime.xhup-flow-audit";
    traits.shared_data_dir = shared;
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
    if (!session || !rime->select_schema(session, schema_id)) {
        fprintf(stderr, "无法创建会话或选择 schema %s(%s)\n", schema_id,
                user_dir);
        return 0;
    }
    *out = session;
    return 1;
}

/* 静态保护不变量(全部模式强制;§83 既有哨兵)。 */
static void verify_frozen_sentinels(RimeSessionId session) {
    char menu[MAX_MENU];
    char detail[256];

    reset_composition(session);
    type_keys(session, "uij");
    capture_menu(session, menu, sizeof(menu));
    snprintf(detail, sizeof(detail), "menu=[%.96s]", menu);
    report(menu_rank(menu, "铈") == 1 && menu_rank(menu, "鼫") == 2 &&
               menu_rank(menu, "时间") == 3,
           "冻结哨兵 uij → [铈,鼫,时间]", detail);
    reset_composition(session);

    type_keys(session, "uijm");
    capture_menu(session, menu, sizeof(menu));
    snprintf(detail, sizeof(detail), "menu=[%.96s]", menu);
    report(menu_rank(menu, "时间") == 1, "冻结哨兵 uijm → 时间 top1", detail);
    reset_composition(session);

    type_keys(session, "uj");
    capture_menu(session, menu, sizeof(menu));
    snprintf(detail, sizeof(detail), "menu=[%.96s]", menu);
    report(!menu_contains(menu, "时间"), "冻结哨兵 uj 不含 时间", detail);
    reset_composition(session);

    type_keys(session, "ujm");
    capture_menu(session, menu, sizeof(menu));
    snprintf(detail, sizeof(detail), "menu=[%.96s]", menu);
    report(!menu_contains(menu, "时间"), "冻结哨兵 ujm 不含 时间", detail);
    reset_composition(session);

    type_keys(session, "q");
    capture_menu(session, menu, sizeof(menu));
    snprintf(detail, sizeof(detail), "menu=[%.96s]", menu);
    report(menu_rank(menu, "去") == 1, "冻结哨兵 q → 去", detail);
    reset_composition(session);

    type_keys(session, "wo");
    capture_menu(session, menu, sizeof(menu));
    snprintf(detail, sizeof(detail), "menu=[%.96s]", menu);
    report(menu_rank(menu, "我") == 1, "冻结哨兵 wo → 我", detail);
    reset_composition(session);

    type_keys(session, "jqu");
    capture_menu(session, menu, sizeof(menu));
    snprintf(detail, sizeof(detail), "menu=[%.96s]", menu);
    report(menu_rank(menu, "就是") == 1, "冻结哨兵 jqu → 就是", detail);
    reset_composition(session);
}

/* 把 manifest 菜单字段(| 分隔)转换为 SEP 分隔格式。 */
static void menu_field_to_sep(const char *field, char *buf, size_t size) {
    size_t out = 0;
    for (const char *p = field; *p && out + 1 < size; ++p) {
        buf[out++] = (*p == '|') ? SEP : *p;
    }
    buf[out] = '\0';
}

/* 检查菜单内可见重复:同一文本出现 > 1 次。 */
static int menu_has_duplicate(const char *menu) {
    char copy[MAX_MENU];
    snprintf(copy, sizeof(copy), "%s", menu);
    char *save = NULL;
    char *seen[512];
    int n = 0;
    for (char *tok = strtok_r(copy, "\x1f", &save); tok && n < 512;
         tok = strtok_r(NULL, "\x1f", &save)) {
        for (int i = 0; i < n; ++i) {
            if (strcmp(seen[i], tok) == 0) {
                return 1;
            }
        }
        seen[n++] = tok;
    }
    return 0;
}

/* ── 模式:baseline-capture / baseline-compare(全静态等值,两趟进程) ──
 *
 * glog 不允许同进程二次 initialize(CI librime 1.10 CHECK 失败),
 * STATIC 与 FLOW 两个 deployment 因此拆成两趟独立进程,菜单经
 * capture 文件衔接(与 FIXED_FIRST 审计同构):
 *
 *   baseline-capture <shared> <static_dir> <manifest> <capture_out>
 *     —— STATIC schema 逐码捕获完整菜单(同时断言 == manifest)。
 *   baseline-compare <shared> <flow_dir> <manifest> <capture_in>
 *     —— FLOW schema(干净 userdb)逐码断言 == capture(== manifest),
 *        并检查可见重复。
 */
static int run_baseline_capture(const char *shared, const char *static_dir,
                                const char *manifest, const char *capture_out) {
    FILE *f = fopen(manifest, "r");
    FILE *cap = fopen(capture_out, "w");
    if (!f || !cap) {
        fprintf(stderr, "无法打开 %s / %s\n", manifest, capture_out);
        return 2;
    }
    RimeSessionId session;
    if (!open_session(shared, static_dir, "xhup_flow_static", &session)) {
        return 2;
    }
    char line[MAX_LINE];
    char menu[MAX_MENU], expected[MAX_MENU];
    char detail[2 * MAX_MENU];
    long codes = 0, mismatches = 0;
    while (fgets(line, sizeof(line), f)) {
        size_t len = strlen(line);
        while (len && (line[len - 1] == '\n' || line[len - 1] == '\r')) {
            line[--len] = '\0';
        }
        if (line[0] == '#' || line[0] == '\0') {
            continue;
        }
        char *tab = strchr(line, '\t');
        if (!tab) {
            continue;
        }
        *tab = '\0';
        menu_field_to_sep(tab + 1, expected, sizeof(expected));

        reset_composition(session);
        type_keys(session, line);
        capture_menu(session, menu, sizeof(menu));
        ++codes;
        fprintf(cap, "%s\t%s\n", line, menu);
        if (strcmp(menu, expected) != 0) {
            ++mismatches;
            snprintf(detail, sizeof(detail),
                     "code=%s manifest=[%.128s] static=[%.128s]", line,
                     expected, menu);
            report(0, "STATIC == manifest", detail);
        }
    }
    fclose(f);
    fclose(cap);
    rime->destroy_session(session);
    rime->finalize();
    printf("----\nbaseline-capture(STATIC):%ld codes,manifest 不一致 %ld\n",
           codes, mismatches);
    return mismatches == 0 ? 0 : 1;
}

static int run_baseline_compare(const char *shared, const char *flow_dir,
                                const char *manifest, const char *capture_in) {
    FILE *f = fopen(manifest, "r");
    FILE *cap = fopen(capture_in, "r");
    if (!f || !cap) {
        fprintf(stderr, "无法打开 %s / %s\n", manifest, capture_in);
        return 2;
    }
    RimeSessionId session;
    if (!open_session(shared, flow_dir, "xhup_flow", &session)) {
        return 2;
    }
    char line[MAX_LINE], capline[MAX_LINE];
    char menu[MAX_MENU], expected[MAX_MENU], static_menu[MAX_MENU];
    char detail[2 * MAX_MENU];
    long codes = 0, mismatches = 0, duplicates = 0;
    clock_t start = clock();
    while (fgets(line, sizeof(line), f)) {
        if (line[0] == '#' || line[0] == '\0' || line[0] == '\n') {
            continue; /* manifest 头部;capture 无对应行。 */
        }
        size_t len = strlen(line);
        while (len && (line[len - 1] == '\n' || line[len - 1] == '\r')) {
            line[--len] = '\0';
        }
        char *tab = strchr(line, '\t');
        if (!tab) {
            continue;
        }
        *tab = '\0';
        menu_field_to_sep(tab + 1, expected, sizeof(expected));
        /* capture 行:码\tSEP 分隔菜单(仅数据行,逐行与 manifest 配对)。 */
        if (!fgets(capline, sizeof(capline), cap)) {
            fprintf(stderr, "capture 文件行数不足:code=%s\n", line);
            break;
        }
        {
            char *ctab = strchr(capline, '\t');
            if (!ctab) {
                continue;
            }
            *ctab = '\0';
            if (strcmp(capline, line) != 0) {
                fprintf(stderr, "capture 错位:manifest=%s capture=%s\n", line,
                        capline);
                break;
            }
            snprintf(static_menu, sizeof(static_menu), "%s", ctab + 1);
            size_t clen = strlen(static_menu);
            while (clen &&
                   (static_menu[clen - 1] == '\n' ||
                    static_menu[clen - 1] == '\r')) {
                static_menu[--clen] = '\0';
            }
        }

        reset_composition(session);
        type_keys(session, line);
        capture_menu(session, menu, sizeof(menu));
        ++codes;
        int equal = strcmp(menu, expected) == 0 && strcmp(menu, static_menu) == 0;
        int dup = menu_has_duplicate(menu);
        if (!equal) {
            ++mismatches;
        }
        if (dup) {
            ++duplicates;
        }
        if (!equal || dup) {
            snprintf(detail, sizeof(detail),
                     "code=%s expected=[%.128s] flow=[%.128s]", line, expected,
                     menu);
            report(0, "FLOW(干净) == STATIC", detail);
        }
    }
    fclose(f);
    fclose(cap);

    verify_frozen_sentinels(session);
    rime->destroy_session(session);
    rime->finalize();

    double elapsed = (double)(clock() - start) / CLOCKS_PER_SEC;
    printf("----\n全静态等值审计(干净 userdb):%ld codes,mismatch %ld,"
           "重复 %ld,%.2fs\n",
           codes, mismatches, duplicates, elapsed);
    return (mismatches == 0 && duplicates == 0) ? 0 : 1;
}

/* ── 模式:static-baseline-learned(学习后静态审计) ──
 *
 * 对 manifest 的每个静态 exact code(在学习发生之后):
 * FLOW 菜单的前缀必须等于完整静态菜单(全部既有候选原次序),
 * 追加候选只允许出现在静态组之后;无可见重复;top1 不变。
 */
static int run_static_baseline_learned(const char *shared,
                                       const char *flow_dir,
                                       const char *manifest) {
    FILE *f = fopen(manifest, "r");
    if (!f) {
        fprintf(stderr, "无法打开 manifest %s\n", manifest);
        return 2;
    }
    RimeSessionId session;
    if (!open_session(shared, flow_dir, "xhup_flow", &session)) {
        return 2;
    }

    char line[MAX_LINE];
    char flow_menu[MAX_MENU], expected[MAX_MENU];
    char detail[2 * MAX_MENU];
    long codes = 0, ordering_changes = 0, top1_changes = 0, duplicates = 0;
    clock_t start = clock();

    while (fgets(line, sizeof(line), f)) {
        size_t len = strlen(line);
        while (len && (line[len - 1] == '\n' || line[len - 1] == '\r')) {
            line[--len] = '\0';
        }
        if (line[0] == '#' || line[0] == '\0') {
            continue;
        }
        char *tab = strchr(line, '\t');
        if (!tab) {
            continue;
        }
        *tab = '\0';
        const char *code = line;
        menu_field_to_sep(tab + 1, expected, sizeof(expected));

        reset_composition(session);
        type_keys(session, code);
        capture_menu(session, flow_menu, sizeof(flow_menu));
        ++codes;

        /* 静态菜单必须是 FLOW 菜单的前缀(逐项同序)。 */
        size_t expected_len = strlen(expected);
        int prefix_ok = strncmp(flow_menu, expected, expected_len) == 0;
        /* 前缀后要么结束,要么恰是 SEP(追加动态候选)。 */
        if (prefix_ok && flow_menu[expected_len] != '\0' &&
            flow_menu[expected_len] != SEP) {
            prefix_ok = 0;
        }
        int top1_ok = 1;
        int dup = 0;
        {
            char copy[MAX_MENU];
            snprintf(copy, sizeof(copy), "%s", flow_menu);
            char *save = NULL;
            char *seen[512];
            int n = 0;
            for (char *tok = strtok_r(copy, "\x1f", &save); tok && n < 512;
                 tok = strtok_r(NULL, "\x1f", &save)) {
                for (int i = 0; i < n; ++i) {
                    if (strcmp(seen[i], tok) == 0) {
                        dup = 1;
                        break;
                    }
                }
                if (dup) {
                    break;
                }
                seen[n++] = tok;
            }
        }
        if (!prefix_ok) {
            ++ordering_changes;
        }
        if (!top1_ok) {
            ++top1_changes;
        }
        if (dup) {
            ++duplicates;
        }
        if (!prefix_ok || !top1_ok || dup) {
            snprintf(detail, sizeof(detail),
                     "code=%s expected=[%.128s] flow=[%.128s]", code, expected,
                     flow_menu);
            report(0, "学习后静态次序", detail);
        }
    }
    fclose(f);

    verify_frozen_sentinels(session);

    rime->destroy_session(session);
    rime->finalize();

    double elapsed = (double)(clock() - start) / CLOCKS_PER_SEC;
    printf("----\n学习后静态审计:%ld codes,次序变化 %ld,top1 变化 %ld,"
           "可见重复 %ld,%.2fs\n",
           codes, ordering_changes, top1_changes, duplicates, elapsed);
    return (ordering_changes == 0 && top1_changes == 0 && duplicates == 0) ? 0
                                                                           : 1;
}

/* ── 模式:learning(学习会话,按脚本) ──
 *
 * 脚本行(三种动作):
 *   type <code>          —— 输入 code(不动作)
 *   commit <code> <rank> —— 输入 code 并选择第 rank 个候选上屏
 *   check <code> <text> <expectation> —— 断言:
 *        contains / first / absent / count=1
 */
static int run_learning(const char *shared, const char *user_dir,
                        const char *script) {
    FILE *f = fopen(script, "r");
    if (!f) {
        fprintf(stderr, "无法打开 script %s\n", script);
        return 2;
    }
    RimeSessionId session;
    if (!open_session(shared, user_dir, "xhup_flow", &session)) {
        return 2;
    }
    char line[MAX_LINE];
    char menu[MAX_MENU];
    char detail[MAX_MENU + 128];
    while (fgets(line, sizeof(line), f)) {
        size_t len = strlen(line);
        while (len && (line[len - 1] == '\n' || line[len - 1] == '\r')) {
            line[--len] = '\0';
        }
        if (line[0] == '#' || line[0] == '\0') {
            continue;
        }
        char *save = NULL;
        char *action = strtok_r(line, " \t", &save);
        if (!action) {
            continue;
        }
        if (strcmp(action, "type") == 0) {
            char *code = strtok_r(NULL, " \t", &save);
            if (!code) {
                continue;
            }
            reset_composition(session);
            type_keys(session, code);
        } else if (strcmp(action, "commit") == 0) {
            char *code = strtok_r(NULL, " \t", &save);
            char *rank_str = strtok_r(NULL, " \t", &save);
            if (!code || !rank_str) {
                continue;
            }
            reset_composition(session);
            type_keys(session, code);
            int rank = atoi(rank_str);
            if (rank < 1) {
                rank = 1;
            }
            if (!rime->select_candidate(session, rank - 1)) {
                snprintf(detail, sizeof(detail), "code=%s rank=%d", code, rank);
                report(0, "commit select_candidate", detail);
            }
            /* 取走 commit 事件。 */
            (void)has_commit(session);
        } else if (strcmp(action, "check") == 0) {
            char *code = strtok_r(NULL, " \t", &save);
            char *text = strtok_r(NULL, " \t", &save);
            char *expectation = strtok_r(NULL, " \t", &save);
            if (!code || !text || !expectation) {
                continue;
            }
            reset_composition(session);
            type_keys(session, code);
            capture_menu(session, menu, sizeof(menu));
            int rank = menu_rank(menu, text);
            int count = menu_count(menu, text);
            int ok = 0;
            if (strcmp(expectation, "first") == 0) {
                ok = rank == 1;
            } else if (strcmp(expectation, "contains") == 0) {
                ok = rank > 0;
            } else if (strcmp(expectation, "absent") == 0) {
                ok = rank == 0;
            } else if (strcmp(expectation, "count=1") == 0) {
                ok = count == 1;
            } else {
                ok = 0;
            }
            make_detail(detail, sizeof(detail), code, text, rank, count, menu);
            report(ok, expectation, detail);
            reset_composition(session);
        }
    }
    fclose(f);
    verify_frozen_sentinels(session);
    rime->destroy_session(session);
    rime->finalize();
    printf("----\n学习会话完成,%d 项检查,%d 项失败\n", checks, failures);
    return failures == 0 ? 0 : 1;
}

/* ── 模式:sentence(组句审计) ──
 *
 * sentences 行:`码\t期望句子文本`。
 * 输入完整码,断言菜单包含期望句子(报告名次;无静态 exact 候选时
 * 通常应为 Flow 输出的 rank 1)。
 */
static int run_sentence(const char *shared, const char *dir,
                        const char *sentences) {
    FILE *f = fopen(sentences, "r");
    if (!f) {
        fprintf(stderr, "无法打开 sentences %s\n", sentences);
        return 2;
    }
    RimeSessionId session;
    if (!open_session(shared, dir, "xhup_flow", &session)) {
        return 2;
    }
    char line[MAX_LINE];
    char menu[MAX_MENU];
    char detail[MAX_MENU + 128];
    while (fgets(line, sizeof(line), f)) {
        size_t len = strlen(line);
        while (len && (line[len - 1] == '\n' || line[len - 1] == '\r')) {
            line[--len] = '\0';
        }
        if (line[0] == '#' || line[0] == '\0') {
            continue;
        }
        char *tab = strchr(line, '\t');
        if (!tab) {
            continue;
        }
        *tab = '\0';
        const char *code = line;
        const char *expected = tab + 1;
        reset_composition(session);
        type_keys(session, code);
        capture_menu(session, menu, sizeof(menu));
        int rank = menu_rank(menu, expected);
        int count = menu_count(menu, expected);
        make_detail(detail, sizeof(detail), code, expected, rank, count, menu);
        report(count == 1 && rank > 0, "句子候选", detail);
        /* 组合必须保持活动(无 auto commit)。 */
        report(!has_commit(session), "句子无 auto commit", code);
        reset_composition(session);
    }
    fclose(f);
    verify_frozen_sentinels(session);
    rime->destroy_session(session);
    rime->finalize();
    printf("----\n组句审计完成,%d 项检查,%d 项失败\n", checks, failures);
    return failures == 0 ? 0 : 1;
}

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr,
                "用法: %s <mode> <shared> [args...]\n"
                "  mode baseline-capture <shared> <static_dir> <manifest> <capture_out>\n"
                "  mode baseline-compare <shared> <flow_dir> <manifest> <capture_in>\n"
                "  mode static-baseline-learned <shared> <flow_dir> <manifest>\n"
                "  mode learning <shared> <user_dir> <script>\n"
                "  mode sentence <shared> <dir> <sentences>\n",
                argv[0]);
        return 2;
    }
    rime = rime_get_api();
    if (!rime) {
        fprintf(stderr, "无法获取 Rime API\n");
        return 2;
    }
    const char *mode = argv[1];
    const char *shared = argv[2];
    if (strcmp(mode, "baseline-capture") == 0 && argc == 6) {
        return run_baseline_capture(shared, argv[3], argv[4], argv[5]);
    }
    if (strcmp(mode, "baseline-compare") == 0 && argc == 6) {
        return run_baseline_compare(shared, argv[3], argv[4], argv[5]);
    }
    if (strcmp(mode, "static-baseline-learned") == 0 && argc == 5) {
        return run_static_baseline_learned(shared, argv[3], argv[4]);
    }
    if (strcmp(mode, "learning") == 0 && argc == 5) {
        return run_learning(shared, argv[3], argv[4]);
    }
    if (strcmp(mode, "sentence") == 0 && argc == 5) {
        return run_sentence(shared, argv[3], argv[4]);
    }
    fprintf(stderr, "未知模式或参数不足: %s\n", mode);
    return 2;
}
