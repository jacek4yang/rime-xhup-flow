-- context_ranker 纯逻辑单测(lua,不依赖 librime)。
--
-- 运行: lua tests/lua/test_context_ranker.lua (工作目录 = 仓库根)

package.path = "rime/lua/?.lua;rime/lua/?/init.lua;" .. package.path

local ranker = dofile("rime/lua/xhup_flow/context_ranker.lua")

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

-- 简码映射桩件:与生成数据一致的强固定映射样例。
local hints = { ["时间"] = "uij", ["我们"] = "wm" }

local cands_1 = {
  { type = "table", text = "时间" },
  { type = "table", text = "已见" },
  { type = "user_table", text = "时间" },
}
local cands_2 = {
  { type = "table", text = "已见" },
  { type = "table", text = "时间" },
  { type = "user_table", text = "时间" },
}

-- 1. 恒等透传场景
check("空上下文不动", table.concat(ranker.bounded_reorder(cands_2, nil, 3, hints, "uij"), ","), "1,2,3")
check("空串上下文不动", table.concat(ranker.bounded_reorder(cands_2, "", 3, hints, "uij"), ","), "1,2,3")
check("单候选不动", table.concat(ranker.bounded_reorder({ cands_2[1] }, "已见", 3, hints, "uij"), ","), "1")

-- 2. 重复词证据:刚提交「已见」→ 窗口内证据词形整组提前(组内原序)。
-- 注意输入码:uij 是「时间」的强固定映射码;非固定码用 uiij。
-- 三桶语义:固定 → 证据 → 其余;组内保持原相对次序。
local cands_rep = {
  { type = "table", text = "时间" },
  { type = "user_table", text = "已见" },
  { type = "table", text = "已见" },
}
check("证据组整组提前(组内原序)",
  order_text(cands_rep, ranker.bounded_reorder(cands_rep, "已见", 3, hints, "uiij")),
  "已见,已见,时间")
check("强固定码下时间恒居最前",
  table.concat(ranker.bounded_reorder(cands_rep, "已见", 3, hints, "uij"), ","),
  "1,2,3")
check("证据是时间且已居首:恒等(证据候选本来就在第一位)",
  table.concat(ranker.bounded_reorder(cands_rep, "时间", 3, hints, "uiij"), ","),
  "1,2,3")

-- 3. 静态强固定 rank-1 永远不被降位
local cands_fixed = {
  { type = "table", text = "时间" },
  { type = "user_table", text = "已见" },
}
check("强固定 rank-1 不被换下",
  table.concat(ranker.bounded_reorder(cands_fixed, "已见", 3, hints, "uij"), ","),
  "1,2")
check("非固定映射码可正常调序",
  table.concat(ranker.bounded_reorder(cands_fixed, "已见", 3, hints, "wmxx"), ","),
  "2,1")

-- 4. 无证据 → 恒等(即使窗口内存在固定映射候选;固定候选由静态层
-- 保证 rank-1,本地状态无证据时绝不重排)
local cands_curr_fixed = {
  { type = "user_table", text = "已见" },
  { type = "table", text = "时间" },
}
check("无证据:恒等(含固定映射候选)",
  table.concat(ranker.bounded_reorder(cands_curr_fixed, "时间", 3, hints, "uij"), ","),
  "1,2")
check("无证据且上下文不匹配任何词形:恒等",
  table.concat(ranker.bounded_reorder(cands_curr_fixed, "不存在", 3, hints, "uij"), ","),
  "1,2")

-- 5. 稳定性:多个证据候选整组提前,组内原相对次序不变(不引入任意挑选)
local cands_cascade = {
  { type = "table", text = "时间" },
  { type = "table", text = "已见" },
  { type = "user_table", text = "已见" },
}
check("证据组整组提前(非固定码)",
  order_text(cands_cascade, ranker.bounded_reorder(cands_cascade, "已见", 3, hints, "uiij")),
  "已见,已见,时间")
check("强固定码阻断一切降位",
  table.concat(ranker.bounded_reorder(cands_cascade, "已见", 3, hints, "uij"), ","),
  "1,2,3")

-- 6. is_fixed_first 语义
check("is_fixed_first:映射一致", ranker.is_fixed_first("table", "时间", "uij", hints), true)
check("is_fixed_first:映射不一致", ranker.is_fixed_first("table", "时间", "uijm", hints), false)
check("is_fixed_first:非 table 类型", ranker.is_fixed_first("user_table", "时间", "uij", hints), false)
check("is_fixed_first:未知词形", ranker.is_fixed_first("table", "词表外", "uij", hints), false)
check("is_fixed_first:空映射表", ranker.is_fixed_first("table", "时间", "uij", nil), false)

-- 7. bound 边界:窗口为 2 时,第 3 位候选不进入决策窗口
-- (func 层流式处理:窗口外候选只透传,不参与任何调序;此处直接以
-- 两元素窗口验证决策面)
check("bound=2 窗口内证据提升",
  table.concat(ranker.bounded_reorder({ cands_cascade[1], cands_cascade[2] }, "已见", 2, hints, "uiij"), ","),
  "2,1")

-- 8. 零词形特判:换一组完全不同的词形,规则仍然成立(纯谓词)
local hints2 = { ["北京"] = "bj" }
local cands_other = {
  { type = "table", text = "上海" },
  { type = "user_table", text = "北京" },
}
check("零特判:任意词形同样规则",
  table.concat(ranker.bounded_reorder(cands_other, "北京", 3, hints2, "bjxx"), ","),
  "2,1")

print(string.format("PASS context_ranker 纯逻辑单测:%d 通过,%d 失败", passed, failed))
if failed > 0 then
  os.exit(1)
end
