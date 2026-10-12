/* Test-only ABI shared by the pinned Lua VM, librime-lua and replay probe. */
#ifndef XHUP_PROFILE_RUNTIME_H
#define XHUP_PROFILE_RUNTIME_H
#include <stddef.h>
#include <stdint.h>
#pragma GCC visibility push(default)
#ifdef __cplusplus
extern "C" {
#endif
struct XhupRuntimeProfile {
  uint64_t allocation_calls;
  uint64_t requested_bytes;
  uint64_t growth_bytes;
  uint64_t free_calls;
  uint64_t live_bytes;
  uint64_t peak_bytes;
  uint64_t incremental_collections;
  uint64_t generational_collections;
  uint64_t dict_lookups;
  uint64_t native_queries;
};
void xhup_profile_alloc(size_t old_size, size_t new_size);
void xhup_profile_gc(int generational);
void xhup_profile_lookup(void);
void xhup_profile_native_query(void);
void xhup_profile_snapshot(struct XhupRuntimeProfile *result);
#ifdef __cplusplus
}
#endif
#pragma GCC visibility pop
#endif
