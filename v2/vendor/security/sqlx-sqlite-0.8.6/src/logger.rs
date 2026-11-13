// Bad casts in this module SHOULD NOT result in a SQL injection
// https://github.com/launchbadge/sqlx/issues/3440
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use crate::connection::intmap::IntMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

pub(crate) use sqlx_core::logger::*;

#[derive(Debug)]
pub(crate) enum BranchResult<R: Debug + 'static> {
    Result(R),
    Dedup(BranchParent),
    Halt,
    Error,
    GasLimit,
    LoopLimit,
    Branched,
}

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq, Ord, PartialOrd)]
pub(crate) struct BranchParent {
    pub id: i64,
    pub idx: i64,
}

#[derive(Debug)]
pub(crate) struct InstructionHistory<S: Debug + DebugDiff> {
    pub program_i: usize,
    pub state: S,
}

pub(crate) trait DebugDiff {
    fn diff(&self, prev: &Self) -> String;
}

pub struct QueryPlanLogger<'q, R: Debug + 'static, S: Debug + DebugDiff + 'static, P: Debug> {
    sql: &'q str,
    unknown_operations: HashSet<usize>,
    branch_origins: IntMap<BranchParent>,
    branch_results: IntMap<BranchResult<R>>,
    branch_operations: IntMap<IntMap<InstructionHistory<S>>>,
    program: &'q [P],
}

