use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

type R<T> = Result<T, sqlx::Error>;

// ── age groups ─────────────────────────────────────────────────────────────────────────────────────
pub async fn age_groups(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT to_jsonb(g) || jsonb_build_object('member_count',
            (SELECT count(*) FROM member_profiles m WHERE m.foc_id = g.foc_id AND m.deleted_at IS NULL AND m.dob IS NOT NULL
               AND date_part('year', age(m.dob)) BETWEEN g.min_age AND COALESCE(g.max_age, 200)))
         FROM age_groups g WHERE g.foc_id = $1 ORDER BY g.min_age, g.sort_order",
    ).bind(foc).fetch_all(pool).await
}

pub async fn age_ranges(pool: &PgPool, foc: Uuid, except: Option<Uuid>) -> R<Vec<(String, i32, Option<i32>)>> {
    sqlx::query_as("SELECT name, min_age, max_age FROM age_groups WHERE foc_id = $1 AND ($2::uuid IS NULL OR id <> $2)")
        .bind(foc).bind(except).fetch_all(pool).await
}

pub async fn add_age_group(pool: &PgPool, foc: Uuid, name: &str, min: i32, max: Option<i32>) -> R<Value> {
    sqlx::query_scalar("INSERT INTO age_groups (foc_id, name, min_age, max_age) VALUES ($1,$2,$3,$4) RETURNING to_jsonb(age_groups)")
        .bind(foc).bind(name).bind(min).bind(max).fetch_one(pool).await
}

pub async fn update_age_group(pool: &PgPool, foc: Uuid, id: Uuid, name: &str, min: i32, max: Option<i32>) -> R<Option<Value>> {
    sqlx::query_scalar("UPDATE age_groups SET name=$3, min_age=$4, max_age=$5, updated_at=now() WHERE id=$2 AND foc_id=$1 RETURNING to_jsonb(age_groups)")
        .bind(foc).bind(id).bind(name).bind(min).bind(max).fetch_optional(pool).await
}

pub async fn delete_age_group(pool: &PgPool, foc: Uuid, id: Uuid) -> R<bool> {
    let mut tx = pool.begin().await?;
    let n = sqlx::query("DELETE FROM age_groups WHERE id=$2 AND foc_id=$1").bind(foc).bind(id).execute(&mut *tx).await?.rows_affected();
    if n > 0 {
        sqlx::query("UPDATE church_roles SET scope_age_group_ids = array_remove(scope_age_group_ids, $2) WHERE foc_id = $1").bind(foc).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(n > 0)
}

pub async fn count_age_groups_in(pool: &PgPool, foc: Uuid, ids: &[Uuid]) -> R<i64> {
    sqlx::query_scalar("SELECT count(*) FROM age_groups WHERE foc_id = $1 AND id = ANY($2)").bind(foc).bind(ids).fetch_one(pool).await
}

pub async fn count_cell_groups_in(pool: &PgPool, foc: Uuid, ids: &[Uuid]) -> R<i64> {
    sqlx::query_scalar("SELECT count(*) FROM cell_groups WHERE foc_id = $1 AND id = ANY($2)").bind(foc).bind(ids).fetch_one(pool).await
}

// ── roles ──────────────────────────────────────────────────────────────────────────────────────────
pub async fn roles(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT to_jsonb(r) || jsonb_build_object('member_count', (SELECT count(*) FROM member_church_roles mr WHERE mr.role_id = r.id))
         FROM church_roles r WHERE r.foc_id = $1 ORDER BY lower(r.name)",
    ).bind(foc).fetch_all(pool).await
}

pub async fn add_role(pool: &PgPool, foc: Uuid, name: &str, desc: Option<&str>, perms: &[String], cells: &[Uuid], ages: &[Uuid]) -> R<Value> {
    sqlx::query_scalar(
        "INSERT INTO church_roles (foc_id, name, description, permissions, scope_cell_group_ids, scope_age_group_ids)
         VALUES ($1,$2,$3,$4,$5,$6) RETURNING to_jsonb(church_roles)",
    ).bind(foc).bind(name).bind(desc).bind(perms).bind(cells).bind(ages).fetch_one(pool).await
}

