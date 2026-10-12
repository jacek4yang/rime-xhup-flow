/* Observational performance driver; correctness remains in replay/extended.
 * By default only materializes the visible page; complete-menu mode is separate.
 * Use a new isolated deployment for each process, never a real user directory. */
#define XHUP_MEASURE_THREAD_CPU 1
#define main original_replay_main
#include "runtime_replay.c"
#undef main
#include <errno.h>
#include <unistd.h>
static int complete_menu_mode;

static void measure_complete_menu(RimeSessionId s, const char *name) {
  if (!complete_menu_mode) return;
  const int limit = 100000;
  int count = 0, exhausted = 0;
  long long before = now();
  RimeCandidateListIterator it = {0};
  if (r->candidate_list_begin(s, &it)) {
    while (count < limit) {
      if (!r->candidate_list_next(&it)) { exhausted = 1; break; }
      ++count;
    }
    r->candidate_list_end(&it);
  } else {
    exhausted = 1;
  }
  long long elapsed = now() - before;
  printf("{\"case\":"); json(name);
  printf(",\"event\":\"complete_menu\",\"enumeration_ns\":%lld,\"candidates\":%d,"
      "\"exhausted\":%s,\"limit\":%d}\n",
      elapsed, count, exhausted ? "true" : "false", limit);
}

int main(int argc, char **argv) {
  complete_menu_mode = getenv("XHUP_MEASURE_COMPLETE_MENU") != NULL;
  const char *quick_hint = getenv("XHUP_MEASURE_QUICK_HINT");
  if (!quick_hint) quick_hint = "on";
  if (strcmp(quick_hint, "on") && strcmp(quick_hint, "off")) {
    fputs("XHUP_MEASURE_QUICK_HINT must be on or off\n", stderr); return 2;
  }
  quick_hint_enabled = !strcmp(quick_hint, "on");
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
  RimeSessionId persistent = getenv("XHUP_MEASURE_SUSTAINED") ? start() : 0;
  for (long round = 0; round < repetitions; ++round) {
    fprintf(stderr, "XHUP_ROUND %ld\n", round);
    for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
      RimeSessionId s = persistent ? persistent : start();
      type(s, cases[i][0], cases[i][1], NULL, NULL);
      measure_complete_menu(s, cases[i][0]);
      if (persistent) r->clear_composition(s); else r->destroy_session(s);
    }
    RimeSessionId s = persistent ? persistent : start();
    type(s, "middle-edit", "nihcnznihcnzqu", NULL, NULL);
    r->set_caret_pos(s, 6);
    step(s, "middle-edit", 0xff08, NULL, NULL);
    step(s, "middle-edit", 'z', NULL, NULL);
    measure_complete_menu(s, "middle-edit");
    if (persistent) r->clear_composition(s); else r->destroy_session(s);
    s = persistent ? persistent : start();
    for (int i = 0; i < 16; ++i) type(s, "long-128", "nihcnzqu", NULL, NULL);
    measure_complete_menu(s, "long-128");
    type(s, "over-limit", "de", NULL, NULL);
    measure_complete_menu(s, "over-limit");
    for (int i = 0; i < 130; ++i) step(s, "backspace", 0xff08, NULL, NULL);
    measure_complete_menu(s, "backspace");
    if (persistent) r->clear_composition(s); else r->destroy_session(s);
    FILE *memory = fopen("/proc/self/statm", "r");
    unsigned long total_pages, resident_pages;
    if (!memory) return 2;
    int fields = fscanf(memory, "%lu %lu", &total_pages, &resident_pages);
    fclose(memory);
    long page_size = sysconf(_SC_PAGESIZE);
    if (fields != 2 || page_size <= 0) return 2;
    fprintf(stderr, "XHUP_ROUND_RSS_KIB %ld %llu\n", round,
        (unsigned long long)resident_pages * (unsigned long long)page_size / 1024);
    if (profile_snapshot) {
      struct XhupRuntimeProfile memory_profile;
      profile_snapshot(&memory_profile);
      fprintf(stderr, "XHUP_ROUND_LUA_BYTES %ld %llu\n", round,
          (unsigned long long)memory_profile.live_bytes);
    }
  }
  if (persistent) r->destroy_session(persistent);
  r->finalize();
  /* ru_maxrss can retain a pre-exec launcher's high-water mark. VmHWM belongs
   * to this process image and does not misattribute launcher memory to Rime. */
  FILE *status = fopen("/proc/self/status", "r");
  if (!status) return 2;
  char line[256];
  unsigned long long peak_rss = 0;
  while (fgets(line, sizeof(line), status)) {
    if (sscanf(line, "VmHWM: %llu kB", &peak_rss) == 1) break;
  }
  int read_error = ferror(status);
  fclose(status);
  if (read_error || !peak_rss) return 2;
  fprintf(stderr, "XHUP_PEAK_RSS_KIB %llu\n", peak_rss);
  return 0;
}
