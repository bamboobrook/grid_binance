use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};

use crate::{
    routes::auth_guard::{require_session_email, require_user_session},
    services::auth_service::{AuthError, AuthService},
    services::telegram_service::{
        BindTelegramRequest, BindTelegramResponse, BotBindTelegramRequest,
        CreateTelegramBindCodeRequest, CreateTelegramBindCodeResponse,
        DispatchNotificationRequest, NotificationInboxQuery, NotificationInboxResponse,
        TelegramService,
    },
    AppState,
};
use shared_events::NotificationRecord;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/telegram/bind-codes", post(create_bind_code))
        .route("/telegram/bind", post(bind_telegram))
        .route("/telegram/bot/bind", post(bind_telegram_from_bot))
        .route("/notifications/dispatch", post(dispatch_notification))
        .route("/notifications", get(list_notifications))
}

async fn create_bind_code(
    State(auth): State<AuthService>,
    State(service): State<TelegramService>,
    headers: HeaderMap,
    Json(request): Json<CreateTelegramBindCodeRequest>,
) -> Result<(StatusCode, Json<CreateTelegramBindCodeResponse>), AuthError> {
    let session = require_user_session(&auth, &headers)?;
    require_session_email(&session, &request.email)?;
    Ok((
        StatusCode::CREATED,
        Json(service.create_bind_code(request).map_err(AuthError::from)?),
    ))
}

async fn bind_telegram(
    State(auth): State<AuthService>,
    State(service): State<TelegramService>,
    headers: HeaderMap,
    Json(request): Json<BindTelegramRequest>,
) -> Result<Json<BindTelegramResponse>, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    if let Some(owner) = service.bind_code_owner(&request.code) {
        require_session_email(&session, &owner)?;
    }
    Ok(Json(service.bind_telegram(request).map_err(AuthError::from)?))
}

async fn bind_telegram_from_bot(
    State(service): State<TelegramService>,
    headers: HeaderMap,
    Json(request): Json<BotBindTelegramRequest>,
) -> Result<Json<BindTelegramResponse>, AuthError> {
    let bot_secret = headers
        .get("x-telegram-bot-secret")
        .and_then(|value| value.to_str().ok());
    service.authorize_bot_secret(bot_secret).map_err(AuthError::from)?;
    Ok(Json(
        service
            .bind_telegram_from_bot(request)
            .map_err(AuthError::from)?,
    ))
}

async fn dispatch_notification(
    State(auth): State<AuthService>,
    State(service): State<TelegramService>,
    headers: HeaderMap,
    Json(request): Json<DispatchNotificationRequest>,
) -> Result<Json<NotificationRecord>, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    require_session_email(&session, &request.email)?;
    Ok(Json(
        service
            .dispatch_notification(request)
            .map_err(AuthError::from)?,
    ))
}

async fn list_notifications(
    State(auth): State<AuthService>,
    State(service): State<TelegramService>,
    headers: HeaderMap,
    Query(query): Query<NotificationInboxQuery>,
) -> Result<Json<NotificationInboxResponse>, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    require_session_email(&session, &query.email)?;
    Ok(Json(service.list_notifications(query).map_err(AuthError::from)?))
}
