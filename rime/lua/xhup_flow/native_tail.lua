-- One native learning owner; read-only lexical providers share its syllable IDs.
-- Opt-in joint search preserves overlapping key/text paths and native Phrase
-- identity. Unsupported APIs/configuration retain the bounded legacy planner.
-- Static exact-code priority is enforced by the unchanged primary translator.
local M = { MAX_INPUT = 128, MAX_EDGE = 32, HEAD = 5, CANDIDATES_PER_QUERY = 2,
  MAX_LEARNING_UPDATES = 65536, MAX_LEARNING_ELEMENTS = 64, MAX_ENCODER_PATHS = 32,
  MAX_LEARNING_TEXT_BYTES = 256, MAX_LEARNING_CODE_BYTES = 4096,
  MIN_JOINT_MARGIN = 2048 }

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
  if not env.native.memorize or phrase_length > 20 or phrase_length < 1 or homographs ~= 1
    or config:get_bool("flow/encode_commit_history") ~= false then
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
      local text_bytes, code_bytes, code_units = 0, 0, 0
      for _, element in ipairs(elements) do
        text_bytes = text_bytes + #element.text
        if text_bytes > M.MAX_LEARNING_TEXT_BYTES then
          learning_status(env, "commit_limit"); return false
        end
        local code, custom = element.code:print(), element.custom_code or ""
        code_bytes = code_bytes + #code + #custom
        if code_bytes > M.MAX_LEARNING_CODE_BYTES then
          learning_status(env, "commit_limit"); return false
        end
        if code:find("[^%d,]") or code:find(",,", 1, true)
          or code:sub(1, 1) == "," or code:sub(-1) == "," then
          learning_status(env, "storage_unverified"); return false
        end
        local numeric, spelled = 0, 0
        for _ in code:gmatch("%d+") do numeric = numeric + 1 end
        for _ in custom:gmatch("%S+") do spelled = spelled + 1 end
        local units = math.max(numeric, spelled)
        code_units = code_units + units
        if units == 0 or code_units > M.MAX_LEARNING_ELEMENTS then
          learning_status(env, "commit_limit"); return false
        end
      end
      -- Native TableEncoder's DFS visits at most 32 complete paths, regardless
      -- of max_homographs/max_phrase_length. History's zero-tick writes are off.
      local reserve = #elements + (#elements > 1 and M.MAX_ENCODER_PATHS or 0)
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
  env.native, env.memory, env.native_error, env.edge_lengths = nil, nil, nil, nil
  env.joint_ready, env.aliases, env.reverse = nil, nil, nil
  env.lexical_score, env.transition_score = nil, nil
  env.lexical_cache = nil
  if not Component or not Component.Translator then
    env.native_error = "native translator API unavailable"
    return
  end
  local config = env.engine.schema.config
  env.ranking_policy = (config.get_string and config:get_string("flow/ranking_policy"))
    or "native-prefix-v1"
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
  if config:get_bool("flow_lookup/enable_user_dict") ~= false then
    env.native_error = "read-only lookup configuration unavailable"; return
  end
  local ready, memory = pcall(Memory, env.engine, env.engine.schema, "flow_lookup")
  if ready and memory then
    env.memory = memory
    -- Only the generated, unmodified dictionary/prism profile may prune edges.
    -- Old schemas/APIs and custom dictionaries/packs retain exhaustive lookup.
    local indexed, lengths = pcall(function()
      if config:get_bool("flow_lookup/enable_compiled_index") ~= true
        or config:get_string("flow_lookup/dictionary") ~= "xhup_flow_user"
        or config:get_list("flow_lookup/packs") then return nil end
      local prism = config:get_string("flow_lookup/prism")
      if prism and prism ~= "xhup_flow_user" then return nil end
      local identity = config:get_string("flow_lookup/compiled_index_id")
      if not identity then return nil end
      local index = require("xhup_flow.data.native_edges")
      if index.identity == identity then return index end
    end)
    if indexed then env.edge_lengths = lengths end
  else env.native_error = "dictionary lookup construction failed" end
