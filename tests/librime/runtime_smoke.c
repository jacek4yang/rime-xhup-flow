/* XHUP Flow librime runtime 冒烟测试。
 *
 * 真实 session 级验收:通过 librime C API 创建会话、逐键输入、读取候选菜单,
 * 验证词语简码层(ZERO_REGRESSION 与 FIXED_FIRST)的运行时行为:
 *
 * - 既有固定层回归(一级简码 / 2 码单字 / 3 码单字 / 4 码规范全码 / 固定词);
 * - ZR 简码 exact 输入可见,且不自动上屏(enable_completion=false 且单候选
 *   也不允许 auto commit);
 * - ZR prefix continuation:shortcut 是更长合法码的 strict prefix 时,继续输入
 *   必须能抵达完整码目标;
 * - 非 prefix 模式的 alias 与完整码共存;
 * - 「时间」:完整码 uijm 行为不变;FIXED_FIRST 使 uij 精确序为
 *   铈 → 鼫 → 时间(既有候选严格在前),ujm 不加入时间;
 * - FIXED_FIRST prefix continuation(3/4/6 键哨兵)与 reverse prefix 哨兵。
 *
 * 用法: runtime_smoke <shared_data_dir> <user_data_dir>
 * user_data_dir 必须已含生成包(7 个 yaml)并完成 rime_deployer --compile。
 *
 * 只使用稳定 C API;无第三方测试框架;不访问用户真实 Rime 目录。
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rime_api.h>

static int failures = 0;
static int checks = 0;

static void report(int ok, const char *name, const char *detail) {
    checks++;
    if (!ok) failures++;
    printf("%s  %s%s%s%s\n", ok ? "PASS" : "FAIL", name, detail ? "  (" : "",
           detail ? detail : "", detail ? ")" : "");
}

static RimeApi *rime;
static RimeSessionId session;

/* 逐键输入一个小写字母串。 */
static void type_keys(const char *keys) {
    for (const char *p = keys; *p; ++p) {
        if (!rime->process_key(session, *p, 0)) {
            printf("WARN  按键 %c 未被处理\n", *p);
        }
    }
}

/* 当前菜单中目标文本的名次(1 起始;不在菜单返回 0)。 */
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

/* 当前是否有已提交文本(auto commit 检测)。 */
static int has_commit(void) {
    RIME_STRUCT(RimeCommit, commit);
    int committed = 0;
    if (rime->get_commit(session, &commit)) {
        committed = commit.text != NULL && commit.text[0] != '\0';
        rime->free_commit(&commit);
    }
    return committed;
}

/* 当前 preedit 是否仍活动(组合未结束)。 */
static int has_active_composition(void) {
    RIME_STRUCT(RimeContext, context);
    int active = 0;
    if (rime->get_context(session, &context)) {
        active = context.composition.length > 0;
        rime->free_context(&context);
    }
    return active;
}

/* 清空组合与未取走的 commit,开始下一条输入。 */
static void reset_composition(void) {
    rime->clear_composition(session);
    (void)has_commit(); /* 取走可能残留的 commit,避免污染下一条断言 */
}

/* 只检查当前菜单(不再输入),供 prefix continuation 的继续输入步骤使用。 */
static void check_only(const char *name, const char *target, int require_first) {
    int n = 0;
    int rank = candidate_rank(target, &n);
    char detail[64];
    snprintf(detail, sizeof(detail), "rank=%d, candidates=%d", rank, n);
    report(require_first ? rank == 1 : rank > 0, name, detail);
}

/* 断言:输入 keys 后,菜单包含 target(或要求第一),无 auto commit,组合活动。 */
static void expect_menu(const char *keys, const char *target, int require_first) {
    char name[128];
    snprintf(name, sizeof(name), "%s → 菜单含 %s%s", keys, target,
             require_first ? "(首个)" : "");
    type_keys(keys);
    check_only(name, target, require_first);

    snprintf(name, sizeof(name), "%s → 无 auto commit", keys);
    report(!has_commit() && has_active_composition(), name, NULL);
}

/* 断言:菜单不包含 target。 */
static void expect_absent(const char *keys, const char *target) {
    char name[128];
    snprintf(name, sizeof(name), "%s → 菜单不含 %s", keys, target);
    type_keys(keys);
    int n = 0;
    int rank = candidate_rank(target, &n);
    char detail[64];
    snprintf(detail, sizeof(detail), "rank=%d, candidates=%d", rank, n);
    report(rank == 0, name, detail);
}

