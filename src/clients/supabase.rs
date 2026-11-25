//! The only place that talks to Supabase over HTTP (Auth + Storage). Axum is the orchestrator:
//! web and mobile never hold Supabase keys for these operations.

use reqwest::{Method, RequestBuilder};
use serde_json::{Value, json};

use crate::{
    config::Settings,
    errors::{AppError, AppResult, AuthError},
};

pub const BUCKET: &str = "churchpoint";

#[derive(Clone)]
pub struct SupabaseClient {
    http: reqwest::Client,
    url: Option<String>,
    anon: Option<String>,
    service: Option<String>,
}

fn message(body: &Value) -> String {
    ["msg", "message", "error_description", "error"]
        .iter()
        .find_map(|k| body.get(*k).and_then(Value::as_str))
        .unwrap_or("The request was refused.")
        .to_string()
}

impl SupabaseClient {
    pub fn new(s: &Settings) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("http client");
        Self {
            http,
            url: s.supabase_url.as_ref().map(|u| u.trim_end_matches('/').to_string()),
            anon: s.supabase_anon_key.clone(),
            service: s.supabase_service_key.clone(),
        }
    }

    fn base(&self) -> AppResult<&str> {
        self.url.as_deref().ok_or(AppError::NotConfigured("SUPABASE_URL"))
    }
    fn anon(&self) -> AppResult<&str> {
        self.anon.as_deref().ok_or(AppError::NotConfigured("SUPABASE_ANON_KEY"))
    }
    fn service(&self) -> AppResult<&str> {
        self.service.as_deref().ok_or(AppError::NotConfigured("SUPABASE_SERVICE_ROLE_KEY"))
    }

    pub fn public_url(&self, path: &str) -> AppResult<String> {
        Ok(format!("{}/storage/v1/object/public/{BUCKET}/{path}", self.base()?))
    }

    fn req(&self, m: Method, path: &str) -> AppResult<RequestBuilder> {
        Ok(self.http.request(m, format!("{}{path}", self.base()?)))
    }

    fn as_anon(&self, r: RequestBuilder) -> AppResult<RequestBuilder> {
        Ok(r.header("apikey", self.anon()?))
    }

    fn as_service(&self, r: RequestBuilder) -> AppResult<RequestBuilder> {
        let k = self.service()?;
        Ok(r.header("apikey", k).bearer_auth(k))
    }

    async fn send(&self, r: RequestBuilder) -> AppResult<(u16, Value)> {
        let res = r.send().await.map_err(|e| AppError::Upstream(format!("Supabase unreachable: {e}")))?;
        let status = res.status().as_u16();
        let text = res.text().await.unwrap_or_default();
        Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
    }

    fn upstream(status: u16, body: &Value) -> AppError {
        AppError::Upstream(format!("Supabase answered {status}: {}", message(body)))
    }

    // ── Auth ──

    pub async fn login(&self, email: &str, password: &str) -> AppResult<Value> {
        let r = self.as_anon(self.req(Method::POST, "/auth/v1/token?grant_type=password")?)?
            .json(&json!({ "email": email, "password": password }));
        match self.send(r).await? {
            (200, b) => Ok(b),
            (400..=403, _) => Err(AuthError::InvalidCredentials.into()),
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    pub async fn refresh(&self, refresh_token: &str) -> AppResult<Value> {
        let r = self.as_anon(self.req(Method::POST, "/auth/v1/token?grant_type=refresh_token")?)?
            .json(&json!({ "refresh_token": refresh_token }));
        match self.send(r).await? {
            (200, b) => Ok(b),
            (400..=403, _) => Err(AuthError::InvalidToken.into()),
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    pub async fn logout(&self, access_token: &str) -> AppResult<()> {
        let r = self.as_anon(self.req(Method::POST, "/auth/v1/logout")?)?.bearer_auth(access_token);
        match self.send(r).await? {
            (200..=299, _) | (401, _) => Ok(()),
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    /// Creates the login. Whether an email confirmation is needed is decided by the Supabase project settings.
    pub async fn sign_up(&self, email: &str, password: &str, meta: Value, redirect_to: Option<&str>) -> AppResult<Value> {
        let mut r = self.as_anon(self.req(Method::POST, "/auth/v1/signup")?)?
            .json(&json!({ "email": email, "password": password, "data": meta }));
        if let Some(url) = redirect_to {
            r = r.query(&[("redirect_to", url)]);
        }
        match self.send(r).await? {
            (200..=299, b) => Ok(b),
            (400..=429, b) => Err(AppError::Validation(message(&b))),
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    pub async fn recover(&self, email: &str, redirect_to: Option<&str>) -> AppResult<()> {
        let mut r = self.as_anon(self.req(Method::POST, "/auth/v1/recover")?)?.json(&json!({ "email": email }));
        if let Some(url) = redirect_to {
            r = r.query(&[("redirect_to", url)]);
        }
        match self.send(r).await? {
            (200..=299, _) | (400..=429, _) => Ok(()), // never reveal whether the email exists
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    /// Changes the signed-in person's password (their own token proves who they are).
    pub async fn update_password(&self, access_token: &str, new_password: &str) -> AppResult<()> {
        let r = self.as_anon(self.req(Method::PUT, "/auth/v1/user")?)?
            .bearer_auth(access_token)
            .json(&json!({ "password": new_password }));
        match self.send(r).await? {
            (200..=299, _) => Ok(()),
            (400..=429, b) => Err(AppError::Validation(message(&b))),
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    pub async fn delete_user(&self, id: uuid::Uuid) -> AppResult<()> {
        let r = self.as_service(self.req(Method::DELETE, &format!("/auth/v1/admin/users/{id}"))?)?;
        match self.send(r).await? {
            (200..=299, _) => Ok(()),
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    // ── Storage ──

    /// One-time URL the browser can PUT the file to; Axum decides who may upload what.
    pub async fn sign_upload(&self, path: &str) -> AppResult<String> {
        let r = self.as_service(self.req(Method::POST, &format!("/storage/v1/object/upload/sign/{BUCKET}/{path}"))?)?
            .header("x-upsert", "true");
        match self.send(r).await? {
            (200..=299, b) => {
                let u = b.get("url").and_then(Value::as_str).ok_or_else(|| AppError::Upstream("Storage gave no upload url.".into()))?;
                if u.starts_with("http") { Ok(u.to_string()) } else { Ok(format!("{}/storage/v1{u}", self.base()?)) }
            }
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    /// Axum puts the (already compressed) file into storage itself, using the server key.
    pub async fn upload_object(&self, path: &str, body: Vec<u8>, content_type: &str) -> AppResult<()> {
        let r = self.as_service(self.req(Method::POST, &format!("/storage/v1/object/{BUCKET}/{path}"))?)?
            .header("x-upsert", "true")
            .header("content-type", content_type)
            .timeout(std::time::Duration::from_secs(300)) // uploads over a slow link need longer than the 15s default
            .body(body);
        match self.send(r).await? {
            (200..=299, _) => Ok(()),
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    pub async fn object_exists(&self, path: &str) -> AppResult<bool> {
        let r = self.http.head(self.public_url(path)?);
        Ok(matches!(r.send().await.map(|r| r.status().as_u16()), Ok(200..=299)))
    }

    pub async fn remove_object(&self, path: &str) -> AppResult<()> {
        let r = self.as_service(self.req(Method::DELETE, &format!("/storage/v1/object/{BUCKET}/{path}"))?)?;
        match self.send(r).await? {
            (200..=299, _) | (404, _) => Ok(()),
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }

    pub async fn list_logos(&self) -> AppResult<Vec<String>> {
        let r = self.as_service(self.req(Method::POST, &format!("/storage/v1/object/list/{BUCKET}"))?)?
            .json(&json!({ "prefix": "", "limit": 100, "offset": 0, "sortBy": { "column": "name", "order": "asc" } }));
        match self.send(r).await? {
            (200..=299, Value::Array(items)) => items
                .iter()
                .filter_map(|i| i.get("name").and_then(Value::as_str))
                .filter(|n| {
                    let n = n.to_ascii_lowercase();
                    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".svg"].iter().any(|e| n.ends_with(e))
                })
                .map(|n| self.public_url(n))
                .collect(),
            (s, b) => Err(Self::upstream(s, &b)),
        }
    }
}
