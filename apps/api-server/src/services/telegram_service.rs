use std::{
    collections::HashMap,
    env, thread,
    sync::{Arc, Mutex, OnceLock},
    time::Duration as StdDuration,
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use getrandom::getrandom;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_db::{NotificationLogRecord, SharedDb, SystemConfigRecord, TelegramBindingRecord};
use shared_events::{NotificationEvent, NotificationKind, NotificationRecord};

use crate::services::auth_service::AuthError;

const DEFAULT_BIND_CODE_TTL_SECONDS: i64 = 300;
const MAX_BIND_CODE_TTL_SECONDS: i64 = 86_400;
const NOTIFICATION_INBOX_LIMIT: usize = 100;
const DEFAULT_TELEGRAM_BOT_BIND_SECRET: &str = "grid-binance-dev-telegram-bot-bind-secret";
const DEFAULT_TELEGRAM_API_BASE_URL: &str = "https://api.telegram.org";
const TELEGRAM_BOT_UPDATE_OFFSET_CONFIG_KEY: &str = "telegram.bot.last_update_id";
const TELEGRAM_BOT_POLL_TIMEOUT_SECONDS: u64 = 2;
const TELEGRAM_BOT_IDLE_SLEEP_MILLIS: u64 = 500;
const TELEGRAM_BOT_ERROR_SLEEP_MILLIS: u64 = 2_000;

#[derive(Clone)]
pub struct TelegramService {
    db: SharedDb,
    inner: Arc<Mutex<TelegramState>>,
    bot_bind_secret: Arc<String>,
    telegram_bot_token: Arc<Option<String>>,
    telegram_api_base_url: Arc<String>,
}

impl Default for TelegramService {
    fn default() -> Self {
        Self::new(SharedDb::ephemeral().expect("ephemeral telegram db should initialize"))
    }
}

#[derive(Default)]
struct TelegramState {
    bind_codes: HashMap<String, BindCodeRecord>,
    active_codes_by_email: HashMap<String, String>,
}

struct BindCodeRecord {
    email: String,
    expires_at: DateTime<Utc>,
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
    #[allow(dead_code)]
    pub chat_id: String,
}

#[derive(Debug, Deserialize)]
pub struct BotBindTelegramRequest {
    pub code: String,
    pub telegram_user_id: String,
    pub chat_id: String,
    pub username: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BindTelegramResponse {
    pub email: String,
    pub chat_id: String,
    pub telegram_user_id: String,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramBindingStatusQuery {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct TelegramBindingStatusResponse {
    pub email: String,
    pub bound: bool,
    pub bound_at: Option<DateTime<Utc>>,
    pub chat_id: Option<String>,
    pub telegram_user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DispatchNotificationRequest {
    pub email: String,
    pub kind: NotificationKind,
    pub title: String,
    pub message: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
pub struct NotificationInboxQuery {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationInboxItem {
    pub event: NotificationEvent,
    pub telegram_delivered: bool,
    pub in_app_delivered: bool,
    pub show_expiry_popup: bool,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct NotificationInboxResponse {
    pub email: String,
    pub items: Vec<NotificationInboxItem>,
}

#[derive(Debug, Deserialize)]
struct TelegramGetUpdatesResponse {
    ok: bool,
    result: Vec<TelegramBotUpdate>,
}

#[derive(Debug, Deserialize, Clone)]
struct TelegramBotUpdate {
    update_id: i64,
    message: Option<TelegramBotMessage>,
}

#[derive(Debug, Deserialize, Clone)]
struct TelegramBotMessage {
    text: Option<String>,
    chat: TelegramBotChat,
    from: Option<TelegramBotUser>,
}

#[derive(Debug, Deserialize, Clone)]
struct TelegramBotChat {
    id: i64,
}

#[derive(Debug, Deserialize, Clone)]
struct TelegramBotUser {
    id: i64,
    username: Option<String>,
}

impl TelegramService {
    pub fn new(db: SharedDb) -> Self {
        Self {
            db,
            inner: Arc::new(Mutex::new(TelegramState::default())),
            bot_bind_secret: Arc::new(
                env::var("TELEGRAM_BOT_BIND_SECRET")
                    .unwrap_or_else(|_| DEFAULT_TELEGRAM_BOT_BIND_SECRET.to_string()),
            ),
            telegram_bot_token: Arc::new(
                env::var("TELEGRAM_BOT_TOKEN")
                    .ok()
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty()),
            ),
            telegram_api_base_url: Arc::new(
                env::var("TELEGRAM_API_BASE_URL")
                    .ok()
                    .map(|value| value.trim().trim_end_matches('/').to_owned())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| DEFAULT_TELEGRAM_API_BASE_URL.to_string()),
            ),
        }
    }

    pub fn new_strict(db: SharedDb) -> Result<Self, shared_db::SharedDbError> {
        let bot_bind_secret = env::var("TELEGRAM_BOT_BIND_SECRET")
            .map_err(|_| shared_db::SharedDbError::new("TELEGRAM_BOT_BIND_SECRET is required"))?;
        Ok(Self {
            db,
            inner: Arc::new(Mutex::new(TelegramState::default())),
            bot_bind_secret: Arc::new(bot_bind_secret),
            telegram_bot_token: Arc::new(
                env::var("TELEGRAM_BOT_TOKEN")
                    .ok()
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty()),
            ),
            telegram_api_base_url: Arc::new(
                env::var("TELEGRAM_API_BASE_URL")
                    .ok()
                    .map(|value| value.trim().trim_end_matches('/').to_owned())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| DEFAULT_TELEGRAM_API_BASE_URL.to_string()),
            ),
        })
    }

    pub fn spawn_bot_update_poller(&self) {
        static POLLER_STARTED: OnceLock<()> = OnceLock::new();
        if self.telegram_bot_token.as_ref().as_ref().is_none() {
            return;
        }
        if POLLER_STARTED.set(()).is_err() {
            return;
        }

        let service = self.clone();
        thread::spawn(move || service.run_bot_update_poller());
    }

    fn run_bot_update_poller(self) {
        let Some(bot_token) = self.telegram_bot_token.as_ref().as_ref().cloned() else {
            return;
        };
        let mut offset = self.read_bot_update_offset().unwrap_or(0);

        loop {
            match self.fetch_bot_updates(&bot_token, offset) {
                Ok(updates) => {
                    for update in updates {
                        if update.update_id >= offset {
                            offset = update.update_id + 1;
                        }
                        if let Some(message) = update.message.as_ref() {
                            match self.process_bind_command_message(message) {
                                Ok(Some(reply)) => {
                                    let _ = self.send_telegram_message(
                                        &bot_token,
                                        &message.chat.id.to_string(),
                                        "GridBinance",
                                        &reply,
                                    );
                                }
                                Ok(None) => {}
                                Err(error) => {
                                    eprintln!(
                                        "telegram bot update processing failed: {}",
                                        error.message
                                    );
                                    let _ = self.send_telegram_message(
                                        &bot_token,
                                        &message.chat.id.to_string(),
                                        "GridBinance",
                                        "系统暂时无法处理绑定，请稍后重试。",
                                    );
                                }
                            }
                        }
                        let _ = self.store_bot_update_offset(offset);
                    }
                    thread::sleep(StdDuration::from_millis(TELEGRAM_BOT_IDLE_SLEEP_MILLIS));
                }
                Err(error) => {
                    eprintln!("telegram bot polling failed: {}", error.message);
                    thread::sleep(StdDuration::from_millis(TELEGRAM_BOT_ERROR_SLEEP_MILLIS));
                }
            }
        }
    }

    fn fetch_bot_updates(&self, bot_token: &str, offset: i64) -> Result<Vec<TelegramBotUpdate>, TelegramError> {
        let http = telegram_http_client()?;
        let mut request = http
            .get(&format!(
                "{}/bot{}/getUpdates",
                self.telegram_api_base_url.as_str(),
                bot_token
            ))
            .query("timeout", &TELEGRAM_BOT_POLL_TIMEOUT_SECONDS.to_string())
            .query("limit", "20");
        if offset > 0 {
            request = request.query("offset", &offset.to_string());
        }

        let response: TelegramGetUpdatesResponse = request
            .call()
            .map_err(|error| TelegramError::storage_message(error.to_string()))?
            .into_json()
            .map_err(|error| TelegramError::storage_message(error.to_string()))?;
        if !response.ok {
            return Err(TelegramError::storage_message(
                "telegram getUpdates returned not ok".to_string(),
            ));
        }
        Ok(response.result)
    }

    fn read_bot_update_offset(&self) -> Result<i64, TelegramError> {
        Ok(self
            .db
            .get_system_config(TELEGRAM_BOT_UPDATE_OFFSET_CONFIG_KEY)
            .map_err(TelegramError::storage)?
            .and_then(|record| record.config_value.as_i64())
            .unwrap_or(0))
    }

    fn store_bot_update_offset(&self, offset: i64) -> Result<(), TelegramError> {
        self.db
            .upsert_system_config(&SystemConfigRecord {
                config_key: TELEGRAM_BOT_UPDATE_OFFSET_CONFIG_KEY.to_string(),
                config_value: serde_json::json!(offset),
                updated_at: Utc::now(),
            })
            .map_err(TelegramError::storage)
    }

    fn process_bind_command_message(
        &self,
        message: &TelegramBotMessage,
    ) -> Result<Option<String>, TelegramError> {
        let text = message.text.as_deref().map(str::trim).unwrap_or("");
        if text.is_empty() {
            return Ok(None);
        }

        if let Some(code) = extract_bind_code_from_bind_command(text) {
            let Some(from) = message.from.as_ref() else {
                return Ok(Some("无法识别你的 Telegram 账户，请稍后重试。".to_string()));
            };
            let owner_email = self.bind_code_owner(&code);
            let replacing_existing_binding = owner_email
                .as_deref()
                .and_then(|email| self.db.find_telegram_binding(email).ok().flatten())
                .map(|binding| {
                    binding.telegram_chat_id != message.chat.id.to_string()
                        || binding.telegram_user_id != from.id.to_string()
                })
                .unwrap_or(false);

            return match self.bind_telegram_from_bot(BotBindTelegramRequest {
                code,
                telegram_user_id: from.id.to_string(),
                chat_id: message.chat.id.to_string(),
                username: from.username.clone(),
            }) {
                Ok(_) if replacing_existing_binding => Ok(Some(
                    "新的 Telegram 账号已绑定成功，旧绑定已自动失效。回到网页刷新即可查看最新状态。".to_string(),
                )),
                Ok(_) => Ok(Some("绑定成功，回到网页刷新即可看到已绑定状态。".to_string())),
                Err(error) if error.status == StatusCode::NOT_FOUND => Ok(Some(
                    "绑定码无效、已过期或已被使用，请回到网页重新生成绑定码。".to_string(),
                )),
                Err(error) if error.status == StatusCode::BAD_REQUEST => Ok(Some(
                    "绑定命令格式不正确，请使用 /bind 绑定码。".to_string(),
                )),
                Err(error) => Err(error),
            };
        }

        if let Some(code) = extract_bind_code_from_start_command(text) {
            return Ok(Some(format!(
                "欢迎使用 GridBinance 机器人。请继续发送 /bind {} 完成 Telegram 绑定。",
                code
            )));
        }

        if is_start_command(text) {
            return Ok(Some(
                "欢迎使用 GridBinance 机器人。请先回到网页生成绑定码，然后发送 /bind 绑定码 完成绑定。".to_string(),
            ));
        }

        if is_bind_command(text) {
            return Ok(Some(
                "绑定格式：/bind 绑定码。请先在网页生成新的绑定码后再发送。".to_string(),
            ));
        }

        if text.starts_with("tg-bind-") {
            return Ok(Some(format!(
                "请发送 /bind {} 完成绑定。",
                text
            )));
        }

        Ok(None)
    }

    pub fn bind_code_owner(&self, code: &str) -> Option<String> {
        let inner = self.inner.lock().expect("telegram state poisoned");
        inner
            .bind_codes
            .get(code.trim())
            .map(|record| record.email.clone())
    }

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

        let ttl_seconds = match request.ttl_seconds {
            Some(ttl_seconds) if (0..=MAX_BIND_CODE_TTL_SECONDS).contains(&ttl_seconds) => {
                ttl_seconds
            }
            Some(_) => {
                return Err(TelegramError::bad_request(
                    "ttl_seconds must be between 0 and 86400",
                ));
            }
            None => DEFAULT_BIND_CODE_TTL_SECONDS,
        };
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
        _request: BindTelegramRequest,
    ) -> Result<BindTelegramResponse, TelegramError> {
        Err(TelegramError::forbidden("telegram bot bind flow required"))
    }

    pub fn authorize_bot_secret(&self, provided_secret: Option<&str>) -> Result<(), TelegramError> {
        match provided_secret {
            Some(secret) if secret == self.bot_bind_secret.as_str() => Ok(()),
            _ => Err(TelegramError::unauthorized(
                "telegram bot authentication failed",
            )),
        }
    }

    pub fn bind_telegram_from_bot(
        &self,
        request: BotBindTelegramRequest,
    ) -> Result<BindTelegramResponse, TelegramError> {
        let code = request.code.trim();
        let telegram_user_id = request.telegram_user_id.trim();
        let chat_id = request.chat_id.trim();
        if code.is_empty() || telegram_user_id.is_empty() || chat_id.is_empty() {
            return Err(TelegramError::bad_request(
                "code, telegram_user_id, and chat_id are required",
            ));
        }

        let mut inner = self.inner.lock().expect("telegram state poisoned");
        let bind_code = inner
            .bind_codes
            .remove(code)
            .ok_or_else(|| TelegramError::not_found("bind code not found"))?;

        if Utc::now() > bind_code.expires_at {
            if inner
                .active_codes_by_email
                .get(&bind_code.email)
                .is_some_and(|active_code| active_code == code)
            {
                inner.active_codes_by_email.remove(&bind_code.email);
            }

            return Err(TelegramError::not_found("bind code expired"));
        }

        inner.active_codes_by_email.remove(&bind_code.email);
        drop(inner);

        self.db
            .upsert_telegram_binding(&TelegramBindingRecord {
                user_email: bind_code.email.clone(),
                telegram_user_id: telegram_user_id.to_owned(),
                telegram_chat_id: chat_id.to_owned(),
                bound_at: Utc::now(),
            })
            .map_err(TelegramError::storage)?;

        Ok(BindTelegramResponse {
            email: bind_code.email,
            chat_id: chat_id.to_owned(),
            telegram_user_id: telegram_user_id.to_owned(),
            username: request.username.map(|value| value.trim().to_owned()),
        })
    }

    pub fn binding_status(
        &self,
        query: TelegramBindingStatusQuery,
    ) -> Result<TelegramBindingStatusResponse, TelegramError> {
        let email = normalize_email(&query.email);
        if email.is_empty() {
            return Err(TelegramError::bad_request("email is required"));
        }

        let binding = self
            .db
            .find_telegram_binding(&email)
            .map_err(TelegramError::storage)?;

        Ok(TelegramBindingStatusResponse {
            email,
            bound: binding.is_some(),
            bound_at: binding.as_ref().map(|record| record.bound_at),
            chat_id: binding
                .as_ref()
                .map(|record| record.telegram_chat_id.clone()),
            telegram_user_id: binding
                .as_ref()
                .map(|record| record.telegram_user_id.clone()),
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

        let binding = self
            .db
            .find_telegram_binding(&email)
            .map_err(TelegramError::storage)?;
        let telegram_delivered = binding
            .as_ref()
            .map(|binding| !binding.telegram_chat_id.is_empty())
            .unwrap_or(false);
        let show_expiry_popup = matches!(&request.kind, NotificationKind::MembershipExpiring);

        let record = NotificationRecord {
            event: NotificationEvent {
                email: email.clone(),
                kind: request.kind,
                title: title.clone(),
                message: message.clone(),
                payload: json_value_to_payload_map(request.payload),
            },
            telegram_delivered,
            in_app_delivered: true,
            show_expiry_popup,
        };

        let now = Utc::now();
        let payload = serde_json::to_value(&record)
            .map_err(|error| TelegramError::storage_message(error.to_string()))?;
        self.db
            .insert_notification_log(&NotificationLogRecord {
                user_email: email.clone(),
                channel: "in_app".to_string(),
                template_key: Some(notification_kind_key(&record.event.kind).to_string()),
                title: title.clone(),
                body: message.clone(),
                status: "delivered".to_string(),
                payload: payload.clone(),
                created_at: now,
                delivered_at: Some(now),
            })
            .map_err(TelegramError::storage)?;

        if let Some(binding) = binding.as_ref() {
            if let Some(token) = self.telegram_bot_token.as_ref().as_ref() {
                let send_result =
                    self.send_telegram_message(token, &binding.telegram_chat_id, &title, &message);
                let (status, delivered_at) = if send_result.is_ok() {
                    ("delivered".to_string(), Some(now))
                } else {
                    ("failed".to_string(), None)
                };
                self.db
                    .insert_notification_log(&NotificationLogRecord {
                        user_email: email,
                        channel: "telegram".to_string(),
                        template_key: Some(notification_kind_key(&record.event.kind).to_string()),
                        title,
                        body: message,
                        status,
                        payload,
                        created_at: now,
                        delivered_at,
                    })
                    .map_err(TelegramError::storage)?;
            }
        }

        Ok(record)
    }

    fn send_telegram_message(
        &self,
        bot_token: &str,
        chat_id: &str,
        title: &str,
        message: &str,
    ) -> Result<(), TelegramError> {
        let http = telegram_http_client()?;
        http.post(&format!(
            "{}/bot{}/sendMessage",
            self.telegram_api_base_url.as_str(),
            bot_token
        ))
        .send_json(ureq::json!({
            "chat_id": chat_id,
            "text": format!("{}\n{}", title, message),
        }))
        .map_err(|error| TelegramError::storage_message(error.to_string()))?;
        Ok(())
    }

    pub fn list_notifications(
        &self,
        query: NotificationInboxQuery,
    ) -> Result<NotificationInboxResponse, TelegramError> {
        let email = normalize_email(&query.email);
        if email.is_empty() {
            return Err(TelegramError::bad_request("email is required"));
        }

        let items = self
            .db
            .list_notification_logs(&email, NOTIFICATION_INBOX_LIMIT)
            .map_err(TelegramError::storage)?
            .into_iter()
            .filter(|record| record.channel == "in_app")
            .map(|record| {
                let parsed: NotificationRecord = serde_json::from_value(record.payload)
                    .map_err(|error| TelegramError::storage_message(error.to_string()))?;
                Ok(NotificationInboxItem {
                    created_at: record.created_at,
                    delivered_at: record.delivered_at,
                    event: parsed.event,
                    in_app_delivered: parsed.in_app_delivered,
                    show_expiry_popup: parsed.show_expiry_popup,
                    telegram_delivered: parsed.telegram_delivered,
                })
            })
            .collect::<Result<Vec<NotificationInboxItem>, TelegramError>>()?;

        Ok(NotificationInboxResponse { email, items })
    }
}

#[derive(Debug)]
pub struct TelegramError {
    status: StatusCode,
    message: String,
}

impl TelegramError {
    fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.to_owned(),
        }
    }

    fn not_found(message: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.to_owned(),
        }
    }

    fn forbidden(message: &'static str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.to_owned(),
        }
    }

    fn unauthorized(message: &'static str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.to_owned(),
        }
    }

    fn storage(_error: shared_db::SharedDbError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal storage error".to_string(),
        }
    }

    fn storage_message(_message: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal storage error".to_string(),
        }
    }
}

