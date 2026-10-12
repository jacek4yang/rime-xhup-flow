-- Fresh-VM module loading only: no librime initialization or frontend latency.
local root = assert(arg[1], "package directory and schema module names required")
assert(#arg > 1, "at least one schema module is required")
package.path = root .. "/lua/?.lua;" .. root .. "/lua/?/init.lua"
package.cpath = ""
collectgarbage("collect")
local before = collectgarbage("count")
local started = os.clock()
for i = 2, #arg do
  require(arg[i])
end
local load_ms = (os.clock() - started) * 1000
collectgarbage("collect")
local live_kib = collectgarbage("count") - before
local repetitions = 1000
started = os.clock()
for _ = 1, repetitions do
  for i = 2, #arg do
    require(arg[i])
  end
end
local warm_us = (os.clock() - started) * 1000000 / repetitions
print(string.format(
  '{"lua_version":"%s","load_cpu_ms":%.9f,"live_heap_kib":%.9f,"warm_require_us_per_pass":%.9f}',
  _VERSION, load_ms, live_kib, warm_us
))
