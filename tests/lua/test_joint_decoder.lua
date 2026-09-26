-- tests/lua/test_joint_decoder.lua
--
-- joint_decoder 纯决策核心的 Lua 单元测试(不依赖 librime)。
-- 运行: lua tests/lua/test_joint_decoder.lua

package.path = "rime/lua/?.lua;rime/lua/?/init.lua;" .. package.path

local jd = require("xhup_flow.joint_decoder")

local pass = 0
local fail = 0

local function check(label, actual, expected)
  if actual == expected then
    pass = pass + 1
  else
    fail = fail + 1
    io.stderr:write(string.format("FAIL: %s (期望 %s, 实际 %s)\n", label, tostring(expected), tostring(actual)))
  end
end

local hints = { ["时间"] = "uij", ["我们"] = "womf" }

-- 1. 模块结构
check("模块返回 table", type(jd) == "table", true)
check("bounded_promote 是函数", type(jd.bounded_promote) == "function", true)
check("DEFAULT_BOUND = 3", jd.DEFAULT_BOUND, 3)

-- 2. 空/无证据 → 恒等
check("nil repeated → 恒等", table.concat(jd.bounded_promote({ { type = "table", text = "时间" } }, nil, hints, "uij"), ","), "1")
check("空 repeated → 恒等",
  table.concat(jd.bounded_promote({ { type = "table", text = "上海" }, { type = "user_table", text = "北京" } }, {}, hints, "uij"), ","),
  "1,2")

-- 3. 重复词提前(非固定候选)
local cands = {
  { type = "table", text = "上海" },
  { type = "user_table", text = "北京" },
}
check("重复词提前", table.concat(jd.bounded_promote(cands, { ["北京"] = true }, hints, "uij"), ","), "2,1")

-- 4. 静态强固定 rank-1 永远最前(即使它也是重复词)
local fixed_cands = {
  { type = "table", text = "时间" },
  { type = "user_table", text = "北京" },
}
check("固定 rank-1 最前(重复词不越过)",
  table.concat(jd.bounded_promote(fixed_cands, { ["时间"] = true, ["北京"] = true }, hints, "uij"), ","),
  "1,2")
check("固定 rank-1 最前(仅另一词重复)",
  table.concat(jd.bounded_promote({ { type = "table", text = "时间" }, { type = "user_table", text = "上海" } }, { ["上海"] = true }, hints, "uij"), ","),
  "1,2")

-- 5. 多个重复词整组提前且保持原相对次序
local multi = {
  { type = "table", text = "上海" },
  { type = "table", text = "甲" },
  { type = "user_table", text = "北京" },
  { type = "user_table", text = "乙" },
}
check("多重复词整组提前(原相对序)",
  table.concat(jd.bounded_promote(multi, { ["北京"] = true, ["乙"] = true }, hints, "uij"), ","),
  "3,4,1,2")

-- 6. 提升结果等于原序 → 无动作(透传恒等)
local already = {
  { type = "table", text = "上海" },
  { type = "user_table", text = "乙" },
}
check("已是首位 → 恒等(无动作)", table.concat(jd.bounded_promote(already, { ["上海"] = true }, hints, "uij"), ","), "1,2")

-- 7. 单候选 → 恒等
check("单候选 → 恒等", table.concat(jd.bounded_promote({ { type = "table", text = "上海" } }, { ["上海"] = true }, hints, "uij"), ","), "1")

-- 8. hints 为 nil 时仍可提升(无固定判据,全部按 repeated/其余分桶)
check("hints=nil 仍提升重复词",
  table.concat(jd.bounded_promote(cands, { ["北京"] = true }, nil, "uij"), ","),
  "2,1")

print(string.format("PASS joint_decoder 纯决策测试:%d 通过,%d 失败", pass, fail))
if fail > 0 then
  os.exit(1)
end
