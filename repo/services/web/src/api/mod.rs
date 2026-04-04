use contracts::{
    AttachmentMetadataDto, AuditLogDto, AuthLoginRequest, AuthLoginResponse, BedDto, BedEventDto,
    BedTransitionRequest, CampaignCreateRequest, CampaignDto, ClinicalEditRequest, DishCategoryDto,
    DishCreateRequest, DishDto, DishOptionRequest, DishStatusRequest, DishWindowRequest,
    ExperimentAssignRequest, ExperimentBacktrackRequest, ExperimentCreateRequest,
    ExperimentVariantRequest, FunnelMetricsDto, IngestionTaskCreateRequest, IngestionTaskDto,
    IngestionTaskRollbackRequest, IngestionTaskRunDto, IngestionTaskUpdateRequest,
    IngestionTaskVersionDto, MenuEntitlementDto, OrderCreateRequest, OrderDto, OrderNoteDto,
    OrderNoteRequest, OrderStatusRequest, PatientProfileDto, PatientSearchResultDto,
    PatientExportDto, PatientUpdateRequest, RankingRuleDto, RankingRuleRequest, RecommendationDto,
    RecommendationKpiDto, RetentionMetricsDto, RevisionTimelineDto, TelemetryEventRequest,
    UserSummaryDto, TicketSplitDto, TicketSplitRequest, VisitNoteRequest,
};
use gloo_net::http::{Request, RequestBuilder, Response};
use serde::de::DeserializeOwned;
#[cfg(target_arch = "wasm32")]
use web_sys::js_sys::Uint8Array;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{Headers, Request as WebRequest, RequestInit, RequestMode, Response as WebResponse};

/// API base URL uses a same-origin relative path so the browser routes
/// requests through the nginx reverse proxy.  This avoids hardcoding
/// `localhost:8000` and allows any intranet client to reach the API via
/// the web server's address.
const API_BASE: &str = "/api/v1";

async fn send_builder(builder: RequestBuilder) -> Result<Response, String> {
    let response = builder.send().await.map_err(|e| format!("request failed: {e}"))?;
    if response.ok() {
        return Ok(response);
    }
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "unable to read error body".to_string());
    Err(format!("http {status}: {body}"))
}

async fn send_request(request: Request) -> Result<Response, String> {
    let response = request.send().await.map_err(|e| format!("request failed: {e}"))?;
    if response.ok() {
        return Ok(response);
    }
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "unable to read error body".to_string());
    Err(format!("http {status}: {body}"))
}

async fn send_json<T: DeserializeOwned>(builder: RequestBuilder) -> Result<T, String> {
    let response = send_builder(builder).await?;
    response
        .json::<T>()
        .await
        .map_err(|e| format!("decode failed: {e}"))
}

async fn send_request_json<T: DeserializeOwned>(request: Request) -> Result<T, String> {
    let response = send_request(request).await?;
    response
        .json::<T>()
        .await
        .map_err(|e| format!("decode failed: {e}"))
}

fn with_auth(builder: RequestBuilder, token: &str) -> RequestBuilder {
    builder.header("X-Session-Token", token)
}

pub async fn login(username: &str, password: &str) -> Result<AuthLoginResponse, String> {
    let payload = AuthLoginRequest {
        username: username.to_string(),
        password: password.to_string(),
    };
    let body = serde_json::to_string(&payload).map_err(|e| format!("encode failed: {e}"))?;
    send_request_json(
        Request::post(&format!("{API_BASE}/auth/login"))
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
}

pub async fn menu_entitlements(token: &str) -> Result<Vec<MenuEntitlementDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/rbac/menu-entitlements")),
        token,
    ))
    .await
}

pub async fn list_users(token: &str) -> Result<Vec<UserSummaryDto>, String> {
    send_json(with_auth(Request::get(&format!("{API_BASE}/admin/users")), token)).await
}

pub async fn disable_user(token: &str, user_id: i64) -> Result<(), String> {
    send_builder(with_auth(
        Request::post(&format!("{API_BASE}/admin/users/{user_id}/disable")),
        token,
    ))
    .await
    .map(|_| ())
}

