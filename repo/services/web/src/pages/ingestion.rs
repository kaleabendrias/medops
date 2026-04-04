use dioxus::prelude::*;

use contracts::{
    IngestionTaskCreateRequest, IngestionTaskDto, IngestionTaskRollbackRequest,
    IngestionTaskRunDto, IngestionTaskUpdateRequest, IngestionTaskVersionDto,
};

use crate::api;
use crate::state::SessionContext;

#[component]
pub fn IngestionPage(
    mut status: Signal<String>,
    mut error: Signal<String>,
    session: Signal<Option<SessionContext>>,
    mut ingestion_tasks: Signal<Vec<IngestionTaskDto>>,
    mut ingestion_versions: Signal<Vec<IngestionTaskVersionDto>>,
    mut ingestion_runs: Signal<Vec<IngestionTaskRunDto>>,
    mut ingestion_task_name: Signal<String>,
    mut ingestion_seed_urls: Signal<String>,
    mut ingestion_rules: Signal<String>,
    mut ingestion_strategy: Signal<String>,
    mut ingestion_depth: Signal<String>,
    mut ingestion_incremental_field: Signal<String>,
    mut ingestion_schedule: Signal<String>,
    mut ingestion_selected_task: Signal<String>,
    mut ingestion_rollback_version: Signal<String>,
    mut ingestion_rollback_reason: Signal<String>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Ingestion Task Manager" }
            p { class: "muted", "Create, update, version, rollback, run, and inspect status with role-aware API errors surfaced inline." }
            button {
                class: "primary",
                onclick: move |_| {
                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                    spawn(async move {
                        match api::list_ingestion_tasks(&token).await {
                            Ok(items) => ingestion_tasks.set(items),
                            Err(e) => error.set(e),
                        }
                    });
                },
                "Refresh Tasks"
            }
            section { class: "subpanel",
                h4 { "Create / Update Task" }
                input { placeholder: "Task name", value: "{ingestion_task_name}", oninput: move |evt| ingestion_task_name.set(evt.value()) }
                textarea { placeholder: "Seed URLs (comma-separated)", value: "{ingestion_seed_urls}", oninput: move |evt| ingestion_seed_urls.set(evt.value()) }
                textarea { placeholder: "Extraction rules JSON", value: "{ingestion_rules}", oninput: move |evt| ingestion_rules.set(evt.value()) }
                input { placeholder: "Pagination strategy", value: "{ingestion_strategy}", oninput: move |evt| ingestion_strategy.set(evt.value()) }
                input { placeholder: "Max depth", value: "{ingestion_depth}", oninput: move |evt| ingestion_depth.set(evt.value()) }
                input { placeholder: "Incremental field", value: "{ingestion_incremental_field}", oninput: move |evt| ingestion_incremental_field.set(evt.value()) }
                input { placeholder: "Schedule cron", value: "{ingestion_schedule}", oninput: move |evt| ingestion_schedule.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let Ok(max_depth) = ingestion_depth().parse::<i32>() {
                            let seed_urls = ingestion_seed_urls()
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>();
                            let req = IngestionTaskCreateRequest {
                                task_name: ingestion_task_name(),
                                seed_urls,
                                extraction_rules_json: ingestion_rules(),
                                pagination_strategy: ingestion_strategy(),
                                max_depth,
                                incremental_field: Some(ingestion_incremental_field()),
                                schedule_cron: ingestion_schedule(),
                            };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::create_ingestion_task(&token, req).await {
                                    Ok(id) => {
                                        ingestion_selected_task.set(id.to_string());
                                        status.set(format!("Ingestion task created #{id}"));
                                    }
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Create Task"
                }
                input { placeholder: "Task ID for update", value: "{ingestion_selected_task}", oninput: move |evt| ingestion_selected_task.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let (Ok(task_id), Ok(max_depth)) = (
                            ingestion_selected_task().parse::<i64>(),
                            ingestion_depth().parse::<i32>(),
                        ) {
                            let seed_urls = ingestion_seed_urls()
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>();
                            let req = IngestionTaskUpdateRequest {
                                seed_urls,
                                extraction_rules_json: ingestion_rules(),
                                pagination_strategy: ingestion_strategy(),
                                max_depth,
                                incremental_field: Some(ingestion_incremental_field()),
                                schedule_cron: ingestion_schedule(),
                            };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::update_ingestion_task(&token, task_id, req).await {
                                    Ok(version) => status.set(format!("Task updated to version {version}")),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Update Task"
                }
            }
            section { class: "subpanel",
                h4 { "Run / Versions / Rollback" }
                button {
                    onclick: move |_| {
                        if let Ok(task_id) = ingestion_selected_task().parse::<i64>() {
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::run_ingestion_task(&token, task_id).await {
                                    Ok(_) => status.set("Ingestion run started".to_string()),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Run Task"
                }
                button {
                    onclick: move |_| {
                        if let Ok(task_id) = ingestion_selected_task().parse::<i64>() {
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                if let Ok(v) = api::ingestion_task_versions(&token, task_id).await {
                                    ingestion_versions.set(v);
                                }
                                if let Ok(r) = api::ingestion_task_runs(&token, task_id).await {
                                    ingestion_runs.set(r);
                                }
                            });
                        }
                    },
                    "Load Versions + Runs"
                }
                input { placeholder: "Rollback target version", value: "{ingestion_rollback_version}", oninput: move |evt| ingestion_rollback_version.set(evt.value()) }
                input { placeholder: "Rollback reason", value: "{ingestion_rollback_reason}", oninput: move |evt| ingestion_rollback_reason.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let (Ok(task_id), Ok(target_version)) = (
                            ingestion_selected_task().parse::<i64>(),
                            ingestion_rollback_version().parse::<i32>(),
                        ) {
                            let req = IngestionTaskRollbackRequest {
                                target_version,
                                reason: ingestion_rollback_reason(),
                            };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::rollback_ingestion_task(&token, task_id, req).await {
                                    Ok(version) => status.set(format!("Rollback created version {version}")),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Rollback"
                }
            }
            section { class: "subpanel",
                h4 { "Task Status" }
                div { class: "cards",
                    for task in ingestion_tasks() {
                        article { class: "card",
                            p { "#{task.id} {task.task_name} (v{task.active_version})" }
                            p { class: "muted", "{task.status} / {task.pagination_strategy} / depth {task.max_depth}" }
                            p { class: "muted", "next: {task.next_run_at:?} last: {task.last_run_at:?}" }
                        }
                    }
                }
                div { class: "cards",
                    for version in ingestion_versions() {
                        article { class: "card",
                            p { "version {version.version_number}" }
                            p { class: "muted", "rollback_of: {version.rollback_of_version:?}" }
                            p { class: "muted", "{version.created_at}" }
                        }
                    }
                }
                div { class: "cards",
                    for run in ingestion_runs() {
                        article { class: "card",
                            p { "run #{run.id} v{run.task_version} {run.status}" }
                            p { class: "muted", "records: {run.records_extracted}" }
                            p { class: "muted", "{run.started_at} -> {run.finished_at:?}" }
                        }
                    }
                }
            }
        }
    }
}
