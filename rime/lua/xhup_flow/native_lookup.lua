-- XHUP Flow native lookup 模块: 生产级原生 translator 候选查询与证据融合。
--
-- 本模块通过 librime-lua 的 Component.Translator 接口查询已部署的
-- 原生 table_translator 实例,获取与给定输入对应的候选列表,
-- 为 Joint Key/Text Lattice 构建提供运行时候选证据。
--
-- 当前版本: 影子模式(shadow mode)——只采集诊断信息,不改变
-- 任何用户可见的候选输出。
--
-- 架构约束:
-- - 查询只读,不写 userdb
-- - 不持有完整词典数据(通过 native translator 查询)
-- - 不改变候选次序(shadow mode)
-- - 不暴露用户文本到日志(统计与诊断只含计数)

local M = {}

-- translator namespace → CandidateSource 映射
-- (与 Rust CandidateSource enum / fixture source 字段对齐)
M.SOURCE_MAP = {
  ["translator"] = "static_table",
  ["flow"]       = "flow_table",
  ["learn"]      = "user_learned",
}

-- Candidate type → CandidateSource 映射
-- (cand.type 是 librime 的候选类型标识;namespace 判定优先于 type)
M.TYPE_SOURCE_MAP = {
  ["table"]      = "table_entry",
  ["user_table"] = "user_learned",
  ["sentence"]   = "sentence",
}

-- 融合边界默认值(与 Rust LatticeBounds::default() 一致)
M.DEFAULT_BOUNDS = {
  max_key_span = 20,
  max_candidates_per_span = 32,
  max_outgoing_per_position = 64,
  max_total_edges = 2048,
}

--- 探测 Component.Translator 是否可用。
-- @return boolean
function M.has_component_translator()
  return type(Component) == "table" and type(Component.Translator) == "function"
end

--- 检查 native lookup 的运行时前置条件。
-- @return table { ok: boolean, errors: table }
function M.check_prerequisites()
  local result = { ok = true, errors = {} }

  if not M.has_component_translator() then
    result.ok = false
    table.insert(result.errors, "Component.Translator 不可用(需要 librime-lua)")
  end

  return result
end

--- 从给定 translator 实例查询候选列表(一次 query)。
-- @param translator  通过 Component.Translator 创建的 translator 实例
-- @param segment     Segment 对象(包含输入范围)
-- @param limit       最大返回候选数
-- @return table      候选列表 { {text, type, quality, comment}, ... }
function M.query_translator(translator, segment, limit)
  limit = limit or 32
  local candidates = {}

  local ok, translation = pcall(function()
    return translator:query(segment, {})
  end)

  if not ok or not translation then
    return candidates
  end

  local count = 0
  while not translation:exhausted() and count < limit do
    local cand = translation:peek()
    if not cand then break end

    table.insert(candidates, {
      text = cand.text,
      type = cand.type,
      quality = cand.quality or 0,
      comment = cand.comment or "",
    })

    translation:next()
    count = count + 1
  end

  return candidates
end

