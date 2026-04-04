use dioxus::prelude::*;

use contracts::{
    OrderCreateRequest, OrderDto, OrderNoteDto, OrderNoteRequest, TicketSplitDto,
    TicketSplitRequest,
};

use crate::api;
use crate::features::orders::{
    friendly_order_status_error, order_status_request, transition_requires_reason, ORDER_STATUSES,
};
use crate::state::SessionContext;

#[component]
pub fn OrdersPage(
    mut status: Signal<String>,
    mut error: Signal<String>,
    session: Signal<Option<SessionContext>>,
    mut menus: Signal<Vec<contracts::DiningMenuDto>>,
    mut orders: Signal<Vec<OrderDto>>,
    mut order_patient_id: Signal<String>,
    mut order_menu_id: Signal<String>,
    mut order_notes: Signal<String>,
    mut order_status_id: Signal<String>,
    mut order_status_value: Signal<String>,
    mut order_status_reason: Signal<String>,
    mut order_note_id: Signal<String>,
    mut order_note_text: Signal<String>,
    mut order_split_id: Signal<String>,
    mut order_split_by: Signal<String>,
    mut order_split_value: Signal<String>,
    mut order_split_quantity: Signal<String>,
    mut order_note_timeline: Signal<Vec<OrderNoteDto>>,
    mut order_split_timeline: Signal<Vec<TicketSplitDto>>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Order Operations" }
            button {
                class: "primary",
                onclick: move |_| {
                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                    spawn(async move {
                        if let Ok(items) = api::list_menus(&token).await { menus.set(items); }
                        if let Ok(items) = api::list_orders(&token).await { orders.set(items); }
                    });
                },
                "Refresh Menus & Orders"
            }
            section { class: "subpanel",
                h4 { "Place Order" }
                input { placeholder: "Patient ID", value: "{order_patient_id}", oninput: move |evt| order_patient_id.set(evt.value()) }
                input { placeholder: "Menu ID", value: "{order_menu_id}", oninput: move |evt| order_menu_id.set(evt.value()) }
                textarea { placeholder: "Notes", value: "{order_notes}", oninput: move |evt| order_notes.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let (Ok(patient_id), Ok(menu_id)) = (order_patient_id().parse::<i64>(), order_menu_id().parse::<i64>()) {
                            let req = OrderCreateRequest {
                                patient_id,
                                menu_id,
                                notes: order_notes(),
                                idempotency_key: None,
                            };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::place_order(&token, req).await {
                                    Ok(id) => {
                                        api::track_ui_event(&token, "ui_instrumentation", "order.place", &format!("{{\"order_id\":{id}}}"));
                                        status.set(format!("Order placed #{id}"));
                                    }
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Place"
                }
            }
            section { class: "subpanel",
                h4 { "Order Status + Notes + Ticket Splits" }
                input { placeholder: "Order ID", value: "{order_status_id}", oninput: move |evt| order_status_id.set(evt.value()) }
                select {
                    value: "{order_status_value}",
                    onchange: move |evt| order_status_value.set(evt.value()),
                    for status_option in ORDER_STATUSES {
                        option { value: "{status_option}", "{status_option}" }
                    }
                }
                textarea {
                    placeholder: "Reason (required for Canceled and Credited)",
                    value: "{order_status_reason}",
                    oninput: move |evt| order_status_reason.set(evt.value())
                }
                if transition_requires_reason(&order_status_value()) {
                    p { class: "muted", "Reason is required for {order_status_value()} transitions." }
                }
                button {
                    onclick: move |_| {
                        let build = order_status_request(
                            &order_status_id(),
                            &order_status_value(),
                            &order_status_reason(),
                            &orders(),
                        );
                        let (order_id, req) = match build {
                            Ok(v) => v,
                            Err(msg) => {
                                error.set(msg);
                                return;
                            }
                        };
                        let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                        spawn(async move {
                            match api::set_order_status(&token, order_id, req).await {
                                Ok(_) => {
                                    status.set("Order status updated".to_string());
                                    order_status_reason.set(String::new());
                                }
                                Err(e) => error.set(friendly_order_status_error(&e)),
                            }
                        });
                    },
                    "Update Status"
                }
                input { placeholder: "Order ID for split", value: "{order_split_id}", oninput: move |evt| order_split_id.set(evt.value()) }
                input { placeholder: "Split by", value: "{order_split_by}", oninput: move |evt| order_split_by.set(evt.value()) }
                input { placeholder: "Split value", value: "{order_split_value}", oninput: move |evt| order_split_value.set(evt.value()) }
                input { placeholder: "Quantity", value: "{order_split_quantity}", oninput: move |evt| order_split_quantity.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let (Ok(order_id), Ok(quantity)) = (order_split_id().parse::<i64>(), order_split_quantity().parse::<i32>()) {
                            let req = TicketSplitRequest { split_by: order_split_by(), split_value: order_split_value(), quantity };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::add_ticket_split(&token, order_id, req).await {
                                    Ok(_) => status.set("Ticket split added".to_string()),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Add Ticket Split"
                }
                input { placeholder: "Order ID for note", value: "{order_note_id}", oninput: move |evt| order_note_id.set(evt.value()) }
                textarea { placeholder: "Operation note", value: "{order_note_text}", oninput: move |evt| order_note_text.set(evt.value()) }
                button {
                    onclick: move |_| {
                        if let Ok(order_id) = order_note_id().parse::<i64>() {
                            let req = OrderNoteRequest { note: order_note_text() };
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::add_order_note(&token, order_id, req).await {
                                    Ok(_) => status.set("Order note added".to_string()),
                                    Err(e) => error.set(e),
                                }
                            });
                        }
                    },
                    "Add Note"
                }
                button {
                    class: "primary",
                    onclick: move |_| {
                        if let Ok(order_id) = order_note_id().parse::<i64>() {
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                if let Ok(items) = api::list_order_notes(&token, order_id).await {
                                    order_note_timeline.set(items);
                                }
                                if let Ok(items) = api::list_ticket_splits(&token, order_id).await {
                                    order_split_timeline.set(items);
                                }
                            });
                        }
                    },
                    "Load Operations Timeline"
                }
            }
            div { class: "cards", for m in menus() { article { class: "card", p { "Menu #{m.id}: {m.item_name} ({m.meal_period})" } } } }
            div { class: "cards", for o in orders() { article { class: "card", p { "Order #{o.id} patient {o.patient_id} menu {o.menu_id}" } p { class: "muted", "{o.status} - {o.notes}" } } } }
            section { class: "subpanel",
                h4 { "Operations Timeline" }
                div { class: "cards",
                    for note in order_note_timeline() {
                        article { class: "card",
                            p { "{note.created_at} - {note.staff_username}" }
                            p { class: "muted", "{note.note}" }
                        }
                    }
                    for split in order_split_timeline() {
                        article { class: "card",
                            p { "Ticket split #{split.id}" }
                            p { class: "muted", "{split.split_by}: {split.split_value} x{split.quantity}" }
                        }
                    }
                }
            }
        }
    }
}
