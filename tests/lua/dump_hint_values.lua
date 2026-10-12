-- Canonical values, independent of Lua comments, formatting, and compiler headers.
local hints = dofile(assert(arg[1], "hint data module required"))
local words = {}
for word, code in pairs(hints) do
  assert(type(word) == "string" and type(code) == "string")
  assert(not word:find("[\t\r\n]") and code:match("^[a-z]+$"))
  words[#words + 1] = word
end
table.sort(words)
for _, word in ipairs(words) do
  io.write(word, "\t", hints[word], "\n")
end
