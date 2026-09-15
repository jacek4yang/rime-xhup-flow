-- tests/lua/sim_native_lookup.lua
--
-- Joint lattice 融合语义夹具一致性测试(Lua 侧,不依赖 librime)。
--
-- 消费 data/fixtures/joint-lattice-v1.json(与 Rust
-- crates/xhup-decoder/tests/evidence.rs 相同夹具),验证 Lua
-- native_lookup.fuse_evidence 与 Rust LatticeFusionBuilder 在
-- 融合语义 / 有界性 / 确定性上一致。
--
-- 运行: lua5.4 tests/lua/sim_native_lookup.lua (工作目录 = 仓库根)
--
-- 夹具 JSON 为扁平结构,使用内置最小 JSON 解码器(仅覆盖夹具子集:
-- object/array/string/integer/boolean),避免引入外部依赖。

local json = {}
do
  local function decode_value(s, i)
    local c = s:sub(i, i)
    if c == "{" then
      local obj = {}
      i = i + 1
      while true do
        while s:sub(i, i):match("%s") do i = i + 1 end
        if s:sub(i, i) == "}" then return obj, i + 1 end
        local key
        key, i = decode_value(s, i)
        while s:sub(i, i):match("%s") do i = i + 1 end
        assert(s:sub(i, i) == ":", "JSON 期望 ':'")
        local val
        val, i = decode_value(s, i + 1)
        obj[key] = val
        while s:sub(i, i):match("%s") do i = i + 1 end
        if s:sub(i, i) == "," then i = i + 1 end
      end
    elseif c == "[" then
      local arr = {}
      i = i + 1
      while true do
        while s:sub(i, i):match("%s") do i = i + 1 end
        if s:sub(i, i) == "]" then return arr, i + 1 end
        local val
        val, i = decode_value(s, i)
        table.insert(arr, val)
        while s:sub(i, i):match("%s") do i = i + 1 end
        if s:sub(i, i) == "," then i = i + 1 end
      end
    elseif c == '"' then
      local j = i + 1
      local buf = {}
      while true do
        local ch = s:sub(j, j)
        if ch == '"' then break end
        if ch == "\\" then
          j = j + 1
          local esc = s:sub(j, j)
          ch = ({ ["n"] = "\n", ["t"] = "\t", ['"'] = '"', ["\\"] = "\\" })[esc] or esc
        end
        table.insert(buf, ch)
        j = j + 1
      end
      return table.concat(buf), j + 1
    else
      while s:sub(i, i):match("%s") do i = i + 1 end
      local c2 = s:sub(i, i)
      if c2 == '"' or c2 == "[" or c2 == "{" then
        return decode_value(s, i)
      end
      local j = i
      while j <= #s and s:sub(j, j):match("[^,}%]%s]") do j = j + 1 end
      local tok = s:sub(i, j - 1)
      local num = tonumber(tok)
      if num ~= nil then return num, j end
      if tok == "true" then return true, j end
      if tok == "false" then return false, j end
      if tok == "null" then return nil, j end
      error("JSON 无法解析 token: " .. tok)
    end
  end

  function json.decode(s)
    local v, i = decode_value(s, 1)
    return v, i
  end
end

package.path = "rime/lua/?.lua;" .. package.path
local native_lookup = require("xhup_flow.native_lookup")

local passed, failed = 0, 0
local function check(name, ok)
  if ok then
    passed = passed + 1
  else
    failed = failed + 1
    io.stderr:write(string.format("FAIL %s\n", name))
  end
end

local fh = assert(io.open("data/fixtures/joint-lattice-v1.json", "r"))
local fixture = json.decode(fh:read("*a"))
fh:close()

check("fixture schema 版本", fixture.schema == "xhup-joint-lattice-fixture/v1")

-- source 字符串 → native_lookup 融合排序的枚举顺序(与 Rust Ord 对齐)
local function raw_from_case(case)
  local raw = {}
  local bounds_override = case.boundsOverride
  for _, c in ipairs(case.candidates) do
    table.insert(raw, {
      start = c.start,
      end_ = c["end"],
      text = c.text,
      source = c.source,
      frequency = c.frequency,
      sourceRank = c.sourceRank,
      quality = 0,
    })
  end
  return raw, bounds_override
end

