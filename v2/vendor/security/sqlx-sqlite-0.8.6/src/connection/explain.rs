// Bad casts in this module SHOULD NOT result in a SQL injection
// https://github.com/launchbadge/sqlx/issues/3440
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
use crate::connection::intmap::IntMap;
use crate::connection::{execute, ConnectionState};
use crate::error::Error;
use crate::from_row::FromRow;
use crate::logger::{BranchParent, BranchResult, DebugDiff};
use crate::type_info::DataType;
use crate::SqliteTypeInfo;
use sqlx_core::{hash_map, HashMap};
use std::fmt::Debug;
use std::str::from_utf8;

// affinity
const SQLITE_AFF_NONE: u8 = 0x40; /* '@' */
const SQLITE_AFF_BLOB: u8 = 0x41; /* 'A' */
const SQLITE_AFF_TEXT: u8 = 0x42; /* 'B' */
const SQLITE_AFF_NUMERIC: u8 = 0x43; /* 'C' */
const SQLITE_AFF_INTEGER: u8 = 0x44; /* 'D' */
const SQLITE_AFF_REAL: u8 = 0x45; /* 'E' */

// opcodes
const OP_INIT: &str = "Init";
const OP_GOTO: &str = "Goto";
const OP_DECR_JUMP_ZERO: &str = "DecrJumpZero";
const OP_DELETE: &str = "Delete";
const OP_ELSE_EQ: &str = "ElseEq";
const OP_EQ: &str = "Eq";
const OP_END_COROUTINE: &str = "EndCoroutine";
const OP_FILTER: &str = "Filter";
const OP_FK_IF_ZERO: &str = "FkIfZero";
const OP_FOUND: &str = "Found";
const OP_GE: &str = "Ge";
const OP_GO_SUB: &str = "Gosub";
const OP_GT: &str = "Gt";
const OP_IDX_GE: &str = "IdxGE";
const OP_IDX_GT: &str = "IdxGT";
const OP_IDX_LE: &str = "IdxLE";
const OP_IDX_LT: &str = "IdxLT";
const OP_IF: &str = "If";
const OP_IF_NO_HOPE: &str = "IfNoHope";
const OP_IF_NOT: &str = "IfNot";
const OP_IF_NOT_OPEN: &str = "IfNotOpen";
const OP_IF_NOT_ZERO: &str = "IfNotZero";
const OP_IF_NULL_ROW: &str = "IfNullRow";
const OP_IF_POS: &str = "IfPos";
const OP_IF_SMALLER: &str = "IfSmaller";
const OP_INCR_VACUUM: &str = "IncrVacuum";
const OP_INIT_COROUTINE: &str = "InitCoroutine";
const OP_IS_NULL: &str = "IsNull";
const OP_IS_NULL_OR_TYPE: &str = "IsNullOrType";
const OP_LAST: &str = "Last";
const OP_LE: &str = "Le";
const OP_LT: &str = "Lt";
const OP_MUST_BE_INT: &str = "MustBeInt";
const OP_NE: &str = "Ne";
const OP_NEXT: &str = "Next";
const OP_NO_CONFLICT: &str = "NoConflict";
const OP_NOT_EXISTS: &str = "NotExists";
const OP_NOT_NULL: &str = "NotNull";
const OP_ONCE: &str = "Once";
const OP_PREV: &str = "Prev";
const OP_PROGRAM: &str = "Program";
const OP_RETURN: &str = "Return";
const OP_REWIND: &str = "Rewind";
const OP_ROW_DATA: &str = "RowData";
const OP_ROW_SET_READ: &str = "RowSetRead";
const OP_ROW_SET_TEST: &str = "RowSetTest";
const OP_SEEK_GE: &str = "SeekGE";
const OP_SEEK_GT: &str = "SeekGT";
const OP_SEEK_LE: &str = "SeekLE";
const OP_SEEK_LT: &str = "SeekLT";
const OP_SEEK_ROW_ID: &str = "SeekRowid";
const OP_SEEK_SCAN: &str = "SeekScan";
const OP_SEQUENCE: &str = "Sequence";
const OP_SEQUENCE_TEST: &str = "SequenceTest";
const OP_SORT: &str = "Sort";
const OP_SORTER_DATA: &str = "SorterData";
const OP_SORTER_INSERT: &str = "SorterInsert";
const OP_SORTER_NEXT: &str = "SorterNext";
const OP_SORTER_OPEN: &str = "SorterOpen";
const OP_SORTER_SORT: &str = "SorterSort";
const OP_V_FILTER: &str = "VFilter";
const OP_V_NEXT: &str = "VNext";
const OP_YIELD: &str = "Yield";
const OP_JUMP: &str = "Jump";
const OP_COLUMN: &str = "Column";
const OP_MAKE_RECORD: &str = "MakeRecord";
const OP_INSERT: &str = "Insert";
const OP_IDX_INSERT: &str = "IdxInsert";
const OP_OPEN_DUP: &str = "OpenDup";
const OP_OPEN_PSEUDO: &str = "OpenPseudo";
const OP_OPEN_READ: &str = "OpenRead";
const OP_OPEN_WRITE: &str = "OpenWrite";
const OP_OPEN_EPHEMERAL: &str = "OpenEphemeral";
const OP_OPEN_AUTOINDEX: &str = "OpenAutoindex";
const OP_AGG_FINAL: &str = "AggFinal";
const OP_AGG_VALUE: &str = "AggValue";
const OP_AGG_STEP: &str = "AggStep";
const OP_FUNCTION: &str = "Function";
const OP_MOVE: &str = "Move";
const OP_COPY: &str = "Copy";
const OP_SCOPY: &str = "SCopy";
const OP_NULL: &str = "Null";
const OP_NULL_ROW: &str = "NullRow";
const OP_INT_COPY: &str = "IntCopy";
const OP_CAST: &str = "Cast";
const OP_STRING8: &str = "String8";
const OP_INT64: &str = "Int64";
const OP_INTEGER: &str = "Integer";
const OP_REAL: &str = "Real";
const OP_NOT: &str = "Not";
const OP_BLOB: &str = "Blob";
const OP_VARIABLE: &str = "Variable";
const OP_COUNT: &str = "Count";
const OP_ROWID: &str = "Rowid";
const OP_NEWROWID: &str = "NewRowid";
const OP_OR: &str = "Or";
const OP_AND: &str = "And";
const OP_BIT_AND: &str = "BitAnd";
const OP_BIT_OR: &str = "BitOr";
const OP_SHIFT_LEFT: &str = "ShiftLeft";
const OP_SHIFT_RIGHT: &str = "ShiftRight";
const OP_ADD: &str = "Add";
const OP_SUBTRACT: &str = "Subtract";
const OP_MULTIPLY: &str = "Multiply";
const OP_DIVIDE: &str = "Divide";
const OP_REMAINDER: &str = "Remainder";
const OP_CONCAT: &str = "Concat";
const OP_OFFSET_LIMIT: &str = "OffsetLimit";
const OP_RESULT_ROW: &str = "ResultRow";
const OP_HALT: &str = "Halt";
const OP_HALT_IF_NULL: &str = "HaltIfNull";

