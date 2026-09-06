/* XHUP Flow Windows 真机部署运行时探针。
 *
 * 用 Weasel 自带的 rime.dll(官方安装目录)对真实 Rime 用户数据目录做
 * 只读运行时验收:创建会话、逐键输入、读取候选菜单 —— **绝不提交、
 * 绝不写用户学习数据**(组合一律以 clear_composition 结束)。
 *
 * 覆盖仓库既有哨兵(与 tests/librime/runtime_smoke.c 同源语义):
 * 一级简码、固定词、ZR/FIXED_FIRST 简码序(时间系)、二码层,以及
 * 部署健康检查(build 产物与三方案可选)。
 *
 * 用法: rime_probe.exe <user_data_dir> [rime_dll]
 *   user_data_dir  真实 Rime 用户数据目录(只读访问)
 *   rime_dll       缺省自动探测 %ProgramFiles%\Rime\weasel-*\rime.dll
 *
 * 只使用稳定 C API;不访问注册表;不修改任何文件。
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

#include "rime_api.h"

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
        (void)rime->process_key(session, *p, 0);
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

/* 组合是否活动。 */
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
}

/* 只读检查:输入 keys 后 target 的期望名次(1 = 首个)。 */
static void expect_menu(const char *keys, const char *target, int expected_rank) {
    char name[128];
    char detail[64];
    snprintf(name, sizeof(name), "%s → %s 期望名次 %d", keys, target,
             expected_rank);
    type_keys(keys);
    int n = 0;
    int rank = candidate_rank(target, &n);
    snprintf(detail, sizeof(detail), "rank=%d, candidates=%d", rank, n);
    report(rank == expected_rank, name, detail);
    snprintf(name, sizeof(name), "%s → 组合活动(只读,不提交)", keys);
    report(has_active_composition(), name, NULL);
    reset_composition();
}

/* 断言:输入 keys 后菜单不包含 target。 */
static void expect_absent(const char *keys, const char *target) {
    char name[128];
    char detail[64];
    snprintf(name, sizeof(name), "%s → 菜单不含 %s", keys, target);
    type_keys(keys);
    int n = 0;
    int rank = candidate_rank(target, &n);
    snprintf(detail, sizeof(detail), "rank=%d, candidates=%d", rank, n);
    report(rank == 0, name, detail);
    reset_composition();
}

static char *find_rime_dll(void) {
    static char path[MAX_PATH];
    const char *bases[] = {"ProgramFiles", "ProgramFiles(x86)"};
    for (size_t i = 0; i < sizeof(bases) / sizeof(bases[0]); ++i) {
        const char *base = getenv(bases[i]);
        if (!base) continue;
        char rime_dir[MAX_PATH];
        snprintf(rime_dir, sizeof(rime_dir), "%s\\Rime", base);
        WIN32_FIND_DATAA data;
        char pattern[MAX_PATH];
        snprintf(pattern, sizeof(pattern), "%s\\weasel-*", rime_dir);
        HANDLE handle = FindFirstFileA(pattern, &data);
        if (handle == INVALID_HANDLE_VALUE) continue;
        /* 取最后一个匹配(字典序最大的版本目录;FindFirstFile 按名字序)。 */
        char best[MAX_PATH] = "";
        do {
            if (!(data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)) continue;
            snprintf(best, sizeof(best), "%s\\%s", rime_dir, data.cFileName);
        } while (FindNextFileA(handle, &data));
        FindClose(handle);
        if (best[0]) {
            snprintf(path, sizeof(path), "%s\\rime.dll", best);
            return path;
        }
    }
    return NULL;
}

