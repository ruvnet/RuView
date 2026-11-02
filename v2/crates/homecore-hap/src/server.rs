//! Bounded HAP IP server with plaintext pairing and encrypted control sessions.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use httparse::Status;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex, Semaphore};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{timeout, Instant};

use crate::accessory::{HapAccessoryType, HapCharacteristic, HapCharacteristicValue};
use crate::bridge::{CharacteristicEvent, ExposedAccessory, HapBridge};
use crate::crypto::{RecordLayer, RECORD_TAG_BYTES};
use crate::error::HapError;
use crate::mdns::{HapServiceRecord, MdnsAdvertiser};
use crate::pair_setup::PairSetup;
use crate::pair_verify::PairVerify;
use crate::pairing::{ControllerPairing, PairingStore};
use crate::protocol::{
    encode_items, error_response as tlv_error_response, Tlv8, TLV_ERROR_AUTHENTICATION,
    TLV_ERROR_MAX_PEERS, TLV_ERROR_UNKNOWN, TLV_IDENTIFIER, TLV_METHOD, TLV_PERMISSIONS,
    TLV_PUBLIC_KEY, TLV_SEPARATOR, TLV_STATE,
};
use crate::session::Session;

const HAP_JSON: &str = "application/hap+json";
const HAP_TLV: &str = "application/pairing+tlv8";

/// Resource bounds for the HAP listener.
#[derive(Debug, Clone)]
pub struct HapServerConfig {
    pub bind_addr: SocketAddr,
    pub max_connections: usize,
    pub max_header_bytes: usize,
    pub max_body_bytes: usize,
    pub request_timeout: Duration,
    pub shutdown_timeout: Duration,
}

impl Default for HapServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([0, 0, 0, 0], 51826)),
            max_connections: 32,
            max_header_bytes: 16 * 1024,
            max_body_bytes: 64 * 1024,
            request_timeout: Duration::from_secs(10),
            shutdown_timeout: Duration::from_secs(5),
        }
    }
}

impl HapServerConfig {
    fn validate(&self) -> Result<(), HapError> {
        if self.max_connections == 0
            || self.max_header_bytes < 512
            || self.max_body_bytes == 0
            || self.request_timeout.is_zero()
            || self.shutdown_timeout.is_zero()
        {
            return Err(HapError::Server(
                "invalid zero or undersized server limit".into(),
            ));
        }
        Ok(())
    }
}

/// Running server handle. Dropping it aborts the listener; [`shutdown`] also
/// retracts mDNS and waits for connection tasks within the configured bound.
pub struct HapServerHandle {
    local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), HapError>>>,
    shutdown_timeout: Duration,
}

impl HapServerHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub async fn shutdown(mut self) -> Result<(), HapError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let Some(mut task) = self.task.take() else {
            return Ok(());
        };
        match timeout(self.shutdown_timeout, &mut task).await {
            Ok(result) => {
                result.map_err(|error| HapError::Server(format!("server task failed: {error}")))?
            }
            Err(_) => {
                task.abort();
                let _ = task.await;
                Err(HapError::Server(
                    "server shutdown timed out; task aborted".into(),
                ))
            }
        }
    }
}

impl Drop for HapServerHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

/// Start the bounded listener and advertise the actual bound port.
pub async fn start_server(
    config: HapServerConfig,
    bridge: HapBridge,
    pairings: Arc<PairingStore>,
    advertiser: Arc<dyn MdnsAdvertiser>,
) -> Result<HapServerHandle, HapError> {
    config.validate()?;
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|error| HapError::Server(format!("bind {}: {error}", config.bind_addr)))?;
    let local_addr = listener
        .local_addr()
        .map_err(|error| HapError::Server(format!("read local address: {error}")))?;

    let mut record = bridge.service_record.clone();
    record.port = local_addr.port();
    let persisted_id = pairings.accessory_id()?;
    if !record.device_id.eq_ignore_ascii_case(&persisted_id) {
        return Err(HapError::Server(format!(
            "mDNS device ID {} does not match persisted accessory identity {persisted_id}",
            record.device_id
        )));
    }
    record.paired = pairings.is_paired()?;
    advertiser.advertise(&record).await?;
    let discovery = Arc::new(DiscoveryState {
        advertiser,
        record: Mutex::new(record),
    });

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task_config = config.clone();
    let task = tokio::spawn(run_listener(
        listener,
        task_config,
        bridge,
        pairings,
        discovery,
        shutdown_rx,
    ));
    Ok(HapServerHandle {
        local_addr,
        shutdown: Some(shutdown_tx),
        task: Some(task),
        // The listener owns the configured drain window; the handle allows a
        // small scheduling/retraction margin before enforcing its outer abort.
        shutdown_timeout: config
            .shutdown_timeout
            .saturating_add(Duration::from_secs(1)),
    })
}

async fn run_listener(
    listener: TcpListener,
    config: HapServerConfig,
    bridge: HapBridge,
    pairings: Arc<PairingStore>,
    discovery: Arc<DiscoveryState>,
    mut shutdown: oneshot::Receiver<()>,
) -> Result<(), HapError> {
    let permits = Arc::new(Semaphore::new(config.max_connections));
    let mut connections = JoinSet::new();

    loop {
        let permit = tokio::select! {
            _ = &mut shutdown => break,
            permit = permits.clone().acquire_owned() => {
                permit.map_err(|_| HapError::Server("connection semaphore closed".into()))?
            }
        };
        let accepted = tokio::select! {
            _ = &mut shutdown => {
                drop(permit);
                break;
            }
            accepted = listener.accept() => accepted
        };
        match accepted {
            Ok((stream, peer)) => {
                let bridge = bridge.clone();
                let pairings = pairings.clone();
                let discovery = discovery.clone();
                let limits = config.clone();
                connections.spawn(async move {
                    let _permit = permit;
                    if let Err(error) =
                        serve_connection(stream, peer, limits, bridge, pairings, discovery).await
                    {
                        tracing::debug!(%peer, %error, "HAP connection closed");
                    }
                });
            }
            Err(error) => {
                tracing::warn!(%error, "HAP accept failed");
            }
        }
        while connections.try_join_next().is_some() {}
    }

    drop(listener);
    let deadline = Instant::now() + config.shutdown_timeout;
    while !connections.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || timeout(remaining, connections.join_next()).await.is_err() {
            connections.abort_all();
            while connections.join_next().await.is_some() {}
            break;
        }
    }
    discovery.retract().await
}

