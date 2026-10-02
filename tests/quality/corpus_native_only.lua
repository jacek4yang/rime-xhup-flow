-- Test-only ablation: retain the same native provider, filters and base stream,
-- but do not supply dictionary-guided boundary alternatives. No learning.
local policy = require("xhup_flow.native_tail")
return {
  init = function(env)
    policy.init(env)
    env.memory = nil
  end,
  func = policy.func,
  fini = policy.fini,
}
