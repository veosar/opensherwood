//! Integer 2D geometry for the motion area: point-in-polygon tests on map-pixel polygons.
//!
//! Every function here accepts any `i32` input without overflowing: products of coordinate
//! differences are formed in `i128`, so a hostile polygon cannot panic a debug build or wrap in a
//! release build (the two must never differ, ADR-0004). Snapshots and `set_geometry` additionally
//! bound vertices to [`MAX_COORD`], which keeps the navigation grid and every later consumer in a
//! documented range.

use serde::{Deserialize, Serialize};

/// Largest magnitude of a geometry vertex coordinate (map pixels). Maps are at most 2^15 pixels a
/// side (`world::MAX_MAP_SIZE`) and retail polygons are 16-bit, so 2^20 leaves room for polygons
/// that extend past the map edge while keeping every coordinate far inside `i32`.
pub const MAX_COORD: i32 = 1 << 20;

/// Walkable-ground description of a map (`docs/formats/rhp.md`, `STAT`): a boundary polygon and
/// obstacle polygons (tree trunks, rocks, walls). All coordinates are map pixels.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Geometry {
    /// Outline of the walkable ground (empty = everything is walkable).
    pub boundary: Vec<(i32, i32)>,
    /// Polygons that cannot be entered.
    pub obstacles: Vec<Vec<(i32, i32)>>,
    /// Further walkable polygons (the map's projection areas: yards, walls, floors of other layers).
    /// A point inside any of them is walkable even outside the boundary.
    #[serde(default)]
    pub areas: Vec<Vec<(i32, i32)>>,
}

impl Geometry {
    /// Whether a map pixel is walkable.
    #[must_use]
    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        let bounded = self.boundary.len() >= 3 || !self.areas.is_empty();
        let inside = !bounded
            || (self.boundary.len() >= 3 && point_in_polygon(x, y, &self.boundary))
            || self
                .areas
                .iter()
                .any(|a| a.len() >= 3 && point_in_polygon(x, y, a));
        if !inside {
            return false;
        }
        !self
            .obstacles
            .iter()
            .any(|o| o.len() >= 3 && point_in_polygon(x, y, o))
    }

    /// [`Geometry::is_walkable`] charged to a work budget: one unit per edge of every polygon
    /// tested, charged before the polygon is tested (the boundary, then the areas until one
    /// contains the point, then every obstacle). `None` when the budget ran out before the
    /// answer was known (nothing was decided; the caller retries with a fresh budget).
    #[must_use]
    pub fn is_walkable_within(&self, x: i32, y: i32, budget: &mut u64) -> Option<bool> {
        let mut test = |poly: &[(i32, i32)]| -> Option<bool> {
            let edges = poly.len() as u64;
            if *budget < edges {
                *budget = 0;
                return None;
            }
            *budget -= edges;
            Some(poly.len() >= 3 && point_in_polygon(x, y, poly))
        };
        let bounded = self.boundary.len() >= 3 || !self.areas.is_empty();
        let mut inside = !bounded;
        if !inside && self.boundary.len() >= 3 {
            inside = test(&self.boundary)?;
        }
        if !inside {
            for a in &self.areas {
                if test(a)? {
                    inside = true;
                    break;
                }
            }
        }
        if !inside {
            return Some(false);
        }
        for o in &self.obstacles {
            if test(o)? {
                return Some(false);
            }
        }
        Some(true)
    }

    /// Number of vertices over all polygons (for hashing / limits).
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.boundary.len()
            + self.obstacles.iter().map(Vec::len).sum::<usize>()
            + self.areas.iter().map(Vec::len).sum::<usize>()
    }

    /// Reject any vertex outside `-MAX_COORD..=MAX_COORD` on either axis.
    pub fn check_bounds(&self) -> Result<(), String> {
        let in_range = |&(x, y): &(i32, i32)| {
            x.unsigned_abs() <= MAX_COORD as u32 && y.unsigned_abs() <= MAX_COORD as u32
        };
        if let Some(v) = self.boundary.iter().find(|v| !in_range(v)) {
            return Err(format!("boundary vertex {v:?} outside +-{MAX_COORD}"));
        }
        for (i, o) in self.obstacles.iter().enumerate() {
            if let Some(v) = o.iter().find(|v| !in_range(v)) {
                return Err(format!("obstacle {i} vertex {v:?} outside +-{MAX_COORD}"));
            }
        }
        for (i, a) in self.areas.iter().enumerate() {
            if let Some(v) = a.iter().find(|v| !in_range(v)) {
                return Err(format!("area {i} vertex {v:?} outside +-{MAX_COORD}"));
            }
        }
        Ok(())
    }
}

