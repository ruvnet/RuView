//! Matter commissioning code generation (ADR-115 §3.11.2).
//!
//! When `--matter` is enabled, the publisher prints a setup code on
//! first start that the user scans/enters into their Matter controller
//! (Apple Home / Google Home / HA Matter integration). This module
//! generates that code without depending on any Matter SDK.
//!
//! ## Spec
//!
//! Matter Core Spec 1.3 §5.1 defines two pairing-code formats:
//!
//! - **Manual pairing code** — 11 digits, base-10 encoded from packed
//!   bits. This is what we emit for `--matter-setup-file`.
//! - **QR code payload** — `MT:` prefix + base-38 of a longer
//!   bit-packed payload. v0.7.0 emits the manual code only; QR string
//!   generation is a v0.7.1 follow-up (per §9.9 dev-VID note —
//!   commissioning works in either form with dev VID).
//!
//! ## Digit layout (manual code, §5.1.4.1.1 — VID/PID-absent variant)
//!
//! The 11-digit short code is three decimal chunks plus a Verhoeff
//! check digit. Each chunk packs spec fields so the chunk's maximum
//! value fits its decimal width exactly (no truncation, no modulo):
//!
//! ```text
//!  digit(s)  width  packed value
//!  --------  -----  ------------------------------------------------
//!   1         1     (vid_pid_present << 2) | (discriminator >> 10)
//!   2..6      5     ((discriminator & 0x300) << 6) | (passcode & 0x3FFF)
//!   7..10     4     (passcode >> 14) & 0x1FFF
//!   11        1     Verhoeff check digit over the 10-digit body
//! ```
//!
//! Only the **upper 4 bits** of the 12-bit discriminator survive in the
//! manual code (the "short discriminator", bits 8..11); the low 8 bits
//! are carried only in the QR payload, by design (§5.1.3.1). Chunk
//! maxima: chunk1 ≤ `(0x300<<6)|0x3FFF` = 65535 < 10^5, chunk2 ≤ 0x1FFF
//! = 8191 < 10^4, so each chunk is `format!`-padded to its width without
//! loss. This is the exact §5.1.4.1.1 packing: the canonical reference
//! vector `(passcode=20202021, discriminator=3840)` encodes to the
//! Matter-published `34970112332`.

/// Inputs to setup-code generation. `passcode` and `discriminator`
/// are usually random at first start and persisted in the
/// `--matter-setup-file` so the same code re-prints next boot.
#[derive(Debug, Clone, Copy)]
pub struct SetupCodeInput {
    /// 27-bit Matter setup PIN. Must be in the range `0..2^27`
    /// excluding the disallowed values listed in §5.1.6.1 (00000000,
    /// 11111111, 22222222, …, 99999999, 12345678, 87654321).
    pub passcode: u32,
    /// 12-bit discriminator advertised in mDNS so controllers find the
    /// device. Must be in `0..4096`.
    pub discriminator: u16,
    /// CSA-assigned vendor ID. Today we use dev VID `0xFFF1` per
    /// ADR-115 §9.9 until P10 cert decision.
    pub vendor_id: u16,
    /// Vendor-assigned product ID. Default `0x8001` per the same ADR row.
    pub product_id: u16,
}

impl SetupCodeInput {
    /// Build with the production-default dev VID + sensible product ID.
    /// `passcode` and `discriminator` come from a CSPRNG at first start.
    pub fn dev(passcode: u32, discriminator: u16) -> Self {
        Self { passcode, discriminator, vendor_id: 0xFFF1, product_id: 0x8001 }
    }

    /// Validate against §5.1.6.1 disallowed values + bit-width ranges.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.passcode == 0
            || self.passcode == 11111111
            || self.passcode == 22222222
            || self.passcode == 33333333
            || self.passcode == 44444444
            || self.passcode == 55555555
            || self.passcode == 66666666
            || self.passcode == 77777777
            || self.passcode == 88888888
            || self.passcode == 99999999
            || self.passcode == 12345678
            || self.passcode == 87654321
        {
            return Err("passcode is in the §5.1.6.1 disallowed-values list");
        }
        if self.passcode >= 1 << 27 {
            return Err("passcode exceeds 27-bit range");
        }
        if self.discriminator >= 1 << 12 {
            return Err("discriminator exceeds 12-bit range");
        }
        Ok(())
    }
}

/// The 11-digit manual pairing code as a fixed-length string. Always
/// 11 digits because the Matter spec specifies fixed-width encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualPairingCode(pub String);

