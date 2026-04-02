use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use chrono::Utc;
use serde::Serialize;
use shared_db::SharedDb;
use shared_domain::membership::{MembershipSnapshot, MembershipStatus};

use crate::{
    routes::auth_guard::require_admin_session,
    services::auth_service::{AuthError, AuthService},
    AppState,
};

#[derive(Debug, Serialize)]
pub struct AdminUserSummary {
    pub email: String,
    pub membership: MembershipSnapshot,
    pub latest_order_status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminUserListResponse {
    pub items: Vec<AdminUserSummary>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/users", get(list_users))
}

async fn list_users(
    State(auth): State<AuthService>,
    State(db): State<SharedDb>,
    headers: HeaderMap,
) -> Result<Json<AdminUserListResponse>, AuthError> {
    require_admin_session(&auth, &headers)?;
    let memberships = db
        .list_membership_records()
        .map_err(|error| AuthError::storage(error))?;
    let orders = db
        .list_billing_orders()
        .map_err(|error| AuthError::storage(error))?;
    let items = memberships
        .into_iter()
        .map(|(email, record): (String, shared_db::MembershipRecord)| {
            let latest_order_status = orders
                .iter()
                .filter(|order| order.email.eq_ignore_ascii_case(&email))
                .max_by_key(|order| order.requested_at)
                .map(|order| order.status.clone());
            AdminUserSummary {
                email: email.clone(),
                membership: MembershipSnapshot {
                    email,
                    status: derive_status(&record, Utc::now()),
                    active_until: record.active_until,
                    grace_until: record.grace_until,
                    override_status: record.override_status,
                },
                latest_order_status,
            }
        })
        .collect();
    Ok(Json(AdminUserListResponse { items }))
}

fn derive_status(record: &shared_db::MembershipRecord, now: chrono::DateTime<chrono::Utc>) -> MembershipStatus {
    if let Some(status) = record.override_status.clone() {
        return status;
    }
    if record.active_until.is_some_and(|active_until| now <= active_until) {
        return MembershipStatus::Active;
    }
    if record.grace_until.is_some_and(|grace_until| now <= grace_until) {
        return MembershipStatus::Grace;
    }
    if record.activated_at.is_some() {
        return MembershipStatus::Expired;
    }
    MembershipStatus::Pending
}