struct DiscoveryState {
    advertiser: Arc<dyn MdnsAdvertiser>,
    record: Mutex<HapServiceRecord>,
}

impl DiscoveryState {
    async fn set_paired(&self, paired: bool) -> Result<(), HapError> {
        let mut record = self.record.lock().await;
        if record.paired == paired {
            return Ok(());
        }
        self.advertiser.retract(&record.instance_name).await?;
        let mut next = record.clone();
        next.paired = paired;
        self.advertiser.advertise(&next).await?;
        *record = next;
        Ok(())
    }

    async fn retract(&self) -> Result<(), HapError> {
        let record = self.record.lock().await;
        self.advertiser.retract(&record.instance_name).await
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    _peer: SocketAddr,
    config: HapServerConfig,
    bridge: HapBridge,
    pairings: Arc<PairingStore>,
    discovery: Arc<DiscoveryState>,
) -> Result<(), HapError> {
    let mut buffer = ConnectionBuffer::default();
    let mut session = Session::new();
    let mut pair_setup = PairSetup::new(pairings.clone());
    let mut pair_verify = PairVerify::new(pairings.clone());
    let mut record_layer = None;
    let mut subscriptions = HashSet::new();
    let mut events = bridge.subscribe_events();
    let mut pairing_changes = pairings.subscribe_changes();

    loop {
        tokio::select! {
            request = timeout(
                config.request_timeout,
                read_request(&mut stream, &mut record_layer, &mut buffer, &config),
            ) => {
                let request = match request {
                    Ok(Ok(Some(request))) => request,
                    Ok(Ok(None)) => break,
                    Ok(Err(RequestReadError::Authentication)) => break,
                    Ok(Err(error)) => {
                        let response = error_response(&error);
                        write_response(&mut stream, record_layer.as_mut(), response).await?;
                        break;
                    }
                    Err(_) => {
                        write_response(
                            &mut stream,
                            record_layer.as_mut(),
                            Response::plain(408, b"request timeout".to_vec()),
                        ).await?;
                        break;
                    }
                };
                let close = request.connection_close;
                let dispatched = dispatch_request(
                    request,
                    &mut session,
                    (&mut pair_setup, &mut pair_verify),
                    &bridge,
                    &pairings,
                    &discovery,
                    &mut subscriptions,
                ).await;
                write_response(
                    &mut stream,
                    record_layer.as_mut(),
                    dispatched.response,
                ).await?;
                if record_layer.is_none() {
                    if let Some(keys) = session.take_session_keys() {
                        record_layer = Some(RecordLayer::accessory(keys));
                    }
                }
                if close || dispatched.close_after_response {
                    break;
                }
            }
            event = events.recv(), if session.state().is_authenticated() && !subscriptions.is_empty() => {
                match event {
                    Ok(event) => {
                        if let Some(payload) = event_payload(&bridge, &event, &subscriptions) {
                            let Some(records) = record_layer.as_mut() else {
                                break;
                            };
                            write_event(&mut stream, records, payload).await?;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // A lagged controller must resynchronize through GET /characteristics.
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            changed = pairing_changes.changed(), if session.state().is_authenticated() => {
                if changed.is_err() {
                    break;
                }
                let Some(controller_id) = session.controller_id() else {
                    break;
                };
                if pairings.get(controller_id)?.is_none() {
                    break;
                }
            }
        }
    }
    session.close();
    Ok(())
}

#[derive(Debug)]
struct Request {
    method: String,
    target: String,
    body: Vec<u8>,
    connection_close: bool,
}

#[derive(Default)]
struct ConnectionBuffer {
    bytes: Vec<u8>,
}

async fn read_request(
    stream: &mut TcpStream,
    record_layer: &mut Option<RecordLayer>,
    buffer: &mut ConnectionBuffer,
    config: &HapServerConfig,
) -> Result<Option<Request>, RequestReadError> {
    let header_end = loop {
        if let Some(position) = find_header_end(&buffer.bytes) {
            break position + 4;
        }
        if buffer.bytes.len() >= config.max_header_bytes {
            return Err(RequestReadError::HeadersTooLarge);
        }
        let chunk = read_transport_chunk(stream, record_layer).await?;
        if chunk.is_empty() {
            return if buffer.bytes.is_empty() {
                Ok(None)
            } else {
                Err(RequestReadError::Malformed("truncated HTTP headers"))
            };
        }
        buffer.bytes.extend_from_slice(&chunk);
    };
    if header_end > config.max_header_bytes {
        return Err(RequestReadError::HeadersTooLarge);
    }

    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut parsed = httparse::Request::new(&mut headers);
    match parsed.parse(&buffer.bytes[..header_end]) {
        Ok(Status::Complete(_)) => {}
        Ok(Status::Partial) => return Err(RequestReadError::Malformed("partial HTTP request")),
        Err(error) => return Err(RequestReadError::MalformedOwned(error.to_string())),
    }
    if parsed.version != Some(1) {
        return Err(RequestReadError::Malformed("HTTP/1.1 required"));
    }
    let method = parsed
        .method
        .ok_or(RequestReadError::Malformed("missing method"))?
        .to_owned();
    let target = parsed
        .path
        .ok_or(RequestReadError::Malformed("missing request target"))?
        .to_owned();
    if target.len() > 2048 || !target.starts_with('/') {
        return Err(RequestReadError::Malformed("invalid request target"));
    }

    let mut content_length = None;
    let mut connection_close = false;
    for header in parsed.headers.iter() {
        if header.name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(RequestReadError::Malformed(
                "Transfer-Encoding is unsupported",
            ));
        }
        if header.name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(RequestReadError::Malformed("duplicate Content-Length"));
            }
            let value = std::str::from_utf8(header.value)
                .map_err(|_| RequestReadError::Malformed("non-UTF8 Content-Length"))?;
            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| RequestReadError::Malformed("invalid Content-Length"))?,
            );
        }
        if header.name.eq_ignore_ascii_case("connection")
            && header.value.eq_ignore_ascii_case(b"close")
        {
            connection_close = true;
        }
    }
    let content_length = content_length.unwrap_or(0);
    if content_length > config.max_body_bytes {
        return Err(RequestReadError::BodyTooLarge);
    }
    let request_end = header_end
        .checked_add(content_length)
        .ok_or(RequestReadError::BodyTooLarge)?;
    while buffer.bytes.len() < request_end {
        let chunk = read_transport_chunk(stream, record_layer).await?;
        if chunk.is_empty() {
            return Err(RequestReadError::Malformed("truncated HTTP body"));
        }
        buffer.bytes.extend_from_slice(&chunk);
    }
    let body = buffer.bytes[header_end..request_end].to_vec();
    buffer.bytes.drain(..request_end);
    Ok(Some(Request {
        method,
        target,
        body,
        connection_close,
    }))
}

