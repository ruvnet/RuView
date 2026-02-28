//! Motion planning: A\* grid search and potential-field velocity commands.
//!
//! Operates on the [`OccupancyGrid`](crate::bridge::OccupancyGrid) type from
//! the bridge module.  Two planners are provided:
//!
//! - [`astar`]: discrete A\* on the occupancy grid returning a cell path.
//! - [`potential_field`]: continuous-space repulsive/attractive field producing
//!   a velocity command.

use crate::bridge::OccupancyGrid;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// A 2-D grid cell coordinate.
pub type Cell = (usize, usize);

/// Result of an A\* search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPath {
    /// Sequence of `(x, y)` cells from start to goal (inclusive).
    pub cells: Vec<Cell>,
    /// Total traversal cost.
    pub cost: f64,
}

/// Errors from planning operations.
#[derive(Debug, thiserror::Error)]
pub enum PlanningError {
    #[error("start cell ({0}, {1}) is out of bounds or occupied")]
    InvalidStart(usize, usize),
    #[error("goal cell ({0}, {1}) is out of bounds or occupied")]
    InvalidGoal(usize, usize),
    #[error("no feasible path found")]
    NoPath,
}

pub type Result<T> = std::result::Result<T, PlanningError>;

// ---------------------------------------------------------------------------
// A* search
// ---------------------------------------------------------------------------

/// Occupancy value above which a cell is considered blocked.
const OCCUPIED_THRESHOLD: f32 = 0.5;

#[derive(PartialEq)]
struct AStarEntry {
    cell: Cell,
    f: f64,
}
impl Eq for AStarEntry {}
impl Ord for AStarEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for AStarEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Run A\* on `grid`, returning the shortest [`GridPath`] from `start` to
/// `goal`.  Cells with occupancy >= 0.5 are treated as impassable.
///
/// Diagonal moves cost √2, cardinal moves cost 1.
pub fn astar(
    grid: &OccupancyGrid,
    start: Cell,
    goal: Cell,
) -> Result<GridPath> {
    if !cell_free(grid, start) {
        return Err(PlanningError::InvalidStart(start.0, start.1));
    }
    if !cell_free(grid, goal) {
        return Err(PlanningError::InvalidGoal(goal.0, goal.1));
    }
    if start == goal {
        return Ok(GridPath { cells: vec![start], cost: 0.0 });
    }

    let mut g_score: HashMap<Cell, f64> = HashMap::with_capacity(128);
    let mut came_from: HashMap<Cell, Cell> = HashMap::with_capacity(128);
    let mut open = BinaryHeap::new();
    let mut closed: HashSet<Cell> = HashSet::with_capacity(128);
    let mut neighbor_buf: Vec<(usize, usize, f64)> = Vec::with_capacity(8);

    g_score.insert(start, 0.0);
    open.push(AStarEntry { cell: start, f: heuristic(start, goal) });

    while let Some(AStarEntry { cell, .. }) = open.pop() {
        if cell == goal {
            return Ok(reconstruct_path(&came_from, goal, &g_score));
        }

        // Skip already-expanded nodes (avoids re-expansion from stale heap entries).
        if !closed.insert(cell) {
            continue;
        }

        let current_g = g_score[&cell];

        neighbors_into(grid, cell, &mut neighbor_buf);
        for &(nx, ny, step_cost) in &neighbor_buf {
            let neighbor = (nx, ny);
            if closed.contains(&neighbor) {
                continue;
            }
            let tentative_g = current_g + step_cost;
            if tentative_g < *g_score.get(&neighbor).unwrap_or(&f64::INFINITY) {
                g_score.insert(neighbor, tentative_g);
                came_from.insert(neighbor, cell);
                open.push(AStarEntry {
                    cell: neighbor,
                    f: tentative_g + heuristic(neighbor, goal),
                });
            }
        }
    }

    Err(PlanningError::NoPath)
}

#[inline]
fn cell_free(grid: &OccupancyGrid, (x, y): Cell) -> bool {
    grid.get(x, y).is_some_and(|v| v < OCCUPIED_THRESHOLD)
}

