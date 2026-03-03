//! GOAP (Goal-Oriented Action Planning) autonomy engine -- ADR-041 WASM edge module.
//!
//! Autonomous module management via A* planning over 8-bit boolean world state.
//! Selects highest-priority unsatisfied goal, plans action sequence (max depth 4),
//! and emits module activation/deactivation events.
//!
//! Event IDs: 800-803 (Autonomy category).

const NUM_PROPS: usize = 8;
const NUM_GOALS: usize = 6;
const NUM_ACTIONS: usize = 8;
const MAX_PLAN_DEPTH: usize = 4;
const OPEN_SET_CAP: usize = 32;
const MOTION_THRESH: f32 = 0.1;
const COHERENCE_THRESH: f32 = 0.4;
const THREAT_THRESH: f32 = 0.7;

pub const EVENT_GOAL_SELECTED: i32 = 800;
pub const EVENT_MODULE_ACTIVATED: i32 = 801;
pub const EVENT_MODULE_DEACTIVATED: i32 = 802;
pub const EVENT_PLAN_COST: i32 = 803;

// World state property bit indices.
const P_PRES: usize = 0; // has_presence
const P_MOT: usize = 1;  // has_motion
const P_NITE: usize = 2; // is_night
const P_MULT: usize = 3; // multi_person
const P_LCOH: usize = 4; // low_coherence
const P_THRT: usize = 5; // high_threat
const P_VIT: usize = 6;  // has_vitals
const P_LRN: usize = 7;  // is_learning

type WorldState = u8;
#[inline] const fn ws_get(ws: WorldState, p: usize) -> bool { (ws >> p) & 1 != 0 }
#[inline] const fn ws_set(ws: WorldState, p: usize, v: bool) -> WorldState {
    if v { ws | (1 << p) } else { ws & !(1 << p) }
}

#[derive(Clone, Copy)] struct Goal { prop: usize, val: bool, priority: f32 }
const GOALS: [Goal; NUM_GOALS] = [
    Goal { prop: P_VIT,  val: true,  priority: 0.9 }, // MonitorHealth
    Goal { prop: P_PRES, val: true,  priority: 0.8 }, // SecureSpace
    Goal { prop: P_MULT, val: false, priority: 0.7 }, // CountPeople
    Goal { prop: P_LRN,  val: true,  priority: 0.5 }, // LearnPatterns
    Goal { prop: P_LRN,  val: false, priority: 0.3 }, // SaveEnergy
    Goal { prop: P_LCOH, val: false, priority: 0.1 }, // SelfTest
];

// Action: pre_mask/pre_vals = precondition bits, effect_set/effect_clear = state changes.
#[derive(Clone, Copy)] struct Action { pre_mask: u8, pre_vals: u8, eset: u8, eclr: u8, cost: u8 }
impl Action {
    const fn ok(&self, ws: WorldState) -> bool { (ws & self.pre_mask) == (self.pre_vals & self.pre_mask) }
    const fn apply(&self, ws: WorldState) -> WorldState { (ws | self.eset) & !self.eclr }
}
const ACTIONS: [Action; NUM_ACTIONS] = [
    Action { pre_mask: 1<<P_PRES, pre_vals: 1<<P_PRES, eset: 1<<P_VIT,  eclr: 0, cost: 2 }, // activate_vitals
    Action { pre_mask: 0,         pre_vals: 0,          eset: 1<<P_PRES, eclr: 0, cost: 1 }, // activate_intrusion
    Action { pre_mask: 1<<P_PRES, pre_vals: 1<<P_PRES, eset: 0, eclr: 1<<P_MULT, cost: 2 }, // activate_occupancy
    Action { pre_mask: 1<<P_LCOH, pre_vals: 0,          eset: 1<<P_LRN,  eclr: 0, cost: 3 }, // activate_gesture_learn
    Action { pre_mask: 0, pre_vals: 0, eset: 0, eclr: (1<<P_LRN)|(1<<P_VIT),  cost: 1 },     // deactivate_heavy
    Action { pre_mask: 0, pre_vals: 0, eset: 0, eclr: 1<<P_LCOH,              cost: 2 },     // run_coherence_check
    Action { pre_mask: 0, pre_vals: 0, eset: 0, eclr: (1<<P_LRN)|(1<<P_MOT),  cost: 1 },     // enter_low_power
    Action { pre_mask: 0, pre_vals: 0, eset: 0, eclr: (1<<P_LCOH)|(1<<P_THRT), cost: 3 },    // run_self_test
];

#[derive(Clone, Copy)]
struct PlanNode {
    ws: WorldState, g: u8, f: u8, depth: u8, acts: [u8; MAX_PLAN_DEPTH],
}
impl PlanNode {
    const fn empty() -> Self { Self { ws: 0, g: 0, f: 0, depth: 0, acts: [0xFF; MAX_PLAN_DEPTH] } }
}

