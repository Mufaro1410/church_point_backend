use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult, AuthError},
    middleware::authentication::AuthUser,
    models::scope::Scope,
};

/// The tenant columns the SERVER decides for a new event/announcement.
/// They mirror the database's `*_scope_check` constraints, so a bad request fails here
/// with a clear message instead of a generic constraint error.
#[derive(Debug, PartialEq, Eq)]
pub struct Ownership {
    pub scope: &'static str,
    pub church_id: Uuid,
    pub province_id: Option<Uuid>,
    pub foc_id: Option<Uuid>,
    /// Which FOC created it. Church/province rows have no `foc_id`, so this is how we
    /// know who may edit them later.
    pub created_by_foc_id: Uuid,
}

pub fn ownership(scope: Scope, auth: &AuthUser) -> AppResult<Ownership> {
    let foc = auth.require_foc()?;
    let church_id = auth.church_id.ok_or(AuthError::NoFoc)?;

    let (province_id, foc_id) = match scope {
        Scope::Church => (None, None),
        Scope::Province => (
            Some(auth.province_id.ok_or_else(|| {
                AppError::Validation(
                    "Your FOC has no assigned province, so you cannot create a province-wide record."
                        .into(),
                )
            })?),
            None,
        ),
        Scope::Foc => (auth.province_id, Some(foc)),
    };
    Ok(Ownership { scope: scope.as_str(), church_id, province_id, foc_id, created_by_foc_id: foc })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(province: bool) -> AuthUser {
        AuthUser {
            user_id: Uuid::new_v4(),
            member_id: Uuid::new_v4(),
            foc_id: Some(Uuid::new_v4()),
            church_id: Some(Uuid::new_v4()),
            province_id: province.then(Uuid::new_v4),
            foc_status: Some("approved".into()),
            role: Some("admin".into()),
        }
    }

    #[test]
    fn church_scope_has_no_province_or_foc() {
        let u = user(true);
        let o = ownership(Scope::Church, &u).unwrap();
        assert_eq!((o.province_id, o.foc_id), (None, None));
        assert_eq!(Some(o.created_by_foc_id), u.foc_id);
    }

    #[test]
    fn province_scope_needs_a_province() {
        assert!(ownership(Scope::Province, &user(false)).is_err());
        let u = user(true);
        let o = ownership(Scope::Province, &u).unwrap();
        assert_eq!(o.province_id, u.province_id);
        assert_eq!(o.foc_id, None);
    }

    #[test]
    fn foc_scope_belongs_to_the_callers_foc() {
        let u = user(false);
        let o = ownership(Scope::Foc, &u).unwrap();
        assert_eq!(o.foc_id, u.foc_id);
    }
}
