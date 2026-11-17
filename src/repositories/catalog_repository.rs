use serde_json::Value;
use sqlx::{PgPool, QueryBuilder};
use uuid::Uuid;

use crate::utils::dynamic::{self, Val};

const DEFAULT_LOGO: &str = "https://fcmeczxeixdczfmgdnra.supabase.co/storage/v1/object/public/churchpoint//churchpoint.png";

// Table names below are string literals from our own code, never user input.

pub async fn insert_row(pool: &PgPool, table: &str, mut vals: Vec<(&'static str, Val)>, extra: Vec<(&'static str, Val)>) -> Result<Value, sqlx::Error> {
    vals.extend(extra);
    let mut qb = QueryBuilder::new(format!("INSERT INTO {table} ("));
    for (i, (n, _)) in vals.iter().enumerate() {
        if i > 0 { qb.push(", "); }
        qb.push(*n);
    }
    qb.push(") VALUES (");
    for (i, (_, v)) in vals.into_iter().enumerate() {
        if i > 0 { qb.push(", "); }
        dynamic::push_val(&mut qb, v);
    }
    qb.push(format!(") RETURNING to_jsonb({table})"));
    qb.build_query_scalar().fetch_one(pool).await
}

/// UPDATE ... WHERE id = $id AND <scope_col> = $scope [AND <extra_where>]
pub async fn update_row(
    pool: &PgPool,
    table: &str,
    id: Uuid,
    scope_col: &str,
    scope: Uuid,
    extra_where: &str,
    vals: Vec<(&'static str, Val)>,
) -> Result<Option<Value>, sqlx::Error> {
    let mut qb = QueryBuilder::new(format!("UPDATE {table} SET "));
    dynamic::push_set(&mut qb, vals);
    qb.push(", updated_at = now() WHERE id = ").push_bind(id);
    qb.push(format!(" AND {scope_col} = ")).push_bind(scope);
    if !extra_where.is_empty() {
        qb.push(format!(" AND {extra_where}"));
    }
    qb.push(format!(" RETURNING to_jsonb({table})"));
    qb.build_query_scalar().fetch_optional(pool).await
}

pub async fn delete_row(pool: &PgPool, table: &str, id: Uuid, scope_col: &str, scope: Uuid) -> Result<u64, sqlx::Error> {
    let sql = format!("DELETE FROM {table} WHERE id = $1 AND {scope_col} = $2");
    Ok(sqlx::query(&sql).bind(id).bind(scope).execute(pool).await?.rows_affected())
}

// ── ministries ──

pub async fn list_ministries(pool: &PgPool, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(m) || jsonb_build_object(
            'selected', EXISTS(SELECT 1 FROM foc_ministries fm WHERE fm.ministry_id = m.id AND fm.foc_id = $1),
            'leader_id', (SELECT l.leader_id FROM ministry_leaders l WHERE l.ministry_id = m.id AND l.foc_id = $1))
         FROM ministries m ORDER BY m.name ASC",
    )
    .bind(foc_id)
    .fetch_all(pool)
    .await
}

pub async fn ministry_exists(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ministries WHERE id = $1)").bind(id).fetch_one(pool).await
}

pub async fn ministry_name_taken(pool: &PgPool, name: &str, except: Option<Uuid>) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ministries WHERE lower(name) = lower($1) AND ($2::uuid IS NULL OR id <> $2))")
        .bind(name)
        .bind(except)
        .fetch_one(pool)
        .await
}

pub async fn insert_ministry(pool: &PgPool, foc_id: Uuid, name: &str, description: Option<&str>) -> Result<Value, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let row: Value = sqlx::query_scalar("INSERT INTO ministries(name, description) VALUES ($1,$2) RETURNING to_jsonb(ministries)")
        .bind(name)
        .bind(description)
        .fetch_one(&mut *tx)
        .await?;
    let id: Uuid = serde_json::from_value(row["id"].clone()).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    sqlx::query("INSERT INTO foc_ministries(foc_id, ministry_id) VALUES ($1,$2)").bind(foc_id).bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn set_ministry_selected(pool: &PgPool, foc_id: Uuid, id: Uuid, selected: bool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    if selected {
        sqlx::query("INSERT INTO foc_ministries(foc_id, ministry_id) SELECT $1, $2 WHERE NOT EXISTS (SELECT 1 FROM foc_ministries WHERE foc_id=$1 AND ministry_id=$2)")
            .bind(foc_id).bind(id).execute(&mut *tx).await?;
    } else {
        sqlx::query("DELETE FROM foc_ministries WHERE foc_id=$1 AND ministry_id=$2").bind(foc_id).bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM ministry_leaders WHERE foc_id=$1 AND ministry_id=$2").bind(foc_id).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await
}

pub async fn ministry_selected(pool: &PgPool, foc_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM foc_ministries WHERE foc_id=$1 AND ministry_id=$2)")
        .bind(foc_id).bind(id).fetch_one(pool).await
}

