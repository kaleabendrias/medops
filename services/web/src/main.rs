mod api;
mod components;
mod features;
mod state;
mod ui_logic;

use std::time::Duration;

use contracts::{
    AttachmentMetadataDto, BedDto, BedEventDto, BedTransitionRequest, CampaignCreateRequest,
    CampaignDto, ClinicalEditRequest, DishCategoryDto, DishCreateRequest, DishDto,
    DishOptionRequest, DishStatusRequest, DishWindowRequest, ExperimentAssignRequest,
    ExperimentBacktrackRequest, ExperimentCreateRequest, ExperimentVariantRequest,
    FunnelMetricsDto, IngestionTaskCreateRequest, IngestionTaskDto, IngestionTaskRollbackRequest,
    IngestionTaskRunDto, IngestionTaskUpdateRequest, IngestionTaskVersionDto, OrderCreateRequest,
    OrderDto, OrderNoteDto, OrderNoteRequest, PatientProfileDto,
    PatientSearchResultDto, RankingRuleRequest, RecommendationDto,
    RecommendationKpiDto, RetentionMetricsDto, RevisionTimelineDto, TicketSplitDto,
    TicketSplitRequest, UserSummaryDto, VisitNoteRequest,
};
use dioxus::prelude::*;
use dioxus::html::HasFileData;
use gloo_timers::future::sleep;
use components::app_shell::{AppShell, ShellNavItem};
use components::auth_gate::AuthGate;
use features::clinical::{
    build_demographics_request, revisions_include_sensitive_values, DemographicsForm,
};
use features::guards::resolve_page_access;
use features::navigation::nav_items;
use features::orders::{
    friendly_order_status_error, order_status_request, transition_requires_reason, ORDER_STATUSES,
};
use features::revisions::deltas_for;
use features::session::can_reveal_revision_fields;
use state::{
    can_access, clear_session, ensure_accessible_page, is_user_switch,
    load_session, save_session, session_from_entitlements, Page, SessionContext, StoredSession,
};
use ui_logic::{
    can_submit_upload, queue_dropped_attachment, upload_state_after_attempt,
    QueuedAttachment, UploadState,
};

fn main() {
    launch(App);
}

fn current_hash_page() -> Option<Page> {
    #[cfg(target_arch = "wasm32")]
    {
        let hash = web_sys::window()?.location().hash().ok()?;
        state::page_from_hash(&hash)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

fn set_hash_page(page: Page) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window
                .location()
                .set_hash(state::page_to_hash(page).trim_start_matches('#'));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = page;
    }
}

fn clear_workspace_data(
    mut selected_patient_id: Signal<Option<i64>>,
    mut patient_results: Signal<Vec<PatientSearchResultDto>>,
    mut patient_profile: Signal<Option<PatientProfileDto>>,
    mut patient_revisions: Signal<Vec<RevisionTimelineDto>>,
    mut patient_attachments: Signal<Vec<AttachmentMetadataDto>>,
    mut orders: Signal<Vec<OrderDto>>,
    mut order_note_timeline: Signal<Vec<OrderNoteDto>>,
    mut order_split_timeline: Signal<Vec<TicketSplitDto>>,
    mut order_status_reason: Signal<String>,
    mut ingestion_tasks: Signal<Vec<IngestionTaskDto>>,
    mut ingestion_versions: Signal<Vec<IngestionTaskVersionDto>>,
    mut ingestion_runs: Signal<Vec<IngestionTaskRunDto>>,
    mut attachment_queue: Signal<Vec<QueuedAttachment>>,
    mut upload_state_kind: Signal<UploadState>,
    mut upload_progress: Signal<u8>,
) {
    selected_patient_id.set(None);
    patient_results.set(Vec::new());
    patient_profile.set(None);
    patient_revisions.set(Vec::new());
    patient_attachments.set(Vec::new());
    orders.set(Vec::new());
    order_note_timeline.set(Vec::new());
    order_split_timeline.set(Vec::new());
    order_status_reason.set(String::new());
    ingestion_tasks.set(Vec::new());
    ingestion_versions.set(Vec::new());
    ingestion_runs.set(Vec::new());
    attachment_queue.set(Vec::new());
    upload_state_kind.set(UploadState::Idle);
    upload_progress.set(0);
}

