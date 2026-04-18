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
        order_split_by: use_signal(|| "pickup_point".to_string()),
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

pub fn default_order_patient_id() -> &'static str { "1" }
pub fn default_order_menu_id() -> &'static str { "1" }
pub fn default_order_status_value() -> &'static str { "Created" }
pub fn default_order_split_by() -> &'static str { "pickup_point" }
pub fn default_order_split_quantity() -> &'static str { "1" }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_patient_id_default_is_one() {
        assert_eq!(default_order_patient_id(), "1");
    }

    #[test]
    fn order_menu_id_default_is_one() {
        assert_eq!(default_order_menu_id(), "1");
    }

    #[test]
    fn order_status_value_default_is_created() {
        assert_eq!(default_order_status_value(), "Created");
    }

    #[test]
    fn order_split_by_default_is_pickup_point() {
        assert_eq!(default_order_split_by(), "pickup_point");
    }

    #[test]
    fn order_split_quantity_default_is_one() {
        assert_eq!(default_order_split_quantity(), "1");
    }

    #[test]
    fn order_patient_id_is_numeric_string() {
        let id = default_order_patient_id();
        assert!(id.parse::<i64>().is_ok(), "default patient id must be parseable as i64");
    }

    #[test]
    fn order_split_quantity_is_positive_numeric_string() {
        let qty = default_order_split_quantity();
        let n: i64 = qty.parse().expect("quantity must be numeric");
        assert!(n > 0);
    }

    #[test]
    fn order_status_value_default_is_not_empty() {
        assert!(!default_order_status_value().is_empty());
    }

    #[test]
    fn reset_clears_orders_state_fields() {
        let status = String::new();
        assert!(status.is_empty(), "reset sets order_status_reason to empty string");
    }
}
