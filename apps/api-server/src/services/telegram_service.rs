use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use getrandom::getrandom;
use serde::{Deserialize, Serialize};
use shared_events::{NotificationEvent, NotificationKind, NotificationRecord};

const DEFAULT_BIND_CODE_TTL_SECONDS: i64 = 300;

#[derive(Clone, Default)]
pub struct TelegramService {
    inner: Arc<Mutex<TelegramState>>,
}

#[derive(Default)]
struct TelegramState {
    bind_codes: HashMap<String, BindCodeRecord>,
    active_codes_by_email: HashMap<String, String>,
    bindings: HashMap<String, TelegramBinding>,
    inboxes: HashMap<String, Vec<NotificationRecord>>,
}

struct BindCodeRecord {
    email: String,
    expires_at: DateTime<Utc>,
}

struct TelegramBinding {
    chat_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTelegramBindCodeRequest {
    pub email: String,
    pub ttl_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CreateTelegramBindCodeResponse {
    pub email: String,
    pub code: String,
    pub expires_at: DateTime<Utc>,
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
        if let Some(previous_code) = inner.active_codes_by_email.remove(&email) {
            inner.bind_codes.remove(&previous_code);
        }

        let ttl_seconds = request.ttl_seconds.unwrap_or(DEFAULT_BIND_CODE_TTL_SECONDS);
        let expires_at = Utc::now() + Duration::seconds(ttl_seconds);
        let code = generate_bind_code(&inner);

        inner.bind_codes.insert(
            code.clone(),
            BindCodeRecord {
                email: email.clone(),
                expires_at,
            },
        );
        inner
            .active_codes_by_email
            .insert(email.clone(), code.clone());

        Ok(CreateTelegramBindCodeResponse {
            email,
            code,
            expires_at,
        })
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
        let bind_code = inner
            .bind_codes
            .remove(code)
            .ok_or_else(|| TelegramError::not_found("bind code not found"))?;

        if Utc::now() > bind_code.expires_at {
            match inner.active_codes_by_email.get(&bind_code.email) {
                Some(active_code) if active_code == code => {
                    inner.active_codes_by_email.remove(&bind_code.email);
                }
                _ => {}
            }

            return Err(TelegramError::not_found("bind code expired"));
        }

        inner.active_codes_by_email.remove(&bind_code.email);

        inner.bindings.insert(
            bind_code.email.clone(),
            TelegramBinding {
                chat_id: chat_id.to_owned(),
            },
        );

        Ok(BindTelegramResponse {
            email: bind_code.email,
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

fn generate_bind_code(state: &TelegramState) -> String {
    loop {
        let mut bytes = [0_u8; 16];
        getrandom(&mut bytes).expect("os randomness available");
        let code = format!("tg-bind-{}", hex_encode(&bytes));
        if !state.bind_codes.contains_key(&code) {
            return code;
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(nibble_to_hex(byte >> 4));
        encoded.push(nibble_to_hex(byte & 0x0f));
    }
    encoded
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => unreachable!("nibble value out of range"),
    }
}
