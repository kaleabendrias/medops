mod api;
mod components;
mod features;
mod hooks;
mod pages;
mod state;
mod ui_logic;

use std::time::Duration;

use dioxus::prelude::*;
use gloo_timers::future::sleep;
use components::app_shell::{AppShell, ShellNavItem};
use components::auth_gate::AuthGate;
use features::guards::resolve_page_access;
use features::navigation::nav_items;
use hooks::admin::use_admin_state;
use hooks::analytics::use_analytics_state;
use hooks::audits::use_audits_state;
use hooks::bedboard::use_bedboard_state;
use hooks::campaigns::use_campaigns_state;
use hooks::dining::use_dining_state;
use hooks::experiments::use_experiments_state;
use hooks::ingestion::use_ingestion_state;
use hooks::orders::use_orders_state;
use hooks::patients::use_patients_state;
use hooks::uploads::use_uploads_state;
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

#[component]
fn App() -> Element {
    let mut status = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut session = use_signal(|| None::<SessionContext>);
    let mut page = use_signal(|| Page::Dashboard);

    let mut login_username = use_signal(String::new);
    let mut login_password = use_signal(String::new);

    // Page-level state hooks — each module owns its own signals
    let mut patients = use_patients_state();
    let bedboard = use_bedboard_state();
    let dining = use_dining_state();
    let mut orders = use_orders_state();
    let campaigns = use_campaigns_state();
    let mut ingestion = use_ingestion_state();
    let experiments = use_experiments_state();
    let analytics = use_analytics_state();
    let admin = use_admin_state();
    let audits = use_audits_state();
    let mut uploads = use_uploads_state();

    let mut reset_workspace = move || {
        patients.reset();
        orders.reset();
        ingestion.reset();
        uploads.reset();
    };

    use_future(move || async move {
        if let Some(stored) = load_session() {
            match api::menu_entitlements(&stored.csrf_token).await {
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

    // Bedboard polling effect
    let mut beds = bedboard.beds;
    let mut bed_events = bedboard.bed_events;
    use_future(move || async move {
        loop {
            if let Some(ctx) = session() {
                if can_access(&ctx, Page::Bedboard) {
                    if let Ok(next_beds) = api::list_beds(&ctx.stored.csrf_token).await {
                        beds.set(next_beds);
                    }
                    if let Ok(next_events) = api::bed_events(&ctx.stored.csrf_token).await {
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
                                match api::menu_entitlements(&auth.csrf_token).await {
                                    Ok(list) => {
                                        let stored = StoredSession {
                                            csrf_token: auth.csrf_token,
                                            user_id: auth.user_id,
                                            username: auth.username,
                                            role: auth.role,
                                        };
                                        let switched_user = is_user_switch(session().as_ref(), &stored);
                                        reset_workspace();
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
                                            &next_session.stored.csrf_token,
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
                spawn(async move {
                    let _ = api::logout().await;
                });
                clear_session();
                session.set(None);
                set_hash_page(Page::Dashboard);
                page.set(Page::Dashboard);
                status.set(String::new());
                error.set(String::new());
                reset_workspace();
            },

                if forbidden {
                    article { class: "panel", h3 { "Access denied" } p { "Your role does not have this entitlement." } }
                } else if page() == Page::Dashboard {
                    DashboardPage { ctx: ctx.clone() }
                } else if page() == Page::Patients {
                    PatientsPage {
                        status, error, session,
                        patient_query: patients.patient_query,
                        patient_results: patients.patient_results,
                        selected_patient_id: patients.selected_patient_id,
                        patient_profile: patients.patient_profile,
                        patient_revisions: patients.patient_revisions,
                        patient_attachments: patients.patient_attachments,
                        demo_first_name: patients.demo_first_name,
                        demo_last_name: patients.demo_last_name,
                        demo_birth_date: patients.demo_birth_date,
                        demo_gender: patients.demo_gender,
                        demo_phone: patients.demo_phone,
                        demo_email: patients.demo_email,
                        demo_reason: patients.demo_reason,
                        allergies_value: patients.allergies_value,
                        contraindications_value: patients.contraindications_value,
                        history_value: patients.history_value,
                        clinical_reason: patients.clinical_reason,
                        visit_note: patients.visit_note,
                        visit_reason: patients.visit_reason,
                        patient_export_format: patients.patient_export_format,
                        upload_progress: uploads.upload_progress,
                        upload_state: uploads.upload_state,
                        upload_state_kind: uploads.upload_state_kind,
                        attachment_queue: uploads.attachment_queue,
                    }
                } else if page() == Page::Bedboard {
                    BedboardPage {
                        status, error, session,
                        beds: bedboard.beds,
                        bed_events: bedboard.bed_events,
                        bed_transition_id: bedboard.bed_transition_id,
                        bed_transition_action: bedboard.bed_transition_action,
                        bed_transition_state: bedboard.bed_transition_state,
                        bed_transition_patient_id: bedboard.bed_transition_patient_id,
                        bed_transition_related: bedboard.bed_transition_related,
                        bed_transition_note: bedboard.bed_transition_note,
                    }
                } else if page() == Page::Dining {
                    DiningPage {
                        status, error, session,
                        categories: dining.categories,
                        dishes: dining.dishes,
                        ranking_rules: dining.ranking_rules,
                        recommendations: dining.recommendations,
                        dish_category_id: dining.dish_category_id,
                        dish_name: dining.dish_name,
                        dish_description: dining.dish_description,
                        dish_price: dining.dish_price,
                        dish_photo_path: dining.dish_photo_path,
                        dish_status_id: dining.dish_status_id,
                        dish_published: dining.dish_published,
                        dish_sold_out: dining.dish_sold_out,
                        dish_option_id: dining.dish_option_id,
                        dish_option_group: dining.dish_option_group,
                        dish_option_value: dining.dish_option_value,
                        dish_option_delta: dining.dish_option_delta,
                        dish_window_id: dining.dish_window_id,
                        dish_window_slot: dining.dish_window_slot,
                        dish_window_start: dining.dish_window_start,
                        dish_window_end: dining.dish_window_end,
                        ranking_rule_key: dining.ranking_rule_key,
                        ranking_rule_weight: dining.ranking_rule_weight,
                        ranking_rule_enabled: dining.ranking_rule_enabled,
                    }
                } else if page() == Page::Orders {
                    OrdersPage {
                        status, error, session,
                        menus: orders.menus,
                        orders: orders.orders,
                        order_patient_id: orders.order_patient_id,
                        order_menu_id: orders.order_menu_id,
                        order_notes: orders.order_notes,
                        order_status_id: orders.order_status_id,
                        order_status_value: orders.order_status_value,
                        order_status_reason: orders.order_status_reason,
                        order_note_id: orders.order_note_id,
                        order_note_text: orders.order_note_text,
                        order_split_id: orders.order_split_id,
                        order_split_by: orders.order_split_by,
                        order_split_value: orders.order_split_value,
                        order_split_quantity: orders.order_split_quantity,
                        order_note_timeline: orders.order_note_timeline,
                        order_split_timeline: orders.order_split_timeline,
                    }
                } else if page() == Page::Campaigns {
                    CampaignsPage {
                        status, error, session,
                        campaigns: campaigns.campaigns,
                        campaign_title: campaigns.campaign_title,
                        campaign_dish_id: campaigns.campaign_dish_id,
                        campaign_threshold: campaigns.campaign_threshold,
                        campaign_deadline: campaigns.campaign_deadline,
                        campaign_join_id: campaigns.campaign_join_id,
                    }
                } else if page() == Page::Ingestion {
                    IngestionPage {
                        status, error, session,
                        ingestion_tasks: ingestion.ingestion_tasks,
                        ingestion_versions: ingestion.ingestion_versions,
                        ingestion_runs: ingestion.ingestion_runs,
                        ingestion_task_name: ingestion.ingestion_task_name,
                        ingestion_seed_urls: ingestion.ingestion_seed_urls,
                        ingestion_rules: ingestion.ingestion_rules,
                        ingestion_strategy: ingestion.ingestion_strategy,
                        ingestion_depth: ingestion.ingestion_depth,
                        ingestion_incremental_field: ingestion.ingestion_incremental_field,
                        ingestion_schedule: ingestion.ingestion_schedule,
                        ingestion_selected_task: ingestion.ingestion_selected_task,
                        ingestion_rollback_version: ingestion.ingestion_rollback_version,
                        ingestion_rollback_reason: ingestion.ingestion_rollback_reason,
                    }
                } else if page() == Page::Admin {
                    AdminPage { status, error, session, users: admin.users }
                } else if page() == Page::Experiments {
                    ExperimentsPage {
                        status, error, session,
                        experiment_id: experiments.experiment_id,
                        experiment_key: experiments.experiment_key,
                        variant_key: experiments.variant_key,
                        variant_weight: experiments.variant_weight,
                        variant_version: experiments.variant_version,
                        assign_user_id: experiments.assign_user_id,
                        assign_mode: experiments.assign_mode,
                        backtrack_from: experiments.backtrack_from,
                        backtrack_to: experiments.backtrack_to,
                        backtrack_reason: experiments.backtrack_reason,
                        assigned_variant: experiments.assigned_variant,
                    }
                } else if page() == Page::Analytics {
                    AnalyticsPage {
                        session,
                        funnel: analytics.funnel,
                        retention: analytics.retention,
                        recommendation_kpi: analytics.recommendation_kpi,
                    }
                } else if page() == Page::Audits {
                    AuditsPage { error, session, audits: audits.audits }
                }
        }
    }
}
