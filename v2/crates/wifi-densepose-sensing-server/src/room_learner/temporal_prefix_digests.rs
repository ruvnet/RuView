//! Ephemeral exact canonical tuple-prefix SHA256 computation, OFFLOCK only.
//! No persistent cache, imported claims, validation waiver or authority issuer.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
};
const MAX_CUTS: usize = 15;
const MAX_ROWS: usize = 128;

pub(super) struct CanonicalPrefixDigests {
    digests: BTreeMap<usize, String>,
}
impl CanonicalPrefixDigests {
    pub(super) fn at(&self, cut: usize) -> Result<&str, String> {
        self.digests
            .get(&cut)
            .map(String::as_str)
            .ok_or_else(|| "temporal_prefix_digest_missing_cut".into())
    }
}
struct HashWriter<'a>(&'a mut Sha256);
impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct Fork {
    cut: usize,
    state: Sha256,
}
struct FanoutWriter<'a> {
    forks: &'a mut [Fork],
    index: usize,
}
impl Write for FanoutWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for fork in self.forks.iter_mut().filter(|f| self.index < f.cut) {
            fork.state.update(bytes);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// EXACTLY SHA256(serde_json::to_vec(&(rows[..n],evidence[..n],spans[..n]))).
/// Requested duplicate cuts collapse; order does not affect results. At most15
/// temporary SHA states. Only requested prefixes are serialized, never a tail
/// outside the largest cut; an empty request does not touch any input value.
pub(super) fn canonical_tuple_prefixes<R: Serialize, E: Serialize, S: Serialize>(
    rows: &[R],
    evidence: &[E],
    spans: &[S],
    cuts: &[usize],
) -> Result<CanonicalPrefixDigests, String> {
    if rows.len() != evidence.len()
        || rows.len() != spans.len()
        || rows.len() > MAX_ROWS
        || cuts.len() > MAX_CUTS
        || cuts.iter().any(|n| *n > rows.len())
    {
        return Err("temporal_prefix_digest_shape".into());
    }
    if cuts.is_empty() {
        return Ok(CanonicalPrefixDigests {
            digests: BTreeMap::new(),
        });
    }
    let cuts = cuts.iter().copied().collect::<BTreeSet<_>>();
    let max = *cuts.last().unwrap();
    let encoding = |error: serde_json::Error| format!("temporal_prefix_digest_encoding:{error}");
    let mut raw = Sha256::new();
    raw.update(b"[[");
    let mut forks = Vec::with_capacity(cuts.len());
    if cuts.contains(&0) {
        forks.push(Fork {
            cut: 0,
            state: raw.clone(),
        });
    }
    for (index, row) in rows[..max].iter().enumerate() {
        if index > 0 {
            raw.update(b",");
        }
        serde_json::to_writer(HashWriter(&mut raw), row).map_err(&encoding)?;
        if cuts.contains(&(index + 1)) {
            forks.push(Fork {
                cut: index + 1,
                state: raw.clone(),
            });
        }
    }
    for fork in &mut forks {
        fork.state.update(b"],[");
    }
    for (index, item) in evidence[..max].iter().enumerate() {
        if index > 0 {
            for fork in forks.iter_mut().filter(|f| index < f.cut) {
                fork.state.update(b",");
            }
        }
        serde_json::to_writer(
            FanoutWriter {
                forks: &mut forks,
                index,
            },
            item,
        )
        .map_err(&encoding)?;
    }
    for fork in &mut forks {
        fork.state.update(b"],[");
    }
    for (index, item) in spans[..max].iter().enumerate() {
        if index > 0 {
            for fork in forks.iter_mut().filter(|f| index < f.cut) {
                fork.state.update(b",");
            }
        }
        serde_json::to_writer(
            FanoutWriter {
                forks: &mut forks,
                index,
            },
            item,
        )
        .map_err(&encoding)?;
    }
    let mut digests = BTreeMap::new();
    for mut fork in forks {
        fork.state.update(b"]]");
        digests.insert(fork.cut, format!("{:x}", fork.state.finalize()));
    }
    Ok(CanonicalPrefixDigests { digests })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Serialize)]
    struct Row {
        source: u8,
        label: String,
        samples: Vec<f64>,
        single: f32,
        optional: Option<String>,
    }
    fn rows() -> Vec<Row> {
        (0..128)
            .map(|i| Row {
                source: if i % 2 == 0 { 255 } else { 1 },
                label: format!("源\\\"\n\u{1}雪-{i}"),
                samples: vec![
                    -0.0,
                    0.0,
                    f64::MAX,
                    f64::MIN,
                    f64::MIN_POSITIVE / 2.0,
                    i as f64 / 7.0,
                ],
                single: if i % 2 == 0 {
                    f32::MIN_POSITIVE / 2.0
                } else {
                    -0.0
                },
                optional: if i % 2 == 0 {
                    None
                } else {
                    Some("é\\\"\r\t".into())
                },
            })
            .collect()
    }
    fn expected<R: Serialize, E: Serialize, S: Serialize>(
        r: &[R],
        e: &[E],
        s: &[S],
        n: usize,
    ) -> String {
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(&r[..n], &e[..n], &s[..n])).unwrap())
        )
    }
    #[test]
    fn every_cut_zero_through128_matches_actual_rust_tuple_bytes() {
        let rows = rows();
        let evidence=(0..128).map(|i|serde_json::json!({"frame":format!("{}",u128::MAX-i),"unicode":"雪\n\\\"","id":i})).collect::<Vec<_>>();
        let spans = (0..128)
            .map(|i| (i, Some(i * 9), [i + 1, i + 2]))
            .collect::<Vec<_>>();
        for cuts in (0..=128).collect::<Vec<_>>().chunks(15) {
            let got = canonical_tuple_prefixes(&rows, &evidence, &spans, cuts).unwrap();
            for n in cuts {
                assert_eq!(
                    got.at(*n).unwrap(),
                    expected(&rows, &evidence, &spans, *n),
                    "cut{n}"
                );
            }
        }
    }
    #[test]
    fn exact_seven_train_and_eight_heldout_boundaries_match_reference() {
        let rows = rows();
        let evidence = (0..128).map(|i| format!("e-{i}")).collect::<Vec<_>>();
        let spans = (0..128).collect::<Vec<_>>();
        let cuts = [
            16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128,
        ];
        let got = canonical_tuple_prefixes(&rows, &evidence, &spans, &cuts).unwrap();
        for n in cuts {
            assert_eq!(got.at(n).unwrap(), expected(&rows, &evidence, &spans, n));
        }
    }
    #[test]
    fn duplicate_unsorted_shared_references_and_repeat_calls_are_exact() {
        let rows = rows();
        let repeated = vec![&rows[0]; 128];
        let evidence = vec![Some("same\\\n雪"); 128];
        let spans = vec![None::<u64>; 128];
        let cuts = [128, 0, 64, 64, 1, 0];
        let first = canonical_tuple_prefixes(&repeated, &evidence, &spans, &cuts).unwrap();
        let second = canonical_tuple_prefixes(&repeated, &evidence, &spans, &cuts).unwrap();
        assert_eq!(first.digests.len(), 4);
        assert_eq!(first.digests, second.digests);
        for n in cuts {
            assert_eq!(
                first.at(n).unwrap(),
                expected(&repeated, &evidence, &spans, n)
            );
        }
    }
    #[test]
    fn malformed_lengths_capacity_and_cuts_fail_without_partial_hashes() {
        assert!(canonical_tuple_prefixes(&[1], &[2, 3], &[4], &[0]).is_err());
        assert!(canonical_tuple_prefixes(&[1], &[2], &[3], &[2]).is_err());
        assert!(canonical_tuple_prefixes(&[1], &[2], &[3], &[0; 16]).is_err());
        assert!(canonical_tuple_prefixes(&[0; 129], &[0; 129], &[0; 129], &[1]).is_err());
    }
    struct ErrorValue;
    impl Serialize for ErrorValue {
        fn serialize<T: serde::Serializer>(&self, _serializer: T) -> Result<T::Ok, T::Error> {
            Err(serde::ser::Error::custom("deliberate serializer failure"))
        }
    }
    #[test]
    fn serialization_errors_in_any_component_return_error_not_partial_results() {
        assert!(canonical_tuple_prefixes(&[ErrorValue], &[1], &[2], &[0, 1]).is_err());
        assert!(canonical_tuple_prefixes(&[1], &[ErrorValue], &[2], &[0, 1]).is_err());
        assert!(canonical_tuple_prefixes(&[1], &[2], &[ErrorValue], &[0, 1]).is_err());
    }
    #[test]
    fn empty_request_and_irrelevant_tail_are_never_serialized() {
        assert!(
            canonical_tuple_prefixes(&[ErrorValue], &[ErrorValue], &[ErrorValue], &[])
                .unwrap()
                .digests
                .is_empty()
        );
        #[derive(Serialize)]
        #[serde(untagged)]
        enum MaybeError {
            Good(u8),
            Bad(ErrorValue),
        }
        let rows = [MaybeError::Good(7), MaybeError::Bad(ErrorValue)];
        let got = canonical_tuple_prefixes(&rows, &[1, 2], &[3, 4], &[0, 1]).unwrap();
        assert_eq!(got.at(1).unwrap(), expected(&rows, &[1, 2], &[3, 4], 1));
        assert!(canonical_tuple_prefixes(&rows, &[1, 2], &[3, 4], &[2]).is_err());
    }
    #[test]
    fn empty_dataset_cut_zero_is_exact_empty_three_tuple() {
        let empty: Vec<u8> = vec![];
        let got = canonical_tuple_prefixes(&empty, &empty, &empty, &[0]).unwrap();
        assert_eq!(
            got.at(0).unwrap(),
            format!("{:x}", Sha256::digest(b"[[],[],[]]"))
        );
    }
}
