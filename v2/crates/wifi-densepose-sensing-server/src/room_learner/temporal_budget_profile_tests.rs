//! Synthetic profile/hash compatibility; the fresh constructor remains private.
use super::*;
use super::super::checkpoint;
const OLD_LITERAL:&str="native-receiver-whole-train-period-v1;all-selected-packets;existing-cadence-gap-arrival-skew;confirmed-durable-seals;no-fit-authority";
#[test]
fn historical_constructor_and_seal_hash_keep_exact_bytes(){
    let old=BridgeState::new();assert_eq!(old.policy,OLD_LITERAL);assert_eq!(BRIDGE_POLICY,OLD_LITERAL);
    let expected=format!("{{\"policy\":\"{OLD_LITERAL}\",\"disposition\":\"ready\",\"seals\":[],\"parts\":[]}}");
    assert_eq!(serde_json::to_vec(&old).unwrap(),expected.as_bytes());
    let seal=TrainSeal{episode:0,trial_id:"synthetic-trial".into(),context_sha256:"a".repeat(64),schedule_sha256:"b".repeat(64),row_prefix_sha256:"c".repeat(64),quality_sha256:"d".repeat(64),parts:vec![],confirmed_at_ns:1,previous_sha256:None,sha256:String::new()};
    // Independent frozen tuple golden, preserving the historical domain.
    assert_eq!(seal.digest(TrainBudgetProfile::Historical329).unwrap(),"467e2740fd4a20ed6e09af36071a5b816522c20888936be1e976b3b8f4b4f4c1");
    assert_ne!(seal.digest(TrainBudgetProfile::FreshTrain343).unwrap(),seal.digest(TrainBudgetProfile::Historical329).unwrap());
}
#[test]
fn generic_and_stored_historical_prefixes_remain329_and_byte_roundtrip(){
    let(_dir,_store,mut w)=super::super::tests::setup();
    for bridge in [None,Some(BridgeState::new())]{
        w.bridge=bridge;
        let bytes=checkpoint::encode(&w).unwrap();let prefix=checkpoint::validated_prefix(&bytes,&w.descriptor,w.last_now_ns).unwrap();
        assert_eq!(prefix.transition_limit(),329);
        let restored=checkpoint::decode(&bytes,&w.descriptor,w.last_now_ns).unwrap();assert_eq!(checkpoint::encode(&restored).unwrap(),bytes);
        if w.bridge.is_some(){assert_eq!(prefix.policy_sha256(),hash(&(w.session.plan_sha256(),OLD_LITERAL)).unwrap());}
    }
}
#[test]
fn fresh_train_profile343_is_explicit_and_cannot_upgrade_existing_generation(){
    let(_dir,_store,mut w)=super::super::tests::setup();w.bridge=Some(BridgeState::new());
    let old_bytes=checkpoint::encode(&w).unwrap();let old=checkpoint::validated_prefix(&old_bytes,&w.descriptor,w.last_now_ns).unwrap();
    w.bridge=Some(BridgeState::fresh_train_343());
    let new_bytes=checkpoint::encode(&w).unwrap();let fresh=checkpoint::validated_prefix(&new_bytes,&w.descriptor,w.last_now_ns).unwrap();
    assert_eq!(old.transition_limit(),329);assert_eq!(fresh.transition_limit(),343);assert_ne!(old.policy_sha256(),fresh.policy_sha256());
    assert!(checkpoint::validate_extension(&old,&fresh).is_err());
    assert_eq!(BridgeState::new().policy,OLD_LITERAL,"generic/C4 constructor must not flip");
}
#[test]
fn unknown_stored_profile_and_injected_budget_never_choose_larger_capacity(){
    let(_dir,_store,mut w)=super::super::tests::setup();w.bridge=Some(BridgeState::new());
    w.bridge.as_mut().unwrap().policy="unknown-budget-profile".into();let bytes=checkpoint::encode(&w).unwrap();assert!(checkpoint::validated_prefix(&bytes,&w.descriptor,w.last_now_ns).is_err());
    w.bridge=Some(BridgeState::fresh_train_343());let bytes=checkpoint::encode(&w).unwrap();let mut v:serde_json::Value=serde_json::from_slice(&bytes).unwrap();v["payload"]["transition_limit"]=serde_json::json!(512);assert!(checkpoint::decode(&serde_json::to_vec(&v).unwrap(),&w.descriptor,w.last_now_ns).is_err());
}
