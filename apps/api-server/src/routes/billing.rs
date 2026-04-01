use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};

use crate::{
    routes::auth_guard::{require_admin_session, require_session_email, require_user_session},
    services::auth_service::AuthService,
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
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<CreateBillingOrderRequest>,
) -> Result<(StatusCode, Json<CreateBillingOrderResponse>), MembershipError> {
    let session = require_user_session(&auth, &headers).map_err(MembershipError::from)?;
    require_session_email(&session, &request.email).map_err(MembershipError::from)?;
    Ok((StatusCode::CREATED, Json(service.create_order(request)?)))
}

async fn match_order(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<MatchBillingOrderRequest>,
) -> Result<Json<MatchBillingOrderResponse>, MembershipError> {
    require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.match_order(request)?))
}
