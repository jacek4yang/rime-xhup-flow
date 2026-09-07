/* XHUP Flow Lua quick_hint runtime 审计。
 *
 * 验证 docs/lua-runtime.md §4.1 的语义与不变量:
 *
 * - 全码输入时,有可用简码的候选注释含 `⚡<简码>`(如 uijm → 时间 ⚡uij);
 * - 提示只是注释追加:开关 quick_hint 开/关两趟的候选菜单**逐项相同**
 *   (候选次序与集合零变化,FROZEN STATIC 契约);
 * - 输入已是简码本身时不提示;
 * - 简码不短于输入时不提示(2 键输入对 3 键简码无提示)。
 *
 * 用法: runtime_lua_audit <shared_data_dir> <user_data_dir>
 * user_data_dir 必须已含生成包(含 lua/ 子目录)并完成部署编译;
 * 运行环境必须已加载 librime-lua(本审计同时反证插件在场)。
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

static void type_keys(const char *keys) {
    for (const char *p = keys; *p; ++p) {
        if (!rime->process_key(session, *p, 0)) {
            printf("WARN  按键 %c 未被处理\n", *p);
        }
    }
}

static void clear_input(void) {
    rime->clear_composition(session);
}

/* 抓取当前菜单:文本与注释,逐项拼入 buffer(次序保持)。 */
static size_t capture_menu(char *out, size_t cap) {
    RIME_STRUCT(RimeContext, context);
    size_t used = 0;
    out[0] = '\0';
    if (rime->get_context(session, &context)) {
        for (int i = 0; i < context.menu.num_candidates; ++i) {
            const char *text = context.menu.candidates[i].text;
            const char *comment = context.menu.candidates[i].comment;
            int n = snprintf(out + used, cap - used, "%s%s%s^_",
                             text ? text : "", comment ? "(" : "",
                             comment ? comment : "");
            if (n < 0 || (size_t)n >= cap - used) break;
            if (comment && comment[0]) {
                /* 注释已写入;补上右括号需要额外空间,改为简单分隔 */
            }
            used += (size_t)n;
        }
        rime->free_context(&context);
    }
    return used;
}

/* 菜单中目标文本的注释(找到返回其 comment 指针副本到 buf)。 */
static int candidate_comment(const char *target, char *buf, size_t cap) {
    RIME_STRUCT(RimeContext, context);
    int found = 0;
    if (rime->get_context(session, &context)) {
        for (int i = 0; i < context.menu.num_candidates; ++i) {
            const char *text = context.menu.candidates[i].text;
            if (text && strcmp(text, target) == 0) {
                const char *comment = context.menu.candidates[i].comment;
                snprintf(buf, cap, "%s", comment ? comment : "");
                found = 1;
                break;
            }
        }
        rime->free_context(&context);
    }
    return found;
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "用法: runtime_lua_audit <shared> <user>\n");
        return 2;
    }
    rime = rime_get_api();
    if (!rime) {
        fprintf(stderr, "Rime API 不可用\n");
        return 2;
    }
    RIME_STRUCT(RimeTraits, traits);
    traits.shared_data_dir = argv[1];
    traits.user_data_dir = argv[2];
    traits.distribution_name = "xhup-flow-lua-audit";
    traits.distribution_code_name = "xhup-flow-lua-audit";
    traits.distribution_version = "0";
    rime->setup(&traits);
    rime->initialize(&traits);
    session = rime->create_session();
    if (!session) {
        fprintf(stderr, "会话创建失败\n");
        return 2;
    }

    static char menu_on[65536];
    static char menu_off[65536];
    char comment[256];

    /* 1. 全码输入:提示存在。 */
    type_keys("uijm");
    int found = candidate_comment("时间", comment, sizeof(comment));
    report(found && strstr(comment, "⚡uij") != NULL,
           "uijm → 时间 注释含 ⚡uij", comment);
    capture_menu(menu_on, sizeof(menu_on));
    clear_input();

    /* 2. 关闭开关:提示消失,菜单逐项相同。 */
    if (!rime->set_option(session, "quick_hint", 0)) {
        report(0, "set_option quick_hint=0", NULL);
    }
    type_keys("uijm");
    found = candidate_comment("时间", comment, sizeof(comment));
    report(found && strstr(comment, "⚡uij") == NULL,
           "quick_hint=0 → 时间 无提示注释", comment);
    capture_menu(menu_off, sizeof(menu_off));
    clear_input();

    /* 菜单逐项相同性:开/关两趟仅注释不同 —— 比较文本序列。
       capture_menu 含注释,这里用「去掉注释段」的等值判断:
       逐条比对两趟菜单中每个候选的 text 部分。 */
    /* 简化:重新各抓一趟仅文本的拼接 */
    {
        /* 开关开 */
        rime->set_option(session, "quick_hint", 1);
        type_keys("uijm");
        RIME_STRUCT(RimeContext, ctx_on);
        static char texts_on[32768];
        texts_on[0] = '\0';
        if (rime->get_context(session, &ctx_on)) {
            for (int i = 0; i < ctx_on.menu.num_candidates; ++i) {
                const char *t = ctx_on.menu.candidates[i].text;
                if (t) strncat(texts_on, t, sizeof(texts_on) - strlen(texts_on) - 2);
                strncat(texts_on, "^_", sizeof(texts_on) - strlen(texts_on) - 1);
            }
            rime->free_context(&ctx_on);
        }
        clear_input();
        /* 开关关 */
        rime->set_option(session, "quick_hint", 0);
        type_keys("uijm");
        RIME_STRUCT(RimeContext, ctx_off);
        static char texts_off[32768];
        texts_off[0] = '\0';
        if (rime->get_context(session, &ctx_off)) {
            for (int i = 0; i < ctx_off.menu.num_candidates; ++i) {
                const char *t = ctx_off.menu.candidates[i].text;
                if (t) strncat(texts_off, t, sizeof(texts_off) - strlen(texts_off) - 2);
                strncat(texts_off, "^_", sizeof(texts_off) - strlen(texts_off) - 1);
            }
            rime->free_context(&ctx_off);
        }
        clear_input();
        report(strcmp(texts_on, texts_off) == 0,
               "quick_hint 开/关候选文本序列逐项相同", NULL);
        rime->set_option(session, "quick_hint", 1);
    }

    /* 3. 输入已是简码本身:不提示。 */
    type_keys("uij");
    found = candidate_comment("时间", comment, sizeof(comment));
    report(found && strstr(comment, "⚡") == NULL,
           "uij(简码本身)→ 时间 无提示", comment);
    clear_input();

    /* 4. 简码不短于输入:不提示(ui 2 键对 3 键简码)。 */
    type_keys("ui");
    found = candidate_comment("时间", comment, sizeof(comment));
    if (found) {
        report(strstr(comment, "⚡") == NULL, "ui(2 键)→ 时间 无提示", comment);
    } else {
        report(1, "ui(2 键)→ 时间 不在菜单(前缀不可达,符合预期)", NULL);
    }
    clear_input();

    rime->destroy_session(session);
    rime->finalize();
    printf("== Lua quick_hint 审计:%d 检查,%d 失败 ==\n", checks, failures);
    return failures ? 1 : 0;
}
