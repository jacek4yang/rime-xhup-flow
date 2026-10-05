#define main original_replay_main
#include "runtime_replay.c"
#undef main

int main(int argc, char **argv) {
  if (argc != 4) return 2;
  r = rime_get_api();
  RIME_STRUCT(RimeTraits, traits);
  traits.user_data_dir = argv[1]; traits.shared_data_dir = argv[2];
  traits.app_name = "rime.xhup-extended-public-test";
  traits.min_log_level = 2; r->setup(&traits); r->initialize(&traits);
  RimeSessionId s;
  if (!strcmp(argv[3], "--learn-once") || !strcmp(argv[3], "--learn-blocked") || !strcmp(argv[3], "--learn-unavailable")) {
    s=start(); State state=type(s,"single-native-writer","nihcnzqu","你好去",NULL);
    selected_commit(s,&state,"你好去","single-native-writer");
    char status[80]="";
    check(r->get_property(s,"xhup_flow_learning_status",status,sizeof(status)), "native learning status exposed");
    fprintf(stderr, "native learning status=%s\n", status);
    const char *expected = !strcmp(argv[3],"--learn-unavailable") ? "bounded_api_unavailable"
      : (!strcmp(argv[3],"--learn-blocked") ? "quota_exhausted" : "ready");
    check(!strcmp(status,expected), "native learning status matches operation");
    r->destroy_session(s);
  } else if (!strcmp(argv[3], "--stress")) {
    char input[130]="", expected[400]="";
    for (int i=0;i<21;i++) {strcat(input,"nihcnz"); strcat(expected,"你好");}
    strcat(input,"ni"); strcat(expected,"你");
    s=start(); State state=type(s,"128-keys",input,expected,NULL);
    check(strlen(r->get_input(s))==128,"128 raw keys retained");
    selected_commit(s,&state,expected,"128-keys"); r->destroy_session(s);
    s=start(); state=type(s,"cursor","nihcnznihcnzqu","你好你好去",NULL);
    r->set_caret_pos(s,6);
    state=step(s,"cursor",0xff08,NULL,NULL);
    check(!strcmp(state.input,"nihcnnihcnzqu"),"backspace at internal boundary deletes exactly one raw key");
    state=step(s,"cursor",'z',NULL,NULL);
    check(!strcmp(state.input,"nihcnznihcnzqu"),"reinsertion restores raw composition");
    r->set_caret_pos(s,strlen(r->get_input(s)));
    state=snapshot(s,"你好你好去",NULL);
    selected_commit(s,&state,"你好你好去","cursor"); r->destroy_session(s);
    s=start(); state=type(s,"pending-multiple-spaces","nihcnznihcnzq","你好你好",NULL);
    check(state.target_rank>=0,"multi-space pending prefix selectable");
    if(state.target_rank>=0) {
      check(r->select_candidate(s,state.target_rank),"multi-space pending selection accepted");
      RIME_STRUCT(RimeCommit, early); char prior[TEXT]="";
      if(r->get_commit(s,&early)) {
        copy(prior,sizeof(prior),early.text);
        check(!strcmp(prior,"你好你好"),"early commit contains only the selected prefix");
        r->free_commit(&early);
      }
      const char *raw=r->get_input(s);
      check(raw && *raw && raw[strlen(raw)-1]=='q',"pending q retained");
      state=step(s,"pending-multiple-spaces",'u',"去",NULL);
      check(state.target_rank>=0,"tail completion after multi-space prefix");
      if(state.target_rank>=0) {
        check(r->select_candidate(s,state.target_rank),"completed pending tail selection accepted");
        RIME_STRUCT(RimeCommit, final);
        int ok=r->get_commit(s,&final);
        if(!ok) {r->commit_composition(s);RIME_STRUCT_INIT(RimeCommit,final);ok=r->get_commit(s,&final);}
        check(ok,"final commit after multi-space prefix");
        if(ok) {
          char all[TEXT*2]; snprintf(all,sizeof(all),"%s%s",prior,final.text);
          check(!strcmp(all,"你好你好去"),"multi-space prefix and tail committed exactly once");
          r->free_commit(&final);
        }
        check(!r->get_input(s) || !*r->get_input(s),"pending prefix and tail consume all raw input");
        RIME_STRUCT_INIT(RimeCommit,final);
        int duplicate=r->get_commit(s,&final);
        check(!duplicate,"pending prefix and tail produce no duplicate commit");
        if(duplicate) r->free_commit(&final);
      }
    }
    r->destroy_session(s);
  } else return 2;
  r->finalize();
  fprintf(stderr,"RESULT EXTENDED checks=%d failures=%d\n",checks,failures);
  return failures?1:0;
}
