/* Source-external observational quality experiment. Rank misses are reported,
 * not silently converted to test passes; input/commit invariants remain hard. */
#define main original_replay_main
#include "runtime_replay.c"
#undef main

int main(int argc, char **argv) {
  if (argc != 5) return 2;
  FILE *fixture = fopen(argv[3], "r");
  if (!fixture) return 2;
  r = rime_get_api();
  RIME_STRUCT(RimeTraits, traits);
  traits.user_data_dir = argv[1]; traits.shared_data_dir = argv[2];
  traits.app_name = "rime.xhup-source-external"; traits.min_log_level = 2;
  r->setup(&traits); r->initialize(&traits);
  check(r->find_module("lua") != NULL, "Lua module registered");
  RimeConfig config = {0};
  if (!r->schema_open("xhup_flow", &config)) return 2;
  const char *settings[] = {"flow/enable_user_dict", "learn/enable_user_dict", "flow_readonly/enable_user_dict"};
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
    int commit_before=failures;
    if (state.target_rank>=0) {
      reached++;
      selected_commit(s,&restored,target,name);
    }
    printf("{\"case\":"); json(name);
    printf(",\"event\":\"corpus_result\",\"mode\":"); json(argv[4]);
    printf(",\"keys\":%zu,\"rank\":%d,\"pending_reachable\":%s,\"commit_exact\":%s,\"contract_failures\":%d}\n",
      length,state.target_rank,pending_reachable?"true":"false",
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
