//! SYNTHETIC engine integration checks; no model or RF hardware is invoked.
#![cfg(feature="pan2-physical")]
use wifi_densepose_engine::{StreamingEngine,physical_forecast::*};
use wifi_densepose_bfld::PrivacyMode;
use wifi_densepose_geo::types::GeoRegistration;
use wifi_densepose_worldgraph::{WorldId,WorldNode};
use wifi_densepose_worldmodel::{PrivacyDecision,RetentionPolicy};

fn config()->PhysicalForecastConfig{PhysicalForecastConfig{enabled:true,allow_fixtures:true}}
fn setup(mode:PrivacyMode)->(StreamingEngine,WorldId,WorldId){
    let mut engine=StreamingEngine::new(mode,1,GeoRegistration::default());
    let room=engine.add_room("synthetic-room","Synthetic Room");let sensor=engine.add_sensor("synthetic-sensor",room);
    engine.update_person_track(7,0.0,0.0,room,sensor);
    (engine,room,sensor)
}
fn context()->PhysicalContext{PhysicalContext{source_epoch:1,observed_at_ms:1000,frame_id:"synthetic-enu".into(),calibration_id:"synthetic-survey-1".into(),action:"approach doorway".into(),horizon_ms:2000,generation_seed:123,
    bounds:PhysicalBounds{min:[-5.0,-5.0,-1.0],max:[5.0,5.0,3.0]},obstacles:vec![],max_speed_mps:3.0,
    privacy:PrivacyDecision{projection_id:"caller-supplied".into(),hosted_processing_approved:true,redactions_applied:1,retention:RetentionPolicy::Ephemeral}}}
fn response(ticket:&PhysicalForecastTicket)->Vec<u8>{serde_json::to_vec(&serde_json::json!({"schema_version":1,"request_sha256":ticket.scene().request_sha256().unwrap(),"model_sha256":"a".repeat(64),"artifact_sha256":"b".repeat(64),"extractor_version":"synthetic-fixture/1","frame_id":"synthetic-enu","calibration_id":"synthetic-survey-1","execution":"fixture","trajectories":[{"track_id":7,"points":[{"offset_ms":1000,"position_m":[1.0,0.0,0.0]},{"offset_ms":2000,"position_m":[2.0,0.0,0.0]}]}]})).unwrap()}

#[test]
fn default_off_and_anonymous_privacy_modes_reject(){
    for mode in [PrivacyMode::PrivateHome,PrivacyMode::EnterpriseAnonymous,PrivacyMode::StrictNoIdentity] {
        let(engine,_,_)=setup(mode);let before=engine.snapshot_json().unwrap();
        assert!(engine.prepare_physical_forecast(config(),&[7],context()).is_err());
        assert_eq!(before,engine.snapshot_json().unwrap());
    }
    let(engine,_,_)=setup(PrivacyMode::RawResearch);
    assert!(engine.prepare_physical_forecast(PhysicalForecastConfig::default(),&[7],context()).is_err());
    assert!(engine.prepare_physical_forecast(config(),&[99],context()).is_err());
    assert!(engine.prepare_physical_forecast(config(),&[7,7],context()).is_err());
}
#[test]
fn replay_returns_separate_hypotheses_and_preserves_live_updates(){
    for mode in [PrivacyMode::RawResearch,PrivacyMode::CareWithConsent] {
        let(mut engine,room,sensor)=setup(mode);
        let ticket=engine.prepare_physical_forecast(config(),&[7],context()).unwrap();
        assert!(!ticket.scene().request().context.privacy.hosted_processing_approved);
        assert!(ticket.scene().request().context.privacy.projection_id.starts_with("ruview:"));
        let track=engine.update_person_track(7,0.5,0.0,room,sensor);
        let live=engine.snapshot_json().unwrap();
        let result=engine.replay_physical_forecast(config(),&ticket,&response(&ticket),1500).unwrap();
        assert_eq!(live,engine.snapshot_json().unwrap());
        assert_eq!(result.hypothesis_graph().node_count(),engine.world().node_count()+1);
        assert_eq!(result.source_cycle(),ticket.source_cycle());
        match engine.world().node(track).unwrap(){WorldNode::PersonTrack{last_position,..}=>assert_eq!(last_position.east_m,0.5),_=>panic!("track disappeared")}
        match result.hypothesis_graph().node(track).unwrap(){WorldNode::PersonTrack{last_position,..}=>assert_eq!(last_position.east_m,0.0),_=>panic!("source track disappeared")}
    }
}
#[test]
fn expiry_fixture_and_policy_changes_fail_closed(){
    let(mut engine,_,_)=setup(PrivacyMode::CareWithConsent);
    let ticket=engine.prepare_physical_forecast(config(),&[7],context()).unwrap();let bytes=response(&ticket);
    for now in [999,3001] {assert!(engine.replay_physical_forecast(config(),&ticket,&bytes,now).is_err());}
    assert!(engine.replay_physical_forecast(PhysicalForecastConfig{enabled:true,allow_fixtures:false},&ticket,&bytes,1500).is_err());
    engine.set_privacy_mode(PrivacyMode::PrivateHome);
    assert!(engine.replay_physical_forecast(config(),&ticket,&bytes,1500).is_err());
    engine.set_privacy_mode(PrivacyMode::CareWithConsent);
    assert!(engine.replay_physical_forecast(config(),&ticket,&bytes,1500).is_err());
}
#[test]
fn wrong_request_and_obstacle_collision_reject(){
    let(engine,_,_)=setup(PrivacyMode::RawResearch);let mut ctx=context();
    ctx.obstacles.push(PhysicalBounds{min:[0.4,-0.5,-1.0],max:[0.6,0.5,1.0]});
    let ticket=engine.prepare_physical_forecast(config(),&[7],ctx).unwrap();
    assert!(engine.replay_physical_forecast(config(),&ticket,&response(&ticket),1500).is_err());
    let ticket=engine.prepare_physical_forecast(config(),&[7],context()).unwrap();
    let mut wrong:serde_json::Value=serde_json::from_slice(&response(&ticket)).unwrap();wrong["request_sha256"]=serde_json::json!("f".repeat(64));
    assert!(engine.replay_physical_forecast(config(),&ticket,&serde_json::to_vec(&wrong).unwrap(),1500).is_err());
}