async fn read_transport_chunk(
    stream: &mut TcpStream,
    record_layer: &mut Option<RecordLayer>,
) -> Result<Vec<u8>, RequestReadError> {
    let Some(records) = record_layer.as_mut() else {
        let mut chunk = vec![0u8; 2048];
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(RequestReadError::Io)?;
        chunk.truncate(read);
        return Ok(chunk);
    };

    let mut length_bytes = [0u8; 2];
    let first = stream
        .read(&mut length_bytes[..1])
        .await
        .map_err(RequestReadError::Io)?;
    if first == 0 {
        return Ok(Vec::new());
    }
    stream
        .read_exact(&mut length_bytes[1..])
        .await
        .map_err(|_| RequestReadError::Authentication)?;
    let length = u16::from_le_bytes(length_bytes) as usize;
    if length > crate::crypto::MAX_RECORD_PLAINTEXT {
        return Err(RequestReadError::Authentication);
    }
    let mut encrypted = vec![0u8; length + RECORD_TAG_BYTES];
    stream
        .read_exact(&mut encrypted)
        .await
        .map_err(|_| RequestReadError::Authentication)?;
    records
        .decrypt(length_bytes, &encrypted)
        .map_err(|_| RequestReadError::Authentication)
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

#[derive(Debug)]
enum RequestReadError {
    Io(std::io::Error),
    Malformed(&'static str),
    MalformedOwned(String),
    HeadersTooLarge,
    BodyTooLarge,
    Authentication,
}

impl std::fmt::Display for RequestReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Malformed(message) => formatter.write_str(message),
            Self::MalformedOwned(message) => formatter.write_str(message),
            Self::HeadersTooLarge => formatter.write_str("HTTP headers too large"),
            Self::BodyTooLarge => formatter.write_str("HTTP body too large"),
            Self::Authentication => formatter.write_str("encrypted HAP record rejected"),
        }
    }
}

fn error_response(error: &RequestReadError) -> Response {
    match error {
        RequestReadError::HeadersTooLarge => Response::plain(431, error.to_string().into_bytes()),
        RequestReadError::BodyTooLarge => Response::plain(413, error.to_string().into_bytes()),
        _ => Response::plain(400, error.to_string().into_bytes()),
    }
}

struct Response {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl Response {
    fn plain(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body,
        }
    }

    fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            content_type: HAP_JSON,
            body: serde_json::to_vec(&value).expect("JSON value serialization cannot fail"),
        }
    }
}

struct DispatchResult {
    response: Response,
    close_after_response: bool,
}

impl DispatchResult {
    fn keep(response: Response) -> Self {
        Self {
            response,
            close_after_response: false,
        }
    }

    fn close(response: Response) -> Self {
        Self {
            response,
            close_after_response: true,
        }
    }
}

async fn dispatch_request(
    request: Request,
    session: &mut Session,
    pair_protocols: (&mut PairSetup, &mut PairVerify),
    bridge: &HapBridge,
    pairings: &Arc<PairingStore>,
    discovery: &DiscoveryState,
    subscriptions: &mut HashSet<(u64, u64)>,
) -> DispatchResult {
    let (pair_setup, pair_verify) = pair_protocols;
    match (
        request.method.as_str(),
        request.target.split('?').next().unwrap_or(""),
    ) {
        ("POST", "/pair-setup") => {
            if request_state(&request.body) == Some(1) && session.begin_pair_setup().is_err() {
                return DispatchResult::close(Response::plain(
                    400,
                    b"invalid Pair-Setup session transition".to_vec(),
                ));
            }
            match pair_setup.handle(&request.body) {
                Ok(result) => {
                    if result.paired {
                        if let Err(error) = discovery.set_paired(true).await {
                            tracing::warn!(%error, "paired state persisted but mDNS update failed");
                        }
                    }
                    if result.terminal {
                        let _ = session.reset_pairing();
                    }
                    DispatchResult::keep(Response {
                        status: 200,
                        content_type: HAP_TLV,
                        body: result.body,
                    })
                }
                Err(error) => {
                    DispatchResult::close(Response::plain(400, error.to_string().into_bytes()))
                }
            }
        }
        ("POST", "/pair-verify") => {
            if request_state(&request.body) == Some(1) && session.begin_pair_verify().is_err() {
                return DispatchResult::close(Response::plain(
                    400,
                    b"invalid Pair-Verify session transition".to_vec(),
                ));
            }
            match pair_verify.handle(&request.body) {
                Ok(result) => {
                    if let Some(authenticated) = result.authenticated {
                        if let Err(error) = session.authenticate(
                            authenticated.controller_id,
                            authenticated.admin,
                            authenticated.keys,
                        ) {
                            return DispatchResult::close(Response::plain(
                                400,
                                error.to_string().into_bytes(),
                            ));
                        }
                    } else if result.terminal {
                        let _ = session.reset_pairing();
                    }
                    DispatchResult::keep(Response {
                        status: 200,
                        content_type: HAP_TLV,
                        body: result.body,
                    })
                }
                Err(error) => {
                    DispatchResult::close(Response::plain(400, error.to_string().into_bytes()))
                }
            }
        }
        _ if !session.state().is_authenticated() => DispatchResult::keep(Response::json(
            470,
            json!({"status": -70401, "message": "Connection Authorization Required"}),
        )),
        ("GET", "/accessories") => {
            DispatchResult::keep(Response::json(200, accessories_json(bridge)))
        }
        ("GET", "/characteristics") => {
            DispatchResult::keep(characteristics_response(&request.target, bridge))
        }
        ("PUT", "/characteristics") => DispatchResult::keep(characteristic_subscription_response(
            &request.body,
            subscriptions,
        )),
        ("POST", "/pairings") => {
            pairings_response(&request.body, session, pairings, discovery).await
        }
        _ => DispatchResult::keep(Response::plain(404, b"not found".to_vec())),
    }
}

