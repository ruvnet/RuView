//! Dedicated TLS authority: never infer trust from headers or LAN reachability.
use super::*;
use axum::{
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::Response,
};
use wifi_densepose_sensing_server::{
    bearer_auth::AuthState,
    companion::{self, Companion, Grant},
    companion_tls,
};

pub struct Integration {
    service: Arc<Companion>,
    auth: AuthState,
    listener: tokio::net::TcpListener,
    tls: Arc<rustls::ServerConfig>,
    room: Arc<companion_room::SharedRoomStore>,
    control: Control,
    advertiser: Option<wifi_densepose_sensing_server::discovery::DiscoveryAdvertiser>,
}
impl Integration {
    pub async fn prepare(
        args: &Args,
        state: &SharedState,
        auth: AuthState,
    ) -> std::io::Result<Option<Self>> {
        let Some(port) = args.companion_port else {
            return Ok(None);
        };
        if port == 0
            || port == args.http_port
            || port == args.ws_port
            || !auth.static_token_enabled()
        {
            return Err(std::io::Error::other(
                "companion requires a dedicated port and owner credential",
            ));
        }
        let dir = args.data_dir.join("companion");
        let identity =
            companion_tls::Identity::load_or_create(&dir, args.installation_id.as_deref())?;
        let hostname =
            wifi_densepose_sensing_server::discovery::discovery_hostname(&identity.installation_id);
        let origin = format!("https://{}:{port}", hostname.trim_end_matches('.'));
        let service = Companion::load(
            &dir,
            identity.installation_id.clone(),
            identity.cert_sha256,
            origin,
        )?;
        state.write().await.installation_id = Some(identity.installation_id.clone());
        let ip: std::net::IpAddr = args.bind_addr.parse().map_err(std::io::Error::other)?;
        let listener = tokio::net::TcpListener::bind(SocketAddr::from((ip, port))).await?;
        let advertiser = if args.no_mdns || ip.is_loopback() {
            None
        } else {
            Some(
                wifi_densepose_sensing_server::discovery::start_tls_advertiser(
                    &format!("{} Companion", args.installation_name),
                    &identity.installation_id,
                    &hostname,
                    port,
                )
                .map_err(std::io::Error::other)?,
            )
        };
        let room = companion_room::SharedRoomStore::load(&dir, &identity.installation_id)?;
        let control = Control {
            state: state.clone(),
            serial: Arc::new(tokio::sync::Mutex::new(())),
        };
        Ok(Some(Self {
            service,
            auth,
            listener,
            tls: identity.config,
            advertiser,
            room,
            control,
        }))
    }
    pub fn owner_router(&self) -> Router {
        companion::owner_router(self.service.clone())
            .merge(companion_room::owner_router(self.room.clone()))
            .layer(axum::middleware::from_fn_with_state(
                self.control.clone(),
                serialize_control,
            ))
            .layer(axum::middleware::from_fn_with_state(
                self.auth.clone(),
                require_owner,
            ))
    }
    pub fn protect_control(&self, app: Router) -> Router {
        app.layer(Extension(self.room.clone()))
            .layer(axum::middleware::from_fn_with_state(
                self.control.clone(),
                serialize_control,
            ))
    }
    pub async fn start(self, app: Router) {
        let app = app
            .merge(companion_room::read_router(self.room.clone()))
            .layer(axum::middleware::from_fn_with_state(
                self.service.clone(),
                require_companion,
            ))
            .merge(companion::public_router(self.service))
            .layer(axum::extract::DefaultBodyLimit::max(64 * 1024));
        tokio::spawn(async move {
            let _advertiser = self.advertiser;
            if let Err(error) = companion_tls::serve(self.listener, self.tls, app).await {
                error!("Companion listener stopped: {error}");
            }
        });
    }
}
#[derive(Clone)]
struct Control {
    state: SharedState,
    serial: Arc<tokio::sync::Mutex<()>>,
}
async fn serialize_control(
    State(control): State<Control>,
    request: Request,
    next: Next,
) -> Response {
    let publish = request.method() == axum::http::Method::PUT
        && request.uri().path() == "/api/v1/companion/room";
    let start = request.method() == axum::http::Method::POST
        && matches!(
            request.uri().path(),
            "/api/v1/calibration/start" | "/api/v1/pose/calibrate"
        );
    if !publish && !start {
        return next.run(request).await;
    }
    let _guard = control.serial.lock().await;
    if publish {
        let state = control.state.read().await;
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        if state.field_model_status_at(now).is_some_and(|status| {
            matches!(
                status,
                CalibrationStatus::Collecting | CalibrationStatus::Uncalibrated
            )
        }) {
            return (
                StatusCode::CONFLICT,
                "Calibration is active; stop it before publishing another room revision",
            )
                .into_response();
        }
    }
    next.run(request).await
}

