use contracts::{BedDto, BedEventDto};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct BedboardState {
    pub beds: Signal<Vec<BedDto>>,
    pub bed_events: Signal<Vec<BedEventDto>>,
    pub bed_transition_id: Signal<String>,
    pub bed_transition_action: Signal<String>,
    pub bed_transition_state: Signal<String>,
    pub bed_transition_patient_id: Signal<String>,
    pub bed_transition_related: Signal<String>,
    pub bed_transition_note: Signal<String>,
}

pub fn use_bedboard_state() -> BedboardState {
    BedboardState {
        beds: use_signal(Vec::<BedDto>::new),
        bed_events: use_signal(Vec::<BedEventDto>::new),
        bed_transition_id: use_signal(String::new),
        bed_transition_action: use_signal(|| "check-in".to_string()),
        bed_transition_state: use_signal(|| "Occupied".to_string()),
        bed_transition_patient_id: use_signal(String::new),
        bed_transition_related: use_signal(String::new),
        bed_transition_note: use_signal(String::new),
    }
}
