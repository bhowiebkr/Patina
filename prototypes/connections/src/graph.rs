//! A seeded test graph: 500 nodes and 1,000 connections, 200 of them buses (spec 19).

use serde::Serialize;

pub const NODE_COUNT: usize = 500;
pub const EDGE_COUNT: usize = 1000;
pub const BUS_COUNT: usize = 200;
pub const ROUTED_COUNT: usize = 100;
/// The distance between two lanes of a bus, in graph pixels.
pub const LANE_PITCH: f32 = 2.0;

const COLUMNS: usize = 25;
const NODE_WIDTH: f32 = 180.0;
const HEADER: f32 = 32.0;
const PADDING: f32 = 10.0;
const ROW: f32 = 24.0;
const COLUMN_STEP: f32 = 300.0;
const NODE_GAP: f32 = 48.0;
const BUS_WIDTHS: [u32; 5] = [8, 12, 16, 24, 32];
const TITLES: [&str; 10] = [
    "Clock", "Counter", "Decoder", "Mux", "Register", "Adder", "Latch", "Shifter", "Compare",
    "Filter",
];

#[derive(Serialize)]
pub struct Pin {
    /// The pin center, from the top of the node.
    pub y: f32,
    pub width: u32,
}

#[derive(Serialize)]
pub struct Node {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub title: String,
    pub family: u32,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub source: u32,
    pub source_pin: u32,
    pub target: u32,
    pub target_pin: u32,
    pub width: u32,
    /// The index of the first lane (the MSB) in the lane states.
    pub lane0: u32,
    pub route: Vec<[f32; 2]>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub lane_count: u32,
    pub lane_pitch: f32,
}

/// xorshift64*: enough for a repeatable layout and random lane states.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn row_height(width: u32) -> f32 {
    ROW.max(width as f32 * LANE_PITCH + 8.0)
}

/// Sets the pin centers and returns the height of the pin rows.
fn place_pins(pins: &mut [Pin]) -> f32 {
    let mut y = HEADER + PADDING;
    for pin in pins {
        let h = row_height(pin.width);
        pin.y = y + h / 2.0;
        y += h;
    }
    y - HEADER - PADDING
}

pub fn generate(seed: u64) -> Graph {
    let mut rng = Rng::new(seed);
    let rows = NODE_COUNT / COLUMNS;
    let mut nodes: Vec<Node> = (0..NODE_COUNT)
        .map(|i| {
            let mut outputs = vec![Pin { y: 0.0, width: 1 }];
            if rng.below(2) == 0 {
                let width = BUS_WIDTHS[rng.below(BUS_WIDTHS.len())];
                outputs.push(Pin { y: 0.0, width });
            }
            Node {
                x: 0.0,
                y: 0.0,
                w: NODE_WIDTH,
                h: 0.0,
                title: format!("{} {}", TITLES[i % TITLES.len()], i),
                family: rng.below(7) as u32,
                inputs: Vec::new(),
                outputs,
            }
        })
        .collect();

    let mut kinds: Vec<bool> = (0..EDGE_COUNT).map(|i| i < BUS_COUNT).collect();
    for i in (1..kinds.len()).rev() {
        kinds.swap(i, rng.below(i + 1));
    }
    let mut edges = Vec::with_capacity(EDGE_COUNT);
    let mut lane0 = 0;
    for bus in kinds {
        loop {
            let target = rows + rng.below(NODE_COUNT - rows);
            if nodes[target].inputs.len() >= 4 {
                continue;
            }
            let (col, row) = (target / rows, target % rows);
            let src_col = col - 1 - rng.below(col.min(3));
            let src_row = (row + rng.below(7)).saturating_sub(3).min(rows - 1);
            let source = src_col * rows + src_row;
            let Some(source_pin) = nodes[source].outputs.iter().position(|p| (p.width > 1) == bus)
            else {
                continue;
            };
            let width = nodes[source].outputs[source_pin].width;
            nodes[target].inputs.push(Pin { y: 0.0, width });
            edges.push(Edge {
                source: source as u32,
                source_pin: source_pin as u32,
                target: target as u32,
                target_pin: nodes[target].inputs.len() as u32 - 1,
                width,
                lane0,
                route: Vec::new(),
            });
            lane0 += width;
            break;
        }
    }

    for col in 0..COLUMNS {
        let mut y = 0.0;
        for row in 0..rows {
            let node = &mut nodes[col * rows + row];
            let h = place_pins(&mut node.inputs).max(place_pins(&mut node.outputs));
            node.h = HEADER + 2.0 * PADDING + h;
            node.x = col as f32 * COLUMN_STEP + rng.below(40) as f32;
            node.y = y;
            y += node.h + NODE_GAP;
        }
    }

    // Long connections get an orthogonal detour with four route points (15.9).
    let mut routed = 0;
    for edge in &mut edges {
        let (s, t) = (edge.source as usize, edge.target as usize);
        if routed == ROUTED_COUNT || t / rows - s / rows < 2 {
            continue;
        }
        let (sn, tn) = (&nodes[s], &nodes[t]);
        let (sx, sy) = (sn.x + sn.w, sn.y + sn.outputs[edge.source_pin as usize].y);
        let (tx, ty) = (tn.x, tn.y + tn.inputs[edge.target_pin as usize].y);
        let (x1, x2, yd) = (sx + 40.0, tx - 40.0, (sy + ty) / 2.0 + 30.0);
        edge.route = vec![[x1, sy], [x1, yd], [x2, yd], [x2, ty]];
        routed += 1;
    }

    Graph { nodes, edges, lane_count: lane0, lane_pitch: LANE_PITCH }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_has_the_sizes_of_the_spec() {
        let g = generate(7);
        assert_eq!(g.nodes.len(), NODE_COUNT);
        assert_eq!(g.edges.len(), EDGE_COUNT);
        assert_eq!(g.edges.iter().filter(|e| e.width > 1).count(), BUS_COUNT);
        assert!(g.edges.iter().all(|e| e.width == 1 || (8..=32).contains(&e.width)));
        assert_eq!(g.edges.iter().filter(|e| !e.route.is_empty()).count(), ROUTED_COUNT);
        let lanes: u32 = g.edges.iter().map(|e| e.width).sum();
        assert_eq!(lanes, g.lane_count);
        // Each input has one driver, and each connection goes to a later column.
        let rows = (NODE_COUNT / COLUMNS) as u32;
        assert!(g.edges.iter().all(|e| e.source / rows < e.target / rows));
    }
}
