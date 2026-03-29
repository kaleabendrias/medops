use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{
    ApiError, DiningMenuDto, DiningMenuRequest, OrderCreateRequest, OrderDto, OrderNoteDto,
    OrderNoteRequest, OrderStatusRequest, TicketSplitDto, TicketSplitRequest,
};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::post("/api/v1/dining/menus", data = "<payload>")]
pub async fn create_menu(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<DiningMenuRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .create_menu(&user.0, payload.into_inner())
        .await?;
    Ok(Json("created"))
}

#[rocket::get("/api/v1/dining/menus")]
pub async fn list_menus(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<DiningMenuDto>>, ApiError> {
    Ok(Json(state.app_service.list_menus(&user.0).await?))
}

#[rocket::post("/api/v1/orders", data = "<payload>")]
pub async fn place_order(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<OrderCreateRequest>,
) -> Result<Json<i64>, ApiError> {
    let id = state.app_service.place_order(&user.0, payload.into_inner()).await?;
    Ok(Json(id))
}

#[rocket::put("/api/v1/orders/<order_id>/status", data = "<payload>")]
pub async fn update_order_status(
    state: &State<AppState>,
    user: CurrentUser,
    order_id: i64,
    payload: Json<OrderStatusRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .set_order_status(&user.0, order_id, payload.into_inner())
        .await?;
    Ok(Json("updated"))
}

#[rocket::get("/api/v1/orders?<limit>&<offset>")]
pub async fn list_orders(
    state: &State<AppState>,
    user: CurrentUser,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Json<Vec<OrderDto>>, ApiError> {
    Ok(Json(
        state
            .app_service
            .list_orders(&user.0, limit.unwrap_or(50), offset.unwrap_or(0))
            .await?,
    ))
}

#[rocket::post("/api/v1/orders/<order_id>/ticket-splits", data = "<payload>")]
pub async fn add_ticket_split(
    state: &State<AppState>,
    user: CurrentUser,
    order_id: i64,
    payload: Json<TicketSplitRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .add_ticket_split(&user.0, order_id, payload.into_inner())
        .await?;
    Ok(Json("added"))
}

#[rocket::get("/api/v1/orders/<order_id>/ticket-splits")]
pub async fn list_ticket_splits(
    state: &State<AppState>,
    user: CurrentUser,
    order_id: i64,
) -> Result<Json<Vec<TicketSplitDto>>, ApiError> {
    Ok(Json(
        state
            .app_service
            .list_ticket_splits(&user.0, order_id)
            .await?,
    ))
}

#[rocket::post("/api/v1/orders/<order_id>/notes", data = "<payload>")]
pub async fn add_order_note(
    state: &State<AppState>,
    user: CurrentUser,
    order_id: i64,
    payload: Json<OrderNoteRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .add_order_note(&user.0, order_id, payload.into_inner())
        .await?;
    Ok(Json("added"))
}

#[rocket::get("/api/v1/orders/<order_id>/notes")]
pub async fn list_order_notes(
    state: &State<AppState>,
    user: CurrentUser,
    order_id: i64,
) -> Result<Json<Vec<OrderNoteDto>>, ApiError> {
    Ok(Json(state.app_service.order_notes(&user.0, order_id).await?))
}