const MAX_LOOP_COUNT: u8 = 2;
const MAX_TOTAL_INSTRUCTION_COUNT: u32 = 100_000;

#[derive(Clone, Eq, PartialEq, Hash)]
enum ColumnType {
    Single {
        datatype: DataType,
        nullable: Option<bool>,
    },
    Record(IntMap<ColumnType>),
}

impl Default for ColumnType {
    fn default() -> Self {
        Self::Single {
            datatype: DataType::Null,
            nullable: None,
        }
    }
}

impl ColumnType {
    fn null() -> Self {
        Self::Single {
            datatype: DataType::Null,
            nullable: Some(true),
        }
    }
    fn map_to_datatype(&self) -> DataType {
        match self {
            Self::Single { datatype, .. } => *datatype,
            Self::Record(_) => DataType::Null, //If we're trying to coerce to a regular Datatype, we can assume a Record is invalid for the context
        }
    }
    fn map_to_nullable(&self) -> Option<bool> {
        match self {
            Self::Single { nullable, .. } => *nullable,
            Self::Record(_) => None, //If we're trying to coerce to a regular Datatype, we can assume a Record is invalid for the context
        }
    }
}

impl core::fmt::Debug for ColumnType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Single { datatype, nullable } => {
                let nullable_str = match nullable {
                    Some(true) => "NULL",
                    Some(false) => "NOT NULL",
                    None => "NULL?",
                };
                write!(f, "{:?} {}", datatype, nullable_str)
            }
            Self::Record(columns) => {
                f.write_str("Record(")?;
                let mut column_iter = columns.iter();
                if let Some(item) = column_iter.next() {
                    write!(f, "{:?}", item)?;
                    for item in column_iter {
                        write!(f, ", {:?}", item)?;
                    }
                }
                f.write_str(")")
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum RegDataType {
    Single(ColumnType),
    Int(i64),
}

impl RegDataType {
    fn map_to_datatype(&self) -> DataType {
        match self {
            RegDataType::Single(d) => d.map_to_datatype(),
            RegDataType::Int(_) => DataType::Integer,
        }
    }
    fn map_to_nullable(&self) -> Option<bool> {
        match self {
            RegDataType::Single(d) => d.map_to_nullable(),
            RegDataType::Int(_) => Some(false),
        }
    }
    fn map_to_columntype(&self) -> ColumnType {
        match self {
            RegDataType::Single(d) => d.clone(),
            RegDataType::Int(_) => ColumnType::Single {
                datatype: DataType::Integer,
                nullable: Some(false),
            },
        }
    }
}

impl Default for RegDataType {
    fn default() -> Self {
        Self::Single(ColumnType::default())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct TableDataType {
    cols: IntMap<ColumnType>,
    is_empty: Option<bool>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum CursorDataType {
    Normal(i64),
    Pseudo(i64),
}

impl CursorDataType {
    fn columns(
        &self,
        tables: &IntMap<TableDataType>,
        registers: &IntMap<RegDataType>,
    ) -> IntMap<ColumnType> {
        match self {
            Self::Normal(i) => match tables.get(i) {
                Some(tab) => tab.cols.clone(),
                None => IntMap::new(),
            },
            Self::Pseudo(i) => match registers.get(i) {
                Some(RegDataType::Single(ColumnType::Record(r))) => r.clone(),
                _ => IntMap::new(),
            },
        }
    }

    fn columns_ref<'s, 'r, 'o>(
        &'s self,
        tables: &'r IntMap<TableDataType>,
        registers: &'r IntMap<RegDataType>,
    ) -> Option<&'o IntMap<ColumnType>>
    where
        's: 'o,
        'r: 'o,
    {
        match self {
            Self::Normal(i) => match tables.get(i) {
                Some(tab) => Some(&tab.cols),
                None => None,
            },
            Self::Pseudo(i) => match registers.get(i) {
                Some(RegDataType::Single(ColumnType::Record(r))) => Some(r),
                _ => None,
            },
        }
    }

    fn columns_mut<'s, 'r, 'o>(
        &'s self,
        tables: &'r mut IntMap<TableDataType>,
        registers: &'r mut IntMap<RegDataType>,
    ) -> Option<&'o mut IntMap<ColumnType>>
    where
        's: 'o,
        'r: 'o,
    {
        match self {
            Self::Normal(i) => match tables.get_mut(i) {
                Some(tab) => Some(&mut tab.cols),
                None => None,
            },
            Self::Pseudo(i) => match registers.get_mut(i) {
                Some(RegDataType::Single(ColumnType::Record(r))) => Some(r),
                _ => None,
            },
        }
    }

    fn table_mut<'s, 'r, 'o>(
        &'s self,
        tables: &'r mut IntMap<TableDataType>,
    ) -> Option<&'o mut TableDataType>
    where
        's: 'o,
        'r: 'o,
    {
        match self {
            Self::Normal(i) => match tables.get_mut(i) {
                Some(tab) => Some(tab),
                None => None,
            },
            _ => None,
        }
    }

    fn is_empty(&self, tables: &IntMap<TableDataType>) -> Option<bool> {
        match self {
            Self::Normal(i) => match tables.get(i) {
                Some(tab) => tab.is_empty,
                None => Some(true),
            },
            Self::Pseudo(_) => Some(false), //pseudo cursors have exactly one row
        }
    }
}

#[allow(clippy::wildcard_in_or_patterns)]
fn affinity_to_type(affinity: u8) -> DataType {
    match affinity {
        SQLITE_AFF_BLOB => DataType::Blob,
        SQLITE_AFF_INTEGER => DataType::Integer,
        SQLITE_AFF_NUMERIC => DataType::Numeric,
        SQLITE_AFF_REAL => DataType::Float,
        SQLITE_AFF_TEXT => DataType::Text,

        SQLITE_AFF_NONE | _ => DataType::Null,
    }
}

#[allow(clippy::wildcard_in_or_patterns)]
fn opcode_to_type(op: &str) -> DataType {
    match op {
        OP_REAL => DataType::Float,
        OP_BLOB => DataType::Blob,
        OP_AND | OP_OR => DataType::Bool,
        OP_NEWROWID | OP_ROWID | OP_COUNT | OP_INT64 | OP_INTEGER => DataType::Integer,
        OP_STRING8 => DataType::Text,
        OP_COLUMN | _ => DataType::Null,
    }
}

fn root_block_columns(
    conn: &mut ConnectionState,
) -> Result<HashMap<(i64, i64), IntMap<ColumnType>>, Error> {
    let table_block_columns: Vec<(i64, i64, i64, String, bool)> = execute::iter(
        conn,
        "SELECT s.dbnum, s.rootpage, col.cid as colnum, col.type, col.\"notnull\"
         FROM (
             select 1 dbnum, tss.* from temp.sqlite_schema tss
             UNION ALL select 0 dbnum, mss.* from main.sqlite_schema mss
             ) s
         JOIN pragma_table_info(s.name) AS col
         WHERE s.type = 'table'
         UNION ALL
         SELECT s.dbnum, s.rootpage, idx.seqno as colnum, col.type, col.\"notnull\"
         FROM (
             select 1 dbnum, tss.* from temp.sqlite_schema tss
             UNION ALL select 0 dbnum, mss.* from main.sqlite_schema mss
             ) s
         JOIN pragma_index_info(s.name) AS idx
         LEFT JOIN pragma_table_info(s.tbl_name) as col
           ON col.cid = idx.cid
           WHERE s.type = 'index'",
        None,
        false,
    )?
    .filter_map(|res| res.map(|either| either.right()).transpose())
    .map(|row| FromRow::from_row(&row?))
    .collect::<Result<Vec<_>, Error>>()?;

    let mut row_info: HashMap<(i64, i64), IntMap<ColumnType>> = HashMap::new();
    for (dbnum, block, colnum, datatype, notnull) in table_block_columns {
        let row_info = row_info.entry((dbnum, block)).or_default();
        row_info.insert(
            colnum,
            ColumnType::Single {
                datatype: datatype.parse().unwrap_or(DataType::Null),
                nullable: Some(!notnull),
            },
        );
    }

    Ok(row_info)
}

struct Sequence(i64);

impl Sequence {
    pub fn new() -> Self {
        Self(0)
    }
    pub fn next(&mut self) -> i64 {
        let curr = self.0;
        self.0 += 1;
        curr
    }
}

#[derive(Debug)]
struct QueryState {
    // The number of times each instruction has been visited
    pub visited: Vec<u8>,
    // A unique identifier of the query branch
    pub branch_id: i64,
    // How many instructions have been executed on this branch (NOT the same as program_i, which is the currently executing instruction of the program)
    pub instruction_counter: i64,
    // Parent branch this branch was forked from (if any)
    pub branch_parent: Option<BranchParent>,
    // State of the virtual machine
    pub mem: MemoryState,
    // Results published by the execution
    pub result: Option<Vec<(Option<SqliteTypeInfo>, Option<bool>)>>,
}

impl From<&QueryState> for MemoryState {
    fn from(val: &QueryState) -> Self {
        val.mem.clone()
    }
}

impl From<QueryState> for MemoryState {
    fn from(val: QueryState) -> Self {
        val.mem
    }
}

impl From<&QueryState> for BranchParent {
    fn from(val: &QueryState) -> Self {
        Self {
            id: val.branch_id,
            idx: val.instruction_counter,
        }
    }
}

impl QueryState {
    fn get_reference(&self) -> BranchParent {
        BranchParent {
            id: self.branch_id,
            idx: self.instruction_counter,
        }
    }
    fn new_branch(&self, branch_seq: &mut Sequence) -> Self {
        Self {
            visited: self.visited.clone(),
            branch_id: branch_seq.next(),
            instruction_counter: 0,
            branch_parent: Some(BranchParent {
                id: self.branch_id,
                idx: self.instruction_counter - 1, //instruction counter is incremented at the start of processing an instruction, so need to subtract 1 to get the 'current' instruction
            }),
            mem: self.mem.clone(),
            result: self.result.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MemoryState {
    // Next instruction to execute
    pub program_i: usize,
    // Registers
    pub r: IntMap<RegDataType>,
    // Rows that pointers point to
    pub p: IntMap<CursorDataType>,
    // Table definitions pointed to by pointers
    pub t: IntMap<TableDataType>,
}

impl DebugDiff for MemoryState {
    fn diff(&self, prev: &Self) -> String {
        let r_diff = self.r.diff(&prev.r);
        let p_diff = self.p.diff(&prev.p);
        let t_diff = self.t.diff(&prev.t);

        let mut differences = String::new();
        for (i, v) in r_diff {
            if !differences.is_empty() {
                differences.push('\n');
            }
            differences.push_str(&format!("r[{}]={:?}", i, v))
        }
        for (i, v) in p_diff {
            if !differences.is_empty() {
                differences.push('\n');
            }
            differences.push_str(&format!("p[{}]={:?}", i, v))
        }
        for (i, v) in t_diff {
            if !differences.is_empty() {
                differences.push('\n');
            }
            differences.push_str(&format!("t[{}]={:?}", i, v))
        }
        differences
    }
}

struct BranchList {
    states: Vec<QueryState>,
    visited_branch_state: HashMap<MemoryState, BranchParent>,
}

impl BranchList {
    pub fn new(state: QueryState) -> Self {
        Self {
            states: vec![state],
            visited_branch_state: HashMap::new(),
        }
    }
    pub fn push<R: Debug, P: Debug>(
        &mut self,
        mut state: QueryState,
        logger: &mut crate::logger::QueryPlanLogger<'_, R, MemoryState, P>,
    ) {
        logger.add_branch(&state, &state.branch_parent.unwrap());
        match self.visited_branch_state.entry(state.mem) {
            hash_map::Entry::Vacant(entry) => {
                //this state is not identical to another state, so it will need to be processed
                state.mem = entry.key().clone(); //replace state.mem since .entry() moved it
                entry.insert(state.get_reference());
                self.states.push(state);
            }
            hash_map::Entry::Occupied(entry) => {
                //already saw a state identical to this one, so no point in processing it
                state.mem = entry.key().clone(); //replace state.mem since .entry() moved it
                logger.add_result(state, BranchResult::Dedup(*entry.get()));
            }
        }
    }
    pub fn pop(&mut self) -> Option<QueryState> {
        self.states.pop()
    }
}

// Opcode Reference: https://sqlite.org/opcode.html
pub(super) fn explain(
    conn: &mut ConnectionState,
    query: &str,
) -> Result<(Vec<SqliteTypeInfo>, Vec<Option<bool>>), Error> {
    let root_block_cols = root_block_columns(conn)?;
    let program: Vec<(i64, String, i64, i64, i64, Vec<u8>)> =
        execute::iter(conn, &format!("EXPLAIN {query}"), None, false)?
            .filter_map(|res| res.map(|either| either.right()).transpose())
            .map(|row| FromRow::from_row(&row?))
            .collect::<Result<Vec<_>, Error>>()?;
    let program_size = program.len();

    let mut logger = crate::logger::QueryPlanLogger::new(query, &program);
    let mut branch_seq = Sequence::new();
    let mut states = BranchList::new(QueryState {
        visited: vec![0; program_size],
        branch_id: branch_seq.next(),
        branch_parent: None,
        instruction_counter: 0,
        result: None,
        mem: MemoryState {
            program_i: 0,
            r: IntMap::new(),
            t: IntMap::new(),
            p: IntMap::new(),
        },
    });

    let mut gas = MAX_TOTAL_INSTRUCTION_COUNT;
    let mut result_states = Vec::new();

    while let Some(mut state) = states.pop() {
        while state.mem.program_i < program_size {
            let (_, ref opcode, p1, p2, p3, ref p4) = program[state.mem.program_i];

            logger.add_operation(state.mem.program_i, &state);
            state.instruction_counter += 1;

            //limit the number of 'instructions' that can be evaluated
            if gas > 0 {
                gas -= 1;
            } else {
                logger.add_result(state, BranchResult::GasLimit);
                break;
            }

            if state.visited[state.mem.program_i] > MAX_LOOP_COUNT {
                logger.add_result(state, BranchResult::LoopLimit);
                //avoid (infinite) loops by breaking if we ever hit the same instruction twice
                break;
            }

            state.visited[state.mem.program_i] += 1;

            match &**opcode {
                OP_INIT => {
                    // start at <p2>
                    state.mem.program_i = p2 as usize;
                    continue;
                }

                OP_GOTO => {
                    // goto <p2>

                    state.mem.program_i = p2 as usize;
                    continue;
                }

                OP_GO_SUB => {
                    // store current instruction in r[p1], goto <p2>
                    state
                        .mem
                        .r
                        .insert(p1, RegDataType::Int(state.mem.program_i as i64));
                    state.mem.program_i = p2 as usize;
                    continue;
                }

                OP_FK_IF_ZERO => {
                    // goto <p2> if no constraints are unsatisfied (assumed to be true)

                    state.mem.program_i = p2 as usize;
                    continue;
                }

                OP_DECR_JUMP_ZERO | OP_ELSE_EQ | OP_EQ | OP_FILTER | OP_FOUND | OP_GE | OP_GT
                | OP_IDX_GE | OP_IDX_GT | OP_IDX_LE | OP_IDX_LT | OP_IF_NO_HOPE | OP_IF_NOT
                | OP_IF_NOT_OPEN | OP_IF_NOT_ZERO | OP_IF_NULL_ROW | OP_IF_SMALLER
                | OP_INCR_VACUUM | OP_IS_NULL_OR_TYPE | OP_LE | OP_LT | OP_NE | OP_NEXT
                | OP_NO_CONFLICT | OP_NOT_EXISTS | OP_ONCE | OP_PREV | OP_PROGRAM
                | OP_ROW_SET_READ | OP_ROW_SET_TEST | OP_SEEK_GE | OP_SEEK_GT | OP_SEEK_LE
                | OP_SEEK_LT | OP_SEEK_ROW_ID | OP_SEEK_SCAN | OP_SEQUENCE_TEST
                | OP_SORTER_NEXT | OP_V_FILTER | OP_V_NEXT => {
                    // goto <p2> or next instruction (depending on actual values)

                    let mut branch_state = state.new_branch(&mut branch_seq);
                    branch_state.mem.program_i = p2 as usize;
                    states.push(branch_state, &mut logger);

                    state.mem.program_i += 1;
                    continue;
                }

                OP_IS_NULL => {
                    // goto <p2> if p1 is null

                    //branch if maybe null
                    let might_branch = match state.mem.r.get(&p1) {
                        Some(r_p1) => !matches!(r_p1.map_to_nullable(), Some(false)),
                        _ => false,
                    };

                    //nobranch if maybe not null
                    let might_not_branch = match state.mem.r.get(&p1) {
                        Some(r_p1) => !matches!(r_p1.map_to_datatype(), DataType::Null),
                        _ => false,
                    };

                    if might_branch {
                        let mut branch_state = state.new_branch(&mut branch_seq);
                        branch_state.mem.program_i = p2 as usize;
                        branch_state
                            .mem
                            .r
                            .insert(p1, RegDataType::Single(ColumnType::default()));

                        states.push(branch_state, &mut logger);
                    }

                    if might_not_branch {
                        state.mem.program_i += 1;
                        if let Some(RegDataType::Single(ColumnType::Single { nullable, .. })) =
                            state.mem.r.get_mut(&p1)
                        {
                            *nullable = Some(false);
                        }
                        continue;
                    } else {
                        logger.add_result(state, BranchResult::Branched);
                        break;
                    }
                }

                OP_NOT_NULL => {
                    // goto <p2> if p1 is not null

                    let might_branch = match state.mem.r.get(&p1) {
                        Some(r_p1) => !matches!(r_p1.map_to_datatype(), DataType::Null),
                        _ => false,
                    };

                    let might_not_branch = match state.mem.r.get(&p1) {
                        Some(r_p1) => !matches!(r_p1.map_to_nullable(), Some(false)),
                        _ => false,
                    };

                    if might_branch {
                        let mut branch_state = state.new_branch(&mut branch_seq);
                        branch_state.mem.program_i = p2 as usize;
                        if let Some(RegDataType::Single(ColumnType::Single { nullable, .. })) =
                            branch_state.mem.r.get_mut(&p1)
                        {
                            *nullable = Some(false);
                        }

                        states.push(branch_state, &mut logger);
                    }

                    if might_not_branch {
                        state.mem.program_i += 1;
                        state
                            .mem
                            .r
                            .insert(p1, RegDataType::Single(ColumnType::default()));
                        continue;
                    } else {
                        logger.add_result(state, BranchResult::Branched);
                        break;
                    }
                }

                OP_MUST_BE_INT => {
                    // if p1 can be coerced to int, continue
                    // if p1 cannot be coerced to int, error if p2 == 0, else jump to p2

                    //don't bother checking actual types, just don't branch to instruction 0
                    if p2 != 0 {
                        let mut branch_state = state.new_branch(&mut branch_seq);
                        branch_state.mem.program_i = p2 as usize;
                        states.push(branch_state, &mut logger);
                    }

                    state.mem.program_i += 1;
                    continue;
                }

                OP_IF => {
                    // goto <p2> if r[p1] is true (1) or r[p1] is null and p3 is nonzero

                    let might_branch = match state.mem.r.get(&p1) {
                        Some(RegDataType::Int(r_p1)) => *r_p1 != 0,
                        _ => true,
                    };

                    let might_not_branch = match state.mem.r.get(&p1) {
                        Some(RegDataType::Int(r_p1)) => *r_p1 == 0,
                        _ => true,
                    };

                    if might_branch {
                        let mut branch_state = state.new_branch(&mut branch_seq);
                        branch_state.mem.program_i = p2 as usize;
                        if p3 == 0 {
                            branch_state.mem.r.insert(p1, RegDataType::Int(1));
                        }

                        states.push(branch_state, &mut logger);
                    }

                    if might_not_branch {
                        state.mem.program_i += 1;
                        if p3 == 0 {
                            state.mem.r.insert(p1, RegDataType::Int(0));
                        }
                        continue;
                    } else {
                        logger.add_result(state, BranchResult::Branched);
                        break;
                    }
                }

                OP_IF_POS => {
                    // goto <p2> if r[p1] is true (1) or r[p1] is null and p3 is nonzero

                    // as a workaround for large offset clauses, both branches will be attempted after 1 loop

                    let might_branch = match state.mem.r.get(&p1) {
                        Some(RegDataType::Int(r_p1)) => *r_p1 >= 1,
                        _ => true,
                    };

                    let might_not_branch = match state.mem.r.get(&p1) {
                        Some(RegDataType::Int(r_p1)) => *r_p1 < 1,
                        _ => true,
                    };

                    let loop_detected = state.visited[state.mem.program_i] > 1;
                    if might_branch || loop_detected {
                        let mut branch_state = state.new_branch(&mut branch_seq);
                        branch_state.mem.program_i = p2 as usize;
                        if let Some(RegDataType::Int(r_p1)) = branch_state.mem.r.get_mut(&p1) {
                            *r_p1 -= 1;
                        }
                        states.push(branch_state, &mut logger);
                    }

                    if might_not_branch {
                        state.mem.program_i += 1;
                        continue;
                    } else if loop_detected {
                        state.mem.program_i += 1;
                        if matches!(state.mem.r.get_mut(&p1), Some(RegDataType::Int(..))) {
                            //forget the exact value, in case some later cares
                            state.mem.r.insert(
                                p1,
                                RegDataType::Single(ColumnType::Single {
                                    datatype: DataType::Integer,
                                    nullable: Some(false),
                                }),
                            );
                        }
                        continue;
                    } else {
                        logger.add_result(state, BranchResult::Branched);
                        break;
                    }
                }

                OP_REWIND | OP_LAST | OP_SORT | OP_SORTER_SORT => {
                    // goto <p2> if cursor p1 is empty and p2 != 0, else next instruction

                    if p2 == 0 {
                        state.mem.program_i += 1;
                        continue;
                    }

                    if let Some(cursor) = state.mem.p.get(&p1) {
                        if matches!(cursor.is_empty(&state.mem.t), None | Some(true)) {
                            //only take this branch if the cursor is empty

                            let mut branch_state = state.new_branch(&mut branch_seq);
                            branch_state.mem.program_i = p2 as usize;

                            if let Some(cur) = branch_state.mem.p.get(&p1) {
                                if let Some(tab) = cur.table_mut(&mut branch_state.mem.t) {
                                    tab.is_empty = Some(true);
                                }
                            }
                            states.push(branch_state, &mut logger);
                        }

                        if matches!(cursor.is_empty(&state.mem.t), None | Some(false)) {
                            //only take this branch if the cursor is non-empty
                            state.mem.program_i += 1;
                            continue;
                        } else {
                            logger.add_result(state, BranchResult::Branched);
                            break;
                        }
                    }

                    logger.add_result(state, BranchResult::Branched);
                    break;
                }

                OP_INIT_COROUTINE => {
                    // goto <p2> or next instruction (depending on actual values)

                    state.mem.r.insert(p1, RegDataType::Int(p3));

                    if p2 != 0 {
                        state.mem.program_i = p2 as usize;
                    } else {
                        state.mem.program_i += 1;
                    }
                    continue;
                }

                OP_END_COROUTINE => {
                    // jump to p2 of the yield instruction pointed at by register p1

                    if let Some(RegDataType::Int(yield_i)) = state.mem.r.get(&p1) {
                        if let Some((_, yield_op, _, yield_p2, _, _)) =
                            program.get(*yield_i as usize)
                        {
                            if OP_YIELD == yield_op.as_str() {
                                state.mem.program_i = (*yield_p2) as usize;
                                state.mem.r.remove(&p1);
                                continue;
                            } else {
                                logger.add_result(state, BranchResult::Error);
                                break;
                            }
                        } else {
                            logger.add_result(state, BranchResult::Error);
                            break;
                        }
                    } else {
                        logger.add_result(state, BranchResult::Error);
                        break;
                    }
                }

                OP_RETURN => {
                    // jump to the instruction after the instruction pointed at by register p1

                    if let Some(RegDataType::Int(return_i)) = state.mem.r.get(&p1) {
                        state.mem.program_i = (*return_i + 1) as usize;
                        state.mem.r.remove(&p1);
                        continue;
                    } else if p3 == 1 {
                        state.mem.program_i += 1;
                        continue;
                    } else {
                        logger.add_result(state, BranchResult::Error);
                        break;
                    }
                }

                OP_YIELD => {
                    // jump to p2 of the yield instruction pointed at by register p1, store prior instruction in p1

                    if let Some(RegDataType::Int(yield_i)) = state.mem.r.get_mut(&p1) {
                        let program_i: usize = state.mem.program_i;

                        //if yielding to a yield operation, go to the NEXT instruction after that instruction
                        if program
                            .get(*yield_i as usize)
                            .map(|(_, yield_op, _, _, _, _)| yield_op.as_str())
                            == Some(OP_YIELD)
                        {
                            state.mem.program_i = (*yield_i + 1) as usize;
                            *yield_i = program_i as i64;
                            continue;
                        } else {
                            state.mem.program_i = *yield_i as usize;
                            *yield_i = program_i as i64;
                            continue;
                        }
                    } else {
                        logger.add_result(state, BranchResult::Error);
                        break;
                    }
                }

                OP_JUMP => {
                    // goto one of <p1>, <p2>, or <p3> based on the result of a prior compare

                    let mut branch_state = state.new_branch(&mut branch_seq);
                    branch_state.mem.program_i = p1 as usize;
                    states.push(branch_state, &mut logger);

                    let mut branch_state = state.new_branch(&mut branch_seq);
                    branch_state.mem.program_i = p2 as usize;
                    states.push(branch_state, &mut logger);

                    let mut branch_state = state.new_branch(&mut branch_seq);
                    branch_state.mem.program_i = p3 as usize;
                    states.push(branch_state, &mut logger);
                }

                OP_COLUMN => {
                    //Get the row stored at p1, or NULL; get the column stored at p2, or NULL
                    let value: ColumnType = state
                        .mem
                        .p
                        .get(&p1)
                        .and_then(|c| c.columns_ref(&state.mem.t, &state.mem.r))
                        .and_then(|cc| cc.get(&p2))
                        .cloned()
                        .unwrap_or_default();

                    // insert into p3 the datatype of the col
                    state.mem.r.insert(p3, RegDataType::Single(value));
                }

                OP_SEQUENCE => {
                    //Copy sequence number from cursor p1 to register p2, increment cursor p1 sequence number

                    //Cursor emulation doesn't sequence value, but it is an int
                    state.mem.r.insert(
                        p2,
                        RegDataType::Single(ColumnType::Single {
                            datatype: DataType::Integer,
                            nullable: Some(false),
                        }),
                    );
                }

                OP_ROW_DATA | OP_SORTER_DATA => {
                    //Get entire row from cursor p1, store it into register p2
                    if let Some(record) = state
                        .mem
                        .p
                        .get(&p1)
                        .map(|c| c.columns(&state.mem.t, &state.mem.r))
                    {
                        state
                            .mem
                            .r
                            .insert(p2, RegDataType::Single(ColumnType::Record(record)));
                    } else {
                        state
                            .mem
                            .r
                            .insert(p2, RegDataType::Single(ColumnType::Record(IntMap::new())));
                    }
                }

                OP_MAKE_RECORD => {
                    // p3 = Record([p1 .. p1 + p2])
                    let mut record = Vec::with_capacity(p2 as usize);
                    for reg in p1..p1 + p2 {
                        record.push(
                            state
                                .mem
                                .r
                                .get(&reg)
                                .map(|d| d.map_to_columntype())
                                .unwrap_or(ColumnType::default()),
                        );
                    }
                    state.mem.r.insert(
                        p3,
                        RegDataType::Single(ColumnType::Record(IntMap::from_dense_record(&record))),
                    );
                }

                OP_INSERT | OP_IDX_INSERT | OP_SORTER_INSERT => {
                    if let Some(RegDataType::Single(columntype)) = state.mem.r.get(&p2) {
                        match columntype {
                            ColumnType::Record(record) => {
                                if let Some(TableDataType { cols, is_empty }) = state
                                    .mem
                                    .p
                                    .get(&p1)
                                    .and_then(|cur| cur.table_mut(&mut state.mem.t))
                                {
                                    // Insert the record into wherever pointer p1 is
                                    *cols = record.clone();
                                    *is_empty = Some(false);
                                }
                            }
                            ColumnType::Single {
                                datatype: DataType::Null,
                                nullable: _,
                            } => {
                                if let Some(TableDataType { is_empty, .. }) = state
                                    .mem
                                    .p
                                    .get(&p1)
                                    .and_then(|cur| cur.table_mut(&mut state.mem.t))
                                {
                                    // Insert a null record into wherever pointer p1 is
                                    *is_empty = Some(false);
                                }
                            }
                            _ => {}
                        }
                    }
                    //Noop if the register p2 isn't a record, or if pointer p1 does not exist
                }

                OP_DELETE => {
                    // delete a record from cursor p1
                    if let Some(TableDataType { is_empty, .. }) = state
                        .mem
                        .p
                        .get(&p1)
                        .and_then(|cur| cur.table_mut(&mut state.mem.t))
                    {
                        if *is_empty == Some(false) {
                            *is_empty = None; //the cursor might be empty now
                        }
                    }
                }

                OP_OPEN_PSEUDO => {
                    // Create a cursor p1 aliasing the record from register p2
                    state.mem.p.insert(p1, CursorDataType::Pseudo(p2));
                }

                OP_OPEN_DUP => {
                    if let Some(cur) = state.mem.p.get(&p2) {
                        state.mem.p.insert(p1, cur.clone());
                    }
                }

                OP_OPEN_READ | OP_OPEN_WRITE => {
                    //Create a new pointer which is referenced by p1, take column metadata from db schema if found
                    let table_info = if p3 == 0 || p3 == 1 {
                        if let Some(columns) = root_block_cols.get(&(p3, p2)) {
                            TableDataType {
                                cols: columns.clone(),
                                is_empty: None,
                            }
                        } else {
                            TableDataType {
                                cols: IntMap::new(),
                                is_empty: None,
                            }
                        }
                    } else {
                        TableDataType {
                            cols: IntMap::new(),
                            is_empty: None,
                        }
                    };

                    state.mem.t.insert(state.mem.program_i as i64, table_info);
                    state
                        .mem
                        .p
                        .insert(p1, CursorDataType::Normal(state.mem.program_i as i64));
                }

                OP_OPEN_EPHEMERAL | OP_OPEN_AUTOINDEX | OP_SORTER_OPEN => {
                    //Create a new pointer which is referenced by p1
                    let table_info = TableDataType {
                        cols: IntMap::from_elem(ColumnType::null(), p2 as usize),
                        is_empty: Some(true),
                    };

                    state.mem.t.insert(state.mem.program_i as i64, table_info);
                    state
                        .mem
                        .p
                        .insert(p1, CursorDataType::Normal(state.mem.program_i as i64));
                }

                OP_VARIABLE => {
                    // r[p2] = <value of variable>
                    state
                        .mem
                        .r
                        .insert(p2, RegDataType::Single(ColumnType::null()));
                }

                // if there is a value in p3, and the query passes, then
                // we know that it is not nullable
                OP_HALT_IF_NULL => {
                    if let Some(RegDataType::Single(ColumnType::Single { nullable, .. })) =
                        state.mem.r.get_mut(&p3)
                    {
                        *nullable = Some(false);
                    }
                }

                OP_FUNCTION => {
                    // r[p3] = func( _ ), registered function name is in p4
                    match from_utf8(p4).map_err(Error::protocol)? {
                        "last_insert_rowid(0)" => {
                            // last_insert_rowid() -> INTEGER
                            state.mem.r.insert(
                                p3,
                                RegDataType::Single(ColumnType::Single {
                                    datatype: DataType::Integer,
                                    nullable: Some(false),
                                }),
                            );
                        }
                        "date(-1)" | "time(-1)" | "datetime(-1)" | "strftime(-1)" => {
                            // date|time|datetime|strftime(...) -> TEXT
                            state.mem.r.insert(
                                p3,
                                RegDataType::Single(ColumnType::Single {
                                    datatype: DataType::Text,
                                    nullable: Some(p2 != 0), //never a null result if no argument provided
                                }),
                            );
                        }
                        "julianday(-1)" => {
                            // julianday(...) -> REAL
                            state.mem.r.insert(
                                p3,
                                RegDataType::Single(ColumnType::Single {
                                    datatype: DataType::Float,
                                    nullable: Some(p2 != 0), //never a null result if no argument provided
                                }),
                            );
                        }
                        "unixepoch(-1)" => {
                            // unixepoch(p2...) -> INTEGER
                            state.mem.r.insert(
                                p3,
                                RegDataType::Single(ColumnType::Single {
                                    datatype: DataType::Integer,
                                    nullable: Some(p2 != 0), //never a null result if no argument provided
                                }),
                            );
                        }

                        _ => logger.add_unknown_operation(state.mem.program_i),
                    }
                }

                OP_NULL_ROW => {
                    // all columns in cursor X are potentially nullable
                    if let Some(cols) = state
                        .mem
                        .p
                        .get_mut(&p1)
                        .and_then(|c| c.columns_mut(&mut state.mem.t, &mut state.mem.r))
                    {
                        for col in cols.values_mut() {
                            if let ColumnType::Single {
                                ref mut nullable, ..
                            } = col
                            {
                                *nullable = Some(true);
                            }
                        }
                    }
                    //else we don't know about the cursor
                }

                OP_AGG_STEP | OP_AGG_VALUE => {
                    //assume that AGG_FINAL will be called
                    let p4 = from_utf8(p4).map_err(Error::protocol)?;

                    if p4.starts_with("count(")
                        || p4.starts_with("row_number(")
                        || p4.starts_with("rank(")
                        || p4.starts_with("dense_rank(")
                        || p4.starts_with("ntile(")
                    {
                        // count(_) -> INTEGER
                        state.mem.r.insert(
                            p3,
                            RegDataType::Single(ColumnType::Single {
                                datatype: DataType::Integer,
                                nullable: Some(false),
                            }),
                        );
                    } else if p4.starts_with("percent_rank(") || p4.starts_with("cume_dist") {
                        // percent_rank(_) -> REAL
                        state.mem.r.insert(
                            p3,
                            RegDataType::Single(ColumnType::Single {
                                datatype: DataType::Float,
                                nullable: Some(false),
                            }),
                        );
                    } else if p4.starts_with("sum(") {
                        if let Some(r_p2) = state.mem.r.get(&p2) {
                            let datatype = match r_p2.map_to_datatype() {
                                // The result of a `SUM()` can be arbitrarily large
                                DataType::Integer | DataType::Int4 | DataType::Bool => {
                                    DataType::Integer
                                }
                                _ => DataType::Float,
                            };
                            let nullable = r_p2.map_to_nullable();
                            state.mem.r.insert(
                                p3,
                                RegDataType::Single(ColumnType::Single { datatype, nullable }),
                            );
                        }
                    } else if p4.starts_with("lead(") || p4.starts_with("lag(") {
                        if let Some(r_p2) = state.mem.r.get(&p2) {
                            let datatype = r_p2.map_to_datatype();
                            state.mem.r.insert(
                                p3,
                                RegDataType::Single(ColumnType::Single {
                                    datatype,
                                    nullable: Some(true),
                                }),
                            );
                        }
                    } else if let Some(v) = state.mem.r.get(&p2).cloned() {
                        // r[p3] = AGG ( r[p2] )
                        state.mem.r.insert(p3, v);
                    }
                }

                OP_AGG_FINAL => {
                    let p4 = from_utf8(p4).map_err(Error::protocol)?;

                    if p4.starts_with("count(")
                        || p4.starts_with("row_number(")
                        || p4.starts_with("rank(")
                        || p4.starts_with("dense_rank(")
                        || p4.starts_with("ntile(")
                    {
                        // count(_) -> INTEGER
                        state.mem.r.insert(
                            p1,
                            RegDataType::Single(ColumnType::Single {
                                datatype: DataType::Integer,
                                nullable: Some(false),
                            }),
                        );
                    } else if p4.starts_with("percent_rank(") || p4.starts_with("cume_dist") {
                        // percent_rank(_) -> REAL
                        state.mem.r.insert(
                            p3,
                            RegDataType::Single(ColumnType::Single {
                                datatype: DataType::Float,
                                nullable: Some(false),
                            }),
                        );
                    } else if p4.starts_with("lead(") || p4.starts_with("lag(") {
                        if let Some(r_p2) = state.mem.r.get(&p2) {
                            let datatype = r_p2.map_to_datatype();
                            state.mem.r.insert(
                                p3,
                                RegDataType::Single(ColumnType::Single {
                                    datatype,
                                    nullable: Some(true),
                                }),
                            );
                        }
                    }
                }

                OP_CAST => {
                    // affinity(r[p1])
                    if let Some(v) = state.mem.r.get_mut(&p1) {
                        *v = RegDataType::Single(ColumnType::Single {
                            datatype: affinity_to_type(p2 as u8),
                            nullable: v.map_to_nullable(),
                        });
                    }
                }

                OP_SCOPY | OP_INT_COPY => {
                    // r[p2] = r[p1]
                    if let Some(v) = state.mem.r.get(&p1).cloned() {
                        state.mem.r.insert(p2, v);
                    }
                }

                OP_COPY => {
                    // r[p2..=p2+p3] = r[p1..=p1+p3]
                    if p3 >= 0 {
                        for i in 0..=p3 {
                            let src = p1 + i;
                            let dst = p2 + i;
                            if let Some(v) = state.mem.r.get(&src).cloned() {
                                state.mem.r.insert(dst, v);
                            }
                        }
                    }
                }

                OP_MOVE => {
                    // r[p2..p2+p3] = r[p1..p1+p3]; r[p1..p1+p3] = null
                    if p3 >= 1 {
                        for i in 0..p3 {
                            let src = p1 + i;
                            let dst = p2 + i;
                            if let Some(v) = state.mem.r.get(&src).cloned() {
                                state.mem.r.insert(dst, v);
                                state
                                    .mem
                                    .r
                                    .insert(src, RegDataType::Single(ColumnType::null()));
                            }
                        }
                    }
                }

                OP_INTEGER => {
                    // r[p2] = p1
                    state.mem.r.insert(p2, RegDataType::Int(p1));
                }

                OP_BLOB | OP_COUNT | OP_REAL | OP_STRING8 | OP_ROWID | OP_NEWROWID => {
                    // r[p2] = <value of constant>
                    state.mem.r.insert(
                        p2,
                        RegDataType::Single(ColumnType::Single {
                            datatype: opcode_to_type(opcode),
                            nullable: Some(false),
                        }),
                    );
                }

                OP_NOT => {
                    // r[p2] = NOT r[p1]
                    if let Some(a) = state.mem.r.get(&p1).cloned() {
                        state.mem.r.insert(p2, a);
                    }
                }

                OP_NULL => {
                    // r[p2..p3] = null
                    let idx_range = if p2 < p3 { p2..=p3 } else { p2..=p2 };

                    for idx in idx_range {
                        state
                            .mem
                            .r
                            .insert(idx, RegDataType::Single(ColumnType::null()));
                    }
                }

                OP_OR | OP_AND | OP_BIT_AND | OP_BIT_OR | OP_SHIFT_LEFT | OP_SHIFT_RIGHT
                | OP_ADD | OP_SUBTRACT | OP_MULTIPLY | OP_DIVIDE | OP_REMAINDER | OP_CONCAT => {
                    // r[p3] = r[p1] + r[p2]
                    let value = match (state.mem.r.get(&p1), state.mem.r.get(&p2)) {
                        (Some(a), Some(b)) => RegDataType::Single(ColumnType::Single {
                            datatype: if matches!(a.map_to_datatype(), DataType::Null) {
                                b.map_to_datatype()
                            } else {
                                a.map_to_datatype()
                            },
                            nullable: match (a.map_to_nullable(), b.map_to_nullable()) {
                                (Some(a_n), Some(b_n)) => Some(a_n | b_n),
                                (Some(a_n), None) => Some(a_n),
                                (None, Some(b_n)) => Some(b_n),
                                (None, None) => None,
                            },
                        }),
                        (Some(v), None) => RegDataType::Single(ColumnType::Single {
                            datatype: v.map_to_datatype(),
                            nullable: None,
                        }),
                        (None, Some(v)) => RegDataType::Single(ColumnType::Single {
                            datatype: v.map_to_datatype(),
                            nullable: None,
                        }),
                        _ => RegDataType::default(),
                    };

                    state.mem.r.insert(p3, value);
                }

                OP_OFFSET_LIMIT => {
                    // r[p2] = if r[p2] < 0 { r[p1] } else if r[p1]<0 { -1 } else { r[p1] + r[p3] }
                    state.mem.r.insert(
                        p2,
                        RegDataType::Single(ColumnType::Single {
                            datatype: DataType::Integer,
                            nullable: Some(false),
                        }),
                    );
                }

                OP_RESULT_ROW => {
                    // output = r[p1 .. p1 + p2]
                    let result: Vec<_> = (p1..p1 + p2)
                        .map(|i| {
                            state
                                .mem
                                .r
                                .get(&i)
                                .map(RegDataType::map_to_columntype)
                                .unwrap_or_default()
                        })
                        .collect();

                    let mut branch_state = state.new_branch(&mut branch_seq);
                    branch_state.mem.program_i += 1;
                    states.push(branch_state, &mut logger);

                    logger.add_result(
                        state,
                        BranchResult::Result(IntMap::from_dense_record(&result)),
                    );

                    result_states.push(result);
                    break;
                }

                OP_HALT => {
                    logger.add_result(state, BranchResult::Halt);
                    break;
                }

                _ => {
                    // ignore unsupported operations
                    // if we fail to find an r later, we just give up
                    logger.add_unknown_operation(state.mem.program_i);
                }
            }

            state.mem.program_i += 1;
        }
    }

    let mut output: Vec<Option<SqliteTypeInfo>> = Vec::new();
    let mut nullable: Vec<Option<bool>> = Vec::new();

    while let Some(result) = result_states.pop() {
        // find the datatype info from each ResultRow execution
        for (idx, this_col) in result.into_iter().enumerate() {
            let this_type = this_col.map_to_datatype();
            let this_nullable = this_col.map_to_nullable();
            if output.len() == idx {
                output.push(Some(SqliteTypeInfo(this_type)));
            } else if output[idx].is_none()
                || matches!(output[idx], Some(SqliteTypeInfo(DataType::Null)))
                    && !matches!(this_type, DataType::Null)
            {
                output[idx] = Some(SqliteTypeInfo(this_type));
            }

            if nullable.len() == idx {
                nullable.push(this_nullable);
            } else if let Some(ref mut null) = nullable[idx] {
                //if any ResultRow's column is nullable, the final result is nullable
                if let Some(this_null) = this_nullable {
                    *null |= this_null;
                }
            } else {
                nullable[idx] = this_nullable;
            }
        }
    }

    let output = output
        .into_iter()
        .map(|o| o.unwrap_or(SqliteTypeInfo(DataType::Null)))
        .collect();

    Ok((output, nullable))
}

#[test]
fn test_root_block_columns_has_types() {
    use crate::SqliteConnectOptions;
    use std::str::FromStr;
    let conn_options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
    let mut conn = super::EstablishParams::from_options(&conn_options)
        .unwrap()
        .establish()
        .unwrap();

    assert!(execute::iter(
        &mut conn,
        r"CREATE TABLE t(a INTEGER PRIMARY KEY, b_null TEXT NULL, b TEXT NOT NULL);",
        None,
        false
    )
    .unwrap()
    .next()
    .is_some());
    assert!(
        execute::iter(&mut conn, r"CREATE INDEX i1 on t (a,b_null);", None, false)
            .unwrap()
            .next()
            .is_some()
    );
    assert!(execute::iter(
        &mut conn,
        r"CREATE UNIQUE INDEX i2 on t (a,b_null);",
        None,
        false
    )
    .unwrap()
    .next()
    .is_some());
    assert!(execute::iter(
        &mut conn,
        r"CREATE TABLE t2(a INTEGER NOT NULL, b_null NUMERIC NULL, b NUMERIC NOT NULL);",
        None,
        false
    )
    .unwrap()
    .next()
    .is_some());
    assert!(execute::iter(
        &mut conn,
        r"CREATE INDEX t2i1 on t2 (a,b_null);",
        None,
        false
    )
    .unwrap()
    .next()
    .is_some());
    assert!(execute::iter(
        &mut conn,
        r"CREATE UNIQUE INDEX t2i2 on t2 (a,b);",
        None,
        false
    )
    .unwrap()
    .next()
    .is_some());

    assert!(execute::iter(
        &mut conn,
        r"CREATE TEMPORARY TABLE t3(a TEXT PRIMARY KEY, b REAL NOT NULL, b_null REAL NULL);",
        None,
        false
    )
    .unwrap()
    .next()
    .is_some());

    let table_block_nums: HashMap<String, (i64,i64)> = execute::iter(
        &mut conn,
        r"select name, 0 db_seq, rootpage from main.sqlite_schema UNION ALL select name, 1 db_seq, rootpage from temp.sqlite_schema",
        None,
        false,
    )
    .unwrap()
    .filter_map(|res| res.map(|either| either.right()).transpose())
    .map(|row| FromRow::from_row(row.as_ref().unwrap()))
    .map(|row| row.map(|(name,seq,block)|(name,(seq,block))))
    .collect::<Result<HashMap<_, _>, Error>>()
    .unwrap();

    let root_block_cols = root_block_columns(&mut conn).unwrap();

    // there should be 7 tables/indexes created explicitly, plus 1 autoindex for t3
    assert_eq!(8, root_block_cols.len());

    //prove that we have some information for each table & index
    for (name, db_seq_block) in dbg!(&table_block_nums) {
        assert!(
            root_block_cols.contains_key(db_seq_block),
            "{:?}",
            (name, db_seq_block)
        );
    }

    //prove that each block has the correct information
    {
        let table_db_block = table_block_nums["t"];
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Integer,
                nullable: Some(true) //sqlite primary key columns are nullable unless declared not null
            }),
            root_block_cols[&table_db_block].get(&0)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Text,
                nullable: Some(true)
            }),
            root_block_cols[&table_db_block].get(&1)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Text,
                nullable: Some(false)
            }),
            root_block_cols[&table_db_block].get(&2)
        );
    }

    {
        let table_db_block = table_block_nums["i1"];
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Integer,
                nullable: Some(true) //sqlite primary key columns are nullable unless declared not null
            }),
            root_block_cols[&table_db_block].get(&0)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Text,
                nullable: Some(true)
            }),
            root_block_cols[&table_db_block].get(&1)
        );
    }

    {
        let table_db_block = table_block_nums["i2"];
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Integer,
                nullable: Some(true) //sqlite primary key columns are nullable unless declared not null
            }),
            root_block_cols[&table_db_block].get(&0)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Text,
                nullable: Some(true)
            }),
            root_block_cols[&table_db_block].get(&1)
        );
    }

    {
        let table_db_block = table_block_nums["t2"];
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Integer,
                nullable: Some(false)
            }),
            root_block_cols[&table_db_block].get(&0)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Null,
                nullable: Some(true)
            }),
            root_block_cols[&table_db_block].get(&1)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Null,
                nullable: Some(false)
            }),
            root_block_cols[&table_db_block].get(&2)
        );
    }

    {
        let table_db_block = table_block_nums["t2i1"];
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Integer,
                nullable: Some(false)
            }),
            root_block_cols[&table_db_block].get(&0)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Null,
                nullable: Some(true)
            }),
            root_block_cols[&table_db_block].get(&1)
        );
    }

    {
        let table_db_block = table_block_nums["t2i2"];
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Integer,
                nullable: Some(false)
            }),
            root_block_cols[&table_db_block].get(&0)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Null,
                nullable: Some(false)
            }),
            root_block_cols[&table_db_block].get(&1)
        );
    }

    {
        let table_db_block = table_block_nums["t3"];
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Text,
                nullable: Some(true)
            }),
            root_block_cols[&table_db_block].get(&0)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Float,
                nullable: Some(false)
            }),
            root_block_cols[&table_db_block].get(&1)
        );
        assert_eq!(
            Some(&ColumnType::Single {
                datatype: DataType::Float,
                nullable: Some(true)
            }),
            root_block_cols[&table_db_block].get(&2)
        );
    }
}