impl IntoResponse for TelegramError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(TelegramErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

impl From<TelegramError> for AuthError {
    fn from(value: TelegramError) -> Self {
        match value.status {
            StatusCode::BAD_REQUEST => {
                AuthError::bad_request(Box::leak(value.message.into_boxed_str()))
            }
            StatusCode::NOT_FOUND => {
                AuthError::not_found(Box::leak(value.message.into_boxed_str()))
            }
            StatusCode::FORBIDDEN => {
                AuthError::forbidden(Box::leak(value.message.into_boxed_str()))
            }
            StatusCode::UNAUTHORIZED => {
                AuthError::unauthorized(Box::leak(value.message.into_boxed_str()))
            }
            StatusCode::INTERNAL_SERVER_ERROR => {
                AuthError::storage(shared_db::SharedDbError::new("telegram storage error"))
            }
            _ => AuthError::unauthorized("valid session token required"),
        }
    }
}

#[derive(Debug, Serialize)]
struct TelegramErrorResponse {
    error: String,
}

fn telegram_http_client() -> Result<&'static ureq::Agent, TelegramError> {
    static CLIENT: OnceLock<ureq::Agent> = OnceLock::new();
    Ok(CLIENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout(StdDuration::from_secs(5))
            .build()
    }))
}

