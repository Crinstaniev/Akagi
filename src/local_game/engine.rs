use crate::schema::local_game::{LocalGameEngineMetadata, LocalWorkerMetadata};

pub const ENGINE_SCHEMA_VERSION: u32 = 1;
pub const ENGINE_SOURCE_DETERMINISTIC_STUB: &str = "deterministic_stub";
pub const ENGINE_STATUS_ACTIVE: &str = "active";
pub const ENGINE_STATUS_TERMINAL: &str = "terminal";
pub const ACTION_TYPE_DISCARD: &str = "discard";
pub const ACTION_TYPE_TSUMO: &str = "tsumo";
pub const ACTION_TYPE_RON: &str = "ron";
pub const ACTION_TYPE_CHI: &str = "chi";
pub const ACTION_TYPE_PON: &str = "pon";
pub const ACTION_TYPE_KAN: &str = "kan";
pub const ACTION_TYPE_RIICHI: &str = "riichi";
pub const ACTION_TYPE_PASS: &str = "pass";

const DETERMINISTIC_ENGINE_NOTE: &str =
    "Deterministic local rule engine fallback; full RiichiEnv rules are not active.";

pub fn deterministic_engine_metadata(ended: bool) -> LocalGameEngineMetadata {
    LocalGameEngineMetadata {
        schema_version: ENGINE_SCHEMA_VERSION,
        source: ENGINE_SOURCE_DETERMINISTIC_STUB.into(),
        status: if ended {
            ENGINE_STATUS_TERMINAL.into()
        } else {
            ENGINE_STATUS_ACTIVE.into()
        },
        capabilities: deterministic_capabilities()
            .iter()
            .map(|capability| (*capability).into())
            .collect(),
        note: DETERMINISTIC_ENGINE_NOTE.into(),
        worker: LocalWorkerMetadata::default(),
    }
}

pub fn deterministic_capabilities() -> &'static [&'static str] {
    &[ACTION_TYPE_DISCARD]
}

pub fn reserved_action_types() -> &'static [&'static str] {
    &[
        ACTION_TYPE_DISCARD,
        ACTION_TYPE_TSUMO,
        ACTION_TYPE_RON,
        ACTION_TYPE_CHI,
        ACTION_TYPE_PON,
        ACTION_TYPE_KAN,
        ACTION_TYPE_RIICHI,
        ACTION_TYPE_PASS,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_metadata_reports_active_and_terminal_status() {
        let active = deterministic_engine_metadata(false);
        assert_eq!(active.schema_version, 1);
        assert_eq!(active.source, ENGINE_SOURCE_DETERMINISTIC_STUB);
        assert_eq!(active.status, ENGINE_STATUS_ACTIVE);
        assert_eq!(active.capabilities, vec![ACTION_TYPE_DISCARD.to_string()]);

        let terminal = deterministic_engine_metadata(true);
        assert_eq!(terminal.status, ENGINE_STATUS_TERMINAL);
    }

    #[test]
    fn reserved_action_types_cover_pre_ai_taxonomy() {
        let reserved = reserved_action_types();
        for action_type in [
            ACTION_TYPE_DISCARD,
            ACTION_TYPE_TSUMO,
            ACTION_TYPE_RON,
            ACTION_TYPE_CHI,
            ACTION_TYPE_PON,
            ACTION_TYPE_KAN,
            ACTION_TYPE_RIICHI,
            ACTION_TYPE_PASS,
        ] {
            assert!(
                reserved.contains(&action_type),
                "missing reserved action type: {action_type}"
            );
        }
    }
}
