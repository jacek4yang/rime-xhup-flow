/* Real deployed Rime selector contract; no UI/GTK simulation. */
#include <rime_api.h>
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
  if (argc != 3) return 2;
  RimeApi *api = rime_get_api();
  RIME_STRUCT(RimeTraits, traits);
  traits.shared_data_dir = argv[1];
  traits.user_data_dir = argv[2];
  traits.distribution_name = "XHUP exclusive schema verification";
  traits.distribution_code_name = "xhup_exclusive_test";
  traits.distribution_version = "1";
  traits.app_name = "rime.xhup_exclusive_test";
  api->setup(&traits);
  api->initialize(&traits);
  int failed = 0;
  RimeSchemaList list = {0};
  if (!api->get_schema_list(&list)) {
    fprintf(stderr, "FAIL schema list unavailable\n"); failed = 1;
  } else {
    if (list.size != 1 || strcmp(list.list[0].schema_id, "xhup_flow")) {
      fprintf(stderr, "FAIL exclusive schema list: count=%zu\n", list.size); failed = 1;
    }
    api->free_schema_list(&list);
  }
  RimeSessionId session = api->create_session();
  char current[128] = "";
  if (!session || !api->get_current_schema(session, current, sizeof(current)) || strcmp(current, "xhup_flow")) {
    fprintf(stderr, "FAIL default active schema: %s\n", current); failed = 1;
  }
  if (session) api->destroy_session(session);
  api->finalize();
  if (!failed) puts("PASS exclusive Rime selector: only xhup_flow; active default xhup_flow");
  return failed;
}
