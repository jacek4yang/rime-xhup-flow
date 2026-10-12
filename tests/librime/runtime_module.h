/* Test harness module identity. Include before any system headers. */
#ifndef XHUP_RUNTIME_MODULE_H
#define XHUP_RUNTIME_MODULE_H
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include <rime_api.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>

static void xhup_verify_lua_module(RimeApi *api, const char *expected) {
  RimeModule *module = api->find_module("lua");
  Dl_info info = {0};
  if (!module || !dladdr(module, &info) || !info.dli_fname) {
    if (!expected) return;
    fputs("pinned Lua module identity unavailable\n", stderr);
    exit(2);
  }
  if (expected) {
    struct stat actual, wanted;
    if (stat(info.dli_fname, &actual) || stat(expected, &wanted) ||
        actual.st_dev != wanted.st_dev || actual.st_ino != wanted.st_ino) {
      fprintf(stderr, "Lua module mismatch: expected %s, loaded %s\n", expected, info.dli_fname);
      exit(2);
    }
  }
  fprintf(stderr, "XHUP_LUA_LIBRARY %s\n", info.dli_fname);
}

static void xhup_initialize(RimeApi *api, RimeTraits *traits) {
  const char *expected = getenv("XHUP_LUA_LIBRARY");
  if (expected) {
    /* `default` contains core/dict/gears; omit the distro's `plugins` scanner,
     * which can replace an already preloaded module with another .so. */
    static const char *modules[] = {"default", "lua", NULL};
    if (!*expected || !dlopen(expected, RTLD_NOW | RTLD_GLOBAL)) {
      fprintf(stderr, "cannot load pinned Lua module: %s\n", expected);
      exit(2);
    }
    traits->modules = modules;
    xhup_verify_lua_module(api, expected);
  }
  api->setup(traits);
  api->initialize(traits);
  xhup_verify_lua_module(api, expected);
}
#endif
