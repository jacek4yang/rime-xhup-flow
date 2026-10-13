/* Source-external observational quality experiment. Rank misses are reported,
 * not silently converted to test passes; input/commit invariants remain hard. */
#define main original_replay_main
#include "runtime_replay.c"
#undef main
#include "candidate_match.h"

int main(int argc, char **argv) {
  if (argc != 5) return 2;
  init_profile();
  FILE *fixture = fopen(argv[3], "r");
  if (!fixture) return 2;
  r = rime_get_api();
  RIME_STRUCT(RimeTraits, traits);
  traits.user_data_dir = argv[1]; traits.shared_data_dir = argv[2];
  traits.app_name = "rime.xhup-source-external"; traits.min_log_level = 2;
  xhup_initialize(r, &traits);
  check(r->find_module("lua") != NULL, "Lua module registered");
  RimeConfig config = {0};
  if (!r->schema_open("xhup_flow", &config)) return 2;
  const char *settings[] = {"flow/enable_user_dict", "flow_lookup/enable_user_dict", "flow_readonly/enable_user_dict"};
  for (int i=0;i<3;i++) {
    Bool enabled = True;
    check(r->config_get_bool(&config, settings[i], &enabled) && !enabled,
          "compiled native learning disabled");
  }
  char provider[160]="";
  check(r->config_get_string(&config, "engine/translators/@2", provider, sizeof(provider)),
        "compiled provider readable");
  const int ablation = !strcmp(argv[4], "native-only");
  check(!strcmp(argv[4], "planner") || ablation, "known evaluation mode");
  check(!strcmp(provider, ablation ? "lua_translator@*corpus_native_only"
                                  : "lua_translator@*xhup_flow.native_tail"),
        "compiled provider matches reported mode");
  r->config_close(&config);
  if (failures) return 1;
  char line[4096]; int cases=0, reached=0;
  while (fgets(line, sizeof(line), fixture)) {
    if (!strchr(line,'\n')) return 2;
    line[strcspn(line,"\r\n")]=0;
    char *name=line, *keys=strchr(name,'\t');
    if (!keys) return 2;
    *keys++=0;
    char *target=strchr(keys,'\t');
    if (!target) return 2;
    *target++=0;
    size_t length=strlen(keys);
    if (!*name || !*target || strchr(target,'\t') || length<4 || length>128 ||
        strspn(keys,"abcdefghijklmnopqrstuvwxyz")!=length) return 2;
    int before=failures, raw_ok=1, early_ok=1;
    RimeSessionId s=start();
    State state={0}, previous={0};
    for (size_t i=0;i<length;i++) {
      previous=state;
      state=step(s,name,(unsigned char)keys[i],target,NULL);
      raw_ok &= strlen(state.input)==i+1 && !strncmp(state.input,keys,i+1);
      RIME_STRUCT(RimeCommit, early);
      if (r->get_commit(s,&early)) { early_ok=0; r->free_commit(&early); }
    }
    check(raw_ok,"corpus keeps every raw key");
    check(early_ok,"corpus never commits automatically");
    int joint = r->get_option(s, "joint_decoder");
    char joint_status[96] = "";
    r->get_property(s, "xhup_flow_joint_decoder_status", joint_status, sizeof(joint_status));
    if (joint && !ablation)
      check(!strcmp(joint_status, "ready") || !strcmp(joint_status, "bounded"),
            "joint evaluation ran the requested decoder without silent fallback");
    State undone=step(s,name,0xff08,target,NULL);
    int undo_equal=equal(&previous,&undone);
    check(undo_equal,"corpus backspace restores captured prefix");
    State redone=step(s,name,(unsigned char)keys[length-1],target,NULL);
    int redo_equal=equal(&state,&redone);
    check(redo_equal,"corpus retyping restores captured final state");
    /* Report continuation availability without assuming ambiguous text must be
     * present. Preservation of an already reachable target is measured. */
    State pending=step(s,name,'d',target,target);
    int pending_reachable=pending.target_rank>=0;
    State restored=step(s,name,0xff08,target,NULL);
    check(equal(&state,&restored),"pending-key removal restores captured final state");
    XhupCandidateMatch match;
    check(xhup_candidate_match(s, target, LOOKUP, &match),
          "native candidate ranges are available and within raw input");
    check(match.text_rank == state.target_rank, "SDK ranges refer to the same visible C API candidates");
    /* A legal short-code prefix can equal the target text while leaving keys.
     * Keep it in the evidence, but never count it as complete sentence recall. */
    state.target_rank = restored.target_rank = match.complete_rank;
    int commit_before=failures;
    if (state.target_rank>=0) {
      reached++;
      selected_commit(s,&restored,target,name);
    }
    printf("{\"case\":"); json(name);
    printf(",\"event\":\"corpus_result\",\"mode\":"); json(argv[4]);
    printf(",\"joint_decoder\":%s,\"joint_status\":", joint ? "true" : "false"); json(joint_status);
    printf(",\"rank_contract\":\"complete_raw_span_v1\",\"text_rank\":%d,\"text_start\":%zu,\"text_end\":%zu,\"complete_start\":%zu,\"complete_end\":%zu",
      match.text_rank, match.start, match.end, match.complete_start, match.complete_end);
    printf(",\"keys\":%zu,\"rank\":%d,\"top1\":%s,\"top3\":%s,\"first_page\":%s,\"page_size\":%d,\"pending_reachable\":%s,\"commit_exact\":%s,\"contract_failures\":%d}\n",
      length,state.target_rank,state.target_rank==0?"true":"false",
      state.target_rank>=0 && state.target_rank<3?"true":"false",
      state.target_rank>=0 && state.target_rank<state.page_size?"true":"false",
      state.page_size,pending_reachable?"true":"false",
      state.target_rank>=0 && failures==commit_before ? "true":"false",failures-before);
    r->destroy_session(s); cases++;
  }
  if (ferror(fixture)) return 2;
  fclose(fixture); r->finalize();
  check(cases>0,"nonempty external corpus");
  fprintf(stderr,"SOURCE_EXTERNAL_OBSERVATION cases=%d reachable=%d checks=%d failures=%d\n",
          cases,reached,checks,failures);
  return failures?1:0;
}