--- 证据融合: 对同一 (span, text) 对合并多来源证据为单一融合候选。
--
-- 输入 raw_candidates: { {start, end, text, source, frequency, sourceRank}, ... }
-- (字段名与 data/fixtures/joint-lattice-v1.json 的 camelCase 对应,
--  此处使用 snake_case Lua 习惯)
--
-- 融合规则(与 Rust LatticeFusionBuilder 语义一致):
-- 1. FusionKey = (span, text),确定性(BTreeMap 语义用排序数组模拟)
-- 2. 证据列表按 source 枚举顺序排序
-- 3. max_frequency 取所有证据最大值
-- 4. 输出按 (span.start, span.len, source, text) 确定性排序
--
-- @param raw_candidates table
-- @param bounds table 可选 {max_key_span, max_candidates_per_span,
--                     max_outgoing_per_position, max_total_edges}
-- @return table { fused: table, stats: table }
--   fused: { {start, end, text, sources, max_frequency, evidence}, ... }
--   stats: { raw_candidates, fused_candidates, total_edges,
--            truncated_candidates, max_fanout, edges_per_position }
function M.fuse_evidence(raw_candidates, bounds)
  bounds = bounds or M.DEFAULT_BOUNDS
  local stats = {
    raw_candidates = #raw_candidates,
    fused_candidates = 0,
    total_edges = 0,
    truncated_candidates = 0,
    max_fanout = 0,
    edges_per_position = {},
  }

  -- 1. 按 FusionKey 分组(span, text);用 span 编码字符串作键保证确定性
  local groups = {}
  local group_list = {}
  for _, raw in ipairs(raw_candidates) do
    local span_len = raw.end_ - raw.start
    if span_len <= bounds.max_key_span and raw.text ~= "" then
      local key = string.format("%d:%d:%s", raw.start, raw.end_, raw.text)
      if groups[key] == nil then
        groups[key] = { start = raw.start, end_ = raw.end_, text = raw.text, evidence = {} }
        table.insert(group_list, groups[key])
      end
      table.insert(groups[key].evidence, {
        source = raw.source,
        quality = raw.quality or 0,
        source_rank = raw.sourceRank or 0,
        frequency = raw.frequency or 0,
      })
    end
  end

  -- 2. 每组融合: 证据按 source 排序(确定性),取最大频率
  local source_order = {
    static_table = 1, flow_table = 2, user_learned = 3,
    sentence_composition = 4, compiler_metadata = 5,
  }
  for _, group in ipairs(group_list) do
    table.sort(group.evidence, function(a, b)
      if a.source ~= b.source then
        return (source_order[a.source] or 99) < (source_order[b.source] or 99)
      end
      return tostring(a.quality) < tostring(b.quality)
    end)

    local max_freq = 0
    for _, ev in ipairs(group.evidence) do
      if (ev.frequency or 0) > max_freq then
        max_freq = ev.frequency or 0
      end
    end

    group.max_frequency = max_freq
    group.sources = {}
    for _, ev in ipairs(group.evidence) do
      table.insert(group.sources, ev.source)
    end

    stats.fused_candidates = stats.fused_candidates + 1
  end

  -- 3. 聚合到 span: span → fused candidate 列表(按 max_frequency 降序,
  --    平手取 text 字典序 —— 与 Rust build() 的截断优先级对齐)
  local by_span = {}
  local span_list = {}
  for _, group in ipairs(group_list) do
    local skey = string.format("%d:%d", group.start, group.end_)
    if by_span[skey] == nil then
      by_span[skey] = { start = group.start, end_ = group.end_, candidates = {} }
      table.insert(span_list, by_span[skey])
    end
    table.insert(by_span[skey].candidates, group)
  end

  -- 4. 有界截断: 每 span 候选上限;单字符(fallback)优先保留;
  --    每位置出边上限;总边数上限。
  local fallback_bound = {}
  local function is_fallback(text)
    if fallback_bound[text] == nil then
      local _, count = text:gsub("[\1-\127\192-\251]", "")
      fallback_bound[text] = count == 1
    end
    return fallback_bound[text]
  end

  table.sort(span_list, function(a, b)
    if a.start ~= b.start then return a.start < b.start end
    return (a.end_ - a.start) < (b.end_ - b.start)
  end)

  for _, span_group in ipairs(span_list) do
    local cands = span_group.candidates
    if #cands > bounds.max_candidates_per_span then
      stats.truncated_candidates = stats.truncated_candidates
        + (#cands - bounds.max_candidates_per_span)
      -- fallback 原语优先保留,其余按 max_frequency 降序,text 升序平手
      table.sort(cands, function(a, b)
        local fa, fb = is_fallback(a.text), is_fallback(b.text)
        if fa ~= fb then return fa end
        if a.max_frequency ~= b.max_frequency then
          return a.max_frequency > b.max_frequency
        end
        return a.text < b.text
      end)
      -- 保留前 N 后按确定性顺序放回(高频在前)
      cands = {}
      for i = 1, bounds.max_candidates_per_span do
        cands[i] = span_group.candidates[i]
      end
      span_group.candidates = cands
    end

    -- 每位置出边上限
    local pos = span_group.start
    stats.edges_per_position[pos] = stats.edges_per_position[pos] or 0
    local allowed = math.max(0, bounds.max_outgoing_per_position - stats.edges_per_position[pos])
    if #cands > allowed then
      stats.truncated_candidates = stats.truncated_candidates + (#cands - allowed)
      cands = {}
      for i = 1, math.max(0, allowed) do
        cands[i] = span_group.candidates[i]
      end
      span_group.candidates = cands
    end

    -- 总边数上限
    local remaining = bounds.max_total_edges - stats.total_edges
    if #cands > remaining then
      stats.truncated_candidates = stats.truncated_candidates + (#cands - remaining)
      cands = {}
      for i = 1, math.max(0, remaining) do
        cands[i] = span_group.candidates[i]
      end
      span_group.candidates = cands
    end

    stats.edges_per_position[pos] = stats.edges_per_position[pos] + #cands
    stats.total_edges = stats.total_edges + #cands
    if #cands > stats.max_fanout then
      stats.max_fanout = #cands
    end
  end

  -- 5. 组装融合输出(确定性顺序)
  local fused = {}
  for _, span_group in ipairs(span_list) do
    for _, group in ipairs(span_group.candidates) do
      table.insert(fused, {
        start = group.start,
        end_ = group.end_,
        text = group.text,
        sources = group.sources,
        max_frequency = group.max_frequency,
        evidence = group.evidence,
      })
    end
  end

  return { fused = fused, stats = stats }
end

--- 证据收集: 从多个 translator 收集候选并融合。
--
-- @param translators table  { namespace_name = translator_instance, ... }
-- @param input       string 原始输入按键流(仅用于确定 span 数量;
--                    生产实现需构造 Segment 对象)
-- @param max_span    number 最大查询 span 长度
-- @param limit       number 每个查询的最大候选数
-- @return table      { fused: table, stats: table, spans_queried: number }
--
-- 纯逻辑版本: 接受 query_fn(span_start, span_len) 返回候选列表的回调,
-- 便于在无 librime 环境下以 mock translator 单测。
-- query_fn 返回 { {text, type, quality}, ... }
function M.collect_evidence(translators, input, max_span, limit, query_fn)
  max_span = max_span or 20
  limit = limit or 32

  local stats = {
    raw_candidates = 0,
    spans_queried = 0,
  }
  local raw = {}

  for start = 0, #input - 1 do
    for len = 1, math.min(#input - start, max_span) do
      stats.spans_queried = stats.spans_queried + 1
      if query_fn then
        local cands = query_fn(start, len)
        for _, c in ipairs(cands or {}) do
          local source = M.TYPE_SOURCE_MAP[c.type] or "table_entry"
          table.insert(raw, {
            start = start,
            end_ = start + len,
            text = c.text,
            source = c.source or source,
            frequency = c.frequency or 0,
            sourceRank = c.sourceRank or 1,
            quality = c.quality or 0,
          })
          stats.raw_candidates = stats.raw_candidates + 1
        end
      end
    end
  end

  if not query_fn then
    -- 兼容旧接口: 无 query_fn 时只返回查询规模统计(不执行真实查询)
    return { fused = {}, stats = { raw_candidates = 0, spans_queried = stats.spans_queried } }
  end

  local result = M.fuse_evidence(raw, M.DEFAULT_BOUNDS)
  result.stats.spans_queried = stats.spans_queried
  return result
end

--- 诊断报告: 生成 native lookup 能力的结构化诊断。
-- @return table
function M.diagnose()
  local report = {
    component_translator_available = M.has_component_translator(),
    prerequisites = M.check_prerequisites(),
    translator_namespaces = { "translator", "flow", "learn" },
  }

  return report
end

return M
