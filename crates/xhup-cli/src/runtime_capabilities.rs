//! Shared capability vocabulary. File presence is never promoted to live evidence.
use serde::Serialize;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Filesystem,
    Configuration,
    Runtime,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Capability {
    pub state: State,
    pub evidence: Evidence,
}

impl Capability {
    pub const UNKNOWN: Self = Self {
        state: State::Unknown,
        evidence: Evidence::None,
    };
    pub fn observed(available: bool, evidence: Evidence) -> Self {
        Self {
            state: if available {
                State::Available
            } else {
                State::Unavailable
            },
            evidence,
        }
    }
    pub fn runtime_available(self) -> bool {
        self.state == State::Available && self.evidence == Evidence::Runtime
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeCapabilities {
    pub lua_payload: Capability,
    pub lua_registered: Capability,
    pub lua_filter_active: Capability,
    pub live_evidence_provider: Capability,
    pub contextual_decoder: Capability,
    pub learning_configured: Capability,
    pub storage_writable: Capability,
    pub static_fallback_usable: Capability,
}

/// Supplied only by an actual same-environment runtime probe/acceptance adapter.
/// This value alone has no artifact provenance or release-acceptance authority.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeObservation {
    pub lua_registered: bool,
    pub lua_filter_active: bool,
    pub live_evidence_provider: bool,
    pub contextual_decoder: bool,
    pub learning_configured: bool,
    pub storage_writable: bool,
    pub static_fallback_usable: bool,
}

impl Default for RuntimeCapabilities {
    fn default() -> Self {
        Self {
            lua_payload: Capability::UNKNOWN,
            lua_registered: Capability::UNKNOWN,
            lua_filter_active: Capability::UNKNOWN,
            live_evidence_provider: Capability::UNKNOWN,
            contextual_decoder: Capability::UNKNOWN,
            learning_configured: Capability::UNKNOWN,
            storage_writable: Capability::UNKNOWN,
            static_fallback_usable: Capability::UNKNOWN,
        }
    }
}

impl RuntimeCapabilities {
    pub fn apply_runtime(&mut self, observed: RuntimeObservation) -> Result<(), &'static str> {
        if observed.lua_filter_active && !observed.lua_registered {
            return Err("Lua filter cannot be active without Lua registration");
        }
        let runtime = |value| Capability::observed(value, Evidence::Runtime);
        self.lua_registered = runtime(observed.lua_registered);
        self.lua_filter_active = runtime(observed.lua_filter_active);
        self.live_evidence_provider = runtime(observed.live_evidence_provider);
        self.contextual_decoder = runtime(observed.contextual_decoder);
        self.learning_configured = runtime(observed.learning_configured);
        self.storage_writable = runtime(observed.storage_writable);
        self.static_fallback_usable = runtime(observed.static_fallback_usable);
        Ok(())
    }

    /// Baseline infrastructure only, NOT decoding/learning/release correctness.
    pub fn live_flow_infrastructure_ready(&self) -> bool {
        self.lua_registered.runtime_available()
            && self.lua_filter_active.runtime_available()
            && self.static_fallback_usable.runtime_available()
    }

    pub fn format_report(&self) -> String {
        let mut text = String::from("能力证据（Unknown 不等于 PASS，文件不等于执行）:\n");
        for (name, capability) in [
            ("lua_payload", self.lua_payload),
            ("lua_registered", self.lua_registered),
            ("lua_filter_active", self.lua_filter_active),
            ("live_evidence_provider", self.live_evidence_provider),
            ("contextual_decoder", self.contextual_decoder),
            ("learning_configured", self.learning_configured),
            ("storage_writable", self.storage_writable),
            ("static_fallback_usable", self.static_fallback_usable),
        ] {
            text.push_str(&format!(
                "  {name}: {:?} ({:?})\n",
                capability.state, capability.evidence
            ));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation() -> RuntimeObservation {
        RuntimeObservation {
            lua_registered: true,
            lua_filter_active: true,
            live_evidence_provider: false,
            contextual_decoder: false,
            learning_configured: false,
            storage_writable: false,
            static_fallback_usable: true,
        }
    }
    #[test]
    fn installed_does_not_mean_registered_or_ready() {
        let mut caps = RuntimeCapabilities {
            lua_payload: Capability::observed(true, Evidence::Filesystem),
            ..Default::default()
        };
        assert!(!caps.live_flow_infrastructure_ready());
        let mut obs = observation();
        obs.lua_registered = false;
        obs.lua_filter_active = false;
        caps.apply_runtime(obs).unwrap();
        assert_eq!(caps.lua_payload.state, State::Available);
        assert_eq!(caps.lua_registered.state, State::Unavailable);
        assert!(!caps.live_flow_infrastructure_ready());
    }
    #[test]
    fn filter_does_not_imply_decoder_learning_or_writable_storage() {
        let mut caps = RuntimeCapabilities::default();
        caps.apply_runtime(observation()).unwrap();
        assert!(caps.live_flow_infrastructure_ready());
        assert!(!caps.contextual_decoder.runtime_available());
        assert!(!caps.live_evidence_provider.runtime_available());
        assert!(!caps.learning_configured.runtime_available());
        assert!(!caps.storage_writable.runtime_available());
    }
    #[test]
    fn contradictory_runtime_evidence_is_rejected_without_mutation() {
        let mut caps = RuntimeCapabilities::default();
        let before = caps.clone();
        let mut obs = observation();
        obs.lua_registered = false;
        assert!(caps.apply_runtime(obs).is_err());
        assert_eq!(caps, before);
    }
}
