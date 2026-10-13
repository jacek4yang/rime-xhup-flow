#include "candidate_match.h"
#include <rime/context.h>
#include <rime/service.h>
#include <rime/candidate.h>
#include <string_view>

int xhup_candidate_match(RimeSessionId id, const char *text, size_t limit,
                         XhupCandidateMatch *result) {
  *result = {-1, -1, 0, 0, 0, 0};
  const auto session = rime::Service::instance().GetSession(id);
  if (!session) return 0;
  const auto *context = session->context();
  const auto &composition = context->composition();
  if (composition.empty()) return 1;
  const std::string_view expected(text);
  const auto length = context->input().size();
  for (size_t index = 0; index < limit; ++index) {
    const auto candidate = composition.back().GetCandidateAt(index);
    if (!candidate) break;
    if (candidate->start() > candidate->end() || candidate->end() > length) return 0;
    if (candidate->text() != expected) continue;
    if (result->text_rank < 0) {
      result->text_rank = static_cast<int>(index);
      result->start = candidate->start();
      result->end = candidate->end();
    }
    if (candidate->start() == 0 && candidate->end() == length) {
      result->complete_rank = static_cast<int>(index);
      result->complete_start = candidate->start();
      result->complete_end = candidate->end();
      break;
    }
  }
  // All SDK-owned references die before the caller finalizes librime/Lua.
  return 1;
}
