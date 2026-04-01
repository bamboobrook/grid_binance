use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use shared_events::{NotificationEvent, NotificationKind, NotificationRecord};

#[derive(Clone, Default)]
pub struct TelegramService {
    inner: Arc<Mutex<TelegramState>>,
}

#[derive(Default)]
struct TelegramState {
    next_bind_code: u64,
    bind_codes: HashMap<String, String>,
    bindings: HashMap<String, TelegramBinding>,
    inboxes: HashMap<String, Vec<NotificationRecord>>,
}

struct TelegramBinding {
    chat_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTelegramBindCodeRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct CreateTelegramBindCodeResponse {
    pub email: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct BindTelegramRequest {
    pub code: String,
    pub chat_id: String,
}

#[derive(Debug, Serialize)]
pub struct BindTelegramResponse {
    pub email: String,
    pub chat_id: String,
}

#[derive(Debug, Deserialize)]
pub struct DispatchNotificationRequest {
    pub email: String,
    pub kind: NotificationKind,
    pub title: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct NotificationInboxQuery {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationInboxResponse {
    pub email: String,
    pub items: Vec<NotificationRecord>,
}

impl TelegramService {
    pub fn create_bind_code(
        &self,
        request: CreateTelegramBindCodeRequest,
    ) -> Result<CreateTelegramBindCodeResponse, TelegramError> {
        let email = normalize_email(&request.email);
        if email.is_empty() {
            return Err(TelegramError::bad_request("email is required"));
        }

        let mut inner = self.inner.lock().expect("telegram state poisoned");
        inner.next_bind_code += 1;
        let code = format!("tg-bind-{:06}", inner.next_bind_code);
        inner.bind_codes.insert(code.clone(), email.clone());

        Ok(CreateTelegramBindCodeResponse { email, code })
    }

    pub fn bind_telegram(
        &self,
        request: BindTelegramRequest,
    ) -> Result<BindTelegramResponse, TelegramError> {
        let code = request.code.trim();
        let chat_id = request.chat_id.trim();
        if code.is_empty() || chat_id.is_empty() {
            return Err(TelegramError::bad_request("code and chat_id are required"));
        }

        let mut inner = self.inner.lock().expect("telegram state poisoned");
        let email = inner
            .bind_codes
            .remove(code)
            .ok_or_else(|| TelegramError::not_found("bind code not found"))?;

        inner.bindings.insert(
            email.clone(),
            TelegramBinding {
                chat_id: chat_id.to_owned(),
            },
        );

        Ok(BindTelegramResponse {
            email,
            chat_id: chat_id.to_owned(),
        })
    }

    pub fn dispatch_notification(
        &self,
        request: DispatchNotificationRequest,
    ) -> Result<NotificationRecord, TelegramError> {
        let email = normalize_email(&request.email);
        let title = request.title.trim().to_owned();
        let message = request.message.trim().to_owned();
        if email.is_empty() || title.is_empty() || message.is_empty() {
            return Err(TelegramError::bad_request(
                "email, title, and message are required",
            ));
        }

        let mut inner = self.inner.lock().expect("telegram state poisoned");
        let telegram_delivered = inner
            .bindings
            .get(&email)
            .map(|binding| !binding.chat_id.is_empty())
            .unwrap_or(false);
        let show_expiry_popup = matches!(&request.kind, NotificationKind::MembershipExpiring);

        let record = NotificationRecord {
            event: NotificationEvent {
                email: email.clone(),
                kind: request.kind,
                title,
                message,
            },
            telegram_delivered,
            in_app_delivered: true,
            show_expiry_popup,
        };

        inner.inboxes.entry(email).or_default().push(record.clone());

        Ok(record)
    }

    pub fn list_notifications(
        &self,
        query: NotificationInboxQuery,
    ) -> Result<NotificationInboxResponse, TelegramError> {
        let email = normalize_email(&query.email);
        if email.is_empty() {
            return Err(TelegramError::bad_request("email is required"));
        }

        let inner = self.inner.lock().expect("telegram state poisoned");
        let items = inner.inboxes.get(&email).cloned().unwrap_or_default();

        Ok(NotificationInboxResponse { email, items })
    }
}

#[derive(Debug)]
pub struct TelegramError {
    status: StatusCode,
    message: &'static str,
}

impl TelegramError {
    fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }

    fn not_found(message: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
        }
    }
}

impl IntoResponse for TelegramError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(TelegramErrorResponse {
                error: self.message.to_owned(),
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Serialize)]
struct TelegramErrorResponse {
    error: String,
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}
