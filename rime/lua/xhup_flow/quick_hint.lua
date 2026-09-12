-- XHUP Flow 简码提示与候选注释 filter(docs/lua-runtime.md §4.1)。
--
-- librime-lua `*module` 组件(schema 以 `lua_filter@*xhup_flow.quick_hint`
-- 注册,零 rime.lua)。
--
-- 语义契约:
-- 1. 正常模式:候选词存在比当前输入更短的简码时,在候选注释追加 `~<简码>`(纯 ASCII)。
-- 2. 清洗装饰性与引擎生成标记,保持纯净 ASCII 与合法语义注释。
-- 3. 调试模式(`debug_candidate_annotations` 开启):输出明确可信的元数据标签(`[S]`, `[USR]`)。
-- 4. 只追加/格式化注释,绝不改变候选次序与 rank,绝不篡改候选文本(FROZEN STATIC 契约)。
--
-- 性能:映射表 init 一次性加载(require 缓存),func 内每候选一次 O(1)
-- 哈希查询,零 IO、零大临时表。

local ok_hints, loaded_hints = pcall(function() return require("xhup_flow.data.quick_hints") end)
local hints = (ok_hints and type(loaded_hints) == "table") and loaded_hints or {}

local ok_ann, loaded_ann = pcall(function() return require("xhup_flow.annotation") end)
local annotation = (ok_ann and type(loaded_ann) == "table") and loaded_ann or nil

-- 纯决策逻辑(脱离 librime 可单测):返回可用的最优简码，或 nil。
-- 仅在简码严格短于当前输入时提示(更长的码没有提示价值)。
local function should_hint(input, text, map)
  local hint = map and map[text] or nil
  if hint == nil or hint == input or #hint >= #input then
    return nil
  end
  return hint
end

-- 兼容旧单测/对外辅助入口:返回格式化后的简码提示字符串或 nil
local function decorate(input, text, map)
  local hint = should_hint(input, text, map)
  if not hint or not annotation then
    return nil
  end
  return annotation.format_shortcut_hint(hint)
end

local function init(env)
  env.hints = hints
  env.annotation = annotation
end

local function func(translation, env)
  local ann = env.annotation or annotation
  if not ann then
    for cand in translation:iter() do
      yield(cand)
    end
    return
  end

  local show_hint = env.engine.context:get_option("quick_hint")
  local is_debug = env.engine.context:get_option("debug_candidate_annotations")
  local input = env.engine.context.input

  for cand in translation:iter() do
    local hint_code = nil
    if show_hint and (cand.type == "table" or cand.type == "user_table") then
      hint_code = should_hint(input, cand.text, env.hints)
    end

    local orig_comment = cand.comment
    local new_comment = ann.format_candidate_comment(
      orig_comment,
      hint_code,
      cand.type,
      is_debug
    )

    if new_comment ~= (orig_comment or "") then
      yield(ShadowCandidate(cand, cand.type, cand.text, new_comment))
    else
      yield(cand)
    end
  end
end

return {
  init = init,
  func = func,
  decorate = decorate,
  should_hint = should_hint,
}
