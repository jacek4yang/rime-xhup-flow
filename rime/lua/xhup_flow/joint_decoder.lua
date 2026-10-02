-- Legacy customization shim, not a joint decoder or an evidence provider.
-- Kept at the owned package path so upgrades replace the old research hook.
-- The supported schema does not register this module or offer an enable switch.
local M = { available = false, reason = "research joint decoder is not integrated" }
function M.init(env)
  env.engine.context:set_property("xhup_flow_joint_decoder_status", "unavailable")
end
function M.func(translation, _env)
  for candidate in translation:iter() do yield(candidate) end
end
return M
