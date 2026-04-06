use contracts::AuditLogDto;
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct AuditsState {
    pub audits: Signal<Vec<AuditLogDto>>,
}

pub fn use_audits_state() -> AuditsState {
    AuditsState {
        audits: use_signal(Vec::<AuditLogDto>::new),
    }
}