#[inline]
fn heuristic(a: Cell, b: Cell) -> f64 {
    let dx = (a.0 as f64 - b.0 as f64).abs();
    let dy = (a.1 as f64 - b.1 as f64).abs();
    // Octile distance.
    let (min, max) = if dx < dy { (dx, dy) } else { (dy, dx) };
    min * std::f64::consts::SQRT_2 + (max - min)
}

/// Write neighbours of `cell` into `out`, reusing the buffer to avoid
/// per-expansion heap allocation.
#[inline]
fn neighbors_into(grid: &OccupancyGrid, (cx, cy): Cell, out: &mut Vec<(usize, usize, f64)>) {
    out.clear();
    for dx in [-1_i64, 0, 1] {
        for dy in [-1_i64, 0, 1] {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = cx as i64 + dx;
            let ny = cy as i64 + dy;
            if nx < 0 || ny < 0 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if cell_free(grid, (nx, ny)) {
                let cost = if dx != 0 && dy != 0 {
                    std::f64::consts::SQRT_2
                } else {
                    1.0
                };
                out.push((nx, ny, cost));
            }
        }
    }
}

fn reconstruct_path(
    came_from: &HashMap<Cell, Cell>,
    goal: Cell,
    g_score: &HashMap<Cell, f64>,
) -> GridPath {
    let mut cells = vec![goal];
    let mut current = goal;
    while let Some(&prev) = came_from.get(&current) {
        cells.push(prev);
        current = prev;
    }
    cells.reverse();
    let cost = g_score.get(&goal).copied().unwrap_or(0.0);
    GridPath { cells, cost }
}

// ---------------------------------------------------------------------------
// Potential field
// ---------------------------------------------------------------------------

/// Output of the potential field planner: a 3-D velocity command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VelocityCommand {
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
}

/// Configuration for the potential field planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotentialFieldConfig {
    /// Attractive gain toward the goal.
    pub attractive_gain: f64,
    /// Repulsive gain away from obstacles.
    pub repulsive_gain: f64,
    /// Influence range for obstacles (metres).
    pub obstacle_influence: f64,
    /// Maximum output speed (m/s).
    pub max_speed: f64,
}

impl Default for PotentialFieldConfig {
    fn default() -> Self {
        Self {
            attractive_gain: 1.0,
            repulsive_gain: 100.0,
            obstacle_influence: 3.0,
            max_speed: 2.0,
        }
    }
}

