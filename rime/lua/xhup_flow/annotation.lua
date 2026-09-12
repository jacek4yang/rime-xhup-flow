-- XHUP Flow 候选注释格式化策略(docs/lua-runtime.md)
--
-- 核心原则:
-- 1. 正常模式视觉极简、从属候选文本、纯 ASCII 呈现。
-- 2. 简码提示统一格式: "~" .. code (例如 "~uij")。
-- 3. 清理已知的项目级/引擎生成装饰标记(如 "⚡", "☯" 等)，保留合法语义注释。
-- 4. 调试模式支持明确可信的元数据标签: [S] (组句), [USR] (用户词)。
-- 5. 绝不修改候选文本 cand.text，绝不修改候选排序与 rank。

local M = {}

-- 简码前缀标识符(纯 ASCII)
local SHORTCUT_PREFIX = "~"

-- 需要在正常模式中移除的装饰性/系统标记
-- 包含 quick_hint 历史遗留 "⚡"、librime table_translator UnitySymbol "☯" 以及其他装饰 emoji
-- 注意: Lua 原生模式匹配以字节处理，多字节 UTF-8 符号不可放入 [...] 字符类中，
-- 必须逐个完整多字节字符串安全替换，防止伤及普通 CJK 字符编码。
local DECORATIVE_SYMBOLS = { "⚡", "☯", "🔥", "🧠", "⭐" }

--- 格式化简码提示
--- @param code string 简码字符串
--- @return string 格式化后的简码提示，如 "~uij"
function M.format_shortcut_hint(code)
  if not code or code == "" then
    return ""
  end
  return SHORTCUT_PREFIX .. code
end

--- 清理注释中的装饰性/符号标记，保留合法文本内容
--- @param comment string|nil 原始注释
--- @return string 清理后的注释(去除非必要装饰及首尾多余空格)
function M.clean_decorative_markers(comment)
  if not comment or comment == "" then
    return ""
  end
  local cleaned = comment
  for _, sym in ipairs(DECORATIVE_SYMBOLS) do
    cleaned = cleaned:gsub(sym, "")
  end
  -- 规范化连续多空格并修剪首尾空格
  cleaned = cleaned:gsub("%s+", " "):gsub("^%s+", ""):gsub("%s+$", "")
  return cleaned
end

--- 根据候选类型推导调试模式标签
--- @param cand_type string 候选类型 (如 "sentence", "user_table", "table")
--- @return string|nil 调试标签 (如 "[S]", "[USR]")，若无明确支持分类则返回 nil
function M.debug_tag_for_type(cand_type)
  if cand_type == "sentence" then
    return "[S]"
  elseif cand_type == "user_table" then
    return "[USR]"
  end
  -- 对于 "table" 或其他类型，在当前元数据下不臆测标签，保持干净
  return nil
end

--- 格式化候选最终注释
--- @param original_comment string|nil 候选原本的注释
--- @param shortcut_code string|nil 待追加的简码 (若有)
--- @param cand_type string|nil 候选类型 (如 "table", "user_table", "sentence")
--- @param is_debug boolean 是否开启调试模式
--- @return string 最终注释字符串
function M.format_candidate_comment(original_comment, shortcut_code, cand_type, is_debug)
  local parts = {}

  -- 1. 处理原始注释(清洗装饰性符号，保留合法语义文本)
  local cleaned = M.clean_decorative_markers(original_comment)
  if cleaned ~= "" then
    table.insert(parts, cleaned)
  end

  -- 2. 简码提示(若提供)
  if shortcut_code and shortcut_code ~= "" then
    table.insert(parts, M.format_shortcut_hint(shortcut_code))
  end

  -- 3. 调试模式标签(若开启且类型具备确信依据)
  if is_debug and cand_type then
    local tag = M.debug_tag_for_type(cand_type)
    if tag then
      table.insert(parts, tag)
    end
  end

  return table.concat(parts, " ")
end

return M
