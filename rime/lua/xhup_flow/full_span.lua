-- Before native uniquifier: a shorter same-text phrase must not hide a full path.
-- Only swap identical-text/start candidates within one global bounded head.
-- Preserve every original object, count, other-text position and complete tail.
local M = { HEAD = 32 }
function M.func(translation, _env)
  local head, first, best = {}, {}, {}
  local emitted = false
  local function key(candidate) return tostring(candidate.start) .. ":" .. candidate.text end
  local function emit()
    for name, initial in pairs(first) do
      local selected = best[name]
      head[initial], head[selected] = head[selected], head[initial]
    end
    for _, candidate in ipairs(head) do yield(candidate) end
    head, first, best, emitted = nil, nil, nil, true
  end
  for candidate in translation:iter() do
    if emitted then yield(candidate)
    else
      local index = #head + 1
      head[index] = candidate
      local name = key(candidate)
      if not first[name] then first[name], best[name] = index, index
      elseif candidate._end > head[best[name]]._end then best[name] = index end
      if index == M.HEAD then emit() end
    end
  end
  if not emitted then emit() end
end
return M