end

function M.fini(env)
  if env.native then
    -- Disconnect before clearing the callback; never reopen an unbounded writer.
    if env.native.disconnect then pcall(env.native.disconnect, env.native) end
    pcall(function() env.native.memorize_callback = nil end)
  end
  env.memory, env.native, env.edge_lengths = nil, nil, nil
  env.aliases, env.reverse, env.lexical_score, env.transition_score = nil, nil, nil, nil
  env.joint_ready = nil
  env.lexical_cache = nil
  env.ranking_policy = nil
end

local function plans(input, env)
  local n = #input
  local best, previous = { [0] = 0 }, {}
  env.lookup_count = 0
  for start = 0, n - 2 do
    if best[start] then
      local limit = math.min(M.MAX_EDGE, n - start)
      local lengths = env.edge_lengths and env.edge_lengths[input:sub(start + 1, start + 2)]
      local count = env.edge_lengths and (lengths and #lengths or 0) or (limit - 1)
      for index = 1, count do
        local length = lengths and lengths:byte(index) or (index + 1)
        if length > limit then break end
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
            -- Native quality is left unchanged. Structural boundary evidence
            -- supplies alternatives, not a calibrated lexical preference.
            result[#result + 1] = candidate
          end
          if visited == M.CANDIDATES_PER_QUERY then break end
        end
      end
    end
  end
  return result
end

local function joint_status(env, status)
  env.engine.context:set_property("xhup_flow_joint_decoder_status", status)
end

local function prepare_joint(env)
  if env.joint_ready ~= nil then return env.joint_ready end
  env.joint_ready = false
  local ready = pcall(function()
    local config = env.engine.schema.config
    assert(env.edge_lengths and env.memory and env.memory.user_dict == nil and env.memory.dict.loaded)
    assert(config:get_bool("flow_aliases/enable_user_dict") == false)
    assert(config:get_string("flow_aliases/dictionary") == "xhup_flow")
    assert(not config:get_list("flow_aliases/packs"))
    local prism = config:get_string("flow_aliases/prism")
    assert(not prism or prism == "xhup_flow")
    assert(type(DictEntry) == "function" and type(Phrase) == "function"
      and type(ReverseDb) == "function")
    assert(type(env.memory.lang_name) == "string" and env.memory.lang_name ~= ""
      and env.native.lang_name == env.memory.lang_name)
    assert(type(env.memory.dict.name) == "string"
      and env.native.dict.name == env.memory.dict.name and env.native.dict.loaded)
    local joint = require("xhup_flow.joint_decoder")
    local lexical, transition = joint.scorer(nil, config:get_string("flow/joint_model_id"))
    local aliases = Memory(env.engine, env.engine.schema, "flow_aliases")
    assert(aliases.user_dict == nil and aliases.dict.loaded)
    env.aliases = aliases
    env.lexical_cache = joint.readonly_cache(env.memory, aliases)
    env.reverse = ReverseDb("build/xhup_flow_user.reverse.bin")
    env.lexical_score, env.transition_score = lexical, transition
  end)
  if ready then
    env.joint_ready = true
    joint_status(env, "ready")
  else
    env.aliases, env.reverse = nil, nil
    env.lexical_cache = nil
    joint_status(env, "configuration_or_api_unavailable")
  end
  return env.joint_ready
end

local function joint_candidates(input, segment, env)
  local joint = require("xhup_flow.joint_decoder")
  local provider, stats = joint.lexicon(input, env.memory, env.aliases, env.reverse,
    env.edge_lengths, env.lexical_score, env.lexical_cache)
  local result = joint.search(input, provider, env.transition_score)
  local full, pending, seen, paths = {}, {}, {}, {}
  local best_full_score, next_full_score
  local personalization = true
  env.user_lookup_count = 0
  for _, node in ipairs(result.paths) do
    local path = joint.describe(input, node, segment.start, false)
    path.user = 0
    if personalization and env.native.user_dict then
      env.user_lookup_count = env.user_lookup_count + 1
      local ok, reward = pcall(joint.user_reward, path, env.memory, env.native.user_dict)
      if ok then path.user = reward
      else personalization = false end
    end
    path.score = path.score + path.user
    paths[#paths + 1] = path
  end
  if not personalization then
    for _, path in ipairs(paths) do path.score, path.user = path.score - path.user, 0 end
  end
  table.sort(paths, function(a, b)
    if a.finish ~= b.finish then return a.finish > b.finish end
    if a.score ~= b.score then return a.score > b.score end
    if a.node.count ~= b.node.count then return a.node.count < b.node.count end
    return a.tie < b.tie
  end)
  for rank, path in ipairs(paths) do
    local key = path.finish .. "\0" .. path.text
    if not seen[key] then
      seen[key], path.rank = true, rank - 1
      local candidate = joint.candidate(path, env.memory, env.native.lang_name, 1000 - rank)
      local target = path.finish == segment._end and full or pending
      target[#target + 1] = candidate
      if target == full then
        if not best_full_score then best_full_score = path.score
        elseif not next_full_score then next_full_score = path.score end
      end
    end
  end
  env.lookup_count = stats.lookups
  joint_status(env, not personalization and "personalization_unavailable"
    or (stats.normalization_budget > 0 and "bounded" or "ready"))
  local confident = best_full_score ~= nil and result.truncated_edges == 0
    and (next_full_score == nil or best_full_score - next_full_score >= M.MIN_JOINT_MARGIN)
  return full, pending, confident
end

-- Exact dictionary and learned phrases retain their native first-choice identity.
-- Only automatic sentences may yield to a distinct-text confidence margin in the
-- joint model; native quality is never compared numerically with joint scores.
local function complete_exact(candidate, segment)
  if candidate.start ~= segment.start or candidate._end ~= segment._end then return false end
  local ok, complete = pcall(function()
    local phrase = candidate:to_phrase()
    local entry = phrase and phrase.entry
    return entry and entry.remaining_code_length == 0
  end)
  return ok and complete
end

function M.func(input, segment, env)
  env.lookup_count, env.query_count, env.user_lookup_count = 0, 0, 0
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
  local supported = #input >= 2 and #input <= M.MAX_INPUT and env.memory
    and not input:find("[^a-z]") and segment._end - segment.start == #input
  local extra, full, used_joint, confident = {}, {}, false, false
  local context = env.engine.context
  if supported and context and context.get_option and context:get_option("joint_decoder")
    and prepare_joint(env) then
    local ready, complete, pending, certain = pcall(joint_candidates, input, segment, env)
    if ready then
      full, extra, used_joint, confident = complete, pending, true, certain
    else
      joint_status(env, "query_failed")
    end
  end
  if supported and not used_joint and #input >= 3 then
    local ready, candidates = pcall(alternatives, input, segment, env)
    if ready then extra = candidates
    else env.native_error = "boundary lookup failed" end
  end
  if #full == 0 and #extra == 0 then
    if base then for candidate in base:iter() do emit(candidate) end end
    return
  end
  local full_appended = false
  local function append_full(start)
    if full_appended then return end
    for index = start or 1, #full do emit(full[index]) end
    full_appended = true
  end
  local collected, appended = 0, false
  local function append_alternatives()
    if appended then return end
    for _, candidate in ipairs(extra) do emit(candidate) end
    appended = true
  end
  if base then
    for candidate in base:iter() do
      local protect = env.ranking_policy ~= "joint-confidence-v1"
        or candidate.type ~= "sentence" or not confident
        or (full[1] and full[1].text == candidate.text)
      if collected == 0 and used_joint and complete_exact(candidate, segment) then
        if protect then
          emit(candidate)
          append_full()
        else
          -- Promote only the confident winner. Keep the native alternative
          -- visible, rather than burying it behind the entire joint beam.
          emit(full[1])
          emit(candidate)
          append_full(2)
        end
      else
        append_full()
        emit(candidate)
      end
      collected = collected + 1
      if collected == M.HEAD then append_alternatives() end
    end
  end
  append_full()
  append_alternatives()
end
return M
