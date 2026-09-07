/* XHUP Flow librime 日常输入控制测试。
 *
 * 真实 session 级验收:普通输入法日常操作在 xhup_flow 方案下的行为,
 * 全部只使用稳定 C API 与标准 librime 组件(ascii_composer / punctuator /
 * key_binder / selector / navigator / express_editor),不引入任何自定义
 * 处理器:
 *
 * - ASCII 临时切换:默认 switch_key(Shift_L: inline_ascii)点按切换
 *   ascii_mode;ASCII 模式下字母键原样穿透(未被处理);
 * - 中文标点:punctuator 预设(逗号/句号直接提交全角);
 * - 数字选择:候选菜单中数字键直接选中对应候选并上屏(uij 哨兵);
 * - 翻页:key_binder 预设 paging_with_minus_equal(=/- 翻页);
 * - Escape 取消组合;Enter 提交原始输入;空格上屏首选候选;
 * - 空组合下数字键原样穿透(不属于 alphabet)。
 *
 * 用法: runtime_ime_controls <shared_data_dir> <user_data_dir>
 * user_data_dir 必须已含生成包并完成 rime_deployer --compile。
 *
 * 只使用稳定 C API;无第三方测试框架;不访问用户真实 Rime 目录。
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rime_api.h>

/* XKB 键值与 librime 按键事件修饰位(key_event.h)。 */
#define XKB_Shift_L 0xffe1
#define XKB_Return 0xff0d
#define XKB_Escape 0xff1b
#define XKB_Equal 0x003d
#define XKB_Minus 0x002d
#define KEY_release (1 << 30)

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
        (void)rime->process_key(session, *p, 0);
    }
}

/* 取走并返回最近一次提交文本(无提交返回空串)。 */
static void take_commit(char *buf, size_t size) {
    buf[0] = '\0';
    RIME_STRUCT(RimeCommit, commit);
    if (rime->get_commit(session, &commit)) {
        if (commit.text) {
            snprintf(buf, size, "%s", commit.text);
        }
        rime->free_commit(&commit);
    }
}

/* 当前组合是否活动。 */
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
    {
        char sink[256];
        take_commit(sink, sizeof(sink));
    }
}

/* ascii_mode 选项当前值。 */
static int ascii_mode_on(void) {
    return rime->get_option(session, "ascii_mode") ? 1 : 0;
}

/* 按键是否被方案处理(process_key 返回值)。 */
static int key_handled(int keycode, int modifier) {
    return rime->process_key(session, keycode, modifier) ? 1 : 0;
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
    traits.distribution_name = "XHUP Flow IME Controls";
    traits.distribution_code_name = "xhup-ime-controls";
    traits.distribution_version = "0";
    traits.app_name = "xhup.runtime_ime_controls";
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

    char commit[256];

    /* ---- 1. ASCII 切换(ascii_composer 默认 switch_key) ---- */
    report(!ascii_mode_on(), "初始 ascii_mode = 中文", NULL);

    /* Shift_L 点按(按下 + 500ms 内释放)→ 临时英文。 */
    (void)key_handled(XKB_Shift_L, 0);
    (void)key_handled(XKB_Shift_L, KEY_release);
    report(ascii_mode_on(), "Shift_L 点按 → ascii_mode = 英文", NULL);

    /* ASCII 模式下字母键原样穿透:未被处理、无组合、无提交。 */
    (void)key_handled('a', 0);
    report(!has_active_composition(), "英文模式下字母键穿透(无组合)", NULL);

    /* 再次点按切回中文。 */
    (void)key_handled(XKB_Shift_L, 0);
    (void)key_handled(XKB_Shift_L, KEY_release);
    report(!ascii_mode_on(), "Shift_L 再次点按 → ascii_mode = 中文", NULL);

    /* 切回后字母键恢复组词。 */
    type_keys("wo");
    report(has_active_composition(), "切回中文后字母键恢复组合", NULL);
    reset_composition();

    /* ---- 2. 中文标点(punctuator 预设:直接提交全角) ---- */
    (void)key_handled(',', 0);
    take_commit(commit, sizeof(commit));
    report(strcmp(commit, "，") == 0, "空组合下 ',' 直接提交全角逗号", commit);

    (void)key_handled('.', 0);
    take_commit(commit, sizeof(commit));
    report(strcmp(commit, "。") == 0, "空组合下 '.' 直接提交全角句号", commit);

    /* ---- 3. 空组合下数字键穿透 ---- */
    report(!key_handled('1', 0) && !has_active_composition(),
           "空组合下数字键 '1' 穿透", NULL);

    /* ---- 4. 数字选择候选(uij 哨兵:铈 / 鼫 / 时间) ---- */
    type_keys("uij");
    report(has_active_composition(), "uij 组合活动", NULL);
    (void)key_handled('2', 0);
    take_commit(commit, sizeof(commit));
    report(strcmp(commit, "鼫") == 0, "uij + 数字 2 → 第 2 候选「鼫」上屏", commit);

    /* ---- 5. 翻页(key_binder 预设 paging_with_minus_equal) ---- */
    type_keys("uj");
    {
        RIME_STRUCT(RimeContext, context);
        int page0 = -1, per_page = 0;
        if (rime->get_context(session, &context)) {
            page0 = context.menu.page_no;
            per_page = context.menu.num_candidates;
            rime->free_context(&context);
        }
        report(page0 == 0 && per_page == 5, "uj 首页(每页 5 候选)", NULL);

        (void)key_handled(XKB_Equal, 0); /* '=' → Page_Down */
        int page1 = -1;
        if (rime->get_context(session, &context)) {
            page1 = context.menu.page_no;
            rime->free_context(&context);
        }
        report(page1 == 1, "'=' 翻到下一页", NULL);

        (void)key_handled(XKB_Minus, 0); /* '-' → Page_Up */
        int page_back = -1;
        if (rime->get_context(session, &context)) {
            page_back = context.menu.page_no;
            rime->free_context(&context);
        }
        report(page_back == 0, "'-' 翻回上一页", NULL);
    }
    reset_composition();

    /* ---- 6. Escape 取消组合 ---- */
    type_keys("wo");
    report(has_active_composition(), "wo 组合活动(Escape 前置)", NULL);
    (void)key_handled(XKB_Escape, 0);
    report(!has_active_composition(), "Escape 取消组合", NULL);
    take_commit(commit, sizeof(commit));
    report(commit[0] == '\0', "Escape 未产生提交", commit);

    /* ---- 7. Enter 提交原始输入 ---- */
    type_keys("wo");
    (void)key_handled(XKB_Return, 0);
    take_commit(commit, sizeof(commit));
    report(strcmp(commit, "wo") == 0, "Enter 原样上屏原始输入", commit);

    /* ---- 8. 空格上屏首选候选(womf 固定词哨兵) ---- */
    type_keys("womf");
    (void)key_handled(' ', 0);
    take_commit(commit, sizeof(commit));
    report(strcmp(commit, "我们") == 0, "womf + 空格 → 「我们」上屏", commit);

    rime->destroy_session(session);
    rime->finalize();

    printf("\n%d 项检查,%d 项失败\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
