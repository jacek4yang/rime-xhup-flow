-- Bounded policy over librime's native sentence search; not a Lua decoder.
-- No corpus scan, file I/O, path scoring, or persisted text here.
local M = { MAX_INPUT = 128, CANDIDATES_PER_QUERY = 2 }

function M.init(env)
  env.native = nil
  env.native_error = nil
  if not Component or not Component.Translator then
    env.native_error = "native translator API unavailable"
    return
  end
  local ok, provider = pcall(Component.Translator, env.engine, "", "table_translator@flow")
  if ok and provider then env.native = provider
  else env.native_error = "native translator construction failed" end
end

local function emit_query(env, input, segment, expected_end, make_candidate)
  local translation = env.native:query(input, segment)
  if not translation then return end
  local visited = 0
  for candidate in translation:iter() do
    visited = visited + 1
    -- Native sentence translations also yield partial phrases. Those are NOT
    -- proof of a complete interpretation of the queried span.
    if candidate.start == segment.start and candidate._end == expected_end then
      yield(make_candidate(candidate))
    end
    if visited == M.CANDIDATES_PER_QUERY then break end
  end
end

function M.func(input, segment, env)
  if not env.native or #input < 3 or #input > M.MAX_INPUT
      or input:find("[^a-z]") or segment._end - segment.start ~= #input then return end
  local ok = pcall(function()
    -- Three possible unfinished suffix lengths for a 2/3/4-key code.
    -- Each query delegates the complete prefix search/ranking to native Rime.
    for tail = 1, 3 do
      local n = #input - tail
      if n >= 2 then
        local prefix = Segment(segment.start, segment.start + n)
        prefix.tags = segment.tags
        emit_query(env, input:sub(1, n), prefix, prefix._end, function(candidate)
          local pending = ShadowCandidate(candidate, "pending_prefix", candidate.text,
            "待续:" .. input:sub(n + 1))
          -- Leave actual unconsumed keys to Rime's composition renderer.
          pending.preedit = candidate.preedit
          -- Explicit utility preference: completed paths precede otherwise-equal
          -- pending paths; same native log-quality units, not lexical evidence.
          pending.quality = candidate.quality - 1
          return pending
        end)
      end
    end
    if #input <= 4 then return end -- do not alter standalone compatibility codes
    -- Ask native Rime for legal alternative last-code boundaries. SPACE is the
    -- native table translator delimiter; apostrophe is not equivalent here.
    for tail = 2, 4 do
      local n = #input - tail
      if n >= 2 then
        local query = input:sub(1, n) .. " " .. input:sub(n + 1)
        local virtual = Segment(segment.start, segment._end + 1)
        virtual.tags = segment.tags
        emit_query(env, query, virtual, virtual._end, function(candidate)
          -- Preserve genuine native Phrase/Sentence identity for native learning.
          -- Only the artificial delimiter's outer span is mapped back to raw input.
          candidate._end = segment._end
          local full = ShadowCandidate(candidate, "native_tail_boundary",
            candidate.text, candidate.comment)
          full.preedit = query
          full.quality = candidate.quality
          return full
        end)
      end
    end
  end)
  if not ok then env.native_error = "native tail query failed" end
end

return M
