-- XHUP Flow 有界上下文调序 filter(§4.3,含 R4 用户记忆消费)。
--
-- librime-lua `*module` 组件(schema 以 `lua_filter@*xhup_flow.context_ranker`
-- 注册,零 rime.lua)。
--
-- 语义契约(§4.3/§24 与 #83 R4):
-- 1. **有界**:只允许前 `bound`(默认 3,可配 2..5)个候选参与调序;
--    绝不扫描全候选流,绝不把任何候选移出窗口之外。
-- 2. **静态强固定映射不参与降位**:一级简码/FF/ZR 的 rank-1(primary
--    translator 的 `table` 候选在输入码 == 其映射码时)永远保持最前,
--    冻结静态契约不受任何运行时状态影响。
-- 3. **低置信度 = 不动**:证据是窗口内的明确信号(重复词 / 用户记忆),
--    无证据 → 透传不重排。
-- 4. **分桶优先级**:重复词(最近选择)> 用户记忆(历史频率)> 静态序;
--    每桶保持原相对次序(稳定、确定性、零词形特判)。
-- 5. **绝不改变候选文本与注释**(只调次序);绝不特判具体词形。
-- 6. 零 IO:证据来自 commit_notifier 内存态与同 schema user_memory
--    组件暴露的内存计数表;不读文件、无网络、无遥测。
-- 7. **默认可关**:方案开关 `context_ranker` 生产开启(#130,依据
--    #129 审计);关闭 = 纯透传。

-- 纯决策逻辑(脱离 librime 可单测)。
local M = {}

-- 默认调序窗口边界(§4.3:默认 3,可配)。
M.DEFAULT_BOUND = 3

-- 判断候选是否为「静态强固定映射的 rank-1」:
-- primary translator(table 字典)的候选在输入码与候选的映射码一致时,
-- 该候选是静态层 canonical 顺序的第一位,本地状态永远不得越过它。
-- `input_code` 为当前输入码,`cand` 为 {type=..., text=...} 桩件。
-- 判据:type == "table" 且该码存在唯一固定映射(hints 表查询,与
-- quick_hint 同源;映射存在且 == 输入码 ⇒ rank-1 强固定)。
local function is_fixed_first(cand_type, cand_text, input_code, hints)
  if cand_type ~= "table" then
    return false
  end
  local mapped = hints and hints[cand_text] or nil
  return mapped ~= nil and mapped == input_code
end

-- 有界重排(纯函数,单测核心):
-- 输入 cands:前 bound 个候选的 {type, text} 数组(最多 bound 个);
-- 输入 context_text:刚提交的文本(可为 nil/空);
-- 输入 user_counts:本地用户记忆计数表({word: count};可为 nil);
-- 返回:重排后的索引数组(每元素为原索引 1..n),透传时为恒等。
--
-- 语义 = 窗口内**稳定分桶**(每桶保持原相对次序,确定性、零特判):
--   1. 静态强固定映射(is_fixed_first)—— 冻结静态契约,永远最前;
--   2. 证据词形(text == context_text)的非固定候选 —— 整组提前;
--   3. 用户记忆词形(在 user_counts 中出现)的非固定候选 —— 整组提前;
--   4. 其余候选。
-- 用户记忆桶(3)优先级低于重复词桶(2):最近选择 > 历史频率。
-- 无任何证据时退化为恒等,行为可解释。
local function bounded_reorder(cands, context_text, bound, hints, input_code, user_counts)
  local n = #cands
  local order = {}
  for i = 1, n do
    order[i] = i
  end
  local has_context = context_text ~= nil and context_text ~= ""
  local has_user = user_counts ~= nil and next(user_counts) ~= nil
  if (not has_context and not has_user) or n < 2 then
    return order
  end
  local fixed, matched, learned, rest = {}, {}, {}, {}
  for i = 1, n do
    local c = cands[i]
    if is_fixed_first(c.type, c.text, input_code, hints) then
      table.insert(fixed, i)
    elseif has_context and c.text == context_text then
      table.insert(matched, i)
    elseif has_user and user_counts[c.text] then
      table.insert(learned, i)
    else
      table.insert(rest, i)
    end
  end
  if #matched == 0 and #learned == 0 then
    return order
  end
  local result = {}
  for _, idx in ipairs(fixed) do
    table.insert(result, idx)
  end
  for _, idx in ipairs(matched) do
    table.insert(result, idx)
  end
  for _, idx in ipairs(learned) do
    table.insert(result, idx)
  end
  for _, idx in ipairs(rest) do
    table.insert(result, idx)
  end
  return result
end

M.is_fixed_first = is_fixed_first
M.bounded_reorder = bounded_reorder

-- librime-lua 组件入口(env.engine, env.name_space)。
function M.init(env)
  local bound = M.DEFAULT_BOUND
  local ok, value = pcall(function()
    return env.engine.schema.config:get_int("context_ranker/bound")
  end)
  if ok and type(value) == "number" and value >= 2 and value <= 5 then
    bound = value
  end
  env.bound = bound
  -- 简码映射与 quick_hint 同源:用于识别静态强固定 rank-1。
  local ok_hints, loaded = pcall(function() return require("xhup_flow.data.quick_hints") end)
  env.hints = (ok_hints and type(loaded) == "table") and loaded or {}
end

function M.func(translation, env)
  local enabled = env.engine.context:get_option("context_ranker")
  if not enabled then
    -- 关闭:严格透传,与无此 filter 完全一致。
    for cand in translation:iter() do
      yield(cand)
    end
    return
  end

  -- 证据源:user_memory 模块共享状态(经 require 读同一份 M.state;
  -- engine userdata 不接受字段赋值,实测静默失败)。
  local um_ok, um = pcall(require, "xhup_flow.user_memory")
  local context_text = nil
  local user_counts = nil
  if um_ok and type(um) == "table" and type(um.state) == "table" then
    context_text = um.state.last_commit
    user_counts = um.state.counts
  end

  local input_code = env.engine.context.input
  local head, head_meta = {}, {}
  local n = 0
  for cand in translation:iter() do
    if n < env.bound then
      n = n + 1
      head[n] = cand
      head_meta[n] = { type = cand.type, text = cand.text }
    else
      -- 决策一次,吐出窗口,再重新缓冲(流式;窗口外的候选不受影响)。
      for _, idx in ipairs(M.bounded_reorder(head_meta, context_text, env.bound, env.hints, input_code, user_counts)) do
        yield(head[idx])
      end
      head, head_meta = {}, {}
      n = 0
      yield(cand)
    end
  end
  if n > 0 then
    for _, idx in ipairs(M.bounded_reorder(head_meta, context_text, env.bound, env.hints, input_code, user_counts)) do
      yield(head[idx])
    end
  end
end

return M
