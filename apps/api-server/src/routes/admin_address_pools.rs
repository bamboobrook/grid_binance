use axum::{
    extract::State,
    http::HeaderMap,
    routing::get,
    Json, Router,
};

use crate::{
    routes::auth_guard::require_admin_session,
    services::auth_service::AuthService,
    services::membership_service::{
        AddressPoolEntryResponse, AddressPoolListResponse, MembershipError, MembershipService,
        UpsertAddressPoolEntryRequest,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/address-pools", get(list_address_pools).post(upsert_address_pool))
}

async fn list_address_pools(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
) -> Result<Json<AddressPoolListResponse>, MembershipError> {
    require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.list_address_pools()?))
}

async fn upsert_address_pool(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<UpsertAddressPoolEntryRequest>,
) -> Result<Json<AddressPoolEntryResponse>, MembershipError> {
    let session = require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.upsert_address_pool_entry(&session.email, request)?))
}
