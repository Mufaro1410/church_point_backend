//! Login, refresh, logout, sign-up. Axum calls Supabase Auth on the user's behalf, so the clients
//! only ever talk to Axum.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, AuthError, OrgError},
    models::auth::{ForgotRequest, LoginRequest, RefreshRequest, RegisterRequest},
    models::catalog::RegisterFoc,
    repositories::{catalog_repository, user_repository},
    services::catalog_service,
    state::SharedState,
    utils::validation::format_line,
};

fn email(raw: &str) -> AppResult<String> {
    let e = format_line(raw).to_lowercase();
    let ok = e.len() <= 254 && e.split_once('@').is_some_and(|(l, d)| !l.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.'));
    if ok { Ok(e) } else { Err(AppError::Validation("Enter a valid email address.".into())) }
}

fn password(p: &str) -> AppResult<()> {
    if p.chars().count() < 8 || p.len() > 72 {
        return Err(AppError::Validation("Password must be 8 to 72 characters.".into()));
    }
    Ok(())
}

fn session_json(raw: &Value) -> Value {
    json!({
        "access_token": raw["access_token"],
        "refresh_token": raw["refresh_token"],
        "expires_in": raw["expires_in"],
        "expires_at": raw["expires_at"],
        "token_type": "bearer",
    })
}

/// Admins can do everything in their church, so the app is told every permission; others get what their roles give.
fn effective_permissions(role: &Option<String>, own: Vec<String>) -> Vec<String> {
    if matches!(role.as_deref(), Some("admin" | "super_admin")) {
        crate::models::rbac::AREAS.iter().flat_map(|a| {
            let acts: &[&str] = if a.delete { &["view", "manage", "delete"] } else { &["view", "manage"] };
            acts.iter().map(move |x| format!("{}.{x}", a.key))
        }).collect()
    } else {
        own
    }
}

async fn me(state: &SharedState, user_id: Uuid) -> AppResult<Value> {
    Ok(match user_repository::find_auth_profile(&state.db, user_id).await? {
        Some(p) => json!({
            "member_id": p.member_id, "foc_id": p.foc_id, "church_id": p.church_id,
            "province_id": p.province_id, "role": p.role, "foc_status": p.foc_status,
            "member_status": p.member_status, "permissions": effective_permissions(&p.role, p.permissions),
        }),
        None => Value::Null,
    })
}

fn user_id(raw: &Value) -> AppResult<Uuid> {
    raw["user"]["id"].as_str().and_then(|s| s.parse().ok()).ok_or_else(|| AppError::Upstream("Auth gave no user id.".into()))
}

/// A profile an admin made for this person (same email, no login yet) becomes theirs the first time they
/// sign in with a VERIFIED email. Nothing is linked while the address is unconfirmed.
async fn claim_if_verified(state: &SharedState, raw: &Value, id: Uuid) -> AppResult<()> {
    let verified = !raw["user"]["email_confirmed_at"].is_null();
    let email = raw["user"]["email"].as_str().unwrap_or("");
    if !verified || email.is_empty() || user_repository::find_auth_profile(&state.db, id).await?.is_some() {
        return Ok(());
    }
    if let Some(member) = crate::repositories::mobile_repository::unclaimed_by_email(&state.db, email).await? {
        if crate::repositories::mobile_repository::claim_profile(&state.db, member, id).await? > 0 {
            state.clear_auth_cache();
        }
    }
    Ok(())
}

async fn session_reply(state: &SharedState, raw: Value) -> AppResult<Value> {
    let id = user_id(&raw)?;
    claim_if_verified(state, &raw, id).await?;
    Ok(json!({
        "session": session_json(&raw),
        "user": { "id": id, "email": raw["user"]["email"] },
        "me": me(state, id).await?,
    }))
}

/// A church member signing up from the mobile app. The profile starts "pending" until a church admin approves it.
pub async fn register_member(state: &SharedState, r: crate::models::mobile::RegisterMemberRequest) -> AppResult<Value> {
    let e = email(&r.email)?;
    password(&r.password)?;
    if !r.accepted_terms {
        return Err(AppError::Validation("You must accept the Terms & Conditions.".into()));
    }
    // An admin may already have added this person: then this is the login for that profile, not a new member.
    let claiming = crate::repositories::mobile_repository::unclaimed_by_email(&state.db, &e).await?.is_some();
    if !claiming && catalog_repository::email_taken(&state.db, &e).await? {
        return Err(OrgError::Duplicate("An account with this email").into());
    }
    let meta = json!({
        "first_name": r.first_name.trim(), "last_name": r.last_name.trim(),
        "display_name": format!("{} {}", r.first_name.trim(), r.last_name.trim()),
    });
    let raw = state.supabase.sign_up(&e, &r.password, meta).await?;
    let user = raw.get("user").unwrap_or(&raw);
    let id: Uuid = user["id"].as_str().and_then(|s| s.parse().ok()).ok_or_else(|| AppError::Upstream("Sign-up gave no user id.".into()))?;
    if user["identities"].as_array().is_some_and(|i| i.is_empty()) {
        return Err(OrgError::Duplicate("An account with this email").into());
    }
    if claiming {
        // Verify the email first; the profile is linked at the first sign-in after that.
        return Ok(json!({ "registered": true, "status": "claim", "verify_email": true }));
    }
    // The database function checks every field again and creates the pending profile.
    let outcome = crate::repositories::mobile_repository::register_profile(&state.db, id, &r, &e).await;
    let problem = match &outcome {
        Ok(v) if v["error"].is_null() => None,
        Ok(v) => Some(AppError::Validation(v["error"].as_str().unwrap_or("Registration failed.").to_string())),
        Err(_) => Some(AppError::Internal("registration failed".into())),
    };
    if let Some(err) = problem {
        if let Err(e2) = state.supabase.delete_user(id).await {
            tracing::error!(error = ?e2, user = %id, "could not remove login after failed registration");
        }
        if let Err(e) = outcome { tracing::error!(error = ?e, "member registration failed"); }
        return Err(err);
    }
    Ok(json!({ "registered": true, "status": "pending" }))
}

pub async fn login(state: &SharedState, r: LoginRequest) -> AppResult<Value> {
    let e = email(&r.email).map_err(|_| AuthError::InvalidCredentials)?;
    if r.password.is_empty() {
        return Err(AuthError::InvalidCredentials.into());
    }
    session_reply(state, state.supabase.login(&e, &r.password).await?).await
}

pub async fn refresh(state: &SharedState, r: RefreshRequest) -> AppResult<Value> {
    session_reply(state, state.supabase.refresh(&r.refresh_token).await?).await
}

pub async fn change_password(state: &SharedState, access_token: &str, new_password: &str) -> AppResult<()> {
    password(new_password)?;
    state.supabase.update_password(access_token, new_password).await
}

pub async fn logout(state: &SharedState, access_token: &str) -> AppResult<()> {
    state.supabase.logout(access_token).await
}

/// Always succeeds from the caller's view: never reveals whether an email has an account.
pub async fn forgot_password(state: &SharedState, r: ForgotRequest) -> AppResult<()> {
    if let Ok(e) = email(&r.email) {
        state.supabase.recover(&e, state.settings.password_reset_redirect.as_deref()).await?;
    }
    Ok(())
}

/// Create the login AND the FOC + admin profile together. Everything is checked before the
/// login is created, and a failed profile insert removes the login again (no ghost accounts).
pub async fn register(state: &SharedState, r: RegisterRequest) -> AppResult<Value> {
    let e = email(&r.email)?;
    password(&r.password)?;
    let mut user_data = r.user_data;
    user_data.email = e.clone();
    let (cleaned, dob) = catalog_service::clean_register(RegisterFoc { church_data: r.church_data, user_data })?;
    if !catalog_repository::church_exists(&state.db, cleaned.church_data.church_id).await? {
        return Err(AppError::Validation("church_id does not match a church.".into()));
    }
    if catalog_repository::email_taken(&state.db, &e).await? {
        return Err(OrgError::Duplicate("An account with this email").into());
    }

    let meta = json!({
        "first_name": cleaned.user_data.first_name,
        "last_name": cleaned.user_data.last_name,
        "display_name": format!("{} {}", cleaned.user_data.first_name, cleaned.user_data.last_name),
    });
    let raw = state.supabase.sign_up(&e, &r.password, meta).await?;
    // Supabase hides "already registered" by returning a user with no identities.
    let user = raw.get("user").unwrap_or(&raw);
    let id: Uuid = user["id"].as_str().and_then(|s| s.parse().ok()).ok_or_else(|| AppError::Upstream("Sign-up gave no user id.".into()))?;
    if user["identities"].as_array().is_some_and(|i| i.is_empty()) {
        return Err(OrgError::Duplicate("An account with this email").into());
    }

    let foc = match catalog_service::register_cleaned(state, id, &cleaned, dob.as_deref()).await {
        Ok(v) => v,
        Err(err) => {
            if let Err(e2) = state.supabase.delete_user(id).await {
                tracing::error!(error = ?e2, user = %id, "could not remove login after failed registration");
            }
            return Err(err);
        }
    };
    let session = if raw.get("access_token").is_some() { session_json(&raw) } else { Value::Null };
    Ok(json!({
        "foc": foc,
        "session": session,
        "confirmation_required": session.is_null(),
    }))
}
