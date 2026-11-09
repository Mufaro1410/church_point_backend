use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}
fn default_page() -> i64 { 1 }
fn default_per_page() -> i64 { 20 }

impl PaginationParams {
    pub fn limit(&self) -> i64 { self.per_page.clamp(1, 100) }
    pub fn offset(&self) -> i64 { (self.page.max(1) - 1) * self.limit() }
}

#[derive(Debug, Serialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

impl<T> Paginated<T> {
    pub fn new(items: Vec<T>, total: i64, p: &PaginationParams) -> Self {
        Self { items, total, page: p.page.max(1), per_page: p.limit() }
    }
}
