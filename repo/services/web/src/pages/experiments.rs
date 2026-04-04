use dioxus::prelude::*;

use contracts::{
    ExperimentAssignRequest, ExperimentBacktrackRequest, ExperimentCreateRequest,
    ExperimentVariantRequest,
};

use crate::api;
use crate::state::SessionContext;

#[component]
pub fn ExperimentsPage(
    mut status: Signal<String>,
    mut error: Signal<String>,
    session: Signal<Option<SessionContext>>,
    mut experiment_id: Signal<String>,
    mut experiment_key: Signal<String>,
    mut variant_key: Signal<String>,
    mut variant_weight: Signal<String>,
    mut variant_version: Signal<String>,
    mut assign_user_id: Signal<String>,
    mut assign_mode: Signal<String>,
    mut backtrack_from: Signal<String>,
    mut backtrack_to: Signal<String>,
    mut backtrack_reason: Signal<String>,
    mut assigned_variant: Signal<String>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Experiments" }
            section { class: "subpanel",
                h4 { "Create Experiment" }
                input { placeholder: "experiment_key", value: "{experiment_key}", oninput: move |evt| experiment_key.set(evt.value()) }
                button {
                    onclick: move |_| {
                        let req = ExperimentCreateRequest { experiment_key: experiment_key() };
                        let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                        spawn(async move {
                            match api::create_experiment(&token, req).await {
                                Ok(id) => {
                                    experiment_id.set(id.to_string());
                                    status.set(format!("Experiment created #{id}"));
                                }
                                Err(e) => error.set(e),
                            }
                        });
                    },
                    "Create"
                }
            }
            section { class: "subpanel",
                h4 { "Variants" }
                input { placeholder: "Experiment ID", value: "{experiment_id}", oninput: move |evt| experiment_id.set(evt.value()) }
                input { placeholder: "Variant key", value: "{variant_key}", oninput: move |evt| variant_key.set(evt.value()) }
                input { placeholder: "Weight", value: "{variant_weight}", oninput: move |evt| variant_weight.set(evt.value()) }
                input { placeholder: "Feature version", value: "{variant_version}", oninput: move |evt| variant_version.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let (Ok(id), Ok(weight)) = (experiment_id().parse::<i64>(), variant_weight().parse::<f64>()) {
                            let req = ExperimentVariantRequest { variant_key: variant_key(), allocation_weight: weight, feature_version: variant_version() };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::add_experiment_variant(&token, id, req).await {
                                    Ok(_) => status.set("Variant added".to_string()),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Add Variant"
                }
            }
            section { class: "subpanel",
                h4 { "Assign / Backtrack" }
                input { placeholder: "User ID", value: "{assign_user_id}", oninput: move |evt| assign_user_id.set(evt.value()) }
                input { placeholder: "Mode", value: "{assign_mode}", oninput: move |evt| assign_mode.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let (Ok(id), Ok(uid)) = (experiment_id().parse::<i64>(), assign_user_id().parse::<i64>()) {
                            let req = ExperimentAssignRequest { user_id: uid, mode: assign_mode() };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::assign_experiment(&token, id, req).await {
                                    Ok(value) => assigned_variant.set(value.unwrap_or_else(|| "none".to_string())),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Assign"
                }
                p { class: "muted", "Assigned variant: {assigned_variant}" }
                input { placeholder: "From version", value: "{backtrack_from}", oninput: move |evt| backtrack_from.set(evt.value()) }
                input { placeholder: "To version", value: "{backtrack_to}", oninput: move |evt| backtrack_to.set(evt.value()) }
                input { placeholder: "Reason", value: "{backtrack_reason}", oninput: move |evt| backtrack_reason.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let Ok(id) = experiment_id().parse::<i64>() {
                            let req = ExperimentBacktrackRequest { from_version: backtrack_from(), to_version: backtrack_to(), reason: backtrack_reason() };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::backtrack_experiment(&token, id, req).await {
                                    Ok(_) => status.set("Backtrack recorded".to_string()),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Record Backtrack"
                }
            }
        }
    }
}
