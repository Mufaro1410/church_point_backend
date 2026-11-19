use serde_json::Value;
use sqlx::{PgPool, QueryBuilder};
use uuid::Uuid;

use crate::{
    models::member::Member,
    utils::dynamic::{self, Val},
};

const COLUMNS: &str = "id, user_id, foc_id, email, role::text AS role, \
                       first_name, last_name, status::text AS status";

pub struct Filter {
    pub status: Option<String>,
    /// Already wrapped as `%text%` with LIKE wildcards escaped.
    pub search: Option<String>,
    pub cell_group_id: Option<Uuid>,
    /// Only members whose age falls in this age group.
    pub age_group_id: Option<Uuid>,
    /// A role limits what this person may see: only these cell groups or age groups.
    pub scope: Option<(Vec<Uuid>, Vec<Uuid>)>,
}

const WHERE: &str = "m.foc_id = $1 AND m.deleted_at IS NULL
    AND ($2::text IS NULL OR m.status = $2)
    AND ($3::text IS NULL OR m.first_name ILIKE $3 OR m.last_name ILIKE $3 OR m.email ILIKE $3 OR m.phone ILIKE $3)
    AND ($4::uuid IS NULL OR m.cell_group_id = $4)
    AND (NOT $5::bool OR m.cell_group_id = ANY($6::uuid[]) OR EXISTS (SELECT 1 FROM age_groups g WHERE g.id = ANY($7::uuid[])
          AND g.foc_id = m.foc_id AND m.dob IS NOT NULL AND date_part('year', age(m.dob)) BETWEEN g.min_age AND COALESCE(g.max_age, 200)))
    AND ($8::uuid IS NULL OR EXISTS (SELECT 1 FROM age_groups g WHERE g.id = $8 AND g.foc_id = m.foc_id AND m.dob IS NOT NULL
          AND date_part('year', age(m.dob)) BETWEEN g.min_age AND COALESCE(g.max_age, 200)))";

/// Every query takes `foc_id`: tenant scoping lives HERE because the Axum
/// connection bypasses Supabase RLS.
pub async fn list_json(pool: &PgPool, foc_id: Uuid, f: &Filter, limit: i64, offset: i64) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT to_jsonb(m) FROM member_profiles m WHERE {WHERE}
         ORDER BY m.first_name ASC, m.last_name ASC LIMIT $9 OFFSET $10"
    ))
    .bind(foc_id)
    .bind(&f.status)
    .bind(&f.search)
    .bind(f.cell_group_id)
    .bind(f.scope.is_some())
    .bind(f.scope.as_ref().map(|s| s.0.clone()).unwrap_or_default())
    .bind(f.scope.as_ref().map(|s| s.1.clone()).unwrap_or_default())
    .bind(f.age_group_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_filtered(pool: &PgPool, foc_id: Uuid, f: &Filter) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(&format!("SELECT count(*) FROM member_profiles m WHERE {WHERE}"))
        .bind(foc_id)
        .bind(&f.status)
        .bind(&f.search)
        .bind(f.cell_group_id)
        .bind(f.scope.is_some())
        .bind(f.scope.as_ref().map(|s| s.0.clone()).unwrap_or_default())
        .bind(f.scope.as_ref().map(|s| s.1.clone()).unwrap_or_default())
        .bind(f.age_group_id)
        .fetch_one(pool)
        .await
}

pub async fn get_json(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(m) FROM member_profiles m WHERE m.id = $1 AND m.foc_id = $2 AND m.deleted_at IS NULL",
    )
    .bind(id)
    .bind(foc_id)
    .fetch_optional(pool)
    .await
}