/// convert a string into dot format
fn dot_escape_string(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .replace('\\', r#"\\"#)
        .replace('"', "'")
        .replace('\n', r#"\n"#)
        .to_string()
}

impl<R: Debug, S: Debug + DebugDiff, P: Debug> core::fmt::Display for QueryPlanLogger<'_, R, S, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        //writes query plan history in dot format
        f.write_str("digraph {\n")?;

        f.write_str("subgraph operations {\n")?;
        f.write_str("style=\"rounded\";\nnode [shape=\"point\"];\n")?;

        let all_states: std::collections::HashMap<BranchParent, &InstructionHistory<S>> = self
            .branch_operations
            .iter_entries()
            .flat_map(
                |(branch_id, instructions): (i64, &IntMap<InstructionHistory<S>>)| {
                    instructions.iter_entries().map(
                        move |(idx, ih): (i64, &InstructionHistory<S>)| {
                            (BranchParent { id: branch_id, idx }, ih)
                        },
                    )
                },
            )
            .collect();

        let mut instruction_uses: IntMap<Vec<BranchParent>> = Default::default();
        for (k, state) in all_states.iter() {
            let entry = instruction_uses.get_mut_or_default(&(state.program_i as i64));
            entry.push(*k);
        }

        let mut branch_children: std::collections::HashMap<BranchParent, Vec<BranchParent>> =
            Default::default();

        let mut branched_with_state: std::collections::HashSet<BranchParent> = Default::default();

        for (branch_id, branch_parent) in self.branch_origins.iter_entries() {
            let entry = branch_children.entry(*branch_parent).or_default();
            entry.push(BranchParent {
                id: branch_id,
                idx: 0,
            });
        }

        for (idx, instruction) in self.program.iter().enumerate() {
            let escaped_instruction = dot_escape_string(format!("{:?}", instruction));
            write!(
                f,
                "subgraph cluster_{} {{ label=\"{}\"",
                idx, escaped_instruction
            )?;

            if self.unknown_operations.contains(&idx) {
                f.write_str(" style=dashed")?;
            }

            f.write_str(";\n")?;

            let mut state_list: std::collections::BTreeMap<
                String,
                Vec<(BranchParent, Option<BranchParent>)>,
            > = Default::default();

            write!(f, "i{}[style=invis];", idx)?;

            if let Some(this_instruction_uses) = instruction_uses.get(&(idx as i64)) {
                for curr_ref in this_instruction_uses.iter() {
                    if let Some(curr_state) = all_states.get(curr_ref) {
                        let next_ref = BranchParent {
                            id: curr_ref.id,
                            idx: curr_ref.idx + 1,
                        };

                        if let Some(next_state) = all_states.get(&next_ref) {
                            let state_diff = next_state.state.diff(&curr_state.state);

                            state_list
                                .entry(state_diff)
                                .or_default()
                                .push((*curr_ref, Some(next_ref)));
                        } else {
                            state_list
                                .entry(Default::default())
                                .or_default()
                                .push((*curr_ref, None));
                        };

                        if let Some(children) = branch_children.get(curr_ref) {
                            for next_ref in children {
                                if let Some(next_state) = all_states.get(next_ref) {
                                    let state_diff = next_state.state.diff(&curr_state.state);

                                    if !state_diff.is_empty() {
                                        branched_with_state.insert(*next_ref);
                                    }

                                    state_list
                                        .entry(state_diff)
                                        .or_default()
                                        .push((*curr_ref, Some(*next_ref)));
                                }
                            }
                        };
                    }
                }

                for curr_ref in this_instruction_uses {
                    if branch_children.contains_key(curr_ref) {
                        write!(f, "\"b{}p{}\";", curr_ref.id, curr_ref.idx)?;
                    }
                }
            } else {
                write!(f, "i{}->i{}[style=invis];", idx - 1, idx)?;
            }

            for (state_num, (state_diff, ref_list)) in state_list.iter().enumerate() {
                if !state_diff.is_empty() {
                    let escaped_state = dot_escape_string(state_diff);
                    write!(
                        f,
                        "subgraph \"cluster_i{}s{}\" {{\nlabel=\"{}\"\n",
                        idx, state_num, escaped_state
                    )?;
                }

                for (curr_ref, next_ref) in ref_list {
                    if let Some(next_ref) = next_ref {
                        let next_program_i = all_states
                            .get(next_ref)
                            .map(|s| s.program_i.to_string())
                            .unwrap_or_default();

                        if branched_with_state.contains(next_ref) {
                            write!(
                                f,
                                "\"b{}p{}_b{}p{}\"[tooltip=\"next:{}\"];",
                                curr_ref.id,
                                curr_ref.idx,
                                next_ref.id,
                                next_ref.idx,
                                next_program_i
                            )?;
                            continue;
                        } else {
                            write!(
                                f,
                                "\"b{}p{}\"[tooltip=\"next:{}\"];",
                                curr_ref.id, curr_ref.idx, next_program_i
                            )?;
                        }
                    } else {
                        write!(f, "\"b{}p{}\";", curr_ref.id, curr_ref.idx)?;
                    }
                }

                if !state_diff.is_empty() {
                    f.write_str("}\n")?;
                }
            }

            f.write_str("}\n")?;
        }

        f.write_str("};\n")?; //subgraph operations

        let max_branch_id: i64 = [
            self.branch_operations.last_index().unwrap_or(0),
            self.branch_results.last_index().unwrap_or(0),
            self.branch_results.last_index().unwrap_or(0),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);

        f.write_str("subgraph branches {\n")?;
        for branch_id in 0..=max_branch_id {
            write!(f, "subgraph b{}{{", branch_id)?;

            let branch_num = branch_id as usize;
            let color_names = [
                "blue",
                "red",
                "cyan",
                "yellow",
                "green",
                "magenta",
                "orange",
                "purple",
                "orangered",
                "sienna",
                "olivedrab",
                "pink",
            ];
            let color_name_root = color_names[branch_num % color_names.len()];
            let color_name_suffix = match (branch_num / color_names.len()) % 4 {
                0 => "1",
                1 => "4",
                2 => "3",
                3 => "2",
                _ => "",
            }; //colors are easily confused after color_names.len() * 2, and outright reused after color_names.len() * 4
            write!(
                f,
                "edge [colorscheme=x11 color={}{}];",
                color_name_root, color_name_suffix
            )?;

            let mut instruction_list: Vec<(BranchParent, &InstructionHistory<S>)> = Vec::new();
            if let Some(parent) = self.branch_origins.get(&branch_id) {
                if let Some(parent_state) = all_states.get(parent) {
                    instruction_list.push((*parent, parent_state));
                }
            }
            if let Some(instructions) = self.branch_operations.get(&branch_id) {
                for instruction in instructions.iter_entries() {
                    instruction_list.push((
                        BranchParent {
                            id: branch_id,
                            idx: instruction.0,
                        },
                        instruction.1,
                    ))
                }
            }

            let mut instructions_iter = instruction_list.into_iter();

            if let Some((cur_ref, _)) = instructions_iter.next() {
                let mut prev_ref = cur_ref;

                for (cur_ref, _) in instructions_iter {
                    if branched_with_state.contains(&cur_ref) {
                        writeln!(
                            f,
                            "\"b{}p{}\" -> \"b{}p{}_b{}p{}\" -> \"b{}p{}\"",
                            prev_ref.id,
                            prev_ref.idx,
                            prev_ref.id,
                            prev_ref.idx,
                            cur_ref.id,
                            cur_ref.idx,
                            cur_ref.id,
                            cur_ref.idx
                        )?;
                    } else {
                        write!(
                            f,
                            "\"b{}p{}\" -> \"b{}p{}\";",
                            prev_ref.id, prev_ref.idx, cur_ref.id, cur_ref.idx
                        )?;
                    }
                    prev_ref = cur_ref;
                }

                //draw edge to the result of this branch
                if let Some(result) = self.branch_results.get(&branch_id) {
                    if let BranchResult::Dedup(dedup_ref) = result {
                        write!(
                            f,
                            "\"b{}p{}\"->\"b{}p{}\" [style=dotted]",
                            prev_ref.id, prev_ref.idx, dedup_ref.id, dedup_ref.idx
                        )?;
                    } else {
                        let escaped_result = dot_escape_string(format!("{:?}", result));
                        write!(
                            f,
                            "\"b{}p{}\" ->\"{}\"; \"{}\" [shape=box];",
                            prev_ref.id, prev_ref.idx, escaped_result, escaped_result
                        )?;
                    }
                } else {
                    write!(
                        f,
                        "\"b{}p{}\" ->\"NoResult\"; \"NoResult\" [shape=box];",
                        prev_ref.id, prev_ref.idx
                    )?;
                }
            }
            f.write_str("};\n")?;
        }
        f.write_str("};\n")?; //branches

        f.write_str("}\n")?;
        Ok(())
    }
}

