//! Strict, whitelisted writes for wide tables (member_profiles, foc_profile, ...).
//! Only fields named in a spec are accepted; unknown fields are a 422, never silently ignored.

use chrono::NaiveDate;
use serde_json::{Map, Value};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Clone, Copy)]
pub enum Kind {
    TextReq,
    Text,
    Date,
    Uuid,
    TextArr,
    UuidArr,
    Json,
    Time,
    Bool,
    Int,
}

#[derive(Debug)]
pub enum Val {
    Text(Option<String>),
    Date(Option<String>),
    Uuid(Option<Uuid>),
    TextArr(Option<Vec<String>>),
    UuidArr(Option<Vec<Uuid>>),
    Json(Option<Value>),
    Time(Option<String>),
    Bool(Option<bool>),
    Int(Option<i32>),
}

fn bad(field: &str, what: &str) -> AppError {
    AppError::Validation(format!("{field} {what}"))
}

fn text(field: &str, v: &Value) -> Result<String, AppError> {
    v.as_str().map(|s| s.trim().to_string()).ok_or_else(|| bad(field, "must be text."))
}

pub fn parse(map: Map<String, Value>, spec: &[(&'static str, Kind)]) -> Result<Vec<(&'static str, Val)>, AppError> {
    let mut out = Vec::new();
    for (key, v) in map {
        let (name, kind) = spec
            .iter()
            .find(|(n, _)| *n == key)
            .copied()
            .ok_or_else(|| AppError::Validation(format!("Unknown or read-only field: {key}")))?;
        let null = v.is_null();
        let val = match kind {
            Kind::TextReq => {
                let s = text(name, &v)?;
                if s.is_empty() {
                    return Err(bad(name, "is required."));
                }
                Val::Text(Some(s))
            }
            Kind::Text => Val::Text(if null { None } else { Some(text(name, &v)?).filter(|s| !s.is_empty()) }),
            Kind::Date => Val::Date(if null {
                None
            } else {
                let s = text(name, &v)?;
                let s = s.get(..10).unwrap_or(&s).to_string();
                NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|_| bad(name, "must look like 2026-11-30."))?;
                Some(s)
            }),
            Kind::Uuid => Val::Uuid(if null {
                None
            } else {
                Some(text(name, &v)?.parse().map_err(|_| bad(name, "must be an id."))?)
            }),
            Kind::TextArr => Val::TextArr(if null {
                None
            } else {
                let a = v.as_array().ok_or_else(|| bad(name, "must be a list."))?;
                Some(a.iter().map(|x| text(name, x)).collect::<Result<_, _>>()?)
            }),
            Kind::UuidArr => Val::UuidArr(if null {
                None
            } else {
                let a = v.as_array().ok_or_else(|| bad(name, "must be a list."))?;
                let mut ids: Vec<Uuid> = a
                    .iter()
                    .map(|x| text(name, x)?.parse().map_err(|_| bad(name, "must be a list of ids.")))
                    .collect::<Result<_, _>>()?;
                ids.sort();
                ids.dedup();
                Some(ids)
            }),
            Kind::Json => Val::Json(if null { None } else { Some(v) }),
            Kind::Time => Val::Time(if null {
                None
            } else {
                let s = text(name, &v)?;
                let t = chrono::NaiveTime::parse_from_str(&s, "%H:%M")
                    .or_else(|_| chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S"))
                    .map_err(|_| bad(name, "must look like 14:30."))?;
                Some(t.format("%H:%M").to_string())
            }),
            Kind::Bool => Val::Bool(if null { None } else { Some(v.as_bool().ok_or_else(|| bad(name, "must be true or false."))?) }),
            Kind::Int => Val::Int(if null {
                None
            } else {
                Some(v.as_i64().and_then(|n| i32::try_from(n).ok()).ok_or_else(|| bad(name, "must be a whole number."))?)
            }),
        };
        out.push((name, val));
    }
    Ok(out)
}

pub fn get_uuid_arr<'a>(vals: &'a [(&'static str, Val)], name: &str) -> Option<&'a [Uuid]> {
    vals.iter().find_map(|(n, v)| match v {
        Val::UuidArr(Some(a)) if *n == name => Some(a.as_slice()),
        _ => None,
    })
}

pub fn get_uuid(vals: &[(&'static str, Val)], name: &str) -> Option<Uuid> {
    vals.iter().find_map(|(n, v)| match v {
        Val::Uuid(Some(a)) if *n == name => Some(*a),
        _ => None,
    })
}

pub fn has(vals: &[(&'static str, Val)], name: &str) -> bool {
    vals.iter().any(|(n, _)| *n == name)
}

pub fn push_val(qb: &mut QueryBuilder<'_, Postgres>, v: Val) {
    match v {
        Val::Text(x) => qb.push_bind(x),
        Val::Date(x) => qb.push_bind(x).push("::date"),
        Val::Uuid(x) => qb.push_bind(x),
        Val::TextArr(x) => qb.push_bind(x),
        Val::UuidArr(x) => qb.push_bind(x),
        Val::Json(x) => qb.push_bind(x),
        Val::Time(x) => qb.push_bind(x).push("::time"),
        Val::Bool(x) => qb.push_bind(x),
        Val::Int(x) => qb.push_bind(x),
    };
}

/// `col = $1, col2 = $2`
pub fn push_set(qb: &mut QueryBuilder<'_, Postgres>, vals: Vec<(&'static str, Val)>) {
    for (i, (name, v)) in vals.into_iter().enumerate() {
        if i > 0 {
            qb.push(", ");
        }
        qb.push(name).push(" = ");
        push_val(qb, v);
    }
}

/// `(a, b, extra...) VALUES ($1, $2, ...)`; `extra` columns are pushed by the caller afterwards.
pub fn push_insert_cols(qb: &mut QueryBuilder<'_, Postgres>, vals: &[(&'static str, Val)]) {
    for (name, _) in vals {
        qb.push(*name).push(", ");
    }
}
pub fn push_insert_vals(qb: &mut QueryBuilder<'_, Postgres>, vals: Vec<(&'static str, Val)>) {
    for (_, v) in vals {
        push_val(qb, v);
        qb.push(", ");
    }
}
