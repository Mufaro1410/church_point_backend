//! Queries for the member (mobile) API. Rows come back in the same shape the app already stores.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

type R<T> = Result<T, sqlx::Error>;

pub async fn foc(pool: &PgPool, foc: Uuid) -> R<Option<Value>> {
    sqlx::query_scalar(
        "SELECT to_jsonb(f)
            || jsonb_build_object('church', (SELECT jsonb_build_object('name', c.name, 'region_label', c.region_label) FROM church c WHERE c.id = f.church_id),
                                  'province', (SELECT jsonb_build_object('name', p.name) FROM province p WHERE p.id = f.province_id))
         FROM foc_profile f WHERE f.id = $1",
    ).bind(foc).fetch_optional(pool).await
}

pub async fn public_focs(pool: &PgPool) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', f.id, 'name', f.name, 'address', f.address, 'logo', f.logo,
                                   'church_id', f.church_id, 'province_id', f.province_id)
         FROM foc_profile f WHERE f.status = 'approved' ORDER BY f.name",
    ).fetch_all(pool).await
}

pub async fn sermons(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar("SELECT to_jsonb(s) - 'content' FROM sermons s WHERE s.foc_id = $1 ORDER BY s.date DESC LIMIT 30")
        .bind(foc).fetch_all(pool).await
}

pub async fn sermon_content(pool: &PgPool, id: Uuid, foc: Uuid) -> R<Option<Option<String>>> {
    sqlx::query_scalar("SELECT s.content FROM sermons s WHERE s.id = $1 AND s.foc_id = $2").bind(id).bind(foc).fetch_optional(pool).await
}

pub async fn ministries(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', m.id, 'name', m.name, 'description', m.description, 'updated_at', m.updated_at)
         FROM foc_ministries fm JOIN ministries m ON m.id = fm.ministry_id WHERE fm.foc_id = $1 ORDER BY m.name",
    ).bind(foc).fetch_all(pool).await
}

pub async fn ministry_leaders(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT to_jsonb(l) || jsonb_build_object('leader', jsonb_build_object('first_name', m.first_name, 'last_name', m.last_name))
         FROM ministry_leaders l LEFT JOIN member_profiles m ON m.id = l.leader_id WHERE l.foc_id = $1",
    ).bind(foc).fetch_all(pool).await
}

pub async fn cell_groups(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT to_jsonb(g) || jsonb_build_object('leader', jsonb_build_object('first_name', m.first_name, 'last_name', m.last_name))
         FROM cell_groups g LEFT JOIN member_profiles m ON m.id = g.leader_id WHERE g.foc_id = $1 ORDER BY g.name LIMIT 50",
    ).bind(foc).fetch_all(pool).await
}

pub async fn courses(pool: &PgPool, church: Option<Uuid>) -> R<Vec<Value>> {
    sqlx::query_scalar("SELECT to_jsonb(c) FROM courses c WHERE c.church_id = $1 ORDER BY c.name LIMIT 50")
        .bind(church).fetch_all(pool).await
}

pub async fn reading_plans(pool: &PgPool) -> R<Vec<Value>> {
    sqlx::query_scalar("SELECT to_jsonb(p) FROM bible_reading_plans p WHERE p.deleted_at IS NULL ORDER BY p.created_at DESC LIMIT 20")
        .fetch_all(pool).await
}

pub async fn reading_progress(pool: &PgPool, member: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar("SELECT to_jsonb(p) FROM bible_reading_progress p WHERE p.profile_id = $1").bind(member).fetch_all(pool).await
}

pub async fn my_incomes(pool: &PgPool, foc: Uuid, member: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT to_jsonb(i) || jsonb_build_object('method', jsonb_build_object('name', mt.name),
                                                  'categories', jsonb_build_object('name', c.name),
                                                  'currencies', jsonb_build_object('symbol', cu.symbol))
         FROM incomes i LEFT JOIN methods mt ON mt.id = i.method_id LEFT JOIN categories c ON c.id = i.category_id
         LEFT JOIN currencies cu ON cu.id = i.currency_id
         WHERE i.foc_id = $1 AND i.member_id = $2 AND i.voided_at IS NULL ORDER BY i.date DESC, i.created_at DESC LIMIT 200",
    ).bind(foc).bind(member).fetch_all(pool).await
}

