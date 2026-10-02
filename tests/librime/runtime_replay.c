/* Real librime per-key contracts. Bounded top-256 reachability, not exhaustive menu proof.
 * No user corpus: isolated deployment, fixed public regression strings, learning off.
 */
#define _POSIX_C_SOURCE 200809L
#include <rime_api.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define LOOKUP 256
#define HEAD 16
#define TEXT 2048
static RimeApi *r;
static int failures, checks, observe;
typedef struct {
  char input[256], preedit[TEXT], menu[HEAD][TEXT];
  int count, target_rank, prefix_rank, sel_start, sel_end;
} State;

static void copy(char *dst, size_t size, const char *src) {
  if (!src) src = "";
  if (strlen(src) >= size) { fputs("snapshot buffer exceeded\n", stderr); exit(2); }
  strcpy(dst, src);
}
static void json(const char *s) {
  putchar('"');
  for (; *s; ++s) {
    unsigned char c = (unsigned char)*s;
    if (c == '"' || c == '\\') { putchar('\\'); putchar(c); }
    else if (c < 32) printf("\\u%04x", c);
    else putchar(c);
  }
  putchar('"');
}
static long long now(void) {
  struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
  return (long long)t.tv_sec * 1000000000LL + t.tv_nsec;
}
static void check(int ok, const char *contract) {
  ++checks; if (!ok) ++failures;
  fprintf(stderr, "%s %s\n", ok ? "PASS" : "FAIL", contract);
}
static RimeSessionId start(void) {
  RimeSessionId s = r->create_session();
  if (!s || !r->select_schema(s, "xhup_flow")) { fputs("Flow unavailable\n", stderr); exit(2); }
  r->set_option(s, "context_ranker", 0);
  r->set_option(s, "joint_decoder", 0);
  r->set_option(s, "quick_hint", 0);
  r->set_option(s, "user_memory", 0);
  return s;
}
static State snapshot(RimeSessionId s, const char *target, const char *prefix) {
  State out = {0}; out.target_rank = out.prefix_rank = -1;
  copy(out.input, sizeof(out.input), r->get_input(s));
  RIME_STRUCT(RimeContext, ctx);
  if (r->get_context(s, &ctx)) {
    copy(out.preedit, sizeof(out.preedit), ctx.composition.preedit);
    out.sel_start = ctx.composition.sel_start; out.sel_end = ctx.composition.sel_end;
    r->free_context(&ctx);
  }
  RimeCandidateListIterator it = {0};
  if (r->candidate_list_begin(s, &it)) {
    for (int i = 0; i < LOOKUP && r->candidate_list_next(&it); ++i) {
      const char *text = it.candidate.text ? it.candidate.text : "";
      if (i < HEAD) { copy(out.menu[i], TEXT, text); ++out.count; }
      if (target && !strcmp(text, target) && out.target_rank < 0) out.target_rank = i;
      if (prefix && !strncmp(text, prefix, strlen(prefix)) && out.prefix_rank < 0) out.prefix_rank = i;
    }
    r->candidate_list_end(&it);
  }
  return out;
}
static State step(RimeSessionId s, const char *name, int key, const char *target, const char *prefix) {
  long long before = now(); int handled = r->process_key(s, key, 0);
  long long elapsed = now() - before;
  State state = snapshot(s, target, prefix);
  printf("{\"case\":"); json(name);
  printf(",\"key\":%d,\"handled\":%d,\"keypress_ns\":%lld,\"raw\":", key, handled, elapsed); json(state.input);
  printf(",\"preedit\":"); json(state.preedit);
  printf(",\"selection\":[%d,%d],\"target_rank\":%d,\"prefix_rank\":%d,\"head\":[",
      state.sel_start, state.sel_end, state.target_rank, state.prefix_rank);
  for (int i = 0; i < state.count; ++i) { if (i) putchar(','); json(state.menu[i]); }
  puts("]}");
  return state;
}
static int equal(const State *a, const State *b) {
  if (strcmp(a->input, b->input) || strcmp(a->preedit, b->preedit) ||
      a->count != b->count || a->target_rank != b->target_rank ||
      a->prefix_rank != b->prefix_rank || a->sel_start != b->sel_start || a->sel_end != b->sel_end) return 0;
  for (int i = 0; i < a->count; ++i) if (strcmp(a->menu[i], b->menu[i])) return 0;
  return 1;
}
static State type(RimeSessionId s, const char *name, const char *keys, const char *target, const char *prefix) {
  State state = {0};
  for (const char *p = keys; *p; ++p) state = step(s, name, (unsigned char)*p, target, prefix);
  return state;
}
static void selected_commit(RimeSessionId s, const State *state, const char *target, const char *name) {
  check(state->target_rank >= 0, name);
  if (state->target_rank < 0) return;
  check(r->select_candidate(s, (size_t)state->target_rank), "native selection accepted");
  RIME_STRUCT(RimeCommit, commit);
  if (!r->get_commit(s, &commit)) {
    r->commit_composition(s);
    RIME_STRUCT_INIT(RimeCommit, commit);
    if (!r->get_commit(s, &commit)) { check(0, "selection produces commit"); return; }
  }
  printf("{\"case\":"); json(name);
  printf(",\"event\":\"selection_commit\",\"commit\":"); json(commit.text ? commit.text : "");
  printf(",\"remaining_raw\":"); json(r->get_input(s) ? r->get_input(s) : ""); puts("}");
  check(commit.text && !strcmp(commit.text, target), "selected target committed exactly once");
  check(!r->get_input(s) || !*r->get_input(s), "full candidate consumes all input");
  r->free_commit(&commit);
  RIME_STRUCT_INIT(RimeCommit, commit);
  int duplicate = r->get_commit(s, &commit);
  check(!duplicate, "commit drained (no duplicate)");
  if (duplicate) r->free_commit(&commit);
}
int main(int argc, char **argv) {
  if (argc != 4) { fprintf(stderr, "usage: %s USER SHARED --qualify|--observe\n", argv[0]); return 2; }
  observe = !strcmp(argv[3], "--observe");
  if (!observe && strcmp(argv[3], "--qualify")) return 2;
  r = rime_get_api();
  RIME_STRUCT(RimeTraits, traits);
  traits.user_data_dir = argv[1]; traits.shared_data_dir = argv[2];
  traits.app_name = "xhup.native-replay"; traits.min_log_level = 2; traits.log_dir = "";
  r->setup(&traits); r->initialize(&traits);
  check(r->find_module("lua") != NULL, "librime Lua module registered");
  RimeConfig config = {0};
  if (!r->schema_open("xhup_flow", &config)) return 2;
  Bool enabled = True;
  check(r->config_get_bool(&config, "flow/enable_user_dict", &enabled) && !enabled,
        "Flow native learning genuinely disabled in compiled fixture");
  enabled = True;
  check(r->config_get_bool(&config, "learn/enable_user_dict", &enabled) && !enabled,
        "learn translator native learning genuinely disabled in compiled fixture");
  r->config_close(&config);
  if (!RIME_API_AVAILABLE(r, candidate_list_begin) || !RIME_API_AVAILABLE(r, get_input)) return 2;

  /* #151: legal jbz + pending q, then complete qu. No demand that ambiguous
   * jb+zq top-1 already be 进去; require the valid 进 prefix remain represented. */
  RimeSessionId s = start();
  State short_state = type(s, "151", "jbzq", "进去", "进");
  check(short_state.prefix_rank >= 0, "#151 intermediate 进 prefix exists in top256");
  check(!strcmp(short_state.input, "jbzq"), "#151 raw pending keys retained");
  State complete = step(s, "151", 'u', "进去", "进");
  check(complete.target_rank >= 0, "#151 jbzqu reaches 进去 in top256");
  State undone = step(s, "151", 0xff08, "进去", "进");
  check(equal(&short_state, &undone), "#151 backspace restores full captured state");
  State redone = step(s, "151", 'u', "进去", "进");
  check(equal(&complete, &redone), "#151 extension deterministic");
  selected_commit(s, &redone, "进去", "#151 selectable target");
  r->destroy_session(s);

  /* #150: pending first key must not erase a reachable complete prefix. */
  const char *keys = "nihcvegeuurufawojtdesuduhduikeyi";
  const char *sentence = "你好这个输入法我觉得速度还是可以";
  s = start();
  State base = type(s, "150", keys, sentence, sentence);
  check(base.target_rank >= 0, "#150 complete sentence reachable in top256");
  State pending = step(s, "150", 'd', sentence, sentence);
  check(pending.prefix_rank >= 0, "#150 pending d preserves decoded sentence prefix");
  char expected[256]; snprintf(expected, sizeof(expected), "%sd", keys);
  check(!strcmp(pending.input, expected), "#150 pending d remains in raw input");
  State restored = step(s, "150", 0xff08, sentence, sentence);
  check(equal(&base, &restored), "#150 backspace restores complete prefix state");
  step(s, "150", 'd', sentence, sentence);
  State extended = step(s, "150", 'e', "你好这个输入法我觉得速度还是可以的", sentence);
  check(extended.prefix_rank >= 0, "#150 de extension preserves decoded sentence prefix");
  r->clear_composition(s);
  State cleared = snapshot(s, NULL, NULL);
  check(!cleared.input[0], "clear/reset empties raw composition");
  r->destroy_session(s);

  /* Real character mappings, not fixed chunking or dictionary additions. */
  const char *left[] = {"ni", "nir", "nirx"}, *right[] = {"hc", "hcn", "hcnz"};
  for (int i = 0; i < 3; ++i) for (int j = 0; j < 3; ++j) {
    char code[16], name[32]; snprintf(code, sizeof(code), "%s%s", left[i], right[j]);
    snprintf(name, sizeof(name), "segmentation-%d+%d", i+2, j+2);
    s = start(); State a = type(s, name, code, "你好", "你");
    check(a.target_rank >= 0, name);
    r->destroy_session(s); s = start();
    State b = type(s, name, code, "你好", "你");
    check(equal(&a, &b), "new-session replay deterministic");
    selected_commit(s, &b, "你好", name);
    r->destroy_session(s);
  }
  r->finalize();
  fprintf(stderr, "RESULT %s checks=%d failures=%d lookup_bound=%d captured_head=%d\n",
      observe ? "OBSERVATION_NOT_ACCEPTANCE" : "QUALIFICATION", checks, failures, LOOKUP, HEAD);
  return failures && !observe ? 1 : 0;
}