/// GOAP autonomy planner.
pub struct GoapPlanner {
    world_state: WorldState,
    current_goal: u8,
    plan: [u8; MAX_PLAN_DEPTH],
    plan_len: u8,
    plan_step: u8,
    goal_priorities: [f32; NUM_GOALS],
    timer_count: u32,
    replan_interval: u32,
    open: [PlanNode; OPEN_SET_CAP],
}

impl GoapPlanner {
    pub const fn new() -> Self {
        let mut p = [0.0f32; NUM_GOALS];
        p[0]=0.9; p[1]=0.8; p[2]=0.7; p[3]=0.5; p[4]=0.3; p[5]=0.1;
        Self {
            world_state: 0, current_goal: 0xFF,
            plan: [0xFF; MAX_PLAN_DEPTH], plan_len: 0, plan_step: 0,
            goal_priorities: p, timer_count: 0, replan_interval: 60,
            open: [PlanNode::empty(); OPEN_SET_CAP],
        }
    }

    /// Update world state from sensor readings.
    pub fn update_world(&mut self, presence: i32, motion: f32, n_persons: i32,
                        coherence: f32, threat: f32, has_vitals: bool, is_night: bool) {
        let ws = &mut self.world_state;
        *ws = ws_set(*ws, P_PRES, presence > 0);
        *ws = ws_set(*ws, P_MOT,  motion > MOTION_THRESH);
        *ws = ws_set(*ws, P_NITE, is_night);
        *ws = ws_set(*ws, P_MULT, n_persons > 1);
        *ws = ws_set(*ws, P_LCOH, coherence < COHERENCE_THRESH);
        *ws = ws_set(*ws, P_THRT, threat > THREAT_THRESH);
        *ws = ws_set(*ws, P_VIT,  has_vitals);
    }

    /// Called at ~1 Hz.  Replans periodically and executes plan steps.
    pub fn on_timer(&mut self) -> &[(i32, f32)] {
        self.timer_count += 1;
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n = 0usize;
        // Replan at interval.
        if self.timer_count % self.replan_interval == 0 {
            let g = self.select_goal();
            if g < NUM_GOALS as u8 {
                self.current_goal = g;
                if n < 4 { unsafe { EVENTS[n] = (EVENT_GOAL_SELECTED, g as f32); } n += 1; }
                let cost = self.plan_for_goal(g as usize);
                if cost < 255 && n < 4 {
                    unsafe { EVENTS[n] = (EVENT_PLAN_COST, cost as f32); } n += 1;
                }
            }
        }
        // Execute next plan step.
        if self.plan_step < self.plan_len {
            let aid = self.plan[self.plan_step as usize];
            if (aid as usize) < NUM_ACTIONS {
                let action = &ACTIONS[aid as usize];
                if action.ok(self.world_state) {
                    let old = self.world_state;
                    self.world_state = action.apply(self.world_state);
                    if (self.world_state & !old) != 0 && n < 4 {
                        unsafe { EVENTS[n] = (EVENT_MODULE_ACTIVATED, aid as f32); } n += 1;
                    }
                    if (old & !self.world_state) != 0 && n < 4 {
                        unsafe { EVENTS[n] = (EVENT_MODULE_DEACTIVATED, aid as f32); } n += 1;
                    }
                }
            }
            self.plan_step += 1;
        }
        unsafe { &EVENTS[..n] }
    }

    fn select_goal(&self) -> u8 {
        let mut best = 0xFFu8;
        let mut bp = -1.0f32;
        let mut i = 0usize;
        while i < NUM_GOALS {
            let g = &GOALS[i];
            if ws_get(self.world_state, g.prop) != g.val && self.goal_priorities[i] > bp {
                bp = self.goal_priorities[i]; best = i as u8;
            }
            i += 1;
        }
        best
    }

    /// A* search for action sequence achieving goal.  Returns cost or 255.
    fn plan_for_goal(&mut self, gid: usize) -> u8 {
        self.plan_len = 0; self.plan_step = 0; self.plan = [0xFF; MAX_PLAN_DEPTH];
        if gid >= NUM_GOALS { return 255; }
        let goal = &GOALS[gid];
        if ws_get(self.world_state, goal.prop) == goal.val { return 0; }
        let h = |ws: WorldState| -> u8 { if ws_get(ws, goal.prop) == goal.val { 0 } else { 1 } };
        self.open[0] = PlanNode { ws: self.world_state, g: 0, f: h(self.world_state),
                                  depth: 0, acts: [0xFF; MAX_PLAN_DEPTH] };
        let mut olen = 1usize;
        let mut iter = 0u16;
        while olen > 0 && iter < 200 {
            iter += 1;
            // Find lowest f-cost node.
            let mut bi = 0usize; let mut bf = self.open[0].f;
            let mut k = 1usize;
            while k < olen { if self.open[k].f < bf { bf = self.open[k].f; bi = k; } k += 1; }
            let cur = self.open[bi];
            olen -= 1; if bi < olen { self.open[bi] = self.open[olen]; }
            // Goal check.
            if ws_get(cur.ws, goal.prop) == goal.val {
                let mut d = 0usize;
                while d < cur.depth as usize && d < MAX_PLAN_DEPTH { self.plan[d] = cur.acts[d]; d += 1; }
                self.plan_len = cur.depth; return cur.g;
            }
            if cur.depth as usize >= MAX_PLAN_DEPTH { continue; }
            // Expand.
            let mut a = 0usize;
            while a < NUM_ACTIONS {
                if ACTIONS[a].ok(cur.ws) && olen < OPEN_SET_CAP {
                    let nws = ACTIONS[a].apply(cur.ws);
                    let ng = cur.g.saturating_add(ACTIONS[a].cost);
                    let mut node = PlanNode { ws: nws, g: ng, f: ng.saturating_add(h(nws)),
                                              depth: cur.depth + 1, acts: cur.acts };
                    node.acts[cur.depth as usize] = a as u8;
                    self.open[olen] = node; olen += 1;
                }
                a += 1;
            }
        }
        255
    }

