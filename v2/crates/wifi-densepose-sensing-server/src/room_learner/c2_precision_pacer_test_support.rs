//! PREPARED ONLY: std-only, no receiver/user-data/network access.
//! One baseline control + one absolute-deadline precision strategy; no tuning loop.
use std::{hint, time::{Duration, Instant}};
const HZ: u64 = 33;
const NS: u64 = 1_000_000_000;
// Frozen r2 temporal_acquisition.rs217 and ingest_checked811–815;
// temporal_execution.rs484–489 also enforces this for whole-period input.
// This is ingestion compatibility, NOT an extra finish_checked predicate.
const MIN_INTERVAL: u64 = 1_000_000_000_000 / 36_000;
const MAX_GAP: u64 = 50_000_000;
const WINDOW_NS: u64 = 2_000_000_000;
const MAX_WINDOW_NS: u64 = 2_050_000_000;
const MIN_ROWS: usize = 61;
const LO: u128 = 30_000;
const HI: u128 = 36_000;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision { Wait(u64), Emit(u64), MissedPeriod(u64) }
fn due_ns(index: usize) -> u64 { (index as u128 * u128::from(NS) / u128::from(HZ)) as u64 }
fn decide(index: usize, actual_ns: u64) -> Decision {
    let due = due_ns(index);
    if actual_ns < due { Decision::Wait(due - actual_ns) }
    else if actual_ns >= due_ns(index + 1) { Decision::MissedPeriod(actual_ns - due) }
    else { Decision::Emit(actual_ns - due) }
}
#[derive(Default, Clone, Copy)]
struct WaitCost { requested_sleep: u64, actual_sleep: u64, spin: u64 }
struct Wake { actual: u64, cost: WaitCost, missed: bool }
/// The emitted timestamp is ACTUAL monotonic wakeup, never due_ns(index).
/// Absolute deadlines never reset from actual wakeup. A missed whole period
/// stops the strategy rather than emitting catch-up/multiple batches.
fn wait_absolute(origin: Instant, index: usize, guard_ns: u64) -> Wake {
    let due = origin.checked_add(Duration::from_nanos(due_ns(index))).expect("bounded study deadline");
    let mut cost = WaitCost::default();
    let now = Instant::now();
    if now < due {
        let remaining = due.duration_since(now);
        let guard = Duration::from_nanos(guard_ns);
        if remaining > guard {
            let sleep_for = remaining - guard;
            cost.requested_sleep = sleep_for.as_nanos() as u64;
            let slept = Instant::now(); std::thread::sleep(sleep_for);
            cost.actual_sleep = slept.elapsed().as_nanos() as u64;
        }
        let spinning = Instant::now();
        while Instant::now() < due { hint::spin_loop(); }
        cost.spin = spinning.elapsed().as_nanos() as u64;
    }
    let actual = origin.elapsed().as_nanos() as u64;
    Wake { actual, cost, missed: matches!(decide(index, actual), Decision::MissedPeriod(_)) }
}
fn rate_ok(count: usize, span: u64) -> bool {
    span > 0 && (count.saturating_sub(1) as u128) * 1_000_000_000_000 >= LO * u128::from(span)
        && (count.saturating_sub(1) as u128) * 1_000_000_000_000 <= HI * u128::from(span)
}
// Mirrors retained-window readiness + finish span/mean cadence only.
fn first_window(times: &[u64]) -> Option<(usize, u64, bool)> {
    let first = *times.first()?;
    for count in MIN_ROWS..=times.len() {
        let span = times[count - 1].checked_sub(first)?;
        if span >= WINDOW_NS { return Some((count, span, span <= MAX_WINDOW_NS && rate_ok(count, span))); }
    }
    None
}
// Complete input compatibility combines REAL ingestion intervals with the
// separately checked retained-window mean/span/horizon. No invented gate.
fn qualifies(times: &[u64], requested: usize, missed: bool) -> bool {
    if missed || times.len() != requested || times.len() < 2 { return false; }
    if times.windows(2).any(|w| w[1] <= w[0] || w[1] - w[0] < MIN_INTERVAL || w[1] - w[0] > MAX_GAP) { return false; }
    let span = times[times.len() - 1] - times[0];
    rate_ok(times.len(), span) && first_window(times).is_some_and(|(_,_,ok)| ok)
}
#[derive(Debug)]
pub(super) struct InputStudy {
    pub qualified:bool, pub requested:usize, pub emitted:usize,
    pub span_ns:u64, pub rate_millihz:u64, pub min_gap_ns:u64, pub max_gap_ns:u64,
    pub missed_index:Option<usize>, pub spin_wall_ns:u128, pub sleep_overshoot_max_ns:u64,
}
pub(super) fn input_study(samples:usize,guard_ns:u64)->InputStudy {
    assert!((70..=330).contains(&samples));assert!((500_000..=3_000_000).contains(&guard_ns));
    let origin=Instant::now();let mut times=Vec::with_capacity(samples);let mut missed=None;let mut spin=0u128;let mut overshoot=0;
    for index in 0..samples {
        let wake=wait_absolute(origin,index,guard_ns);spin+=u128::from(wake.cost.spin);overshoot=overshoot.max(wake.cost.actual_sleep.saturating_sub(wake.cost.requested_sleep));
        if wake.missed{missed=Some(index);break;}times.push(wake.actual);
    }
    let span=times.last().zip(times.first()).map_or(0,|(a,b)|a.saturating_sub(*b));
    let gaps=times.windows(2).map(|w|w[1].saturating_sub(w[0])).collect::<Vec<_>>();
    InputStudy{qualified:qualifies(&times,samples,missed.is_some()),requested:samples,emitted:times.len(),span_ns:span,
        rate_millihz:if span==0{0}else{((times.len().saturating_sub(1)as u128)*1_000_000_000_000/u128::from(span))as u64},
        min_gap_ns:gaps.iter().min().copied().unwrap_or(0),max_gap_ns:gaps.iter().max().copied().unwrap_or(0),missed_index:missed,spin_wall_ns:spin,sleep_overshoot_max_ns:overshoot}
}
pub(super) fn next_actual_receipt(origin:Instant,index:usize,guard_ns:u64)->Result<u64,String>{
    let wake=wait_absolute(origin,index,guard_ns);
    if wake.missed{return Err(format!("input_unqualified: whole period missed index={index} actual_ns={} planned_ns={}",wake.actual,due_ns(index)));}
    Ok(wake.actual)
}
pub(super) fn actual_stream_qualifies(times:&[u64])->bool{qualifies(times,times.len(),false)}