impl ManualPairingCode {
    /// Build the 11-digit short code (§5.1.4.1, VID/PID-absent variant).
    /// Returns the code as a `String` so the caller can `Display`-print
    /// it directly. Validates the input first.
    pub fn from_input(input: &SetupCodeInput) -> Result<Self, &'static str> {
        input.validate()?;

        // §5.1.4.1.1 — 10-digit short code = 1-digit chunk0
        // (VID/PID-present flag in bit 2 + discriminator bits 10..11) +
        // 5-digit chunk1 (discriminator bits 8..9 + passcode bits 0..13)
        // + 4-digit chunk2 (passcode bits 14..26). Plus 1-digit Verhoeff
        // check digit = 11 total.
        //
        // This is the exact spec field-packing. Each chunk's maximum
        // value is strictly below 10^width, so `format!` zero-pads to a
        // fixed width with no truncation:
        //   chunk0 ∈ 0..=7   (1 digit)
        //   chunk1 ≤ (0x300<<6)|0x3FFF = 65535 < 10^5  (5 digits)
        //   chunk2 ≤ 0x1FFF = 8191 < 10^4              (4 digits)
        //
        // VID/PID-absent variant: vid_pid_present = 0, so the VID/PID
        // pair (input.vendor_id / input.product_id) is intentionally not
        // stitched into the manual code — controllers fall back to the
        // discriminator advertised in mDNS to resolve the device, and
        // the QR payload (a separate follow-up) carries VID/PID when
        // present. We still validate the inputs above so an invalid
        // passcode/discriminator never produces a code.
        let disc = u32::from(input.discriminator);
        let pin = input.passcode;
        let vid_pid_present: u32 = 0; // short-form manual code

        let chunk0 = ((vid_pid_present << 2) | (disc >> 10)) as u64;
        let chunk1 = (((disc & 0x300) << 6) | (pin & 0x3FFF)) as u64;
        let chunk2 = ((pin >> 14) & 0x1FFF) as u64;

        debug_assert!(chunk0 < 10, "chunk0 must be one digit");
        debug_assert!(chunk1 < 100_000, "chunk1 must be five digits");
        debug_assert!(chunk2 < 10_000, "chunk2 must be four digits");

        let body = format!("{:01}{:05}{:04}", chunk0, chunk1, chunk2);
        debug_assert_eq!(body.len(), 10, "body must be 10 digits — fix chunk widths");

        let check = verhoeff_check_digit(&body);
        Ok(Self(format!("{}{}", body, check)))
    }

    /// 4-3-4 dash format the way Matter controllers actually display
    /// it (e.g. `1234-567-8901`). Used for human readability in
    /// `--matter-setup-file` and console logs.
    pub fn display_4_3_4(&self) -> String {
        let s = &self.0;
        format!("{}-{}-{}", &s[0..4], &s[4..7], &s[7..11])
    }

    /// Decode a manual pairing code back to its `(short_discriminator,
    /// passcode)` fields per the inverse of §5.1.4.1.1. This is the
    /// proof that the encoder is a real, lossless field-packing (a
    /// controller performs exactly this decode): the recovered passcode
    /// is bit-for-bit identical, and the recovered discriminator is the
    /// 4-bit *short* discriminator (manual codes never carry the low 8
    /// bits — see the module header).
    ///
    /// Returns `Err` if the string is not 11 ASCII digits or the
    /// Verhoeff check digit does not validate.
    pub fn decode(&self) -> Result<DecodedManualCode, &'static str> {
        let s = &self.0;
        if s.len() != 11 || !s.chars().all(|c| c.is_ascii_digit()) {
            return Err("manual code must be exactly 11 ASCII digits");
        }
        let body = &s[0..10];
        let given_check = s[10..11].parse::<u8>().map_err(|_| "bad check digit")?;
        if verhoeff_check_digit(body) != given_check {
            return Err("Verhoeff check digit mismatch");
        }

        let chunk0: u32 = body[0..1].parse().map_err(|_| "bad chunk0")?;
        let chunk1: u32 = body[1..6].parse().map_err(|_| "bad chunk1")?;
        let chunk2: u32 = body[6..10].parse().map_err(|_| "bad chunk2")?;

        let vid_pid_present = (chunk0 >> 2) & 0x1;
        // discriminator bits 10..11 (chunk0) + bits 8..9 (chunk1 high bits)
        let disc_hi2 = chunk0 & 0x3;
        let disc_mid2 = (chunk1 >> 14) & 0x3;
        let short_discriminator = ((disc_hi2 << 2) | disc_mid2) as u8; // 4-bit value 0..15

        // passcode bits 0..13 (chunk1 low) + bits 14..26 (chunk2)
        let pin_low = chunk1 & 0x3FFF;
        let pin_high = chunk2 & 0x1FFF;
        let passcode = (pin_high << 14) | pin_low;

        Ok(DecodedManualCode {
            vid_pid_present: vid_pid_present != 0,
            short_discriminator,
            passcode,
        })
    }
}