fn request_state(body: &[u8]) -> Option<u8> {
    Tlv8::parse(body).ok()?.byte(TLV_STATE)
}

async fn pairings_response(
    body: &[u8],
    session: &Session,
    pairings: &PairingStore,
    discovery: &DiscoveryState,
) -> DispatchResult {
    let response = |body| Response {
        status: 200,
        content_type: HAP_TLV,
        body,
    };
    let Some(controller_id) = session.controller_id() else {
        return DispatchResult::close(response(tlv_error_response(2, TLV_ERROR_AUTHENTICATION)));
    };
    let authorized = pairings
        .get(controller_id)
        .ok()
        .flatten()
        .is_some_and(|pairing| pairing.admin);
    if !authorized {
        return DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_AUTHENTICATION)));
    }
    let Ok(tlv) = Tlv8::parse(body) else {
        return DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN)));
    };
    if tlv.byte(TLV_STATE) != Some(1) {
        return DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN)));
    }

    match tlv.byte(TLV_METHOD) {
        Some(3) => {
            let Some(identifier) = pairing_identifier(&tlv) else {
                return DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN)));
            };
            let Some(public_key) = tlv
                .get(TLV_PUBLIC_KEY)
                .and_then(|value| <[u8; 32]>::try_from(value).ok())
            else {
                return DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN)));
            };
            let Some(admin) = tlv
                .byte(TLV_PERMISSIONS)
                .and_then(|permission| match permission {
                    0 => Some(false),
                    1 => Some(true),
                    _ => None,
                })
            else {
                return DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN)));
            };
            let pairing = ControllerPairing {
                controller_id: identifier,
                public_key,
                admin,
            };
            match pairings.upsert(pairing) {
                Ok(()) => {
                    DispatchResult::keep(response(encode_items([(TLV_STATE, [2].as_slice())])))
                }
                Err(HapError::PairingCapacity) => {
                    DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_MAX_PEERS)))
                }
                Err(_) => DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN))),
            }
        }
        Some(4) => {
            let Some(identifier) = pairing_identifier(&tlv) else {
                return DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN)));
            };
            if pairings.remove_hap(&identifier).is_err() {
                return DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN)));
            }
            let paired = pairings.is_paired().unwrap_or(true);
            if let Err(error) = discovery.set_paired(paired).await {
                tracing::warn!(%error, "pairing removal persisted but mDNS update failed");
            }
            let removed_current = pairings.get(controller_id).ok().flatten().is_none();
            let result = response(encode_items([(TLV_STATE, [2].as_slice())]));
            if removed_current {
                DispatchResult::close(result)
            } else {
                DispatchResult::keep(result)
            }
        }
        Some(5) => {
            let Ok(records) = pairings.list() else {
                return DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN)));
            };
            let mut body = encode_items([(TLV_STATE, [2].as_slice())]);
            for (index, pairing) in records.iter().enumerate() {
                if index != 0 {
                    body.extend_from_slice(&[TLV_SEPARATOR, 0]);
                }
                body.extend_from_slice(&encode_items([
                    (TLV_IDENTIFIER, pairing.controller_id.as_bytes()),
                    (TLV_PUBLIC_KEY, pairing.public_key.as_slice()),
                    (TLV_PERMISSIONS, [u8::from(pairing.admin)].as_slice()),
                ]));
            }
            DispatchResult::keep(response(body))
        }
        _ => DispatchResult::keep(response(tlv_error_response(2, TLV_ERROR_UNKNOWN))),
    }
}

fn pairing_identifier(tlv: &Tlv8) -> Option<String> {
    let value = tlv.get(TLV_IDENTIFIER)?;
    if value.is_empty() || value.len() > 64 {
        return None;
    }
    let identifier = std::str::from_utf8(value).ok()?;
    if identifier.chars().any(char::is_control) {
        return None;
    }
    Some(identifier.to_owned())
}

fn accessories_json(bridge: &HapBridge) -> Value {
    let accessories = indexed_accessories(bridge);
    let mut output = vec![json!({
        "aid": 1,
        "services": [accessory_information(1, "HOMECORE Bridge")]
    })];
    for (aid, accessory) in accessories {
        let mut characteristics = Vec::new();
        for (index, (kind, value)) in accessory.mapping.characteristics.iter().enumerate() {
            characteristics.push(json!({
                "iid": 8 + index as u64,
                "type": characteristic_type(*kind),
                "perms": ["pr", "ev"],
                "format": characteristic_format(value),
                "value": characteristic_value(value),
            }));
        }
        output.push(json!({
            "aid": aid,
            "services": [
                accessory_information(1, accessory.entity_id.as_str()),
                {
                    "iid": 7,
                    "type": service_type(accessory.accessory_type),
                    "primary": true,
                    "characteristics": characteristics,
                }
            ]
        }));
    }
    json!({ "accessories": output })
}

fn accessory_information(iid: u64, name: &str) -> Value {
    json!({
        "iid": iid,
        "type": "3E",
        "characteristics": [
            {"iid": iid + 1, "type": "23", "perms": ["pr"], "format": "string", "value": name},
            {"iid": iid + 2, "type": "20", "perms": ["pr"], "format": "string", "value": "HOMECORE"},
            {"iid": iid + 3, "type": "21", "perms": ["pr"], "format": "string", "value": "HOMECORE HAP Bridge"},
            {"iid": iid + 4, "type": "30", "perms": ["pr"], "format": "string", "value": name},
            {"iid": iid + 5, "type": "52", "perms": ["pr"], "format": "string", "value": env!("CARGO_PKG_VERSION")}
        ]
    })
}

