use sqlx::{Postgres, QueryBuilder};

use crate::middleware::authentication::AuthUser;

/// Appends: records the caller's tenant may SEE (their FOC's, their province's, their church's).
/// `table` must be a trusted constant ("events", "announcements").
pub fn push_visible(qb: &mut QueryBuilder<'_, Postgres>, table: &str, auth: &AuthUser) {
    qb.push(format!("(({table}.scope = 'church' AND {table}.church_id = "))
        .push_bind(auth.church_id)
        .push(format!(") OR ({table}.scope = 'province' AND {table}.province_id = "))
        .push_bind(auth.province_id)
        .push(format!(") OR ({table}.scope = 'foc' AND {table}.foc_id = "))
        .push_bind(auth.foc_id)
        .push("))");
}

/// Appends: records the caller may CHANGE or DELETE.
///  - FOC-scoped: the caller's own FOC.
///  - Church/province-scoped: visible AND created by the caller's FOC.
///    Rows with no recorded owner (created before `created_by_foc_id` existed) stay
///    editable by any admin who can see them, which is the previous behaviour.
pub fn push_manageable(qb: &mut QueryBuilder<'_, Postgres>, table: &str, auth: &AuthUser) {
    let owner_ok = format!("({table}.created_by_foc_id IS NULL OR {table}.created_by_foc_id = ");
    qb.push(format!("(({table}.scope = 'foc' AND {table}.foc_id = "))
        .push_bind(auth.foc_id)
        .push(format!(") OR ({table}.scope = 'church' AND {table}.church_id = "))
        .push_bind(auth.church_id)
        .push(format!(" AND {owner_ok}"))
        .push_bind(auth.foc_id)
        .push(format!(")) OR ({table}.scope = 'province' AND {table}.province_id = "))
        .push_bind(auth.province_id)
        .push(format!(" AND {owner_ok}"))
        .push_bind(auth.foc_id)
        .push(")))");
}