pub async fn birthdays(pool: &PgPool, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', m.id, 'first_name', m.first_name, 'last_name', m.last_name, 'profile_image', m.profile_image, 'dob', m.dob)
         FROM member_profiles m WHERE m.foc_id = $1 AND m.deleted_at IS NULL AND m.status = 'approved' AND m.dob IS NOT NULL",
    ).bind(foc).fetch_all(pool).await
}

// ── events ──

pub async fn set_rsvp(pool: &PgPool, event: Uuid, member: Uuid, status: &str) -> R<Value> {
    sqlx::query_scalar(
        "UPDATE events SET rsvp = jsonb_set(COALESCE(rsvp, '{}'::jsonb), ARRAY[$2::text], to_jsonb($3::text), true), updated_at = now()
         WHERE id = $1 RETURNING COALESCE(rsvp, '{}'::jsonb)",
    ).bind(event).bind(member.to_string()).bind(status).fetch_one(pool).await
}

/// Records the first check-in only; the event must be happening around today.
pub async fn check_in(pool: &PgPool, event: Uuid, member: Uuid) -> R<Option<Value>> {
    sqlx::query_scalar(
        "UPDATE events SET attendance = CASE WHEN COALESCE(attendance, '{}'::jsonb) ? $2::text THEN attendance
                                             ELSE COALESCE(attendance, '{}'::jsonb) || jsonb_build_object($2::text, to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')) END,
                           updated_at = now()
         WHERE id = $1 AND date BETWEEN CURRENT_DATE - 1 AND CURRENT_DATE + 1
         RETURNING COALESCE(attendance, '{}'::jsonb)",
    ).bind(event).bind(member.to_string()).fetch_optional(pool).await
}

pub async fn upsert_serving(pool: &PgPool, event: Uuid, member: Uuid, ministry: Option<Uuid>, capacity: &str) -> R<Value> {
    sqlx::query_scalar(
        "INSERT INTO event_serving (event_id, profile_id, ministry_id, capacity, marked_at) VALUES ($1,$2,$3,$4, now())
         ON CONFLICT (event_id, profile_id) DO UPDATE SET ministry_id = $3, capacity = $4, marked_at = now(), updated_at = now()
         RETURNING to_jsonb(event_serving)",
    ).bind(event).bind(member).bind(ministry).bind(capacity).fetch_one(pool).await
}

// ── reactions ──

pub async fn toggle_prayer_reaction(pool: &PgPool, id: Uuid, foc: Uuid, member: Uuid) -> R<Option<Vec<String>>> {
    sqlx::query_scalar(
        "UPDATE prayers SET reactions = CASE WHEN $2::text = ANY(COALESCE(reactions, '{}')) THEN array_remove(reactions, $2::text)
                                             ELSE array_append(COALESCE(reactions, '{}'), $2::text) END, updated_at = now()
         WHERE id = $1 AND foc_id = $3 RETURNING COALESCE(reactions, '{}')",
    ).bind(id).bind(member.to_string()).bind(foc).fetch_optional(pool).await
}

pub async fn toggle_testimony_reaction(pool: &PgPool, id: Uuid, foc: Uuid, member: Uuid) -> R<Option<Value>> {
    sqlx::query_scalar(
        "UPDATE testimonies SET reactions = CASE WHEN COALESCE(reactions, '[]'::jsonb) ? $2::text THEN reactions - $2::text
                                                 ELSE COALESCE(reactions, '[]'::jsonb) || to_jsonb($2::text) END, updated_at = now()
         WHERE id = $1 AND foc_id = $3 AND status = 'approved' RETURNING reactions",
    ).bind(id).bind(member.to_string()).bind(foc).fetch_optional(pool).await
}

// ── reading progress ──

pub async fn merge_progress(pool: &PgPool, member: Uuid, plan: Uuid, progress: &Value) -> R<Value> {
    sqlx::query_scalar(
        "INSERT INTO bible_reading_progress (profile_id, plan_id, progress_data) VALUES ($1,$2,$3)
         ON CONFLICT (profile_id, plan_id) DO UPDATE SET progress_data = bible_reading_progress.progress_data || EXCLUDED.progress_data, updated_at = now()
         RETURNING to_jsonb(bible_reading_progress)",
    ).bind(member).bind(plan).bind(progress).fetch_one(pool).await
}

pub async fn progress_for(pool: &PgPool, member: Uuid, plan: Uuid) -> R<Option<Value>> {
    sqlx::query_scalar("SELECT to_jsonb(p) FROM bible_reading_progress p WHERE p.profile_id = $1 AND p.plan_id = $2")
        .bind(member).bind(plan).fetch_optional(pool).await
}

pub async fn plan_exists(pool: &PgPool, plan: Uuid) -> R<bool> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bible_reading_plans WHERE id = $1 AND deleted_at IS NULL)").bind(plan).fetch_one(pool).await
}

