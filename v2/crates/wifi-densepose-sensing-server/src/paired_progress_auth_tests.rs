//! Synthetic HTTP producer plus actual companion policy/credential boundaries.
use super::*;
use axum::{body::Body,http::{Method,Request},routing::get};
use tower::ServiceExt;
use wifi_densepose_sensing_server::room_learner::{lifecycle::RoomLearningRuntime,observation::{CaptureContext,SourceSpec}};
const PATH:&str="/api/v1/room-learner/progress";
async fn json_call(router:Router,method:Method,path:&str,body:serde_json::Value)->(StatusCode,serde_json::Value){
    let r=router.oneshot(Request::builder().method(method).uri(path).header("content-type","application/json").body(Body::from(body.to_string())).unwrap()).await.unwrap();
    let status=r.status();let b=axum::body::to_bytes(r.into_body(),65536).await.unwrap();(status,serde_json::from_slice(&b).unwrap_or(serde_json::Value::Null))
}
async fn pair(service:Arc<Companion>,device:&str)->String {
    let owner=companion::owner_router(service.clone());let public=companion::public_router(service);
    let (s,invite)=json_call(owner.clone(),Method::POST,"/api/v1/companion/invites",serde_json::json!({})).await;assert_eq!(s,StatusCode::OK);
    let proof=companion::secret();
    let (s,enrolled)=json_call(public.clone(),Method::POST,"/companion/v1/enroll",serde_json::json!({"installation_id":"synthetic-install","invite_id":invite["invite_id"],"invite_secret":invite["invite_secret"],"device_id":device,"device_name":"Synthetic phone","pairing_secret":proof})).await;assert_eq!(s,StatusCode::OK);
    let (s,_)=json_call(owner,Method::POST,&format!("/api/v1/companion/requests/{}/approve",enrolled["request_id"].as_str().unwrap()),serde_json::json!({"comparison_code":enrolled["comparison_code"]})).await;assert_eq!(s,StatusCode::OK);
    let (s,session)=json_call(public,Method::POST,"/companion/v1/session",serde_json::json!({"installation_id":"synthetic-install","device_id":device,"pairing_secret":proof})).await;assert_eq!(s,StatusCode::OK);session["access_token"].as_str().unwrap().into()
}
fn room()->serde_json::Value{serde_json::json!({"schema":"ruview.companion.room.v1","id":"synthetic-room","name":"Synthetic room","widthM":3,"lengthM":3,"heightM":2.8,"nodes":[{"nodeId":1,"slot":"north_west","xM":0,"yM":0,"role":"base_station","headingDeg":90,"orientationConfirmed":true,"confirmedAtUnixMs":1},{"nodeId":2,"slot":"south_east","xM":3,"yM":3,"role":"receiver","headingDeg":270,"orientationConfirmed":true,"confirmedAtUnixMs":1}],"accessPoint":null})}
async fn fixture()->(tempfile::TempDir,SharedState,Arc<companion_room::SharedRoomStore>){
    let dir=tempfile::tempdir().unwrap();let owner="a".repeat(64);
    let rt=Arc::new(RoomLearningRuntime::new(&dir.path().join(&owner).join("SensingServer"),&owner,"synthetic-install").unwrap());
    let publication=companion_room::SharedRoomStore::load(&dir.path().join("publication"),"synthetic-install").unwrap();
    let (status,envelope)=json_call(companion_room::owner_router(publication.clone()),Method::PUT,"/api/v1/companion/room",serde_json::json!({"expected_revision":0,"room":room()})).await;assert_eq!(status,StatusCode::OK);
    let context=CaptureContext{owner_scope:owner,installation_id:"synthetic-install".into(),room_digest:envelope["binding_digest"].as_str().unwrap().into(),revision:77,sources:(1..=2).map(|id|SourceSpec{source_id:id,device_id:format!("esp32-node-{id}"),grid_id:"ant1-bins64-ppdu0-usable-drop2-linear-v2".into(),feature_count:56,signal_kind:ruview_unified::RfModality::WifiCsi,center_frequency_hz:2_437_000_000,bandwidth_hz:20_000_000,link_id:format!("synthetic-link-{id}"),link_index:0,geometry:None}).collect()};
    rt.configure(&rt.mutation_guard().unwrap(),context,"synthetic-actual-http-run".into(),true).unwrap();
    let mut inner=AppStateInner::minimal();inner.room_learning=Some(rt);inner.current_room_store=Some(publication.clone());inner.installation_id=Some("synthetic-install".into());inner.calibration_boot_epoch="cal-boot-synthetic".into();
    let now=std::time::Instant::now();
    for row in 0..120u32 {for node in 1..=2u8 {
        let mut wire=Vec::new();wire.extend(0xC5110001u32.to_le_bytes());wire.extend([node,1]);wire.extend(64u16.to_le_bytes());wire.extend(2437u32.to_le_bytes());wire.extend((row+1).to_le_bytes());wire.extend([(-40i8)as u8,(-90i8)as u8,0,0x20]);for _ in 0..64 {wire.extend([3,4]);}
        let frame=parse_esp32_frame(&wire).unwrap();room_learner_api::observe(&mut inner,&frame,&wire,now-std::time::Duration::from_millis(u64::from(120-row)*100));
    }}
    (dir,Arc::new(RwLock::new(inner)),publication)
}
fn app(state:SharedState,service:Arc<Companion>)->Router{
    Router::new().route(PATH,get(room_learner_api::progress)).route("/api/v1/room-learner/status",get(room_learner_api::status))
        .route("/api/v1/room-learner/phase/start",post(room_learner_api::begin_phase))
        .with_state(state).layer(axum::middleware::from_fn_with_state(service,require_companion))
}
async fn fetch(router:Router,method:Method,installation:&str,token:&str,path:&str)->(StatusCode,serde_json::Value){
    let response=router.oneshot(Request::builder().method(method).uri(path).header("x-ruview-installation-id",installation).header("authorization",format!("Bearer {token}")).body(Body::empty()).unwrap()).await.unwrap();
    let status=response.status();let b=axum::body::to_bytes(response.into_body(),65536).await.unwrap();(status,serde_json::from_slice(&b).unwrap_or(serde_json::Value::Null))
}
#[tokio::test]
async fn progress_http_read_scope_installation_revocation_and_no_mutation_grant_compose(){
    let (dir,state,_)=fixture().await;let service=Companion::load(&dir.path().join("pair"),"synthetic-install".into(),"f".repeat(64),"https://synthetic.local:3443".into()).unwrap();let token=pair(service.clone(),"phone").await;let router=app(state.clone(),service.clone());
    let rt=state.read().await.room_learning.clone().unwrap();let guard=rt.mutation_guard().unwrap();
    let (s,p)=fetch(router.clone(),Method::GET,"synthetic-install",&token,PATH).await;assert_eq!(s,StatusCode::OK);assert_eq!(p["schema"],"ruview.room-learner.progress.v1");assert_eq!(p["context_match"],true);assert_eq!(p["room"]["revision"],1);assert_eq!(p["sources"]["required"],2);assert_eq!(p["sources"]["ready"],2);assert_eq!(p["save_status"],"not_reported");assert_eq!(rt.mutation_guard().unwrap(),guard);
    assert!(service.authorize("synthetic-install",&token,true).is_err(),"read grant must remain read-only");
    for (method,path,install,t) in [(Method::POST,PATH,"synthetic-install",token.as_str()),(Method::POST,"/api/v1/room-learner/phase/start","synthetic-install",token.as_str()),(Method::GET,"/api/v1/room-learner/status","synthetic-install",token.as_str()),(Method::GET,PATH,"foreign-install",token.as_str()),(Method::GET,PATH,"synthetic-install","invalid")] {
        let status=fetch(router.clone(),method,install,t,path).await.0;assert!(matches!(status,StatusCode::UNAUTHORIZED|StatusCode::FORBIDDEN));
    }
    assert_eq!(fetch(router.clone(),Method::GET,"synthetic-install",&token,&format!("{PATH}?token=ignored")).await.0,StatusCode::BAD_REQUEST);
    assert_eq!(json_call(companion::owner_router(service.clone()),Method::POST,"/api/v1/companion/devices/phone/calibration",serde_json::json!({})).await.0,StatusCode::OK);
    assert!(service.authorize("synthetic-install",&token,true).is_ok());
    assert_eq!(fetch(router.clone(),Method::POST,"synthetic-install",&token,"/api/v1/room-learner/phase/start").await.0,StatusCode::FORBIDDEN,"legacy calibration grant must not enable learner mutations");
    assert_eq!(fetch(router.clone(),Method::POST,"synthetic-install",&token,PATH).await.0,StatusCode::FORBIDDEN);

    // A separate account/installation service does not recognize this device proof.
    let foreign=Companion::load(&dir.path().join("foreign-pair"),"synthetic-install".into(),"e".repeat(64),"https://foreign.local:3443".into()).unwrap();
    assert_eq!(fetch(app(state,foreign),Method::GET,"synthetic-install",&token,PATH).await.0,StatusCode::UNAUTHORIZED);
    let (s,_)=json_call(companion::owner_router(service),Method::DELETE,"/api/v1/companion/devices/phone",serde_json::json!({})).await;assert_eq!(s,StatusCode::OK);
    assert_eq!(fetch(router,Method::GET,"synthetic-install",&token,PATH).await.0,StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn progress_http_private_projection_context_and_privacy_withhold_other_room_data(){
    let (dir,state,publication)=fixture().await;let service=Companion::load(&dir.path().join("pair"),"synthetic-install".into(),"f".repeat(64),"https://synthetic.local:3443".into()).unwrap();let token=pair(service.clone(),"phone").await;let router=app(state.clone(),service);
    let (_,body)=fetch(router.clone(),Method::GET,"synthetic-install",&token,PATH).await;
    let serialized=body.to_string();for secret in ["owner_scope","mutation_guard","trial_id","session_id","frame_ids","sequence_ids","last_accepted_sequence","training_data_sha256","encoder_weights","occupied_probability","heart_rate","amplitudes","geometry"]{assert!(!serialized.contains(secret),"leaked {secret}");}
    let mut changed=room();changed["id"]=serde_json::json!("another-room");assert_eq!(json_call(companion_room::owner_router(publication.clone()),Method::PUT,"/api/v1/companion/room",serde_json::json!({"expected_revision":1,"room":changed})).await.0,StatusCode::OK);
    let (_,mismatch)=fetch(router.clone(),Method::GET,"synthetic-install",&token,PATH).await;assert_eq!(mismatch["room"]["revision"],2);assert_eq!(mismatch["context_match"],false);assert!(mismatch["conditions"].is_null()&&mismatch["phase"].is_null()&&mismatch["sources"].is_null());assert_eq!(mismatch["blocker"],"context_mismatch");
    state.write().await.privacy_mode=true;
    let (s,privacy)=fetch(router,Method::GET,"synthetic-install",&token,PATH).await;assert_eq!(s,StatusCode::FORBIDDEN);assert_eq!(privacy,serde_json::json!({"schema":"ruview.room-learner.progress.v1","success":false,"error_code":"privacy_redacted"}));
}
#[tokio::test]
async fn export_actual_paired_progress_producer_cases(){
    let (dir,state,publication)=fixture().await;let service=Companion::load(&dir.path().join("pair"),"synthetic-install".into(),"f".repeat(64),"https://synthetic.local:3443".into()).unwrap();let token=pair(service.clone(),"phone").await;let router=app(state.clone(),service);
    let mut cases=vec![];
    let (s,p)=fetch(router.clone(),Method::GET,"synthetic-install",&token,PATH).await;assert_eq!(s,StatusCode::OK);cases.push(serde_json::json!({"name":"configured","payload":p}));
    let rt=state.read().await.room_learning.clone().unwrap();
    rt.begin_phase(&rt.mutation_guard().unwrap(),wifi_densepose_sensing_server::room_learner::capture::PhaseRequest{trial_id:"private-operator-trial".into(),session_id:"private-operator-session".into(),role:wifi_densepose_sensing_server::room_learner::observation::DataRole::Train,label:wifi_densepose_sensing_server::room_learner::observation::RoomLabel::Empty,duration_ms:600_000,lead_in_ms:15_000,conditions_confirmed:true},rt.receiver_now_ns()).unwrap();
    let (_,p)=fetch(router.clone(),Method::GET,"synthetic-install",&token,PATH).await;assert_eq!(p["phase"]["stage"],"lead_in");cases.push(serde_json::json!({"name":"preparing_empty","payload":p}));rt.cancel_phase(&rt.mutation_guard().unwrap()).unwrap();
    let mut changed=room();changed["id"]=serde_json::json!("changed-room");assert_eq!(json_call(companion_room::owner_router(publication.clone()),Method::PUT,"/api/v1/companion/room",serde_json::json!({"expected_revision":1,"room":changed})).await.0,StatusCode::OK);let (_,p)=fetch(router.clone(),Method::GET,"synthetic-install",&token,PATH).await;cases.push(serde_json::json!({"name":"context_mismatch","payload":p}));
    state.write().await.privacy_mode=true;let (_,p)=fetch(router,Method::GET,"synthetic-install",&token,PATH).await;cases.push(serde_json::json!({"name":"privacy_redacted","payload":p}));
    if let Ok(path)=std::env::var("RUVIEW_TEST_PAIRED_PROGRESS_OUTPUT") {
        use std::io::Write;use std::os::unix::fs::OpenOptionsExt;
        let path=std::path::PathBuf::from(path);assert!(path.is_absolute()&&path.parent().unwrap().canonicalize().unwrap().starts_with(std::env::temp_dir().canonicalize().unwrap()));
        let mut file=std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).unwrap();
        file.write_all(serde_json::to_string_pretty(&serde_json::json!({"schema":"ruview.test.paired-progress-producer.v1","evidence":"SYNTHETIC_ACTUAL_HTTP_PRODUCER_NOT_PHYSICAL_QUALIFICATION","cases":cases})).unwrap().as_bytes()).unwrap();
    }
}

#[tokio::test]
async fn response_snapshot_cannot_outlive_or_resurrect_an_existing_peer_grant(){
    let (_dir,state,_)=fixture().await;
    let (tx,rx)=tokio::sync::watch::channel(false);
    let now=chrono::Utc::now().timestamp().max(0) as u64;
    let grant=Grant{device_id:"private-phone-id".into(),expires_at:now+8,revoked:rx.clone()};
    let make=|grant:Grant|Router::new().route(PATH,get(room_learner_api::progress)).with_state(state.clone()).layer(axum::Extension(grant));
    let (s,body)=fetch(make(grant.clone()),Method::GET,"synthetic-install","unused",PATH).await;
    assert_eq!(s,StatusCode::OK);assert!(body["expires_at_unix_ms"].as_u64().unwrap()<=grant.expires_at*1000);assert!(body["expires_at_unix_ms"].as_u64().unwrap()<body["observed_at_unix_ms"].as_u64().unwrap()+10000);
    tx.send(true).unwrap();assert_eq!(fetch(make(grant),Method::GET,"synthetic-install","unused",PATH).await.0,StatusCode::UNAUTHORIZED);
    let (_tx,rx)=tokio::sync::watch::channel(false);
    assert_eq!(fetch(make(Grant{device_id:"phone".into(),expires_at:now.saturating_sub(1),revoked:rx}),Method::GET,"synthetic-install","unused",PATH).await.0,StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn older_tls_helpers_can_deny_unknown_paths_before404_so_discovery_is_explicit(){
    // Mirrors the exact old companion policy on an absent progress route. Router.layer
    // applies to fallback too: a bare403 is not evidence of privacy-safe support.
    async fn old_policy(State(service):State<Arc<Companion>>,request:axum::extract::Request,next:Next)->Response {
        if request.uri().path()==PATH {return StatusCode::FORBIDDEN.into_response();}
        require_companion(State(service),request,next).await
    }
    let (dir,state,_)=fixture().await;
    let service=Companion::load(&dir.path().join("pair"),"synthetic-install".into(),"f".repeat(64),"https://synthetic.local:3443".into()).unwrap();let token=pair(service.clone(),"phone").await;
    let legacy=Router::new().route("/api/v1/room-learner/selection",get(|State(s):State<SharedState>|async move {
        let Json(mut v)=room_learner_api::selection(State(s)).await;
        v.as_object_mut().unwrap().remove("progress_schema");Json(v)
    })).with_state(state.clone()).layer(axum::middleware::from_fn_with_state(service.clone(),old_policy));
    assert_eq!(fetch(legacy.clone(),Method::GET,"synthetic-install",&token,PATH).await.0,StatusCode::FORBIDDEN);
    let (status,selection)=fetch(legacy,Method::GET,"synthetic-install",&token,"/api/v1/room-learner/selection").await;
    assert_eq!(status,StatusCode::OK);assert!(selection.get("progress_schema").is_none());assert_eq!(selection["success"],true);
    let new=Router::new().route("/api/v1/room-learner/selection",get(room_learner_api::selection)).with_state(state).layer(axum::middleware::from_fn_with_state(service,require_companion));
    assert_eq!(fetch(new,Method::GET,"synthetic-install",&token,"/api/v1/room-learner/selection").await.1["progress_schema"],"ruview.room-learner.progress.v1");
}
