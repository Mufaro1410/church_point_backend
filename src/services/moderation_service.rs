//! Reporting, blocking and the language filter for member posts.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, PrayerError, TestimonyError},
    middleware::authentication::AuthUser,
    models::moderation::{ActionRequest, BlockRequest, ReportRequest},
    repositories::moderation_repository as repo,
    state::SharedState,
};

/// Words that are never acceptable in a prayer or testimony. Matched as whole words, ignoring case and
/// common letter swaps (@ for a, 0 for o, 1 for i, $ for s). Church admins can still remove anything the filter misses.
const BLOCKED: &[&str] = &[
    "fuck", "fucking", "fucker", "motherfucker", "shit", "bullshit", "bitch", "bastard", "asshole", "dickhead",
    "cunt", "slut", "whore", "pussy", "nigger", "nigga", "faggot", "retard", "porn", "pornography",
];

fn normalise(c: char) -> char {
    match c { '@' | '4' => 'a', '0' => 'o', '1' | '!' | '|' => 'i', '3' => 'e', '$' | '5' => 's', _ => c.to_ascii_lowercase() }
}

/// Rejects text containing blocked words. Call it on every field a member can write.
pub fn check_text(fields: &[&str]) -> AppResult<()> {
    for f in fields {
        let cleaned: String = f.chars().map(normalise).collect();
        // collapse "f u c k" and "f.u.c.k" style spacing into one run before matching
        let squashed: String = cleaned.chars().filter(|c| c.is_alphabetic()).collect();
        let hit = cleaned
            .split(|c: char| !c.is_alphabetic())
            .any(|w| BLOCKED.contains(&w))
            || BLOCKED.iter().any(|b| b.len() >= 4 && squashed.contains(b) && cleaned.split(|c: char| !c.is_alphabetic()).filter(|w| w.len() == 1).count() >= b.len() - 1);
        if hit {
            return Err(AppError::Validation(
                "Your post contains language that is not allowed in this community. Please change it and try again.".into(),
            ));
        }
    }
    Ok(())
}

fn kind_ok(kind: &str) -> AppResult<&str> {
    match kind {
        "prayer" | "testimony" => Ok(kind),
        _ => Err(AppError::Validation("kind must be prayer or testimony.".into())),
    }
}

fn not_found(kind: &str) -> AppError {
    if kind == "prayer" { PrayerError::NotFound.into() } else { TestimonyError::NotFound.into() }
}

pub async fn report(state: &SharedState, auth: &AuthUser, r: ReportRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let kind = kind_ok(&r.kind)?;
    let reason = r.reason.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if reason.is_some_and(|s| s.chars().count() > 500) {
        return Err(AppError::Validation("Reason is too long (max 500 characters).".into()));
    }
    let (author, _, excerpt) = repo::content_info(&state.db, kind, r.content_id, foc).await?.ok_or_else(|| not_found(kind))?;
    if author == Some(auth.member_id) {
        return Err(AppError::Validation("This is your own post. You can delete it instead.".into()));
    }
    repo::insert_report(&state.db, foc, auth.member_id, kind, r.content_id, author, &excerpt, reason).await?;
    Ok(json!({ "ok": true }))
}

pub async fn block(state: &SharedState, auth: &AuthUser, r: BlockRequest) -> AppResult<Value> {
    let foc = auth.require_foc()?;
    let (target, anonymous) = match (&r.kind, r.content_id, r.member_id) {
        (Some(k), Some(id), _) => {
            let kind = kind_ok(k)?;
            let (author, anon, _) = repo::content_info(&state.db, kind, id, foc).await?.ok_or_else(|| not_found(kind))?;
            (author.ok_or_else(|| AppError::Validation("This author can no longer be blocked.".into()))?, anon)
        }
        (_, _, Some(m)) => (m, false),
        _ => return Err(AppError::Validation("Say whose posts to hide: a post or a member.".into())),
    };
    if target == auth.member_id {
        return Err(AppError::Validation("You cannot block yourself.".into()));
    }
    repo::insert_block(&state.db, auth.member_id, target, anonymous).await?;
    Ok(json!({ "ok": true }))
}

pub async fn blocks(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::blocks(&state.db, auth.member_id).await?)
}

pub async fn unblock(state: &SharedState, auth: &AuthUser, id: Uuid) -> AppResult<()> {
    repo::delete_block(&state.db, id, auth.member_id).await?;
    Ok(())
}

pub async fn accept_terms(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    repo::accept_terms(&state.db, auth.member_id).await?;
    Ok(json!({ "ok": true }))
}

// ── for church admins ──

pub async fn open_reports(state: &SharedState, auth: &AuthUser) -> AppResult<Vec<Value>> {
    Ok(repo::open_reports(&state.db, auth.require_foc()?).await?)
}

pub async fn open_count(state: &SharedState, auth: &AuthUser) -> AppResult<Value> {
    Ok(json!({ "open": repo::open_count(&state.db, auth.require_foc()?).await? }))
}

pub async fn act(state: &SharedState, auth: &AuthUser, kind: &str, id: Uuid, r: ActionRequest) -> AppResult<()> {
    let foc = auth.require_foc()?;
    let kind = kind_ok(kind)?;
    if !["dismiss", "remove", "remove_and_eject"].contains(&r.action.as_str()) {
        return Err(AppError::Validation("action must be dismiss, remove or remove_and_eject.".into()));
    }
    if !repo::act(&state.db, foc, kind, id, &r.action, auth.member_id).await? {
        return Err(not_found(kind));
    }
    state.clear_auth_cache();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_text;

    #[test]
    fn blocks_plain_and_disguised_words() {
        assert!(check_text(&["you are a b1tch"]).is_err());
        assert!(check_text(&["what the f.u.c.k"]).is_err());
        assert!(check_text(&["SHIT happens"]).is_err());
    }

    #[test]
    fn lets_ordinary_prayers_through() {
        assert!(check_text(&["Please pray for my mother's recovery", "Assistance with school fees for the class of 2027"]).is_ok());
        assert!(check_text(&["Pray for a Scunthorpe-style place name: Cassidy and Classic"]).is_ok());
    }
}
