-- Pure Lua failure injection, NOT native-platform or crash-durability evidence.
package.path = "rime/lua/?.lua;" .. package.path
local um = require("xhup_flow.user_memory")
local original = { open = io.open, tmpname = os.tmpname, rename = os.rename,
                   remove = os.remove, config = package.config }
local passed = 0
local function check(value, label)
    assert(value, label)
    passed = passed + 1
end
local function restore()
    io.open, os.tmpname = original.open, original.tmpname
    os.rename, os.remove = original.rename, original.remove
    package.config = original.config
end
-- All errors leave the previously durable snapshot intact and close the handle.
for _, failure in ipairs({ "create", "open", "write", "flush", "close", "replace", "permission" }) do
    local old, closed, removed = "old snapshot", false, false
    package.config = "/\n"
    os.tmpname = function()
        if failure == "create" then error("injected create failure") end
        return "/private/reserved-temp"
    end
    io.open = function(path)
        check(path == "/private/reserved-temp", "reserved temporary file")
        if failure == "open" or failure == "permission" then return nil, "denied" end
        return {
            write = function() return failure ~= "write" and true or nil end,
            flush = function() return failure ~= "flush" and true or nil end,
            close = function() closed = true; return failure ~= "close" and true or nil end,
        }
    end
    os.rename = function()
        if failure == "replace" then return nil, "injected rename failure" end
        old = "new snapshot"
        return true
    end
    os.remove = function(path)
        check(path ~= "/snapshot", "never remove durable destination")
        removed = true
        return true
    end
    local ok, err = um.atomic_write("/snapshot", "new snapshot")
    check(not ok and type(err) == "string", failure .. " propagates")
    check(old == "old snapshot", failure .. " preserves old state")
    if failure ~= "create" then check(removed, failure .. " cleans temporary file") end
    if failure ~= "create" and failure ~= "open" and failure ~= "permission" then
        check(closed, failure .. " closes handle")
    end
    restore()
end
package.config = "\\\n"
os.tmpname = function() error("Windows must not create an unsafe temp") end
check(not um.atomic_write("/snapshot", "new"), "Windows fails closed")
restore()

local writer = um.atomic_write
local env = { dirty = true, pending = 20, snapshot_path = "/snapshot", counts = { test = 1 } }
um.atomic_write = function() return nil, "injected replacement failure" end
local ok, err = um.flush(env)
check(not ok and err == env.persistence_error, "flush propagates error")
check(env.dirty and env.pending == 20, "failure retains retry state")
local disconnected = false
env.commit_connection = { disconnect = function() disconnected = true end }
check(not um.fini(env), "dirty shutdown failure propagates")
check(disconnected and env.commit_connection == nil, "shutdown disconnects notifier")
check(env.dirty and env.pending == 20, "shutdown failure retains dirty state")
um.atomic_write = function() return true end
check(um.flush(env), "retry succeeds")
check(not env.dirty and env.pending == 0 and not env.persistence_error, "success clears dirty only")
env.dirty, env.pending = true, 1
check(um.fini(env) and not env.dirty, "dirty shutdown saves")
um.atomic_write = writer
check(not um.flush({ dirty = true }), "missing path fails explicitly")

-- Actual filesystem: a destination directory must survive failed replacement.
local path = os.tmpname()
os.remove(path)
assert(os.execute('mkdir "' .. path .. '"'))
check(not um.atomic_write(path, "snapshot"), "conflicting destination rejected")
local f = assert(io.open(path .. "/sentinel", "wb"))
assert(f:write("still a directory")); assert(f:close())
check(true, "conflicting directory preserved")
os.remove(path .. "/sentinel")
os.remove(path)
print(string.format("PASS user_memory failure injection: %d assertions", passed))
