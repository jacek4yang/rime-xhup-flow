/* XHUP Flow canonical v2 librime session 冒烟。
 *
 * 全量 exact-code 菜单由 runtime_flow_audit.c 对 static-menu.manifest
 * 逐项验证;这里锁定真实 session、传统别名、merged ranking 与 prefix
 * continuation。只访问隔离的测试 user dir。
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rime_api.h>

static int failures = 0;
static int checks = 0;
static RimeApi *rime;
static RimeSessionId session;

static void report(int ok, const char *name, const char *detail) {
    checks++;
    if (!ok) failures++;
    printf("%s  %s%s%s%s\n", ok ? "PASS" : "FAIL", name,
           detail ? "  (" : "", detail ? detail : "", detail ? ")" : "");
}

static void type_keys(const char *keys) {
    for (const char *p = keys; *p; ++p) {
        if (!rime->process_key(session, *p, 0)) {
            printf("WARN  按键 %c 未被处理\n", *p);
        }
    }
}

static int candidate_rank(const char *target, int *num_candidates) {
    RIME_STRUCT(RimeContext, context);
    int rank = 0;
    *num_candidates = 0;
    if (rime->get_context(session, &context)) {
        *num_candidates = context.menu.num_candidates;
        for (int i = 0; i < context.menu.num_candidates; ++i) {
            if (context.menu.candidates[i].text &&
                strcmp(context.menu.candidates[i].text, target) == 0) {
                rank = i + 1;
                break;
            }
        }
        rime->free_context(&context);
    }
    return rank;
}

static int has_commit(void) {
    RIME_STRUCT(RimeCommit, commit);
    int committed = 0;
    if (rime->get_commit(session, &commit)) {
        committed = commit.text != NULL && commit.text[0] != '\0';
        rime->free_commit(&commit);
    }
    return committed;
}

static int has_active_composition(void) {
    RIME_STRUCT(RimeContext, context);
    int active = 0;
    if (rime->get_context(session, &context)) {
        active = context.composition.length > 0;
        rime->free_context(&context);
    }
    return active;
}

static void reset_composition(void) {
    rime->clear_composition(session);
    (void)has_commit();
}

static void check_only(const char *name, const char *target, int require_first) {
    int n = 0;
    int rank = candidate_rank(target, &n);
    char detail[64];
    snprintf(detail, sizeof(detail), "rank=%d, candidates=%d", rank, n);
    report(require_first ? rank == 1 : rank > 0, name, detail);
}

static void expect_menu(const char *keys, const char *target, int require_first) {
    char name[160];
    snprintf(name, sizeof(name), "%s → 菜单含 %s%s", keys, target,
             require_first ? "(首个)" : "");
    type_keys(keys);
    check_only(name, target, require_first);
    snprintf(name, sizeof(name), "%s → 无 auto commit", keys);
    report(!has_commit() && has_active_composition(), name, NULL);
}

static void expect_absent(const char *keys, const char *target) {
    char name[128];
    type_keys(keys);
    int n = 0;
    int rank = candidate_rank(target, &n);
    snprintf(name, sizeof(name), "%s → 菜单不含 %s", keys, target);
    report(rank == 0, name, NULL);
}

static void expect_commit_first(const char *label, const char *target) {
    char name[128];
    int ok = 0;
    if (rime->select_candidate(session, 0)) {
        RIME_STRUCT(RimeCommit, commit);
        if (rime->get_commit(session, &commit)) {
            ok = commit.text && strcmp(commit.text, target) == 0;
            rime->free_commit(&commit);
        }
    }
    snprintf(name, sizeof(name), "%s → 选择后上屏 %s", label, target);
    report(ok, name, NULL);
}

static void expect_exact_order(const char *keys, const char *const *expected,
                               int expected_len) {
    char actual[512] = "";
    char detail[640];
    char name[128];
    int n = 0;
    int active = 0;
    type_keys(keys);
    RIME_STRUCT(RimeContext, context);
    if (rime->get_context(session, &context)) {
        active = context.composition.length > 0;
        n = context.menu.num_candidates;
        for (int i = 0; i < n; ++i) {
            strncat(actual, i ? "," : "", sizeof(actual) - strlen(actual) - 1);
            strncat(actual, context.menu.candidates[i].text,
                    sizeof(actual) - strlen(actual) - 1);
        }
        rime->free_context(&context);
    }
    int ok = n == expected_len;
    for (int i = 0; ok && i < expected_len; ++i) {
        const char *p = actual;
        for (int j = 0; j < i; ++j) {
            p = strchr(p, ',');
            if (!p) {
                ok = 0;
                break;
            }
            p++;
        }
        if (!ok) break;
        const char *end = strchr(p, ',');
        size_t len = end ? (size_t)(end - p) : strlen(p);
        ok = len == strlen(expected[i]) && strncmp(p, expected[i], len) == 0;
    }
    snprintf(name, sizeof(name), "%s → 精确候选序", keys);
    snprintf(detail, sizeof(detail), "actual=[%s]", actual);
    report(ok, name, detail);
    report(!has_commit() && active, "精确序检查无 auto commit", NULL);
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "用法: %s <shared_data_dir> <user_data_dir>\n", argv[0]);
        return 2;
    }
    rime = rime_get_api();
    if (!rime) return 2;
    RIME_STRUCT(RimeTraits, traits);
    traits.shared_data_dir = argv[1];
    traits.user_data_dir = argv[2];
    traits.distribution_name = "XHUP Flow Runtime Smoke";
    traits.distribution_code_name = "xhup-runtime-smoke";
    traits.distribution_version = "1";
    traits.app_name = "xhup.runtime_smoke";
    rime->setup(&traits);
    rime->initialize(&traits);
    if (rime->is_maintenance_mode && rime->is_maintenance_mode()) {
        rime->join_maintenance_thread();
    }
    session = rime->create_session();
    if (!session || !rime->select_schema(session, "xhup_flow")) {
        fprintf(stderr, "无法创建 session 或选择 xhup_flow\n");
        rime->finalize();
        return 2;
    }

    /* baseline 固定层。 */
    const char *const baseline[][2] = {
        {"q", "去"}, {"wo", "我"}, {"jid", "急"}, {"jumk", "橘"},
        {"womf", "我们"}, {"uurufa", "输入法"},
    };
    for (size_t i = 0; i < sizeof(baseline) / sizeof(baseline[0]); ++i) {
        expect_menu(baseline[i][0], baseline[i][1], 1);
        reset_composition();
    }

    /* P0 输入可达性：事实字符码与完全不在固定词表中的开放组合。 */
    const char *const reachability[][2] = {
        {"eiyu", "诶"}, {"ogkx", "嗯"}, {"enkx", "嗯"},
        {"tiuici", "提示词"}, {"tiogei", "提嗯诶"},
        {"enwojtdeveyhjqkeyile", "嗯我觉得这样就可以了"},
    };
    for (size_t i = 0; i < sizeof(reachability) / sizeof(reachability[0]); ++i) {
        expect_menu(reachability[i][0], reachability[i][1], 0);
        reset_composition();
    }

    /* 高频传统 alias:必须全部是 runtime rank 1。 */
    const char *const aliases[][2] = {
        {"jqu", "就是"}, {"vdc", "知道"}, {"buu", "不是"},
        {"nim", "你们"}, {"hdu", "还是"}, {"yww", "因为"},
        {"rgo", "如果"},
    };
    for (size_t i = 0; i < sizeof(aliases) / sizeof(aliases[0]); ++i) {
        expect_menu(aliases[i][0], aliases[i][1], 1);
        reset_composition();
    }

    /* 2~5 键各层 deterministic 哨兵。 */
    const char *const tiers[][2] = {
        {"yg", "一个"}, {"aaa", "啊啊啊"}, {"aajj", "安安静静"},
        {"uijiu", "实际上"},
    };
    for (size_t i = 0; i < sizeof(tiers) / sizeof(tiers[0]); ++i) {
        expect_menu(tiers[i][0], tiers[i][1], 1);
        reset_composition();
    }
    type_keys("jqu");
    expect_commit_first("jqu", "就是");
    reset_composition();

    /* 每个码长一条 prefix continuation。 */
    const char *const prefixes[][4] = {
        {"dq", "丢弃", "qi", "dqqi"},
        {"jqu", "就是", "i", "jqui"},
        {"bdwu", "百五十", "ui", "bdwuui"},
        {"baigu", "八成是", "i", "baigui"},
    };
    for (size_t i = 0; i < sizeof(prefixes) / sizeof(prefixes[0]); ++i) {
        type_keys(prefixes[i][0]);
        check_only("shortcut continuation 前", prefixes[i][1], 0);
        report(!has_commit() && has_active_composition(), "继续前无 auto commit", NULL);
        type_keys(prefixes[i][2]);
        check_only(prefixes[i][3], prefixes[i][1], 1);
        report(has_active_composition(), "继续后组合仍活动", NULL);
        reset_composition();
    }

    /* legacy IF alias 与完整码并存。 */
    expect_menu("vdc", "知道", 1);
    reset_composition();
    expect_menu("vidc", "知道", 1);
    reset_composition();
    expect_menu("abc", "安保", 1);
    reset_composition();
    expect_menu("anbc", "安保", 1);
    reset_composition();

    /* PRIMARY + FIXED_FIRST + baseline 的真实 merged menu。 */
    expect_menu("uijm", "时间", 1);
    reset_composition();
    {
        const char *const expected[] = {"时间", "史记", "实践", "事迹", "铈", "鼫"};
        expect_exact_order("uij", expected, 6);
    }
    reset_composition();
    {
        const char *const expected[] = {"不是", "布", "步上"};
        expect_exact_order("buu", expected, 3);
    }
    reset_composition();
    expect_absent("uj", "时间");
    reset_composition();

    rime->destroy_session(session);
    rime->finalize();
    printf("----\n%d 项检查,%d 项失败\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