pub async fn set_ministry_leader(pool: &PgPool, foc_id: Uuid, id: Uuid, leader: Option<Uuid>) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO ministry_leaders(ministry_id, foc_id, leader_id) VALUES ($1,$2,$3)
         ON CONFLICT (ministry_id, foc_id) DO UPDATE SET leader_id = EXCLUDED.leader_id, updated_at = now()
         RETURNING to_jsonb(ministry_leaders)",
    )
    .bind(id).bind(foc_id).bind(leader).fetch_one(pool).await
}

pub async fn rename_ministry(pool: &PgPool, id: Uuid, name: Option<&str>, description: Option<&str>) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE ministries SET name = COALESCE($2, name), description = COALESCE($3, description), updated_at = now()
         WHERE id = $1 RETURNING to_jsonb(ministries)",
    )
    .bind(id).bind(name).bind(description).fetch_optional(pool).await
}

pub async fn delete_ministry(pool: &PgPool, id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM ministries WHERE id = $1").bind(id).execute(pool).await?.rows_affected())
}

// ── courses (belong to a church) ──

pub async fn list_courses(pool: &PgPool, church_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(c) FROM courses c WHERE c.church_id = $1 ORDER BY c.name ASC")
        .bind(church_id).fetch_all(pool).await
}

pub async fn course_name_taken(pool: &PgPool, church_id: Uuid, name: &str, except: Option<Uuid>) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM courses WHERE church_id=$1 AND lower(name)=lower($2) AND ($3::uuid IS NULL OR id <> $3))")
        .bind(church_id).bind(name).bind(except).fetch_one(pool).await
}

pub async fn insert_course(pool: &PgPool, church_id: Uuid, name: &str, description: Option<&str>) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar("INSERT INTO courses(name, description, church_id) VALUES ($1,$2,$3) RETURNING to_jsonb(courses)")
        .bind(name).bind(description).bind(church_id).fetch_one(pool).await
}

pub async fn rename_course(pool: &PgPool, id: Uuid, name: Option<&str>, description: Option<&str>) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE courses SET name = COALESCE($2, name), description = COALESCE($3, description), updated_at = now()
         WHERE id = $1 RETURNING to_jsonb(courses)",
    )
    .bind(id).bind(name).bind(description).fetch_optional(pool).await
}

pub async fn delete_course(pool: &PgPool, id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM courses WHERE id = $1").bind(id).execute(pool).await?.rows_affected())
}

// ── event serving ──

pub async fn list_serving(pool: &PgPool, event: Uuid, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM event_serving s JOIN member_profiles m ON m.id = s.profile_id
         WHERE s.event_id = $1 AND m.foc_id = $2 ORDER BY s.created_at DESC",
    )
    .bind(event).bind(foc_id).fetch_all(pool).await
}

pub async fn insert_serving(pool: &PgPool, event: Uuid, profile: Uuid, ministry: Option<Uuid>, capacity: Option<&str>) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO event_serving(event_id, profile_id, ministry_id, capacity) VALUES ($1,$2,$3,$4)
         RETURNING to_jsonb(event_serving)",
    )
    .bind(event).bind(profile).bind(ministry).bind(capacity).fetch_one(pool).await
}

pub async fn delete_serving(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM event_serving s USING member_profiles m WHERE s.id = $1 AND m.id = s.profile_id AND m.foc_id = $2")
        .bind(id).bind(foc_id).execute(pool).await?.rows_affected())
}

// ── social events, bible plans ──

pub async fn list_social(pool: &PgPool, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(s) FROM social_events s WHERE s.foc_id = $1 ORDER BY s.date DESC, s.created_at DESC")
        .bind(foc_id).fetch_all(pool).await
}

