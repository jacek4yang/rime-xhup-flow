-- context_ranker 模块级仿真(真实生成数据,无 librime):
-- 用真实简码映射验证「强固定 rank-1 识别」在生成数据上成立。
-- 用法: lua tests/lua/sim_context_ranker.lua <生成包目录>
local pkg = arg[1] or error("用法: lua sim_context_ranker.lua <生成包目录>")
package.path = pkg .. "/lua/?.lua;" .. pkg .. "/lua/?/init.lua;" .. package.path

local ranker = dofile(pkg .. "/lua/xhup_flow/context_ranker.lua")
local hints = require("xhup_flow.data.quick_hints")

local passed, failed = 0, 0
local function check(name, actual, expected)
  if actual == expected then
    passed = passed + 1
  else
    failed = failed + 1
    print(string.format("FAIL %s: 期望 %s,实际 %s", name, tostring(expected), tostring(actual)))
  end
end

local function order_text(cands, order)
  local out = {}
  for i, idx in ipairs(order) do
    out[i] = cands[idx].text
  end
  return table.concat(out, ",")
end

-- 1. 真实数据规模(与 sim_quick_hint 同源)
local count = 0
for _ in pairs(hints) do
  count = count + 1
end
check("提示表规模 > 40000", count > 40000, true)
check("时间 → uij", hints["时间"], "uij")

-- 2. 抽样验证强固定识别:任取 1000 个映射,映射码 == 输入码时必须识别
--    为固定(纯谓词在真实数据上零特判)。
local sample, seen = 0, 0
for text, code in pairs(hints) do
  seen = seen + 1
  if seen % 69 == 0 then
    local cands = {
      { type = "table", text = text },
      { type = "user_table", text = "___非证据词形___" },
    }
    local order = ranker.bounded_reorder(cands, "___非证据词形___", 3, hints, code)
    check("强固定识别(真实词形 " .. text .. ")", order[1], 1)
    sample = sample + 1
    if sample >= 1000 then
      break
    end
  end
end
check("抽样规模", sample >= 1000, true)

-- 3. 证据提升在真实词形上工作:
--    用户刚提交「已见」(非固定映射词形)时,窗口内它应整组提前。
local cands = {
  { type = "table", text = "时间" },
  { type = "user_table", text = "已见" },
  { type = "table", text = "已见" },
}
check("真实词形证据提升(非固定码)",
  order_text(cands, ranker.bounded_reorder(cands, "已见", 3, hints, "uijm")),
  "已见,已见,时间")
check("固定码下时间恒居最前",
  table.concat(ranker.bounded_reorder(cands, "已见", 3, hints, "uij"), ","),
  "1,2,3")

print(string.format("PASS context_ranker 模块级仿真(真实数据 %d 条,抽样 %d)", count, sample))
if failed > 0 then
  os.exit(1)
end
