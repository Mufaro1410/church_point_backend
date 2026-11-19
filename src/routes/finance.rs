use axum::{
    Router,
    middleware::from_fn,
    routing::{get, post, put},
};

use crate::{handlers::finance as h, middleware::authorization::require_perm, state::SharedState};

/// Money records are never deleted: they are voided with a reason (there is deliberately no DELETE route).
pub fn routes() -> Router<SharedState> {
    let giving = Router::new()
        .route("/incomes", get(h::list_incomes).post(h::create_income))
        .route("/incomes/{id}", get(h::get_income).patch(h::update_income))
        .route("/incomes/{id}/void", post(h::void_income))
        .route_layer(from_fn(|req, next| require_perm(&["giving"], req, next)));
    let expenses = Router::new()
        .route("/expenses", get(h::list_expenses).post(h::create_expense))
        .route("/expenses/{id}", get(h::get_expense).patch(h::update_expense))
        .route("/expenses/{id}/void", post(h::void_expense))
        .route_layer(from_fn(|req, next| require_perm(&["expenses"], req, next)));
    // What the finance screens read to work (currencies, categories, methods, totals, history).
    let lookups = Router::new()
        .route("/finance/setup", get(h::setup))
        .route("/finance/summary", get(h::summary))
        .route("/finance/history/{id}", get(h::history))
        .route("/finance/currencies/{id}/usage", get(h::currency_usage))
        .route("/finance/categories/{id}/usage", get(h::category_usage))
        .route("/finance/methods/{id}/usage", get(h::method_usage))
        .route_layer(from_fn(|req, next| require_perm(&["giving", "expenses", "finance_settings"], req, next)));
    let setup = Router::new()
        .route("/finance/currencies", post(h::add_currency))
        .route("/finance/currencies/{id}", put(h::set_currency))
        .route("/finance/categories", post(h::add_category))
        .route("/finance/categories/{id}", put(h::set_category))
        .route("/finance/methods", post(h::add_method))
        .route("/finance/methods/{id}", put(h::set_method))
        .route_layer(from_fn(|req, next| require_perm(&["finance_settings"], req, next)));
    giving.merge(expenses).merge(lookups).merge(setup)
}
