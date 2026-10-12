/* Instrumentation only: never linked into distributed Rime packages. */
#include "profile_runtime.h"
#include <stdatomic.h>
#include <stdio.h>

static _Atomic uint64_t allocation_calls, requested_bytes, growth_bytes;
static _Atomic uint64_t free_calls, live_bytes, peak_bytes;
static _Atomic uint64_t incremental_collections, generational_collections;
static _Atomic uint64_t dict_lookups, native_queries;

void xhup_profile_alloc(size_t old_size, size_t new_size) {
  if (new_size) {
    atomic_fetch_add_explicit(&allocation_calls, 1, memory_order_relaxed);
    atomic_fetch_add_explicit(&requested_bytes, new_size, memory_order_relaxed);
    if (new_size > old_size)
      atomic_fetch_add_explicit(&growth_bytes, new_size - old_size, memory_order_relaxed);
  } else if (old_size) {
    atomic_fetch_add_explicit(&free_calls, 1, memory_order_relaxed);
  }
  /* Unsigned subtraction handles shrinking reallocations without signed overflow. */
  uint64_t current = atomic_fetch_add_explicit(
      &live_bytes, (uint64_t)new_size - old_size, memory_order_relaxed)
      + new_size - old_size;
  uint64_t peak = atomic_load_explicit(&peak_bytes, memory_order_relaxed);
  while (current > peak && !atomic_compare_exchange_weak_explicit(
      &peak_bytes, &peak, current, memory_order_relaxed, memory_order_relaxed)) {}
}

void xhup_profile_gc(int generational) {
  atomic_fetch_add_explicit(generational ? &generational_collections
                                        : &incremental_collections,
                            1, memory_order_relaxed);
}
void xhup_profile_lookup(void) {
  atomic_fetch_add_explicit(&dict_lookups, 1, memory_order_relaxed);
}
void xhup_profile_native_query(void) {
  atomic_fetch_add_explicit(&native_queries, 1, memory_order_relaxed);
}
void xhup_profile_snapshot(struct XhupRuntimeProfile *result) {
#define READ(field) result->field = atomic_load_explicit(&field, memory_order_relaxed)
  READ(allocation_calls);
  READ(requested_bytes);
  READ(growth_bytes);
  READ(free_calls);
  READ(live_bytes);
  READ(peak_bytes);
  READ(incremental_collections);
  READ(generational_collections);
  READ(dict_lookups);
  READ(native_queries);
#undef READ
}

__attribute__((destructor)) static void report_profile(void) {
  struct XhupRuntimeProfile p;
  xhup_profile_snapshot(&p);
  fprintf(stderr, "XHUP_PROFILE {\"allocation_calls\":%llu,\"requested_bytes\":%llu,"
          "\"growth_bytes\":%llu,\"free_calls\":%llu,\"live_bytes\":%llu,"
          "\"peak_bytes\":%llu,\"incremental_collections\":%llu,"
          "\"generational_collections\":%llu,\"dict_lookups\":%llu,\"native_queries\":%llu}\n",
          (unsigned long long)p.allocation_calls, (unsigned long long)p.requested_bytes,
          (unsigned long long)p.growth_bytes, (unsigned long long)p.free_calls,
          (unsigned long long)p.live_bytes, (unsigned long long)p.peak_bytes,
          (unsigned long long)p.incremental_collections,
          (unsigned long long)p.generational_collections,
          (unsigned long long)p.dict_lookups, (unsigned long long)p.native_queries);
}
