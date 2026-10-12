dofile("tests/lua/runtime_path.lua")
local policy = require("xhup_flow.native_tail")
local package_root = assert(arg[1], "pass the independently verified generated package")
local index = dofile(package_root .. "/lua/xhup_flow/data/native_edges.lua")
package.loaded["xhup_flow.data.native_edges"] = index
local checks = 0
local function check(ok, why) assert(ok, why); checks = checks + 1 end

-- Independently consume the actual native dictionary format, not the generator's
-- internal rows. Every spelling the old planner could query must remain possible.
for _, name in ipairs({"xhup_flow_flow.dict.yaml", "xhup_flow_learn.dict.yaml"}) do
  local file = assert(io.open(package_root .. "/" .. name, "r"))
  local body = false
  for line in file:lines() do
    if body then
      local code = assert(line:match("^[^\t]+\t([a-z]+)\t"), "invalid native dictionary row")
      if #code >= 2 and #code <= policy.MAX_EDGE then
        local lengths = index[code:sub(1, 2)]
        check(lengths and lengths:find(string.char(#code), 1, true),
          "compiled projection cannot remove a native spelling")
      end
    elseif line == "..." then body = true end
  end
  file:close()
end

Segment = function(start, finish) return { start = start, _end = finish } end
local function fixture(settings, exact)
  local queries, lookups, current = {}, 0, ""
  local config = {
    get_bool = function(_, name)
      if name == "flow_lookup/enable_compiled_index" then return settings.enabled end
      return false -- learning off, including the read-only capability contract
    end,
    get_string = function(_, name)
      if name == "flow_lookup/dictionary" then return settings.dictionary end
      if name == "flow_lookup/prism" then return settings.prism end
      if name == "flow_lookup/compiled_index_id" then
        if settings.missing_identity then return nil end
        return settings.identity or index.identity
      end
    end,
    get_list = function() return settings.packs end,
  }
  if settings.old_api then config.get_list = nil end
  local env = { engine = { schema = { config = config } } }
  local native = { query = function(_, query)
    queries[#queries + 1] = query
    return nil -- observe boundary requests; native candidate semantics have their own suite
  end }
  Component = { Translator = function() return native end }
  Component.TableTranslator = Component.Translator
  Memory = function() return {
    dict_lookup = function(_, code) current = code; lookups = lookups + 1 end,
    iter_dict = function()
      local done = false
      return function()
        if not done and exact[current] then done = true; return { text = "edge" } end
      end
    end,
  } end
  yield = function() error("this fixture has no native candidates") end
  policy.init(env)
  return env, function(input, start)
    queries, lookups = {}, 0
    policy.func(input, { start = start, _end = start + #input, tags = {} }, env)
    return table.concat(queries, "|"), lookups
  end
end

local exact = { ni = true, hc = true, hcnz = true, nihc = true, qu = true, jbz = true }
local env, indexed = fixture({ enabled = true, dictionary = "xhup_flow_learn" }, exact)
local _, exhaustive = fixture({ enabled = false, dictionary = "xhup_flow_learn" }, exact)
-- Reuse each session across appends, backspaces and edits; the immutable index
-- must never retain per-input negative results or alter deterministic boundaries.
for _, input in ipairs({"jbzqu", "nihcnzqu", "nihcnznihcnzqu", string.rep("nihcnzqu", 16)}) do
  for length = 1, #input do
    local raw = input:sub(1, length)
    local expected = exhaustive(raw, 7)
    local actual = indexed(raw, 7)
    check(actual == expected, "every growing prefix retains exhaustive boundary queries")
  end
  for length = #input, 1, -1 do
    check(indexed(input:sub(1, length), 0) == exhaustive(input:sub(1, length), 0),
      "backspace and cursor-origin changes retain boundaries")
  end
end
local queries, reduced = indexed(string.rep("nihcnzqu", 16), 0)
local expected, original = exhaustive(string.rep("nihcnzqu", 16), 0)
check(queries == expected and reduced < original, "same plans with fewer native lookups")
check(indexed("nihcnzqu", 0):find("|ni hcnz qu", 1, true), "variable-length full path survives")
check(indexed("jbzqu", 0):find("|jbz qu", 1, true), "three-plus-two path survives")

-- Five-key custom spellings are deliberately outside the canonical length set.
-- Each unsupported profile must retain them instead of pruning with stale data.
for _, settings in ipairs({
  { dictionary = "xhup_flow_learn" },
  { enabled = false, dictionary = "xhup_flow_learn" },
  { enabled = true, dictionary = "custom" },
  { enabled = true, dictionary = "xhup_flow_learn", prism = "custom" },
  { enabled = true, dictionary = "xhup_flow_learn", packs = {} },
  { enabled = true, dictionary = "xhup_flow_learn", old_api = true },
  { enabled = true, dictionary = "xhup_flow_learn", missing_identity = true },
  { enabled = true, dictionary = "xhup_flow_learn", identity = "previous-deployment" },
}) do
  local _, custom = fixture(settings, { aaaaa = true, qu = true })
  check(custom("aaaaaqu", 0):find("|aaaaa qu", 1, true), "custom/legacy profile keeps exhaustive lookup")
end
policy.fini(env)
env.engine.schema.config.get_bool = function() return false end
policy.init(env)
check(env.edge_lengths == nil, "schema reinitialization cannot retain the previous index")
policy.fini(env)
print("PASS native edge index: " .. checks .. " coverage and behavior assertions; lookups " .. original .. " -> " .. reduced)
