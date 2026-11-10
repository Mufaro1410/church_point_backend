use serde::Deserialize;

/// Who a church-wide / province-wide / FOC-only record belongs to.
/// Shared by events and announcements (both have the same scope columns + constraint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Church,
    Province,
    Foc,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Church => "church",
            Self::Province => "province",
            Self::Foc => "foc",
        }
    }
}
