-- One native translation/learning provider plus bounded boundary planning.
-- The planner minimizes lexical-unit count (not a frequency estimate). Native
-- lookup supplies every edge; native Rime constructs and learns every candidate.
local M = { MAX_INPUT = 128, MAX_EDGE = 32, HEAD = 32, CANDIDATES_PER_QUERY = 2,
  MAX_LEARNING_UPDATES = 65536, MAX_LEARNING_ELEMENTS = 64, MAX_LEARNING_TEXT_BYTES = 256 }

local function learning_status(env, status)
  env.learning_status = status
  local context = env.engine and env.engine.context
  if context and context.set_property then
    context:set_property("xhup_flow_learning_status", status)
  end
end

local function bound_learning(env)
  local config = env.engine.schema.config
  -- Refuse by default before inspecting storage or configuration.
  env.native.memorize_callback = function() return false end
  if not env.native.user_dict then
    learning_status(env, "storage_unavailable")
    return false
  end
  local tick = env.native.user_dict.tick
  if type(tick) ~= "number" then
    learning_status(env, "bounded_api_unavailable")
    return false
  end
  if tick < 0 or tick ~= tick or tick == math.huge then
    learning_status(env, "storage_unverified")
    return false
  end
  local limit = config:get_int("flow/learning_max_updates") or M.MAX_LEARNING_UPDATES
  limit = math.max(0, math.min(limit, M.MAX_LEARNING_UPDATES))
  local phrase_length = config:get_int("flow/max_phrase_length") or 20
  local homographs = config:get_int("flow/max_homographs") or 1
  if not env.native.memorize or phrase_length > 20 or phrase_length < 1 or homographs ~= 1 then
    error("unsupported bounded native learning API/configuration")
  end
  env.native.memorize_callback = function(native, entry)
    local ok, updated = pcall(function()
      local elements, tick = entry:get(), native.user_dict.tick
      if type(tick) ~= "number" or tick < 0 or tick ~= tick or tick == math.huge then
        learning_status(env, "storage_unverified"); return false
      end
      if #elements < 1 then return false end
      if #elements > M.MAX_LEARNING_ELEMENTS then
        learning_status(env, "commit_limit"); return false
      end
      for _, element in ipairs(elements) do
        if #element.text > M.MAX_LEARNING_TEXT_BYTES then
          learning_status(env, "commit_limit"); return false
        end
      end
      -- Reserve direct updates plus every possible commit-history phrase in
      -- the configured <=20-element encoder window, with one homograph.
      local reserve = (#elements + 1) * (phrase_length + 1)
      if tick + reserve > limit then
        learning_status(env, "quota_exhausted"); return false
      end
      local result = native:memorize(entry)
      learning_status(env, result and "ready" or "storage_write_failed")
      return result
    end)
    if not ok then learning_status(env, "storage_unverified"); return false end
    return updated
  end
  learning_status(env, "ready")
  return true
end

function M.init(env)
  env.native, env.memory, env.native_error = nil, nil, nil
  if not Component or not Component.Translator then
    env.native_error = "native translator API unavailable"
    return
  end
  local config = env.engine.schema.config
  -- Probe callback/disconnect on a READ-ONLY object first. Distro backports can
  -- expose TableTranslator yet omit UserDictionary.tick, so version/date or
  -- constructor presence alone is not sufficient evidence.
  if config:get_bool("flow_readonly/enable_user_dict") ~= false then
    learning_status(env, "readonly_config_unverified")
    env.native_error = "read-only fallback configuration unavailable"
    return
  end
  local bounded_api = type(Component.TableTranslator) == "function"
  local constructor = bounded_api and Component.TableTranslator or Component.Translator
  local ok, readonly = pcall(constructor, env.engine, "", "table_translator@flow_readonly")
  if not ok or not readonly then
    env.native_error = "native translator construction failed"
    return
  end
  env.native = readonly
  learning_status(env, "bounded_api_unavailable")
  if config:get_bool("flow/enable_user_dict") == false then
    learning_status(env, "off")
  elseif bounded_api then
    local safe = pcall(function()
      assert(type(readonly.memorize) == "function" and type(readonly.disconnect) == "function")
      readonly.memorize_callback = function() return false end
      assert(type(readonly.memorize_callback) == "function")
    end)
    if safe then
      local made, writer = pcall(constructor, env.engine, "", "table_translator@flow")
      if made and writer then
        env.native = writer
        local verified, bounded = pcall(bound_learning, env)
        if verified and bounded then
          readonly:disconnect()
        else
          -- Initialization is synchronous: the deny callback is installed before
          -- any commit can be processed. No fallback path may retain this writer.
          writer:disconnect()
          env.native = readonly
          if not verified then learning_status(env, "bounded_api_unavailable") end
        end
      else
        learning_status(env, "storage_unavailable")
      end
    end
  end
  if not Memory then env.native_error = "dictionary lookup API unavailable"; return end
  local ready, memory = pcall(Memory, env.engine, env.engine.schema, "flow_lookup")
  if ready and memory then env.memory = memory
  else env.native_error = "dictionary lookup construction failed" end
end

function M.fini(env)
  if env.native then
    -- Disconnect before clearing the callback; never reopen an unbounded writer.
    if env.native.disconnect then pcall(env.native.disconnect, env.native) end
    pcall(function() env.native.memorize_callback = nil end)
  end
  env.memory, env.native = nil, nil
end

local function plans(input, env)
  local n = #input
  local best, previous = { [0] = 0 }, {}
  env.lookup_count = 0
  for start = 0, n - 2 do
    if best[start] then
      for length = 2, math.min(M.MAX_EDGE, n - start) do
        -- Return type differs across librime-lua versions. Always inspect the
        -- iterator; consume at most one entry, never a corpus-wide enumeration.
        env.lookup_count = env.lookup_count + 1
        env.memory:dict_lookup(input:sub(start + 1, start + length), false, 1)
        for _entry in env.memory:iter_dict() do
          local finish, cost = start + length, best[start] + 1
          if not best[finish] or cost < best[finish] then
            best[finish], previous[finish] = cost, start
          end
          break
        end
      end
    end
  end
  return previous
end

local function query_for(input, finish, previous)
  local reversed, cursor = {}, finish
  while cursor > 0 do
    local start = previous[cursor]
    if not start then return nil end
    reversed[#reversed + 1] = input:sub(start + 1, cursor)
    cursor = start
  end
  local ordered = {}
  for i = #reversed, 1, -1 do ordered[#ordered + 1] = reversed[i] end
  return table.concat(ordered, " ")
end

local function alternatives(input, segment, env)
  local result, previous, n = {}, plans(input, env), #input
  for tail = 0, 3 do
    local finish = n - tail
    -- Never replace standalone 2/3/4-key codes with a structural full-path plan.
    local query = finish >= 2 and (tail > 0 or n > 4)
      and query_for(input, finish, previous)
    if query then
      local virtual = Segment(segment.start, segment.start + #query)
      virtual.tags = segment.tags
      env.query_count = env.query_count + 1
      local translation = env.native:query(query, virtual)
      if translation then
        local visited = 0
        for candidate in translation:iter() do
          visited = visited + 1
          if candidate.start == segment.start and candidate._end == virtual._end then
            -- Keep the actual native Phrase/Sentence, NOT a ShadowCandidate.
            -- Native GetGenuineCandidate unwraps one shadow; quick_hint may add
            -- that one wrapper later. Nested shadows silently disable learning.
            candidate._end = segment.start + finish
            -- Preserve native type too: commit-history encoding recognizes
            -- native table/sentence types, not presentation-specific labels.
            if tail > 0 then candidate.comment = "待续:" .. input:sub(finish + 1) end
            candidate.preedit = query
            -- Explicit structural utility. Does not change lexical weights.
            candidate.quality = candidate.quality + (tail == 0 and 0.5 or -1)
            result[#result + 1] = candidate
          end
          if visited == M.CANDIDATES_PER_QUERY then break end
        end
      end
    end
  end
  return result
end

function M.func(input, segment, env)
  env.lookup_count, env.query_count = 0, 0
  if not env.native then return end -- static primary remains available
  local first = true
  local function emit(candidate)
    if first and env.learning_status and env.learning_status ~= "ready" and env.learning_status ~= "off" then
      candidate.comment = (candidate.comment or "") .. "〔学习暂停: " .. env.learning_status .. "〕"
    end
    first = false
    yield(candidate)
  end
  env.query_count = 1
  local ok, base = pcall(env.native.query, env.native, input, segment)
  if not ok then env.native_error = "native query failed"; return end
  local supported = #input >= 3 and #input <= M.MAX_INPUT and env.memory
    and not input:find("[^a-z]") and segment._end - segment.start == #input
  local extra = {}
  if supported then
    local ready, candidates = pcall(alternatives, input, segment, env)
    if ready then extra = candidates
    else env.native_error = "boundary lookup failed" end
  end
  if #extra == 0 then
    if base then for candidate in base:iter() do emit(candidate) end end
    return
  end
  local head, order, collected = {}, 0, 0
  local function add(candidate)
    order = order + 1
    head[#head + 1] = { candidate = candidate, order = order }
  end
  for _, candidate in ipairs(extra) do add(candidate) end
  local function emit_head()
    table.sort(head, function(a, b)
      -- A genuine complete interpretation must not be hidden behind a shorter
      -- phrase. Static primary remains outside this provider and unchanged.
      local full_a = a.candidate._end == segment._end and a.candidate.type ~= "completion"
      local full_b = b.candidate._end == segment._end and b.candidate.type ~= "completion"
      if #input > 4 and full_a ~= full_b then return full_a end
      if a.candidate.quality ~= b.candidate.quality then
        return a.candidate.quality > b.candidate.quality
      end
      return a.order < b.order
    end)
    for _, item in ipairs(head) do emit(item.candidate) end
    head = nil
  end
  if base then
    for candidate in base:iter() do
      if head then
        add(candidate)
        collected = collected + 1
        if collected == M.HEAD then emit_head() end
      else emit(candidate) end
    end
  end
  if head then emit_head() end
end
return M
