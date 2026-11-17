//! Paynow (Zimbabwe) client. Every request and reply is signed: SHA-512 over the values in order
//! followed by the church's integration key, upper-case hex.

use sha2::{Digest, Sha512};

use crate::errors::{AppError, AppResult};

#[derive(Clone)]
pub struct PaynowClient {
    http: reqwest::Client,
    base: String,
}

pub struct Initiated {
    pub paynow_reference: Option<String>,
    pub poll_url: String,
    pub instructions: Option<String>,
}

pub struct Polled {
    pub status: String,
    pub amount: Option<String>,
    pub paynow_reference: Option<String>,
}

pub struct MobileOrder<'a> {
    pub integration_id: &'a str,
    pub key: &'a str,
    pub reference: &'a str,
    pub amount: &'a str,
    pub info: &'a str,
    pub return_url: &'a str,
    pub result_url: &'a str,
    pub auth_email: &'a str,
    pub phone: &'a str,
    pub method: &'a str,
}

pub fn sign(values: &[&str], key: &str) -> String {
    let mut h = Sha512::new();
    for v in values {
        h.update(v.as_bytes());
    }
    h.update(key.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02X}")).collect()
}

pub fn parse(body: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(body.as_bytes()).map(|(k, v)| (k.to_lowercase(), v.into_owned())).collect()
}

fn field<'a>(f: &'a [(String, String)], name: &str) -> Option<&'a str> {
    f.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

/// A Paynow message is genuine only if its hash matches the values it carries (in order) plus our key.
pub fn verify(f: &[(String, String)], key: &str) -> bool {
    let Some(given) = field(f, "hash") else { return false };
    let values: Vec<&str> = f.iter().filter(|(k, _)| k != "hash").map(|(_, v)| v.as_str()).collect();
    sign(&values, key).eq_ignore_ascii_case(given)
}

impl PaynowClient {
    pub fn new(base: &str) -> Self {
        let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().expect("http client");
        Self { http, base: base.trim_end_matches('/').to_string() }
    }

    async fn post(&self, url: &str, form: &[(&str, &str)]) -> AppResult<Vec<(String, String)>> {
        let body = url::form_urlencoded::Serializer::new(String::new()).extend_pairs(form).finish();
        let res = self.http
            .post(url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send().await
            .map_err(|e| AppError::Upstream(format!("Paynow unreachable: {e}")))?;
        let text = res.text().await.map_err(|e| AppError::Upstream(format!("Paynow reply unreadable: {e}")))?;
        Ok(parse(&text))
    }

    /// Sends a mobile-money payment prompt (EcoCash / OneMoney) to the person's phone.
    pub async fn initiate_mobile(&self, o: &MobileOrder<'_>) -> AppResult<Initiated> {
        let values = [o.integration_id, o.reference, o.amount, o.info, o.return_url, o.result_url, o.auth_email, o.phone, o.method, "Message"];
        let hash = sign(&values, o.key);
        let form = [
            ("id", o.integration_id), ("reference", o.reference), ("amount", o.amount), ("additionalinfo", o.info),
            ("returnurl", o.return_url), ("resulturl", o.result_url), ("authemail", o.auth_email),
            ("phone", o.phone), ("method", o.method), ("status", "Message"), ("hash", hash.as_str()),
        ];
        let r = self.post(&format!("{}/interface/remotetransaction", self.base), &form).await?;
        match field(&r, "status").map(str::to_lowercase).as_deref() {
            Some("ok") => {
                if !verify(&r, o.key) {
                    return Err(AppError::Upstream("Paynow reply failed signature check (is the integration key right?)".into()));
                }
                let poll_url = field(&r, "pollurl").ok_or_else(|| AppError::Upstream("Paynow gave no poll url".into()))?.to_string();
                Ok(Initiated {
                    paynow_reference: field(&r, "paynowreference").map(str::to_string),
                    poll_url,
                    instructions: field(&r, "instructions").map(str::to_string),
                })
            }
            _ => Err(AppError::Validation(format!("Paynow refused the payment: {}", field(&r, "error").unwrap_or("unknown reason")))),
        }
    }

    /// Asks Paynow what really happened. This, not an incoming message, decides if money arrived.
    pub async fn poll(&self, poll_url: &str, key: &str) -> AppResult<Polled> {
        if !poll_url.starts_with(&self.base) {
            return Err(AppError::Upstream("Unexpected Paynow poll url".into()));
        }
        let r = self.post(poll_url, &[]).await?;
        if !verify(&r, key) {
            return Err(AppError::Upstream("Paynow reply failed signature check".into()));
        }
        Ok(Polled {
            status: field(&r, "status").unwrap_or("").to_string(),
            amount: field(&r, "amount").map(str::to_string),
            paynow_reference: field(&r, "paynowreference").map(str::to_string),
        })
    }
}