fn bearer(request: &Request) -> Option<&str> {
    let (scheme, token) = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    };
    Some(token)
}
async fn require_owner(State(auth): State<AuthState>, request: Request, next: Next) -> Response {
    let local = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|p| p.0.ip().is_loopback());
    if !local || !bearer(&request).is_some_and(|t| auth.is_owner_token(t)) {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}
fn policy(method: &axum::http::Method, path: &str) -> Option<bool> {
    use axum::http::Method;
    if *method == Method::GET || *method == Method::HEAD {
        if matches!(
            path,
            "/health"
                | "/api/v1/status"
                | "/api/v1/info"
                | "/api/v1/sensing/latest"
                | "/api/v1/nodes"
                | "/api/v1/stream/status"
                | "/api/v1/vital-signs"
                | "/api/v1/edge-vitals"
                | "/api/v1/pose/current"
                | "/api/v1/pose/stats"
                | "/api/v1/pose/zones/summary"
                | "/api/v1/pose/activities"
                | "/api/v1/calibration/status"
                | "/api/v1/pose/calibration/status"
                | "/ws/sensing"
                | "/companion/v1/room"
        ) {
            return Some(false);
        }
    }
    if *method == Method::POST
        && matches!(
            path,
            "/api/v1/calibration/start"
                | "/api/v1/calibration/stop"
                | "/api/v1/calibration/cancel"
                | "/api/v1/calibration/bootstrap/promote"
                | "/api/v1/calibration/bootstrap/refine"
        )
    {
        return Some(true);
    }
    None
}
async fn require_companion(
    State(service): State<Arc<Companion>>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(calibration) = policy(request.method(), request.uri().path()) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    // Ticket and credential query parameters are never accepted on this listener.
    if request.uri().query().is_some_and(|q| {
        q.split('&').any(|part| {
            matches!(
                part.split('=').next(),
                Some("ticket" | "token" | "access_token")
            )
        })
    }) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let installation = request
        .headers()
        .get("x-ruview-installation-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let grant = match service.authorize(installation, bearer(&request).unwrap_or(""), calibration) {
        Ok(grant) => grant,
        Err(status) => return status.into_response(),
    };
    request.extensions_mut().insert(grant);
    next.run(request).await
}
pub async fn guard_stream(grant: Grant, stream: impl std::future::Future<Output = ()>) {
    let mut revoked = grant.revoked;
    if *revoked.borrow() {
        return;
    }
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    if grant.expires_at <= now {
        return;
    }
    tokio::select! {
        _ = stream => {},
        _ = revoked.changed() => {},
        _ = tokio::time::sleep(Duration::from_secs(grant.expires_at-now)) => {},
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_policy_is_not_the_legacy_read_policy() {
        use axum::http::Method;
        assert_eq!(policy(&Method::GET, "/api/v1/sensing/latest"), Some(false));
        assert_eq!(
            policy(&Method::POST, "/api/v1/calibration/start"),
            Some(true)
        );
        for path in [
            "/api/v1/ws-ticket",
            "/api/v1/models/load",
            "/api/v1/recording/start",
            "/api/v1/calibration/reset",
            "/api/v1/companion/invites",
        ] {
            assert_eq!(policy(&Method::POST, path), None, "{path}");
        }
        assert_eq!(policy(&Method::GET, "/ws/train/progress"), None);
    }
}
