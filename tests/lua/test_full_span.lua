package.path = "rime/lua/?.lua;" .. package.path
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
print(string.format("PASS full-span bounded policy: %d assertions (Lua stub)", checks))
