-- Shared source/compiled test entry. This file is never a production module.
local root = os.getenv("XHUP_LUA_ROOT")
if not root then
  root = "rime/lua"
  package.preload["xhup_flow.data.build_info"] = function()
    return { version = "source-test" }
  end
end
package.path = root .. "/?.lua;" .. root .. "/?/init.lua;" .. package.path
return root
