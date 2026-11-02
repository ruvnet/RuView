//! SYNTHETIC wire payloads through the real native and signal contracts.
//! These test contract behavior, never sensing accuracy or real hardware.
use super::*;

fn bytes(bins:usize,i:i8,q:i8)->Vec<u8> {
    (0..bins).flat_map(|_|[i as u8,q as u8]).collect()
}
fn input(payload:&[u8],bins:usize)->NativePacketInput<'_> {
    NativePacketInput {node_id:7,n_antennas:1,raw_bins:bins,frequency_hz:2_437_000_000,
        ppdu_type:if bins==64 {PpduType::HtLegacy} else {PpduType::HeSu},sequence:23,
        received_at_ns:1_234_000_000,receiver_sample_rate_hz:42.0,iq_bytes:payload,flags:Adr018Flags::default(),
        provenance:CaptureProvenance::Synthetic}
}

#[test]
fn ht64_packet_retains_original_signed_iq_phase_mask_and_unknown_geometry() {
    let payload=bytes(64,-3,4);
    let packet=canonicalize_esp32(input(&payload,64)).unwrap();
    assert_eq!(packet.native_frame.native_shape,vec![1,64,1]);
    assert_eq!(packet.native_frame.axes,vec![FieldAxis::Antenna,FieldAxis::Frequency,FieldAxis::Time]);
    assert!(packet.native_frame.native_iq.iter().all(|z|z.re==-3.0 && z.im==4.0));
    assert_eq!(packet.native_frame.native_iq[0].arg(),4.0f64.atan2(-3.0));
    assert!(packet.native_frame.valid_mask.iter().all(|v|*v));
    assert!(packet.native_frame.transmitter_pose.is_none() && packet.native_frame.receiver_pose.is_none()
        && packet.native_frame.antenna_geometry.is_empty());
    assert_eq!(packet.grid_id,"ant1-bins64-ppdu0-usable-drop2-linear-v2");
}
#[test]
fn he256_packet_uses_actual_frame_contract_and_existing_56_bin_projection() {
    let payload=bytes(256,6,8);
    let packet=canonicalize_esp32(input(&payload,256)).unwrap();
    let direct=HardwareNormalizer::new().resample_magnitude_to_canonical(&vec![10.0;254]).unwrap();
    assert_eq!(packet.amplitudes,direct);
    assert_eq!(packet.native_frame.native_shape,vec![1,256,1]);
    assert_eq!(packet.grid_id,"ant1-bins256-ppdu1-usable-drop2-linear-v2");
    assert_eq!(packet.amplitudes.len(),56);
}
#[test]
fn gain_and_dc_levels_survive_canonicalization_without_median_or_zscore() {
    let low=bytes(64,3,4);let high=bytes(64,9,12);
    let a=canonicalize_esp32(input(&low,64)).unwrap();
    let b=canonicalize_esp32(input(&high,64)).unwrap();
    assert!(a.amplitudes.iter().all(|v|*v==5.0));
    assert!(b.amplitudes.iter().all(|v|*v==15.0));
    assert_eq!(a.native_frame.native_iq[0],Complex64::new(3.0,4.0));
}
#[test]
fn varying_native_iq_is_not_reconstructed_from_magnitude_or_overwritten() {
    let payload:Vec<_>=(0..64).flat_map(|n|[(n as i8-32) as u8,(-(n as i8)/2) as u8]).collect();
    let packet=canonicalize_esp32(input(&payload,64)).unwrap();
    for (sample,pair) in packet.native_frame.native_iq.iter().zip(payload.chunks_exact(2)) {
        assert_eq!((sample.re,sample.im),(f64::from(pair[0] as i8),f64::from(pair[1] as i8)));
    }
}
#[test]
fn raw_phase_is_retained_but_amplitude_learner_consumes_no_phase_or_source_clock() {
    let payload=bytes(64,3,4);let mut input=input(&payload,64);
    input.flags.ieee802154_sync_valid=true;
    let packet=canonicalize_esp32(input).unwrap();
    assert_eq!(packet.native_frame.calibration.phase_state,PhaseState::Raw);
    assert!(!packet.phase_consumed && !packet.temporal_sample_rate_known && !packet.signal_quality_known);
    assert_eq!(packet.clock,ClockInfo {kind:ClockKind::ReceiverOnly,uncertainty_ns:u64::MAX});
    assert_eq!(packet.native_frame.timestamp_ns,1_234_000_000);
    assert_eq!(packet.native_frame.calibration.confidence,0.0);
    assert!(!packet.native_frame.calibration.gain_calibrated);
}
#[test]
fn provenance_never_grants_lab_or_physical_qualification() {
    for provenance in [CaptureProvenance::Synthetic,CaptureProvenance::CapturedReplay,CaptureProvenance::LiveReceiverObserved] {
        let payload=bytes(64,3,4);let mut input=input(&payload,64);input.provenance=provenance;
        let packet=canonicalize_esp32(input).unwrap();
        let expected=if provenance==CaptureProvenance::Synthetic {(ProvenanceClass::Synthetic,EvidenceLevel::L0Simulation)}
            else {(ProvenanceClass::Measured,EvidenceLevel::L1CapturedReplay)};
        assert_eq!((packet.native_frame.provenance.class,packet.native_frame.provenance.evidence),expected);
        assert_eq!(packet.provenance,provenance);
        assert_eq!(packet.clock.kind,ClockKind::ReceiverOnly);
    }
}
#[test]
fn public_summary_and_debug_never_export_native_samples_amplitudes_or_frame_ids() {
    let payload=bytes(64,-3,4);let packet=canonicalize_esp32(input(&payload,64)).unwrap();
    let public=serde_json::to_string(&packet.summary()).unwrap();let debug=format!("{packet:?}");
    for sensitive in ["native_iq","amplitudes","samples","frame_id","valid_mask"] {
        assert!(!public.contains(sensitive));assert!(!debug.contains(sensitive));
    }
    assert!(public.contains("ReceiverOnly") && public.contains("\"phase_consumed\":false"));
}
#[test]
fn malformed_grid_payload_bandwidth_frequency_and_validity_fail_closed() {
    for change in 0..11 {
        let payload=bytes(256,3,4);let mut i=input(&payload[..128],64);
        match change {0=>i.n_antennas=0,1=>i.n_antennas=2,2=>i.raw_bins=usize::MAX,
            3=>i.raw_bins=128,4=>i.ppdu_type=PpduType::Unknown,5=>i.ppdu_type=PpduType::HeSu,
            6=>i.flags.bw40=true,7=>i.frequency_hz=0,8=>i.frequency_hz=2_437_000_001,
            9=>i.received_at_ns=0,_=>i.iq_bytes=&payload}
        assert!(canonicalize_esp32(i).is_err(),"survived input mutant {change}");
    }
}
#[test]
fn iq_wire_integer_domain_cannot_produce_nan_or_inf_native_samples() {
    let payload:Vec<_>=(0u8..=255).cycle().take(512).collect();
    let packet=canonicalize_esp32(input(&payload,256)).unwrap();
    assert!(packet.native_frame.native_iq.iter().all(|z|z.re.is_finite() && z.im.is_finite()));
    assert!(packet.amplitudes.iter().all(|x|x.is_finite()));
}
#[test]
fn exact_declared_packet_size_is_required_without_padding_or_trailing_data() {
    for size in [0,1,127,129,1024] {
        let payload=vec![1;size];
        assert!(canonicalize_esp32(input(&payload,64)).is_err());
    }
}
#[test]
fn actual_raw_grid_identity_survives_both_supported_families() {
    for ppdu in [PpduType::HtLegacy,PpduType::HeSu,PpduType::HeMu,PpduType::HeTb] {
        let bins=if ppdu==PpduType::HtLegacy {64} else {256};let payload=bytes(bins,3,4);
        let mut i=input(&payload,bins);i.ppdu_type=ppdu;
        let packet=canonicalize_esp32(i).unwrap();
        assert_eq!(packet.grid_id,format!("ant1-bins{bins}-ppdu{}-usable-drop2-linear-v2",ppdu.to_byte()));
        assert_eq!((packet.frequency_hz,packet.bandwidth_hz),(2_437_000_000,20_000_000));
    }
}

