use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, de::Error as _};
use serde_json::Value;
use uuid::Uuid;

use super::common::double_option;

/// The two sides of the ledger share one implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Income,
    Expense,
}

impl Kind {
    pub fn table(self) -> &'static str {
        match self {
            Self::Income => "incomes",
            Self::Expense => "expenses",
        }
    }
    /// Also the `categories.type` value and the `finance_audit.record_type`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Income => "income",
            Self::Expense => "expense",
        }
    }
}

fn parse_amount(v: Value) -> Result<Decimal, String> {
    let text = match v {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => return Err("must be a number or a numeric string".into()),
    };
    text.trim().parse::<Decimal>().map_err(|_| "is not a valid number".to_string())
}

/// Accepts 10, 10.5 or "10.50" (money is sent as a string when exactness matters).
pub fn de_amount<'de, D: Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
    parse_amount(Value::deserialize(d)?).map_err(|e| D::Error::custom(format!("amount {e}")))
}

pub fn de_amount_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    match Option::<Value>::deserialize(d)? {
        None | Some(Value::Null) => Ok(None),
        Some(v) => parse_amount(v).map(Some).map_err(|e| D::Error::custom(format!("value {e}"))),
    }
}

/// One shape for both sides. Income-only fields: member_id, event_id. Expense-only: supplier, ministry_id.
#[derive(Debug, Deserialize)]
pub struct NewEntry {
    #[serde(deserialize_with = "de_amount")]
    pub amount: Decimal,
    pub currency_id: Uuid,
    pub category_id: Uuid,
    pub method_id: Uuid,
    pub description: Option<String>,
    pub date: Option<NaiveDate>,
    pub member_id: Option<Uuid>,
    pub event_id: Option<Uuid>,
    pub supplier: Option<String>,
    pub ministry_id: Option<Uuid>,
}

/// Every change needs a reason; it is written to the audit trail.
#[derive(Debug, Deserialize)]
pub struct EntryChanges {
    pub reason: String,
    #[serde(default, deserialize_with = "de_amount_opt")]
    pub amount: Option<Decimal>,
    pub currency_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub method_id: Option<Uuid>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    pub date: Option<NaiveDate>,
    pub supplier: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub ministry_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "double_option")]
    pub member_id: Option<Option<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct VoidRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct EntriesQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    pub category_id: Option<Uuid>,
    pub member_id: Option<Uuid>,
    pub event_id: Option<Uuid>,
    #[serde(default)]
    pub include_voided: bool,
    /// Default 200, maximum 1000.
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RangeQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
}

// ── catalogue set-up (currencies, categories, payment methods) ──

#[derive(Debug, Deserialize)]
pub struct SelectCurrency {
    pub selected: bool,
    /// Units of this currency per 1 USD. USD is always 1.
    #[serde(default, deserialize_with = "de_amount_opt")]
    pub rate: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct SelectItem {
    pub selected: bool,
}

#[derive(Debug, Deserialize)]
pub struct NewCurrency {
    pub name: String,
    pub symbol: Option<String>,
    #[serde(default, deserialize_with = "de_amount_opt")]
    pub rate: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct NewCategory {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewMethod {
    pub name: String,
    pub description: Option<String>,
}

// ── validated values handed to the repository ──

pub struct ValidEntry {
    pub amount: Decimal,
    pub currency_id: Uuid,
    pub category_id: Uuid,
    pub method_id: Uuid,
    pub description: Option<String>,
    pub date: NaiveDate,
    pub rate: Decimal,
    pub member_id: Option<Uuid>,
    pub event_id: Option<Uuid>,
    pub supplier: Option<String>,
    pub ministry_id: Option<Uuid>,
}

#[derive(Default)]
pub struct ValidChanges {
    pub amount: Option<Decimal>,
    /// (new currency, the FOC's current rate for it): the rate is snapshotted again when the currency changes
    pub currency: Option<(Uuid, Decimal)>,
    pub category_id: Option<Uuid>,
    pub method_id: Option<Uuid>,
    pub description: Option<Option<String>>,
    pub date: Option<NaiveDate>,
    pub supplier: Option<String>,
    pub ministry_id: Option<Option<Uuid>>,
    pub member_id: Option<Option<Uuid>>,
}
