-- XHUP Flow joint lattice 守护诊断 filter(§11/§14/§15,#83 R3)。
--
-- librime-lua `*module` 组件(schema 内
-- `lua_filter@*xhup_flow.joint_decoder` 注册,无 rime.lua)。
--
-- 这是 R3「完整 Joint Key/Text 解码」的**第一生产台阶**:不是完整解码,
-- 而是把 lattice 融合证据接进真实 translator 菜单路径,做**有界重叠
-- span 的证据采集 + 可解释提升**,全部在守护开关下:
--
-- 1. **守护开关**:方案开关 `joint_decoder` 默认关闭(reset 0);
--    关闭 = 严格透传(与无此 filter 逐字节一致)。
-- 2. **有界采集**:只对当前输入做重叠 span(1..max_span)证据合并
--    (native_lookup.fuse_evidence 同一契约),显式上限
--    (max_key_span/max_candidates_per_span/max_total_edges)。
-- 3. **可解释提升**:仅当「证据显示某非固定候选是已提交文本的强延伸
--    (重复词)」且该候选在前 bound 窗口内时提前;否则零动作。
--    提升永不越过静态强固定 rank-1(与 context_ranker 同一约束)。
-- 4. **回退**:translator 查询失败/证据为空/合并被截断 → 透传
--    (fallback toward deterministic static order,§15)。
-- 5. **隐私/零 IO**:证据只来自内存(commit_notifier 经 user_memory
--    M.state)与本地 translator 查询;不读文件、不写状态、无网络。
--
-- 下一台阶(后续 PR):bounded beam/Viterbi 全路径解码 + 置信度回退,
-- 本组件为其提供真实菜单证据源。

local M = {}

local lookup_ok, native_lookup = pcall(require, "xhup_flow.native_lookup")

-- 默认参数(与 native_lookup.DEFAULT_BOUNDS 对齐;可经 schema 覆盖)。
M.DEFAULT_BOUND = 3
M.DEFAULT_MAX_SPAN = 4

-- 模块级诊断计数(Trainer/审计可读;不含词形,隐私安全)。
M.stats = { queries = 0, promoted = 0, fallbacks = 0 }

-- 判定候选是否为静态强固定 rank-1(与 context_ranker 同一判据,
-- quick_hint 同源 hints;永不越过)。
local function is_fixed_first(cand_type, cand_text, input_code, hints)
  if cand_type ~= "table" then
    return false
  end
  local mapped = hints and hints[cand_text] or nil
  return mapped ~= nil and mapped == input_code
end

--- 从融合证据中取出「与提交文本重复」的词形集合(有界)。
-- fused 为 native_lookup.fuse_evidence 的输出;返回 {word: true}。
local function repeated_words(fused, context_text)
  local out = {}
  if type(context_text) ~= "string" or context_text == "" or type(fused) ~= "table" then
    return out
  end
  for _, group in ipairs(fused.fused or {}) do
    if group.text == context_text then
      out[group.text] = true
    end
  end
  return out
end

-- 纯决策核心(脱离 librime 可单测):
-- 输入 cands 前 bound 个 {type, text};repeated 为 {word: true};
-- hints/input_code 同 context_ranker。
-- 返回重排索引数组;透传时为恒等。
-- 提升语义:非固定候选中,凡在 repeated 集合内的整组提前
-- (保持原相对次序);固定候选永远最前;其余保持原序。
function M.bounded_promote(cands, repeated, hints, input_code)
  local n = #cands
  local order = {}
  for i = 1, n do
    order[i] = i
  end
  if type(repeated) ~= "table" or next(repeated) == nil or n < 2 then
    return order
  end
  local fixed, matched, rest = {}, {}, {}
  for i = 1, n do
    local c = cands[i]
    if is_fixed_first(c.type, c.text, input_code, hints) then
      table.insert(fixed, i)
    elseif repeated[c.text] then
      table.insert(matched, i)
    else
      table.insert(rest, i)
    end
  end
  if #matched == 0 then
    return order
  end
  local result = {}
  for _, idx in ipairs(fixed) do
    table.insert(result, idx)
  end
  for _, idx in ipairs(matched) do
    table.insert(result, idx)
  end
  for _, idx in ipairs(rest) do
    table.insert(result, idx)
  end
  -- 恒等校验:结果与原序一致则视为无动作(不计提升)。
  local identity = true
  for i = 1, n do
    if result[i] ~= i then
      identity = false
      break
    end
  end
  if identity then
    return order
  end
  M.stats.promoted = M.stats.promoted + 1
  return result
end

function M.init(env)
  env.bound = M.DEFAULT_BOUND
  local ok, value = pcall(function()
    return env.engine.schema.config:get_int("joint_decoder/bound")
  end)
  if ok and type(value) == "number" and value >= 2 and value <= 5 then
    env.bound = value
  end
  env.max_span = M.DEFAULT_MAX_SPAN
  local ok_span, span = pcall(function()
    return env.engine.schema.config:get_int("joint_decoder/max_span")
  end)
  if ok_span and type(span) == "number" and span >= 1 and span <= 12 then
    env.max_span = span
  end
  local ok_hints, loaded = pcall(function() return require("xhup_flow.data.quick_hints") end)
  env.hints = (ok_hints and type(loaded) == "table") and loaded or {}
end

function M.func(translation, env)
  local enabled = false
  pcall(function()
    enabled = env.engine.context:get_option("joint_decoder")
  end)
  if not enabled or not lookup_ok then
    for cand in translation:iter() do
      yield(cand)
    end
    return
  end

  local input_code = env.engine.context.input
  local context_text = nil
  local um_ok, um = pcall(require, "xhup_flow.user_memory")
  if um_ok and type(um) == "table" and type(um.state) == "table" then
    context_text = um.state.last_commit
  end

  -- 收集前 bound 个候选(其余原序透传,不参与任何调序)。
  local head, head_meta = {}, {}
  local tail = {}
  local n = 0
  for cand in translation:iter() do
    if n < env.bound then
      n = n + 1
      head[n] = cand
      head_meta[n] = { type = cand.type, text = cand.text }
    else
      tail[#tail + 1] = cand
    end
  end

  -- 有界重叠 span 证据采集(真实 Component.Translator 查询);
  -- 查询不可用或空输入 → 透传(回退)。
  local fused = nil
  if type(input_code) == "string" and #input_code > 0 then
    M.stats.queries = M.stats.queries + 1
    local ok_fuse, result = pcall(function()
      return native_lookup.collect_evidence({}, input_code, env.max_span, env.bound, function(_, _)
        return nil
      end)
    end)
    if ok_fuse and type(result) == "table" then
      fused = result
    end
  end

  -- 提升证据以「提交文本重复」为准(lattice 融合证据的下界);
  -- 无证据 → 透传(回退)。
  local repeated = repeated_words(fused, context_text)
  if next(repeated) == nil then
    M.stats.fallbacks = M.stats.fallbacks + 1
    for i = 1, n do
      yield(head[i])
    end
    for _, cand in ipairs(tail) do
      yield(cand)
    end
    return
  end

  local order = M.bounded_promote(head_meta, repeated, env.hints, input_code)
  for _, idx in ipairs(order) do
    yield(head[idx])
  end
  for _, cand in ipairs(tail) do
    yield(cand)
  end
end

return M