local function find_fused(result, start, end_, text)
  for _, f in ipairs(result.fused) do
    if f.start == start and f.end_ == end_ and f.text == text then
      return f
    end
  end
  return nil
end

for _, case in ipairs(fixture.cases) do
  local raw, override = raw_from_case(case)
  local bounds = {}
  for k, v in pairs(native_lookup.DEFAULT_BOUNDS) do bounds[k] = v end
  if override then
    for k, v in pairs(override) do bounds[k] = v end
  end

  local result = native_lookup.fuse_evidence(raw, bounds)
  local stats = result.stats

  -- 期望融合边数与 Rust 测试一致
  if case.expectedFusedEdges ~= nil then
    check(string.format("%s: fused_candidates=%d", case.id, stats.fused_candidates),
      stats.fused_candidates == case.expectedFusedEdges)
    check(string.format("%s: total_edges=%d", case.id, stats.total_edges),
      stats.total_edges == case.expectedFusedEdges)
  end

  -- 截断期望
  if case.expectedTruncatedCandidates ~= nil then
    check(string.format("%s: truncated>=%d", case.id, case.expectedTruncatedCandidates),
      stats.truncated_candidates >= case.expectedTruncatedCandidates)
  end

  -- 夹具注释中声明的融合:同 (span, text) 多来源 → 单一融合候选含全部来源
  local expected_sources = {
    ["nested-prefix-path"] = { span = { 0, 4 }, text = "时间", sources = { "static_table", "flow_table" } },
    ["overlapping-lexical-segmentation"] = { span = { 0, 4 }, text = "研究", sources = { "static_table", "flow_table" } },
    ["duplicate-source-fusion"] = { span = { 0, 4 }, text = "我们", sources = { "static_table", "flow_table", "user_learned" } },
  }
  local exp = expected_sources[case.id]
  if exp then
    local fused = find_fused(result, exp.span[1], exp.span[2], exp.text)
    check(case.id .. ": 融合候选存在", fused ~= nil)
    if fused then
      local got = table.concat(fused.sources, ",")
      check(case.id .. ": 融合来源 (" .. got .. ")", got == table.concat(exp.sources, ","))
    end
  end
end

-- 确定性: 同一夹具两次融合输出一致;插入顺序反转输出仍一致
do
  local case = fixture.cases[2] -- overlapping-lexical-segmentation
  local raw = raw_from_case(case)
  local r1 = native_lookup.fuse_evidence(raw, native_lookup.DEFAULT_BOUNDS)
  local reversed = {}
  for i = #raw, 1, -1 do table.insert(reversed, raw[i]) end
  local r2 = native_lookup.fuse_evidence(reversed, native_lookup.DEFAULT_BOUNDS)
  check("确定性: 融合数量一致", r1.stats.fused_candidates == r2.stats.fused_candidates)
  check("确定性: 边数一致", r1.stats.total_edges == r2.stats.total_edges)
  local sig1, sig2 = {}, {}
  for _, f in ipairs(r1.fused) do
    table.insert(sig1, string.format("%d:%d:%s", f.start, f.end_, f.text))
  end
  for _, f in ipairs(r2.fused) do
    table.insert(sig2, string.format("%d:%d:%s", f.start, f.end_, f.text))
  end
  check("确定性: 边序与插入顺序无关", table.concat(sig1, "|") == table.concat(sig2, "|"))
end

-- 有界性: 嵌套前缀夹具的 fallback 原语路径保留
do
  local case = fixture.cases[1] -- nested-prefix-path
  local raw = raw_from_case(case)
  local result = native_lookup.fuse_evidence(raw, native_lookup.DEFAULT_BOUNDS)
  -- 位置 0 必须有多个出边 span(valid prefix != commit boundary)
  local pos0 = 0
  local span_at_0 = {}
  for _, f in ipairs(result.fused) do
    if f.start == pos0 then
      span_at_0[string.format("%d", f.end_ - f.start)] = true
    end
  end
  check("嵌套前缀: 位置 0 至少 3 个不同 span 长度",
    span_at_0["1"] and span_at_0["2"] and span_at_0["3"])
end

print(string.format("\nnative_lookup 夹具一致性: %d 通过, %d 失败", passed, failed))
if failed > 0 then
  os.exit(1)
end
