use auth::{AuthError, JwtClaims};
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use axum_extra::{
    extract::cookie::CookieJar,
    headers::{Authorization, authorization::Bearer},
    typed_header::TypedHeader,
};
use domain::Role;

use crate::state::AppState;

pub const AUTH_TOKEN_COOKIE: &str = "rw3p_token";
pub const AUTH_ROLE_COOKIE: &str = "rw3p_role";

#[derive(Debug, Clone)]
pub struct CurrentUser(pub JwtClaims);

impl CurrentUser {
    pub fn ensure_role(&self, required: Role) -> Result<(), StatusCode> {
        if self.has_role(required) {
            Ok(())
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }

    fn has_role(&self, required: Role) -> bool {
        match required {
            Role::Admin => self.0.role == Role::Admin,
            Role::Viewer => matches!(self.0.role, Role::Admin | Role::Viewer),
            Role::None => true,
        }
    }

    pub fn claims(&self) -> &JwtClaims {
        &self.0
    }
}

#[async_trait]
impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token_from_cookie = CookieJar::from_request_parts(parts, state)
            .await
            .ok()
            .and_then(|jar| {
                jar.get(AUTH_TOKEN_COOKIE)
                    .map(|cookie| cookie.value().to_owned())
            });

        let token = if let Some(token) = token_from_cookie {
            token
        } else {
            let TypedHeader(Authorization(bearer)) =
                TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                    .await
                    .map_err(|_| StatusCode::UNAUTHORIZED)?;
            bearer.token().to_string()
        };

        state
            .auth
            .validate_token(&token)
            .await
            .map(CurrentUser)
            .map_err(|err| match err {
                AuthError::InvalidToken => StatusCode::UNAUTHORIZED,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            })
    }
}