/// Compute a velocity command using attractive + repulsive potential fields.
///
/// * `robot` — current robot position `[x, y, z]`.
/// * `goal` — target position `[x, y, z]`.
/// * `obstacles` — positions of nearby obstacles.
pub fn potential_field(
    robot: &[f64; 3],
    goal: &[f64; 3],
    obstacles: &[[f64; 3]],
    config: &PotentialFieldConfig,
) -> VelocityCommand {
    // Attractive force: linear pull toward goal.
    let mut fx = config.attractive_gain * (goal[0] - robot[0]);
    let mut fy = config.attractive_gain * (goal[1] - robot[1]);
    let mut fz = config.attractive_gain * (goal[2] - robot[2]);

    // Repulsive force: push away from each obstacle within influence range.
    for obs in obstacles {
        let dx = robot[0] - obs[0];
        let dy = robot[1] - obs[1];
        let dz = robot[2] - obs[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(0.01);

        if dist < config.obstacle_influence {
            let strength =
                config.repulsive_gain * (1.0 / dist - 1.0 / config.obstacle_influence) / (dist * dist);
            fx += strength * dx / dist;
            fy += strength * dy / dist;
            fz += strength * dz / dist;
        }
    }

    // Clamp to max speed.
    let speed = (fx * fx + fy * fy + fz * fz).sqrt();
    if speed > config.max_speed {
        let s = config.max_speed / speed;
        fx *= s;
        fy *= s;
        fz *= s;
    }

    VelocityCommand { vx: fx, vy: fy, vz: fz }
}

// ---------------------------------------------------------------------------
// Path smoothing
// ---------------------------------------------------------------------------

/// Convert a [`GridPath`] (grid cells) to world-space waypoints using the
/// grid resolution and origin.
pub fn path_to_waypoints(path: &GridPath, resolution: f64, origin: &[f64; 3]) -> Vec<[f64; 3]> {
    path.cells
        .iter()
        .map(|&(x, y)| {
            [
                origin[0] + x as f64 * resolution,
                origin[1] + y as f64 * resolution,
                origin[2],
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn free_grid(w: usize, h: usize) -> OccupancyGrid {
        OccupancyGrid::new(w, h, 1.0)
    }

    #[test]
    fn test_astar_straight_line() {
        let grid = free_grid(10, 10);
        let path = astar(&grid, (0, 0), (5, 0)).unwrap();
        assert_eq!(*path.cells.first().unwrap(), (0, 0));
        assert_eq!(*path.cells.last().unwrap(), (5, 0));
        assert!((path.cost - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_astar_diagonal() {
        let grid = free_grid(10, 10);
        let path = astar(&grid, (0, 0), (3, 3)).unwrap();
        assert_eq!(*path.cells.last().unwrap(), (3, 3));
        // Pure diagonal = 3 * sqrt(2) ≈ 4.24
        assert!((path.cost - 3.0 * std::f64::consts::SQRT_2).abs() < 1e-6);
    }

    #[test]
    fn test_astar_same_cell() {
        let grid = free_grid(5, 5);
        let path = astar(&grid, (2, 2), (2, 2)).unwrap();
        assert_eq!(path.cells.len(), 1);
        assert!((path.cost).abs() < 1e-9);
    }

    #[test]
    fn test_astar_around_wall() {
        let mut grid = free_grid(10, 10);
        // Vertical wall at x=3 from y=0 to y=4.
        for y in 0..5 {
            grid.set(3, y, 1.0);
        }
        let path = astar(&grid, (1, 2), (5, 2)).unwrap();
        assert_eq!(*path.cells.last().unwrap(), (5, 2));
        // Path must go around the wall, so cost > 4 (straight line).
        assert!(path.cost > 4.0);
    }

    #[test]
    fn test_astar_blocked() {
        let mut grid = free_grid(5, 5);
        // Full wall across the grid.
        for y in 0..5 {
            grid.set(2, y, 1.0);
        }
        let result = astar(&grid, (0, 2), (4, 2));
        assert!(result.is_err());
    }

    #[test]
    fn test_astar_invalid_start() {
        let grid = free_grid(5, 5);
        let result = astar(&grid, (10, 10), (2, 2));
        assert!(result.is_err());
    }

    #[test]
    fn test_potential_field_towards_goal() {
        let cmd = potential_field(
            &[0.0, 0.0, 0.0],
            &[5.0, 0.0, 0.0],
            &[],
            &PotentialFieldConfig::default(),
        );
        assert!(cmd.vx > 0.0);
        assert!(cmd.vy.abs() < 1e-9);
    }

    #[test]
    fn test_potential_field_obstacle_repulsion() {
        let cmd = potential_field(
            &[0.0, 0.0, 0.0],
            &[5.0, 0.0, 0.0],
            &[[1.0, 0.0, 0.0]],
            &PotentialFieldConfig::default(),
        );
        // Obstacle directly ahead — repulsion should reduce forward velocity.
        let cmd_no_obs = potential_field(
            &[0.0, 0.0, 0.0],
            &[5.0, 0.0, 0.0],
            &[],
            &PotentialFieldConfig::default(),
        );
        assert!(cmd.vx < cmd_no_obs.vx);
    }

    #[test]
    fn test_potential_field_max_speed() {
        let config = PotentialFieldConfig { max_speed: 1.0, ..Default::default() };
        let cmd = potential_field(
            &[0.0, 0.0, 0.0],
            &[100.0, 100.0, 0.0],
            &[],
            &config,
        );
        let speed = (cmd.vx * cmd.vx + cmd.vy * cmd.vy + cmd.vz * cmd.vz).sqrt();
        assert!((speed - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_path_to_waypoints() {
        let path = GridPath {
            cells: vec![(0, 0), (1, 0), (2, 0)],
            cost: 2.0,
        };
        let wps = path_to_waypoints(&path, 0.5, &[0.0, 0.0, 0.0]);
        assert_eq!(wps.len(), 3);
        assert!((wps[1][0] - 0.5).abs() < 1e-9);
        assert!((wps[2][0] - 1.0).abs() < 1e-9);
    }
}
