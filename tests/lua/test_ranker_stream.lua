-- Stub stream boundary tests, not native librime acceptance.
package.path = "rime/lua/?.lua;" .. package.path
local ranker = require("xhup_flow.context_ranker")
local memory = require("xhup_flow.user_memory")
local assertions = 0
local function check(value, why)
  assert(value, why)
  assertions = assertions + 1
end
local function replay(length, bound, enabled, inject)
  local input = {}
  for i = 1, length do
    input[i] = { type = "table", text = i % 3 == 0 and "repeat" or "other", id = i }
  end
  -- Same object multiple times, as well as distinct objects with equal text.
  if length >= 8 then input[8] = input[4] end
  local consumed, output = 0, {}
  local translation = { iter = function()
    return function()
      consumed = consumed + 1
      return input[consumed]
    end
  end }
  local env = { bound = bound, hints = {}, engine = { context = {
    input = "abc", get_option = function() return enabled end
  } } }
  memory.state.last_commit = "repeat"
  memory.state.counts = {}
  local old_yield, old_reorder = _G.yield, ranker.bounded_reorder
  local decisions = 0
  ranker.bounded_reorder = function(...)
    decisions = decisions + 1
    if inject then error("injected") end
    return old_reorder(...)
  end
  _G.yield = function(candidate)
    if #output == 0 and enabled and length >= bound then
      check(consumed == bound, "head must emit without reading an extra candidate")
    end
    output[#output + 1] = candidate
  end
  ranker.func(translation, env)
  _G.yield, ranker.bounded_reorder = old_yield, old_reorder
  check(#output == #input, "multiplicity preserved")
  check(decisions == (enabled and 1 or 0), "one global decision, never repeated windows")
  local multiset = {}
  for _, c in ipairs(input) do multiset[c] = (multiset[c] or 0) + 1 end
  for i, c in ipairs(output) do
    multiset[c] = (multiset[c] or 0) - 1
    if not enabled or inject or i > bound then
      check(c == input[i], "tail/fallback object identity and position unchanged")
    end
  end
  for _, count in pairs(multiset) do check(count == 0, "candidate object count preserved") end
  if inject then check(env.rank_error ~= nil, "decision failure explicit") end
  if enabled and not inject and length >= 3 and bound >= 3 then
    check(output[1] == input[3], "head evidence still promoted")
  end
end
for bound = 2, 5 do
  for _, length in ipairs({0, 1, 2, 3, 4, 5, 8, 31, 10000}) do
    replay(length, bound, true, false)
    replay(length, bound, false, false)
    replay(length, bound, true, true)
  end
end
local cands = {
  { type = "table", text = "a" }, { type = "table", text = "b" },
  { type = "table", text = "b" }, { type = "table", text = "a" },
}
local order = ranker.bounded_reorder(cands, "b", 2, {}, "", {})
check(table.concat(order, ",") == "2,1,3,4", "pure helper enforces global bound too")
print(string.format("PASS global head stream: %d assertions", assertions))
