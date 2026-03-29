use rocket::data::ToByteUnit;
use rocket::http::ContentType;
use rocket::serde::json::Json;
use rocket::{Data, State};

use crate::contracts::{
    ApiError, AttachmentMetadataDto, ClinicalEditRequest, PatientAssignRequest,
    PatientCreateRequest, PatientExportDto, PatientProfileDto, PatientSearchResultDto,
    PatientUpdateRequest, RevisionTimelineDto, VisitNoteRequest,
};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::post("/api/v1/patients", data = "<payload>")]
pub async fn create_patient(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<PatientCreateRequest>,
) -> Result<Json<i64>, ApiError> {
    let id = state
        .app_service
        .create_patient(&user.0, payload.into_inner())
        .await?;
    Ok(Json(id))
}

#[rocket::get("/api/v1/patients?<limit>&<offset>&<reveal_sensitive>")]
pub async fn list_patients(
    state: &State<AppState>,
    user: CurrentUser,
    limit: Option<i64>,
    offset: Option<i64>,
    reveal_sensitive: Option<bool>,
) -> Result<Json<Vec<PatientProfileDto>>, ApiError> {
    let items = state
        .app_service
        .list_patients(
            &user.0,
            limit.unwrap_or(30),
            offset.unwrap_or(0),
            reveal_sensitive.unwrap_or(false),
        )
        .await?;
    Ok(Json(items))
}

#[rocket::get("/api/v1/patients/search?<q>&<limit>&<offset>")]
pub async fn search_patients(
    state: &State<AppState>,
    user: CurrentUser,
    q: &str,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Json<Vec<PatientSearchResultDto>>, ApiError> {
    let items = state
        .app_service
        .search_patients(&user.0, q, limit.unwrap_or(30), offset.unwrap_or(0))
        .await?;
    Ok(Json(items))
}

#[rocket::get("/api/v1/patients/<patient_id>?<reveal_sensitive>")]
pub async fn get_patient(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    reveal_sensitive: Option<bool>,
) -> Result<Json<PatientProfileDto>, ApiError> {
    let item = state
        .app_service
        .get_patient(&user.0, patient_id, reveal_sensitive.unwrap_or(false))
        .await?;
    Ok(Json(item))
}

#[rocket::post("/api/v1/patients/<patient_id>/assign", data = "<payload>")]
pub async fn assign_patient(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    payload: Json<PatientAssignRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .assign_patient(
            &user.0,
            patient_id,
            payload.target_user_id,
            payload.assignment_type.trim(),
        )
        .await?;
    Ok(Json("assigned"))
}

#[rocket::put("/api/v1/patients/<patient_id>", data = "<payload>")]
pub async fn update_patient(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    payload: Json<PatientUpdateRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .update_patient(&user.0, patient_id, payload.into_inner())
        .await?;
    Ok(Json("updated"))
}

#[rocket::put("/api/v1/patients/<patient_id>/allergies", data = "<payload>")]
pub async fn edit_allergies(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    payload: Json<ClinicalEditRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .edit_clinical_field(&user.0, patient_id, "allergies", payload.into_inner())
        .await?;
    Ok(Json("updated"))
}

#[rocket::put("/api/v1/patients/<patient_id>/contraindications", data = "<payload>")]
pub async fn edit_contraindications(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    payload: Json<ClinicalEditRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .edit_clinical_field(
            &user.0,
            patient_id,
            "contraindications",
            payload.into_inner(),
        )
        .await?;
    Ok(Json("updated"))
}

#[rocket::put("/api/v1/patients/<patient_id>/history", data = "<payload>")]
pub async fn edit_history(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    payload: Json<ClinicalEditRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .edit_clinical_field(&user.0, patient_id, "history", payload.into_inner())
        .await?;
    Ok(Json("updated"))
}

#[rocket::post("/api/v1/patients/<patient_id>/visit-notes", data = "<payload>")]
pub async fn add_visit_note(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    payload: Json<VisitNoteRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .add_visit_note(&user.0, patient_id, payload.into_inner())
        .await?;
    Ok(Json("created"))
}

#[rocket::get("/api/v1/patients/<patient_id>/revisions?<reveal_sensitive>")]
pub async fn revisions(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    reveal_sensitive: Option<bool>,
) -> Result<Json<Vec<RevisionTimelineDto>>, ApiError> {
    let items = state
        .app_service
        .patient_revisions(&user.0, patient_id, reveal_sensitive.unwrap_or(false))
        .await?;
    Ok(Json(items))
}

#[rocket::post("/api/v1/patients/<patient_id>/attachments?<filename>&<mime_type>", data = "<data>")]
pub async fn upload_attachment(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    filename: &str,
    mime_type: &str,
    data: Data<'_>,
) -> Result<Json<&'static str>, ApiError> {
    let stream = data.open(26.megabytes()).into_bytes().await.map_err(|_| ApiError::Internal)?;
    if !stream.is_complete() {
        return Err(ApiError::PayloadTooLarge);
    }
    state
        .app_service
        .save_attachment(&user.0, patient_id, filename, mime_type, &stream.value)
        .await?;
    Ok(Json("uploaded"))
}

#[rocket::get("/api/v1/patients/<patient_id>/attachments")]
pub async fn list_attachments(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
) -> Result<Json<Vec<AttachmentMetadataDto>>, ApiError> {
    let items = state.app_service.list_attachments(&user.0, patient_id).await?;
    Ok(Json(items))
}

#[rocket::get("/api/v1/patients/<patient_id>/attachments/<attachment_id>/download")]
pub async fn download_attachment(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    attachment_id: i64,
) -> Result<(ContentType, Vec<u8>), ApiError> {
    let (mime, bytes) = state
        .app_service
        .download_attachment(&user.0, patient_id, attachment_id)
        .await?;
    let content_type = ContentType::parse_flexible(&mime).unwrap_or(ContentType::Binary);
    Ok((content_type, bytes))
}

#[rocket::get("/api/v1/patients/<patient_id>/export?<format>&<reveal_sensitive>")]
pub async fn export_patient(
    state: &State<AppState>,
    user: CurrentUser,
    patient_id: i64,
    format: Option<&str>,
    reveal_sensitive: Option<bool>,
) -> Result<Json<PatientExportDto>, ApiError> {
    let exported = state
        .app_service
        .export_patient(
            &user.0,
            patient_id,
            format.unwrap_or("json"),
            reveal_sensitive.unwrap_or(false),
        )
        .await?;
    Ok(Json(exported))
}
