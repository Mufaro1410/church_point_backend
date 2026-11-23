use axum::{Extension, Json, extract::{Query, State}, http::StatusCode};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{ApiResponse, extract::{ApiJson, ApiPath}};
use crate::{
    errors::AppResult,
    middleware::authentication::AuthUser,
    models::finance::{de_amount, de_amount_opt},
    services::pledge_service::{self as svc, NewPledge},
    state::SharedState,
};

type St = State<SharedState>;
type Me = Extension<AuthUser>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBody {
    pub member_id: Option<Uuid>,
    pub ministry_id: Option<Uuid>,
    pub pledger_name: Option<String>,
    pub category_id: Uuid,
    pub currency_id: Uuid,
    #[serde(deserialize_with = "de_amount")]
    pub amount: Decimal,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateBody {
    #[serde(default, deserialize_with = "de_amount_opt")]
    pub amount: Option<Decimal>,
    pub end_date: Option<NaiveDate>,
    pub status: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery { pub member_id: Option<Uuid> }

pub async fn list(State(s): St, Extension(a): Me, Query(q): Query<ListQuery>) -> AppResult<Json<ApiResponse<Vec<Value>>>> {
    Ok(Json(ApiResponse::ok(svc::list(&s, &a, q.member_id).await?)))
}
pub async fn get(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(svc::get(&s, &a, id).await?)))
}
pub async fn create(State(s): St, Extension(a): Me, ApiJson(b): ApiJson<CreateBody>) -> AppResult<(StatusCode, Json<ApiResponse<Value>>)> {
    let p = NewPledge { member_id: b.member_id, ministry_id: b.ministry_id, pledger_name: b.pledger_name, category_id: b.category_id, currency_id: b.currency_id, amount: b.amount, start_date: b.start_date, end_date: b.end_date, note: b.note };
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(svc::create(&s, &a, p).await?))))
}
pub async fn update(State(s): St, Extension(a): Me, ApiPath(id): ApiPath<Uuid>, ApiJson(b): ApiJson<UpdateBody>) -> AppResult<Json<ApiResponse<Value>>> {
    Ok(Json(ApiResponse::ok(svc::update(&s, &a, id, b.amount, b.end_date, b.status, b.note).await?)))
}