impl<'q, R: Debug, S: Debug + DebugDiff, P: Debug> QueryPlanLogger<'q, R, S, P> {
    pub fn new(sql: &'q str, program: &'q [P]) -> Self {
        Self {
            sql,
            unknown_operations: HashSet::new(),
            branch_origins: IntMap::new(),
            branch_results: IntMap::new(),
            branch_operations: IntMap::new(),
            program,
        }
    }

    pub fn log_enabled(&self) -> bool {
        log::log_enabled!(target: "sqlx::explain", log::Level::Trace)
            || private_tracing_dynamic_enabled!(target: "sqlx::explain", tracing::Level::TRACE)
    }

    pub fn add_branch<I: Copy>(&mut self, state: I, parent: &BranchParent)
    where
        BranchParent: From<I>,
    {
        if !self.log_enabled() {
            return;
        }
        let branch: BranchParent = BranchParent::from(state);
        self.branch_origins.insert(branch.id, *parent);
    }

    pub fn add_operation<I: Copy>(&mut self, program_i: usize, state: I)
    where
        BranchParent: From<I>,
        S: From<I>,
    {
        if !self.log_enabled() {
            return;
        }
        let branch: BranchParent = BranchParent::from(state);
        let state: S = S::from(state);
        self.branch_operations
            .get_mut_or_default(&branch.id)
            .insert(branch.idx, InstructionHistory { program_i, state });
    }

    pub fn add_result<I>(&mut self, state: I, result: BranchResult<R>)
    where
        BranchParent: for<'a> From<&'a I>,
        S: From<I>,
    {
        if !self.log_enabled() {
            return;
        }
        let branch: BranchParent = BranchParent::from(&state);
        self.branch_results.insert(branch.id, result);
    }

    pub fn add_unknown_operation(&mut self, operation: usize) {
        if !self.log_enabled() {
            return;
        }
        self.unknown_operations.insert(operation);
    }

    pub fn finish(&self) {
        if !self.log_enabled() {
            return;
        }

        let mut summary = parse_query_summary(self.sql);

        let sql = if summary != self.sql {
            summary.push_str(" …");
            format!(
                "\n\n{}\n",
                self.sql /*
                         sqlformat::format(
                             self.sql,
                             &sqlformat::QueryParams::None,
                             sqlformat::FormatOptions::default()
                         )
                         */
            )
        } else {
            String::new()
        };

        sqlx_core::private_tracing_dynamic_event!(
            target: "sqlx::explain",
            tracing::Level::TRACE,
            "{}; program:\n{}\n\n{:?}", summary, self, sql
        );
    }
}

impl<'q, R: Debug, S: Debug + DebugDiff, P: Debug> Drop for QueryPlanLogger<'q, R, S, P> {
    fn drop(&mut self) {
        self.finish();
    }
}
