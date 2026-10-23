-- Exhaustive native audit fixture. Never registered or shipped as a runtime module.
local M = {}
function M.init(env)
  local config = env.engine.schema.config
  assert(config:get_bool("flow_lookup/enable_user_dict") == false)
  local memory = Memory(env.engine, env.engine.schema, "flow_lookup")
  assert(memory.user_dict == nil and memory.dict.loaded)
  local original = ReverseDb("build/xhup_flow_user.reverse.bin")
  local projection = ReverseDb("build/xhup_flow_alias_projection.reverse.bin")
  local identity = config:get_string("flow/alias_index_id")
  assert(identity and #identity == 64 and projection:lookup("__xhup_alias_identity__") == identity)
  local source = assert(io.open(config:get_string("alias_projection_audit/expected")))
  local count, texts = 0, 0
  for line in source:lines() do
    local text, expected = line:match("^(.-)\t(.*)$")
    assert(text and expected, "invalid audit input")
    assert(projection:lookup(text) == expected, "projected reverse value mismatch: " .. text)
    local original_codes, values = {}, {}
    for code in original:lookup(text):gmatch("[a-z]+") do
      if #original_codes == 8 then break end
      original_codes[#original_codes + 1] = code
    end
    for value in expected:gmatch("%S+") do values[#values + 1] = value end
    assert(#values == #original_codes, "missing or extra reverse codes: " .. text)
    for position, value in ipairs(values) do
      local spelling, suffix = value:match("^([a-z]+)!(.+)$")
      assert(spelling == original_codes[position], "reverse-code order mismatch: " .. text)
      local id, weight = suffix:match("^(%d+)!(.+)$")
      local found, examined = nil, 0
      memory:dict_lookup(spelling, false, 64)
      for entry in memory:iter_dict() do
        examined = examined + 1
        if entry.text == text then found = entry; break end
        if examined == 64 then break end
      end
      if suffix == "x" then
        assert(found == nil, "eligible native row omitted: " .. text)
      else
        assert(found and found.code:print() == id, "native ID mismatch: " .. text .. "/" .. spelling)
        assert(found.weight == tonumber(weight), "native weight mismatch: " .. text .. "/" .. spelling)
        local decoded = memory.dict:decode(found.code)
        assert(#decoded == 1 and decoded[1] == spelling, "native decode mismatch")
      end
      count = count + 1
    end
    texts = texts + 1
  end
  source:close()
  local result = assert(io.open(config:get_string("alias_projection_audit/result"), "w"))
  result:write('{"records":', tostring(count), ',"texts":', tostring(texts), '}\n')
  result:close()
end
function M.func() end
return M
