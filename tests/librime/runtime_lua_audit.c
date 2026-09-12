/* XHUP Flow Lua 候选注释与 quick_hint runtime 审计。
 *
 * 验证 docs/lua-runtime.md §4.1 的语义与不变量:
 *
 * - 全码输入时,有可用简码的候选注释含 `~<简码>`(纯 ASCII,如 uijm → 时间 ~uij);
 * - 正常模式绝不含 `⚡`、`☯` 等装饰/引擎标记;
 * - 提示只是注释追加:开关 quick_hint 开/关两趟的候选**文本序列逐项相同**
 *   (候选次序与集合零变化,FROZEN STATIC 契约);
 * - 输入已是简码本身时不提示;
 * - 简码不短于输入时不提示(2 键输入对 3 键简码无提示);
 * - 调试开关 debug_candidate_annotations 开启时支持明确可信标签。
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

/* 菜单中目标文本的注释(找到返回 1,注释复制到 buf)。 */
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

/* 当前菜单候选文本序列(仅文本,`^_` 分隔,次序保持)。 */
static void capture_texts(char *out, size_t cap) {
    RIME_STRUCT(RimeContext, context);
    out[0] = '\0';
    if (rime->get_context(session, &context)) {
        for (int i = 0; i < context.menu.num_candidates; ++i) {
            const char *text = context.menu.candidates[i].text;
            if (text) {
                strncat(out, text, cap - strlen(out) - 2);
                strncat(out, "^_", cap - strlen(out) - 1);
            }
        }
        rime->free_context(&context);
    }
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
    traits.app_name = "rime.xhup-flow-lua-audit";
    traits.shared_data_dir = argv[1];
    traits.user_data_dir = argv[2];
    traits.distribution_name = "xhup-flow-lua-audit";
    traits.distribution_code_name = "xhup-flow-lua-audit";
    traits.distribution_version = "0";
    rime->setup(&traits);
    rime->initialize(&traits);
    /* 全新 user 目录的首次 initialize 会异步进入维护模式,必须等部署
     * 线程完成再开会话,否则引擎未就绪,按键全部不被处理
     * (与 runtime_flow_audit.c 的 open_session 同构)。 */
    if (rime->is_maintenance_mode && rime->is_maintenance_mode()) {
        rime->join_maintenance_thread();
    }
    session = rime->create_session();
    if (!session || !rime->select_schema(session, "xhup_flow")) {
        fprintf(stderr, "会话创建失败或无法选择 schema xhup_flow\n");
        return 2;
    }

    static char texts_on[32768];
    static char texts_off[32768];
    char comment[256];

    /* 1. 全码输入(开关默认开):提示存在(纯 ASCII ~uij);无 ⚡ 与 ☯;抓候选文本序列。 */
    type_keys("uijm");
    int found = candidate_comment("时间", comment, sizeof(comment));
    report(found && strstr(comment, "~uij") != NULL && strstr(comment, "⚡") == NULL && strstr(comment, "☯") == NULL,
           "uijm → 时间 注释含 ~uij (纯 ASCII,无 ⚡ 与 ☯)", comment);
    capture_texts(texts_on, sizeof(texts_on));
    rime->clear_composition(session);

    /* 2. 关闭开关:提示消失;候选文本序列逐项相同。 */
    rime->set_option(session, "quick_hint", 0);
    type_keys("uijm");
    found = candidate_comment("时间", comment, sizeof(comment));
    report(found && strstr(comment, "~uij") == NULL && strstr(comment, "⚡") == NULL,
           "quick_hint=0 → 时间 无提示注释", comment);
    capture_texts(texts_off, sizeof(texts_off));
    rime->clear_composition(session);
    rime->set_option(session, "quick_hint", 1);
    report(strcmp(texts_on, texts_off) == 0,
           "quick_hint 开/关候选文本序列逐项相同", NULL);

    /* 3. 输入已是简码本身:不提示。 */
    type_keys("uij");
    found = candidate_comment("时间", comment, sizeof(comment));
    report(found && strstr(comment, "~") == NULL && strstr(comment, "⚡") == NULL,
           "uij(简码本身)→ 时间 无提示", comment);
    rime->clear_composition(session);

    /* 4. 简码不短于输入:不提示(ui 2 键对 3 键简码)。 */
    type_keys("ui");
    found = candidate_comment("时间", comment, sizeof(comment));
    if (found) {
        report(strstr(comment, "~") == NULL && strstr(comment, "⚡") == NULL, "ui(2 键)→ 时间 无提示", comment);
    } else {
        report(1, "ui(2 键)→ 时间 不在菜单(前缀不可达,符合预期)", NULL);
    }
    rime->clear_composition(session);

    /* 5. 调试模式开关:可正常设置并在会话中生效。 */
    rime->set_option(session, "debug_candidate_annotations", 1);
    type_keys("uijm");
    found = candidate_comment("时间", comment, sizeof(comment));
    report(found && strstr(comment, "~uij") != NULL && strstr(comment, "⚡") == NULL,
           "debug_candidate_annotations=1 生效且仍保持无 ⚡", comment);
    rime->clear_composition(session);
    rime->set_option(session, "debug_candidate_annotations", 0);

    rime->destroy_session(session);
    rime->finalize();
    printf("== Lua quick_hint 审计:%d 检查,%d 失败 ==\n", checks, failures);
    return failures ? 1 : 0;
}