pub async fn search_patients(token: &str, query: &str) -> Result<Vec<PatientSearchResultDto>, String> {
    let q = urlencoding::encode(query);
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/patients/search?q={q}")),
        token,
    ))
    .await
}

pub async fn get_patient(token: &str, patient_id: i64) -> Result<PatientProfileDto, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/patients/{patient_id}")),
        token,
    ))
    .await
}

pub async fn update_patient(token: &str, patient_id: i64, req: PatientUpdateRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::put(&format!("{API_BASE}/patients/{patient_id}")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn edit_clinical_field(
    token: &str,
    patient_id: i64,
    field: &str,
    req: ClinicalEditRequest,
) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::put(&format!("{API_BASE}/patients/{patient_id}/{field}")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn add_visit_note(token: &str, patient_id: i64, req: VisitNoteRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::post(&format!("{API_BASE}/patients/{patient_id}/visit-notes")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn patient_revisions(token: &str, patient_id: i64, reveal_sensitive: bool) -> Result<Vec<RevisionTimelineDto>, String> {
    let reveal = if reveal_sensitive { "true" } else { "false" };
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/patients/{patient_id}/revisions?reveal_sensitive={reveal}")),
        token,
    ))
    .await
}

pub async fn list_attachments(token: &str, patient_id: i64) -> Result<Vec<AttachmentMetadataDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/patients/{patient_id}/attachments")),
        token,
    ))
    .await
}

pub async fn upload_attachment(
    token: &str,
    patient_id: i64,
    filename: &str,
    mime_type: &str,
    content: Vec<u8>,
) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (token, patient_id, filename, mime_type, content);
        return Err("binary upload is available only in browser runtime".to_string());
    }

    #[cfg(target_arch = "wasm32")]
    {
    let file = urlencoding::encode(filename);
    let mime = urlencoding::encode(mime_type);
    let url = format!("{API_BASE}/patients/{patient_id}/attachments?filename={file}&mime_type={mime}");
    let init = RequestInit::new();
    init.set_method("POST");
    init.set_mode(RequestMode::Cors);

    let body = Uint8Array::from(content.as_slice());
    init.set_body(&body.into());

    let headers = Headers::new().map_err(|_| "failed to create headers".to_string())?;
    headers
        .set("Content-Type", "application/octet-stream")
        .map_err(|_| "failed to set content type".to_string())?;
    headers
        .set("X-Session-Token", token)
        .map_err(|_| "failed to set auth header".to_string())?;
    init.set_headers(&headers);

    let request = WebRequest::new_with_str_and_init(&url, &init)
        .map_err(|_| "failed to build upload request".to_string())?;
    let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
    let fetched = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| "upload request failed".to_string())?;
    let response: WebResponse = fetched
        .dyn_into()
        .map_err(|_| "invalid upload response".to_string())?;
    if response.ok() {
        return Ok(());
    }

    let status = response.status();
    let body_text = match response.text() {
        Ok(promise) => JsFuture::from(promise)
            .await
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| "unable to read error body".to_string()),
        Err(_) => "unable to read error body".to_string(),
    };
    Err(format!("http {status}: {body_text}"))
    }
}

pub async fn export_patient(
    token: &str,
    patient_id: i64,
    format: &str,
    reveal_sensitive: bool,
) -> Result<PatientExportDto, String> {
    let reveal = if reveal_sensitive { "true" } else { "false" };
    let fmt = urlencoding::encode(format);
    send_json(with_auth(
        Request::get(&format!(
            "{API_BASE}/patients/{patient_id}/export?format={fmt}&reveal_sensitive={reveal}"
        )),
        token,
    ))
    .await
}

pub async fn list_beds(token: &str) -> Result<Vec<BedDto>, String> {
    send_json(with_auth(Request::get(&format!("{API_BASE}/bedboard/beds")), token)).await
}

pub async fn bed_events(token: &str) -> Result<Vec<BedEventDto>, String> {
    send_json(with_auth(Request::get(&format!("{API_BASE}/bedboard/events")), token)).await
}

