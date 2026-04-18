use dioxus::prelude::*;

use contracts::{
    DishCategoryDto, DishCreateRequest, DishDto, DishOptionRequest, DishStatusRequest,
    DishWindowRequest, RankingRuleDto, RankingRuleRequest, RecommendationDto,
};

use crate::api;
use crate::state::SessionContext;

#[component]
pub fn DiningPage(
    mut status: Signal<String>,
    mut error: Signal<String>,
    session: Signal<Option<SessionContext>>,
    mut categories: Signal<Vec<DishCategoryDto>>,
    mut dishes: Signal<Vec<DishDto>>,
    mut ranking_rules: Signal<Vec<RankingRuleDto>>,
    mut recommendations: Signal<Vec<RecommendationDto>>,
    mut dish_category_id: Signal<String>,
    mut dish_name: Signal<String>,
    mut dish_description: Signal<String>,
    mut dish_price: Signal<String>,
    mut dish_photo_path: Signal<String>,
    mut dish_status_id: Signal<String>,
    mut dish_published: Signal<bool>,
    mut dish_sold_out: Signal<bool>,
    mut dish_option_id: Signal<String>,
    mut dish_option_group: Signal<String>,
    mut dish_option_value: Signal<String>,
    mut dish_option_delta: Signal<String>,
    mut dish_window_id: Signal<String>,
    mut dish_window_slot: Signal<String>,
    mut dish_window_start: Signal<String>,
    mut dish_window_end: Signal<String>,
    mut ranking_rule_key: Signal<String>,
    mut ranking_rule_weight: Signal<String>,
    mut ranking_rule_enabled: Signal<bool>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Cafeteria Manager" }
            button {
                class: "primary",
                onclick: move |_| {
                    let token = session().as_ref().map(|s| s.stored.csrf_token.clone()).unwrap_or_default();
                    spawn(async move {
                        if let Ok(items) = api::list_dish_categories(&token).await { categories.set(items); }
                        if let Ok(items) = api::list_dishes(&token).await { dishes.set(items); }
                        if let Ok(items) = api::ranking_rules(&token).await { ranking_rules.set(items); }
                        if let Ok(items) = api::recommendations(&token).await { recommendations.set(items); }
                    });
                },
                "Refresh Dining Data"
            }
            div { class: "grid-2",
                section { class: "subpanel",
                    h4 { "Create Dish" }
                    input { placeholder: "Category ID", value: "{dish_category_id}", oninput: move |evt| dish_category_id.set(evt.value()) }
                    input { placeholder: "Name", value: "{dish_name}", oninput: move |evt| dish_name.set(evt.value()) }
                    textarea { placeholder: "Description", value: "{dish_description}", oninput: move |evt| dish_description.set(evt.value()) }
                    input { placeholder: "Price cents", value: "{dish_price}", oninput: move |evt| dish_price.set(evt.value()) }
                    input { placeholder: "Photo path", value: "{dish_photo_path}", oninput: move |evt| dish_photo_path.set(evt.value()) }
                    button {
                        class: "primary",
                        onclick: move |_| {
                            let cid = dish_category_id().parse::<i64>();
                            let price = dish_price().parse::<i32>();
                            if let (Ok(category_id), Ok(base_price_cents)) = (cid, price) {
                                let req = DishCreateRequest {
                                    category_id,
                                    name: dish_name(),
                                    description: dish_description(),
                                    base_price_cents,
                                    photo_path: dish_photo_path(),
                                };
                                let token = session().as_ref().map(|s| s.stored.csrf_token.clone()).unwrap_or_default();
                                spawn(async move {
                                    match api::create_dish(&token, req).await {
                                        Ok(id) => status.set(format!("Dish created #{id}")),
                                        Err(e) => error.set(e),
                                    }
                                });
                            } else {
                                error.set("Category ID and price must be numbers".to_string());
                            }
                        },
                        "Create Dish"
                    }
                }
                section { class: "subpanel",
                    h4 { "Dish Lifecycle" }
                    input { placeholder: "Dish ID", value: "{dish_status_id}", oninput: move |evt| dish_status_id.set(evt.value()) }
                    label { input { r#type: "checkbox", checked: dish_published(), onchange: move |evt| dish_published.set(evt.checked()) } " published" }
                    label { input { r#type: "checkbox", checked: dish_sold_out(), onchange: move |evt| dish_sold_out.set(evt.checked()) } " sold out" }
                    button {
                        onclick: move |_| {
                            if let Ok(id) = dish_status_id().parse::<i64>() {
                                let req = DishStatusRequest { is_published: dish_published(), is_sold_out: dish_sold_out() };
                                let token = session().as_ref().map(|s| s.stored.csrf_token.clone()).unwrap_or_default();
                                spawn(async move {
                                    match api::set_dish_status(&token, id, req).await {
                                        Ok(_) => status.set("Dish status updated".to_string()),
                                        Err(e) => error.set(e),
                                    }
                                });
                            }
                        },
                        "Update Status"
                    }
                    input { placeholder: "Dish ID for option", value: "{dish_option_id}", oninput: move |evt| dish_option_id.set(evt.value()) }
                    input { placeholder: "Option group", value: "{dish_option_group}", oninput: move |evt| dish_option_group.set(evt.value()) }
                    input { placeholder: "Option value", value: "{dish_option_value}", oninput: move |evt| dish_option_value.set(evt.value()) }
                    input { placeholder: "Delta cents", value: "{dish_option_delta}", oninput: move |evt| dish_option_delta.set(evt.value()) }
                    button {
                        onclick: move |_| {
                            if let (Ok(id), Ok(delta)) = (dish_option_id().parse::<i64>(), dish_option_delta().parse::<i32>()) {
                                let req = DishOptionRequest { option_group: dish_option_group(), option_value: dish_option_value(), delta_price_cents: delta };
                                let token = session().as_ref().map(|s| s.stored.csrf_token.clone()).unwrap_or_default();
                                spawn(async move {
                                    match api::add_dish_option(&token, id, req).await {
                                        Ok(_) => status.set("Dish option added".to_string()),
                                        Err(e) => error.set(e),
                                    }
                                });
                            }
                        },
                        "Add Option"
                    }
                    input { placeholder: "Dish ID for window", value: "{dish_window_id}", oninput: move |evt| dish_window_id.set(evt.value()) }
                    input { placeholder: "Slot", value: "{dish_window_slot}", oninput: move |evt| dish_window_slot.set(evt.value()) }
                    input { placeholder: "Start HH:MM", value: "{dish_window_start}", oninput: move |evt| dish_window_start.set(evt.value()) }
                    input { placeholder: "End HH:MM", value: "{dish_window_end}", oninput: move |evt| dish_window_end.set(evt.value()) }
                    button {
                        onclick: move |_| {
                            if let Ok(id) = dish_window_id().parse::<i64>() {
                                let req = DishWindowRequest { slot_name: dish_window_slot(), start_hhmm: dish_window_start(), end_hhmm: dish_window_end() };
                                let token = session().as_ref().map(|s| s.stored.csrf_token.clone()).unwrap_or_default();
                                spawn(async move {
                                    match api::add_sales_window(&token, id, req).await {
                                        Ok(_) => status.set("Sales window added".to_string()),
                                        Err(e) => error.set(e),
                                    }
                                });
                            }
                        },
                        "Add Window"
                    }
                }
            }
            section { class: "subpanel",
                h4 { "Ranking Rules" }
                input { placeholder: "Rule key", value: "{ranking_rule_key}", oninput: move |evt| ranking_rule_key.set(evt.value()) }
                input { placeholder: "Weight", value: "{ranking_rule_weight}", oninput: move |evt| ranking_rule_weight.set(evt.value()) }
                label { input { r#type: "checkbox", checked: ranking_rule_enabled(), onchange: move |evt| ranking_rule_enabled.set(evt.checked()) } " enabled" }
                button {
                    onclick: move |_| {
                        if let Ok(weight) = ranking_rule_weight().parse::<f64>() {
                            let req = RankingRuleRequest { rule_key: ranking_rule_key(), weight, enabled: ranking_rule_enabled() };
                            let token = session().as_ref().map(|s| s.stored.csrf_token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::upsert_ranking_rule(&token, req).await {
                                    Ok(_) => status.set("Ranking rule updated".to_string()),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Upsert Rule"
                }
                div { class: "cards", for cat in categories() { article { class: "card", strong { "Category #{cat.id}: {cat.name}" } } } }
                div { class: "cards", for dish in dishes() { article { class: "card", strong { "{dish.name}" } p { class: "muted", "{dish.description}" } p { class: "muted", "{dish.base_price_cents} cents" } } } }
                div { class: "cards", for rule in ranking_rules() { article { class: "card", p { "{rule.rule_key}: {rule.weight} (enabled: {rule.enabled})" } } } }
                div { class: "cards", for rec in recommendations() { article { class: "card", p { "Dish #{rec.dish_id} score {rec.score}" } } } }
            }
        }
    }
}