#[component]
fn App() -> Element {
    let mut status = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut session = use_signal(|| None::<SessionContext>);
    let mut page = use_signal(|| Page::Dashboard);

    let mut login_username = use_signal(String::new);
    let mut login_password = use_signal(String::new);

    let mut patient_query = use_signal(String::new);
    let mut patient_results = use_signal(Vec::<PatientSearchResultDto>::new);
    let mut selected_patient_id = use_signal(|| None::<i64>);
    let mut patient_profile = use_signal(|| None::<PatientProfileDto>);
    let mut patient_revisions = use_signal(Vec::<RevisionTimelineDto>::new);
    let mut patient_attachments = use_signal(Vec::<AttachmentMetadataDto>::new);
    let mut demo_first_name = use_signal(String::new);
    let mut demo_last_name = use_signal(String::new);
    let mut demo_birth_date = use_signal(String::new);
    let mut demo_gender = use_signal(String::new);
    let mut demo_phone = use_signal(String::new);
    let mut demo_email = use_signal(String::new);
    let mut demo_reason = use_signal(String::new);
    let mut allergies_value = use_signal(String::new);
    let mut contraindications_value = use_signal(String::new);
    let mut history_value = use_signal(String::new);
    let mut clinical_reason = use_signal(String::new);
    let mut visit_note = use_signal(String::new);
    let mut visit_reason = use_signal(String::new);
    let mut patient_export_format = use_signal(|| "json".to_string());
    let mut beds = use_signal(Vec::<BedDto>::new);
    let mut bed_events = use_signal(Vec::<BedEventDto>::new);
    let mut bed_transition_id = use_signal(String::new);
    let mut bed_transition_action = use_signal(|| "check-in".to_string());
    let mut bed_transition_state = use_signal(|| "Occupied".to_string());
    let mut bed_transition_related = use_signal(String::new);
    let mut bed_transition_note = use_signal(String::new);

    let mut categories = use_signal(Vec::<DishCategoryDto>::new);
    let mut dishes = use_signal(Vec::<DishDto>::new);
    let mut ranking_rules = use_signal(Vec::<contracts::RankingRuleDto>::new);
    let mut recommendations = use_signal(Vec::<RecommendationDto>::new);
    let mut dish_category_id = use_signal(String::new);
    let mut dish_name = use_signal(String::new);
    let mut dish_description = use_signal(String::new);
    let mut dish_price = use_signal(|| "0".to_string());
    let mut dish_photo_path = use_signal(|| "/var/lib/rocket-api/dishes/new.jpg".to_string());
    let mut dish_status_id = use_signal(String::new);
    let mut dish_published = use_signal(|| true);
    let mut dish_sold_out = use_signal(|| false);
    let mut dish_option_id = use_signal(String::new);
    let mut dish_option_group = use_signal(String::new);
    let mut dish_option_value = use_signal(String::new);
    let mut dish_option_delta = use_signal(|| "0".to_string());
    let mut dish_window_id = use_signal(String::new);
    let mut dish_window_slot = use_signal(|| "Lunch".to_string());
    let mut dish_window_start = use_signal(|| "11:00".to_string());
    let mut dish_window_end = use_signal(|| "14:00".to_string());
    let mut ranking_rule_key = use_signal(String::new);
    let mut ranking_rule_weight = use_signal(|| "0.5".to_string());
    let mut ranking_rule_enabled = use_signal(|| true);

    let mut menus = use_signal(Vec::<contracts::DiningMenuDto>::new);
    let mut orders = use_signal(Vec::<OrderDto>::new);
    let mut order_patient_id = use_signal(|| "1".to_string());
    let mut order_menu_id = use_signal(|| "1".to_string());
    let mut order_notes = use_signal(String::new);
    let mut order_status_id = use_signal(String::new);
    let mut order_status_value = use_signal(|| "Created".to_string());
    let mut order_status_reason = use_signal(String::new);
    let mut order_note_id = use_signal(String::new);
    let mut order_note_text = use_signal(String::new);
    let mut order_split_id = use_signal(String::new);
    let mut order_split_by = use_signal(|| "room".to_string());
    let mut order_split_value = use_signal(String::new);
    let mut order_split_quantity = use_signal(|| "1".to_string());
    let mut order_note_timeline = use_signal(Vec::<OrderNoteDto>::new);
    let mut order_split_timeline = use_signal(Vec::<TicketSplitDto>::new);

    let mut upload_progress = use_signal(|| 0u8);
    let mut upload_state = use_signal(String::new);
    let mut upload_state_kind = use_signal(|| UploadState::Idle);
    let mut attachment_queue = use_signal(Vec::<QueuedAttachment>::new);

    let mut ingestion_tasks = use_signal(Vec::<IngestionTaskDto>::new);
    let mut ingestion_versions = use_signal(Vec::<IngestionTaskVersionDto>::new);
    let mut ingestion_runs = use_signal(Vec::<IngestionTaskRunDto>::new);
    let mut ingestion_task_name = use_signal(|| "patient-feed-ui".to_string());
    let mut ingestion_seed_urls = use_signal(|| "file:///app/config/ingestion_fixture/page1.html".to_string());
    let mut ingestion_rules =
        use_signal(|| "{\"mode\":\"css\",\"fields\":[\".record\"],\"pagination_selector\":\"a.next\"}".to_string());
    let mut ingestion_strategy = use_signal(|| "breadth-first".to_string());
    let mut ingestion_depth = use_signal(|| "2".to_string());
    let mut ingestion_incremental_field = use_signal(|| "value".to_string());
    let mut ingestion_schedule = use_signal(|| "0 * * * *".to_string());
    let mut ingestion_selected_task = use_signal(String::new);
    let mut ingestion_rollback_version = use_signal(String::new);
    let mut ingestion_rollback_reason = use_signal(String::new);

    let mut campaigns = use_signal(Vec::<CampaignDto>::new);
    let mut campaign_title = use_signal(String::new);
    let mut campaign_dish_id = use_signal(|| "1".to_string());
    let mut campaign_threshold = use_signal(|| "5".to_string());
    let mut campaign_deadline = use_signal(|| "2099-01-01 10:30:00".to_string());
    let mut campaign_join_id = use_signal(String::new);

    let mut users = use_signal(Vec::<UserSummaryDto>::new);

    let mut experiment_id = use_signal(String::new);
    let mut experiment_key = use_signal(String::new);
    let mut variant_key = use_signal(String::new);
    let mut variant_weight = use_signal(|| "1.0".to_string());
    let mut variant_version = use_signal(|| "v1".to_string());
    let mut assign_user_id = use_signal(|| "1".to_string());
    let mut assign_mode = use_signal(|| "manual".to_string());
    let mut backtrack_from = use_signal(|| "v2".to_string());
    let mut backtrack_to = use_signal(|| "v1".to_string());
    let mut backtrack_reason = use_signal(String::new);
    let mut assigned_variant = use_signal(String::new);

    let mut funnel = use_signal(Vec::<FunnelMetricsDto>::new);
    let mut retention = use_signal(Vec::<RetentionMetricsDto>::new);
    let mut recommendation_kpi = use_signal(|| None::<RecommendationKpiDto>);
    let mut audits = use_signal(Vec::<contracts::AuditLogDto>::new);

    use_future(move || async move {
        if let Some(stored) = load_session() {
            match api::menu_entitlements(&stored.token).await {
                Ok(list) => {
                    let next_session = session_from_entitlements(stored, list);
                    let requested = current_hash_page().unwrap_or(Page::Dashboard);
                    let guarded = ensure_accessible_page(&next_session, requested);
                    set_hash_page(guarded);
                    session.set(Some(next_session));
                    page.set(guarded);
                }
                Err(_) => {
                    clear_session();
                }
            }
        }
    });

    use_future(move || async move {
        loop {
            if let Some(ctx) = session() {
                if can_access(&ctx, Page::Bedboard) {
                    if let Ok(next_beds) = api::list_beds(&ctx.stored.token).await {
                        beds.set(next_beds);
                    }
                    if let Ok(next_events) = api::bed_events(&ctx.stored.token).await {
                        bed_events.set(next_events);
                    }
                }
            }
            sleep(Duration::from_secs(8)).await;
        }
    });

    if session().is_none() {
        return rsx! {
            AuthGate {
                username: login_username(),
                password: login_password(),
                status: status(),
                error: error(),
                on_username: move |value| login_username.set(value),
                on_password: move |value| login_password.set(value),
                on_sign_in: move |_| {
                    spawn(async move {
                        error.set(String::new());
                        status.set("Authenticating...".to_string());
                        match api::login(&login_username(), &login_password()).await {
                            Ok(auth) => {
                                match api::menu_entitlements(&auth.token).await {
                                    Ok(list) => {
                                        let stored = StoredSession {
                                            token: auth.token,
                                            user_id: auth.user_id,
                                            username: auth.username,
                                            role: auth.role,
                                        };
                                        let switched_user = is_user_switch(session().as_ref(), &stored);
                                        clear_workspace_data(
                                            selected_patient_id,
                                            patient_results,
                                            patient_profile,
                                            patient_revisions,
                                            patient_attachments,
                                            orders,
                                            order_note_timeline,
                                            order_split_timeline,
                                            order_status_reason,
                                            ingestion_tasks,
                                            ingestion_versions,
                                            ingestion_runs,
                                            attachment_queue,
                                            upload_state_kind,
                                            upload_progress,
                                        );
                                        save_session(&stored);
                                        let next_session = session_from_entitlements(stored, list);
                                        let requested = if switched_user {
                                            Page::Dashboard
                                        } else {
                                            current_hash_page().unwrap_or(Page::Dashboard)
                                        };
                                        let guarded = ensure_accessible_page(&next_session, requested);
                                        set_hash_page(guarded);
                                        session.set(Some(next_session));
                                        page.set(guarded);
                                        status.set("Signed in".to_string());
                                    }
                                    Err(e) => {
                                        error.set(e);
                                        status.set(String::new());
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(e);
                                status.set(String::new());
                            }
                        }
                    });
                },
            }
        };
    }

    let ctx = session().unwrap_or_else(|| unreachable!());
    let guard = resolve_page_access(&ctx, page());
    let guarded_page = guard.page;
    if guarded_page != page() {
        page.set(guarded_page);
        set_hash_page(guarded_page);
    }
    let forbidden = guard.forbidden;
    let available_nav = nav_items()
        .iter()
        .copied()
        .filter(|item| can_access(&ctx, item.page))
        .map(|item| ShellNavItem {
            page: item.page,
            label: item.label,
        })
        .collect::<Vec<_>>();

    rsx! {
        AppShell {
            username: ctx.stored.username.clone(),
            role: ctx.stored.role.clone(),
            current_page: page(),
            nav_items: available_nav,
            status: status(),
            error: error(),
            on_select_page: move |next_page| {
                page.set(next_page);
                set_hash_page(next_page);
            },
            on_sign_out: move |_| {
                clear_session();
                session.set(None);
                set_hash_page(Page::Dashboard);
                page.set(Page::Dashboard);
                status.set(String::new());
                error.set(String::new());
                clear_workspace_data(
                    selected_patient_id,
                    patient_results,
                    patient_profile,
                    patient_revisions,
                    patient_attachments,
                    orders,
                    order_note_timeline,
                    order_split_timeline,
                    order_status_reason,
                    ingestion_tasks,
                    ingestion_versions,
                    ingestion_runs,
                    attachment_queue,
                    upload_state_kind,
                    upload_progress,
                );
            },

                if forbidden {
                    article { class: "panel", h3 { "Access denied" } p { "Your role does not have this entitlement." } }
                } else if page() == Page::Dashboard {
                    article { class: "panel",
                        h3 { "Role Entitlements" }
                        p { class: "muted", "Navigation and page guards are driven from backend entitlements." }
                        div { class: "chips",
                            for key in ctx.entitlements.iter() { span { class: "chip", "{key}" } }
                        }
                    }
                } else if page() == Page::Patients {
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
                                            Ok(items) => patient_results.set(items),
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
                } else if page() == Page::Bedboard {
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
                                input { placeholder: "Related bed ID (optional)", value: "{bed_transition_related}", oninput: move |evt| bed_transition_related.set(evt.value()) }
                                input { placeholder: "Note", value: "{bed_transition_note}", oninput: move |evt| bed_transition_note.set(evt.value()) }
                                button {
                                    class: "primary",
                                    onclick: move |_| {
                                        if let Ok(bed_id) = bed_transition_id().parse::<i64>() {
                                            let related = bed_transition_related().parse::<i64>().ok();
                                            let req = BedTransitionRequest {
                                                action: bed_transition_action(),
                                                target_state: bed_transition_state(),
                                                related_bed_id: related,
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
                } else if page() == Page::Dining {
                    article { class: "panel",
                        h3 { "Cafeteria Manager" }
                        button {
                            class: "primary",
                            onclick: move |_| {
                                let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
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
                                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
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
                                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
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
                                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
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
                                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
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
                                        let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
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
                } else if page() == Page::Orders {
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
                                                Ok(id) => status.set(format!("Order placed #{id}")),
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
                } else if page() == Page::Campaigns {
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
                                                Ok(_) => status.set("Joined campaign".to_string()),
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
                } else if page() == Page::Ingestion {
                    article { class: "panel",
                        h3 { "Ingestion Task Manager" }
                        p { class: "muted", "Create, update, version, rollback, run, and inspect status with role-aware API errors surfaced inline." }
                        button {
                            class: "primary",
                            onclick: move |_| {
                                let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                spawn(async move {
                                    match api::list_ingestion_tasks(&token).await {
                                        Ok(items) => ingestion_tasks.set(items),
                                        Err(e) => error.set(e),
                                    }
                                });
                            },
                            "Refresh Tasks"
                        }
                        section { class: "subpanel",
                            h4 { "Create / Update Task" }
                            input { placeholder: "Task name", value: "{ingestion_task_name}", oninput: move |evt| ingestion_task_name.set(evt.value()) }
                            textarea { placeholder: "Seed URLs (comma-separated)", value: "{ingestion_seed_urls}", oninput: move |evt| ingestion_seed_urls.set(evt.value()) }
                            textarea { placeholder: "Extraction rules JSON", value: "{ingestion_rules}", oninput: move |evt| ingestion_rules.set(evt.value()) }
                            input { placeholder: "Pagination strategy", value: "{ingestion_strategy}", oninput: move |evt| ingestion_strategy.set(evt.value()) }
                            input { placeholder: "Max depth", value: "{ingestion_depth}", oninput: move |evt| ingestion_depth.set(evt.value()) }
                            input { placeholder: "Incremental field", value: "{ingestion_incremental_field}", oninput: move |evt| ingestion_incremental_field.set(evt.value()) }
                            input { placeholder: "Schedule cron", value: "{ingestion_schedule}", oninput: move |evt| ingestion_schedule.set(evt.value()) }
                            button {
                                onclick: move |_| {
                                    if let Ok(max_depth) = ingestion_depth().parse::<i32>() {
                                        let seed_urls = ingestion_seed_urls()
                                            .split(',')
                                            .map(|s| s.trim().to_string())
                                            .filter(|s| !s.is_empty())
                                            .collect::<Vec<_>>();
                                        let req = IngestionTaskCreateRequest {
                                            task_name: ingestion_task_name(),
                                            seed_urls,
                                            extraction_rules_json: ingestion_rules(),
                                            pagination_strategy: ingestion_strategy(),
                                            max_depth,
                                            incremental_field: Some(ingestion_incremental_field()),
                                            schedule_cron: ingestion_schedule(),
                                        };
                                        let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                        spawn(async move {
                                            match api::create_ingestion_task(&token, req).await {
                                                Ok(id) => {
                                                    ingestion_selected_task.set(id.to_string());
                                                    status.set(format!("Ingestion task created #{id}"));
                                                }
                                                Err(e) => error.set(e),
                                            }
                                        });
                                    }
                                },
                                "Create Task"
                            }
                            input { placeholder: "Task ID for update", value: "{ingestion_selected_task}", oninput: move |evt| ingestion_selected_task.set(evt.value()) }
                            button {
                                onclick: move |_| {
                                    if let (Ok(task_id), Ok(max_depth)) = (
                                        ingestion_selected_task().parse::<i64>(),
                                        ingestion_depth().parse::<i32>(),
                                    ) {
                                        let seed_urls = ingestion_seed_urls()
                                            .split(',')
                                            .map(|s| s.trim().to_string())
                                            .filter(|s| !s.is_empty())
                                            .collect::<Vec<_>>();
                                        let req = IngestionTaskUpdateRequest {
                                            seed_urls,
                                            extraction_rules_json: ingestion_rules(),
                                            pagination_strategy: ingestion_strategy(),
                                            max_depth,
                                            incremental_field: Some(ingestion_incremental_field()),
                                            schedule_cron: ingestion_schedule(),
                                        };
                                        let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                        spawn(async move {
                                            match api::update_ingestion_task(&token, task_id, req).await {
                                                Ok(version) => status.set(format!("Task updated to version {version}")),
                                                Err(e) => error.set(e),
                                            }
                                        });
                                    }
                                },
                                "Update Task"
                            }
                        }
                        section { class: "subpanel",
                            h4 { "Run / Versions / Rollback" }
                            button {
                                onclick: move |_| {
                                    if let Ok(task_id) = ingestion_selected_task().parse::<i64>() {
                                        let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                        spawn(async move {
                                            match api::run_ingestion_task(&token, task_id).await {
                                                Ok(_) => status.set("Ingestion run started".to_string()),
                                                Err(e) => error.set(e),
                                            }
                                        });
                                    }
                                },
                                "Run Task"
                            }
                            button {
                                onclick: move |_| {
                                    if let Ok(task_id) = ingestion_selected_task().parse::<i64>() {
                                        let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                        spawn(async move {
                                            if let Ok(v) = api::ingestion_task_versions(&token, task_id).await {
                                                ingestion_versions.set(v);
                                            }
                                            if let Ok(r) = api::ingestion_task_runs(&token, task_id).await {
                                                ingestion_runs.set(r);
                                            }
                                        });
                                    }
                                },
                                "Load Versions + Runs"
                            }
                            input { placeholder: "Rollback target version", value: "{ingestion_rollback_version}", oninput: move |evt| ingestion_rollback_version.set(evt.value()) }
                            input { placeholder: "Rollback reason", value: "{ingestion_rollback_reason}", oninput: move |evt| ingestion_rollback_reason.set(evt.value()) }
                            button {
                                onclick: move |_| {
                                    if let (Ok(task_id), Ok(target_version)) = (
                                        ingestion_selected_task().parse::<i64>(),
                                        ingestion_rollback_version().parse::<i32>(),
                                    ) {
                                        let req = IngestionTaskRollbackRequest {
                                            target_version,
                                            reason: ingestion_rollback_reason(),
                                        };
                                        let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                        spawn(async move {
                                            match api::rollback_ingestion_task(&token, task_id, req).await {
                                                Ok(version) => status.set(format!("Rollback created version {version}")),
                                                Err(e) => error.set(e),
                                            }
                                        });
                                    }
                                },
                                "Rollback"
                            }
                        }
                        section { class: "subpanel",
                            h4 { "Task Status" }
                            div { class: "cards",
                                for task in ingestion_tasks() {
                                    article { class: "card",
                                        p { "#{task.id} {task.task_name} (v{task.active_version})" }
                                        p { class: "muted", "{task.status} / {task.pagination_strategy} / depth {task.max_depth}" }
                                        p { class: "muted", "next: {task.next_run_at:?} last: {task.last_run_at:?}" }
                                    }
                                }
                            }
                            div { class: "cards",
                                for version in ingestion_versions() {
                                    article { class: "card",
                                        p { "version {version.version_number}" }
                                        p { class: "muted", "rollback_of: {version.rollback_of_version:?}" }
                                        p { class: "muted", "{version.created_at}" }
                                    }
                                }
                            }
                            div { class: "cards",
                                for run in ingestion_runs() {
                                    article { class: "card",
                                        p { "run #{run.id} v{run.task_version} {run.status}" }
                                        p { class: "muted", "records: {run.records_extracted}" }
                                        p { class: "muted", "{run.started_at} -> {run.finished_at:?}" }
                                    }
                                }
                            }
                        }
                    }
                } else if page() == Page::Admin {
                    article { class: "panel",
                        h3 { "Administrator Console" }
                        button {
                            class: "primary",
                            onclick: move |_| {
                                let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                spawn(async move {
                                    match api::list_users(&token).await {
                                        Ok(items) => users.set(items),
                                        Err(e) => error.set(e),
                                    }
                                });
                            },
                            "Refresh Users"
                        }
                        div { class: "cards",
                            for user in users() {
                                article { class: "card",
                                    strong { "{user.username}" }
                                    p { class: "muted", "role: {user.role} / disabled: {user.disabled}" }
                                    button {
                                        class: "danger",
                                        onclick: move |_| {
                                            let uid = user.id;
                                            let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                            spawn(async move {
                                                match api::disable_user(&token, uid).await {
                                                    Ok(_) => status.set(format!("User #{uid} disabled immediately")),
                                                    Err(e) => error.set(e),
                                                }
                                            });
                                        },
                                        "Disable"
                                    }
                                }
                            }
                        }
                    }
                } else if page() == Page::Experiments {
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
                } else if page() == Page::Analytics {
                    article { class: "panel",
                        h3 { "Analytics" }
                        button {
                            class: "primary",
                            onclick: move |_| {
                                let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                spawn(async move {
                                    if let Ok(items) = api::funnel_metrics(&token).await { funnel.set(items); }
                                    if let Ok(items) = api::retention_metrics(&token).await { retention.set(items); }
                                    if let Ok(item) = api::recommendation_kpi(&token).await { recommendation_kpi.set(Some(item)); }
                                });
                            },
                            "Refresh Analytics"
                        }
                        div { class: "cards", for f in funnel() { article { class: "card", p { "{f.step}: {f.users} users" } } } }
                        div { class: "cards", for r in retention() { article { class: "card", p { "{r.cohort}: {r.retained_users} retained" } } } }
                        if let Some(kpi) = recommendation_kpi() {
                            article { class: "card", p { "Recommendation CTR: {kpi.ctr}" } p { "Recommendation conversion: {kpi.conversion}" } }
                        }
                    }
                } else if page() == Page::Audits {
                    article { class: "panel",
                        h3 { "Audit Feed" }
                        button {
                            class: "primary",
                            onclick: move |_| {
                                let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                                spawn(async move {
                                    match api::list_audits(&token).await {
                                        Ok(items) => audits.set(items),
                                        Err(e) => error.set(e),
                                    }
                                });
                            },
                            "Refresh Audits"
                        }
                        div { class: "cards",
                            for a in audits() {
                                article { class: "card",
                                    p { "{a.created_at} - {a.action_type}" }
                                    p { class: "muted", "{a.entity_type} #{a.entity_id} by {a.actor_username}" }
                                }
                            }
                        }
                    }
                }
        }
    }
}