pub async fn transition_bed(token: &str, bed_id: i64, req: BedTransitionRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::post(&format!("{API_BASE}/bedboard/beds/{bed_id}/transition")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn list_dish_categories(token: &str) -> Result<Vec<DishCategoryDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/cafeteria/categories")),
        token,
    ))
    .await
}

pub async fn list_dishes(token: &str) -> Result<Vec<DishDto>, String> {
    send_json(with_auth(Request::get(&format!("{API_BASE}/cafeteria/dishes")), token)).await
}

pub async fn create_dish(token: &str, req: DishCreateRequest) -> Result<i64, String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request_json(
        with_auth(Request::post(&format!("{API_BASE}/cafeteria/dishes")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
}

pub async fn set_dish_status(token: &str, dish_id: i64, req: DishStatusRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::put(&format!("{API_BASE}/cafeteria/dishes/{dish_id}/status")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn add_dish_option(token: &str, dish_id: i64, req: DishOptionRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::post(&format!("{API_BASE}/cafeteria/dishes/{dish_id}/options")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn add_sales_window(token: &str, dish_id: i64, req: DishWindowRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::post(&format!("{API_BASE}/cafeteria/dishes/{dish_id}/windows")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn ranking_rules(token: &str) -> Result<Vec<RankingRuleDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/cafeteria/ranking-rules")),
        token,
    ))
    .await
}

pub async fn upsert_ranking_rule(token: &str, req: RankingRuleRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::put(&format!("{API_BASE}/cafeteria/ranking-rules")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn recommendations(token: &str) -> Result<Vec<RecommendationDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/cafeteria/recommendations")),
        token,
    ))
    .await
}

pub async fn list_menus(token: &str) -> Result<Vec<contracts::DiningMenuDto>, String> {
    send_json(with_auth(Request::get(&format!("{API_BASE}/dining/menus")), token)).await
}

pub async fn place_order(token: &str, req: OrderCreateRequest) -> Result<i64, String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request_json(
        with_auth(Request::post(&format!("{API_BASE}/orders")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
}

pub async fn list_orders(token: &str) -> Result<Vec<OrderDto>, String> {
    send_json(with_auth(Request::get(&format!("{API_BASE}/orders")), token)).await
}

pub async fn set_order_status(token: &str, order_id: i64, req: OrderStatusRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::put(&format!("{API_BASE}/orders/{order_id}/status")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn add_order_note(token: &str, order_id: i64, req: OrderNoteRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::post(&format!("{API_BASE}/orders/{order_id}/notes")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn list_order_notes(token: &str, order_id: i64) -> Result<Vec<OrderNoteDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/orders/{order_id}/notes")),
        token,
    ))
    .await
}

pub async fn add_ticket_split(token: &str, order_id: i64, req: TicketSplitRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(
            Request::post(&format!("{API_BASE}/orders/{order_id}/ticket-splits")),
            token,
        )
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn list_ticket_splits(token: &str, order_id: i64) -> Result<Vec<TicketSplitDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/orders/{order_id}/ticket-splits")),
        token,
    ))
    .await
}

pub async fn list_campaigns(token: &str) -> Result<Vec<CampaignDto>, String> {
    send_json(with_auth(Request::get(&format!("{API_BASE}/campaigns")), token)).await
}

pub async fn create_campaign(token: &str, req: CampaignCreateRequest) -> Result<i64, String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request_json(
        with_auth(Request::post(&format!("{API_BASE}/campaigns")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
}

pub async fn join_campaign(token: &str, campaign_id: i64) -> Result<(), String> {
    send_builder(with_auth(
        Request::post(&format!("{API_BASE}/campaigns/{campaign_id}/join")),
        token,
    ))
    .await
    .map(|_| ())
}

pub async fn create_experiment(token: &str, req: ExperimentCreateRequest) -> Result<i64, String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request_json(
        with_auth(Request::post(&format!("{API_BASE}/experiments")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
}

pub async fn add_experiment_variant(
    token: &str,
    experiment_id: i64,
    req: ExperimentVariantRequest,
) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::post(&format!("{API_BASE}/experiments/{experiment_id}/variants")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn assign_experiment(
    token: &str,
    experiment_id: i64,
    req: ExperimentAssignRequest,
) -> Result<Option<String>, String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request_json(
        with_auth(Request::post(&format!("{API_BASE}/experiments/{experiment_id}/assign")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
}

pub async fn backtrack_experiment(
    token: &str,
    experiment_id: i64,
    req: ExperimentBacktrackRequest,
) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(Request::post(&format!("{API_BASE}/experiments/{experiment_id}/backtrack")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

pub async fn funnel_metrics(token: &str) -> Result<Vec<FunnelMetricsDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/analytics/funnel")),
        token,
    ))
    .await
}

pub async fn retention_metrics(token: &str) -> Result<Vec<RetentionMetricsDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/analytics/retention")),
        token,
    ))
    .await
}

pub async fn recommendation_kpi(token: &str) -> Result<RecommendationKpiDto, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/analytics/recommendation-kpi")),
        token,
    ))
    .await
}

pub async fn list_audits(token: &str) -> Result<Vec<AuditLogDto>, String> {
    send_json(with_auth(Request::get(&format!("{API_BASE}/audits")), token)).await
}

pub async fn list_ingestion_tasks(token: &str) -> Result<Vec<IngestionTaskDto>, String> {
    send_json(with_auth(Request::get(&format!("{API_BASE}/ingestion/tasks")), token)).await
}

pub async fn create_ingestion_task(token: &str, req: IngestionTaskCreateRequest) -> Result<i64, String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request_json(
        with_auth(Request::post(&format!("{API_BASE}/ingestion/tasks")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
}

pub async fn update_ingestion_task(
    token: &str,
    task_id: i64,
    req: IngestionTaskUpdateRequest,
) -> Result<i32, String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request_json(
        with_auth(Request::put(&format!("{API_BASE}/ingestion/tasks/{task_id}")), token)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
}

pub async fn rollback_ingestion_task(
    token: &str,
    task_id: i64,
    req: IngestionTaskRollbackRequest,
) -> Result<i32, String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request_json(
        with_auth(
            Request::post(&format!("{API_BASE}/ingestion/tasks/{task_id}/rollback")),
            token,
        )
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
}

pub async fn run_ingestion_task(token: &str, task_id: i64) -> Result<(), String> {
    send_builder(with_auth(
        Request::post(&format!("{API_BASE}/ingestion/tasks/{task_id}/run")),
        token,
    ))
    .await
    .map(|_| ())
}

pub async fn ingestion_task_versions(token: &str, task_id: i64) -> Result<Vec<IngestionTaskVersionDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/ingestion/tasks/{task_id}/versions")),
        token,
    ))
    .await
}

pub async fn ingestion_task_runs(token: &str, task_id: i64) -> Result<Vec<IngestionTaskRunDto>, String> {
    send_json(with_auth(
        Request::get(&format!("{API_BASE}/ingestion/tasks/{task_id}/runs")),
        token,
    ))
    .await
}

pub async fn send_telemetry_event(token: &str, req: TelemetryEventRequest) -> Result<(), String> {
    let body = serde_json::to_string(&req).map_err(|e| format!("encode failed: {e}"))?;
    send_request(
        with_auth(
            Request::post(&format!("{API_BASE}/telemetry/events")),
            token,
        )
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("request build failed: {e}"))?,
    )
    .await
    .map(|_| ())
}

/// Fire-and-forget telemetry helper for UI workflows.
/// Silently drops errors to avoid disrupting user flows.
pub fn track_ui_event(token: &str, experiment_key: &str, event_name: &str, payload_json: &str) {
    let req = TelemetryEventRequest {
        experiment_key: experiment_key.to_string(),
        event_name: event_name.to_string(),
        payload_json: payload_json.to_string(),
    };
    let token = token.to_string();
    #[cfg(target_arch = "wasm32")]
    dioxus::prelude::spawn(async move {
        let _ = send_telemetry_event(&token, req).await;
    });
    #[cfg(not(target_arch = "wasm32"))]
    { let _ = (token, req); }
}
