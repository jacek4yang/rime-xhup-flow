-- Use the actual generated frequency model, not a hand-copied frequency fixture.
dofile("tests/lua/runtime_path.lua")
local search = require("xhup_flow.joint_decoder")
local package_root = assert(arg[1], "generated package required")
local model = assert(loadfile(package_root .. "/lua/xhup_flow/data/joint_scores.lua"))()
local score = search.scorer(model, model.identity)
local result = search.search("difh", function(start)
  if start == 0 then
    return {
      { keys = "di", text = "的", lexical = score("的", 0, "di") },
      { keys = "di", text = "地", lexical = score("地", 0, "di") },
    }
  elseif start == 2 then
    return { { keys = "fh", text = "方", lexical = score("方", 0, "fh") } }
  end
  return {}
end)
local first = search.describe("difh", result.paths[1])
assert(first.text == "地方", "common de frequency must not leak into the di decoding path; got " .. first.text)
print("PASS generated reading-frequency search: 地方 outranks 的方 on the di path")
