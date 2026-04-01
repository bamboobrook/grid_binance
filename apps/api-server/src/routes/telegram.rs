use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use crate::{
    services::telegram_service::{
        BindTelegramRequest, BindTelegramResponse, CreateTelegramBindCodeRequest,
        CreateTelegramBindCodeResponse, DispatchNotificationRequest, NotificationInboxQuery,
        NotificationInboxResponse, TelegramError, TelegramService,
    },
    AppState,
};
use shared_events::NotificationRecord;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/telegram/bind-codes", post(create_bind_code))
        .route("/telegram/bind", post(bind_telegram))
        .route("/notifications/dispatch", post(dispatch_notification))
        .route("/notifications", get(list_notifications))
}

async fn create_bind_code(
    State(service): State<TelegramService>,
    Json(request): Json<CreateTelegramBindCodeRequest>,
) -> Result<(StatusCode, Json<CreateTelegramBindCodeResponse>), TelegramError> {
    Ok((
        StatusCode::CREATED,
        Json(service.create_bind_code(request)?),
    ))
}

async fn bind_telegram(
    State(service): State<TelegramService>,
    Json(request): Json<BindTelegramRequest>,
) -> Result<Json<BindTelegramResponse>, TelegramError> {
    Ok(Json(service.bind_telegram(request)?))
}

async fn dispatch_notification(
    State(service): State<TelegramService>,
    Json(request): Json<DispatchNotificationRequest>,
) -> Result<Json<NotificationRecord>, TelegramError> {
    Ok(Json(service.dispatch_notification(request)?))
}

async fn list_notifications(
    State(service): State<TelegramService>,
    Query(query): Query<NotificationInboxQuery>,
) -> Result<Json<NotificationInboxResponse>, TelegramError> {
    Ok(Json(service.list_notifications(query)?))
}
