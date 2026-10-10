//! Original synthetic navigation fixtures, shared by core acceptance tests and RPC scenarios.

use super::map::{DoorSpec, EdgeSpec, FarmSpec, NavContent, NodeSpec, RecordSpec, SectorSpec};

/// Fixture F1 of `spec-navigation.md` 7.2: one layer, one sector `(0,0) (400,0) (400,200)
/// (0,200)` with one obstacle `(180,80) (220,80) (220,120) (180,120)`, size class `(6, 4)`,
/// the four outline corners and the four obstacle corners as graph nodes.
pub(crate) fn obstacle_course() -> NavContent {
    let outline = vec![(0, 0), (400, 0), (400, 200), (0, 200)];
    let obstacle = vec![(180, 80), (220, 80), (220, 120), (180, 120)];
    let node = |corner: (i32, i32), bits: u8, edges: Vec<u32>| NodeSpec {
        offset_bits: vec![bits],
        corner,
        mask: 0,
        entering_edges: edges,
    };
    // Region 0: the outline's corners (no edges). Region 1: the obstacle's four nodes.
    let outline_nodes = vec![
        node((0, 0), 4, vec![]),
        node((400, 0), 8, vec![]),
        node((400, 200), 1, vec![]),
        node((0, 200), 2, vec![]),
    ];
    // Edges, in the order N1-N2, N2-N1, N2-N3, N3-N2, N3-N4, N4-N3, N4-N1, N1-N4.
    let pairs: [(u32, u32, &[u8], &[u8]); 4] = [
        (0, 1, &[1, 2], &[1, 2]),
        (1, 2, &[2, 4], &[2, 4]),
        (2, 3, &[4, 8], &[4, 8]),
        (3, 0, &[8, 1], &[8, 1]),
    ];
    let corners = [(180, 80), (220, 80), (220, 120), (180, 120)];
    let bits = [11u8, 7, 14, 13];
    let mut edges = Vec::new();
    let mut records = Vec::new();
    let mut entering: Vec<Vec<u32>> = vec![Vec::new(); 4];
    for (a, b, ba, bb) in pairs {
        for (from, to, fb, tb) in [(a, b, ba, bb), (b, a, bb, ba)] {
            let i = edges.len() as u32;
            let (ca, cb) = (corners[from as usize], corners[to as usize]);
            let cost = super::search::length((ca.0 - cb.0) as f32, (ca.1 - cb.1) as f32);
            edges.push(EdgeSpec {
                a: (0, 0, 1, from),
                b: (0, 0, 1, to),
                cost,
                mask: 0,
                records: vec![i],
            });
            records.push(RecordSpec {
                passable: true,
                bits_b: tb.to_vec(),
                bits_a: fb.to_vec(),
            });
            entering[to as usize].push(i);
        }
    }
    let obstacle_nodes: Vec<NodeSpec> = (0..4)
        .map(|i| node(corners[i], bits[i], entering[i].clone()))
        .collect();
    NavContent {
        map_size: (400, 200),
        layers: vec![vec![SectorSpec {
            kind: 0,
            outline,
            barriers: Vec::new(),
            unknown_flag: 0,
            obstacles: vec![(0, obstacle)],
        }]],
        size_classes: vec![(6.0, 4.0)],
        regions: vec![vec![vec![outline_nodes, obstacle_nodes]]],
        edges,
        records,
        ..NavContent::default()
    }
}

/// Two floors joined by a passage; the lower floor is acceptance fixture F1.
pub(crate) fn scenario() -> NavContent {
    let mut c = obstacle_course();
    c.map_size = (640, 480);
    c.layers.push(vec![SectorSpec {
        outline: vec![(420, 20), (620, 20), (620, 180), (420, 180)],
        ..SectorSpec::default()
    }]);
    c.regions.push(vec![vec![vec![]]]);
    c.farm.push(FarmSpec {
        kind: 0,
        doors: vec![DoorSpec {
            door_type: 0,
            open: true,
            a: (360, 100),
            sector_a: 0,
            layer_a: 0,
            threshold: (410, 100),
            b: (450, 100),
            sector_b: 2,
            layer_b: 1,
            ..DoorSpec::default()
        }],
    });
    c
}