pub async fn list_plans(pool: &PgPool, user_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(p) || jsonb_build_object('is_mine', p.user_id = $1)
         FROM bible_reading_plans p WHERE p.deleted_at IS NULL ORDER BY p.created_at DESC",
    )
    .bind(user_id).fetch_all(pool).await
}

pub async fn soft_delete_plan(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("UPDATE bible_reading_plans SET deleted_at = now() WHERE id=$1 AND user_id=$2 AND deleted_at IS NULL")
        .bind(id).bind(user_id).execute(pool).await?.rows_affected())
}

// ── church / focs ──

pub async fn church(pool: &PgPool, id: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar("SELECT to_jsonb(c) FROM church c WHERE c.id = $1").bind(id).fetch_optional(pool).await
}

pub async fn focs_in_church(pool: &PgPool, church_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(f) - 'payment_details' FROM foc_profile f
         WHERE f.church_id = $1 AND f.status = 'approved' ORDER BY f.name ASC",
    )
    .bind(church_id).fetch_all(pool).await
}

pub async fn church_exists(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM church WHERE id = $1)").bind(id).fetch_one(pool).await
}

pub async fn profile_exists_for_user(pool: &PgPool, user_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM member_profiles WHERE user_id = $1)").bind(user_id).fetch_one(pool).await
}

pub async fn register_foc(
    pool: &PgPool,
    user_id: Uuid,
    c: &crate::models::catalog::ChurchData,
    u: &crate::models::catalog::UserData,
    dob: Option<&str>,
) -> Result<Value, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let foc_id: Uuid = sqlx::query_scalar(
        "INSERT INTO foc_profile(name, address, email, contact, country, church_id, logo, status)
         VALUES ($1,$2,$3,$4,COALESCE($5,''),$6, COALESCE($7, $8), 'pending')
         RETURNING id",
    )
    .bind(&c.name).bind(&c.address).bind(&c.email).bind(&c.contact)
    .bind(c.country.as_deref()).bind(c.church_id).bind(c.logo.as_deref().filter(|s| !s.trim().is_empty()))
    .bind(DEFAULT_LOGO)
    .fetch_one(&mut *tx).await?;
    let profile_id: Uuid = sqlx::query_scalar(
        "INSERT INTO member_profiles(user_id, first_name, last_name, email, phone, address, dob, gender, role, status, foc_id, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7::date,$8,'admin','approved',$9,now()) RETURNING id",
    )
    .bind(user_id).bind(&u.first_name).bind(&u.last_name).bind(&u.email).bind(&u.phone)
    .bind(u.address.as_deref().unwrap_or("")).bind(dob).bind(&u.gender).bind(foc_id)
    .fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(serde_json::json!({ "foc_id": foc_id, "profile_id": profile_id, "church_id": c.church_id, "foc_status": "pending" }))
}

// ── event services ──

pub async fn list_services(pool: &PgPool, event: Uuid, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM event_services s JOIN member_profiles m ON m.id = s.member_id
         WHERE s.event_id = $1 AND m.foc_id = $2 ORDER BY s.created_at ASC",
    )
    .bind(event).bind(foc_id).fetch_all(pool).await
}

pub async fn insert_service(
    pool: &PgPool,
    event: Uuid,
    member: Uuid,
    role: &str,
    domain: Option<&str>,
    notes: Option<&str>,
    marked_by: Uuid,
) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO event_services(event_id, member_id, service_role, service_domain, notes, marked_by, source)
         VALUES ($1,$2,$3,$4,$5,$6,'manual') RETURNING to_jsonb(event_services)",
    )
    .bind(event).bind(member).bind(role).bind(domain).bind(notes).bind(marked_by)
    .fetch_one(pool).await
}

pub async fn delete_service(pool: &PgPool, id: Uuid, foc_id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM event_services s USING member_profiles m WHERE s.id = $1 AND m.id = s.member_id AND m.foc_id = $2")
        .bind(id).bind(foc_id).execute(pool).await?.rows_affected())
}

pub async fn email_taken(pool: &PgPool, email: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM member_profiles WHERE lower(email) = lower($1) AND deleted_at IS NULL)")
        .bind(email).fetch_one(pool).await
}

pub async fn list_all_serving(pool: &PgPool, foc_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM event_serving s JOIN member_profiles m ON m.id = s.profile_id
         WHERE m.foc_id = $1 ORDER BY s.created_at DESC",
    )
    .bind(foc_id).fetch_all(pool).await
}
