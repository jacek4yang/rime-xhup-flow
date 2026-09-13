-- tests/lua/test_native_lookup.lua
--
-- Native lookup 模块的纯 Lua 单元测试。
-- 不依赖 librime；验证模块接口与诊断逻辑。
-- 使用 Lua 5.4 运行: lua5.4 tests/lua/test_native_lookup.lua

-- 调整 package.path 以支持从仓库根目录运行
package.path = "rime/lua/?.lua;rime/lua/?/init.lua;" .. package.path

local native_lookup = require("xhup_flow.native_lookup")

local pass = 0
local fail = 0

local function check(label, condition)
  if condition then
    pass = pass + 1
  else
    fail = fail + 1
    io.stderr:write(string.format("FAIL: %s\n", label))
  end
end

-- ── 模块结构 ──
check("模块返回 table", type(native_lookup) == "table")
check("SOURCE_MAP 存在", type(native_lookup.SOURCE_MAP) == "table")
check("TYPE_SOURCE_MAP 存在", type(native_lookup.TYPE_SOURCE_MAP) == "table")
check("has_component_translator 是函数", type(native_lookup.has_component_translator) == "function")
check("check_prerequisites 是函数", type(native_lookup.check_prerequisites) == "function")
check("query_translator 是函数", type(native_lookup.query_translator) == "function")
check("collect_evidence 是函数", type(native_lookup.collect_evidence) == "function")
check("diagnose 是函数", type(native_lookup.diagnose) == "function")

-- ── SOURCE_MAP 与 CandidateSource Rust enum 对齐 ──
check("translator → static_table", native_lookup.SOURCE_MAP["translator"] == "static_table")
check("flow → flow_table", native_lookup.SOURCE_MAP["flow"] == "flow_table")
check("learn → user_learned", native_lookup.SOURCE_MAP["learn"] == "user_learned")

-- ── TYPE_SOURCE_MAP ──
check("table → table_entry", native_lookup.TYPE_SOURCE_MAP["table"] == "table_entry")
check("user_table → user_learned", native_lookup.TYPE_SOURCE_MAP["user_table"] == "user_learned")
check("sentence → sentence", native_lookup.TYPE_SOURCE_MAP["sentence"] == "sentence")

-- ── 无 librime 环境下的 prerequisites 检查 ──
-- 在纯 Lua 环境中，Component 全局不存在
local prereqs = native_lookup.check_prerequisites()
check("无 librime 环境 prerequisites 不 ok", prereqs.ok == false)
check("prerequisites 有错误信息", #prereqs.errors > 0)
check("错误信息提及 Component.Translator",
  string.find(prereqs.errors[1], "Component.Translator") ~= nil)

-- ── has_component_translator ──
check("无 librime 环境下 has_component_translator 返回 false",
  native_lookup.has_component_translator() == false)

-- ── diagnose ──
local diag = native_lookup.diagnose()
check("diagnose 返回 table", type(diag) == "table")
check("diagnose.component_translator_available 为 false", diag.component_translator_available == false)
check("diagnose.prerequisites.ok 为 false", diag.prerequisites.ok == false)
check("diagnose.translator_namespaces 有 3 项", #diag.translator_namespaces == 3)
check("translator_namespaces[1] = translator", diag.translator_namespaces[1] == "translator")
check("translator_namespaces[2] = flow", diag.translator_namespaces[2] == "flow")
check("translator_namespaces[3] = learn", diag.translator_namespaces[3] == "learn")

-- ── collect_evidence (无 query_fn 时仅返回查询规模统计) ──
local r = native_lookup.collect_evidence({}, "uijm", 20, 32)
check("空模式返回 fused 表", type(r.fused) == "table")
check("空模式 stats.raw_candidates=0", r.stats.raw_candidates == 0)
-- 4 键输入的 span 数: 4+3+2+1 = 10
check("空模式 spans_queried=10", r.stats.spans_queried == 10)

-- ── fuse_evidence: 融合语义(与 Rust LatticeFusionBuilder 对齐) ──
do
  local raw = {
    { start = 0, end_ = 4, text = "我们", source = "static_table", frequency = 500000, sourceRank = 1 },
    { start = 0, end_ = 4, text = "我们", source = "flow_table", frequency = 500000, sourceRank = 1 },
    { start = 0, end_ = 4, text = "我们", source = "user_learned", frequency = 10, sourceRank = 1 },
    { start = 0, end_ = 2, text = "我", source = "flow_table", frequency = 1200000, sourceRank = 1 },
    { start = 2, end_ = 4, text = "们", source = "flow_table", frequency = 300000, sourceRank = 1 },
  }
  local result = native_lookup.fuse_evidence(raw, native_lookup.DEFAULT_BOUNDS)
  check("三源融合: fused=3", result.stats.fused_candidates == 3)
  check("三源融合: edges=3", result.stats.total_edges == 3)
  local fused_wm
  for _, f in ipairs(result.fused) do
    if f.text == "我们" then fused_wm = f end
  end
  check("我们 融合候选存在", fused_wm ~= nil)
  check("我们 含 3 条证据", fused_wm and #fused_wm.evidence == 3)
  check("我们 max_frequency=500000", fused_wm and fused_wm.max_frequency == 500000)
  check("我们 来源顺序确定性", fused_wm and fused_wm.sources[1] == "static_table"
    and fused_wm.sources[2] == "flow_table" and fused_wm.sources[3] == "user_learned")
end

-- ── fuse_evidence: 空文本拒绝 ──
do
  local result = native_lookup.fuse_evidence({
    { start = 0, end_ = 1, text = "", source = "static_table", frequency = 1, sourceRank = 1 },
  }, native_lookup.DEFAULT_BOUNDS)
  check("空文本不产生融合候选", result.stats.fused_candidates == 0)
end

-- ── fuse_evidence: span 超过 max_key_span 被拒绝 ──
do
  local bounds = { max_key_span = 2, max_candidates_per_span = 32,
    max_outgoing_per_position = 64, max_total_edges = 2048 }
  local result = native_lookup.fuse_evidence({
    { start = 0, end_ = 4, text = "超长", source = "static_table", frequency = 1, sourceRank = 1 },
    { start = 0, end_ = 2, text = "短码", source = "static_table", frequency = 1, sourceRank = 1 },
  }, bounds)
  check("超长 span 被过滤", result.stats.fused_candidates == 1)
end

-- ── 模拟 Component.Translator 环境 ──
-- 注入全局 Component table 验证 has_component_translator
Component = { Translator = function() end }
check("注入 Component 后 has_component_translator 返回 true",
  native_lookup.has_component_translator() == true)
local prereqs2 = native_lookup.check_prerequisites()
check("注入 Component 后 prerequisites ok", prereqs2.ok == true)
Component = nil  -- 清理

-- ── 汇总 ──
print(string.format("\nnative_lookup 单测: %d 通过, %d 失败", pass, fail))
if fail > 0 then
  os.exit(1)
end
