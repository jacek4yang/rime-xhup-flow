/* librime inter-translator 优先级栅栏(initial_quality)预检。
 *
 * 在真实 session 中输入固定码 abc,断言候选菜单与期望序列逐项、逐顺序
 * 完全一致,且无 auto commit、组合保持活动。CONTROL(preflight_control,
 * 仅 primary translator)期望 甲,乙,丙;PRODUCTION(preflight,双
 * translator)期望 甲,乙,丙,目标 —— 共同证明 initial_quality fence 让
 * secondary 候选严格追加到全部 primary 候选之后,且不改变 primary 内部
 * 相对次序。
 *
 * 用法:
 *   runtime_priority_preflight <shared_data_dir> <user_data_dir>
 *                              <schema_id> <期望菜单,逗号分隔>
 *
 * user_data_dir 必须已含 preflight fixture 并完成 rime_deployer --compile。
 * 只使用稳定 C API;无第三方测试框架;不访问用户真实 Rime 目录。
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rime_api.h>

int main(int argc, char **argv) {
    if (argc != 5) {
        fprintf(stderr,
                "用法: %s <shared_data_dir> <user_data_dir> <schema_id> "
                "<期望菜单,逗号分隔>\n",
                argv[0]);
        return 2;
    }
    const char *schema_id = argv[3];
    const char *expected = argv[4];

    RimeApi *rime = rime_get_api();
    if (!rime) {
        fprintf(stderr, "rime_get_api 失败\n");
        return 2;
    }

    RIME_STRUCT(RimeTraits, traits);
    traits.shared_data_dir = argv[1];
    traits.user_data_dir = argv[2];
    traits.distribution_name = "XHUP Flow Priority Preflight";
    traits.distribution_code_name = "xhup-priority-preflight";
    traits.distribution_version = "0";
    traits.app_name = "xhup.priority_preflight";
    rime->setup(&traits);
    rime->initialize(&traits);
    if (rime->is_maintenance_mode && rime->is_maintenance_mode()) {
        rime->join_maintenance_thread();
    }

    int ok = 1;
    RimeSessionId session = rime->create_session();
    if (!session || !rime->select_schema(session, schema_id)) {
        fprintf(stderr, "无法创建会话或选择 schema %s\n", schema_id);
        rime->finalize();
        return 2;
    }

    for (const char *p = "abc"; *p; ++p) {
        rime->process_key(session, *p, 0);
    }

    /* 把实际菜单拼成逗号分隔字符串,与期望整体比较。 */
    char actual[1024];
    actual[0] = '\0';
    int composition_active = 0;
    RIME_STRUCT(RimeContext, context);
    if (rime->get_context(session, &context)) {
        composition_active = context.composition.length > 0;
        for (int i = 0; i < context.menu.num_candidates; ++i) {
            if (i > 0) strncat(actual, ",", sizeof(actual) - strlen(actual) - 1);
            strncat(actual, context.menu.candidates[i].text,
                    sizeof(actual) - strlen(actual) - 1);
        }
        rime->free_context(&context);
    }

    int committed = 0;
    RIME_STRUCT(RimeCommit, commit);
    if (rime->get_commit(session, &commit)) {
        committed = commit.text != NULL && commit.text[0] != '\0';
        rime->free_commit(&commit);
    }

    if (strcmp(actual, expected) != 0) {
        printf("FAIL  %s → abc 菜单应为 [%s],实际 [%s]\n", schema_id, expected,
               actual);
        ok = 0;
    } else {
        printf("PASS  %s → abc 菜单 [%s]\n", schema_id, actual);
    }
    if (committed || !composition_active) {
        printf("FAIL  %s → abc auto commit=%d composition=%d(期望无提交且活动)\n",
               schema_id, committed, composition_active);
        ok = 0;
    } else {
        printf("PASS  %s → abc 无 auto commit,组合活动\n", schema_id);
    }

    rime->destroy_session(session);
    rime->finalize();
    return ok ? 0 : 1;
}