pub async fn find_in_foc(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<Option<Member>, sqlx::Error> {
    sqlx::query_as::<_, Member>(&format!(
        "SELECT {COLUMNS} FROM member_profiles WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL"
    ))
    .bind(id)
    .bind(foc_id)
    .fetch_optional(pool)
    .await
}

pub async fn insert_member(pool: &PgPool, foc_id: Uuid, vals: Vec<(&'static str, Val)>) -> Result<Value, sqlx::Error> {
    let mut qb = QueryBuilder::new("INSERT INTO member_profiles (");
    dynamic::push_insert_cols(&mut qb, &vals);
    qb.push("foc_id, role, status, created_at) VALUES (");
    dynamic::push_insert_vals(&mut qb, vals);
    qb.push_bind(foc_id).push(", 'member', 'approved', now()) RETURNING to_jsonb(member_profiles)");
    qb.build_query_scalar().fetch_one(pool).await
}

pub async fn update_member(
    pool: &PgPool,
    id: Uuid,
    foc_id: Uuid,
    vals: Vec<(&'static str, Val)>,
) -> Result<Option<Value>, sqlx::Error> {
    let mut qb = QueryBuilder::new("UPDATE member_profiles SET ");
    dynamic::push_set(&mut qb, vals);
    qb.push(", updated_at = now() WHERE id = ").push_bind(id);
    qb.push(" AND foc_id = ").push_bind(foc_id);
    qb.push(" AND deleted_at IS NULL RETURNING to_jsonb(member_profiles)");
    qb.build_query_scalar().fetch_optional(pool).await
}

pub async fn set_status(pool: &PgPool, id: Uuid, foc_id: Uuid, status: &str) -> Result<Option<Value>, sqlx::Error> {
    // status is validated by the service; sent as a SQL parameter cast to text.
    sqlx::query_scalar(
        "UPDATE member_profiles SET status = $3, updated_at = now()
         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL RETURNING to_jsonb(member_profiles)",
    )
    .bind(id)
    .bind(foc_id)
    .bind(status)
    .fetch_optional(pool)
    .await
}

/// Takes the member out of this FOC (they keep their account) with the given status.
pub async fn release(pool: &PgPool, id: Uuid, foc_id: Uuid, status: &str) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE member_profiles SET foc_id = NULL, cell_group_id = NULL, status = $3, updated_at = now()
         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(foc_id)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn soft_delete(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE member_profiles SET deleted_at = now()
         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(foc_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn ministries_exist(pool: &PgPool, ids: &[Uuid]) -> Result<bool, sqlx::Error> {
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM ministries WHERE id = ANY($1)")
        .bind(ids)
        .fetch_one(pool)
        .await?;
    Ok(n as usize == ids.len())
}

pub async fn list_family(pool: &PgPool, member: Uuid, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(rm) || jsonb_build_object('relationship_type', f.relationship_type, 'relationship_id', f.id)
         FROM family_relationships f
         JOIN member_profiles me ON me.id = f.member_id AND me.foc_id = $2
         JOIN member_profiles rm ON rm.id = f.related_member_id AND rm.foc_id = $2 AND rm.deleted_at IS NULL
         WHERE f.member_id = $1 ORDER BY rm.first_name",
    )
    .bind(member)
    .bind(foc_id)
    .fetch_all(pool)
    .await
}

pub async fn add_relationship(pool: &PgPool, member: Uuid, related: Uuid, kind: &str) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO family_relationships(member_id, related_member_id, relationship_type)
         VALUES ($1,$2,$3) RETURNING to_jsonb(family_relationships)",
    )
    .bind(member)
    .bind(related)
    .bind(kind)
    .fetch_one(pool)
    .await
}

pub async fn remove_relationship(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM family_relationships f USING member_profiles m
         WHERE f.id = $1 AND m.id = f.member_id AND m.foc_id = $2",
    )
    .bind(id)
    .bind(foc_id)
    .execute(pool)
    .await?
    .rows_affected())
}

/// Names for a set of member ids, restricted to the caller's FOC.
pub async fn names_by_ids(
    pool: &PgPool,
    ids: &[Uuid],
    foc_id: Uuid,
) -> Result<Vec<crate::models::event::MemberName>, sqlx::Error> {
    sqlx::query_as::<_, crate::models::event::MemberName>(
        "SELECT id, first_name, last_name FROM member_profiles
         WHERE id = ANY($1) AND foc_id = $2 AND deleted_at IS NULL",
    )
    .bind(ids)
    .bind(foc_id)
    .fetch_all(pool)
    .await
}

