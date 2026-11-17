use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath, ApiQuery}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::finance::{
        EntriesQuery, EntryChanges, Kind, NewCategory, NewCurrency, NewEntry, NewMethod, RangeQuery,
        SelectCurrency, SelectItem, VoidRequest,
    },
    services::finance_service as svc,
    state::SharedState,
};

type Reply<T> = Json<ApiResponse<T>>;

macro_rules! entry_handlers {
    ($list:ident, $get:ident, $create:ident, $update:ident, $void:ident, $kind:expr) => {
        pub async fn $list(
            State(state): State<SharedState>, Extension(auth): Extension<AuthUser>, ApiQuery(q): ApiQuery<EntriesQuery>,
        ) -> AppResult<Reply<Vec<Value>>> {
            Ok(Json(ApiResponse::ok(svc::list(&state, &auth, $kind, q).await?)))
        }
        pub async fn $get(
            State(state): State<SharedState>, Extension(auth): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>,
        ) -> AppResult<Reply<Value>> {
            Ok(Json(ApiResponse::ok(svc::get(&state, &auth, $kind, id).await?)))
        }
        pub async fn $create(
            State(state): State<SharedState>, Extension(auth): Extension<AuthUser>, ApiJson(body): ApiJson<NewEntry>,
        ) -> AppResult<(StatusCode, Reply<Value>)> {
            Ok((StatusCode::CREATED, Json(ApiResponse::ok(svc::create(&state, &auth, $kind, body).await?))))
        }
        pub async fn $update(
            State(state): State<SharedState>, Extension(auth): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>,
            ApiJson(body): ApiJson<EntryChanges>,
        ) -> AppResult<Reply<Value>> {
            Ok(Json(ApiResponse::ok(svc::update(&state, &auth, $kind, id, body).await?)))
        }
        pub async fn $void(
            State(state): State<SharedState>, Extension(auth): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>,
            ApiJson(body): ApiJson<VoidRequest>,
        ) -> AppResult<Reply<Value>> {
            Ok(Json(ApiResponse::ok(svc::void(&state, &auth, $kind, id, body).await?)))
        }
    };
}

entry_handlers!(list_incomes, get_income, create_income, update_income, void_income, Kind::Income);
entry_handlers!(list_expenses, get_expense, create_expense, update_expense, void_expense, Kind::Expense);

pub async fn history(
    State(state): State<SharedState>, Extension(auth): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Reply<Vec<Value>>> {
    Ok(Json(ApiResponse::ok(svc::history(&state, &auth, id).await?)))
}

pub async fn summary(
    State(state): State<SharedState>, Extension(auth): Extension<AuthUser>, ApiQuery(q): ApiQuery<RangeQuery>,
) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(svc::summary(&state, &auth, q).await?)))
}

pub async fn setup(State(state): State<SharedState>, Extension(auth): Extension<AuthUser>) -> AppResult<Reply<Value>> {
    Ok(Json(ApiResponse::ok(svc::setup(&state, &auth).await?)))
}

macro_rules! catalogue_handlers {
    ($set:ident, $set_fn:ident, $set_body:ty, $add:ident, $add_fn:ident, $add_body:ty) => {
        pub async fn $set(
            State(state): State<SharedState>, Extension(auth): Extension<AuthUser>, ApiPath(id): ApiPath<Uuid>,
            ApiJson(body): ApiJson<$set_body>,
        ) -> AppResult<Reply<Value>> {
            Ok(Json(ApiResponse::ok(svc::$set_fn(&state, &auth, id, body).await?)))
        }
        pub async fn $add(
            State(state): State<SharedState>, Extension(auth): Extension<AuthUser>, ApiJson(body): ApiJson<$add_body>,
        ) -> AppResult<(StatusCode, Reply<Value>)> {
            Ok((StatusCode::CREATED, Json(ApiResponse::ok(svc::$add_fn(&state, &auth, body).await?))))
        }
    };
}

catalogue_handlers!(set_currency, set_currency, SelectCurrency, add_currency, add_currency, NewCurrency);
catalogue_handlers!(set_category, set_category, SelectItem, add_category, add_category, NewCategory);
catalogue_handlers!(set_method, set_method, SelectItem, add_method, add_method, NewMethod);

pub async fn currency_usage(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, super::extract::ApiPath(id): super::extract::ApiPath<Uuid>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(crate::services::finance_service::usage(&s, &a, "currency_id", id).await?)))
}
pub async fn category_usage(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, super::extract::ApiPath(id): super::extract::ApiPath<Uuid>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(crate::services::finance_service::usage(&s, &a, "category_id", id).await?)))
}
pub async fn method_usage(State(s): State<SharedState>, Extension(a): Extension<AuthUser>, super::extract::ApiPath(id): super::extract::ApiPath<Uuid>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(crate::services::finance_service::usage(&s, &a, "method_id", id).await?)))
}
