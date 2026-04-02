use std::collections::{BTreeMap, BTreeSet};

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
    pub registered: bool,
    pub email_verified: bool,
    pub totp_enabled: bool,
    pub admin_role: Option<String>,
    pub membership: Option<MembershipSnapshot>,
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
    let auth_users = db.list_auth_users().map_err(AuthError::storage)?;
    let memberships = db.list_membership_records().map_err(AuthError::storage)?;
    let orders = db.list_billing_orders().map_err(AuthError::storage)?;

    let mut items = BTreeMap::<String, AdminUserSummary>::new();
    let mut ordered_emails = BTreeSet::<String>::new();
    let mut latest_orders = BTreeMap::<String, (chrono::DateTime<chrono::Utc>, String)>::new();

    for user in auth_users {
        let email = user.email.clone();
        ordered_emails.insert(email.clone());
        items.insert(
            email.clone(),
            AdminUserSummary {
                admin_role: auth
                    .admin_role_for_email(&email)
                    .map(|role| role.as_str().to_owned()),
                email,
                email_verified: user.email_verified,
                latest_order_status: None,
                membership: None,
                registered: true,
                totp_enabled: user.totp_enabled,
            },
        );
    }

    for order in orders {
        ordered_emails.insert(order.email.clone());
        let current = latest_orders.get(&order.email).map(|(at, _)| *at);
        if current.is_none_or(|seen| order.requested_at > seen) {
            latest_orders.insert(order.email.clone(), (order.requested_at, order.status));
        }
        items
            .entry(order.email.clone())
            .or_insert_with(|| empty_user_summary(&auth, &order.email));
    }

    for (email, record) in memberships {
        ordered_emails.insert(email.clone());
        let entry = items
            .entry(email.clone())
            .or_insert_with(|| empty_user_summary(&auth, &email));
        entry.membership = Some(MembershipSnapshot {
            email: email.clone(),
            status: derive_status(&record, Utc::now()),
            active_until: record.active_until,
            grace_until: record.grace_until,
            override_status: record.override_status,
        });
    }

    for (email, (_, status)) in latest_orders {
        if let Some(entry) = items.get_mut(&email) {
            entry.latest_order_status = Some(status);
        }
    }

    let items = ordered_emails
        .into_iter()
        .filter_map(|email| items.remove(&email))
        .collect();

    Ok(Json(AdminUserListResponse { items }))
}

fn empty_user_summary(auth: &AuthService, email: &str) -> AdminUserSummary {
    AdminUserSummary {
        admin_role: auth
            .admin_role_for_email(email)
            .map(|role| role.as_str().to_owned()),
        email: email.to_owned(),
        email_verified: false,
        latest_order_status: None,
        membership: None,
        registered: false,
        totp_enabled: false,
    }
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
