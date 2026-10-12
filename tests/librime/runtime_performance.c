/* Observational performance driver; correctness remains in replay/extended.
 * Only materializes the visible page, so top-256 probing cannot warm later keys.
 * Use a new isolated deployment for each process, never a real user directory. */
#define main original_replay_main
#include "runtime_replay.c"
#undef main
#include <errno.h>
#include <sys/resource.h>

int main(int argc, char **argv) {
  if (argc != 5) {
    fputs("usage: runtime_performance USER SHARED SCHEMA REPETITIONS\n", stderr);
    return 2;
  }
  char *end;
  errno = 0;
  long repetitions = strtol(argv[4], &end, 10);
  if (errno || *end || repetitions < 1 || repetitions > 1000) return 2;
  schema_id = argv[3];
  if (strcmp(schema_id, "xhup_flow") && strcmp(schema_id, "xhup_flow_static")) return 2;
  page_only = 1;
  init_profile();
  r = rime_get_api();
  RIME_STRUCT(RimeTraits, traits);
  traits.user_data_dir = argv[1]; traits.shared_data_dir = argv[2];
  traits.app_name = "rime.xhup-performance"; traits.min_log_level = 2; traits.log_dir = "";
  long long initialization = now();
  xhup_initialize(r, &traits);
  fprintf(stderr, "XHUP_INITIALIZE_NS %lld\n", now() - initialization);
  if (!r->find_module("lua")) return 2;
  const char *cases[][2] = {
    {"single", "ni"},
    {"word-shortcut", "jqu"},
    {"mixed-shortcut", "jqujbzqu"},
    {"continuous", "nihcnzqu"},
    {"long", "nihcvegeuurufawojtdesuduhduikeyi"},
    {"pending", "nihcvegeuurufawojtdesuduhduikeyid"},
    {"extended", "nihcvegeuurufawojtdesuduhduikeyide"},
  };
  for (long round = 0; round < repetitions; ++round) {
    fprintf(stderr, "XHUP_ROUND %ld\n", round);
    for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
      RimeSessionId s = start();
      type(s, cases[i][0], cases[i][1], NULL, NULL);
      r->destroy_session(s);
    }
    RimeSessionId s = start();
    type(s, "middle-edit", "nihcnznihcnzqu", NULL, NULL);
    r->set_caret_pos(s, 6);
    step(s, "middle-edit", 0xff08, NULL, NULL);
    step(s, "middle-edit", 'z', NULL, NULL);
    r->destroy_session(s);
    s = start();
    for (int i = 0; i < 16; ++i) type(s, "long-128", "nihcnzqu", NULL, NULL);
    type(s, "over-limit", "de", NULL, NULL);
    for (int i = 0; i < 130; ++i) step(s, "backspace", 0xff08, NULL, NULL);
    r->destroy_session(s);
  }
  r->finalize();
  struct rusage usage;
  if (getrusage(RUSAGE_SELF, &usage)) return 2;
  fprintf(stderr, "XHUP_PEAK_RSS_KIB %ld\n", usage.ru_maxrss);
  return 0;
}
