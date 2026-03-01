//! Core types: Date arithmetic, constraints, puzzles, and solver.
//!
//! Replaces chrono with pure-integer date math for no_std WASM compatibility.
//! All date operations use serial day numbers (days since 0000-03-01).

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;
use serde::{Deserialize, Serialize};

// ═════════════════════════════════════════════════════════════════════
// Weekday
// ═════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub fn from_index(i: u32) -> Self {
        match i % 7 {
            0 => Weekday::Mon,
            1 => Weekday::Tue,
            2 => Weekday::Wed,
            3 => Weekday::Thu,
            4 => Weekday::Fri,
            5 => Weekday::Sat,
            _ => Weekday::Sun,
        }
    }
}

// ═════════════════════════════════════════════════════════════════════
// Date (pure-integer, no_std)
// ═════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    pub fn new(year: i32, month: u32, day: u32) -> Option<Self> {
        if month < 1 || month > 12 || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        Some(Date { year, month, day })
    }

    /// Serial day number (Rata Die variant). Uses the algorithm from
    /// Howard Hinnant's date library, epoch = 0000-03-01.
    pub fn to_serial(&self) -> i64 {
        let (y, m) = if self.month <= 2 {
            (self.year as i64 - 1, self.month as i64 + 9)
        } else {
            (self.year as i64, self.month as i64 - 3)
        };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    pub fn from_serial(days: i64) -> Self {
        let z = days + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = if month <= 2 { y + 1 } else { y } as i32;
        Date { year, month, day }
    }

    pub fn add_days(self, n: i64) -> Self {
        Self::from_serial(self.to_serial() + n)
    }

    pub fn days_until(self, other: Self) -> i64 {
        other.to_serial() - self.to_serial()
    }

    pub fn weekday(&self) -> Weekday {
        let d = self.to_serial();
        // 2000-01-03 (serial 10960) = Monday
        let w = ((d % 7) + 7 + 3) % 7; // Monday = 0
        Weekday::from_index(w as u32)
    }

    pub fn succ(self) -> Self {
        self.add_days(1)
    }
    pub fn pred(self) -> Self {
        self.add_days(-1)
    }
}

impl PartialOrd for Date {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Date {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_serial().cmp(&other.to_serial())
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

// ═════════════════════════════════════════════════════════════════════
// Temporal constraints
// ═════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    Exact(Date),
    After(Date),
    Before(Date),
    Between(Date, Date),
    DayOfWeek(Weekday),
    DaysAfter(String, i64),
    DaysBefore(String, i64),
    InMonth(u32),
    InYear(i32),
    DayOfMonth(u32),
}

pub fn constraint_type_name(c: &Constraint) -> &'static str {
    match c {
        Constraint::Exact(_) => "Exact",
        Constraint::After(_) => "After",
        Constraint::Before(_) => "Before",
        Constraint::Between(_, _) => "Between",
        Constraint::DayOfWeek(_) => "DayOfWeek",
        Constraint::DaysAfter(_, _) => "DaysAfter",
        Constraint::DaysBefore(_, _) => "DaysBefore",
        Constraint::InMonth(_) => "InMonth",
        Constraint::InYear(_) => "InYear",
        Constraint::DayOfMonth(_) => "DayOfMonth",
    }
}

// ═════════════════════════════════════════════════════════════════════
// Puzzle
// ═════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Puzzle {
    pub id: String,
    pub constraints: Vec<Constraint>,
    pub references: BTreeMap<String, Date>,
    pub solutions: Vec<Date>,
    pub difficulty: u8,
}

impl Puzzle {
    pub fn check_date(&self, date: Date) -> bool {
        self.constraints.iter().all(|c| check_one(date, c, &self.references))
    }
}

fn check_one(date: Date, c: &Constraint, refs: &BTreeMap<String, Date>) -> bool {
    match c {
        Constraint::Exact(d) => date == *d,
        Constraint::After(d) => date > *d,
        Constraint::Before(d) => date < *d,
        Constraint::Between(a, b) => date >= *a && date <= *b,
        Constraint::DayOfWeek(w) => date.weekday() == *w,
        Constraint::DaysAfter(name, n) => {
            refs.get(name).map(|r| date == r.add_days(*n)).unwrap_or(false)
        }
        Constraint::DaysBefore(name, n) => {
            refs.get(name).map(|r| date == r.add_days(-*n)).unwrap_or(false)
        }
        Constraint::InMonth(m) => date.month == *m,
        Constraint::InYear(y) => date.year == *y,
        Constraint::DayOfMonth(d) => date.day == *d,
    }
}

// ═════════════════════════════════════════════════════════════════════
// Deterministic RNG (xorshift64)
// ═════════════════════════════════════════════════════════════════════

pub struct Rng64(pub u64);

impl Rng64 {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn next_f64(&mut self) -> f64 {
        self.next_u64() as f64 / u64::MAX as f64
    }
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo + 1) as u64) as i32
    }
}