int main(int argc, char **argv) {
    if (argc < 2 || argc > 3) {
        fprintf(stderr, "用法: %s <user_data_dir> [rime_dll]\n", argv[0]);
        return 2;
    }
    const char *dll = argc == 3 ? argv[2] : find_rime_dll();
    if (!dll) {
        fprintf(stderr, "未找到 Weasel 的 rime.dll,请显式传入路径\n");
        return 2;
    }

    /* 从 Weasel 安装目录加载 rime.dll(依赖 DLL 同目录解析)。 */
    char dll_dir[MAX_PATH];
    snprintf(dll_dir, sizeof(dll_dir), "%s", dll);
    char *slash = strrchr(dll_dir, '\\');
    if (slash) *slash = '\0';
    SetDllDirectoryA(dll_dir);
    HMODULE module = LoadLibraryExA(dll, NULL, LOAD_WITH_ALTERED_SEARCH_PATH);
    if (!module) {
        fprintf(stderr, "LoadLibrary(%s) 失败: %lu\n", dll, GetLastError());
        return 2;
    }
    typedef RimeApi *(*rime_get_api_t)(void);
    rime_get_api_t get_api =
        (rime_get_api_t)(void *)GetProcAddress(module, "rime_get_api");
    if (!get_api) {
        fprintf(stderr, "GetProcAddress(rime_get_api) 失败\n");
        return 2;
    }
    rime = get_api();
    if (!rime) {
        fprintf(stderr, "rime_get_api 返回空\n");
        return 2;
    }
    printf("rime.dll: %s\n", dll);

    RIME_STRUCT(RimeTraits, traits);
    /* Weasel 共享数据目录 = rime.dll 同级的 data\ 子目录(default.yaml)。 */
    static char shared_dir[MAX_PATH];
    snprintf(shared_dir, sizeof(shared_dir), "%s\\data", dll_dir);
    traits.shared_data_dir = shared_dir;
    traits.user_data_dir = argv[1];
    traits.distribution_name = "XHUP Flow Deploy Probe";
    traits.distribution_code_name = "xhup-deploy-probe";
    traits.distribution_version = "0";
    traits.app_name = "xhup.deploy_probe";
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

    /* ---- 部署健康:三方案可选 ---- */
    report(rime->select_schema(session, "xhup_flow"), "方案可选: xhup_flow", NULL);
    report(rime->select_schema(session, "xhup_flow_static"), "方案可选: xhup_flow_static", NULL);
    report(rime->select_schema(session, "xhup_fullcode"), "方案可选: xhup_fullcode", NULL);
    report(rime->select_schema(session, "xhup_flow"), "回到 xhup_flow", NULL);

    /* ---- 一级简码 ---- */
    expect_menu("q", "去", 1);
    expect_menu("wo", "我", 1);

    /* ---- 固定词 ---- */
    expect_menu("womf", "我们", 1);
    expect_menu("uurufa", "输入法", 1);

    /* ---- 词语简码哨兵(与 runtime_smoke.c 同源语义) ---- */
    expect_menu("jd", "记得", 1);      /* 二码层 */
    expect_menu("jqu", "就是", 1);     /* ZR 简码 */
    expect_menu("uijm", "时间", 1);    /* 完整码 */
    expect_menu("uij", "时间", 3);     /* FIXED_FIRST:铈 → 鼫 → 时间 */
    expect_absent("ujm", "时间");      /* ujm 不加入时间 */
    expect_menu("uj", "山", 1);        /* uj 首位山,且不含时间 */
    {
        type_keys("uj");
        int n = 0;
        int rank = candidate_rank("时间", &n);
        char detail[64];
        snprintf(detail, sizeof(detail), "rank=%d, candidates=%d", rank, n);
        report(rank == 0, "uj → 菜单不含 时间", detail);
        reset_composition();
    }

    /* ---- 组句能力(Flow;只读组合,不提交) ---- */
    {
        type_keys("womfuijm");
        int n = 0;
        int rank = candidate_rank("我们时间", &n);
        char detail[64];
        snprintf(detail, sizeof(detail), "rank=%d, candidates=%d", rank, n);
        report(rank > 0, "womfuijm → 组句「我们时间」可见", detail);
        reset_composition();
    }

    /* ---- ASCII 穿透(中文模式下数字键不属于 alphabet) ---- */
    {
        int handled = rime->process_key(session, '1', 0) ? 1 : 0;
        report(!handled && !has_active_composition(),
               "空组合下数字键 '1' 穿透", NULL);
        reset_composition();
    }

    rime->destroy_session(session);
    rime->finalize();

    printf("\n%d 项检查,%d 项失败\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
