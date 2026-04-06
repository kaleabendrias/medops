use contracts::{IngestionTaskDto, IngestionTaskRunDto, IngestionTaskVersionDto};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct IngestionState {
    pub ingestion_tasks: Signal<Vec<IngestionTaskDto>>,
    pub ingestion_versions: Signal<Vec<IngestionTaskVersionDto>>,
    pub ingestion_runs: Signal<Vec<IngestionTaskRunDto>>,
    pub ingestion_task_name: Signal<String>,
    pub ingestion_seed_urls: Signal<String>,
    pub ingestion_rules: Signal<String>,
    pub ingestion_strategy: Signal<String>,
    pub ingestion_depth: Signal<String>,
    pub ingestion_incremental_field: Signal<String>,
    pub ingestion_schedule: Signal<String>,
    pub ingestion_selected_task: Signal<String>,
    pub ingestion_rollback_version: Signal<String>,
    pub ingestion_rollback_reason: Signal<String>,
}

pub fn use_ingestion_state() -> IngestionState {
    IngestionState {
        ingestion_tasks: use_signal(Vec::<IngestionTaskDto>::new),
        ingestion_versions: use_signal(Vec::<IngestionTaskVersionDto>::new),
        ingestion_runs: use_signal(Vec::<IngestionTaskRunDto>::new),
        ingestion_task_name: use_signal(|| "patient-feed-ui".to_string()),
        ingestion_seed_urls: use_signal(|| "file:///app/config/ingestion_fixture/page1.html".to_string()),
        ingestion_rules: use_signal(|| "{\"mode\":\"css\",\"fields\":[\".record\"],\"pagination_selector\":\"a.next\"}".to_string()),
        ingestion_strategy: use_signal(|| "breadth-first".to_string()),
        ingestion_depth: use_signal(|| "2".to_string()),
        ingestion_incremental_field: use_signal(|| "value".to_string()),
        ingestion_schedule: use_signal(|| "0 * * * *".to_string()),
        ingestion_selected_task: use_signal(String::new),
        ingestion_rollback_version: use_signal(String::new),
        ingestion_rollback_reason: use_signal(String::new),
    }
}

impl IngestionState {
    pub fn reset(&mut self) {
        self.ingestion_tasks.set(Vec::new());
        self.ingestion_versions.set(Vec::new());
        self.ingestion_runs.set(Vec::new());
    }
}
