-- Stub contracts only. Real session/privacy checks also run through librime.
local m = dofile("rime/lua/xhup_flow/user_memory.lua")
local shim = dofile("rime/lua/xhup_flow/joint_decoder.lua")
local passed = 0
local function check(value, label) assert(value, label); passed = passed + 1 end
local function notifier()
  local n = { callbacks = {} }
  function n:connect(fn)
    local active = true
    self.callbacks[#self.callbacks + 1] = function(...) if active then fn(...) end end
    return { disconnect = function() active = false end }
  end
  function n:emit() for _, fn in ipairs(self.callbacks) do fn() end end
  return n
end
local function environment()
  local c = { options = {}, properties = {}, reads = 0,
    commit_notifier = notifier(), option_update_notifier = notifier() }
  function c:get_option(name) return self.options[name] or false end
  function c:set_option(name, value) self.options[name] = value; self.option_update_notifier:emit() end
  function c:get_property(name) return self.properties[name] or "" end
  function c:set_property(name, value) self.properties[name] = value end
  function c:get_commit_text() self.reads = self.reads + 1; return self.text end
  function c:commit(text) self.text = text; self.commit_notifier:emit() end
  return { engine = { context = c } }, c
end
-- Any attempted persistent access during init/commit/disable/fini fails the test.
local open, remove, rename = io.open, os.remove, os.rename
io.open = function() error("session memory must not read or write files") end
os.remove, os.rename = io.open, io.open
local a, ac = environment()
m.init(a)
ac:commit("private text while off")
check(ac.reads == 0, "off never reads committed text")
check(m.get_state(ac).last_commit == nil, "off has no last commit")
ac:set_option("context_ranker", true)
ac:commit("时间")
check(m.get_state(ac).last_commit == "时间", "ranker consent enables session last commit")
check(next(m.get_state(ac).counts) == nil, "ranker alone has no count history")
local b, bc = environment()
m.init(b)
check(m.get_state(bc).last_commit == nil, "second engine never inherits first context")
check(m.get_state(ac).last_commit == "时间", "second init never resets first context")
ac:set_option("user_memory", true)
ac:commit("时间")
check(m.get_state(ac).counts["时间"] == 1, "session count")
for i = 1, m.MAX_ENTRIES + 100 do ac:commit("item" .. i) end
local state = m.get_state(ac)
check(state.size == m.MAX_ENTRIES, "bounded cardinality")
check(state.counts["时间"] == nil and state.counts.item1 == nil, "FIFO eviction")
local count = 0
for _ in pairs(state.counts) do count = count + 1 end
check(count == m.MAX_ENTRIES, "actual count map bounded")
local last = "item" .. (m.MAX_ENTRIES + 100)
state.counts[last] = m.MAX_COUNT
ac:commit(last)
check(state.counts[last] == m.MAX_COUNT, "count saturates")
ac:commit(string.rep("x", m.MAX_TEXT_BYTES + 1))
check(state.last_commit == nil and state.size == m.MAX_ENTRIES, "long commits not retained")
ac:set_option("user_memory", false)
check(state.size == 0 and next(state.counts) == nil, "disable erases counts immediately")
ac:commit("ranker-only")
ac:set_option("context_ranker", false)
check(state.last_commit == nil, "all off erases last commit immediately")
local reads = ac.reads
ac:commit("not observed")
check(ac.reads == reads, "disabled after enable does not read text")
-- Distinct Lua wrapper pointing at the same native property store.
local wrapper = { get_property = function(_, key) return ac:get_property(key) end,
  get_option = function(_, key) return ac:get_option(key) end,
  set_property = function(_, key, value) ac:set_property(key, value) end }
check(m.get_state(wrapper) == state, "native session property survives userdata wrappers")
m.fini(a)
check(m.get_state(ac) == nil, "fini erases session state")
ac:commit("after fini")
check(ac.reads == reads, "fini disconnects observer")
m.fini(b)
check(shim.available == false, "legacy joint decoder explicitly unavailable")
shim.init(a)
check(ac:get_property("xhup_flow_joint_decoder_status") == "unavailable", "shim diagnostic")
local visited, emitted = 0, 0
yield = function(candidate)
  emitted = emitted + 1
  check(candidate == emitted and visited == emitted, "shim streams without buffering or reorder")
end
shim.func({iter = function() return function()
  visited = visited + 1
  if visited <= 1000 then return visited end
end end}, a)
check(emitted == 1000, "shim preserves full stream")
io.open, os.remove, os.rename = open, remove, rename
print("PASS session privacy/bounds/isolation + compatibility shim: " .. passed .. " stub assertions")
