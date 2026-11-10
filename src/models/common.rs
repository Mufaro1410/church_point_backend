use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Deserializer};

/// Distinguishes "field absent" (leave alone) from "field is null" (clear it).
/// Use with `#[serde(default, deserialize_with = "double_option")]` on `Option<Option<T>>`.
pub fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// `start`/`end` query params with the current year as the default window.
pub fn resolve_range(start: Option<NaiveDate>, end: Option<NaiveDate>) -> (NaiveDate, NaiveDate) {
    let year = Utc::now().year();
    (
        start.unwrap_or_else(|| NaiveDate::from_ymd_opt(year, 1, 1).unwrap()),
        end.unwrap_or_else(|| NaiveDate::from_ymd_opt(year, 12, 31).unwrap()),
    )
}

#[derive(Debug, Deserialize)]
pub struct DateRangeQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
}
