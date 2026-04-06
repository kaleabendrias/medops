use contracts::{
    AttachmentMetadataDto, PatientProfileDto, PatientSearchResultDto, RevisionTimelineDto,
};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct PatientsState {
    pub patient_query: Signal<String>,
    pub patient_results: Signal<Vec<PatientSearchResultDto>>,
    pub selected_patient_id: Signal<Option<i64>>,
    pub patient_profile: Signal<Option<PatientProfileDto>>,
    pub patient_revisions: Signal<Vec<RevisionTimelineDto>>,
    pub patient_attachments: Signal<Vec<AttachmentMetadataDto>>,
    pub demo_first_name: Signal<String>,
    pub demo_last_name: Signal<String>,
    pub demo_birth_date: Signal<String>,
    pub demo_gender: Signal<String>,
    pub demo_phone: Signal<String>,
    pub demo_email: Signal<String>,
    pub demo_reason: Signal<String>,
    pub allergies_value: Signal<String>,
    pub contraindications_value: Signal<String>,
    pub history_value: Signal<String>,
    pub clinical_reason: Signal<String>,
    pub visit_note: Signal<String>,
    pub visit_reason: Signal<String>,
    pub patient_export_format: Signal<String>,
}

pub fn use_patients_state() -> PatientsState {
    PatientsState {
        patient_query: use_signal(String::new),
        patient_results: use_signal(Vec::<PatientSearchResultDto>::new),
        selected_patient_id: use_signal(|| None::<i64>),
        patient_profile: use_signal(|| None::<PatientProfileDto>),
        patient_revisions: use_signal(Vec::<RevisionTimelineDto>::new),
        patient_attachments: use_signal(Vec::<AttachmentMetadataDto>::new),
        demo_first_name: use_signal(String::new),
        demo_last_name: use_signal(String::new),
        demo_birth_date: use_signal(String::new),
        demo_gender: use_signal(String::new),
        demo_phone: use_signal(String::new),
        demo_email: use_signal(String::new),
        demo_reason: use_signal(String::new),
        allergies_value: use_signal(String::new),
        contraindications_value: use_signal(String::new),
        history_value: use_signal(String::new),
        clinical_reason: use_signal(String::new),
        visit_note: use_signal(String::new),
        visit_reason: use_signal(String::new),
        patient_export_format: use_signal(|| "json".to_string()),
    }
}

impl PatientsState {
    pub fn reset(&mut self) {
        self.selected_patient_id.set(None);
        self.patient_results.set(Vec::new());
        self.patient_profile.set(None);
        self.patient_revisions.set(Vec::new());
        self.patient_attachments.set(Vec::new());
    }
}
