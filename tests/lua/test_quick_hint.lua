-- quick_hint 与 annotation 纯逻辑单测(lua5.4,不依赖 librime)。
--
-- 运行: lua tests/lua/test_quick_hint.lua (工作目录 = 仓库根)
-- 通过 package.preload 注入提示表桩件,隔离生成数据细节。

package.path = "rime/lua/?.lua;rime/lua/?/init.lua;" .. package.path

package.preload["xhup_flow.data.quick_hints"] = function()
  return {}
end

local annotation = require("xhup_flow.annotation")
local quick_hint = dofile("rime/lua/xhup_flow/quick_hint.lua")
local decorate = quick_hint.decorate

local passed, failed = 0, 0
local function check(name, actual, expected)
  if actual == expected then
    passed = passed + 1
  else
    failed = failed + 1
    print(string.format("FAIL %s: 期望 %s,实际 %s", name, tostring(expected), tostring(actual)))
  end
end

local hints = { ["时间"] = "uij", ["我们"] = "wm" }

-- 1. 正常模式:简码提示纯 ASCII 格式 (~uij，绝不含 ⚡)
check("全码提示 ASCII", decorate("uijm", "时间", hints), "~uij")
check("已是简码本身不提示", decorate("uij", "时间", hints), nil)
check("不省键不提示", decorate("ui", "时间", hints), nil)
check("等长不提示", decorate("wm", "我们", hints), nil)
check("无映射不提示", decorate("womf", "他们", hints), nil)

-- 2. annotation 模块基础与通用格式化
check("format_shortcut_hint 基础", annotation.format_shortcut_hint("uij"), "~uij")
check("format_shortcut_hint 空输入", annotation.format_shortcut_hint(""), "")
check("format_shortcut_hint nil", annotation.format_shortcut_hint(nil), "")

-- 3. 装饰标记清洗(清除已知装饰符，保留合法语义文本)
check("清洗 ☯ 单独标记", annotation.clean_decorative_markers(" ☯ "), "")
check("清洗 ⚡ 标记", annotation.clean_decorative_markers("⚡uij"), "uij")
check("清洗装饰保留语义文本", annotation.clean_decorative_markers("☯ 词库注释"), "词库注释")
check("清洗 emoji 装饰", annotation.clean_decorative_markers("🔥 常用"), "常用")
check("无装饰纯文本保持不变", annotation.clean_decorative_markers("shí jiān"), "shí jiān")
check("空注释清洗", annotation.clean_decorative_markers(""), "")

-- 4. 候选最终注释格式化(正常模式 vs 调试模式)
-- 4.1 正常模式:极简从属，只显示 ~uij，去除 ☯ 等装饰，不显示调试标签
check("正常模式加简码",
  annotation.format_candidate_comment(nil, "uij", "table", false),
  "~uij")
check("正常模式清洗 ☯ 用户词",
  annotation.format_candidate_comment(" ☯ ", nil, "user_table", false),
  "")
check("正常模式清洗 ☯ 并追加简码",
  annotation.format_candidate_comment(" ☯ ", "uij", "user_table", false),
  "~uij")
check("正常模式组句无多余标签",
  annotation.format_candidate_comment(nil, nil, "sentence", false),
  "")
check("正常模式保留正当语义注释并追加简码",
  annotation.format_candidate_comment("shí", "uij", "table", false),
  "shí ~uij")

-- 4.2 调试模式:明确有据的元数据标签 ([S], [USR])
check("调试模式组句标签",
  annotation.format_candidate_comment(nil, nil, "sentence", true),
  "[S]")
check("调试模式用户词标签",
  annotation.format_candidate_comment(" ☯ ", nil, "user_table", true),
  "[USR]")
check("调试模式用户词加简码",
  annotation.format_candidate_comment(" ☯ ", "uij", "user_table", true),
  "~uij [USR]")
check("调试模式普通码表不臆造标签",
  annotation.format_candidate_comment(nil, "uij", "table", true),
  "~uij")

if failed > 0 then
  os.exit(1)
end
print(string.format("PASS quick_hint 与 annotation 纯逻辑 %d 项", passed))
