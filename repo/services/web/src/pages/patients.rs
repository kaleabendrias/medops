use dioxus::prelude::*;
use dioxus::html::HasFileData;

use contracts::{
    AttachmentMetadataDto, ClinicalEditRequest, PatientProfileDto, PatientSearchResultDto,
    RevisionTimelineDto, VisitNoteRequest,
};

use crate::api;
use crate::features::clinical::{
    build_demographics_request, revisions_include_sensitive_values, DemographicsForm,
};
use crate::features::revisions::deltas_for;
use crate::features::session::can_reveal_revision_fields;
use crate::state::SessionContext;
use crate::ui_logic::{
    can_submit_upload, queue_dropped_attachment, upload_state_after_attempt, QueuedAttachment,
    UploadState,
};

#[component]
pub fn PatientsPage(
    mut status: Signal<String>,
    mut error: Signal<String>,
    session: Signal<Option<SessionContext>>,
    mut patient_query: Signal<String>,
    mut patient_results: Signal<Vec<PatientSearchResultDto>>,
    mut selected_patient_id: Signal<Option<i64>>,
    mut patient_profile: Signal<Option<PatientProfileDto>>,
    mut patient_revisions: Signal<Vec<RevisionTimelineDto>>,
    mut patient_attachments: Signal<Vec<AttachmentMetadataDto>>,
    mut demo_first_name: Signal<String>,
    mut demo_last_name: Signal<String>,
    mut demo_birth_date: Signal<String>,
    mut demo_gender: Signal<String>,
    mut demo_phone: Signal<String>,
    mut demo_email: Signal<String>,
    mut demo_reason: Signal<String>,
    mut allergies_value: Signal<String>,
    mut contraindications_value: Signal<String>,
    mut history_value: Signal<String>,
    mut clinical_reason: Signal<String>,
    mut visit_note: Signal<String>,
    mut visit_reason: Signal<String>,
    mut patient_export_format: Signal<String>,
    mut upload_progress: Signal<u8>,
    mut upload_state: Signal<String>,
    mut upload_state_kind: Signal<UploadState>,
    mut attachment_queue: Signal<Vec<QueuedAttachment>>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Patient Workspace" }
            div { class: "row",
                input { placeholder: "Search by MRN or name", value: "{patient_query}", oninput: move |evt| patient_query.set(evt.value()) }
                button {
                    class: "primary",
                    onclick: move |_| {
                        let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                        spawn(async move {
                            error.set(String::new());
                            match api::search_patients(&token, &patient_query()).await {
                                Ok(items) => {
                                    api::track_ui_event(&token, "ui_instrumentation", "patient.search", &format!("{{\"result_count\":{}}}", items.len()));
                                    patient_results.set(items);
                                }
                                Err(e) => error.set(e),
                            }
                        });
                    },
                    "Search"
                }
            }
            div { class: "cards",
                for patient in patient_results() {
                    button {
                        class: "card left",
                        onclick: move |_| {
                            let pid = patient.id;
                            selected_patient_id.set(Some(pid));
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            spawn(async move {
                                match api::get_patient(&token, pid).await {
                                    Ok(profile) => {
                                        demo_first_name.set(profile.first_name.clone());
                                        demo_last_name.set(profile.last_name.clone());
                                        demo_birth_date.set(profile.birth_date.clone());
                                        demo_gender.set(profile.gender.clone());
                                        demo_phone.set(profile.phone.clone());
                                        demo_email.set(profile.email.clone());
                                        allergies_value.set(profile.allergies.clone());
                                        contraindications_value.set(profile.contraindications.clone());
                                        history_value.set(profile.history.clone());
                                        patient_profile.set(Some(profile));
                                    }
                                    Err(e) => error.set(e),
                                }
                                let can_reveal = session()
                                    .as_ref()
                                    .map(|s| can_reveal_revision_fields(&s.stored.role))
                                    .unwrap_or(false);
                                if let Ok(items) = api::patient_revisions(&token, pid, can_reveal).await { patient_revisions.set(items); }
                                if let Ok(items) = api::list_attachments(&token, pid).await { patient_attachments.set(items); }
                            });
                        },
                        strong { "{patient.display_name}" }
                        p { class: "muted", "MRN: {patient.mrn}" }
                    }
                }
            }

            if let Some(profile) = patient_profile() {
                div { class: "grid-2",
                    section { class: "subpanel",
                        h4 { "Demographics" }
                        p { class: "muted", "Patient #{profile.id}" }
                        input { placeholder: "First name", value: "{demo_first_name}", oninput: move |evt| demo_first_name.set(evt.value()) }
                        input { placeholder: "Last name", value: "{demo_last_name}", oninput: move |evt| demo_last_name.set(evt.value()) }
                        input { placeholder: "Birth date (YYYY-MM-DD)", value: "{demo_birth_date}", oninput: move |evt| demo_birth_date.set(evt.value()) }
                        input { placeholder: "Gender", value: "{demo_gender}", oninput: move |evt| demo_gender.set(evt.value()) }
                        input { placeholder: "Phone", value: "{demo_phone}", oninput: move |evt| demo_phone.set(evt.value()) }
                        input { placeholder: "Email", value: "{demo_email}", oninput: move |evt| demo_email.set(evt.value()) }
                        input { placeholder: "Reason for change", value: "{demo_reason}", oninput: move |evt| demo_reason.set(evt.value()) }
                        button {
                            class: "primary",
                            onclick: move |_| {
                                if let Some(pid) = selected_patient_id() {
                                    let req = match build_demographics_request(DemographicsForm {
                                        first_name: &demo_first_name(),
                                        last_name: &demo_last_name(),
                                        birth_date: &demo_birth_date(),
                                        gender: &demo_gender(),
                                        phone: &demo_phone(),
                                        email: &demo_email(),
                                        reason_for_change: &demo_reason(),
                                    }) {
                                        Ok(v) => v,
                                        Err(msg) => {
                                            error.set(msg);
                                            return;
                                        }
                                    };
                                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                    spawn(async move {
                                        match api::update_patient(&token, pid, req).await {
                                            Ok(_) => status.set("Patient demographics updated".to_string()),
                                            Err(e) => error.set(e),
                                        }
                                    });
                                }
                            },
                            "Save Demographics"
                        }
                    }
                    section { class: "subpanel",
                        h4 { "Clinical Fields" }
                        textarea { placeholder: "Allergies", value: "{allergies_value}", oninput: move |evt| allergies_value.set(evt.value()) }
                        textarea { placeholder: "Contraindications", value: "{contraindications_value}", oninput: move |evt| contraindications_value.set(evt.value()) }
                        textarea { placeholder: "History", value: "{history_value}", oninput: move |evt| history_value.set(evt.value()) }
                        input { placeholder: "Reason for change (required)", value: "{clinical_reason}", oninput: move |evt| clinical_reason.set(evt.value()) }
                        button {
                            class: "primary",
                            onclick: move |_| {
                                if let Some(pid) = selected_patient_id() {
                                    let reason = clinical_reason();
                                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                    let allergies = allergies_value();
                                    let contraindications = contraindications_value();
                                    let history = history_value();
                                    spawn(async move {
                                        let a = api::edit_clinical_field(&token, pid, "allergies", ClinicalEditRequest { value: allergies, reason_for_change: reason.clone() }).await;
                                        let c = api::edit_clinical_field(&token, pid, "contraindications", ClinicalEditRequest { value: contraindications, reason_for_change: reason.clone() }).await;
                                        let h = api::edit_clinical_field(&token, pid, "history", ClinicalEditRequest { value: history, reason_for_change: reason }).await;
                                        if a.is_ok() && c.is_ok() && h.is_ok() {
                                            status.set("Clinical fields updated with revision reasons".to_string());
                                        } else {
                                            error.set("Failed to update one or more clinical fields".to_string());
                                        }
                                    });
                                }
                            },
                            "Save Clinical Fields"
                        }
                        textarea { placeholder: "Visit note", value: "{visit_note}", oninput: move |evt| visit_note.set(evt.value()) }
                        input { placeholder: "Visit note reason", value: "{visit_reason}", oninput: move |evt| visit_reason.set(evt.value()) }
                        button {
                            onclick: move |_| {
                                if let Some(pid) = selected_patient_id() {
                                    let req = VisitNoteRequest { note: visit_note(), reason_for_change: visit_reason() };
                                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                    spawn(async move {
                                        match api::add_visit_note(&token, pid, req).await {
                                            Ok(_) => status.set("Visit note saved".to_string()),
                                            Err(e) => error.set(e),
                                        }
                                    });
                                }
                            },
                            "Add Visit Note"
                        }
                    }
                }

                section { class: "subpanel",
                    h4 { "Attachment Panel" }
                    p { class: "muted", "Drop files on the picker or select files. Queue is binary-safe (PDF/JPG/PNG, <= 25 MB)." }
                    div {
                        class: "card left",
                        ondragover: move |evt| {
                            evt.prevent_default();
                            upload_state.set("Drop files on the picker to queue them".to_string());
                        },
                        ondrop: move |evt| {
                            evt.prevent_default();
                            if let Some(files) = evt.files() {
                                let names = files.files();
                                for name in names {
                                    let file_name = name.clone();
                                    let files = files.clone();
                                    spawn(async move {
                                        if let Some(contents) = files.read_file(&file_name).await {
                                            let guessed_mime = if file_name.to_ascii_lowercase().ends_with(".pdf") {
                                                "application/pdf"
                                            } else if file_name.to_ascii_lowercase().ends_with(".png") {
                                                "image/png"
                                            } else {
                                                "image/jpeg"
                                            };
                                            let mut queue = attachment_queue();
                                            match queue_dropped_attachment(
                                                &mut queue,
                                                &file_name,
                                                guessed_mime,
                                                contents,
                                            ) {
                                                Ok(_) => {
                                                    attachment_queue.set(queue);
                                                    upload_state.set(format!("Queued {file_name}"));
                                                    upload_state_kind.set(UploadState::Idle);
                                                    error.set(String::new());
                                                }
                                                Err(e) => error.set(e),
                                            }
                                        } else {
                                            error.set(format!("Failed to read dropped file {file_name}"));
                                        }
                                    });
                                }
                            }
                        },
                        strong { "Drop + Queue" }
                        input {
                            r#type: "file",
                            multiple: true,
                            accept: ".pdf,.jpg,.jpeg,.png",
                            onchange: move |evt| {
                                if let Some(files) = evt.files() {
                                    let names = files.files();
                                    for name in names {
                                        let file_name = name.clone();
                                        let files = files.clone();
                                        spawn(async move {
                                            if let Some(contents) = files.read_file(&file_name).await {
                                                let guessed_mime = if file_name.to_ascii_lowercase().ends_with(".pdf") {
                                                    "application/pdf"
                                                } else if file_name.to_ascii_lowercase().ends_with(".png") {
                                                    "image/png"
                                                } else {
                                                    "image/jpeg"
                                                };
                                                let mut queue = attachment_queue();
                                                match queue_dropped_attachment(
                                                    &mut queue,
                                                    &file_name,
                                                    guessed_mime,
                                                    contents,
                                                ) {
                                                    Ok(_) => {
                                                        attachment_queue.set(queue);
                                                        upload_state.set(format!("Queued {file_name}"));
                                                        upload_state_kind.set(UploadState::Idle);
                                                        error.set(String::new());
                                                    }
                                                    Err(e) => error.set(e),
                                                }
                                            } else {
                                                error.set(format!("Failed to read selected file {file_name}"));
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }
                    button {
                        class: "primary",
                        onclick: move |_| {
                            if !can_submit_upload(
                                selected_patient_id(),
                                matches!(upload_state_kind(), UploadState::Uploading),
                                attachment_queue().len(),
                            ) {
                                return;
                            }
                            let Some(pid) = selected_patient_id() else {
                                return;
                            };
                            let queued = attachment_queue();
                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                            upload_state_kind.set(UploadState::Uploading);
                            upload_progress.set(0);
                            upload_state.set("Uploading queued files...".to_string());
                            error.set(String::new());

                            spawn(async move {
                                let total = queued.len().max(1);
                                for (idx, item) in queued.clone().into_iter().enumerate() {
                                    if let Err(e) = api::upload_attachment(
                                        &token,
                                        pid,
                                        &item.file_name,
                                        &item.mime_type,
                                        item.bytes,
                                    )
                                    .await
                                    {
                                        upload_progress.set(0);
                                        upload_state_kind.set(upload_state_after_attempt(
                                            UploadState::Uploading,
                                            false,
                                        ));
                                        upload_state.set("Upload failed".to_string());
                                        error.set(e);
                                        return;
                                    }
                                    let pct = (((idx + 1) * 100) / total) as u8;
                                    upload_progress.set(pct);
                                }

                                attachment_queue.set(Vec::new());
                                upload_state_kind.set(upload_state_after_attempt(
                                    UploadState::Uploading,
                                    true,
                                ));
                                upload_state.set("Upload complete".to_string());
                                status.set("Attachment batch uploaded".to_string());
                                if let Ok(items) = api::list_attachments(&token, pid).await {
                                    patient_attachments.set(items);
                                }
                            });
                        },
                        disabled: !can_submit_upload(
                            selected_patient_id(),
                            matches!(upload_state_kind(), UploadState::Uploading),
                            attachment_queue().len(),
                        ),
                        "Upload Queued Attachments"
                    }
                    if !attachment_queue().is_empty() {
                        button {
                            class: "danger",
                            onclick: move |_| {
                                attachment_queue.set(Vec::new());
                                upload_progress.set(0);
                                upload_state_kind.set(UploadState::Idle);
                                upload_state.set("Queue cleared".to_string());
                            },
                            disabled: matches!(upload_state_kind(), UploadState::Uploading),
                            "Clear Queue"
                        }
                    }
                    div { class: "cards",
                        for item in attachment_queue() {
                            article { class: "card",
                                strong { "{item.file_name}" }
                                p { class: "muted", "{item.mime_type} / {item.bytes.len()} bytes queued" }
                            }
                        }
                    }
                    p { class: "muted", "Progress: {upload_progress}%" }
                    if !upload_state().is_empty() { p { class: "muted", "{upload_state}" } }
                    div { class: "row",
                        input {
                            placeholder: "Export format (json/csv)",
                            value: "{patient_export_format}",
                            oninput: move |evt| patient_export_format.set(evt.value())
                        }
                        button {
                            onclick: move |_| {
                                if let Some(pid) = selected_patient_id() {
                                    let fmt = patient_export_format();
                                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                    let can_reveal = session()
                                        .as_ref()
                                        .map(|s| can_reveal_revision_fields(&s.stored.role))
                                        .unwrap_or(false);
                                    spawn(async move {
                                        match api::export_patient(&token, pid, &fmt, can_reveal).await {
                                            Ok(_) => {
                                                status.set("Patient export generated and audited".to_string());
                                                error.set(String::new());
                                            }
                                            Err(e) => error.set(e),
                                        }
                                    });
                                }
                            },
                            "Export Patient"
                        }
                    }
                    div { class: "cards",
                        for file in patient_attachments() {
                            article { class: "card",
                                strong { "{file.file_name}" }
                                p { class: "muted", "{file.mime_type} / {file.file_size_bytes} bytes" }
                                p { class: "muted", "by {file.uploaded_by} at {file.uploaded_at}" }
                            }
                        }
                    }
                }

                section { class: "subpanel",
                    h4 { "Revision Timeline" }
                    p {
                        class: "muted",
                        if revisions_include_sensitive_values(&patient_revisions(), "allergies")
                            || revisions_include_sensitive_values(&patient_revisions(), "contraindications")
                            || revisions_include_sensitive_values(&patient_revisions(), "history")
                        {
                            "Sensitive values are visible for your role."
                        } else {
                            "Sensitive values remain masked for your role."
                        }
                    }
                    div { class: "cards",
                        for rev in patient_revisions() {
                            article { class: "card",
                                p { "{rev.created_at} - {rev.entity_type}" }
                                p { class: "muted", "Reason: {rev.reason_for_change}" }
                                p { class: "muted", "Actor: {rev.actor_username}" }
                                for delta in deltas_for(&rev) {
                                    div { class: "delta",
                                        p { class: "muted", "{delta.field}" }
                                        p { "{delta.before} -> {delta.after}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
