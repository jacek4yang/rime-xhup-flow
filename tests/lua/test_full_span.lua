dofile("tests/lua/runtime_path.lua")
local filter = require("xhup_flow.full_span")
local checks = 0
local function check(value, message) assert(value, message); checks = checks + 1 end
local input = {}
for i = 1, 500 do
  input[i] = { text = tostring(i), start = 0, _end = i }
end
input[2] = { text = "same", start = 0, _end = 2 }
input[7] = { text = "same", start = 0, _end = 7 }
input[8] = { text = "same", start = 0, _end = 5 }
input[9] = { text = "same", start = 1, _end = 9 }
input[60] = { text = "same", start = 0, _end = 600 } -- outside global head
input[80] = input[7] -- repeated exact object must retain its count
local output, index = {}, 0
yield = function(candidate) output[#output+1] = candidate end
filter.func({ iter = function() return function() index = index+1; return input[index] end end }, {})
check(#output == #input, "multiplicity")
check(output[2] == input[7] and output[7] == input[2], "longest native span swaps to first duplicate")
local counts = {}
for i, candidate in ipairs(input) do
  counts[candidate] = (counts[candidate] or 0) + 1
  check(output[i].text == candidate.text, "other-text positions never reorder")
  if i ~= 2 and i ~= 7 then check(output[i] == candidate, "all other identities/tail unchanged") end
end
for _, candidate in ipairs(output) do counts[candidate] = counts[candidate] - 1 end
for _, count in pairs(counts) do check(count == 0, "original object counts preserved") end
-- An unavailable tail must not prevent showing an already complete first page.
do
  local consumed = 0
  yield = coroutine.yield
  local stream = coroutine.create(function()
    filter.func({iter=function() return function()
      consumed = consumed + 1
      if consumed > 5 then error("tail unavailable") end
      return {text=tostring(consumed), start=0, _end=4}
    end end}, {engine={context={input="aabb"}}})
  end)
  for i = 1, 5 do
    local ok, candidate = coroutine.resume(stream)
    check(ok and candidate.text == tostring(i) and candidate._end == 4,
      "complete first page remains available without waiting for the tail")
  end
end

-- Streaming complete entries must not restart the global bounded window or
-- swap later duplicates of an already emitted, fully consumed entry.
do
  local rows = {}
  for i = 1, 40 do rows[i] = {text=tostring(i), start=0, _end=12} end
  rows[1] = {text="complete", start=0, _end=12}
  rows[4] = {text="pending", start=0, _end=4}
  rows[5] = {text="complete", start=0, _end=2}
  rows[6] = {text="complete", start=0, _end=12}
  rows[8] = {text="pending", start=0, _end=12}
  rows[31] = {text="outside", start=0, _end=3}
  rows[33] = {text="outside", start=0, _end=12}
  local result, position = {}, 0
  yield = function(candidate) result[#result + 1] = candidate end
  filter.func({iter=function() return function()
    position = position + 1
    return rows[position]
  end end}, {engine={context={input="aabbccddeeff"}}})
  check(result[4] == rows[8] and result[8] == rows[4],
    "pending duplicate still receives its complete native candidate")
  check(result[1] == rows[1] and result[5] == rows[5] and result[6] == rows[6],
    "later duplicates cannot change the identity of an already complete entry")
  check(result[31] == rows[31] and result[33] == rows[33],
    "streamed entries still count toward the single global head boundary")
end

print(string.format("PASS full-span bounded policy: %d assertions (Lua stub)", checks))
