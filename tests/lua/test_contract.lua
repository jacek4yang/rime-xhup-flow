-- XHUP Flow 2.0 Lua 运行时合同纯逻辑单测(lua5.4,不依赖 librime)。
--
-- 运行: lua tests/lua/test_contract.lua (工作目录 = 仓库根)

package.path = "rime/lua/?.lua;rime/lua/?/init.lua;" .. package.path

package.preload["xhup_flow.data.quick_hints"] = function()
  return {
    ["时间"] = "uij",
    ["我们"] = "wm",
  }
end

local xhup_flow = require("xhup_flow")

local passed, failed = 0, 0
local function check(name, actual, expected)
  if actual == expected then
    passed = passed + 1
  else
    failed = failed + 1
    print(string.format("FAIL %s: 期望 %s, 实际 %s", name, tostring(expected), tostring(actual)))
  end
end

-- 1. 命名空间与版本
check("模块名", xhup_flow._MODULE, "xhup_flow")
check("模块版本", xhup_flow._VERSION, "2.0.0")

-- 2. 合同自检 (所有依赖正常在场)
local report = xhup_flow.check_contract()
check("健康合同检查通过", report.ok, true)
check("报告版本一致", report.version, "2.0.0")
check("annotation 组件状态", report.components.annotation.ok, true)
check("annotation 导出 format_shortcut_hint", report.components.annotation.has_format_shortcut_hint, true)
check("annotation 导出 clean_decorative_markers", report.components.annotation.has_clean_decorative_markers, true)
check("annotation 导出 format_candidate_comment", report.components.annotation.has_format_candidate_comment, true)
check("quick_hints 数据组件状态", report.components.quick_hints_data.ok, true)
check("quick_hints 条目计数", report.components.quick_hints_data.count, 2)
check("quick_hint filter 组件状态", report.components.quick_hint.ok, true)
check("quick_hint 导出 init", report.components.quick_hint.has_init, true)
check("quick_hint 导出 func", report.components.quick_hint.has_func, true)
check("无报错", #report.errors, 0)

-- 3. 诊断格式化输出
local diag = xhup_flow.diagnose_string()
check("诊断包含满足信息", string.find(diag, "满足 %(PASS%)") ~= nil, true)
check("诊断包含 annotation", string.find(diag, "annotation: 正常") ~= nil, true)

-- 4. 模拟故障注入: 缺少组件时应返回明确失败与诊断
local orig_loader = package.preload["xhup_flow.data.quick_hints"]
local orig_loaded = package.loaded["xhup_flow.data.quick_hints"]
package.loaded["xhup_flow.data.quick_hints"] = nil
package.preload["xhup_flow.data.quick_hints"] = function()
  error("模拟数据损坏")
end

local fail_report = xhup_flow.check_contract()
check("故障时合同检查失败", fail_report.ok, false)
check("数据组件标记失败", fail_report.components.quick_hints_data.ok, false)
check("报错清单非空", #fail_report.errors > 0, true)
check("报错包含故障详情", string.find(fail_report.errors[1], "模拟数据损坏") ~= nil, true)

-- 恢复桩件
package.preload["xhup_flow.data.quick_hints"] = orig_loader
package.loaded["xhup_flow.data.quick_hints"] = orig_loaded

print(string.format("----\n测试完成: %d 项通过, %d 项失败", passed, failed))
if failed > 0 then
  os.exit(1)
end
