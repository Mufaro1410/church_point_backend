use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum FinanceError {
    #[error("The requested record was not found.")]
    EntryNotFound,
    #[error("This record has been voided and can no longer be changed.")]
    EntryVoided,
    #[error("This currency is not enabled for your FOC.")]
    CurrencyNotEnabled,
    #[error("This category is not enabled for your FOC.")]
    CategoryNotEnabled,
    #[error("This payment method is not enabled for your FOC.")]
    MethodNotEnabled,
    #[error("No exchange rate is set for this currency. Set the rate (units per 1 USD) first.")]
    MissingRate,
    #[error("The category type does not match (income categories for income, expense categories for expenses).")]
    WrongCategoryType,
    #[error("The requested currency, category or payment method was not found.")]
    CatalogueItemNotFound,
    #[error("A category with this name already exists with a different type.")]
    CategoryTypeConflict,
}

impl FinanceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::EntryNotFound => "ENTRY_NOT_FOUND",
            Self::EntryVoided => "ENTRY_VOIDED",
            Self::CurrencyNotEnabled => "CURRENCY_NOT_ENABLED",
            Self::CategoryNotEnabled => "CATEGORY_NOT_ENABLED",
            Self::MethodNotEnabled => "METHOD_NOT_ENABLED",
            Self::MissingRate => "MISSING_RATE",
            Self::WrongCategoryType => "WRONG_CATEGORY_TYPE",
            Self::CatalogueItemNotFound => "CATALOGUE_ITEM_NOT_FOUND",
            Self::CategoryTypeConflict => "CATEGORY_TYPE_CONFLICT",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::EntryNotFound | Self::CatalogueItemNotFound => StatusCode::NOT_FOUND,
            Self::EntryVoided | Self::CategoryTypeConflict => StatusCode::CONFLICT,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}
