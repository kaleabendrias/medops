use dioxus::prelude::*;

use contracts::{CampaignCreateRequest, CampaignDto};

use crate::api;
use crate::state::SessionContext;

#[component]
pub fn CampaignsPage(
    mut status: Signal<String>,
    mut error: Signal<String>,
    session: Signal<Option<SessionContext>>,
    mut campaigns: Signal<Vec<CampaignDto>>,
    mut campaign_title: Signal<String>,
    mut campaign_dish_id: Signal<String>,
    mut campaign_threshold: Signal<String>,
    mut campaign_deadline: Signal<String>,
    mut campaign_join_id: Signal<String>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Group-Buy Campaigns" }
            button {
                class: "primary",
                onclick: move |_| {
                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                    spawn(async move {
                        match api::list_campaigns(&token).await {
                            Ok(items) => campaigns.set(items),
                            Err(e) => error.set(e),
                        }
                    });
                },
                "Refresh Campaigns"
            }
            section { class: "subpanel",
                h4 { "Create Campaign" }
                input { placeholder: "Title", value: "{campaign_title}", oninput: move |evt| campaign_title.set(evt.value()) }
                input { placeholder: "Dish ID", value: "{campaign_dish_id}", oninput: move |evt| campaign_dish_id.set(evt.value()) }
                input { placeholder: "Success threshold", value: "{campaign_threshold}", oninput: move |evt| campaign_threshold.set(evt.value()) }
                input { placeholder: "Deadline UTC (YYYY-MM-DD HH:MM:SS)", value: "{campaign_deadline}", oninput: move |evt| campaign_deadline.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let (Ok(dish_id), Ok(success_threshold)) = (campaign_dish_id().parse::<i64>(), campaign_threshold().parse::<i32>()) {
                            let req = CampaignCreateRequest {
                                title: campaign_title(),
                                dish_id,
                                success_threshold,
                                success_deadline_at: campaign_deadline(),
                            };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::create_campaign(&token, req).await {
                                    Ok(id) => status.set(format!("Campaign created #{id}")),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Create"
                }
            }
            section { class: "subpanel",
                h4 { "Join Campaign" }
                input { placeholder: "Campaign ID", value: "{campaign_join_id}", oninput: move |evt| campaign_join_id.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let Ok(id) = campaign_join_id().parse::<i64>() {
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::join_campaign(&token, id).await {
                                    Ok(_) => {
                                        api::track_ui_event(&token, "ui_instrumentation", "campaign.join", &format!("{{\"campaign_id\":{id}}}"));
                                        status.set("Joined campaign".to_string());
                                    }
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Join"
                }
            }
            div { class: "cards",
                for c in campaigns() {
                    article { class: "card",
                        strong { "{c.title}" }
                        p { class: "muted", "Dish #{c.dish_id} / threshold {c.success_threshold} / deadline {c.success_deadline_at}" }
                        p { class: "muted", "{c.status} / participants {c.participants} / qualifying orders {c.qualifying_orders}" }
                    }
                }
            }
        }
    }
}
