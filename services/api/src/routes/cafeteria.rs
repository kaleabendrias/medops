use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{
    ApiError, DishCategoryDto, DishCreateRequest, DishDto, DishOptionRequest, DishStatusRequest,
    DishWindowRequest, RankingRuleDto, RankingRuleRequest, RecommendationDto,
};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::get("/api/v1/cafeteria/categories")]
pub async fn categories(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<DishCategoryDto>>, ApiError> {
    Ok(Json(state.app_service.list_dish_categories(&user.0).await?))
}

#[rocket::post("/api/v1/cafeteria/dishes", data = "<payload>")]
pub async fn create_dish(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<DishCreateRequest>,
) -> Result<Json<i64>, ApiError> {
    Ok(Json(state.app_service.create_dish(&user.0, payload.into_inner()).await?))
}

#[rocket::get("/api/v1/cafeteria/dishes")]
pub async fn list_dishes(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<DishDto>>, ApiError> {
    Ok(Json(state.app_service.list_dishes(&user.0).await?))
}

#[rocket::put("/api/v1/cafeteria/dishes/<dish_id>/status", data = "<payload>")]
pub async fn dish_status(
    state: &State<AppState>,
    user: CurrentUser,
    dish_id: i64,
    payload: Json<DishStatusRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .set_dish_status(&user.0, dish_id, payload.into_inner())
        .await?;
    Ok(Json("updated"))
}

#[rocket::post("/api/v1/cafeteria/dishes/<dish_id>/options", data = "<payload>")]
pub async fn add_option(
    state: &State<AppState>,
    user: CurrentUser,
    dish_id: i64,
    payload: Json<DishOptionRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .add_dish_option(&user.0, dish_id, payload.into_inner())
        .await?;
    Ok(Json("added"))
}

#[rocket::post("/api/v1/cafeteria/dishes/<dish_id>/windows", data = "<payload>")]
pub async fn add_window(
    state: &State<AppState>,
    user: CurrentUser,
    dish_id: i64,
    payload: Json<DishWindowRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .add_sales_window(&user.0, dish_id, payload.into_inner())
        .await?;
    Ok(Json("added"))
}

#[rocket::put("/api/v1/cafeteria/ranking-rules", data = "<payload>")]
pub async fn upsert_rule(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<RankingRuleRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .upsert_ranking_rule(&user.0, payload.into_inner())
        .await?;
    Ok(Json("updated"))
}

#[rocket::get("/api/v1/cafeteria/ranking-rules")]
pub async fn rules(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<RankingRuleDto>>, ApiError> {
    Ok(Json(state.app_service.ranking_rules(&user.0).await?))
}

#[rocket::get("/api/v1/cafeteria/recommendations")]
pub async fn recommendations(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<RecommendationDto>>, ApiError> {
    Ok(Json(state.app_service.recommendations(&user.0).await?))
}
