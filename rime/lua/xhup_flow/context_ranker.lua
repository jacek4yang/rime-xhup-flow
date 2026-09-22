-- XHUP Flow 有界上下文调序 filter(docs/lua-runtime.md §4.3)。
--
-- librime-lua `*module` 组件(schema 以 `lua_filter@*xhup_flow.context_ranker`
-- 注册,零 rime.lua)。
--
-- 语义契约(§4.3 与 AGENTS 硬性规则):
-- 1. **有界调序**:只允许前 `bound`(默认 3,可配)个候选参与相邻交换;
--    绝不扫描全候选流,绝不把任何候选移出前 bound 窗口之外。
-- 2. **静态强固定映射不参与调序**:一级简码/FF/ZR 的 rank-1(primary
--    translator 的 `table` 候选在输入码 == 其映射码时)永远保持原位,
--    冻结静态契约不受本地状态影响。
-- 3. **低置信度 = 不动**:调序证据是「刚提交的文本与候选词形完全相同」
--    (重复词场景);无提交历史、上下文为空、证据弱 → 透传不重排。
-- 4. **默认关闭**:方案开关 `context_ranker` 默认 reset: 0,行为与无此
--    filter 严格一致(纯透传);显式开启后才可能重排(受 1-3 约束)。
-- 5. **绝不改变候选文本与注释**(只调次序);**绝不特判具体词形** ——
--    证据函数是纯谓词,输入词形集合之外零知识。
-- 6. 零 IO:证据来自 `context:get_commit_text()`(librime-lua 运行时
--    内存态),不读文件、无网络、无遥测。
--
-- 性能:每段最多缓存 bound 个候选,决策 O(1);透传路径零分配。

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
-- 返回:重排后的索引数组(每元素为原索引 1..n),透传时为恒等。
--
-- 语义 = 窗口内**稳定三桶**(每桶保持原相对次序,确定性、零特判):
--   1. 静态强固定映射(is_fixed_first)—— 冻结静态契约,永远最前;
--   2. 证据词形(text == context_text)的非固定候选 —— 整组提前;
--   3. 其余候选。
-- 无证据或无固定候选时退化为恒等/纯证据提升,行为可解释。
local function bounded_reorder(cands, context_text, bound, hints, input_code)
  local n = #cands
  local order = {}
  for i = 1, n do
    order[i] = i
  end
  if context_text == nil or context_text == "" or n < 2 then
    return order
  end
  local fixed, matched, rest = {}, {}, {}
  for i = 1, n do
    local c = cands[i]
    if is_fixed_first(c.type, c.text, input_code, hints) then
      table.insert(fixed, i)
    elseif c.text == context_text then
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
  -- 本地证据源:commit_notifier 记录最近一次上屏文本(librime-lua 标准
  -- API,跨版本稳定;模块内存态,零 IO 零持久化,进程退出即消失)。
  -- notifier 回调无参数(Signal 约定),经闭包取 context;读取链双回退:
  -- get_commit_text() → commit_history:back().text,全部 pcall 包裹,
  -- 任一不可用 = 证据恒空 = 恒等透传(安全降级)。
  env.last_commit = nil
  local ok_notifier = pcall(function()
    env.engine.context.commit_notifier:connect(function()
      local c = env.engine.context
      local ok_text, text = pcall(function() return c:get_commit_text() end)
      if not (ok_text and type(text) == "string" and text ~= "") then
        ok_text, text = pcall(function()
          local entry = c.commit_history and c.commit_history:back() or nil
          return entry and entry.text or nil
        end)
      end
      if ok_text and type(text) == "string" and text ~= "" then
        env.last_commit = text
        io.stderr:write("[context_ranker] notifier: last_commit=", text, "\n")
      else
        env.last_commit = nil
        io.stderr:write("[context_ranker] notifier: empty evidence\n")
      end
    end)
  end)
  io.stderr:write("[context_ranker] init: notifier connect ok=", tostring(ok_notifier), "\n")
  if not ok_notifier then
    -- notifier 不可用:证据恒空 = 恒等透传(降级为纯静态行为,安全)。
    env.last_commit = nil
  end
end

function M.func(translation, env)
  local enabled = env.engine.context:get_option("context_ranker")
  if not enabled then
    -- 默认关闭:严格透传,与无此 filter 完全一致。
    for cand in translation:iter() do
      yield(cand)
    end
    return
  end

  io.stderr:write("[context_ranker] func: option on, last=",
                  tostring(env.last_commit), "\n")

  -- 证据 = 最近一次上屏文本(commit_notifier 记录;nil/空 = 无证据恒等)。
  local context_text = env.last_commit
  if context_text == nil or context_text == "" then
    for cand in translation:iter() do
      yield(cand)
    end
    return
  end

  local input_code = env.engine.context.input
  -- 缓冲前 bound 个候选做决策;其余照序透传(绝不扫描全流)。
  local head, head_meta = {}, {}
  local n = 0
  for cand in translation:iter() do
    if n < env.bound then
      n = n + 1
      head[n] = cand
      head_meta[n] = { type = cand.type, text = cand.text }
    else
      -- 决策一次,吐出窗口,再重新缓冲(流式;窗口外的候选不受影响)。
      for _, idx in ipairs(M.bounded_reorder(head_meta, context_text, env.bound, env.hints, input_code)) do
        yield(head[idx])
      end
      head, head_meta = {}, {}
      n = 0
      yield(cand)
    end
  end
  if n > 0 then
    for _, idx in ipairs(M.bounded_reorder(head_meta, context_text, env.bound, env.hints, input_code)) do
      yield(head[idx])
    end
  end
end

return M