pub async fn update_role(pool: &PgPool, foc: Uuid, id: Uuid, name: &str, desc: Option<&str>, perms: &[String], cells: &[Uuid], ages: &[Uuid]) -> R<Option<Value>> {
    sqlx::query_scalar(
        "UPDATE church_roles SET name=$3, description=$4, permissions=$5, scope_cell_group_ids=$6, scope_age_group_ids=$7, updated_at=now()
         WHERE id=$2 AND foc_id=$1 RETURNING to_jsonb(church_roles)",
    ).bind(foc).bind(id).bind(name).bind(desc).bind(perms).bind(cells).bind(ages).fetch_optional(pool).await
}

pub async fn delete_role(pool: &PgPool, foc: Uuid, id: Uuid) -> R<bool> {
    Ok(sqlx::query("DELETE FROM church_roles WHERE id=$2 AND foc_id=$1").bind(foc).bind(id).execute(pool).await?.rows_affected() > 0)
}

pub async fn role_members(pool: &PgPool, foc: Uuid, id: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', m.id, 'first_name', m.first_name, 'last_name', m.last_name, 'email', m.email)
         FROM member_church_roles mr JOIN church_roles r ON r.id = mr.role_id AND r.foc_id = $1
         JOIN member_profiles m ON m.id = mr.member_id AND m.deleted_at IS NULL
         WHERE mr.role_id = $2 ORDER BY m.first_name, m.last_name",
    ).bind(foc).bind(id).fetch_all(pool).await
}

pub async fn member_roles(pool: &PgPool, foc: Uuid, member: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM member_church_roles mr JOIN church_roles r ON r.id = mr.role_id AND r.foc_id = $1
         WHERE mr.member_id = $2 ORDER BY lower(r.name)",
    ).bind(foc).bind(member).fetch_all(pool).await
}

pub async fn count_roles_in(pool: &PgPool, foc: Uuid, ids: &[Uuid]) -> R<i64> {
    sqlx::query_scalar("SELECT count(*) FROM church_roles WHERE foc_id = $1 AND id = ANY($2)").bind(foc).bind(ids).fetch_one(pool).await
}

pub async fn set_member_roles(pool: &PgPool, member: Uuid, by: Uuid, ids: &[Uuid]) -> R<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM member_church_roles WHERE member_id = $1 AND NOT (role_id = ANY($2))").bind(member).bind(ids).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO member_church_roles (member_id, role_id, assigned_by) SELECT $1, x, $3 FROM unnest($2::uuid[]) x ON CONFLICT DO NOTHING")
        .bind(member).bind(ids).bind(by).execute(&mut *tx).await?;
    tx.commit().await
}

/// None = no limit. Some((cells, ages)) = only members in these cell groups or age groups.
pub async fn member_scope(pool: &PgPool, member: Uuid, foc: Uuid, perm: &str) -> R<Option<(Vec<Uuid>, Vec<Uuid>)>> {
    let rows: Vec<(Vec<Uuid>, Vec<Uuid>)> = sqlx::query_as(
        "SELECT r.scope_cell_group_ids, r.scope_age_group_ids FROM member_church_roles mr
         JOIN church_roles r ON r.id = mr.role_id AND r.foc_id = $2 WHERE mr.member_id = $1 AND $3 = ANY(r.permissions)",
    ).bind(member).bind(foc).bind(perm).fetch_all(pool).await?;
    if rows.iter().any(|(c, a)| c.is_empty() && a.is_empty()) || rows.is_empty() {
        return Ok(None);
    }
    let (mut cells, mut ages) = (Vec::new(), Vec::new());
    for (c, a) in rows { cells.extend(c); ages.extend(a); }
    Ok(Some((cells, ages)))
}

pub async fn in_scope(pool: &PgPool, id: Uuid, foc: Uuid, cells: &[Uuid], ages: &[Uuid]) -> R<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM member_profiles m WHERE m.id = $1 AND m.foc_id = $2 AND (m.cell_group_id = ANY($3)
           OR EXISTS (SELECT 1 FROM age_groups g WHERE g.id = ANY($4) AND g.foc_id = m.foc_id AND m.dob IS NOT NULL
              AND date_part('year', age(m.dob)) BETWEEN g.min_age AND COALESCE(g.max_age, 200))))",
    ).bind(id).bind(foc).bind(cells).bind(ages).fetch_one(pool).await
}
