-- XHUP Flow 简码提示 filter(docs/lua-runtime.md §4.1)。
--
-- librime-lua `*module` 组件(schema 以 `lua_filter@*xhup_flow.quick_hint`
-- 注册,零 rime.lua)。语义:候选词存在比当前输入更短的简码时,在候选
-- 注释追加 `⚡<简码>`。只追加注释,绝不改变候选次序(FROZEN STATIC
-- 契约);提示数据由 xhup-generator 生成(canonical 简码映射的提示视图)。
--
-- 性能:映射表 init 一次性加载(require 缓存),func 内每候选一次 O(1)
-- 哈希查询,零 IO、零正则、零大临时表。

local hints = require("xhup_flow.data.quick_hints")

-- 纯决策逻辑(脱离 librime 可单测):返回应追加的提示注释,或 nil。
-- 仅在简码严格短于当前输入时提示(更长的码没有提示价值)。
local function decorate(input, text, map)
  local hint = map[text]
  if hint == nil or hint == input or #hint >= #input then
    return nil
  end
  return "⚡" .. hint
end

local function init(env)
  env.hints = hints
end

local function func(translation, env)
  if not env.engine.context:get_option("quick_hint") then
    for cand in translation:iter() do
      yield(cand)
    end
    return
  end
  local input = env.engine.context.input
  for cand in translation:iter() do
    local suffix = decorate(input, cand.text, env.hints)
    if suffix and (cand.type == "table" or cand.type == "user_table") then
      local comment = cand.comment
      if comment and comment ~= "" then
        comment = comment .. " " .. suffix
      else
        comment = suffix
      end
      yield(ShadowCandidate(cand, cand.type, cand.text, comment))
    else
      yield(cand)
    end
  end
end

return { init = init, func = func, decorate = decorate }
