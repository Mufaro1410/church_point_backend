//! Expo push service client: delivers a notification to a phone through Expo (which relays to Google/Apple).

use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone)]
pub struct ExpoPush {
    http: reqwest::Client,
    url: String,
}

pub struct PushMessage {
    pub to: String,
    pub title: String,
    pub body: String,
    pub data: Value,
    pub channel: &'static str,
}

#[derive(Deserialize)]
struct Reply {
    data: Option<Vec<Ticket>>,
}
#[derive(Deserialize)]
struct Ticket {
    status: String,
    details: Option<Value>,
}

impl ExpoPush {
    pub fn new(url: &str) -> Self {
        let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().expect("http client");
        Self { http, url: url.to_string() }
    }

    /// Sends up to 100 messages. Returns the tokens Expo says no longer exist (the app was removed), so we can forget them.
    pub async fn send(&self, messages: &[PushMessage]) -> Result<Vec<String>, String> {
        let body: Vec<Value> = messages
            .iter()
            .map(|m| json!({ "to": m.to, "title": m.title, "body": m.body, "data": m.data, "sound": "default", "channelId": m.channel }))
            .collect();
        let res = self.http.post(&self.url).header("Accept", "application/json").json(&body).send().await.map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(format!("expo push returned {}", res.status()));
        }
        let reply: Reply = res.json().await.map_err(|e| e.to_string())?;
        let mut dead = Vec::new();
        for (m, t) in messages.iter().zip(reply.data.unwrap_or_default()) {
            if t.status != "ok" && t.details.as_ref().and_then(|d| d.get("error")).and_then(Value::as_str) == Some("DeviceNotRegistered") {
                dead.push(m.to.clone());
            }
        }
        Ok(dead)
    }
}
