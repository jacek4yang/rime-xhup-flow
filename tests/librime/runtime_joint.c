/* Real mixed-alias selection and raw-span regression; not a scoring benchmark. */
#define main replay_main
#include "runtime_replay.c"
#undef main

static void annotation(RimeSessionId s, int rank, const char *name) {
  RimeCandidateListIterator it = {0};
  if (r->candidate_list_begin(s, &it)) {
    for (int i = 0; i <= rank && i < LOOKUP && r->candidate_list_next(&it); ++i) {
      if (i == rank) {
        printf("{\"case\":"); json(name);
        printf(",\"event\":\"candidate_annotation\",\"text\":"); json(it.candidate.text ? it.candidate.text : "");
        printf(",\"comment\":"); json(it.candidate.comment ? it.candidate.comment : ""); puts("}");
      }
    }
    r->candidate_list_end(&it);
  }
}

static void compound_learning_case(int joint) {
  RimeSessionId s = start();
  r->set_option(s, "joint_decoder", joint);
  State state = type(s, "compound-learning", "nirxhcnz", "你好", NULL);
  annotation(s, state.target_rank, "compound-learning");
  selected_commit(s, &state, "你好", "compound commits with quick hints enabled");
  char status[96] = "unavailable";
  r->get_property(s, "xhup_flow_learning_status", status, sizeof(status));
  check(!strcmp(status, "ready"), "complete native choice uses the active bounded writer");
  r->destroy_session(s);
}

static void learning_case(const char *mode) {
  int resumed = !strcmp(mode, "--learned");
  int quota = !strcmp(mode, "--quota");
  const char *target = "就是拟好";
  RimeSessionId s = start();
  r->set_option(s, "joint_decoder", 1);
  State before = type(s, "joint-learning-before", "jqunihc", target, NULL);
  check(resumed ? before.target_rank == 0 : before.target_rank > 0,
        resumed ? "learned choice survives process restart" : "fresh choice is reachable but not first");
  selected_commit(s, &before, target, "non-default mixed-code choice commits completely");
  char status[96] = "unavailable";
  r->get_property(s, "xhup_flow_learning_status", status, sizeof(status));
  check(!strcmp(status, quota ? "quota_exhausted" : "ready"),
        "native writer reports the expected learning outcome");
  State after = type(s, "joint-learning-after", "jqunihc", target, NULL);
  check(quota ? after.target_rank > 0 : after.target_rank == 0,
        quota ? "refused learning cannot influence the next query" : "actual choice becomes first without restarting");
  r->get_property(s, "xhup_flow_joint_decoder_status", status, sizeof(status));
  check(!strcmp(status, "ready") || !strcmp(status, "bounded"),
        "personalized search does not silently degrade");
  r->clear_composition(s);
  r->destroy_session(s);
}

static void cursor_case(void) {
  RimeSessionId s = start();
  r->set_option(s, "joint_decoder", 1);
  type(s, "mixed-cursor", "jqunihc", "就是你好", NULL);
  r->set_caret_pos(s, 3);
  State state = step(s, "mixed-cursor", 0xff08, NULL, NULL);
  check(!strcmp(state.input, "jqnihc"), "editing an alias boundary deletes one original key");
  state = step(s, "mixed-cursor", 'u', NULL, NULL);
  check(!strcmp(state.input, "jqunihc"), "restoring an alias boundary preserves the suffix");
  state = type(s, "mixed-cursor", "nirx", NULL, NULL);
  check(!strcmp(state.input, "jqunirxnihc"), "middle insertion preserves both original sides");
  r->set_caret_pos(s, strlen(r->get_input(s)));
  state = snapshot(s, "就是你你好", NULL);
  selected_commit(s, &state, "就是你你好", "edited mixed-code sentence commits completely");
  r->clear_composition(s);
  state = snapshot(s, NULL, NULL);
  check(!state.input[0] && !state.count, "clearing leaves neither raw input nor stale joint candidates");
  r->destroy_session(s);
}

static void encoded_learning_case(const char *mode) {
  int seed = !strcmp(mode, "--seed-legacy");
  RimeSessionId s = start();
  r->set_option(s, "joint_decoder", !strcmp(mode, "--recall-joint"));
  r->set_option(s, "debug_candidate_annotations", !strcmp(mode, "--recall-joint"));
  State state = type(s, mode, seed ? "nihcnznihcnzqu" : "nhnhq", "你好你好去", NULL);
  if (!seed) check(state.target_rank == 0, "native learned compound remains first across decoder policies");
  annotation(s, state.target_rank, mode);
  selected_commit(s, &state, "你好你好去", "native encoded compound commits completely");
  r->destroy_session(s);
}

