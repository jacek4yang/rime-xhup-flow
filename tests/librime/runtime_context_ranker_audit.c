/* Real librime: bounded session ranking, explicit consent, no secondary store.
 * Historical TSV persistence is intentionally retired; native learning has its
 * separate exact-count/export/import/restart gate in run-flow-audit.sh. */
#include <rime_api.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static RimeApi *rime;
static RimeSessionId session;
static int checks, failures;

static void check(int ok, const char *name) {
  ++checks;
  if (!ok) ++failures;
  printf("%s %s\n", ok ? "PASS" : "FAIL", name);
  fflush(stdout);
}
static void type(const char *keys) {
  for (const char *p = keys; *p; ++p) {
    if (!rime->process_key(session, *p, 0)) {
      fprintf(stderr, "FAIL key not processed\n");
      exit(2);
    }
  }
}
static void clear(void) { rime->clear_composition(session); }
static int candidate(int rank, char *out, size_t cap) {
  RIME_STRUCT(RimeContext, c);
  out[0] = 0;
  if (!rime->get_context(session, &c)) return 0;
  int found = rank >= 0 && rank < c.menu.num_candidates;
  if (found) snprintf(out, cap, "%s", c.menu.candidates[rank].text);
  rime->free_context(&c);
  return found;
}
static void menu(char *out, size_t cap) {
  size_t used = 0;
  out[0] = 0;
  RIME_STRUCT(RimeContext, c);
  if (!rime->get_context(session, &c)) return;
  for (int i = 0; i < c.menu.num_candidates; ++i) {
    const char *text = c.menu.candidates[i].text;
    size_t len = strlen(text);
    if (used + len + 2 >= cap) { check(0, "capture capacity"); break; }
    if (used) out[used++] = '\n';
    memcpy(out + used, text, len);
    used += len; out[used] = 0;
  }
  rime->free_context(&c);
}
static int property(const char *expected) {
  char value[32] = "";
  return rime->get_property(session, "xhup_flow_session_memory_entries", value,
                            sizeof(value)) && strcmp(value, expected) == 0;
}
static void commit_rank(const char *code, int rank) {
  char expected[1024];
  clear(); type(code);
  if (!candidate(rank, expected, sizeof(expected))) { check(0, "commit candidate exists"); return; }
  check(rime->select_candidate(session, rank), "native selection accepted");
  RIME_STRUCT(RimeCommit, c);
  int got = rime->get_commit(session, &c);
  if (!got) {
    rime->commit_composition(session);
    RIME_STRUCT_INIT(RimeCommit, c);
    got = rime->get_commit(session, &c);
  }
  check(got, "real commit observed");
  if (got) {
    check(strcmp(c.text, expected) == 0, "exact selected text committed");
    rime->free_commit(&c);
  }
  const char *raw = rime->get_input(session);
  check(!raw || !*raw, "commit consumes entire input");
}
static RimeSessionId start(void) {
  RimeSessionId id = rime->create_session();
  if (!id || !rime->select_schema(id, "xhup_flow")) exit(2);
  return id;
}
int main(int argc, char **argv) {
  if (argc != 3) return 2;
  rime = rime_get_api();
  RIME_STRUCT(RimeTraits, traits);
  traits.app_name = "rime.xhup-session-evidence-audit";
  traits.shared_data_dir = argv[1]; traits.user_data_dir = argv[2];
  traits.min_log_level = 2;
  rime->setup(&traits); rime->initialize(&traits);
  if (rime->is_maintenance_mode()) rime->join_maintenance_thread();
  check(rime->find_module("lua") != NULL, "actual Lua registration");
  session = start();
  check(property("0"), "new process/session does not load historical TSV");
  rime->set_option(session, "context_ranker", 0);
  rime->set_option(session, "user_memory", 0);
  static char off[65536], on[65536], off_again[65536], text[1024], evidence[1024];
  type("uijm"); menu(off, sizeof(off)); clear();
  rime->set_option(session, "context_ranker", 1);
  type("uijm"); menu(on, sizeof(on)); clear();
  check(strcmp(off, on) == 0, "no evidence: exact menu identity");

  type("uijm");
  check(candidate(1, evidence, sizeof(evidence)), "independent rank-2 evidence exists");
  clear();
  /* Consent precedes the actual commit, never retrospective observation. */
  commit_rank("uijm", 1);
  type("uijm"); menu(on, sizeof(on));
  int hit = 0;
  for (int i = 0; i < 3; ++i)
    if (candidate(i, text, sizeof(text)) && strcmp(text, evidence) == 0) hit = 1;
  clear();
  rime->set_option(session, "context_ranker", 0);
  type("uijm"); menu(off_again, sizeof(off_again)); clear();
  check(strcmp(on, off_again) != 0, "consented repeat changes bounded head");
  check(hit, "repeat remains inside global head");
  check(strstr(on, evidence) && strstr(off_again, evidence), "on/off preserve evidence candidate");
  check(property("0"), "ranker-only keeps no frequency history");

  rime->set_option(session, "context_ranker", 1);
  type("uij"); check(candidate(0, text, sizeof(text)) && !strcmp(text, "时间"),
                     "static fixed-first protection"); clear();
  type("uiui"); int oov_on = candidate(0, text, sizeof(text)); clear();
  rime->set_option(session, "context_ranker", 0);
  type("uiui"); int oov_off = candidate(0, text, sizeof(text)); clear();
  check(oov_on && oov_off, "open composition survives option toggle");

  rime->set_option(session, "user_memory", 1);
  for (int i = 0; i < 25; ++i) commit_rank("uij", 0);
  check(property("1"), "bounded session history contains one distinct word");
  RimeSessionId first = session;
  session = start();
  check(property("0"), "second engine cannot inherit first engine state");
  rime->destroy_session(session); session = first;
  check(property("1"), "second engine lifecycle cannot erase first engine state");
  rime->set_option(session, "user_memory", 0);
  check(property("0"), "turning off clears session frequency history immediately");
  commit_rank("uij", 0);
  check(property("0"), "off commit creates no history");
  rime->set_option(session, "user_memory", 1);
  commit_rank("uij", 0);
  check(property("1"), "re-enabled memory starts fresh");
  rime->destroy_session(session);
  session = start();
  check(property("0"), "session destruction erases transient state");
  type("uijm"); menu(off, sizeof(off)); clear();
  rime->set_option(session, "context_ranker", 1);
  type("uijm"); menu(on, sizeof(on)); clear();
  check(strcmp(off, on) == 0, "new session has no stale ranking evidence");
  rime->destroy_session(session); rime->finalize();
  printf("RESULT session evidence checks=%d failures=%d\n", checks, failures);
  return failures ? 1 : 0;
}
