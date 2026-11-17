use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use super::finance::de_amount;

/// Saved by a church admin. `integration_key` is only needed the first time or when changing it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentSettingsRequest {
    pub integration_id: String,
    pub integration_key: Option<String>,
    pub merchant_email: String,
    #[serde(default = "yes")]
    pub test_mode: bool,
    pub category_id: Uuid,
    pub method_id: Uuid,
    #[serde(default)]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewPayment {
    #[serde(deserialize_with = "de_amount")]
    pub amount: Decimal,
    pub currency_id: Uuid,
    /// "ecocash" or "onemoney"
    pub method: String,
    pub phone: String,
    pub purpose: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentsQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}
