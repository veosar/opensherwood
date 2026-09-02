//! Integer 2D geometry for the motion area: point-in-polygon tests on map-pixel polygons.

use serde::{Deserialize, Serialize};

/// Walkable-ground description of a map (`docs/formats/rhp.md`, `STAT`): a boundary polygon and
/// obstacle polygons (tree trunks, rocks, walls). All coordinates are map pixels.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Geometry {
    /// Outline of the walkable ground (empty = everything is walkable).
    pub boundary: Vec<(i32, i32)>,
    /// Polygons that cannot be entered.
    pub obstacles: Vec<Vec<(i32, i32)>>,
}

impl Geometry {
    /// Whether a map pixel is walkable.
    #[must_use]
    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        if self.boundary.len() >= 3 && !point_in_polygon(x, y, &self.boundary) {
            return false;
        }
        !self
            .obstacles
            .iter()
            .any(|o| o.len() >= 3 && point_in_polygon(x, y, o))
    }

    /// Number of vertices over all polygons (for hashing / limits).
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.boundary.len() + self.obstacles.iter().map(Vec::len).sum::<usize>()
    }
}

/// Even-odd rule point-in-polygon in `i64` arithmetic; points on an edge count as inside.
#[must_use]
pub fn point_in_polygon(x: i32, y: i32, poly: &[(i32, i32)]) -> bool {
    let (px, py) = (i64::from(x), i64::from(y));
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (x1, y1) = (i64::from(poly[i].0), i64::from(poly[i].1));
        let (x2, y2) = (
            i64::from(poly[(i + 1) % n].0),
            i64::from(poly[(i + 1) % n].1),
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
        };
        assert!(g.is_walkable(10, 10));
        assert!(!g.is_walkable(50, 50));
        assert!(!g.is_walkable(150, 50));
        assert!(Geometry::default().is_walkable(-5, 7));
    }
}