// ── family ──

pub async fn my_family(pool: &PgPool, member: Uuid, foc: Uuid) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', rm.id, 'first_name', rm.first_name, 'last_name', rm.last_name,
                                   'profile_image', rm.profile_image, 'relationship_type', f.relationship_type)
         FROM family_relationships f JOIN member_profiles rm ON rm.id = f.related_member_id AND rm.foc_id = $2 AND rm.deleted_at IS NULL
         WHERE f.member_id = $1 ORDER BY rm.first_name",
    ).bind(member).bind(foc).fetch_all(pool).await
}

pub async fn search_members(pool: &PgPool, foc: Uuid, me: Uuid, like: &str) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('id', m.id, 'first_name', m.first_name, 'last_name', m.last_name, 'profile_image', m.profile_image)
         FROM member_profiles m
         WHERE m.foc_id = $1 AND m.id <> $2 AND m.deleted_at IS NULL AND m.status = 'approved'
           AND (m.first_name ILIKE $3 OR m.last_name ILIKE $3 OR concat_ws(' ', m.first_name, m.last_name) ILIKE $3)
         ORDER BY m.first_name LIMIT 10",
    ).bind(foc).bind(me).bind(like).fetch_all(pool).await
}

pub async fn member_in_foc(pool: &PgPool, id: Uuid, foc: Uuid) -> R<bool> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM member_profiles WHERE id = $1 AND foc_id = $2 AND deleted_at IS NULL AND status = 'approved')")
        .bind(id).bind(foc).fetch_one(pool).await
}

pub async fn add_family(pool: &PgPool, me: Uuid, related: Uuid, kind: &str) -> R<u64> {
    Ok(sqlx::query("INSERT INTO family_relationships (member_id, related_member_id, relationship_type) VALUES ($1,$2,$3)
                    ON CONFLICT (member_id, related_member_id) DO UPDATE SET relationship_type = $3")
        .bind(me).bind(related).bind(kind).execute(pool).await?.rows_affected())
}

pub async fn remove_family(pool: &PgPool, me: Uuid, related: Uuid) -> R<u64> {
    Ok(sqlx::query("DELETE FROM family_relationships WHERE member_id = $1 AND related_member_id = $2")
        .bind(me).bind(related).execute(pool).await?.rows_affected())
}

// ── own profile ──

pub async fn choose_church(pool: &PgPool, me: Uuid, foc: Uuid) -> R<u64> {
    Ok(sqlx::query(
        "UPDATE member_profiles m SET foc_id = $2, status = 'pending', updated_at = now()
         WHERE m.id = $1 AND m.foc_id IS NULL AND m.deleted_at IS NULL
           AND EXISTS (SELECT 1 FROM foc_profile f WHERE f.id = $2 AND f.status = 'approved')",
    ).bind(me).bind(foc).execute(pool).await?.rows_affected())
}

pub async fn change_status(pool: &PgPool, me: Uuid, from: &str, to: &str) -> R<u64> {
    Ok(sqlx::query("UPDATE member_profiles SET status = $3, updated_at = now() WHERE id = $1 AND status = $2 AND deleted_at IS NULL")
        .bind(me).bind(from).bind(to).execute(pool).await?.rows_affected())
}

pub async fn save_push_token(pool: &PgPool, me: Uuid, token: &str) -> R<()> {
    sqlx::query("INSERT INTO member_push_tokens (member_id, token) VALUES ($1,$2)
                 ON CONFLICT (member_id) DO UPDATE SET token = $2, updated_at = now()")
        .bind(me).bind(token).execute(pool).await?;
    Ok(())
}

pub async fn register_profile(pool: &PgPool, user: Uuid, r: &crate::models::mobile::RegisterMemberRequest, email: &str) -> R<Value> {
    sqlx::query_scalar("SELECT register_user_with_profile($1,$2,$3,$4,$5,$6,$7, now())::jsonb")
        .bind(user).bind(&r.first_name).bind(&r.last_name).bind(&r.gender).bind(email).bind(&r.phone).bind(r.foc_id)
        .fetch_one(pool).await
}
