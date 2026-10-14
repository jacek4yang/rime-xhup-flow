-- Before native uniquifier: a shorter same-text phrase must not hide a full path.
-- Only swap identical-text/start candidates within one global bounded head.
-- Preserve every original object, count, other-text position and complete tail.
local M = { HEAD = 32 }
function M.func(translation, env)
  local head, first, best = {}, {}, {}
  local emitted, scanned = false, 0
  local context = env and env.engine and env.engine.context
  local raw = context and context.input
  local raw_end = type(raw) == "string" and #raw or nil
  local function key(candidate) return tostring(candidate.start) .. ":" .. candidate.text end
  local function emit()
    for name, initial in pairs(first) do
      if initial ~= 0 then
        local selected = best[name]
        head[initial], head[selected] = head[selected], head[initial]
      end
    end
    for _, candidate in ipairs(head) do yield(candidate) end
    head, first, best, emitted = nil, nil, nil, true
  end
  for candidate in translation:iter() do
    if emitted then yield(candidate)
    else
      scanned = scanned + 1
      if #head == 0 and raw_end and candidate._end == raw_end then
        -- Valid raw spans cannot exceed the input. No later duplicate can
        -- improve this entry; do not pull an unseen tail just to display it.
        first[key(candidate)] = 0
        yield(candidate)
      else
        local index = #head + 1
        head[index] = candidate
        local name = key(candidate)
        if not first[name] then first[name], best[name] = index, index
        elseif first[name] ~= 0 and candidate._end > head[best[name]]._end then best[name] = index end
      end
      if scanned == M.HEAD then emit() end
    end
  end
  if not emitted then emit() end
end
return M
