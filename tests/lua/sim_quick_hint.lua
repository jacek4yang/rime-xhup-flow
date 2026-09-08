-- 生成包 Lua 模块级仿真(真实数据,无 librime):
-- 用法: lua tests/lua/sim_quick_hint.lua <生成包目录>
local pkg = arg[1] or error("用法: lua sim_quick_hint.lua <生成包目录>")
package.path = pkg .. "/lua/?.lua;" .. pkg .. "/lua/?/init.lua;" .. package.path

local quick_hint = dofile(pkg .. "/lua/xhup_flow/quick_hint.lua")
local hints = require("xhup_flow.data.quick_hints")

local count = 0
for _ in pairs(hints) do count = count + 1 end
assert(count > 40000, "提示表规模")
assert(hints["时间"] == "uij", "时间 → uij")
assert(quick_hint.decorate("uijm", "时间", hints) == "⚡uij", "全码提示")
assert(quick_hint.decorate("uij", "时间", hints) == nil, "简码本身不提示")
assert(quick_hint.decorate("ui", "时间", hints) == nil, "不省键不提示")
print(string.format("PASS 模块级仿真(真实数据 %d 条)", count))
