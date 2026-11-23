use chrono::NaiveDate;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

type R<T> = Result<T, sqlx::Error>;

/// One row per member who gave in the period, with totals kept separate per currency.
pub async fn summary(pool: &PgPool, foc: Uuid, from: NaiveDate, to: NaiveDate) -> R<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('member_id', t.member_id, 'name', t.name, 'gifts', sum(t.n)::int,
                 'totals', jsonb_object_agg(t.currency, t.total::text))
         FROM (SELECT i.member_id, btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')) AS name,
                      cu.name AS currency, count(*) AS n, sum(i.amount)::numeric(14,2) AS total
               FROM incomes i JOIN member_profiles m ON m.id = i.member_id JOIN currencies cu ON cu.id = i.currency_id
               WHERE i.foc_id = $1 AND i.voided_at IS NULL AND i.date BETWEEN $2 AND $3
               GROUP BY i.member_id, m.first_name, m.last_name, cu.name) t
         GROUP BY t.member_id, t.name ORDER BY t.name")
        .bind(foc).bind(from).bind(to).fetch_all(pool).await
}

/// A member's statement: church and member details, every gift, and totals per currency and per fund.
pub async fn statement(pool: &PgPool, foc: Uuid, member: Uuid, from: NaiveDate, to: NaiveDate) -> R<Option<Value>> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'church', (SELECT jsonb_build_object('name', f.name, 'address', f.address, 'contact', f.contact, 'email', f.email, 'logo', f.logo)
                       FROM foc_profile f WHERE f.id = $1),
            'member', jsonb_build_object('id', m.id, 'name', btrim(coalesce(m.first_name,'') || ' ' || coalesce(m.last_name,'')), 'email', m.email, 'phone', m.phone),
            'from', $3::date, 'to', $4::date,
            'gifts', coalesce((SELECT jsonb_agg(jsonb_build_object('id', i.id, 'date', i.date, 'fund', c.name, 'method', me.name,
                                  'currency', cu.name, 'amount', i.amount::text, 'description', i.description) ORDER BY i.date, i.created_at)
                               FROM incomes i JOIN categories c ON c.id = i.category_id JOIN methods me ON me.id = i.method_id
                               JOIN currencies cu ON cu.id = i.currency_id
                               WHERE i.foc_id = $1 AND i.member_id = m.id AND i.voided_at IS NULL AND i.date BETWEEN $3 AND $4), '[]'::jsonb),
            'totals', coalesce((SELECT jsonb_object_agg(x.currency, x.total::text) FROM (
                               SELECT cu.name AS currency, sum(i.amount)::numeric(14,2) AS total FROM incomes i JOIN currencies cu ON cu.id = i.currency_id
                               WHERE i.foc_id = $1 AND i.member_id = m.id AND i.voided_at IS NULL AND i.date BETWEEN $3 AND $4 GROUP BY cu.name) x), '{}'::jsonb),
            'by_fund', coalesce((SELECT jsonb_agg(jsonb_build_object('fund', y.fund, 'currency', y.currency, 'total', y.total::text) ORDER BY y.fund, y.currency) FROM (
                               SELECT c.name AS fund, cu.name AS currency, sum(i.amount)::numeric(14,2) AS total FROM incomes i
                               JOIN categories c ON c.id = i.category_id JOIN currencies cu ON cu.id = i.currency_id
                               WHERE i.foc_id = $1 AND i.member_id = m.id AND i.voided_at IS NULL AND i.date BETWEEN $3 AND $4 GROUP BY c.name, cu.name) y), '[]'::jsonb))
         FROM member_profiles m WHERE m.id = $2 AND m.foc_id = $1")
        .bind(foc).bind(member).bind(from).bind(to).fetch_optional(pool).await
}
