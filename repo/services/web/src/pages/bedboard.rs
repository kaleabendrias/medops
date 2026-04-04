use dioxus::prelude::*;

use contracts::{BedDto, BedEventDto, BedTransitionRequest};

use crate::api;
use crate::state::SessionContext;

#[component]
pub fn BedboardPage(
    mut status: Signal<String>,
    mut error: Signal<String>,
    session: Signal<Option<SessionContext>>,
    beds: Signal<Vec<BedDto>>,
    bed_events: Signal<Vec<BedEventDto>>,
    mut bed_transition_id: Signal<String>,
    mut bed_transition_action: Signal<String>,
    mut bed_transition_state: Signal<String>,
    mut bed_transition_patient_id: Signal<String>,
    mut bed_transition_related: Signal<String>,
    mut bed_transition_note: Signal<String>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Bed Board" }
            p { class: "muted", "Live polling every 8 seconds with state chips and an event timeline." }
            div { class: "grid-2",
                section { class: "subpanel",
                    h4 { "Beds" }
                    div { class: "cards",
                        for bed in beds() {
                            article { class: "card bed-card",
                                div { class: "row",
                                    strong { "{bed.building} / {bed.unit} / {bed.room} / {bed.bed_label}" }
                                    span { class: format!("chip bed-state {}", bed.state.to_ascii_lowercase().replace(' ', "-")), "{bed.state}" }
                                }
                            }
                        }
                    }
                }
                section { class: "subpanel",
                    h4 { "Transition" }
                    input { placeholder: "Bed ID", value: "{bed_transition_id}", oninput: move |evt| bed_transition_id.set(evt.value()) }
                    input { placeholder: "Action", value: "{bed_transition_action}", oninput: move |evt| bed_transition_action.set(evt.value()) }
                    input { placeholder: "Target state", value: "{bed_transition_state}", oninput: move |evt| bed_transition_state.set(evt.value()) }
                    input { placeholder: "Patient ID (optional)", value: "{bed_transition_patient_id}", oninput: move |evt| bed_transition_patient_id.set(evt.value()) }
                    input { placeholder: "Related bed ID (optional)", value: "{bed_transition_related}", oninput: move |evt| bed_transition_related.set(evt.value()) }
                    input { placeholder: "Note", value: "{bed_transition_note}", oninput: move |evt| bed_transition_note.set(evt.value()) }
                    button {
                        class: "primary",
                        onclick: move |_| {
                            if let Ok(bed_id) = bed_transition_id().parse::<i64>() {
                                let related = bed_transition_related().parse::<i64>().ok();
                                let patient = bed_transition_patient_id().parse::<i64>().ok();
                                let req = BedTransitionRequest {
                                    action: bed_transition_action(),
                                    target_state: bed_transition_state(),
                                    related_bed_id: related,
                                    patient_id: patient,
                                    note: bed_transition_note(),
                                };
                                let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                spawn(async move {
                                    match api::transition_bed(&token, bed_id, req).await {
                                        Ok(_) => status.set("Bed transitioned".to_string()),
                                        Err(e) => error.set(e),
                                    }
                                });
                            } else {
                                error.set("Bed ID must be a number".to_string());
                            }
                        },
                        "Apply Transition"
                    }
                }
            }
            section { class: "subpanel",
                h4 { "Operation History" }
                div { class: "cards bed-events",
                    for evt in bed_events() {
                        article { class: "card",
                            p { "{evt.occurred_at} - {evt.action}" }
                            p { class: "muted", "from {evt.from_bed_id:?} ({evt.from_state:?}) to {evt.to_bed_id:?} ({evt.to_state:?})" }
                            p { class: "muted", "actor {evt.actor_username}" }
                        }
                    }
                }
            }
        }
    }
}
