mod api;
mod components;
mod features;
mod pages;
mod state;
mod ui_logic;

use std::time::Duration;

use contracts::{
    AttachmentMetadataDto, BedDto, BedEventDto, CampaignDto, DishCategoryDto, DishDto,
    FunnelMetricsDto, IngestionTaskDto, IngestionTaskRunDto, IngestionTaskVersionDto,
    OrderDto, OrderNoteDto, PatientProfileDto, PatientSearchResultDto,
    RecommendationDto, RecommendationKpiDto, RetentionMetricsDto, RevisionTimelineDto,
    TicketSplitDto, UserSummaryDto,
};
use dioxus::prelude::*;
use gloo_timers::future::sleep;
use components::app_shell::{AppShell, ShellNavItem};
use components::auth_gate::AuthGate;
use features::guards::resolve_page_access;
use features::navigation::nav_items;
use pages::admin::AdminPage;
use pages::analytics::AnalyticsPage;
use pages::audits::AuditsPage;
use pages::bedboard::BedboardPage;
use pages::campaigns::CampaignsPage;
use pages::dashboard::DashboardPage;
use pages::dining::DiningPage;
use pages::experiments::ExperimentsPage;
use pages::ingestion::IngestionPage;
use pages::orders::OrdersPage;
use pages::patients::PatientsPage;
use state::{
    can_access, clear_session, ensure_accessible_page, is_user_switch,
    load_session, save_session, session_from_entitlements, Page, SessionContext, StoredSession,
};
use ui_logic::{QueuedAttachment, UploadState};

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
    let mut bed_transition_patient_id = use_signal(String::new);
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
                                        api::track_ui_event(
                                            &next_session.stored.token,
                                            "ui_instrumentation",
                                            "session.login",
                                            &format!("{{\"role\":\"{}\"}}", next_session.stored.role),
                                        );
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
                    DashboardPage { ctx: ctx.clone() }
                } else if page() == Page::Patients {
                    PatientsPage {
                        status, error, session,
                        patient_query, patient_results, selected_patient_id,
                        patient_profile, patient_revisions, patient_attachments,
                        demo_first_name, demo_last_name, demo_birth_date,
                        demo_gender, demo_phone, demo_email, demo_reason,
                        allergies_value, contraindications_value, history_value,
                        clinical_reason, visit_note, visit_reason,
                        patient_export_format,
                        upload_progress, upload_state, upload_state_kind, attachment_queue,
                    }
                } else if page() == Page::Bedboard {
                    BedboardPage {
                        status, error, session,
                        beds, bed_events,
                        bed_transition_id, bed_transition_action,
                        bed_transition_state, bed_transition_patient_id, bed_transition_related, bed_transition_note,
                    }
                } else if page() == Page::Dining {
                    DiningPage {
                        status, error, session,
                        categories, dishes, ranking_rules, recommendations,
                        dish_category_id, dish_name, dish_description, dish_price, dish_photo_path,
                        dish_status_id, dish_published, dish_sold_out,
                        dish_option_id, dish_option_group, dish_option_value, dish_option_delta,
                        dish_window_id, dish_window_slot, dish_window_start, dish_window_end,
                        ranking_rule_key, ranking_rule_weight, ranking_rule_enabled,
                    }
                } else if page() == Page::Orders {
                    OrdersPage {
                        status, error, session,
                        menus, orders,
                        order_patient_id, order_menu_id, order_notes,
                        order_status_id, order_status_value, order_status_reason,
                        order_note_id, order_note_text,
                        order_split_id, order_split_by, order_split_value, order_split_quantity,
                        order_note_timeline, order_split_timeline,
                    }
                } else if page() == Page::Campaigns {
                    CampaignsPage {
                        status, error, session,
                        campaigns,
                        campaign_title, campaign_dish_id, campaign_threshold,
                        campaign_deadline, campaign_join_id,
                    }
                } else if page() == Page::Ingestion {
                    IngestionPage {
                        status, error, session,
                        ingestion_tasks, ingestion_versions, ingestion_runs,
                        ingestion_task_name, ingestion_seed_urls, ingestion_rules,
                        ingestion_strategy, ingestion_depth, ingestion_incremental_field,
                        ingestion_schedule, ingestion_selected_task,
                        ingestion_rollback_version, ingestion_rollback_reason,
                    }
                } else if page() == Page::Admin {
                    AdminPage { status, error, session, users }
                } else if page() == Page::Experiments {
                    ExperimentsPage {
                        status, error, session,
                        experiment_id, experiment_key,
                        variant_key, variant_weight, variant_version,
                        assign_user_id, assign_mode,
                        backtrack_from, backtrack_to, backtrack_reason,
                        assigned_variant,
                    }
                } else if page() == Page::Analytics {
                    AnalyticsPage { session, funnel, retention, recommendation_kpi }
                } else if page() == Page::Audits {
                    AuditsPage { error, session, audits }
                }
        }
    }
}
