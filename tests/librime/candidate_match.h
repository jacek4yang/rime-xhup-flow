/* Native QA only: text equality does not prove complete raw-input consumption. */
#ifndef XHUP_CANDIDATE_MATCH_H
#define XHUP_CANDIDATE_MATCH_H
#include <rime_api.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct {
  int text_rank, complete_rank;
  size_t start, end, complete_start, complete_end;
} XhupCandidateMatch;
int xhup_candidate_match(RimeSessionId session, const char *text, size_t limit,
                         XhupCandidateMatch *result);
#ifdef __cplusplus
}
#endif
#endif