fn indexed_accessories(bridge: &HapBridge) -> Vec<(u64, ExposedAccessory)> {
    let mut accessories = bridge.running_accessories();
    accessories.sort_by(|left, right| left.entity_id.as_str().cmp(right.entity_id.as_str()));
    accessories
        .into_iter()
        .enumerate()
        .map(|(index, accessory)| (index as u64 + 2, accessory))
        .collect()
}

fn characteristics_response(target: &str, bridge: &HapBridge) -> Response {
    let Some(query) = target.split_once('?').map(|(_, query)| query) else {
        return Response::plain(400, b"missing characteristic query".to_vec());
    };
    let Some(ids) = query.split('&').find_map(|part| part.strip_prefix("id=")) else {
        return Response::plain(400, b"missing id query".to_vec());
    };
    if ids.len() > 4096 || ids.split(',').count() > 128 {
        return Response::plain(400, b"characteristic query too large".to_vec());
    }
    let accessories = indexed_accessories(bridge);
    let mut values = Vec::new();
    for id in ids.split(',') {
        let Some((aid, iid)) = parse_aid_iid(id) else {
            return Response::plain(400, b"invalid aid.iid".to_vec());
        };
        let value = accessories
            .iter()
            .find(|(candidate, _)| *candidate == aid)
            .and_then(|(_, accessory)| {
                accessory
                    .mapping
                    .characteristics
                    .get(iid.saturating_sub(8) as usize)
            })
            .map(|(_, value)| characteristic_value(value));
        values.push(match value {
            Some(value) => json!({"aid": aid, "iid": iid, "value": value}),
            None => json!({"aid": aid, "iid": iid, "status": -70409}),
        });
    }
    Response::json(207, json!({"characteristics": values}))
}

// Per-connection retained state must be bounded across requests, not just
// within one request's 128-item batch.
const MAX_CHARACTERISTIC_SUBSCRIPTIONS: usize = 1024;

fn characteristic_subscription_response(
    body: &[u8],
    subscriptions: &mut HashSet<(u64, u64)>,
) -> Response {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return Response::plain(400, b"invalid characteristic JSON".to_vec());
    };
    let Some(items) = value.get("characteristics").and_then(Value::as_array) else {
        return Response::plain(400, b"missing characteristics array".to_vec());
    };
    if items.len() > 128 {
        return Response::plain(400, b"too many characteristic writes".to_vec());
    }
    for item in items {
        let (Some(aid), Some(iid), Some(enabled)) = (
            item.get("aid").and_then(Value::as_u64),
            item.get("iid").and_then(Value::as_u64),
            item.get("ev").and_then(Value::as_bool),
        ) else {
            // Entity writes are not yet connected to HOMECORE service calls.
            return Response::json(207, json!({"characteristics": [{"status": -70405}]}));
        };
        if enabled {
            if subscriptions.len() >= MAX_CHARACTERISTIC_SUBSCRIPTIONS
                && !subscriptions.contains(&(aid, iid))
            {
                return Response::json(207, json!({"characteristics": [
                    {"aid": aid, "iid": iid, "status": -70407}
                ]}));
            }
            subscriptions.insert((aid, iid));
        } else {
            subscriptions.remove(&(aid, iid));
        }
    }
    Response {
        status: 204,
        content_type: HAP_JSON,
        body: Vec::new(),
    }
}

fn event_payload(
    bridge: &HapBridge,
    event: &CharacteristicEvent,
    subscriptions: &HashSet<(u64, u64)>,
) -> Option<Vec<u8>> {
    let (aid, _) = indexed_accessories(bridge)
        .into_iter()
        .find(|(_, accessory)| accessory.entity_id == event.entity_id)?;
    let values: Vec<Value> = event
        .characteristics
        .iter()
        .enumerate()
        .filter_map(|(index, (_, value))| {
            let iid = index as u64 + 8;
            subscriptions
                .contains(&(aid, iid))
                .then(|| json!({"aid": aid, "iid": iid, "value": characteristic_value(value)}))
        })
        .collect();
    (!values.is_empty())
        .then(|| serde_json::to_vec(&json!({"characteristics": values})).expect("serialize event"))
}

fn parse_aid_iid(value: &str) -> Option<(u64, u64)> {
    let (aid, iid) = value.split_once('.')?;
    Some((aid.parse().ok()?, iid.parse().ok()?))
}

fn characteristic_value(value: &HapCharacteristicValue) -> Value {
    match value {
        HapCharacteristicValue::Bool(value) => json!(value),
        HapCharacteristicValue::UInt8(value) => json!(value),
        HapCharacteristicValue::Float(value) => json!(value),
    }
}

fn characteristic_format(value: &HapCharacteristicValue) -> &'static str {
    match value {
        HapCharacteristicValue::Bool(_) => "bool",
        HapCharacteristicValue::UInt8(_) => "uint8",
        HapCharacteristicValue::Float(_) => "float",
    }
}

fn service_type(kind: HapAccessoryType) -> &'static str {
    match kind {
        HapAccessoryType::Lightbulb => "43",
        HapAccessoryType::Switch => "49",
        HapAccessoryType::OccupancySensor => "86",
        HapAccessoryType::MotionSensor => "85",
        HapAccessoryType::TemperatureSensor => "8A",
        HapAccessoryType::HumiditySensor => "82",
        HapAccessoryType::LeakSensor => "83",
        HapAccessoryType::ContactSensor => "80",
        HapAccessoryType::Door => "81",
        HapAccessoryType::Lock => "45",
        HapAccessoryType::SecuritySystem => "7E",
    }
}

fn characteristic_type(kind: HapCharacteristic) -> &'static str {
    match kind {
        HapCharacteristic::On => "25",
        HapCharacteristic::Brightness => "8",
        HapCharacteristic::CurrentTemperature => "11",
        HapCharacteristic::CurrentRelativeHumidity => "10",
        HapCharacteristic::OccupancyDetected => "71",
        HapCharacteristic::MotionDetected => "22",
        HapCharacteristic::LeakDetected => "70",
        HapCharacteristic::ContactSensorState => "6A",
        HapCharacteristic::CurrentDoorState => "E",
        HapCharacteristic::LockCurrentState => "1D",
        HapCharacteristic::SecuritySystemCurrentState => "66",
    }
}