fn extract_bind_code_from_bind_command(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let mut parts = trimmed.split_whitespace();
    let command = parts.next()?;
    if !command.starts_with("/bind") {
        return None;
    }
    let code = parts.next()?.trim();
    if code.starts_with("tg-bind-") {
        return Some(code.to_string());
    }
    None
}

fn extract_bind_code_from_start_command(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let mut parts = trimmed.split_whitespace();
    let command = parts.next()?;
    if !command.starts_with("/start") {
        return None;
    }
    let code = parts.next()?.trim();
    if code.starts_with("tg-bind-") {
        return Some(code.to_string());
    }
    None
}

fn is_start_command(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed == "/start" || trimmed.starts_with("/start@")
}

fn is_bind_command(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed == "/bind" || trimmed.starts_with("/bind@")
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

fn notification_kind_key(kind: &NotificationKind) -> &'static str {
    match kind {
        NotificationKind::StrategyStarted => "StrategyStarted",
        NotificationKind::StrategyPaused => "StrategyPaused",
        NotificationKind::MembershipExpiring => "MembershipExpiring",
        NotificationKind::DepositConfirmed => "DepositConfirmed",
        NotificationKind::RuntimeError => "RuntimeError",
        NotificationKind::ApiCredentialsInvalidated => "ApiCredentialsInvalidated",
        NotificationKind::GridFillExecuted => "GridFillExecuted",
        NotificationKind::FillProfitReported => "FillProfitReported",
        NotificationKind::OverallTakeProfitTriggered => "OverallTakeProfitTriggered",
        NotificationKind::OverallStopLossTriggered => "OverallStopLossTriggered",
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

fn json_value_to_payload_map(value: Value) -> std::collections::BTreeMap<String, String> {
    match value {
        Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| (key, json_scalar_to_string(value)))
            .collect(),
        _ => std::collections::BTreeMap::new(),
    }
}

fn json_scalar_to_string(value: Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value,
        Value::Array(values) => values
            .into_iter()
            .map(json_scalar_to_string)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => value.to_string(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use shared_db::SharedDb;

    #[test]
    fn start_message_returns_welcome_copy_without_binding() {
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let service = TelegramService::new(db.clone());

        let reply = service
            .process_bind_command_message(&TelegramBotMessage {
                text: Some("/start".to_string()),
                chat: TelegramBotChat { id: 778899 },
                from: Some(TelegramBotUser {
                    id: 112233,
                    username: Some("grid_user".to_string()),
                }),
            })
            .expect("process bind command")
            .expect("reply text");

        assert!(reply.contains("欢迎"));
        assert!(reply.contains("/bind"));
        let status = service
            .binding_status(TelegramBindingStatusQuery {
                email: "bind-from-start@example.com".to_string(),
            })
            .expect("binding status");
        assert!(!status.bound);
    }

    #[test]
    fn start_message_with_bind_code_returns_guidance_instead_of_binding_immediately() {
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let service = TelegramService::new(db.clone());
        let created = service
            .create_bind_code(CreateTelegramBindCodeRequest {
                email: "start-guidance@example.com".to_string(),
                ttl_seconds: Some(300),
            })
            .expect("create bind code");

        let reply = service
            .process_bind_command_message(&TelegramBotMessage {
                text: Some(format!("/start {}", created.code)),
                chat: TelegramBotChat { id: 889900 },
                from: Some(TelegramBotUser {
                    id: 778800,
                    username: Some("start_user".to_string()),
                }),
            })
            .expect("process start with bind code")
            .expect("reply text");

        assert!(reply.contains("欢迎"));
        assert!(reply.contains("/bind"));
        let status = service
            .binding_status(TelegramBindingStatusQuery {
                email: "start-guidance@example.com".to_string(),
            })
            .expect("binding status");
        assert!(!status.bound);
    }

    #[test]
    fn bind_command_binds_the_sender() {
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let service = TelegramService::new(db.clone());
        let created = service
            .create_bind_code(CreateTelegramBindCodeRequest {
                email: "bind-from-start@example.com".to_string(),
                ttl_seconds: Some(300),
            })
            .expect("create bind code");

        let reply = service
            .process_bind_command_message(&TelegramBotMessage {
                text: Some(format!("/bind {}", created.code)),
                chat: TelegramBotChat { id: 778899 },
                from: Some(TelegramBotUser {
                    id: 112233,
                    username: Some("grid_user".to_string()),
                }),
            })
            .expect("process bind command")
            .expect("reply text");

        assert!(reply.contains("绑定成功"));

        let binding = service
            .binding_status(TelegramBindingStatusQuery {
                email: "bind-from-start@example.com".to_string(),
            })
            .expect("binding status");
        assert!(binding.bound);
        assert_eq!(binding.chat_id.as_deref(), Some("778899"));
        assert_eq!(binding.telegram_user_id.as_deref(), Some("112233"));
    }

    #[test]
    fn bind_command_replaces_previous_telegram_binding_without_old_account_confirmation() {
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let service = TelegramService::new(db.clone());

        let first = service
            .create_bind_code(CreateTelegramBindCodeRequest {
                email: "rebind@example.com".to_string(),
                ttl_seconds: Some(300),
            })
            .expect("first bind code");
        let first_reply = service
            .process_bind_command_message(&TelegramBotMessage {
                text: Some(format!("/bind {}", first.code)),
                chat: TelegramBotChat { id: 1001 },
                from: Some(TelegramBotUser {
                    id: 2001,
                    username: Some("first_user".to_string()),
                }),
            })
            .expect("first bind")
            .expect("first reply");
        assert!(first_reply.contains("绑定成功"));

        let second = service
            .create_bind_code(CreateTelegramBindCodeRequest {
                email: "rebind@example.com".to_string(),
                ttl_seconds: Some(300),
            })
            .expect("second bind code");
        let second_reply = service
            .process_bind_command_message(&TelegramBotMessage {
                text: Some(format!("/bind {}", second.code)),
                chat: TelegramBotChat { id: 1002 },
                from: Some(TelegramBotUser {
                    id: 2002,
                    username: Some("second_user".to_string()),
                }),
            })
            .expect("second bind")
            .expect("second reply");
        assert!(second_reply.contains("绑定成功"));

        let binding = service
            .binding_status(TelegramBindingStatusQuery {
                email: "rebind@example.com".to_string(),
            })
            .expect("binding status");
        assert!(binding.bound);
        assert_eq!(binding.chat_id.as_deref(), Some("1002"));
        assert_eq!(binding.telegram_user_id.as_deref(), Some("2002"));
    }
}
