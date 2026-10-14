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
-- Late arrivals replace future beams; retained ancestors must never be recycled.
local replacements = { edge("b", "尾", 0), edge("c", "终", 0) }
for i = 1, search.beam_width + 5 do
  replacements[#replacements + 1] = edge("a", "字" .. i, i)
  replacements[#replacements + 1] = edge("ab", "旧" .. i, i - 100)
end
local replaced = search.search("abc", provider("abc", replacements))
-- An older translation may still hold its paths while another query runs.
local next_input = string.rep("b", search.max_input)
search.search(next_input, provider(next_input, {edge("b", "新", -1)}))
for finish = 1, 3 do
  local retained = describe("abc", replaced, finish)
  for rank = 1, search.beam_width do
    local value = search.beam_width + 6 - rank
    local suffix = finish == 3 and "尾终" or (finish == 2 and "尾" or "")
    check(retained[rank].text == "字" .. value .. suffix
      and retained[rank].score == value and retained[rank].node.count == finish,
      "beam replacement preserves ancestor text, score and edge count")
  end
end
do
  local entries = {edge("b", "尾", 0), edge("c", "甲终", 0), edge("c", "乙终", 0)}
  for i = 1, search.beam_width do entries[#entries + 1] = edge("a", "字" .. i, i) end
  local paths = describe("abc", search.search("abc", provider("abc", entries), function(previous, current)
    if previous and previous.text == "字8" then return math.huge end
    if previous and previous.text == "尾" then return current.text == "乙终" and 100 or -100 end
    return 0
  end), 3)
  check(paths[1].text == "字7尾乙终" and paths[1].score == 107,
    "shared predecessor scores remain specific to the current edge")
  for _, path in ipairs(paths) do
    check(not path.text:find("字8", 1, true), "nonfinite transitions never enter retained paths")
  end
end
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
DictEntry = function()
  return {code={push=function(self, id) self[#self + 1] = id end}}
end
local function native_entry(text, id, weight)
  return { text = text, weight = weight or -1, code = { id, print = function() return tostring(id) end } }
end
local function dictionary(rows)
  local decoded = {}
  for spelling, items in pairs(rows) do
    for _, item in ipairs(items) do
      local id = tonumber(item.code:print())
      assert(item.code:print() == items[1].code:print(), "a native spelling has one ID")
      if id then
        assert(not decoded[id] or decoded[id] == spelling, "native IDs are unique per spelling")
        decoded[id] = spelling
      end
    end
  end
  local current
  return {
    dict = {decode = function(_, code)
      local result = {}
      for _, id in ipairs(code) do
        if not decoded[id] then return {} end
        result[#result + 1] = decoded[id]
      end
      return result
    end},
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
local reverse_map = {["就"]="jq!3!-1", ["就是"]="jqui!7!-1",
  ["你好"]="niha!99!10", ["你"]="ni!13!-1", ["好"]="hc!17!-1"}
local reverse = { lookup = function(_, text) return reverse_map[text] or "" end }
local lengths = {jq="\2\4", qu="\2", ni="\2\4", hc="\2"}
local raw = "jqunihc"
local lookup = search.lexicon(raw, native, aliases, reverse, lengths, function() return -1 end)
local mixed = describe(raw, search.search(raw, lookup), #raw)
check(mixed[1].text == "就是你好" and mixed[1].preedit == "jqu nihc", "legal alias and canonical word compose")
check(table.concat(mixed[1].codes, ",") == "7,11", "aliases resolve native IDs without replacing an exact canonical route")
check(mixed[1].sources[1] == "static_alias" and mixed[1].sources[2] == "canonical", "source provenance distinguishes alias resolution")
do
  local selected = mixed[1]
  local records = {
    ["jqui nihc \t就是你好"] = {text="就是你好", custom_code="jqui nihc ",
      remaining_code_length=0, commit_count=2, weight=1},
  }
  local learned = {lookup_words=function(_, prefix)
    return {iter=function()
      local row = records[prefix]
      return function() local result = row; row = nil; return result end
    end}
  end}
  check(search.user_reward(selected, learned) > 0,
    "mixed alias learning reads canonical native syllables, not typed keys or another homophone route")
  records["jqui nihc \t就是你好"].commit_count = 0
  check(search.user_reward(selected, learned) == 0, "uncommitted records cannot personalize a path")
  local missing = {codes={7,11}, text=selected.text, score=selected.score}
  check(not pcall(search.user_reward, missing, learned), "missing canonical metadata fails closed")
end
do
  local memo = search.readonly_cache(native, aliases, reverse)
  local function signature(input, cache)
    local resolve, stats = search.lexicon(input, native, aliases, reverse, lengths,
      function() return -1 end, cache)
    local result = {}
    for _, node in ipairs(search.search(input, resolve).paths) do
      local path = search.describe(input, node)
      result[#result + 1] = table.concat({ path.finish, path.score, path.tie, path.text,
        path.preedit, table.concat(path.codes, ","), path.canonical_key, path.remaining }, "|")
    end
    return table.concat(result, "\n"), stats
  end
  for _, input in ipairs({"jqunihc", "jqunih", "jqunhc", "jqunihc", "abqu", "jqunihc"}) do
    check(signature(input, memo) == signature(input), "append/backspace/edit preserves cold paths and native IDs")
  end
  local _, warm = signature("jqunihc", memo)
  check(warm.lookups == 0, "repeated immutable lookups require no native dictionary calls")
  local replacement = dictionary({ jqui = {native_entry("就是", 77)} })
  check(not pcall(search.lexicon, "jqu", replacement, aliases, reverse, lengths,
    function() return -1 end, memo), "schema dictionary replacement rejects old cached IDs")
  local replacement_reverse = { lookup = function(_, text)
    return text == "就是" and "jqui!77!-1" or ""
  end }
  check(not pcall(search.lexicon, "jqu", native, aliases, replacement_reverse, lengths,
    function() return -1 end, memo), "projection replacement rejects old cached records")
  local resolve = search.lexicon("jqu", replacement, aliases, replacement_reverse, lengths,
    function() return -1 end, search.readonly_cache(replacement, aliases, replacement_reverse))
  local changed = describe("jqu", search.search("jqu", resolve), 3)
  check(changed[1].text == "就是" and changed[1].codes[1] == 77, "new schema uses its own syllable IDs")
end

for _, invalid in ipairs({"jqui!77!-1", "zz!7!-1", "jqui!7!1e999", "jqui"}) do
  local projection = {lookup = function(_, text)
    return text == "就是" and invalid or reverse:lookup(text)
  end}
  local resolve = search.lexicon("jqu", native, aliases, projection, lengths, function() return -1 end)
  check(not pcall(search.search, "jqu", resolve),
    "stale IDs, mismatched spellings and malformed weights cannot produce native candidates")
end

do
  local rows = {}
  for text in ("甲乙丙丁戊己庚辛末"):gmatch("[%z\1-\127\194-\244][\128-\191]*") do
    rows[#rows + 1] = native_entry(text, 1)
  end
  local owner = dictionary({ab = rows})
  local alias_rows = dictionary({z = {native_entry("末", 0)}})
  local projection = {lookup = function(_, text)
    return text == "末" and "ab!1!-1" or ""
  end}
  local memo = search.readonly_cache(owner, alias_rows, projection)
  for _ = 1, 2 do
    local resolve = search.lexicon("abz", owner, alias_rows, projection, {ab = "\2"},
      function() return -1 end, memo)
    local paths = describe("abz", search.search("abz", resolve), 3)
    check(paths[1].text == "甲末" and paths[1].remaining == "",
      "alias beyond the canonical edge shortlist remains reachable on cold and warm caches")
    check(table.concat(paths[1].codes, ",") == "1,1",
      "shortlist truncation does not change the alias's original native code identity")
  end
end

do
  local empty, alias_rows = dictionary({}), dictionary({})
  local memo = search.readonly_cache(empty, alias_rows, reverse)
  local bounds = {}
  for i = 0, 675 do
    local code = string.char(97 + math.floor(i / 26), 97 + i % 26)
    bounds[code] = "\2"
    search.lexicon(code, empty, alias_rows, reverse, bounds, function() return -1 end, memo)(0)
  end
  check(memo.count <= 512 and memo.rows == 0 and memo.bytes <= 262144,
    "negative lookups cannot create an unbounded session cache")
  local resolve, evicted = search.lexicon("aa", empty, alias_rows, reverse, bounds,
    function() return -1 end, memo)
  resolve(0)
  check(evicted.lookups > 0, "old negative entries are evicted rather than retained forever")
  local dense = {}
  for i = 0, 675 do
    local code, rows = string.char(97 + math.floor(i / 26), 97 + i % 26), {}
    for j = 1, 64 do rows[j] = native_entry("字" .. j, i) end
    dense[code] = rows
  end
  local owner = dictionary(dense)
  memo = search.readonly_cache(owner, alias_rows, reverse)
  for i = 0, 49 do
    local code = string.char(97 + math.floor(i / 26), 97 + i % 26)
    search.lexicon(code, owner, alias_rows, reverse, bounds, function() return -1 end, memo)(0)
  end
  check(memo.rows <= 2048 and memo.count <= 512 and memo.bytes <= 262144,
    "dense homophone buckets remain bounded by total rows, not only lookup count")
  owner = dictionary({aa = {native_entry(string.rep("字", 90000), 1)}})
  memo = search.readonly_cache(owner, alias_rows, reverse)
  search.lexicon("aa", owner, alias_rows, reverse, bounds, function() return -1 end, memo)(0)
  local again, oversized = search.lexicon("aa", owner, alias_rows, reverse, bounds,
    function() return -1 end, memo)
  again(0)
  check(memo.bytes <= 262144 and oversized.lookups > 0, "oversized rows are not retained in the session cache")
end
do
  -- The same alias can initially hit the normalization budget, then acquire a
  -- whole-word ID at a later position. Reusing an edge must not freeze fallback IDs.
  local rows = dictionary({
    ab = { native_entry("你", 1), native_entry("好", 1) },
    zz = { native_entry("你好", 3) },
  })
  local alias_rows = dictionary({ a = {
    native_entry("甲", 0), native_entry("乙", 0), native_entry("丙", 0),
    native_entry("丁", 0), native_entry("你好", 0),
  } })
  local spellings = {
    ["甲"] = "xa xb xc xd xe xf xg xh", ["乙"] = "xi xj xk xl xm xn xo xp",
    ["丙"] = "xq xr xs xt xu xv xw xx", ["丁"] = "ya yb yc yd ye yf yg yh",
    ["你"] = "ab!1!-1", ["好"] = "ab!1!-1", ["你好"] = "zz!3!-1",
  }
  for _, text in ipairs({"甲", "乙", "丙", "丁"}) do
    spellings[text] = spellings[text]:gsub("([a-z]+)", "%1!x")
  end
  local projection = {lookup = function(_, text) return spellings[text] or "" end}
  local memo = search.readonly_cache(rows, alias_rows, projection)
  for _ = 1, 2 do
    local resolve = search.lexicon("abab", rows, alias_rows, projection,
      { ab = "\2" }, function() return -1 end, memo)
    local function alias_at(start)
      for _, item in ipairs(resolve(start)) do
        if item.keys == "a" and item.text == "你好" then return item end
      end
      error("valid alias missing")
    end
    local before, after = alias_at(0), alias_at(2)
    check(before.source == "spelled_alias" and table.concat(before.codes, ",") == "1,1"
      and after.source == "static_alias" and table.concat(after.codes, ",") == "3",
      "cold/warm normalization budgets preserve fallback and whole-word learning IDs")
  end
end
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

do
  local rows = dictionary({
    ab = {native_entry("甲", 1)}, abcd = {native_entry("乙", 2)},
    abc = {native_entry("丙丁", 3)}, dd = {native_entry("戊", 4)},
  })
  local static_rows = dictionary({
    abcd = {native_entry("乙", 5)}, d = {native_entry("戊", 6)},
  })
  local projection = {lookup = function(_, text)
    return text == "戊" and "dd!4!-1" or ""
  end}
  local function lexical(text) return text == "乙" and -9000 or -1 end
  local bounds = {ab = "\2\3\4"}
  local provider = search.lexicon("abcdabxx", rows, static_rows, projection,
    bounds, lexical, nil, 9544)
  local function edge_at(position, keys)
    for _, edge in ipairs(provider(position)) do
      if edge.keys == keys then return edge end
    end
    error("missing fixture edge")
  end
  check(edge_at(0, "ab").compatibility == -9544,
    "splitting an actual primary full character carries a compatibility cost")
  check(edge_at(4, "ab").compatibility == 0 and edge_at(0, "ab").compatibility == -9544,
    "repeated lexical edges do not inherit another position's shape evidence")
  provider = search.lexicon("abcd", rows, static_rows, projection, bounds, lexical, nil, 0)
  check(describe("abcd", search.search("abcd", provider), 4)[1].text == "丙丁戊",
    "frequency alone prefers the shorter-code combination in this ambiguity")
  provider = search.lexicon("abcd", rows, static_rows, projection, bounds, lexical, nil, 9544)
  local paths = describe("abcd", search.search("abcd", provider), 4)
  check(paths[1].text == "乙" and paths[1].score == -9000 and paths[1].compatibility == 0,
    "primary full-code evidence can outweigh a frequent shorter-code combination")
  check(paths[2].text == "丙丁戊" and paths[2].compatibility == -9544
    and paths[2].score == paths[2].lexical + paths[2].context + paths[2].compatibility,
    "the alternative remains reachable with independently explainable score components")
  local word_first = dictionary({abcd = {native_entry("丙丁", 7)}})
  provider = search.lexicon("abcd", rows, word_first, projection, bounds, lexical, nil, 9544)
  local short
  for _, edge in ipairs(provider(0)) do if edge.keys == "abc" then short = edge end end
  check(short.compatibility == 0, "a four-key word alias is not evidence of a primary full character")
end

local stale_model = {identity="old", policy="joint-frequency-context-shape-v3",
  primary_full_code_split_penalty=9544, total_reward=0, characters={}, transitions={}}
check(not pcall(search.scorer, stale_model, "new"), "a cached model from another deployment cannot score candidates")
check(not pcall(search.scorer, stale_model), "missing schema/model binding fails closed")

-- Native quality and joint scores are not cross-calibrated. Protect exact words,
-- low-confidence sentences, and legacy policies; partial/predictive heads cannot
-- hide a fully consumed path. Exercise the real search and provider ordering.
local policy = require("xhup_flow.native_tail")
Phrase = function(_, _, start, finish, entry)
  local phrase = {start=start, _end=finish, text=entry.text, type="phrase"}
  phrase.toCandidate = function() return phrase end
  return phrase
end
native.lang_name = "fixture"
for _, case in ipairs({
  {finish=12, remaining=0, preserve=true},
  {finish=11, remaining=0, preserve=false},
  {finish=12, remaining=1, preserve=false},
  {finish=12, remaining=0, preserve=true, ranking="joint-confidence-v1"},
  {finish=12, remaining=0, preserve=true, ranking="unknown"},
  {finish=12, remaining=0, preserve=true, ranking="joint-confidence-v1", kind="table"},
  {finish=12, remaining=0, preserve=true, ranking="joint-confidence-v1", kind="user_table"},
  {finish=12, remaining=0, preserve=true, ranking="joint-confidence-v1",
    kind="sentence", text="就是你好"},
}) do
  local head = {start=5, _end=case.finish, text=case.text or "原生首选", type=case.kind or "sentence",
    quality=-1000, to_phrase=function() return {entry={remaining_code_length=case.remaining}} end}
  local output = {}
  yield = function(candidate) output[#output + 1] = candidate end
  local env = {
    engine={context={get_option=function() return true end, set_property=function() end}},
    native={lang_name="fixture", query=function()
      return {iter=function() local done=false; return function()
        if not done then done=true; return head end
      end end}
    end},
    joint_ready=true, memory=native, aliases=aliases, reverse=reverse,
    ranking_policy=case.ranking,
    edge_lengths=lengths, lexical_score=function() return case.kind and -4096 or -1 end,
  }
  policy.func("jqunihc", {start=5, _end=12}, env)
  if case.preserve then
    check(output[1] == head and output[2].text == "就是你好",
      "protected complete native first choice keeps its genuine identity")
  else
    check(output[1].text == "就是你好" and output[1]._end == 12,
      "partial or predictive native head cannot suppress full joint consumption")
  end
  local occurrences = 0
  for _, candidate in ipairs(output) do if candidate == head then occurrences=occurrences+1 end end
  check(occurrences == 1, "native candidate is neither duplicated nor wrapped")
end

-- A distinct-text confidence margin, not incomparable native quality, decides
-- whether an automatically composed sentence may yield first place.
for _, margin in ipairs({2047, 2048, 2049}) do
  local memory = dictionary({
    aa={native_entry("甲", 1)},
    bb={native_entry("乙", 2), native_entry("丙", 2), native_entry("丁", 2),
      native_entry("戊", 2), native_entry("己", 2), native_entry("庚", 2),
      native_entry("辛", 2), native_entry("壬", 2)},
  })
  memory.lang_name = "fixture"
  local head = {start=0, _end=4, text="原生整句", type="sentence",
    to_phrase=function() return {entry={remaining_code_length=0}} end}
  local output = {}
  yield = function(candidate) output[#output + 1] = candidate end
  policy.func("aabb", {start=0, _end=4}, {
    engine={context={get_option=function() return true end, set_property=function() end}},
    ranking_policy="joint-confidence-v1", joint_ready=true,
    memory=memory, aliases=dictionary({}), reverse=reverse, edge_lengths={aa="\2", bb="\2"},
    lexical_score=function(text) return text ~= "甲" and text ~= "乙" and -1 - margin or -1 end,
    native={lang_name="fixture", query=function()
      return {iter=function() local done=false; return function()
        if not done then done=true; return head end
      end end}
    end},
  })
  if margin >= 2048 then
    check(output[1].text == "甲乙", "confident full path outranks an automatic native sentence")
    local visible_native = false
    for index = 1, math.min(5, #output) do
      if output[index] == head then visible_native = true end
    end
    check(visible_native, "confident promotion keeps genuine native fallback on the first page")
  else
    check(output[1] == head, "ambiguous full paths retain the genuine native first choice")
  end
end

-- A pending user-read failure cannot discard an already valid complete choice.
-- Request the stream incrementally, as a real translation consumer does.
do
  local memory = dictionary({
    aa={native_entry("甲", 1)}, bb={native_entry("乙", 2), native_entry("丁", 2)},
  })
  memory.lang_name = "fixture"
  local head = {start=0, _end=4, text="甲乙", type="sentence",
    to_phrase=function() return {entry={remaining_code_length=0}} end}
  local original_reward, failed_pending, status = search.user_reward, false, nil
  search.user_reward = function(path)
    if path.finish < 4 then failed_pending = true; error("pending dictionary read failed") end
    return path.text == "甲丁" and 10000 or 0
  end
  yield = coroutine.yield
  local stream = coroutine.create(function()
    policy.func("aabb", {start=0, _end=4}, {
      engine={context={get_option=function() return true end,
        set_property=function(_, _, value) status=value end}},
      ranking_policy="joint-confidence-v1", joint_ready=true,
      memory=memory, aliases=dictionary({}), reverse=reverse, edge_lengths={aa="\2", bb="\2"},
      lexical_score=function() return -2 end,
      native={lang_name="fixture", user_dict={}, query=function()
        return {iter=function() local done=false; return function()
          if not done then done=true; return head end
        end end}
      end},
    })
  end)
  local ok, first = coroutine.resume(stream)
  check(ok and first.text == "甲丁" and first._end == 4 and not failed_pending,
    "first page uses successful complete-path personalization before pending reads")
  local pending
  while coroutine.status(stream) ~= "dead" do
    local resumed, candidate = coroutine.resume(stream)
    assert(resumed, candidate)
    if candidate and candidate._end == 2 then pending = candidate end
  end
  check(failed_pending and status == "personalization_unavailable",
    "later user-read failure is reported, not hidden")
  check(pending and pending.text == "甲" and pending.comment == "待续:bb",
    "failed pending personalization retains a valid prefix and unconsumed keys")
  search.user_reward = original_reward
end

print(string.format("PASS joint key/text search: %d assertions (pure Lua, not librime)", checks))
