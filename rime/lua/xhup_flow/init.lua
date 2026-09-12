-- XHUP Flow 2.0 Lua 运行时入口与合同诊断模块 (docs/lua-runtime.md)
--
-- 1. 命名空间出口: require("xhup_flow") 导出核心模块与诊断接口
-- 2. 运行时合同诊断: check_contract() 检验 Lua 环境、版本与子模块健康度
-- 3. 2.0 mandatory Lua 合同: 主方案 xhup_flow 依赖 librime-lua 运行时;
--    若环境未加载 librime-lua 或关键模块缺失，提供明确诊断与安装指引。

local M = {
  _VERSION = "2.0.0",
  _MODULE = "xhup_flow",
}

-- 延迟/安全加载子模块
function M.annotation()
  return require("xhup_flow.annotation")
end

function M.quick_hint()
  return require("xhup_flow.quick_hint")
end

function M.quick_hints_data()
  return require("xhup_flow.data.quick_hints")
end

-- 运行时环境与合同自检
-- 返回: { ok = boolean, version = string, lua_version = string, components = table, errors = table }
function M.check_contract()
  local report = {
    ok = true,
    module = M._MODULE,
    version = M._VERSION,
    lua_version = _VERSION,
    components = {},
    errors = {},
  }

  -- 1. 检查 annotation 模块
  local ok_ann, ann_or_err = pcall(require, "xhup_flow.annotation")
  if ok_ann and type(ann_or_err) == "table" then
    report.components.annotation = {
      ok = true,
      has_format_shortcut_hint = type(ann_or_err.format_shortcut_hint) == "function",
      has_format_candidate_comment = type(ann_or_err.format_candidate_comment) == "function",
      has_clean_decorative_markers = type(ann_or_err.clean_decorative_markers) == "function",
    }
  else
    report.ok = false
    report.components.annotation = { ok = false, error = tostring(ann_or_err) }
    table.insert(report.errors, "annotation 模块加载失败: " .. tostring(ann_or_err))
  end

  -- 2. 检查 quick_hints 数据模块
  local ok_data, data_or_err = pcall(require, "xhup_flow.data.quick_hints")
  if ok_data and type(data_or_err) == "table" then
    local count = 0
    for _ in pairs(data_or_err) do
      count = count + 1
    end
    report.components.quick_hints_data = {
      ok = true,
      count = count,
    }
  else
    report.ok = false
    report.components.quick_hints_data = { ok = false, error = tostring(data_or_err) }
    table.insert(report.errors, "quick_hints 数据模块加载失败: " .. tostring(data_or_err))
  end

  -- 3. 检查 quick_hint filter 模块
  local ok_qh, qh_or_err = pcall(require, "xhup_flow.quick_hint")
  if ok_qh and type(qh_or_err) == "table" then
    report.components.quick_hint = {
      ok = true,
      has_init = type(qh_or_err.init) == "function",
      has_func = type(qh_or_err.func) == "function",
    }
  else
    report.ok = false
    report.components.quick_hint = { ok = false, error = tostring(qh_or_err) }
    table.insert(report.errors, "quick_hint 模块加载失败: " .. tostring(qh_or_err))
  end

  return report
end

-- 格式化为人类可读的诊断文本
function M.diagnose_string()
  local report = M.check_contract()
  local lines = {}
  table.insert(lines, string.format("XHUP Flow Lua 运行时诊断 (v%s, %s)", report.version, report.lua_version or "unknown"))
  table.insert(lines, string.format("合同状态: %s", report.ok and "满足 (PASS)" or "未满足 (FAIL)"))
  for comp_name, comp in pairs(report.components) do
    if comp.ok then
      if comp.count ~= nil then
        table.insert(lines, string.format("  - %s: 正常 (条目数: %d)", comp_name, comp.count))
      else
        table.insert(lines, string.format("  - %s: 正常", comp_name))
      end
    else
      table.insert(lines, string.format("  - %s: 失败 (%s)", comp_name, comp.error or "未知错误"))
    end
  end
  if #report.errors > 0 then
    table.insert(lines, "错误清单:")
    for _, err in ipairs(report.errors) do
      table.insert(lines, "  * " .. err)
    end
  end
  return table.concat(lines, "\n")
end

return M
