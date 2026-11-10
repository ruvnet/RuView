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

fn c5_datagram(payload: &[u8], first_word_invalid: bool) -> Vec<u8> {
    let mut packet = vec![0;20];
    packet[..4].copy_from_slice(&0xC5110001u32.to_le_bytes());
    packet[4]=20;packet[5]=1;packet[6..8].copy_from_slice(&53u16.to_le_bytes());
    packet[8..12].copy_from_slice(&5180u32.to_le_bytes());
    packet[12..16].copy_from_slice(&23u32.to_le_bytes());
    packet[18]=0;packet[19]=if first_word_invalid {0x20}else{0};
    packet.extend(payload);
    packet.extend([b'C',b'5',b'L',b'8',1,8,1,1,1,1,0,20,1,u8::from(first_word_invalid),106,0]);
    packet
}
fn c5_input(payload: &[u8], sanitized: bool) -> NativePacketInput<'_> {
    NativePacketInput {node_id:20,n_antennas:1,raw_bins:53,frequency_hz:5_180_000_000,
        ppdu_type:PpduType::HtLegacy,sequence:23,received_at_ns:1_234_000_000,
        receiver_sample_rate_hz:10.,iq_bytes:payload,
        flags:Adr018Flags{first_word_sanitized:sanitized,..Adr018Flags::default()},
        provenance:CaptureProvenance::Synthetic}
}
#[test]
fn c5_lltf8_claim_retains_original_bytes_slots_order_dc_mask_and_feature_identity() {
    let payload:Vec<u8>=(0..53).flat_map(|n|[(n as i8-30) as u8,(53-n as i8) as u8]).collect();
    let wire=c5_datagram(&payload,false);let i=c5_input(&payload,false);
    let format=validate_adr018_native_format(&wire,1,53,i.ppdu_type,i.flags).unwrap();
    let packet=canonicalize_esp32_with_format(i,format).unwrap();
    assert_eq!(packet.native_iq_wire.as_deref(),Some(payload.as_slice()));
    assert_eq!(packet.native_frame.native_shape,vec![1,53,1]);
    for (native,raw) in packet.native_frame.native_iq.iter().zip(payload.chunks_exact(2)) {
        assert_eq!((native.im,native.re),(f64::from(raw[0] as i8),f64::from(raw[1] as i8)));
    }
    assert!(!packet.native_frame.valid_mask[0]);
    assert!(packet.native_frame.valid_mask[1..].iter().all(|x|*x));
    assert_eq!(packet.native_tone_indices.unwrap(),&C5_LLTF53_TONES);
    assert_eq!(&C5_LLTF53_TONES[..27],&(0..=26).collect::<Vec<i16>>());
    assert_eq!(&C5_LLTF53_TONES[27..],&(-26..=-1).collect::<Vec<i16>>());
    let magnitudes:Vec<_>=payload.chunks_exact(2).skip(2).map(|p|
        f64::from(p[0] as i8).hypot(f64::from(p[1] as i8))).collect();
    assert_eq!(packet.amplitudes,HardwareNormalizer::new().resample_magnitude_to_canonical(&magnitudes).unwrap());
    assert_eq!(packet.grid_id,C5_LLTF8_V1_GRID);
    assert_eq!((packet.frequency_hz,packet.bandwidth_hz),(5_180_000_000,20_000_000));
    assert!(!packet.phase_consumed && !packet.temporal_sample_rate_known && !packet.signal_quality_known);
    assert_eq!(packet.clock.kind,ClockKind::ReceiverOnly);
    assert_eq!(packet.native_frame.provenance.class,ProvenanceClass::Synthetic);
    assert!(packet.native_frame.transmitter_pose.is_none() && packet.native_frame.receiver_pose.is_none());
    let public=serde_json::to_string(&packet.summary()).unwrap();let debug=format!("{packet:?}");
    for raw in ["native_iq","native_iq_wire","native_tone_indices","amplitudes","valid_mask"] {
        assert!(!public.contains(raw) && !debug.contains(raw));
    }
}
#[test]
fn c5_lltf8_invalid_first_word_keeps_two_masked_complex_slots_without_renumbering() {
    let payload=bytes(53,-3,4);let mut sanitized=payload.clone();sanitized[..4].fill(0);
    let wire=c5_datagram(&sanitized,true);let i=c5_input(&sanitized,true);
    let format=validate_adr018_native_format(&wire,1,53,i.ppdu_type,i.flags).unwrap();
    let packet=canonicalize_esp32_with_format(i,format).unwrap();
    assert_eq!(&packet.native_frame.valid_mask[..2],&[false,false]);
    assert!(packet.native_frame.valid_mask[2..].iter().all(|x|*x));
    assert_eq!(packet.native_iq_wire.as_deref(),Some(sanitized.as_slice()));
    assert_eq!(packet.native_frame.native_iq.len(),53);
    assert!(packet.amplitudes.iter().all(|x|*x==5.));
    assert_eq!(packet.grid_id,C5_LLTF8_V1_GRID);
}
#[test]
fn c5_lltf8_missing_wrong_or_ambiguous_claim_never_becomes_a_supported_grid() {
    let payload=bytes(53,-3,4);let wire=c5_datagram(&payload,false);let i=c5_input(&payload,false);
    assert!(grid_id(1,53,PpduType::HtLegacy).is_err());
    assert!(canonicalize_esp32(c5_input(&payload,false)).is_err());
    assert!(validate_adr018_native_format(&wire[..126],1,53,i.ppdu_type,i.flags).is_err());
    for field in 0..16 {
        let mut changed=wire.clone();changed[126+field]^=1;
        assert!(validate_adr018_native_format(&changed,1,53,i.ppdu_type,i.flags).is_err(),"field {field}");
    }
    let mut trailing=wire.clone();trailing.push(0);
    assert!(validate_adr018_native_format(&trailing,1,53,i.ppdu_type,i.flags).is_err());
    for ppdu in [PpduType::HeSu,PpduType::HeMu,PpduType::HeTb,PpduType::Unknown] {
        assert!(validate_adr018_native_format(&wire,1,53,ppdu,i.flags).is_err());
    }
    assert!(validate_adr018_native_format(&wire,1,53,i.ppdu_type,Adr018Flags{bw40:true,..i.flags}).is_err());
    assert!(validate_adr018_native_format(&wire,2,53,i.ppdu_type,i.flags).is_err());
    for bit in [0x02,0x04,0x08,0x10,0x40,0x80] {
        let mut changed=wire.clone();changed[19]|=bit;
        assert!(validate_adr018_native_format(&changed,1,53,i.ppdu_type,Adr018Flags::from_byte(changed[19])).is_err());
    }
    for mhz in [2437u32,5185,5470,5730,5955] {
        let mut changed=wire.clone();changed[8..12].copy_from_slice(&mhz.to_le_bytes());
        assert!(validate_adr018_native_format(&changed,1,53,i.ppdu_type,i.flags).is_err());
    }
    let mut channel112=wire.clone();channel112[8..12].copy_from_slice(&5560u32.to_le_bytes());
    assert!(validate_adr018_native_format(&channel112,1,53,i.ppdu_type,i.flags).is_ok());
}
#[test]
fn c5_lltf8_format_token_cannot_bypass_exact_iq_width_or_supported_frequency() {
    let payload=bytes(53,-3,4);let wire=c5_datagram(&payload,false);let i=c5_input(&payload,false);
    let format=validate_adr018_native_format(&wire,1,53,i.ppdu_type,i.flags).unwrap();
    for size in [0,1,104,105,107,108,128] {
        let wrong=vec![1;size];assert!(canonicalize_esp32_with_format(c5_input(&wrong,false),format).is_err());
    }
    for frequency in [0,5_180_000_001,8_000_000_000] {
        let mut input=c5_input(&payload,false);input.frequency_hz=frequency;
        assert!(canonicalize_esp32_with_format(input,format).is_err());
    }
    let mut foreign=c5_input(&payload,false);foreign.node_id=21;
    assert!(canonicalize_esp32_with_format(foreign,format).is_err());
    let mut foreign=c5_input(&payload,false);foreign.sequence+=1;
    assert!(canonicalize_esp32_with_format(foreign,format).is_err());
    let mut foreign=c5_input(&payload,false);foreign.frequency_hz=5_200_000_000;
    assert!(canonicalize_esp32_with_format(foreign,format).is_err());
    let mut foreign=c5_input(&payload,false);foreign.flags.ieee802154_sync_valid=true;
    assert!(canonicalize_esp32_with_format(foreign,format).is_err());
}
