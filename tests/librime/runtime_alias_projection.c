/* Launch the exhaustive read-only projection audit in a real Lua/Rime session. */
#include "runtime_module.h"

int main(int argc, char **argv) {
  if (argc != 3) return 2;
  RimeApi *api = rime_get_api();
  RIME_STRUCT(RimeTraits, traits);
  traits.user_data_dir = argv[1];
  traits.shared_data_dir = argv[2];
  traits.app_name = "xhup.alias-projection-audit";
  traits.min_log_level = 2;
  traits.log_dir = "";
  xhup_initialize(api, &traits);
  RimeSessionId session = api->create_session();
  int selected = session && api->select_schema(session, "xhup_flow");
  if (session) api->destroy_session(session);
  api->finalize();
  /* The driver also requires the complete Lua receipt and rejects Lua errors. */
  return selected ? 0 : 1;
}
