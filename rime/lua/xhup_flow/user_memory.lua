-- Bounded, consent-gated SESSION evidence. Native Rime is the only durable
-- runtime learning store. Historical TSV files are neither read nor modified.
-- user_memory is retained as a configuration key, now explicitly session-only.
local M = { MAX_ENTRIES = 512, MAX_TEXT_BYTES = 256, MAX_COUNT = 65535 }
local states, serial = {}, 0
local ID = "xhup_flow_session_memory_id"

local function flags(context)
  return context:get_option("user_memory"), context:get_option("context_ranker")
end

local function clear_counts(state)
  state.counts, state.queue, state.size, state.next = {}, {}, 0, 1
end

local function synchronize(context, state)
  local memory, ranker = flags(context)
  if not memory then clear_counts(state) end
  if not memory and not ranker then state.last_commit = nil end
  context:set_property("xhup_flow_session_memory_entries", tostring(state.size))
  return memory, ranker
end

function M.get_state(context)
  local state = states[context:get_property(ID)]
  if state then synchronize(context, state) end
  return state
end

function M.init(env)
  local context = env.engine.context
  -- Context properties identify the native session across Lua userdata wrappers.
  local prior = context:get_property(ID)
  if prior and prior ~= "" then states[prior] = nil end
  serial = serial + 1
  env.memory_id = tostring(serial)
  local state = { last_commit = nil }
  clear_counts(state)
  states[env.memory_id] = state
  context:set_property(ID, env.memory_id)
  synchronize(context, state)
  env.commit_connection = context.commit_notifier:connect(function()
    local memory, ranker = synchronize(context, state)
    if not memory and not ranker then return end -- Do not even read committed text.
    local text = context:get_commit_text()
    if type(text) ~= "string" or text == "" or #text > M.MAX_TEXT_BYTES then
      state.last_commit = nil
      return
    end
    state.last_commit = text
    if not memory then return end
    if state.counts[text] then
      state.counts[text] = math.min(M.MAX_COUNT, state.counts[text] + 1)
    else
      -- Fixed-size FIFO ring: bounded work and retention, no unbounded sequence.
      local evicted = state.queue[state.next]
      if evicted then state.counts[evicted] = nil
      else state.size = state.size + 1 end
      state.queue[state.next], state.counts[text] = text, 1
      state.next = state.next % M.MAX_ENTRIES + 1
    end
    context:set_property("xhup_flow_session_memory_entries", tostring(state.size))
  end)
  env.option_connection = context.option_update_notifier:connect(function()
    synchronize(context, state)
  end)
end

function M.fini(env)
  if env.commit_connection then env.commit_connection:disconnect() end
  if env.option_connection then env.option_connection:disconnect() end
  if env.memory_id then states[env.memory_id] = nil end
  local context = env.engine.context
  if context:get_property(ID) == env.memory_id then
    context:set_property(ID, "")
    context:set_property("xhup_flow_session_memory_entries", "0")
  end
  env.commit_connection, env.option_connection, env.memory_id = nil, nil, nil
end

function M.func(translation, _env)
  for candidate in translation:iter() do yield(candidate) end
end

return M
