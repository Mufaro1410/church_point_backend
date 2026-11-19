//! Push notifications: who gets what, and the event-reminder clock.
//! Sending never blocks or fails the request that caused it — problems are only logged.

use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{clients::expo_push::PushMessage, state::SharedState};

#[derive(Clone, Copy)]
pub enum Kind {
    Announcements,
    Events,
}

impl Kind {
    fn column(self) -> &'static str {
        match self {
            Kind::Announcements => "announcements",
            Kind::Events => "events",
        }
    }
    fn channel(self) -> &'static str {
        self.column()
    }
}

/// Approved members of a church (FOC), a province or a whole church.
async fn audience(db: &PgPool, scope: &str, foc: Option<Uuid>, province: Option<Uuid>, church: Option<Uuid>) -> Vec<Uuid> {
    let (sql, id) = match (scope, foc, province, church) {
        ("foc", Some(id), _, _) => ("SELECT m.id FROM member_profiles m WHERE m.foc_id = $1", id),
        ("province", _, Some(id), _) => ("SELECT m.id FROM member_profiles m JOIN foc_profile f ON f.id = m.foc_id WHERE f.province_id = $1", id),
        ("church", _, _, Some(id)) => ("SELECT m.id FROM member_profiles m JOIN foc_profile f ON f.id = m.foc_id WHERE f.church_id = $1", id),
        _ => return Vec::new(),
    };
    let sql = format!("{sql} AND m.deleted_at IS NULL AND m.status::text IN ('approved','transfer_requested')");
    sqlx::query_scalar(&sql).bind(id).fetch_all(db).await.unwrap_or_default()
}

/// Sends to every listed member who has a token and has this kind of notification switched on.
pub async fn deliver(state: &SharedState, members: &[Uuid], kind: Kind, title: &str, body: &str, data: Value) {
    if members.is_empty() {
        return;
    }
    let sql = format!(
        "SELECT t.token FROM member_push_tokens t LEFT JOIN member_notification_prefs p ON p.member_id = t.member_id
         WHERE t.member_id = ANY($1) AND COALESCE(p.enabled, true) AND COALESCE(p.{}, true)",
        kind.column()
    );
    let tokens: Vec<String> = match sqlx::query_scalar(&sql).bind(members).fetch_all(&state.db).await {
        Ok(t) => t,
        Err(e) => return tracing::warn!("push: could not load tokens: {e}"),
    };
    for chunk in tokens.chunks(100) {
        let msgs: Vec<PushMessage> = chunk
            .iter()
            .map(|t| PushMessage { to: t.clone(), title: title.to_string(), body: body.to_string(), data: data.clone(), channel: kind.channel() })
            .collect();
        match state.push.send(&msgs).await {
            Ok(dead) if !dead.is_empty() => {
                let _ = sqlx::query("DELETE FROM member_push_tokens WHERE token = ANY($1)").bind(&dead).execute(&state.db).await;
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("push: send failed: {e}"),
        }
    }
}

fn text<'a>(row: &'a Value, k: &str) -> &'a str {
    row.get(k).and_then(Value::as_str).unwrap_or("")
}
fn uuid_of(row: &Value, k: &str) -> Option<Uuid> {
    row.get(k).and_then(Value::as_str).and_then(|s| s.parse().ok())
}
fn short(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n).collect::<String>()) }
}

/// Fire-and-forget: tell the audience about a newly published announcement.
pub fn announcement_created(state: &SharedState, row: &Value) {
    if row.get("is_active").and_then(Value::as_bool) == Some(false) {
        return;
    }
    let (state, row) = (state.clone(), row.clone());
    tokio::spawn(async move {
        let members = audience(&state.db, text(&row, "scope"), uuid_of(&row, "foc_id"), uuid_of(&row, "province_id"), uuid_of(&row, "church_id")).await;
        let data = json!({ "screen": "announcements", "id": row.get("id") });
        deliver(&state, &members, Kind::Announcements, &format!("📢 {}", text(&row, "title")), &short(text(&row, "content"), 110), data).await;
    });
}

/// Fire-and-forget: tell the audience about a newly created event.
pub fn event_created(state: &SharedState, row: &Value) {
    let (state, row) = (state.clone(), row.clone());
    tokio::spawn(async move {
        let members = audience(&state.db, text(&row, "scope"), uuid_of(&row, "foc_id"), uuid_of(&row, "province_id"), uuid_of(&row, "church_id")).await;
        let when = format!("{} · {}", text(&row, "date"), short(text(&row, "start_time"), 5));
        let body = format!("{when} · {}", text(&row, "location"));
        let data = json!({ "screen": "events", "id": row.get("id") });
        deliver(&state, &members, Kind::Events, &format!("📅 New event: {}", text(&row, "title")), &body, data).await;
    });
}

/// One pass of the reminder clock (also used by tests). Sends at most one "evening before" and one "morning of"
/// reminder per event, only to members who said going/maybe. Returns how many reminders were sent.
pub async fn run_reminders_once(state: &SharedState) -> usize {
    run_reminders_at(state, &state.settings.app_timezone).await
}

pub async fn run_reminders_at(state: &SharedState, tz: &str) -> usize {
    let due: Vec<(Uuid, String, String, String, String, Vec<Uuid>)> = match sqlx::query_as(
        "WITH clock AS (SELECT (now() AT TIME ZONE $1) AS t),
              due AS (
                SELECT e.id, e.title, left(e.start_time::text, 5) AS at, COALESCE(e.location, '') AS location,
                       CASE WHEN e.date = (SELECT t::date FROM clock) + 1 THEN 'eve' ELSE 'morning' END AS kind,
                       e.rsvp
                FROM events e, clock
                WHERE (e.date = clock.t::date + 1 AND extract(hour FROM clock.t) >= 17)
                   OR (e.date = clock.t::date     AND extract(hour FROM clock.t) >= 7 AND extract(hour FROM clock.t) < 20)
              ),
              claimed AS (
                INSERT INTO event_reminders_sent (event_id, kind) SELECT id, kind FROM due ON CONFLICT DO NOTHING RETURNING event_id, kind
              )
         SELECT d.id, d.title, d.at, d.location, d.kind,
                COALESCE((SELECT array_agg(k::uuid) FROM jsonb_each_text(COALESCE(d.rsvp, '{}'::jsonb)) AS r(k, v) WHERE v IN ('going', 'maybe')), '{}') AS members
         FROM due d JOIN claimed c ON c.event_id = d.id AND c.kind = d.kind",
    )
    .bind(tz)
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("reminders: query failed: {e}");
            return 0;
        }
    };
    let n = due.len();
    for (id, title, at, location, kind, members) in due {
        let (head, when) = if kind == "eve" { ("Tomorrow", "tomorrow") } else { ("Today", "today") };
        let body = format!("{title} is {when} at {at}{}", if location.is_empty() { String::new() } else { format!(" · {location}") });
        deliver(state, &members, Kind::Events, &format!("⏰ {head}: {title}"), &body, json!({ "screen": "events", "id": id })).await;
    }
    n
}

/// Background clock: checks every 10 minutes.
pub fn spawn_reminder_clock(state: SharedState) {
    if !state.settings.reminders_enabled {
        return;
    }
    tokio::spawn(async move {
        loop {
            run_reminders_once(&state).await;
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
        }
    });
}
