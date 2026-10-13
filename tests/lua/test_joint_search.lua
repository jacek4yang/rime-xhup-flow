dofile("tests/lua/runtime_path.lua")
local search = require("xhup_flow.joint_decoder")
local checks = 0
local function check(ok, message) assert(ok, message); checks = checks + 1 end
local function edge(keys, text, score)
  return { keys = keys, text = text, lexical = score, source = "fixture" }
end
local lexicon = {
  edge("a", "甲", -4), edge("ab", "乙", -2), edge("bc", "丙", -2),
  edge("c", "丁", -4), edge("d", "戊", -2),
}
local function provider(raw, entries)
  return function(start)
    local result = {}
    for _, item in ipairs(entries or lexicon) do
      if raw:sub(start + 1, start + #item.keys) == item.keys then result[#result + 1] = item end
    end
    return result
  end
end
local function describe(raw, result, finish)
  local paths = {}
  for _, node in ipairs(result.paths) do
    if node.finish == finish then paths[#paths + 1] = search.describe(raw, node, 5) end
  end
  return paths
end
local result = search.search("abcd", provider("abcd"), function(previous, next_edge)
  return previous and previous.text == "丙" and next_edge.text == "戊" and 8 or 0
end)
local full = describe("abcd", result, 4)
check(#full == 2, "overlapping 1+2 and 2+1 boundaries both survive")
check(full[1].text == "甲丙戊" and full[2].text == "乙丁戊", "later context may rescue a competing boundary")
check(full[1].lexical == -8 and full[1].context == 8 and full[1].score == 0, "score components remain auditable")
check(full[1].preedit == "a bc d" and full[1].start == 5 and full[1].finish == 9, "display separators never consume raw keys")
check(full[1].spans[2].start == 6 and full[1].spans[2].finish == 8, "each key span uses original offsets")
check(table.concat(full[1].words, "/") == "甲/丙/戊" and full[1].remaining == "", "word boundaries and complete consumption")
for tail = 1, search.max_pending do
  local raw = "abcd" .. string.rep("z", tail)
  local pending = describe(raw, search.search(raw, provider(raw)), 4)
  check(#pending == 2 and pending[1].remaining == string.rep("z", tail), "incomplete suffix preserves complete frontier")
end
local ambiguous = { edge("a", "甲", -1), edge("b", "乙", -1), edge("bc", "丙", -1) }
local pending = describe("ab", search.search("ab", provider("ab", ambiguous)), 1)
check(#pending == 1 and pending[1].text == "甲" and pending[1].remaining == "b", "a complete alternative does not erase a pending boundary")
local many = {}
for i = 1, search.beam_width + 5 do many[i] = edge("a", tostring(i), -i) end
local bounded = describe("a", search.search("a", provider("a", many)), 1)
check(#bounded == search.beam_width and bounded[1].text == "1" and bounded[#bounded].text == "8", "beam retains the highest scoring lexical alternatives")
local ties = { edge("ab", "词", -2), edge("a", "字", -1), edge("b", "符", -1) }
local tied = describe("ab", search.search("ab", provider("ab", ties)), 2)
check(tied[1].text == "词" and tied[2].text == "字符", "equal scores prefer fewer edges deterministically")
local bad = { edge("a", "无效", 0/0), edge("a", "无穷", math.huge), edge("b", "错码", 100), edge("", "空", 100), edge("a", "有效", -1) }
local valid = describe("a", search.search("a", function() return bad end), 1)
check(#valid == 1 and valid[1].text == "有效", "invalid weights and nonmatching or empty edges cannot enter a path")
local long = string.rep("a", search.max_input)
local maximum = describe(long, search.search(long, provider(long, {edge("a", "甲", -1)})), #long)
check(#maximum == 1 and maximum[1].score == -search.max_input and maximum[1].finish == 5 + #long, "maximum length has bounded complete consumption")
for _, raw in ipairs({"", long .. "a", "a b", "a1"}) do
  local unsupported = search.search(raw, function() error("unsupported input must not query storage") end)
  check(unsupported.reason == "unsupported_input" and #unsupported.paths == 0, "unsupported input is explicit")
end
local function native_entry(text, id, weight)
  return { text = text, weight = weight or -1, code = { print = function() return tostring(id) end } }
end
local function dictionary(rows)
  local current
  return {
    dict_lookup = function(_, code, predictive)
      assert(not predictive, "lexical edges must be exact")
      current = rows[code] or {}
    end,
    iter_dict = function()
      local i = 0
      return function() i = i + 1; return current[i] end
    end,
  }
end
local native = dictionary({
  jq = {native_entry("就", 3)}, qu = {native_entry("去", 5)},
  jqui = {native_entry("就是", 7)}, nihc = {native_entry("你好", 11)},
  niha = {native_entry("你好", 99, 10)},
  ni = {native_entry("你", 13)}, hc = {native_entry("好", 17)},
})
local aliases = dictionary({
  j = {native_entry("就", 1)}, jqu = {native_entry("就是", 2)},
  nihc = {native_entry("你好", 4)}, ab = {native_entry("你你", 6)},
})
local reverse_map = {["就"]="jq", ["就是"]="jqui", ["你好"]="niha", ["你"]="ni", ["好"]="hc"}
local reverse = { lookup = function(_, text) return reverse_map[text] or "" end }
local lengths = {jq="\2\4", qu="\2", ni="\2\4", hc="\2"}
local raw = "jqunihc"
local lookup = search.lexicon(raw, native, aliases, reverse, lengths, function() return -1 end)
local mixed = describe(raw, search.search(raw, lookup), #raw)
check(mixed[1].text == "就是你好" and mixed[1].preedit == "jqu nihc", "legal alias and canonical word compose")
check(table.concat(mixed[1].codes, ",") == "7,11", "aliases resolve native IDs without replacing an exact canonical route")
check(mixed[1].sources[1] == "static_alias" and mixed[1].sources[2] == "canonical", "source provenance distinguishes alias resolution")
raw = "abqu"
lookup = search.lexicon(raw, native, aliases, reverse, lengths, function() return -1 end)
local spelled = describe(raw, search.search(raw, lookup), #raw)
check(spelled[1].text == "你你去" and table.concat(spelled[1].codes, ",") == "13,13,5",
  "a legitimate alias missing a whole-word native row uses canonical character IDs")
local visits = 0
local malformed = {
  dict_lookup = function() end,
  iter_dict = function() return function()
    visits = visits + 1
    assert(visits <= 64, "rejected native entries must still consume the lookup budget")
    return native_entry("坏", "1,2")
  end end,
}
lookup = search.lexicon("zz", malformed, dictionary({}), reverse, {zz="\2"}, function() return -1 end)
check(#search.search("zz", lookup).paths == 0 and visits == 64, "invalid canonical code vectors cannot cause an unbounded scan")

local stale_model = {identity="old", policy="joint-frequency-context-v2",
  total_reward=0, characters={}, transitions={}}
check(not pcall(search.scorer, stale_model, "new"), "a cached model from another deployment cannot score candidates")
check(not pcall(search.scorer, stale_model), "missing schema/model binding fails closed")

print(string.format("PASS joint key/text search: %d assertions (pure Lua, not librime)", checks))
