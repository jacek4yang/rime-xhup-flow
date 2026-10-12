/* Real Lua allocator/GC counter boundaries, including NULL allocator type tags. */
#include "profile_runtime.h"
#include <lua.h>
#include <lauxlib.h>
#include <lualib.h>
#include <stdio.h>
#include <stdlib.h>

static void execute(lua_State *L, const char *script) {
  if (luaL_dostring(L, script) != LUA_OK) {
    fprintf(stderr, "Lua counter check failed: %s\n", lua_tostring(L, -1));
    exit(1);
  }
}

int main(void) {
  lua_State *L = luaL_newstate();
  if (!L) return 1;
  luaL_openlibs(L);
  execute(L, "collectgarbage('collect'); collectgarbage('stop')");
  for (int generational = 0; generational < 2; ++generational) {
    if (generational) execute(L, "collectgarbage('generational'); collectgarbage('stop')");
    struct XhupRuntimeProfile before, retained, released;
    xhup_profile_snapshot(&before);
    execute(L, "payload = {}; for i=1,16 do payload[i]=string.rep('x',65536) end");
    xhup_profile_snapshot(&retained);
    execute(L, "payload=nil; collectgarbage('collect')");
    xhup_profile_snapshot(&released);
    if (retained.live_bytes < before.live_bytes + 1048576 ||
        retained.live_bytes < released.live_bytes + 1048576 ||
        retained.peak_bytes < retained.live_bytes ||
        retained.incremental_collections != before.incremental_collections ||
        retained.generational_collections != before.generational_collections ||
        released.incremental_collections != before.incremental_collections + !generational ||
        released.generational_collections != before.generational_collections + generational) {
      fputs("FAIL retained payload or completed collection accounting\n", stderr);
      return 1;
    }
  }
  lua_close(L);
  struct XhupRuntimeProfile closed;
  xhup_profile_snapshot(&closed);
  if (closed.live_bytes != 0) {
    fputs("FAIL allocator accounting did not return to zero after lua_close\n", stderr);
    return 1;
  }
  puts("PASS real Lua payload release, incremental/generational boundaries and final zero balance");
  return 0;
}