/// "First Last" for a member (used to stamp the testimony `author` column server-side).
pub async fn full_name(pool: &PgPool, id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT NULLIF(btrim(concat_ws(' ', first_name, last_name)), '')
         FROM member_profiles WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map(Option::flatten)
}

pub async fn contributions(pool: &PgPool, member: Uuid, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(i) || jsonb_build_object(
            'category', jsonb_build_object('id', c.id, 'name', c.name),
            'currency', jsonb_build_object('id', cu.id, 'name', cu.name, 'symbol', cu.symbol),
            'method',   jsonb_build_object('id', me.id, 'name', me.name))
         FROM incomes i
         JOIN categories c ON c.id = i.category_id
         JOIN currencies cu ON cu.id = i.currency_id
         JOIN methods me ON me.id = i.method_id
         WHERE i.member_id = $1 AND i.foc_id = $2 AND i.voided_at IS NULL
         ORDER BY i.created_at DESC",
    )
    .bind(member)
    .bind(foc_id)
    .fetch_all(pool)
    .await
}

/// Events (visible to the caller) this member checked in to, newest first.
pub async fn attendance(
    pool: &PgPool,
    member: Uuid,
    foc_id: Uuid,
    church_id: Option<Uuid>,
    province_id: Option<Uuid>,
) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('eventId', e.id, 'eventTitle', e.title, 'eventDate', e.date,
                                   'eventLocation', e.location, 'timestamp', e.attendance ->> $1::text)
         FROM events e
         WHERE e.attendance ? $1::text
           AND (e.foc_id = $2 OR (e.scope = 'church' AND e.church_id = $3) OR (e.scope = 'province' AND e.province_id = $4))
         ORDER BY (e.attendance ->> $1::text) DESC",
    )
    .bind(member.to_string())
    .bind(foc_id)
    .bind(church_id)
    .bind(province_id)
    .fetch_all(pool)
    .await
}

/// Prayers, testimonies and cell meetings of a member. Anonymous items are never attributed.
pub async fn activity(pool: &PgPool, member: Uuid, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(a) FROM (
           SELECT 'prayer' AS type, 'Prayer Request' AS title, left(p.title, 80) AS description,
                  p.created_at AS date, p.id::text AS id
             FROM prayers p WHERE p.profile_id = $1 AND p.foc_id = $2 AND NOT p.is_anonymous
           UNION ALL
           SELECT 'testimony', 'Testimony', left(t.title, 80), t.created_at, t.id::text
             FROM testimonies t WHERE t.profile_id = $1 AND t.foc_id = $2 AND NOT COALESCE(t.is_anonymous, false)
           UNION ALL
           SELECT 'cell_meeting', 'Cell Meeting', 'Attended cell meeting', m.date::timestamptz, m.id::text
             FROM cell_meetings m JOIN cell_groups g ON g.id = m.cell_group_id
            WHERE g.foc_id = $2 AND EXISTS (
              SELECT 1 FROM jsonb_array_elements(COALESCE(m.attendance, '[]'::jsonb)) x
               WHERE x #>> '{}' = $1::text OR x ->> 'member_id' = $1::text OR x ->> 'id' = $1::text)
         ) a ORDER BY a.date DESC",
    )
    .bind(member)
    .bind(foc_id)
    .fetch_all(pool)
    .await
}

/// Role change. Only people who have a login can hold a role above `member`.
pub async fn set_role(pool: &PgPool, id: Uuid, foc_id: Uuid, role: &str) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE member_profiles SET role = $3, updated_at = now()
         WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL AND role <> 'super_admin'
           AND ($3 = 'member' OR user_id IS NOT NULL)
         RETURNING to_jsonb(member_profiles)",
    )
    .bind(id).bind(foc_id).bind(role).fetch_optional(pool).await
}
