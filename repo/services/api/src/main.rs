mod config;
mod contracts;
mod infrastructure;
mod repositories;
mod routes;
mod services;

use std::sync::Arc;

use config::AppConfig;
use contracts::ApiError;
use infrastructure::adapters::mysql_app_repository::MySqlAppRepository;
use infrastructure::auth::middleware::AuthFairing;
use infrastructure::database::{connect_pool, run_migrations};
use infrastructure::security::field_crypto::FieldCrypto;
use repositories::app_repository::AppRepository;
use rocket::http::Method;
use rocket::serde::json::Json;
use rocket::{routes, Build, Rocket, State};
use rocket_cors::{AllowedOrigins, CorsOptions};
use services::app_service::AppService;

#[derive(Clone)]
pub struct AppState {
    pub app_service: AppService,
    pub retention: contracts::RetentionSnapshot,
    pub session: contracts::SessionSnapshot,
    pub db_pool: sqlx::MySqlPool,
}

#[rocket::get("/api/v1/health")]
async fn health(state: &State<AppState>) -> Result<Json<contracts::HealthResponse>, ApiError> {
    let ping = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db_pool)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(contracts::HealthResponse {
        status: "ok".to_string(),
        db: if ping == 1 {
            "connected".to_string()
        } else {
            "unknown".to_string()
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = AppConfig::load("/app/config/default.toml")?;
    if !cfg.auth_policy.offline_only {
        return Err("auth_policy.offline_only must remain true for this deployment".into());
    }

    let pool = connect_pool(&cfg.database).await?;
    run_migrations(&pool).await?;
    let field_crypto = FieldCrypto::load_or_bootstrap(&cfg.security.key_bootstrap).await?;

    let repository: Arc<dyn AppRepository> = Arc::new(MySqlAppRepository::new(
        pool.clone(),
        cfg.auth_policy.lockout_failed_attempts as i32,
        cfg.auth_policy.lockout_minutes as i32,
        cfg.auth_policy.session_inactivity_minutes as i32,
        field_crypto,
    ));
    let app_service = AppService::new(repository, &cfg.auth_policy, &cfg.retention);

    let state = AppState {
        app_service,
        retention: contracts::RetentionSnapshot {
            audit_log_days: cfg.retention.audit_log_days,
            session_days: cfg.retention.session_days,
            patient_record_days: cfg.retention.patient_record_days,
        },
        session: contracts::SessionSnapshot {
            cookie_name: cfg.session.cookie_name.clone(),
            secure: cfg.session.secure,
            http_only: cfg.session.http_only,
            same_site: cfg.session.same_site.clone(),
            ttl_minutes: cfg.session.ttl_minutes,
        },
        db_pool: pool,
    };

    build_rocket(&cfg, state).launch().await?;
    Ok(())
}

fn build_rocket(cfg: &AppConfig, state: AppState) -> Rocket<Build> {
    let origins = AllowedOrigins::some_exact(&cfg.server.allowed_origins);

    let cors = CorsOptions {
        allowed_origins: origins,
        allowed_methods: vec![Method::Get, Method::Post, Method::Put, Method::Delete]
            .into_iter()
            .map(From::from)
            .collect(),
        allow_credentials: true,
        ..Default::default()
    }
    .to_cors()
    .expect("valid CORS configuration");

    let figment = rocket::Config::figment()
        .merge(("address", cfg.server.host.clone()))
        .merge(("port", cfg.server.port));

    rocket::custom(figment)
        .manage(state)
        .attach(cors)
        .attach(AuthFairing)
        .mount(
            "/",
            routes![
                health,
                routes::auth::login,
                routes::catalog::hospitals,
                routes::catalog::roles,
                routes::rbac::menu_entitlements,
                routes::rbac::list_users,
                routes::rbac::disable_user,
                routes::patients::create_patient,
                routes::patients::list_patients,
                routes::patients::search_patients,
                routes::patients::get_patient,
                routes::patients::assign_patient,
                routes::patients::update_patient,
                routes::patients::edit_allergies,
                routes::patients::edit_contraindications,
                routes::patients::edit_history,
                routes::patients::add_visit_note,
                routes::patients::revisions,
                routes::patients::upload_attachment,
                routes::patients::list_attachments,
                routes::patients::download_attachment,
                routes::patients::export_patient,
                routes::bedboard::list_beds,
                routes::bedboard::transition,
                routes::bedboard::events,
                routes::dining::create_menu,
                routes::dining::list_menus,
                routes::dining::place_order,
                routes::dining::update_order_status,
                routes::dining::list_orders,
                routes::dining::add_ticket_split,
                routes::dining::list_ticket_splits,
                routes::dining::add_order_note,
                routes::dining::list_order_notes,
                routes::cafeteria::categories,
                routes::cafeteria::create_dish,
                routes::cafeteria::list_dishes,
                routes::cafeteria::dish_status,
                routes::cafeteria::add_option,
                routes::cafeteria::add_window,
                routes::cafeteria::upsert_rule,
                routes::cafeteria::rules,
                routes::cafeteria::recommendations,
                routes::campaigns::create_campaign,
                routes::campaigns::join_campaign,
                routes::campaigns::list_campaigns,
                routes::experiments::create_experiment,
                routes::experiments::add_variant,
                routes::experiments::assign_variant,
                routes::experiments::backtrack,
                routes::analytics::funnel,
                routes::analytics::retention,
                routes::analytics::recommendation_kpi,
                routes::governance::create_record,
                routes::governance::list_records,
                routes::governance::tombstone_record,
                routes::ingestion::create_task,
                routes::ingestion::update_task,
                routes::ingestion::rollback_task,
                routes::ingestion::run_task,
                routes::ingestion::list_tasks,
                routes::ingestion::task_versions,
                routes::ingestion::task_runs,
                routes::telemetry::create_event,
                routes::audits::list_audits,
                routes::audits::reject_audit_update,
                routes::audits::reject_audit_delete,
                routes::retention::retention_settings,
                routes::retention::retention_policies,
                routes::retention::upsert_retention_policy,
                routes::session::session_settings
            ],
        )
}