    pub fn world_state(&self) -> u8 { self.world_state }
    pub fn current_goal(&self) -> u8 { self.current_goal }
    pub fn plan_len(&self) -> u8 { self.plan_len }
    pub fn plan_step(&self) -> u8 { self.plan_step }
    pub fn has_property(&self, p: usize) -> bool { p < NUM_PROPS && ws_get(self.world_state, p) }
    pub fn set_goal_priority(&mut self, gid: usize, priority: f32) {
        if gid < NUM_GOALS { self.goal_priorities[gid] = priority; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let p = GoapPlanner::new();
        assert_eq!(p.world_state(), 0);
        assert_eq!(p.current_goal(), 0xFF);
        assert_eq!(p.plan_len(), 0);
    }

    #[test]
    fn test_world_state_update() {
        let mut p = GoapPlanner::new();
        p.update_world(1, 0.5, 2, 0.8, 0.1, true, false);
        assert!(p.has_property(P_PRES));
        assert!(p.has_property(P_MOT));
        assert!(!p.has_property(P_NITE));
        assert!(p.has_property(P_MULT));
        assert!(!p.has_property(P_LCOH));
        assert!(!p.has_property(P_THRT));
        assert!(p.has_property(P_VIT));
    }

    #[test]
    fn test_ws_bit_ops() {
        let ws = ws_set(0u8, 3, true);
        assert!(ws_get(ws, 3));
        assert!(!ws_get(ws, 0));
        assert!(!ws_get(ws_set(ws, 3, false), 3));
    }

    #[test]
    fn test_goal_selection_highest_priority() {
        let p = GoapPlanner::new();
        assert_eq!(p.select_goal(), 0); // MonitorHealth (prio 0.9)
    }

    #[test]
    fn test_goal_satisfied_skipped() {
        let mut p = GoapPlanner::new();
        p.world_state = ws_set(ws_set(p.world_state, P_VIT, true), P_PRES, true);
        assert_eq!(p.select_goal(), 3); // LearnPatterns (next unsatisfied)
    }

    #[test]
    fn test_action_preconditions() {
        assert!(!ACTIONS[0].ok(0)); // activate_vitals needs presence
        assert!(ACTIONS[0].ok(ws_set(0, P_PRES, true)));
    }

    #[test]
    fn test_action_effects() {
        let ws = ACTIONS[0].apply(ws_set(0, P_PRES, true));
        assert!(ws_get(ws, P_VIT));
    }

    #[test]
    fn test_plan_simple() {
        let mut p = GoapPlanner::new();
        let cost = p.plan_for_goal(0);
        assert!(cost < 255, "should find a plan for MonitorHealth");
        assert!(p.plan_len() >= 1);
    }

    #[test]
    fn test_plan_already_satisfied() {
        let mut p = GoapPlanner::new();
        p.world_state = ws_set(p.world_state, P_VIT, true);
        assert_eq!(p.plan_for_goal(0), 0);
        assert_eq!(p.plan_len(), 0);
    }

    #[test]
    fn test_plan_execution() {
        let mut p = GoapPlanner::new();
        p.timer_count = p.replan_interval - 1;
        let events = p.on_timer();
        assert!(events.iter().any(|&(et, _)| et == EVENT_GOAL_SELECTED));
    }

    #[test]
    fn test_step_execution_emits_events() {
        let mut p = GoapPlanner::new();
        p.plan[0] = 1; p.plan_len = 1; p.plan_step = 0;
        p.timer_count = 1;
        let events = p.on_timer();
        assert!(events.iter().any(|&(et, _)| et == EVENT_MODULE_ACTIVATED));
        assert!(p.has_property(P_PRES));
    }

    #[test]
    fn test_set_goal_priority() {
        let mut p = GoapPlanner::new();
        p.set_goal_priority(5, 0.99);
        p.world_state = ws_set(p.world_state, P_LCOH, true);
        assert_eq!(p.select_goal(), 5); // SelfTest now highest unsatisfied
    }
}