async fn write_response(
    stream: &mut TcpStream,
    records: Option<&mut RecordLayer>,
    response: Response,
) -> Result<(), HapError> {
    let reason = match response.status {
        200 => "OK",
        204 => "No Content",
        207 => "Multi-Status",
        400 => "Bad Request",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        470 => "Connection Authorization Required",
        _ => "Error",
    };
    let mut message = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len()
    )
    .into_bytes();
    message.extend_from_slice(&response.body);
    write_transport(stream, records, &message).await
}

async fn write_event(
    stream: &mut TcpStream,
    records: &mut RecordLayer,
    body: Vec<u8>,
) -> Result<(), HapError> {
    let mut message = format!(
        "EVENT/1.0 200 OK\r\nContent-Type: {HAP_JSON}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    message.extend_from_slice(&body);
    write_transport(stream, Some(records), &message).await
}

async fn write_transport(
    stream: &mut TcpStream,
    records: Option<&mut RecordLayer>,
    plaintext: &[u8],
) -> Result<(), HapError> {
    let output = match records {
        Some(records) => records.encrypt(plaintext)?,
        None => plaintext.to_vec(),
    };
    stream
        .write_all(&output)
        .await
        .map_err(|error| HapError::Server(format!("write HAP transport: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{hkdf_sha512, open_labeled, seal_labeled, SessionKeys};
    use crate::mdns::{HapServiceRecord, NullAdvertiser};
    use crate::pairing::SetupCode;
    use crate::protocol::{TLV_ENCRYPTED_DATA, TLV_SIGNATURE};
    use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
    use homecore::entity::{EntityId, State};
    use homecore::event::Context;
    use x25519_dalek::{PublicKey, StaticSecret};

    #[test]
    fn subscriptions_are_bounded_across_requests_and_recover_capacity() {
        let mut subscriptions = HashSet::new();
        for aid in 0..1024 {
            let body =
                serde_json::to_vec(&json!({"characteristics": [{"aid": aid, "iid": 8, "ev": true}]}))
                    .unwrap();
            assert_eq!(
                characteristic_subscription_response(&body, &mut subscriptions).status,
                204
            );
        }
        let overflow =
            serde_json::to_vec(&json!({"characteristics": [{"aid": 1024, "iid": 8, "ev": true}]}))
                .unwrap();
        let response = characteristic_subscription_response(&overflow, &mut subscriptions);
        assert_eq!(
            subscriptions.len(),
            1024,
            "per-request limit must also bound retained subscriptions"
        );
        assert_eq!(response.status, 207);
        let error: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(error["characteristics"][0]["status"], -70407);
        let duplicate =
            serde_json::to_vec(&json!({"characteristics": [{"aid": 0, "iid": 8, "ev": true}]}))
                .unwrap();
        assert_eq!(
            characteristic_subscription_response(&duplicate, &mut subscriptions).status,
            204
        );
        let remove =
            serde_json::to_vec(&json!({"characteristics": [{"aid": 0, "iid": 8, "ev": false}]}))
                .unwrap();
        assert_eq!(
            characteristic_subscription_response(&remove, &mut subscriptions).status,
            204
        );
        assert_eq!(
            characteristic_subscription_response(&overflow, &mut subscriptions).status,
            204
        );
        assert_eq!(subscriptions.len(), 1024);
    }

    fn bridge() -> HapBridge {
        let bridge = HapBridge::new(HapServiceRecord::bridge(
            "RuView Sense",
            51826,
            "AA:BB:CC:DD:EE:FF",
        ));
        let entity_id = EntityId::parse("binary_sensor.room_occupancy").unwrap();
        let state = State::new(
            entity_id.clone(),
            "on",
            json!({"device_class": "occupancy"}),
            Context::default(),
        );
        bridge.add_accessory(&entity_id, &state).unwrap();
        bridge
    }

    async fn server() -> (HapServerHandle, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let pairings = Arc::new(
            PairingStore::create(
                directory.path().join("pairings.json"),
                SetupCode::parse("518-26-003").unwrap(),
                Some("AA:BB:CC:DD:EE:FF".into()),
            )
            .unwrap(),
        );
        let config = HapServerConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            request_timeout: Duration::from_secs(1),
            shutdown_timeout: Duration::from_secs(1),
            ..HapServerConfig::default()
        };
        let handle = start_server(config, bridge(), pairings, Arc::new(NullAdvertiser))
            .await
            .unwrap();
        (handle, directory)
    }

    async fn exchange(addr: SocketAddr, request: &[u8]) -> Vec<u8> {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(request).await.unwrap();
        stream.shutdown().await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        response
    }

    async fn paired_server() -> (HapServerHandle, tempfile::TempDir, SigningKey) {
        let directory = tempfile::tempdir().unwrap();
        let pairings = Arc::new(
            PairingStore::create(
                directory.path().join("pairings.json"),
                SetupCode::parse("518-26-003").unwrap(),
                Some("AA:BB:CC:DD:EE:FF".into()),
            )
            .unwrap(),
        );
        let controller = SigningKey::from_bytes(&[0x42; 32]);
        pairings
            .add_initial(ControllerPairing {
                controller_id: "network-controller".into(),
                public_key: controller.verifying_key().to_bytes(),
                admin: true,
            })
            .unwrap();
        let config = HapServerConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            request_timeout: Duration::from_secs(1),
            shutdown_timeout: Duration::from_secs(1),
            ..HapServerConfig::default()
        };
        let handle = start_server(config, bridge(), pairings, Arc::new(NullAdvertiser))
            .await
            .unwrap();
        (handle, directory, controller)
    }

    async fn post_tlv(stream: &mut TcpStream, path: &str, body: &[u8]) -> Vec<u8> {
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: {HAP_TLV}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        stream.write_all(body).await.unwrap();
        read_plain_http(stream).await
    }

    async fn read_plain_http(stream: &mut TcpStream) -> Vec<u8> {
        let mut response = Vec::new();
        while find_header_end(&response).is_none() {
            let mut byte = [0u8; 1];
            stream.read_exact(&mut byte).await.unwrap();
            response.push(byte[0]);
        }
        let header_end = find_header_end(&response).unwrap() + 4;
        let header = std::str::from_utf8(&response[..header_end]).unwrap();
        let length = header
            .lines()
            .find_map(|line| {
                line.strip_prefix("Content-Length: ")
                    .and_then(|value| value.parse::<usize>().ok())
            })
            .unwrap();
        response.resize(header_end + length, 0);
        stream
            .read_exact(&mut response[header_end..])
            .await
            .unwrap();
        response
    }

    async fn read_encrypted_http(stream: &mut TcpStream, records: &mut RecordLayer) -> Vec<u8> {
        let mut plaintext = Vec::new();
        loop {
            let mut length = [0u8; 2];
            stream.read_exact(&mut length).await.unwrap();
            let payload_length = u16::from_le_bytes(length) as usize;
            let mut encrypted = vec![0u8; payload_length + RECORD_TAG_BYTES];
            stream.read_exact(&mut encrypted).await.unwrap();
            plaintext.extend_from_slice(&records.decrypt(length, &encrypted).unwrap());
            if let Some(header_position) = find_header_end(&plaintext) {
                let header_end = header_position + 4;
                let header = std::str::from_utf8(&plaintext[..header_end]).unwrap();
                let content_length = header
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("Content-Length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .unwrap();
                if plaintext.len() >= header_end + content_length {
                    return plaintext;
                }
            }
        }
    }

    #[tokio::test]
    async fn lifecycle_binds_and_shuts_down() {
        let (server, _directory) = server().await;
        assert_ne!(server.local_addr().port(), 0);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_remains_bounded_with_idle_connection() {
        let (server, _directory) = server().await;
        let _idle = TcpStream::connect(server.local_addr()).await.unwrap();
        timeout(Duration::from_secs(3), server.shutdown())
            .await
            .expect("shutdown exceeded its outer bound")
            .unwrap();
    }

    #[tokio::test]
    async fn unauthenticated_accessory_request_is_gated() {
        let (server, _directory) = server().await;
        let response = exchange(
            server.local_addr(),
            b"GET /accessories HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(response.starts_with(b"HTTP/1.1 470"));
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn pair_setup_m1_returns_real_srp_challenge() {
        let (server, _directory) = server().await;
        let response = exchange(
            server.local_addr(),
            b"POST /pair-setup HTTP/1.1\r\nHost: localhost\r\nContent-Length: 6\r\nConnection: close\r\n\r\n\x00\x01\x00\x06\x01\x01",
        )
        .await;
        assert!(response.starts_with(b"HTTP/1.1 200"));
        let body_start = find_header_end(&response).unwrap() + 4;
        let tlv = Tlv8::parse(&response[body_start..]).unwrap();
        assert_eq!(tlv.byte(TLV_STATE), Some(2));
        assert_eq!(tlv.get(crate::protocol::TLV_SALT).unwrap().len(), 16);
        assert_eq!(tlv.get(TLV_PUBLIC_KEY).unwrap().len(), 384);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn pair_verify_enables_encrypted_access_and_replay_closes_connection() {
        let (server, _directory, controller_signing) = paired_server().await;
        let mut stream = TcpStream::connect(server.local_addr()).await.unwrap();
        let controller_secret = StaticSecret::from([0x24; 32]);
        let controller_public = PublicKey::from(&controller_secret).to_bytes();
        let m1 = encode_items([
            (TLV_STATE, [1].as_slice()),
            (TLV_PUBLIC_KEY, controller_public.as_slice()),
        ]);
        let m2_http = post_tlv(&mut stream, "/pair-verify", &m1).await;
        let m2 = Tlv8::parse(&m2_http[find_header_end(&m2_http).unwrap() + 4..]).unwrap();
        assert_eq!(m2.byte(TLV_STATE), Some(2));
        let accessory_public: [u8; 32] = m2.get(TLV_PUBLIC_KEY).unwrap().try_into().unwrap();
        let shared = controller_secret.diffie_hellman(&PublicKey::from(accessory_public));
        let verify_key = hkdf_sha512(
            b"Pair-Verify-Encrypt-Salt",
            shared.as_bytes(),
            b"Pair-Verify-Encrypt-Info",
        )
        .unwrap();
        let accessory_data = Tlv8::parse(
            &open_labeled(
                &verify_key,
                b"PV-Msg02",
                m2.get(TLV_ENCRYPTED_DATA).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let accessory_id = accessory_data.get(TLV_IDENTIFIER).unwrap();
        let accessory_signature: [u8; 64] = accessory_data
            .get(TLV_SIGNATURE)
            .unwrap()
            .try_into()
            .unwrap();
        let mut accessory_info = Vec::new();
        accessory_info.extend_from_slice(&accessory_public);
        accessory_info.extend_from_slice(accessory_id);
        accessory_info.extend_from_slice(&controller_public);
        let persisted = PairingStore::open(_directory.path().join("pairings.json")).unwrap();
        VerifyingKey::from_bytes(&persisted.accessory_public_key().unwrap())
            .unwrap()
            .verify_strict(
                &accessory_info,
                &Signature::from_bytes(&accessory_signature),
            )
            .unwrap();

        let controller_id = b"network-controller";
        let mut controller_info = Vec::new();
        controller_info.extend_from_slice(&controller_public);
        controller_info.extend_from_slice(controller_id);
        controller_info.extend_from_slice(&accessory_public);
        let signature = controller_signing.sign(&controller_info).to_bytes();
        let sub_tlv = encode_items([
            (TLV_IDENTIFIER, controller_id.as_slice()),
            (TLV_SIGNATURE, signature.as_slice()),
        ]);
        let encrypted = seal_labeled(&verify_key, b"PV-Msg03", &sub_tlv).unwrap();
        let m3 = encode_items([
            (TLV_STATE, [3].as_slice()),
            (TLV_ENCRYPTED_DATA, encrypted.as_slice()),
        ]);
        let m4_http = post_tlv(&mut stream, "/pair-verify", &m3).await;
        let m4 = Tlv8::parse(&m4_http[find_header_end(&m4_http).unwrap() + 4..]).unwrap();
        assert_eq!(m4.byte(TLV_STATE), Some(4));

        let keys = SessionKeys::derive(shared.as_bytes())
            .unwrap()
            .controller_view();
        let mut records = RecordLayer::controller(keys);
        let request = b"GET /accessories HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n";
        let encrypted_request = records.encrypt(request).unwrap();
        stream.write_all(&encrypted_request).await.unwrap();
        let response = read_encrypted_http(&mut stream, &mut records).await;
        assert!(response.starts_with(b"HTTP/1.1 200"));
        assert!(response.windows(11).any(|window| window == b"accessories"));

        stream.write_all(&encrypted_request).await.unwrap();
        let mut byte = [0u8; 1];
        let read = timeout(Duration::from_secs(2), stream.read(&mut byte))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read, 0);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn malformed_and_oversized_requests_are_rejected() {
        let (server, _directory) = server().await;
        let malformed = exchange(
            server.local_addr(),
            b"GET / HTTP/1.0\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(malformed.starts_with(b"HTTP/1.1 400"));
        let oversized = exchange(
            server.local_addr(),
            b"POST /pair-setup HTTP/1.1\r\nContent-Length: 999999\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(oversized.starts_with(b"HTTP/1.1 413"));
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn authenticated_internal_dispatch_exposes_accessories_and_events() {
        let bridge = bridge();
        let directory = tempfile::tempdir().unwrap();
        let pairings = Arc::new(
            PairingStore::create(
                directory.path().join("pairings.json"),
                SetupCode::parse("518-26-003").unwrap(),
                Some("AA:BB:CC:DD:EE:FF".into()),
            )
            .unwrap(),
        );
        pairings
            .add_initial(ControllerPairing {
                controller_id: "test-controller".into(),
                public_key: SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes(),
                admin: true,
            })
            .unwrap();
        let mut session = Session::authenticated_for_test(true);
        let mut pair_setup = PairSetup::new(pairings.clone());
        let mut pair_verify = PairVerify::new(pairings.clone());
        let discovery = DiscoveryState {
            advertiser: Arc::new(NullAdvertiser),
            record: Mutex::new(bridge.service_record.clone()),
        };
        let mut subscriptions = HashSet::new();
        let response = dispatch_request(
            Request {
                method: "GET".into(),
                target: "/accessories".into(),
                body: Vec::new(),
                connection_close: false,
            },
            &mut session,
            (&mut pair_setup, &mut pair_verify),
            &bridge,
            &pairings,
            &discovery,
            &mut subscriptions,
        )
        .await;
        assert_eq!(response.response.status, 200);
        let body: Value = serde_json::from_slice(&response.response.body).unwrap();
        assert_eq!(body["accessories"].as_array().unwrap().len(), 2);

        let response = dispatch_request(
            Request {
                method: "PUT".into(),
                target: "/characteristics".into(),
                body: br#"{"characteristics":[{"aid":2,"iid":8,"ev":true}]}"#.to_vec(),
                connection_close: false,
            },
            &mut session,
            (&mut pair_setup, &mut pair_verify),
            &bridge,
            &pairings,
            &discovery,
            &mut subscriptions,
        )
        .await;
        assert_eq!(response.response.status, 204);
        assert!(subscriptions.contains(&(2, 8)));
    }

    #[tokio::test]
    async fn pairing_management_rechecks_admin_and_enforces_last_admin_invariant() {
        let bridge = bridge();
        let directory = tempfile::tempdir().unwrap();
        let pairings = Arc::new(
            PairingStore::create(
                directory.path().join("pairings.json"),
                SetupCode::parse("518-26-003").unwrap(),
                Some("AA:BB:CC:DD:EE:FF".into()),
            )
            .unwrap(),
        );
        let admin_key = SigningKey::from_bytes(&[8; 32]);
        pairings
            .add_initial(ControllerPairing {
                controller_id: "test-controller".into(),
                public_key: admin_key.verifying_key().to_bytes(),
                admin: true,
            })
            .unwrap();
        let mut session = Session::authenticated_for_test(true);
        let mut pair_setup = PairSetup::new(pairings.clone());
        let mut pair_verify = PairVerify::new(pairings.clone());
        let discovery = DiscoveryState {
            advertiser: Arc::new(NullAdvertiser),
            record: Mutex::new(bridge.service_record.clone()),
        };
        let mut subscriptions = HashSet::new();
        let member_key = SigningKey::from_bytes(&[9; 32]).verifying_key().to_bytes();
        let add = encode_items([
            (TLV_STATE, [1].as_slice()),
            (TLV_METHOD, [3].as_slice()),
            (TLV_IDENTIFIER, b"member".as_slice()),
            (TLV_PUBLIC_KEY, member_key.as_slice()),
            (TLV_PERMISSIONS, [0].as_slice()),
        ]);
        let result = dispatch_request(
            Request {
                method: "POST".into(),
                target: "/pairings".into(),
                body: add,
                connection_close: false,
            },
            &mut session,
            (&mut pair_setup, &mut pair_verify),
            &bridge,
            &pairings,
            &discovery,
            &mut subscriptions,
        )
        .await;
        assert_eq!(
            Tlv8::parse(&result.response.body).unwrap().byte(TLV_STATE),
            Some(2)
        );
        assert!(pairings.get("member").unwrap().is_some());

        let remove = encode_items([
            (TLV_STATE, [1].as_slice()),
            (TLV_METHOD, [4].as_slice()),
            (TLV_IDENTIFIER, b"test-controller".as_slice()),
        ]);
        let result = dispatch_request(
            Request {
                method: "POST".into(),
                target: "/pairings".into(),
                body: remove,
                connection_close: false,
            },
            &mut session,
            (&mut pair_setup, &mut pair_verify),
            &bridge,
            &pairings,
            &discovery,
            &mut subscriptions,
        )
        .await;
        assert!(result.close_after_response);
        assert!(pairings.list().unwrap().is_empty());
    }
}
