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
    companion_tls, companion_sensors,
};

// Share the QR authority with Host validation; DNS-SD names retain their terminal dot.
fn companion_authority(identity: &companion_tls::Identity, port: u16) -> String {
    let hostname =
        wifi_densepose_sensing_server::discovery::discovery_hostname(&identity.installation_id);
    format!("{}:{port}", hostname.trim_end_matches('.'))
}

pub fn host_allowlist(
    args: &Args,
    companion_authority: Option<&str>,
) -> wifi_densepose_sensing_server::host_validation::HostAllowlist {
    let discovery_label = args.installation_id.as_deref().unwrap_or("installation");
    let discovery_host =
        wifi_densepose_sensing_server::discovery::discovery_hostname(discovery_label);
    wifi_densepose_sensing_server::host_validation::HostAllowlist::from_cli_and_env(
        args.allowed_hosts
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(discovery_host.as_str()))
            .chain(companion_authority),
    )
}
pub struct Integration {
    service: Arc<Companion>,
    authority: String,
    auth: AuthState,
    listener: tokio::net::TcpListener,
    tls: Arc<rustls::ServerConfig>,
    room: Arc<companion_room::SharedRoomStore>,
    control: Control,
    sensors: companion_sensors::Shared,
    spatial: Option<Arc<companion_spatial::SpatialStore>>,
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
        let authority = companion_authority(&identity, port);
        let origin = format!("https://{authority}");
        let service = Companion::load_for_account(
            &dir,
            identity.installation_id.clone(),
            identity.cert_sha256,
            origin,
            auth.owner_account_scope().map(str::to_owned),
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
        let sensors = companion_sensors::SensorStore::new(service.clone());
        let spatial = auth.owner_account_scope().map(|scope| companion_spatial::SpatialStore::load(&dir, scope, &identity.installation_id, service.clone(), room.clone())).transpose()?;
        Ok(Some(Self {
            service,
            authority,
            auth,
            listener,
            tls: identity.config,
            advertiser,
            room,
            control,
            sensors,
            spatial,
        }))
    }
    pub fn authority(&self) -> &str {
        &self.authority
    }
    pub fn owner_router(&self) -> Router {
        companion::owner_router(self.service.clone())
            .merge(companion_sensors::owner_router(self.sensors.clone()))
            .merge(companion_room::owner_router(self.room.clone()))
            .merge(self.spatial.as_ref().map(|s| companion_spatial::owner_router(s.clone())).unwrap_or_default())
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
        let app = attach_peer_artifacts(
            app.merge(companion_sensors::companion_router(self.sensors.clone()))
                .merge(companion_room::read_router(self.room.clone())),
            self.spatial.as_ref().map(|s| companion_spatial::peer_router(s.clone())).unwrap_or_default(),
            self.control.clone(),
        )
            .layer(axum::middleware::from_fn_with_state(
                self.service.clone(),
                require_companion,
            ))
            .merge(companion::public_router(self.service))
            .layer(axum::middleware::from_fn_with_state(self.auth, companion::require_account_ownership))
            .layer(axum::extract::DefaultBodyLimit::max(64 * 1024));
        tokio::spawn(async move {
            let _advertiser = self.advertiser;
            if let Err(error) = companion_tls::serve(self.listener, self.tls, app).await {
                error!("Companion listener stopped: {error}");
            }
        });
    }
}
// Existing calibration routes already use protect_control. Layering their
// router a second time would reacquire the same non-reentrant mutex.
fn attach_peer_artifacts(app: Router, artifacts: Router, control: Control) -> Router {
    app.merge(artifacts.layer(axum::middleware::from_fn_with_state(control, serialize_control)))
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
    let artifact = request.method() == axum::http::Method::POST && matches!(request.uri().path(), "/api/v1/companion/spatial" | "/companion/v1/spatial");
    if !publish && !start && !artifact {
        return next.run(request).await;
    }
    // Do not let a permitted phone hold the shared calibration lock while
    // streaming an unfinished body. Bound and time-limit it before locking.
    let request = if artifact {
        let (parts, body) = request.into_parts();
        let bytes = match tokio::time::timeout(std::time::Duration::from_secs(5), axum::body::to_bytes(body, 64 * 1024)).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(_)) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
            Err(_) => return StatusCode::REQUEST_TIMEOUT.into_response(),
        };
        Request::from_parts(parts, axum::body::Body::from(bytes))
    } else { request };
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
                | "/companion/v1/sensors"
                | "/companion/v1/spatial"
        ) {
            return Some(false);
        }
    }
    if *method == Method::POST && path == "/companion/v1/spatial" { return Some(false); }
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
    let mut grant = match service.authorize(installation, bearer(&request).unwrap_or(""), calibration) {
        Ok(grant) => grant,
        Err(status) => return status.into_response(),
    };
    if let Some(companion::AccountOwnershipExpiry(expiry)) = request.extensions().get::<companion::AccountOwnershipExpiry>() {
        grant.expires_at = grant.expires_at.min(*expiry);
    }
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
    fn qr_authority_is_allowed_for_explicit_and_persisted_generated_identity() {
        for preferred in [Some("standalone-installation"), None] {
            let dir = tempfile::tempdir().unwrap();
            let identity = companion_tls::Identity::load_or_create(dir.path(), preferred).unwrap();
            let reloaded = companion_tls::Identity::load_or_create(dir.path(), preferred).unwrap();
            let mut argv = vec!["sensing-server"];
            if let Some(id) = preferred {
                argv.extend(["--installation-id", id]);
            }
            let args = Args::try_parse_from(argv).unwrap();
            assert!(args.allowed_hosts.is_empty());
            for prepared in [&identity, &reloaded] {
                let authority = companion_authority(prepared, 3443);
                let origin = format!("https://{authority}");
                let parsed: axum::http::Uri = origin.parse().unwrap();
                let allowlist = host_allowlist(&args, Some(&authority));
                assert!(allowlist.is_enabled());
                assert!(allowlist.is_allowed(parsed.authority().unwrap().as_str()));
                assert!(!allowlist.is_allowed("untrusted.example:3443"));
                assert!(!allowlist.is_allowed(&format!("{authority}.untrusted.example")));
                // Only the exact QR authority is added, without allowing arbitrary ports.
                assert!(!allowlist.is_allowed(&authority.replace(":3443", ":3444")));
            }
            assert_eq!(
                companion_authority(&identity, 3443),
                companion_authority(&reloaded, 3443)
            );
        }
    }

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
    #[tokio::test]
    async fn peer_composition_never_takes_the_calibration_lock_twice() {
        use tower::ServiceExt;
        use axum::body::Body;
        let control=Control{state:Arc::new(RwLock::new(AppStateInner::minimal())),serial:Arc::new(tokio::sync::Mutex::new(()))};
        let existing=Router::new().route("/api/v1/calibration/start",post(||async{StatusCode::OK}))
            .layer(axum::middleware::from_fn_with_state(control.clone(),serialize_control));
        let artifacts=Router::new().route("/companion/v1/spatial",post(||async{StatusCode::OK}));
        let app=attach_peer_artifacts(existing,artifacts,control);
        for path in ["/api/v1/calibration/start","/companion/v1/spatial"] {
            let request=Request::builder().method("POST").uri(path).body(Body::from("{}")).unwrap();
            let response=tokio::time::timeout(std::time::Duration::from_secs(1),app.clone().oneshot(request)).await.expect("nested calibration lock").unwrap();
            assert_eq!(response.status(),StatusCode::OK);
        }
    }
    #[tokio::test]
    async fn unfinished_spatial_upload_does_not_hold_calibration_lock() {
        use tower::ServiceExt;
        use axum::body::Body;
        let serial=Arc::new(tokio::sync::Mutex::new(()));
        let control=Control{state:Arc::new(RwLock::new(AppStateInner::minimal())),serial:serial.clone()};
        let router=Router::new().route("/companion/v1/spatial",post(||async{StatusCode::OK}))
            .layer(axum::middleware::from_fn_with_state(control,serialize_control));
        let stream=futures_util::stream::pending::<Result<String,std::io::Error>>();
        let request=Request::builder().method("POST").uri("/companion/v1/spatial").body(Body::from_stream(stream)).unwrap();
        let pending=tokio::spawn(router.clone().oneshot(request));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(serial.try_lock().is_ok());
        assert_eq!(pending.await.unwrap().unwrap().status(),StatusCode::REQUEST_TIMEOUT);
        let huge=Request::builder().method("POST").uri("/companion/v1/spatial").body(Body::from(vec![b'x';65537])).unwrap();
        assert_eq!(router.oneshot(huge).await.unwrap().status(),StatusCode::PAYLOAD_TOO_LARGE);
    }
    #[tokio::test]
    async fn sensor_routes_preserve_owner_peer_and_companion_isolation() {
        use tower::ServiceExt;
        use axum::{body::Body, http::{Request, Method}};
        let dir = tempfile::tempdir().unwrap();
        let service = Companion::load(dir.path(), "test".into(), "a".repeat(64), "https://test.local:3443".into()).unwrap();
        let sensors = companion_sensors::SensorStore::new(service.clone());
        let owner = companion_sensors::owner_router(sensors).layer(axum::middleware::from_fn_with_state(AuthState::from_token("owner"), require_owner));
        for (peer, credential, expected) in [("127.0.0.1:1", "owner", 200), ("127.0.0.1:1", "phone", 403), ("10.0.0.1:1", "owner", 403)] {
            let mut req=Request::builder().uri("/api/v1/companion/sensors").header("authorization",format!("Bearer {credential}")).body(Body::empty()).unwrap();
            req.extensions_mut().insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
            assert_eq!(owner.clone().oneshot(req).await.unwrap().status().as_u16(), expected);
        }
        assert_eq!(policy(&Method::GET,"/companion/v1/sensors"),Some(false));
        assert_eq!(policy(&Method::GET,"/companion/v1/spatial"),Some(false));
        assert_eq!(policy(&Method::POST,"/companion/v1/spatial"),Some(false));
        assert_eq!(policy(&Method::PUT,"/companion/v1/spatial"),None);
        assert_eq!(policy(&Method::GET,"/api/v1/companion/spatial"),None);
        assert_eq!(policy(&Method::POST,"/api/v1/companion/spatial"),None);
        for path in ["/api/v1/companion/sensors", "/api/v1/companion/devices/phone/sensors"] {
            assert_eq!(policy(&Method::GET,path),None); assert_eq!(policy(&Method::POST,path),None); assert_eq!(policy(&Method::DELETE,path),None);
        }
        let denied=Router::new().route("/companion/v1/sensors",get(||async {StatusCode::OK})).layer(axum::middleware::from_fn_with_state(service,require_companion));
        let response=denied.oneshot(Request::builder().uri("/companion/v1/sensors").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(),StatusCode::UNAUTHORIZED);
    }

}
