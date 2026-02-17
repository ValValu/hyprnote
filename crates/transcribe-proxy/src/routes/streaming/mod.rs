mod common;
mod hyprnote;
mod passthrough;
mod session;

#[cfg(test)]
mod tests;

use axum::{
    extract::{FromRequestParts, State, WebSocketUpgrade},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};

use crate::hyprnote_routing::should_use_hyprnote_routing;
use crate::query_params::QueryParams;

use super::AppState;
use common::ProxyBuildError;

use hypr_analytics::{AuthenticatedUserId, DeviceFingerprint};

pub struct AnalyticsContext {
    pub fingerprint: Option<String>,
    pub user_id: Option<String>,
}

impl<S> FromRequestParts<S> for AnalyticsContext
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let fingerprint = parts
            .extensions
            .get::<DeviceFingerprint>()
            .map(|id| id.0.clone());
        let user_id = parts
            .extensions
            .get::<AuthenticatedUserId>()
            .map(|id| id.0.clone());
        Ok(AnalyticsContext {
            fingerprint,
            user_id,
        })
    }
}

pub async fn handler(
    State(state): State<AppState>,
    analytics_ctx: AnalyticsContext,
    ws: WebSocketUpgrade,
    mut params: QueryParams,
) -> Response {
    let is_hyprnote_routing = should_use_hyprnote_routing(params.get_first("provider"));

    let selected = match state.resolve_provider(&mut params) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let proxy = if is_hyprnote_routing {
        hyprnote::build_proxy(&state, &selected, &params, analytics_ctx).await
    } else {
        passthrough::build_proxy(&state, &selected, &params, analytics_ctx).await
    };

    let proxy = match proxy {
        Ok(p) => p,
        Err(ProxyBuildError::SessionInitFailed(e)) => {
            tracing::error!(
                error = %e,
                provider = ?selected.provider(),
                "session_init_failed"
            );
            return (StatusCode::BAD_GATEWAY, e).into_response();
        }
        Err(ProxyBuildError::ProxyError(e)) => {
            tracing::error!(
                error = ?e,
                provider = ?selected.provider(),
                "proxy_build_failed"
            );
            return (StatusCode::BAD_REQUEST, format!("{}", e)).into_response();
        }
    };

    proxy.handle_upgrade(ws).await.into_response()
}
