/* XHUP Flow context_ranker 有界上下文调序 runtime 审计(真实 librime-lua)。
 *
 * 用法: runtime_context_ranker_audit <shared_data_dir> <user_data_dir>
 *
 * 断言目标(#128 转正条件):
 * 1. 默认关闭(context_ranker=0)时行为与无此 filter 严格一致 ——
 *    同输入候选序列逐项相同(透传恒等);
 * 2. 开启后:无提交历史 → 恒等(无证据不重排);
 * 3. 开启后:刚提交词形的重复场景 → 证据候选窗口内提前,且候选集合
 *    不变(不删不减不改文本);
 * 4. 静态强固定映射 rank-1 永不降位:输入固定码时第一位不变;
 * 5. OOV 可达性:开启前后,词表外组合路径候选仍可达(不被调序吞掉)。
 *
 * 与 runtime_lua_audit.c 同构:Rime API 直连,部署等待,会话级断言。
 */

#include <rime_api.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static RimeApi *rime;
static int failures = 0;
static int checks = 0;

static void report(int ok, const char *name, const char *detail) {
  ++checks;
  if (ok) {
    printf("PASS  %s\n", name);
  } else {
    ++failures;
    printf("FAIL  %s", name);
    if (detail && detail[0]) {
      printf("  [%s]", detail);
    }
    printf("\n");
  }
  /* 段落式断点输出:即使后续场景段错误,已完成的检查结果也可见于日志。 */
  fflush(stdout);
}

static RimeSessionId session;

/* 场景 5 的证据词(数据驱动);5A 写入 5B 读用(跨进程经 TSV 持久)。 */
static char evidence_word[256];

static void type_keys(const char *keys) {
  for (const char *p = keys; *p; ++p) {
    if (!rime->process_key(session, *p, 0)) {
      fprintf(stderr, "按键未被处理: %c\n", *p);
      exit(2);
    }
  }
}

/* 捕获当前候选文本序列,以 '\n' 分隔写入 out(截断安全)。 */
static void capture_texts(char *out, size_t cap) {
  size_t used = 0;
  out[0] = '\0';
  RIME_STRUCT(RimeContext, context);
  if (!rime->get_context(session, &context)) {
    return;
  }
  const RimeMenu *menu = &context.menu;
  for (int i = 0; i < menu->num_candidates && used + 4 < cap; ++i) {
    const char *text = menu->candidates[i].text;
    if (!text) {
      continue;
    }
    size_t len = strlen(text);
    if (used + len + 2 >= cap) {
      break;
    }
    if (used > 0) {
      out[used++] = '\n';
    }
    memcpy(out + used, text, len);
    used += len;
    out[used] = '\0';
  }
  rime->free_context(&context);
}

/* 第 rank(1-based)个候选是否为 target;命中时写回其文本。 */
static int candidate_at(int rank, char *buf, size_t cap) {
  RIME_STRUCT(RimeContext, context);
  if (!rime->get_context(session, &context)) {
    return 0;
  }
  int ok = 0;
  if (rank >= 1 && rank <= context.menu.num_candidates) {
    const char *text = context.menu.candidates[rank - 1].text;
    if (text) {
      snprintf(buf, cap, "%s", text);
      ok = 1;
    }
  }
  rime->free_context(&context);
  return ok;
}

/* 输入 code 并选第 rank 个候选上屏(取走 commit 事件)。 */
static void commit_rank(const char *code, int rank) {
  type_keys(code);
  if (!rime->select_candidate(session, rank - 1)) {
    fprintf(stderr, "select_candidate 失败: code=%s rank=%d\n", code, rank);
    exit(2);
  }
  RIME_STRUCT(RimeCommit, commit);
  if (rime->get_commit(session, &commit)) {
    rime->free_commit(&commit);
  }
}

static void clear_all(void) {
  rime->clear_composition(session);
}

