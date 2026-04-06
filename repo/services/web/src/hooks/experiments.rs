use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct ExperimentsState {
    pub experiment_id: Signal<String>,
    pub experiment_key: Signal<String>,
    pub variant_key: Signal<String>,
    pub variant_weight: Signal<String>,
    pub variant_version: Signal<String>,
    pub assign_user_id: Signal<String>,
    pub assign_mode: Signal<String>,
    pub backtrack_from: Signal<String>,
    pub backtrack_to: Signal<String>,
    pub backtrack_reason: Signal<String>,
    pub assigned_variant: Signal<String>,
}

pub fn use_experiments_state() -> ExperimentsState {
    ExperimentsState {
        experiment_id: use_signal(String::new),
        experiment_key: use_signal(String::new),
        variant_key: use_signal(String::new),
        variant_weight: use_signal(|| "1.0".to_string()),
        variant_version: use_signal(|| "v1".to_string()),
        assign_user_id: use_signal(|| "1".to_string()),
        assign_mode: use_signal(|| "manual".to_string()),
        backtrack_from: use_signal(|| "v2".to_string()),
        backtrack_to: use_signal(|| "v1".to_string()),
        backtrack_reason: use_signal(String::new),
        assigned_variant: use_signal(String::new),
    }
}