static void first_choice_case(const char *keys, const char *expected) {
  RimeSessionId s = start();
  r->set_option(s, "joint_decoder", 1);
  State state = type(s, "first-choice", keys, expected, NULL);
  check(state.target_rank == 0, "full target is first, not merely reachable");
  RIME_STRUCT(RimeCommit, commit);
  int early = r->get_commit(s, &commit);
  check(!early, "typing does not commit before explicit selection");
  if (early) r->free_commit(&commit);
  r->process_key(s, ' ', 0);
  int committed = r->get_commit(s, &commit);
  check(committed && commit.text && !strcmp(commit.text, expected),
        "space selects and commits the exact full first choice");
  if (committed) r->free_commit(&commit);
  check(!r->get_input(s) || !*r->get_input(s), "first-choice selection consumes all raw keys");
  int duplicate = r->get_commit(s, &commit);
  check(!duplicate, "first-choice selection has no duplicate commit");
  if (duplicate) r->free_commit(&commit);
  r->destroy_session(s);
}

int main(int argc, char **argv) {
  if (argc != 3 && argc != 4) return 2;
  if (argc == 4 && strcmp(argv[3], "--learn") && strcmp(argv[3], "--learned")
      && strcmp(argv[3], "--quota") && strcmp(argv[3], "--seed-legacy")
      && strcmp(argv[3], "--recall-legacy") && strcmp(argv[3], "--recall-joint")
      && strcmp(argv[3], "--learn-compound") && strcmp(argv[3], "--native-compound")) return 2;
  r = rime_get_api();
  quick_hint_enabled = 1;
  RIME_STRUCT(RimeTraits, traits);
  traits.user_data_dir = argv[1]; traits.shared_data_dir = argv[2];
  traits.app_name = "xhup.joint-regression"; traits.min_log_level = 2; traits.log_dir = "";
  xhup_initialize(r, &traits);
  check(r->find_module("lua") != NULL, "real Lua module available");
  if (argc == 4) {
    if (!strcmp(argv[3], "--learn-compound") || !strcmp(argv[3], "--native-compound"))
      compound_learning_case(!strcmp(argv[3], "--learn-compound"));
    else if (!strcmp(argv[3], "--learn") || !strcmp(argv[3], "--learned") || !strcmp(argv[3], "--quota"))
      learning_case(argv[3]);
    else encoded_learning_case(argv[3]);
    r->finalize();
    fprintf(stderr, "RESULT JOINT LEARNING checks=%d failures=%d\n", checks, failures);
    return failures ? 1 : 0;
  }
  const char *cases[][2] = {
    {"jqunihc", "就是你好"}, {"nihcjqu", "你好就是"},
    {"jqunirx", "就是你"}, {"nirxjqu", "你就是"},
    {"jqujqu", "就是就是"}, {"dnirx", "的你"},
    {"jqunihcjqu", "就是你好就是"},
  };
  for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
    RimeSessionId s = start();
    r->set_option(s, "joint_decoder", 1);
    State state = type(s, cases[i][0], cases[i][0], cases[i][1], NULL);
    char status[96] = "unavailable";
    r->get_property(s, "xhup_flow_joint_decoder_status", status, sizeof(status));
    check(!strcmp(status, "ready") || !strcmp(status, "bounded"), "joint search actually ran; no silent legacy fallback");
    selected_commit(s, &state, cases[i][1], cases[i][0]);
    r->destroy_session(s);
  }
  const char *first_choices[][2] = {
    {"q", "去"}, {"jqu", "就是"}, {"jbzqu", "进去"},
    {"nihcnzqu", "你好去"}, {"nihcnznihcnzqu", "你好你好去"},
    {"nihcvegeuurufawojtdesuduhduikeyi", "你好这个输入法我觉得速度还是可以"},
  };
  for (size_t i = 0; i < sizeof(first_choices) / sizeof(first_choices[0]); ++i)
    first_choice_case(first_choices[i][0], first_choices[i][1]);
  cursor_case();
  RimeSessionId s = start();
  r->set_option(s, "joint_decoder", 1);
  State pending = type(s, "mixed-pending", "jqunihcd", "就是你好", NULL);
  check(pending.target_rank >= 0, "decoded alias prefix remains selectable with a tail key");
  annotation(s, pending.target_rank, "mixed-pending");
  if (pending.target_rank >= 0) {
    check(r->select_candidate(s, (size_t)pending.target_rank), "partial native Phrase accepted");
    RIME_STRUCT(RimeCommit, commit);
    int early = r->get_commit(s, &commit);
    check(!early, "prefix selection does not prematurely commit unfinished input");
    if (early) r->free_commit(&commit);
    check(r->get_input(s) && !strcmp(r->get_input(s), "jqunihcd"), "partial selection preserves original raw keys including tail");
    State completed = step(s, "mixed-pending", 'e', "的", NULL);
    selected_commit(s, &completed, "就是你好的", "selected prefix and completed tail commit together");
  }
  r->destroy_session(s);
  r->finalize();
  fprintf(stderr, "RESULT JOINT checks=%d failures=%d\n", checks, failures);
  return failures ? 1 : 0;
}
