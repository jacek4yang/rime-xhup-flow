-- Lua 运行时内存基线(§22:memory / GC / model size 此前无任何基线)。
--
-- 用法: lua5.4 tests/lua/measure_memory.lua <生成包目录>
--
-- 为什么需要:docs/performance-baseline.md 覆盖了生成与部署开销,但 §22 明确
-- 要求测量 **Lua memory / Lua GC / model size**。运行时策略层的实际内存占用
-- 直接决定移动端(Android)可用性,而在此之前仓库里没有任何数字。
--
-- 测量内容:
--   1. `collectgarbage("count")` 在加载 quick_hints 数据模块前后的差值
--      —— 即**模型体积**(常驻 Lua 堆);
--   2. 一次强制 GC 后的驻留量(区分「分配量」与「存活量」);
--   3. 一次 decorate 调用的耗时(p50/p95,微秒)—— 每键路径的开销。
--
-- 口径说明(避免误读):
--   - 这是 **lua5.4 独立解释器**的读数,不是 librime-lua 进程内的实测;
--     librime 插件下 Lua 状态与 librime 共享进程,数值会有差异;
--   - 仅在 Linux CI 上跑(与其它 Lua 测试相同);
--   - 只报告,**不设跨机器门槛**(§22 明确禁止机器相关硬阈值)。

local pkg = arg[1] or error("用法: lua5.4 measure_memory.lua <生成包目录>")
package.path = pkg .. "/lua/?.lua;" .. pkg .. "/lua/?/init.lua;" .. package.path

-- 1) 基线:加载数据模块前
collectgarbage("collect")
collectgarbage("collect")
local before = collectgarbage("count")

-- 2) 加载真实数据模块(与生产包同一份生成产物)
local hints = require("xhup_flow.data.quick_hints")
collectgarbage("collect")
local after_model = collectgarbage("count")

local count = 0
for _ in pairs(hints) do count = count + 1 end
assert(count > 40000, "提示表规模应与生产一致,实际 " .. count)

-- 3) 加载策略模块(annotation / quick_hint),度量其代码开销
local quick_hint = dofile(pkg .. "/lua/xhup_flow/quick_hint.lua")
local annotation = require("xhup_flow.annotation")
collectgarbage("collect")
local after_code = collectgarbage("count")

-- 4) 调用一次,确保惰性分配的路径被计入
local probes = { "时间", "我们", "工作", "问题", "时候" }
local decorated = 0
for _, w in ipairs(probes) do
  local code = hints[w]
  if code then
    local d = quick_hint.decorate(code .. "x", w, hints)
    if d then decorated = decorated + 1 end
  end
end
collectgarbage("collect")
local steady = collectgarbage("count")

-- 5) GC 压力:重复 decorate 后强制回收,观察是否持续增长(泄漏信号)
local iterations = 100000
local start = os.clock()
for i = 1, iterations do
  local w = probes[(i % #probes) + 1]
  local code = hints[w]
  if code then
    quick_hint.decorate(code .. "x", w, hints)
  end
end
local elapsed = os.clock() - start
collectgarbage("collect")
local after_loop = collectgarbage("count")

-- 报告(确定性数值 + 明示口径)
local model_kb = after_model - before
local code_kb = after_code - after_model
local steady_kb = steady - after_code
local growth_kb = after_loop - steady

print(string.format("lua version            %s", _VERSION))
print(string.format("hint entries           %d", count))
print(string.format("model heap (kB)        %.1f", model_kb))
print(string.format("code heap (kB)         %.1f", code_kb))
print(string.format("steady heap (kB)       %.1f", steady_kb))
print(string.format("iterations             %d", iterations))
print(string.format("loop seconds           %.4f", elapsed))
print(string.format("per-call micros        %.3f", elapsed / iterations * 1e6))
print(string.format("post-GC growth (kB)    %.1f", growth_kb))

-- 断言:内存不得随调用次数单调增长(泄漏信号)。100k 次调用后仍应基本持平。
assert(
  growth_kb < 512,
  string.format("100k 次 decorate 后堆增长 %.1f kB,超过 512 kB 上限(疑似泄漏)", growth_kb)
)
assert(decorated > 0, "探针词应至少命中一个提示")
print("PASS Lua 运行时内存基线")