#[test]
fn sanitized_word_is_retained_with_two_invalid_samples_and_fixed_projection() {
    let original=bytes(256,3,4);let mut sanitized=original.clone();sanitized[..4].fill(0);
    let normal=canonicalize_esp32(input(&original,256)).unwrap();
    let mut i=input(&sanitized,256);i.flags.first_word_sanitized=true;
    let masked=canonicalize_esp32(i).unwrap();
    assert_eq!(normal.amplitudes,masked.amplitudes);
    assert_eq!(normal.grid_id,masked.grid_id);
    assert_eq!(&masked.native_frame.valid_mask[..2],&[false,false]);
    assert!(masked.native_frame.valid_mask[2..].iter().all(|v|*v));
    assert_eq!(masked.native_frame.native_iq[0],Complex64::new(0.0,0.0));
    assert_eq!(normal.native_frame.native_iq[0],Complex64::new(3.0,4.0));
}
#[test]
fn source_record_requires_actual_receiver_rate_and_never_invents_one_hz() {
    let payload=bytes(64,3,4);
    for rate in [0.0,-1.0,f64::NAN,f64::INFINITY,f64::NEG_INFINITY] {
        let mut i=input(&payload,64);i.receiver_sample_rate_hz=rate;
        assert!(canonicalize_esp32(i).is_err());
    }
    let packet=canonicalize_esp32(input(&payload,64)).unwrap();
    assert_eq!(packet.native_frame.sample_rate_hz,42.0);
    assert_eq!(packet.receiver_sample_rate_hz,42.0);
    assert!(!packet.temporal_sample_rate_known);
}

#[test]
fn valid_native_spectral_nulls_are_admitted_without_negative_projected_magnitudes() {
    for bins in [64, 256] {
        let payload: Vec<u8> = (0..bins)
            .flat_map(|bin| [if bin % 8 == 0 { 100 } else { 0 }, 0])
            .collect();
        let result = canonicalize_esp32(input(&payload, bins));
        assert!(
            result.is_ok(),
            "valid {bins}-bin spectral nulls must not be rejected by interpolation: {result:?}"
        );
        let packet = result.unwrap();
        assert!(packet
            .amplitudes
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0 && *v <= 100.0));
        for (actual, original) in packet
            .native_frame
            .native_iq
            .iter()
            .zip(payload.chunks_exact(2))
        {
            assert_eq!(actual.re, f64::from(original[0] as i8));
            assert_eq!(actual.im, 0.0);
        }
    }
}
