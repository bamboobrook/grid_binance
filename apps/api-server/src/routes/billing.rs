use axum::{extract::State, http::StatusCode, routing::post, Json, Router};

use crate::{
    services::membership_service::{
        CreateBillingOrderRequest, CreateBillingOrderResponse, MatchBillingOrderRequest,
        MatchBillingOrderResponse, MembershipError, MembershipService,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/billing/orders", post(create_order))
        .route("/billing/orders/match", post(match_order))
}

async fn create_order(
    State(service): State<MembershipService>,
    Json(request): Json<CreateBillingOrderRequest>,
) -> Result<(StatusCode, Json<CreateBillingOrderResponse>), MembershipError> {
    Ok((StatusCode::CREATED, Json(service.create_order(request)?)))
}

async fn match_order(
    State(service): State<MembershipService>,
    Json(request): Json<MatchBillingOrderRequest>,
) -> Result<Json<MatchBillingOrderResponse>, MembershipError> {
    Ok(Json(service.match_order(request)?))
}
