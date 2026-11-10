use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::{common::double_option, scope::Scope};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    /// Inclusive date range (YYYY-MM-DD). Defaults to the current year.
    pub start: Option<chrono::NaiveDate>,
    pub end: Option<chrono::NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct MarkAttendanceRequest {
    pub member_id: Uuid,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MemberName {
    pub id: Uuid,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

/// Same shape the React `AttendanceCategory` type uses.
#[derive(Debug, Serialize)]
pub struct RsvpCategory {
    pub count: usize,
    pub profiles: Vec<MemberName>,
}

#[derive(Debug, Serialize)]
pub struct RsvpStats {
    pub going: RsvpCategory,
    pub maybe: RsvpCategory,
    #[serde(rename = "notGoing")]
    pub not_going: RsvpCategory,
    pub total: usize,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct Attendee {
    pub profile: MemberName,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct AttendanceSummary {
    pub attendees: Vec<Attendee>,
    pub total: usize,
    pub expected_from_rsvp: usize,
    pub attendance_rate: f64,
}

// ── Strictly typed write requests (matches the real `events` table) ──

/// `manual_stats` jsonb column. Keys match what the React app already stores.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManualStats {
    #[serde(default)]
    pub arena: i32,
    #[serde(default)]
    pub males: i32,
    #[serde(default)]
    pub females: i32,
    #[serde(default)]
    pub children: i32,
    #[serde(default)]
    pub visitors: i32,
    #[serde(default, rename = "alterCall")]
    pub alter_call: i32,
    #[serde(default)]
    pub salvation: i32,
    #[serde(default)]
    pub reassurance: i32,
}

impl ManualStats {
    pub fn is_valid(&self) -> bool {
        [
            self.arena, self.males, self.females, self.children,
            self.visitors, self.alter_call, self.salvation, self.reassurance,
        ]
        .iter()
        .all(|n| *n >= 0)
    }
}

/// Partial version used by PATCH: only the counters that are sent are changed, so two
/// people updating different counters can't wipe each other's numbers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManualStatsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arena: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub males: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub females: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visitors: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "alterCall")]
    pub alter_call: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salvation: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reassurance: Option<i32>,
}

impl ManualStatsPatch {
    pub fn is_valid(&self) -> bool {
        [
            self.arena, self.males, self.females, self.children,
            self.visitors, self.alter_call, self.salvation, self.reassurance,
        ]
        .iter()
        .flatten()
        .all(|n| *n >= 0)
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub title: String,
    pub date: NaiveDate,
    /// e.g. "09:00" or "09:00:00+02" (Postgres `timetz` input)
    pub start_time: String,
    pub end_time: String,
    pub location: String,
    pub scope: Scope,
    pub description: Option<String>,
    #[serde(rename = "isRecurring")]
    pub is_recurring: Option<String>,
    pub sermon_id: Option<Uuid>,
    pub manual_stats: Option<ManualStats>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEventRequest {
    pub title: Option<String>,
    pub date: Option<NaiveDate>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub location: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, rename = "isRecurring", deserialize_with = "double_option")]
    pub is_recurring: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub sermon_id: Option<Option<Uuid>>,
    pub manual_stats: Option<ManualStatsPatch>,
}

/// A fully validated event ready to insert. Built by the service, used by the repository.
pub struct NewEvent {
    pub id: Uuid,
    pub title: String,
    pub date: NaiveDate,
    pub start_time: String,
    pub end_time: String,
    pub location: String,
    pub description: Option<String>,
    pub scope: &'static str,
    pub church_id: Uuid,
    pub province_id: Option<Uuid>,
    pub foc_id: Option<Uuid>,
    pub created_by_foc_id: Uuid,
    pub is_recurring: Option<String>,
    pub sermon_id: Option<Uuid>,
    pub manual_stats: Value,
    pub qr_code: String,
    pub qr_generated_at: DateTime<Utc>,
}
