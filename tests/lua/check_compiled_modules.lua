-- No source-tree fallback: every declared production module must load from the package.
local root = assert(arg[1], "compiled package required")
package.path = root .. "/lua/?.lua;" .. root .. "/lua/?/init.lua"
for index = 3, #arg do
  assert(type(require(arg[index])) == "table", "invalid module: " .. arg[index])
end
local build = require("xhup_flow.data.build_info")
assert(build.version == arg[2], "runtime/package version mismatch")
local report = require("xhup_flow").check_contract()
assert(report.ok, table.concat(report.errors, "; "))
print("PASS compiled module loading and version contract: " .. build.version)
