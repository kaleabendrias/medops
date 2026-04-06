use contracts::{DiningMenuDto, OrderDto, OrderNoteDto, TicketSplitDto};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct OrdersState {
    pub menus: Signal<Vec<DiningMenuDto>>,
    pub orders: Signal<Vec<OrderDto>>,
    pub order_patient_id: Signal<String>,
    pub order_menu_id: Signal<String>,
    pub order_notes: Signal<String>,
    pub order_status_id: Signal<String>,
    pub order_status_value: Signal<String>,
    pub order_status_reason: Signal<String>,
    pub order_note_id: Signal<String>,
    pub order_note_text: Signal<String>,
    pub order_split_id: Signal<String>,
    pub order_split_by: Signal<String>,
    pub order_split_value: Signal<String>,
    pub order_split_quantity: Signal<String>,
    pub order_note_timeline: Signal<Vec<OrderNoteDto>>,
    pub order_split_timeline: Signal<Vec<TicketSplitDto>>,
}

pub fn use_orders_state() -> OrdersState {
    OrdersState {
        menus: use_signal(Vec::<DiningMenuDto>::new),
        orders: use_signal(Vec::<OrderDto>::new),
        order_patient_id: use_signal(|| "1".to_string()),
        order_menu_id: use_signal(|| "1".to_string()),
        order_notes: use_signal(String::new),
        order_status_id: use_signal(String::new),
        order_status_value: use_signal(|| "Created".to_string()),
        order_status_reason: use_signal(String::new),
        order_note_id: use_signal(String::new),
        order_note_text: use_signal(String::new),
        order_split_id: use_signal(String::new),
        order_split_by: use_signal(|| "room".to_string()),
        order_split_value: use_signal(String::new),
        order_split_quantity: use_signal(|| "1".to_string()),
        order_note_timeline: use_signal(Vec::<OrderNoteDto>::new),
        order_split_timeline: use_signal(Vec::<TicketSplitDto>::new),
    }
}

impl OrdersState {
    pub fn reset(&mut self) {
        self.orders.set(Vec::new());
        self.order_note_timeline.set(Vec::new());
        self.order_split_timeline.set(Vec::new());
        self.order_status_reason.set(String::new());
    }
}
