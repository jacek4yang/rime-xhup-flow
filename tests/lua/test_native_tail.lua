package.path = "rime/lua/?.lua;" .. package.path
local module = require("xhup_flow.native_tail")
local checks = 0
local function check(value, message) assert(value, message); checks = checks + 1 end
local queries, emitted, visited = {}, {}, 0
Segment = function(start, finish) return { start = start, _end = finish } end
ShadowCandidate = function(candidate, kind, text, comment)
  return { start = candidate.start, _end = candidate._end, text = text, comment = comment,
    type = kind, preedit = candidate.preedit, quality = candidate.quality, genuine = candidate }
end
yield = function(candidate) emitted[#emitted+1] = candidate end
local provider = { query = function(_, input, segment)
  queries[#queries+1] = { input = input, segment = segment }
  return { iter = function()
    local index = 0
    return function()
      index = index + 1
      if index > 100 then return end
      visited = visited + 1
      return { start = segment.start, _end = segment._end, text = "公共测试",
        comment = "", preedit = input, quality = 2 }
    end
  end }
end }
Component = { Translator = function(_, _, name)
  check(name == "table_translator@flow", "one native search authority")
  return provider
end }
local env = { engine = {} }; module.init(env)
module.func("nihcnz", { start = 10, _end = 16, tags = {} }, env)
check(#queries == 6 and visited == 12 and #emitted == 12, "constant query/candidate budgets")
for i = 1, 6 do
  check(emitted[i]._end < 16, "pending suffix is not consumed")
  check(emitted[i].quality == 1, "explicit pending utility penalty")
end
for i = 7, 12 do
  check(emitted[i]._end == 16, "virtual delimiter mapped back to raw span")
  check(emitted[i].quality == 2, "complete native score unchanged")
  check(emitted[i].genuine._end == 16, "native genuine identity preserved")
end
for i = 4, 6 do check(queries[i].input:find(" ", 1, true), "native SPACE delimiter") end
for _, input in ipairs({ "", "ni", "ni'hc", "ni2", string.rep("n", 129) }) do
  local before = #queries
  module.func(input, { start = 0, _end = #input }, env)
  check(#queries == before, "unsupported input uses untouched native fallback")
end
local before = #queries
module.func("nihcnz", { start = 0, _end = 5 }, env)
check(#queries == before, "mismatched segment never remapped")
env.native = { query = function() error("injected query failure") end }
module.func("nihcnz", { start = 0, _end = 6 }, env)
check(env.native_error == "native tail query failed", "query errors fail closed")
Component = nil; module.init(env)
check(not env.native and env.native_error, "missing provider explicit")
print(string.format("PASS native tail policy stubs: %d assertions (not real librime)", checks))