int main(int argc, char **argv) {
  if (argc != 3) {
    fprintf(stderr, "用法: runtime_context_ranker_audit <shared> <user>\n");
    return 2;
  }
  rime = rime_get_api();
  if (!rime) {
    fprintf(stderr, "Rime API 不可用\n");
    return 2;
  }
  RIME_STRUCT(RimeTraits, traits);
  traits.app_name = "rime.xhup-flow-context-ranker-audit";
  traits.shared_data_dir = argv[1];
  traits.user_data_dir = argv[2];
  traits.distribution_name = "xhup-flow-context-ranker-audit";
  traits.distribution_code_name = "xhup-flow-context-ranker-audit";
  traits.distribution_version = "0";
  rime->setup(&traits);
  rime->initialize(&traits);
  if (rime->is_maintenance_mode && rime->is_maintenance_mode()) {
    rime->join_maintenance_thread();
  }
  session = rime->create_session();
  if (!session || !rime->select_schema(session, "xhup_flow")) {
    fprintf(stderr, "会话创建失败或无法选择 schema xhup_flow\n");
    return 2;
  }

  static char order_off[32768];
  static char order_on[32768];
  static char order_off_again[32768];
  static char first_text[256];

  /* ---- 场景 1:默认关闭 = 恒等透传(与开启但无证据逐项一致) ---- */
  printf("-- 场景 1:默认关闭恒等 --\n");
  fflush(stdout);
  rime->set_option(session, "context_ranker", 0);
  type_keys("uijm");
  capture_texts(order_off, sizeof(order_off));
  clear_all();

  rime->set_option(session, "context_ranker", 1);
  type_keys("uijm");
  capture_texts(order_on, sizeof(order_on));
  clear_all();
  report(strcmp(order_off, order_on) == 0,
         "无提交历史:context_ranker 开/关候选序列逐项相同(无证据恒等)", NULL);

  /* ---- 场景 2:重复词有界提升 ----
   * 证据词形必须**原本不在第一位**,提升才可观测:取第 2 候选为证据词
   * (数据驱动,不硬编码词形 —— 词形由当前词典与候选序决定),上屏后
   * 重输同码,断言它被提升进前 bound 窗口。第 1 候选(上例中为全码
   * 本字)作为非证据对照,允许在窗口内有界换位,但不得被删减。 */
  printf("-- 场景 2:重复词有界提升 --\n");
  fflush(stdout);
  char evidence[256];
  rime->set_option(session, "context_ranker", 0);
  type_keys("uijm");
  if (!candidate_at(2, evidence_word, sizeof(evidence_word)) || !evidence_word[0]) {
    fprintf(stderr, "uijm 第 2 候选不存在,无法构造重复词场景\n");
    return 2;
  }
  clear_all();
  /* 上屏证据词(第 2 候选)。 */
  type_keys("uijm");
  if (!rime->select_candidate(session, 1)) {
    fprintf(stderr, "select_candidate(2) 失败\n");
    return 2;
  }
  {
    RIME_STRUCT(RimeCommit, commit);
    if (rime->get_commit(session, &commit)) {
      rime->free_commit(&commit);
    }
  }
  /* 开启:证据词应被提升进前 3。 */
  rime->set_option(session, "context_ranker", 1);
  type_keys("uijm");
  capture_texts(order_on, sizeof(order_on));
  clear_all();
  /* 关闭:对照(证据词保持在原位附近)。 */
  rime->set_option(session, "context_ranker", 0);
  type_keys("uijm");
  capture_texts(order_off_again, sizeof(order_off_again));
  clear_all();
  rime->set_option(session, "context_ranker", 1);
  report(strcmp(order_on, order_off_again) != 0,
         "重复词场景:开启后候选次序与关闭不同(证据生效)", NULL);
  {
    int hit = 0;
    for (int rank = 1; rank <= 3 && !hit; ++rank) {
      if (candidate_at(rank, first_text, sizeof(first_text)) &&
          strcmp(first_text, evidence_word) == 0) {
        hit = 1;
      }
    }
    /* 注意:上面 clear 后菜单为空,重新输入读取。 */
    type_keys("uijm");
    hit = 0;
    for (int rank = 1; rank <= 3 && !hit; ++rank) {
      if (candidate_at(rank, first_text, sizeof(first_text)) &&
          strcmp(first_text, evidence_word) == 0) {
        hit = 1;
      }
    }
    clear_all();
    report(hit, "重复词场景:证据词形位于前 3 候选内(有界提升)", evidence);
    {
      int on_has = strstr(order_on, evidence) != NULL;
      int off_has = strstr(order_off_again, evidence) != NULL;
      char diag[600];
      snprintf(diag, sizeof(diag), "on_has=%d off_has=%d on_head=%.40s off_head=%.40s",
               on_has, off_has, order_on, order_off_again);
      report(on_has && off_has,
             "候选集合不删减:开/关均含证据词形", diag);
    }
  }
  /* ---- 场景 3:静态强固定映射 rank-1 永不降位 ---- */
  printf("-- 场景 3:固定 rank-1 不降位 --\n");
  fflush(stdout);
  /* 输入固定码 uij:静态层第一位必是「时间」,开启调序后仍第一位。 */
  type_keys("uij");
  int fixed_first = candidate_at(1, first_text, sizeof(first_text));
  clear_all();
  report(fixed_first && strcmp(first_text, "时间") == 0,
         "固定码 uij:开启后第一位仍是静态 rank-1「时间」",
         fixed_first ? first_text : "(无候选)");

  /* ---- 场景 4:OOV 可达性(词表外组合不被调序吞掉) ---- */
  printf("-- 场景 4:OOV 可达性 --\n");
  fflush(stdout);
  /* 逐字两键音码原语组合(与 flow 组句同路径):开启前后都应产出候选。 */
  commit_rank("uijm", 1);
  type_keys("uiui");
  int oov_on = 0;
  {
    RIME_STRUCT(RimeContext, context);
    if (rime->get_context(session, &context)) {
      oov_on = context.menu.num_candidates > 0;
      rime->free_context(&context);
    }
  }
  clear_all();
  rime->set_option(session, "context_ranker", 0);
  type_keys("uiui");
  int oov_off = 0;
  {
    RIME_STRUCT(RimeContext, context);
    if (rime->get_context(session, &context)) {
      oov_off = context.menu.num_candidates > 0;
      rime->free_context(&context);
    }
  }
  clear_all();
  rime->set_option(session, "context_ranker", 1);
  report(oov_on && oov_off,
         "OOV 组合路径:开/关均有候选可达(调序不吞路径)", NULL);

  /* ---- 场景 5:用户记忆闭环(观察 → 写盘 → 重启 → 排序变化) ----
   * a. 开启 user_memory + context_ranker;
   * b. 对同一码的第 2 候选(数据驱动)连续上屏 FLUSH_EVERY 次以上,
   *    触发周期写盘;断言 TSV 快照文件已产生;
   * c. destroy_session + finalize(模拟重启);
   * d. 重新 initialize + create_session;两开关开启;重输同码;
   *    断言证据词形位于前 bound 窗口内(持久化记忆跨重启生效);
   * e. 固定码静态 rank-1 仍最前(用户桶不越静态契约)。 */
  printf("-- 场景 5:用户记忆闭环(观察→写盘→重启→排序) --\n");
  fflush(stdout);
  {
    /* 证据词:同码第 2 候选(数据驱动,不硬编码词形)。 */
    rime->set_option(session, "context_ranker", 1);
    rime->set_option(session, "user_memory", 1);
    type_keys("uijm");
    if (!candidate_at(2, evidence_word, sizeof(evidence_word)) || !evidence_word[0]) {
      report(0, "场景 5 前置:uijm 第 2 候选存在", "(缺失)");
      printf("== context_ranker runtime 审计:%d 项检查,%d 项失败 ==\n",
             checks, failures);
      fflush(stdout);
      rime->destroy_session(session);
      rime->finalize();
      return failures == 0 ? 0 : 1;
    }
    clear_all();
    /* 连续上屏 FLUSH_EVERY 次,触发周期写盘。 */
    for (int i = 0; i < 25; ++i) {
      type_keys("uijm");
      if (!rime->select_candidate(session, 1)) {
        fprintf(stderr, "场景 5:select_candidate(2) 失败\n");
        return 2;
      }
      RIME_STRUCT(RimeCommit, commit);
      if (rime->get_commit(session, &commit)) {
        rime->free_commit(&commit);
      }
    }
    /* 断言 TSV 快照已在工作目录产生(user_memory 默认相对路径)。 */
    {
      FILE *f = fopen("xhup_flow_user_model.tsv", "rb");
      report(f != NULL, "用户记忆快照 TSV 已周期落盘", "xhup_flow_user_model.tsv");
      if (f) {
        fclose(f);
      }
    }
    /* 固定码静态 rank-1 仍最前(用户记忆桶不越静态契约)。 */
    type_keys("uij");
    int fixed_still_first = candidate_at(1, first_text, sizeof(first_text));
    clear_all();
    report(fixed_still_first && strcmp(first_text, "时间") == 0,
           "重启前:固定码 uij 第一位仍是静态 rank-1「时间」",
           fixed_still_first ? first_text : "(无候选)");
  }

  /* ---- 场景 5B(独立进程调用,经 XHUP_AUDIT_PHASE=2 门控):重启后验证 ----
   * librime 不能在进程内二次 initialize(glog InitGoogleLogging 重复检查
   * 会 abort),「重启」由驱动脚本第二次调用本二进制完成;TSV 快照在
   * 工作目录跨进程持久,等效真实重启。 */
  const char *phase = getenv("XHUP_AUDIT_PHASE");
  if (phase && strcmp(phase, "2") == 0) {
    rime->set_option(session, "context_ranker", 1);
    rime->set_option(session, "user_memory", 1);
    type_keys("uijm");
    int learned_hit = 0;
    for (int rank = 1; rank <= 3 && !learned_hit; ++rank) {
      if (candidate_at(rank, first_text, sizeof(first_text)) &&
          strcmp(first_text, evidence_word) == 0) {
        learned_hit = 1;
      }
    }
    clear_all();
    report(learned_hit,
           "重启后(独立进程):用户记忆词形位于前 3 候选内(持久化生效)",
           learned_hit ? NULL : evidence_word);

    type_keys("uij");
    int fixed_first2 = candidate_at(1, first_text, sizeof(first_text));
    clear_all();
    report(fixed_first2 && strcmp(first_text, "时间") == 0,
           "重启前:固定码 uij 第一位仍是静态 rank-1「时间」",
           fixed_first2 ? first_text : "(无候选)");
  }

  printf("== context_ranker runtime 审计:%d 项检查,%d 项失败 ==\n",
         checks, failures);
  fflush(stdout);
  /* 显式清理:会话销毁 + 引擎终结,避免进程退出时 librime 内部状态
   * (用户会话/翻译链)的清理路径触发段错误,掩盖真实审计结果。 */
  rime->destroy_session(session);
  rime->finalize();
  return failures == 0 ? 0 : 1;
}
