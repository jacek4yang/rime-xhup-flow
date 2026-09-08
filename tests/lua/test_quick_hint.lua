-- quick_hint.decorate 纯逻辑单测(lua5.4,不依赖 librime)。
--
-- 运行: lua5.4 tests/lua/test_quick_hint.lua (工作目录 = 仓库根)
-- 通过 package.preload 注入提示表桩件,隔离生成数据细节。

package.preload["xhup_flow.data.quick_hints"] = function()
  return {}
end
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

-- 全码输入(4 键)→ 提示 3 键简码。
check("全码提示", decorate("uijm", "时间", hints), "⚡uij")
-- 输入已是简码本身 → 不提示。
check("已是简码", decorate("uij", "时间", hints), nil)
-- 简码不短于输入 → 不提示(没有省键价值)。
check("不省键", decorate("ui", "时间", hints), nil)
check("等长", decorate("wm", "我们", hints), nil)
-- 无简码的词 → 不提示。
check("无映射", decorate("womf", "他们", hints), nil)
-- 候选次序相关:函数纯映射,不接触候选顺序(架构不变量由集成审计覆盖)。

if failed > 0 then
  os.exit(1)
end
print(string.format("PASS quick_hint 纯逻辑 %d 项", passed))