/// Even-odd rule point-in-polygon; points on an edge count as inside. Differences of `i32`
/// coordinates fit `i64`, their products need `i128`: the function never overflows.
#[must_use]
pub fn point_in_polygon(x: i32, y: i32, poly: &[(i32, i32)]) -> bool {
    let (px, py) = (i128::from(x), i128::from(y));
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (x1, y1) = (i128::from(poly[i].0), i128::from(poly[i].1));
        let (x2, y2) = (
            i128::from(poly[(i + 1) % n].0),
            i128::from(poly[(i + 1) % n].1),
        );
        // On the segment?
        let cross = (px - x1) * (y2 - y1) - (py - y1) * (x2 - x1);
        if cross == 0
            && px >= x1.min(x2)
            && px <= x1.max(x2)
            && py >= y1.min(y2)
            && py <= y1.max(y2)
        {
            return true;
        }
        if (y1 > py) != (y2 > py) {
            // x coordinate of the edge at py, compared without division.
            let lhs = (px - x1) * (y2 - y1);
            let rhs = (x2 - x1) * (py - y1);
            let crosses = if y2 > y1 { lhs < rhs } else { lhs > rhs };
            if crosses {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_and_concave() {
        let sq = vec![(0, 0), (10, 0), (10, 10), (0, 10)];
        assert!(point_in_polygon(5, 5, &sq));
        assert!(point_in_polygon(0, 5, &sq));
        assert!(!point_in_polygon(11, 5, &sq));
        assert!(!point_in_polygon(-1, -1, &sq));
        let u = vec![
            (0, 0),
            (30, 0),
            (30, 30),
            (20, 30),
            (20, 10),
            (10, 10),
            (10, 30),
            (0, 30),
        ];
        assert!(point_in_polygon(5, 20, &u));
        assert!(!point_in_polygon(15, 20, &u));
        assert!(point_in_polygon(25, 20, &u));
    }

    #[test]
    fn geometry_rules() {
        let g = Geometry {
            boundary: vec![(0, 0), (100, 0), (100, 100), (0, 100)],
            obstacles: vec![vec![(40, 40), (60, 40), (60, 60), (40, 60)]],
            areas: Vec::new(),
        };
        assert!(g.is_walkable(10, 10));
        assert!(!g.is_walkable(50, 50));
        assert!(!g.is_walkable(150, 50));
        assert!(Geometry::default().is_walkable(-5, 7));
        // The charged form agrees with the plain one and charges one unit per edge tested:
        // the boundary (4) and the obstacle (4) for a walkable point, the boundary alone for
        // a point outside it; a budget short of the next polygon decides nothing.
        let mut b = 100;
        assert_eq!(g.is_walkable_within(10, 10, &mut b), Some(true));
        assert_eq!(b, 92);
        let mut b = 100;
        assert_eq!(g.is_walkable_within(150, 50, &mut b), Some(false));
        assert_eq!(b, 96);
        let mut b = 7;
        assert_eq!(g.is_walkable_within(10, 10, &mut b), None);
        assert_eq!(b, 0);
        let mut b = 0;
        assert_eq!(
            Geometry::default().is_walkable_within(-5, 7, &mut b),
            Some(true)
        );
    }

    #[test]
    fn extreme_coordinates_never_overflow_and_bounds_are_enforced() {
        let ext = [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX];
        let mut polys = vec![vec![
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MIN),
            (0, i32::MAX),
        ]];
        polys.push(vec![
            (i32::MAX, i32::MAX),
            (i32::MIN, i32::MAX),
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MIN),
        ]);
        polys.push(vec![(5, 5), (i32::MAX, 6), (i32::MIN, i32::MAX)]);
        for poly in &polys {
            for &x in &ext {
                for &y in &ext {
                    // Any answer is fine; the point is that neither build mode can misbehave.
                    let _ = point_in_polygon(x, y, poly);
                }
            }
        }
        // The full-range rectangle contains the origin and its own corners (edges count).
        assert!(point_in_polygon(0, 0, &polys[1]));
        assert!(point_in_polygon(i32::MIN, i32::MIN, &polys[1]));
        let g = Geometry {
            boundary: vec![
                (-MAX_COORD, -MAX_COORD),
                (MAX_COORD, -MAX_COORD),
                (MAX_COORD, MAX_COORD),
            ],
            obstacles: vec![vec![(0, 0), (1, 0), (0, 1)]],
            areas: Vec::new(),
        };
        g.check_bounds().unwrap();
        let mut bad = g.clone();
        bad.obstacles[0][1].0 = MAX_COORD + 1;
        assert!(bad.check_bounds().unwrap_err().contains("obstacle 0"));
        let mut bad = g;
        bad.boundary[0].1 = i32::MIN;
        assert!(bad.check_bounds().unwrap_err().contains("boundary"));
    }
}