/* 断言:选择首个候选后上屏文本为 target。 */
static void expect_commit_first(const char *label, const char *target) {
    char name[128];
    snprintf(name, sizeof(name), "%s → 选择后上屏 %s", label, target);
    int ok = 0;
    if (rime->select_candidate(session, 0)) {
        RIME_STRUCT(RimeCommit, commit);
        if (rime->get_commit(session, &commit)) {
            ok = commit.text && strcmp(commit.text, target) == 0;
            rime->free_commit(&commit);
        }
    }
    report(ok, name, NULL);
}

/* 断言:输入 keys 后菜单与期望文本序列逐项、逐顺序完全一致,
 * 且无 auto commit、组合活动。 */
static void expect_exact_order(const char *keys, const char *const *expected,
                               int expected_len) {
    char name[128];
    char detail[512];
    snprintf(name, sizeof(name), "%s → 精确候选序", keys);
    type_keys(keys);
    char actual[512];
    actual[0] = '\0';
    int n = 0;
    int active = 0;
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
        /* 从 actual 中取第 i 项逐字比较(逗号分隔)。 */
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
        size_t item_len = end ? (size_t)(end - p) : strlen(p);
        ok = item_len == strlen(expected[i]) &&
             strncmp(p, expected[i], item_len) == 0;
    }
    snprintf(detail, sizeof(detail), "actual=[%s]", actual);
    report(ok, name, detail);
    snprintf(name, sizeof(name), "%s → 无 auto commit", keys);
    report(!has_commit() && active, name, NULL);
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "用法: %s <shared_data_dir> <user_data_dir>\n", argv[0]);
        return 2;
    }

    rime = rime_get_api();
    if (!rime) {
        fprintf(stderr, "rime_get_api 失败\n");
        return 2;
    }

    RIME_STRUCT(RimeTraits, traits);
    traits.shared_data_dir = argv[1];
    traits.user_data_dir = argv[2];
    traits.distribution_name = "XHUP Flow Runtime Smoke";
    traits.distribution_code_name = "xhup-runtime-smoke";
    traits.distribution_version = "0";
    traits.app_name = "xhup.runtime_smoke";
    rime->setup(&traits);
    rime->initialize(&traits);
    if (rime->is_maintenance_mode && rime->is_maintenance_mode()) {
        rime->join_maintenance_thread();
    }

    session = rime->create_session();
    if (!session) {
        fprintf(stderr, "无法创建 Rime 会话\n");
        rime->finalize();
        return 2;
    }
    if (!rime->select_schema(session, "xhup_flow")) {
        fprintf(stderr, "无法选择 schema xhup_flow\n");
        rime->destroy_session(session);
        rime->finalize();
        return 2;
    }

    /* ---- 既有固定层回归 ---- */
    expect_menu("q", "去", 1);
    reset_composition();
    expect_menu("wo", "我", 1);
    reset_composition();
    expect_menu("jid", "急", 1); /* 3 码单字哨兵 */
    reset_composition();
    expect_menu("jumk", "橘", 1); /* 4 码规范全码,多候选组名次 */
    reset_composition();
    expect_menu("womf", "我们", 1); /* 固定词完整码 */
    reset_composition();
    expect_menu("uurufa", "输入法", 1);
    reset_composition();

    /* ---- 新简码 exact 输入(每层:高频代表 + 字典序首条) ---- */
    expect_menu("jqu", "就是", 0); /* 3 键高频 */
    reset_composition();
    expect_menu("aaa", "啊啊啊", 0); /* 3 键字典序首条 */
    reset_composition();
    expect_menu("veyd", "这样的", 0); /* 4 键高频 */
    reset_composition();
    expect_menu("aajj", "安安静静", 0); /* 4 键字典序首条 */
    reset_composition();
    expect_menu("vejqu", "这就是", 0); /* 5 键高频 */
    reset_composition();
    expect_menu("aabdl", "阿卜杜拉", 0); /* 5 键字典序首条 */
    reset_composition();
    /* 6/7 键层当前为空,无哨兵(N/A)。 */

    /* 简码可选择并上屏(alias 是真实候选)。 */
    type_keys("jqu");
    expect_commit_first("jqu", "就是");
    reset_composition();

    /* ---- prefix continuation:每层一个「shortcut 是自身完整码 prefix」哨兵 ---- */
    type_keys("jqu"); /* 3 键:就是 jqui 的 prefix */
    check_only("jqu → 菜单含 就是(继续前)", "就是", 0);
    report(!has_commit() && has_active_composition(), "jqu → 继续前无 auto commit", NULL);
    type_keys("i");
    check_only("jqui → 菜单含 就是(继续后)", "就是", 0);
    report(has_active_composition(), "jqui → 组合仍活动(未截断)", NULL);
    reset_composition();

    type_keys("bkti"); /* 4 键:并提出 bktiiu 的 prefix */
    check_only("bkti → 菜单含 并提出(继续前)", "并提出", 0);
    report(!has_commit() && has_active_composition(), "bkti → 继续前无 auto commit", NULL);
    type_keys("iu");
    check_only("bktiiu → 菜单含 并提出(继续后)", "并提出", 0);
    report(has_active_composition(), "bktiiu → 组合仍活动(未截断)", NULL);
    reset_composition();

    type_keys("vejqu"); /* 5 键:这就是 vejqui 的 prefix */
    check_only("vejqu → 菜单含 这就是(继续前)", "这就是", 0);
    report(!has_commit() && has_active_composition(), "vejqu → 继续前无 auto commit", NULL);
    type_keys("i");
    check_only("vejqui → 菜单含 这就是(继续后)", "这就是", 0);
    report(has_active_composition(), "vejqui → 组合仍活动(未截断)", NULL);
    reset_composition();

    /* ---- 非 prefix 模式:alias 与完整码共存 ---- */
    expect_menu("veyd", "这样的", 0); /* FII,非完整码 prefix */
    reset_composition();
    expect_menu("veyhde", "这样的", 0); /* 完整码仍独立可用 */
    reset_composition();
    expect_menu("abc", "安保", 0); /* 3 键非 prefix(IF) */
    reset_composition();
    expect_menu("anbc", "安保", 0);
    reset_composition();

    /* ---- 「时间」回归与 FIXED_FIRST 哨兵 ---- */
    /* 完整码不变:uijm 仍以时间为首。 */
    expect_menu("uijm", "时间", 1);
    reset_composition();
    /* FIXED_FIRST 核心哨兵:uij 精确序为 铈 → 鼫 → 时间
     * (全部既有固定候选在前,时间严格追加为 rank 3)。 */
    {
        const char *const expected[] = {"铈", "鼫", "时间"};
        expect_exact_order("uij", expected, 3);
    }
    reset_composition();
    /* ujm(闪 杉 栅 骟)不加入时间(一词最多一条简码:uij)。 */
    expect_absent("ujm", "时间");
    reset_composition();
    /* FIXED_FIRST exact 输入:高频代表(与 3 码单字重码,追加在组尾)。 */
    expect_menu("yeu", "也是", 0); /* yeu:baseline 1 候选,也是 rank 2 */
    reset_composition();

    /* ---- FIXED_FIRST prefix continuation 哨兵(静态审计 deterministic 选择) ---- */
    /* 3 键:aib(爱比)是 4 码规范全码 aibe(癌)的 strict prefix。 */
    type_keys("aib");
    check_only("aib → 菜单含 爱比(继续前)", "爱比", 0);
    report(!has_commit() && has_active_composition(), "aib → 继续前无 auto commit", NULL);
    type_keys("e");
    check_only("aibe → 菜单含 癌(继续后)", "癌", 1);
    report(has_active_composition(), "aibe → 组合仍活动(未截断)", NULL);
    reset_composition();

    /* 4 键:bcjy(本次交易)是 5 键 ZR 简码 bcjyu(不参与)的 strict prefix。 */
    type_keys("bcjy");
    check_only("bcjy → 菜单含 本次交易(继续前)", "本次交易", 0);
    report(!has_commit() && has_active_composition(), "bcjy → 继续前无 auto commit", NULL);
    type_keys("u");
    check_only("bcjyu → 菜单含 不参与(继续后)", "不参与", 1);
    report(has_active_composition(), "bcjyu → 组合仍活动(未截断)", NULL);
    reset_composition();

    /* 6 键:mwyzng(没有哪个)是 8 键固定词码 mwyznggz(没有能够)的 strict prefix。 */
    type_keys("mwyzng");
    check_only("mwyzng → 菜单含 没有哪个(继续前)", "没有哪个", 0);
    report(!has_commit() && has_active_composition(), "mwyzng → 继续前无 auto commit", NULL);
    type_keys("gz");
    check_only("mwyznggz → 菜单含 没有能够(继续后)", "没有能够", 1);
    report(has_active_composition(), "mwyznggz → 组合仍活动(未截断)", NULL);
    reset_composition();

    /* reverse prefix:一级简码 a 是 FF 简码 aib(爱比)的 strict prefix,
     * 继续输入不被更短既有候选截断。 */
    type_keys("a");
    report(!has_commit() && has_active_composition(), "a → 无 auto commit", NULL);
    type_keys("ib");
    check_only("aib → 菜单含 爱比(reverse 继续后)", "爱比", 0);
    report(has_active_composition(), "aib → 组合仍活动(reverse 未截断)", NULL);
    reset_composition();

    rime->destroy_session(session);
    rime->finalize();

    printf("----\n%d 项检查,%d 项失败\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