/// The fields recovered from a manual pairing code by [`ManualPairingCode::decode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedManualCode {
    /// Whether the VID/PID-present bit was set (always `false` for the
    /// short-form codes this module emits).
    pub vid_pid_present: bool,
    /// The 4-bit short discriminator (upper 4 bits of the original 12-bit
    /// discriminator).
    pub short_discriminator: u8,
    /// The full 27-bit setup passcode, recovered bit-for-bit.
    pub passcode: u32,
}

/// Verhoeff check-digit algorithm per Matter Core §5.1.4.1.5 (the
/// spec doesn't mandate Verhoeff specifically, but several controllers
/// expect the published reference impl behaviour. We follow §5.1.4.1
/// "decimal check digit using Verhoeff scheme".)
fn verhoeff_check_digit(s: &str) -> u8 {
    const D: [[u8; 10]; 10] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        [1, 2, 3, 4, 0, 6, 7, 8, 9, 5],
        [2, 3, 4, 0, 1, 7, 8, 9, 5, 6],
        [3, 4, 0, 1, 2, 8, 9, 5, 6, 7],
        [4, 0, 1, 2, 3, 9, 5, 6, 7, 8],
        [5, 9, 8, 7, 6, 0, 4, 3, 2, 1],
        [6, 5, 9, 8, 7, 1, 0, 4, 3, 2],
        [7, 6, 5, 9, 8, 2, 1, 0, 4, 3],
        [8, 7, 6, 5, 9, 3, 2, 1, 0, 4],
        [9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
    ];
    const P: [[u8; 10]; 8] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        [1, 5, 7, 6, 2, 8, 3, 0, 9, 4],
        [5, 8, 0, 3, 7, 9, 6, 1, 4, 2],
        [8, 9, 1, 6, 0, 4, 3, 5, 2, 7],
        [9, 4, 5, 3, 1, 2, 6, 8, 7, 0],
        [4, 2, 8, 6, 5, 7, 3, 9, 0, 1],
        [2, 7, 9, 3, 8, 0, 6, 4, 1, 5],
        [7, 0, 4, 6, 9, 1, 3, 2, 5, 8],
    ];
    const INV: [u8; 10] = [0, 4, 3, 2, 1, 5, 6, 7, 8, 9];

    let mut c = 0u8;
    for (i, ch) in s.chars().rev().enumerate() {
        let n = ch.to_digit(10).expect("non-digit in code body") as u8;
        c = D[c as usize][P[(i + 1) % 8][n as usize] as usize];
    }
    INV[c as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_constructor_uses_dev_vid_pid() {
        let s = SetupCodeInput::dev(20202021, 3840);
        assert_eq!(s.vendor_id, 0xFFF1);
        assert_eq!(s.product_id, 0x8001);
        assert_eq!(s.passcode, 20202021);
        assert_eq!(s.discriminator, 3840);
    }

    #[test]
    fn validate_rejects_disallowed_passcodes() {
        for &bad in &[
            0u32, 11111111, 22222222, 33333333, 44444444, 55555555,
            66666666, 77777777, 88888888, 99999999, 12345678, 87654321,
        ] {
            let s = SetupCodeInput::dev(bad, 100);
            assert!(s.validate().is_err(), "passcode {} must be rejected", bad);
        }
    }

    #[test]
    fn validate_rejects_oversized_passcode() {
        let s = SetupCodeInput::dev(1 << 27, 100);
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_rejects_oversized_discriminator() {
        let s = SetupCodeInput::dev(20202021, 4096);
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_accepts_canonical_test_vectors() {
        // Common test values seen across Matter test suites.
        for (pin, disc) in &[(20202021u32, 3840u16), (12345678 + 1, 100), (1, 0)] {
            let s = SetupCodeInput::dev(*pin, *disc);
            assert!(s.validate().is_ok(), "({}, {}) should validate", pin, disc);
        }
    }

    #[test]
    fn manual_code_is_11_digits() {
        let s = SetupCodeInput::dev(20202021, 3840);
        let code = ManualPairingCode::from_input(&s).unwrap();
        assert_eq!(code.0.len(), 11);
        assert!(code.0.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn manual_code_display_format_is_4_3_4() {
        let s = SetupCodeInput::dev(20202021, 3840);
        let code = ManualPairingCode::from_input(&s).unwrap();
        let pretty = code.display_4_3_4();
        // 4-3-4 + 2 dashes = 13 chars.
        assert_eq!(pretty.len(), 13);
        assert_eq!(&pretty[4..5], "-");
        assert_eq!(&pretty[8..9], "-");
    }

    #[test]
    fn manual_code_is_deterministic_for_same_input() {
        let s = SetupCodeInput::dev(20202021, 3840);
        let a = ManualPairingCode::from_input(&s).unwrap();
        let b = ManualPairingCode::from_input(&s).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn manual_code_differs_when_passcode_changes() {
        let a = ManualPairingCode::from_input(&SetupCodeInput::dev(20202021, 3840))
            .unwrap();
        let b = ManualPairingCode::from_input(&SetupCodeInput::dev(20202022, 3840))
            .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn manual_code_differs_when_discriminator_changes() {
        let a = ManualPairingCode::from_input(&SetupCodeInput::dev(20202021, 3840))
            .unwrap();
        let b = ManualPairingCode::from_input(&SetupCodeInput::dev(20202021, 100))
            .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn manual_code_matches_canonical_matter_vector() {
        // Matter Core Spec 1.3 §5.1 reference: passcode 20202021 +
        // discriminator 3840 (0xF00) → published manual pairing code
        // "34970112332". This is the real spec encoding (not a
        // placeholder): chunk0=3, chunk1=49701, chunk2=1233, check=2.
        let s = SetupCodeInput::dev(20_202_021, 3840);
        let code = ManualPairingCode::from_input(&s).unwrap();
        assert_eq!(
            code.0, "34970112332",
            "encoder must match the canonical Matter reference vector"
        );
        assert_eq!(code.display_4_3_4(), "3497-011-2332");
    }

    #[test]
    fn manual_code_decode_round_trips_passcode_and_short_discriminator() {
        // A controller decodes the manual code; the passcode must come
        // back bit-for-bit and the short discriminator must be the top
        // 4 bits of the original 12-bit discriminator. This is what
        // makes the encoding *real* rather than a one-way hash.
        let passcode = 20_202_021u32;
        let discriminator = 3840u16; // 0xF00 → short disc = 0xF = 15
        let code =
            ManualPairingCode::from_input(&SetupCodeInput::dev(passcode, discriminator)).unwrap();
        let decoded = code.decode().unwrap();
        assert!(!decoded.vid_pid_present);
        assert_eq!(decoded.passcode, passcode, "passcode must round-trip exactly");
        assert_eq!(
            decoded.short_discriminator,
            (discriminator >> 8) as u8,
            "short discriminator = top 4 bits of the 12-bit discriminator"
        );
    }

    #[test]
    fn manual_code_decode_rejects_tampered_check_digit() {
        let code = ManualPairingCode::from_input(&SetupCodeInput::dev(20_202_021, 3840)).unwrap();
        // Flip the last (check) digit → Verhoeff must reject.
        let last = code.0[10..11].parse::<u8>().unwrap();
        let tampered = format!("{}{}", &code.0[0..10], (last + 1) % 10);
        let bad = ManualPairingCode(tampered);
        assert!(bad.decode().is_err(), "tampered check digit must be rejected");
    }

    #[test]
    fn verhoeff_check_digit_is_self_consistent() {
        // The Verhoeff scheme has the property that appending the
        // check digit to the body produces a string with check-digit-
        // appended == 0. Verify the recursive property holds.
        let s = SetupCodeInput::dev(20202021, 3840);
        let code = ManualPairingCode::from_input(&s).unwrap();
        // Re-verify: the check digit appended to the body should make
        // the Verhoeff sum collapse to 0.
        let body = &code.0[0..10];
        let check_recomputed = verhoeff_check_digit(body);
        let body_digit = code.0[10..11].parse::<u8>().unwrap();
        assert_eq!(check_recomputed, body_digit);
    }

    #[test]
    fn from_input_rejects_invalid_input() {
        // Build with a disallowed passcode; from_input must return Err.
        let s = SetupCodeInput::dev(11111111, 3840);
        assert!(ManualPairingCode::from_input(&s).is_err());
    }

    // ─── Property-based invariants for the commissioning encoder ─────

    use proptest::prelude::*;

    /// The §5.1.6.1 disallowed-passcodes set, hoisted to a const for
    /// reuse in property tests.
    const DISALLOWED_PASSCODES: &[u32] = &[
        0u32, 11111111, 22222222, 33333333, 44444444, 55555555,
        66666666, 77777777, 88888888, 99999999, 12345678, 87654321,
    ];

    proptest! {
        /// For ANY (passcode, discriminator) in the valid range that
        /// is not in the §5.1.6.1 disallowed set, from_input MUST
        /// produce a code with the same shape:
        ///   - exactly 11 ASCII digits
        ///   - Verhoeff-self-consistent
        ///   - 4-3-4 display form is 13 chars with dashes at positions 4 and 8
        #[test]
        fn manual_code_shape_invariants(
            passcode in 1u32..((1 << 27) - 1),
            disc in 0u16..4095,
        ) {
            // Reject the disallowed-by-spec set inside the proptest body
            // so the input strategy stays simple.
            prop_assume!(!DISALLOWED_PASSCODES.contains(&passcode));

            let s = SetupCodeInput::dev(passcode, disc);
            let code = ManualPairingCode::from_input(&s);
            prop_assert!(code.is_ok(), "valid input rejected: {:?}", code.err());
            let code = code.unwrap();

            // 11 ASCII digits.
            prop_assert_eq!(code.0.len(), 11);
            prop_assert!(code.0.chars().all(|c| c.is_ascii_digit()));

            // Verhoeff self-consistency.
            let body = &code.0[0..10];
            let body_digit = code.0[10..11].parse::<u8>().unwrap();
            prop_assert_eq!(verhoeff_check_digit(body), body_digit);

            // 4-3-4 form.
            let pretty = code.display_4_3_4();
            prop_assert_eq!(pretty.len(), 13);
            prop_assert_eq!(&pretty[4..5], "-");
            prop_assert_eq!(&pretty[8..9], "-");
        }

        /// Every disallowed passcode in the §5.1.6.1 list MUST be
        /// rejected by validate(), regardless of discriminator.
        #[test]
        fn disallowed_passcodes_always_rejected(
            disc in 0u16..4095,
            bad_idx in 0usize..DISALLOWED_PASSCODES.len(),
        ) {
            let bad = DISALLOWED_PASSCODES[bad_idx];
            let s = SetupCodeInput::dev(bad, disc);
            prop_assert!(s.validate().is_err(), "passcode {} must be rejected", bad);
        }

        /// Oversized inputs always rejected, regardless of the
        /// allowed dim.
        #[test]
        fn oversized_inputs_always_rejected(
            big_pin in (1u32 << 27)..u32::MAX,
            big_disc in 4096u16..,
        ) {
            prop_assert!(SetupCodeInput::dev(big_pin, 100).validate().is_err());
            prop_assert!(SetupCodeInput::dev(20202021, big_disc).validate().is_err());
        }

        /// Same input → same code (determinism property under random sampling).
        #[test]
        fn manual_code_deterministic_under_random_input(
            passcode in 1u32..((1 << 27) - 1),
            disc in 0u16..4095,
        ) {
            prop_assume!(!DISALLOWED_PASSCODES.contains(&passcode));
            let s = SetupCodeInput::dev(passcode, disc);
            let a = ManualPairingCode::from_input(&s).unwrap();
            let b = ManualPairingCode::from_input(&s).unwrap();
            prop_assert_eq!(a, b);
        }

        /// encode→decode is lossless for the passcode and the short
        /// discriminator, for ANY valid input. Proves the §5.1.4.1.1
        /// field-packing is a real, reversible code (not a placeholder).
        #[test]
        fn manual_code_decode_round_trips_under_random_input(
            passcode in 1u32..((1 << 27) - 1),
            disc in 0u16..4095,
        ) {
            prop_assume!(!DISALLOWED_PASSCODES.contains(&passcode));
            let code =
                ManualPairingCode::from_input(&SetupCodeInput::dev(passcode, disc)).unwrap();
            let decoded = code.decode().unwrap();
            prop_assert_eq!(decoded.passcode, passcode);
            prop_assert_eq!(decoded.short_discriminator, (disc >> 8) as u8);
            prop_assert!(!decoded.vid_pid_present);
        }
    }
}
