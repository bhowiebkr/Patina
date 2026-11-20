//! A measured height field. Missing stations remain holes in the mesh.
use crate::core::{
    results::Results,
    survey::{Heights, Run},
};

#[derive(Clone, Debug)]
pub struct Surface {
    pub xy: Vec<[f64; 2]>,
    pub heights: Vec<Option<[f64; 4]>>,
    pub cells: Vec<[usize; 4]>,
    pub bounds: [[f64; 2]; 2],
    pub leveled: bool,
}

impl Surface {
    pub fn new(run: &Run, heights: &Heights, result: Option<&Results>) -> Self {
        let xy: Vec<_> =
            run.stations.iter().map(|s| [s.x_mm, s.y_mm].map(|v| if v == 0.0 { 0.0 } else { v })).collect();
        let mut samples = vec![None; xy.len()];
        for p in &heights.stations {
            samples[p.station] = Some([f64::NAN, p.height_um, f64::NAN, f64::NAN]);
        }
        if let Some(result) = result {
            for p in &result.points {
                samples[p.station] = Some([p.flattened_um, p.measured_um, p.above_min_um, p.below_max_um]);
            }
        }
        let mut xs: Vec<_> = xy.iter().map(|p| p[0]).collect();
        let mut ys: Vec<_> = xy.iter().map(|p| p[1]).collect();
        for axis in [&mut xs, &mut ys] {
            axis.sort_by(f64::total_cmp);
            axis.dedup();
        }
        let bounds = [
            [xs.first().copied().unwrap_or(0.0), ys.first().copied().unwrap_or(0.0)],
            [xs.last().copied().unwrap_or(1.0), ys.last().copied().unwrap_or(1.0)],
        ];
        let mut cells = Vec::new();
        // Imported irregular plans can have many unique coordinates. Never allocate their cross product.
        if xs.len().saturating_mul(ys.len()) <= 4096 {
            let mut grid = vec![None; xs.len() * ys.len()];
            for (i, p) in xy.iter().enumerate() {
                let x = xs.binary_search_by(|v| v.total_cmp(&p[0])).unwrap();
                let y = ys.binary_search_by(|v| v.total_cmp(&p[1])).unwrap();
                grid[y * xs.len() + x] = Some(i);
            }
            for y in 0..ys.len().saturating_sub(1) {
                for x in 0..xs.len().saturating_sub(1) {
                    let i = y * xs.len() + x;
                    if let [Some(a), Some(b), Some(c), Some(d)] =
                        [grid[i], grid[i + 1], grid[i + 1 + xs.len()], grid[i + xs.len()]]
                    {
                        cells.push([a, b, c, d]);
                    }
                }
            }
        }
        Self { xy, heights: samples, cells, bounds, leveled: result.is_some() }
    }

    pub fn range(&self, channel: usize) -> [f64; 2] {
        let mut range = [f64::INFINITY, f64::NEG_INFINITY];
        for h in self.heights.iter().flatten().filter_map(|h| h.get(channel)).filter(|v| v.is_finite()) {
            range[0] = range[0].min(*h);
            range[1] = range[1].max(*h);
        }
        if range[0].is_finite() { range } else { [0.0, 0.0] }
    }

    pub fn measured(&self) -> usize {
        self.heights.iter().flatten().count()
    }
}

/// Position in mm, unit normal, and sRGB data color.
pub type Vertex = [f32; 9];
#[derive(Default)]
pub struct Mesh {
    pub faces: Vec<Vertex>,
    pub points: Vec<[f32; 3]>,
    pub bounds: [[f32; 3]; 2],
}
impl Mesh {
    pub fn build(&mut self, surface: &Surface, channel: usize, exaggeration: f32) {
        self.faces.clear();
        self.points.clear();
        let [lo, hi] = surface.range(channel);
        let position = |i: usize| -> Option<[f32; 3]> {
            let z = *surface.heights[i]?.get(channel)?;
            z.is_finite()
                .then(|| [surface.xy[i][0] as f32, surface.xy[i][1] as f32, (z / 1000.0) as f32 * exaggeration])
        };
        self.bounds = [
            [surface.bounds[0][0] as f32, surface.bounds[0][1] as f32, (lo / 1000.0) as f32 * exaggeration],
            [surface.bounds[1][0] as f32, surface.bounds[1][1] as f32, (hi / 1000.0) as f32 * exaggeration],
        ];
        self.points.extend((0..surface.xy.len()).filter_map(position));
        for &[a, b, c, d] in &surface.cells {
            let [Some(pa), Some(pb), Some(pc), Some(pd)] = [position(a), position(b), position(c), position(d)] else {
                continue;
            };
            for (ids, positions) in [([a, b, c], [pa, pb, pc]), ([a, c, d], [pa, pc, pd])] {
                let [p, q, r] = positions;
                let u = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
                let v = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
                let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                let length = n.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
                for (id, p) in ids.into_iter().zip(positions) {
                    let value = surface.heights[id].unwrap()[channel];
                    let color = color(((value - lo) / (hi - lo).max(0.001)) as f32);
                    self.faces.push([
                        p[0],
                        p[1],
                        p[2],
                        n[0] / length,
                        n[1] / length,
                        n[2] / length,
                        color[0] as f32 / 255.0,
                        color[1] as f32 / 255.0,
                        color[2] as f32 / 255.0,
                    ]);
                }
            }
        }
    }
}

pub fn color(t: f32) -> [u8; 3] {
    const STOPS: [[f32; 3]; 6] =
        [[13., 8., 135.], [106., 0., 168.], [177., 42., 144.], [225., 100., 98.], [252., 166., 54.], [240., 249., 33.]];
    let t = t.clamp(0.0, 1.0) * 5.0;
    let i = (t as usize).min(4);
    let f = t - i as f32;
    std::array::from_fn(|k| (STOPS[i][k] * (1.0 - f) + STOPS[i + 1][k] * f).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn holes_are_not_bridged_and_display_scale_preserves_values() {
        let mut s = Surface {
            xy: vec![[0., 0.], [1., 0.], [0., 1.], [1., 1.]],
            heights: vec![Some([0., 500., 0., 0.]); 4],
            cells: vec![[0, 1, 3, 2]],
            bounds: [[0., 0.], [1., 1.]],
            leveled: true,
        };
        let mut m = Mesh::default();
        m.build(&s, 1, 10.);
        assert_eq!(m.faces.len(), 6);
        assert_eq!(m.faces[0][2], 5.);
        assert_eq!(s.range(1), [500., 500.]);
        s.heights[3] = None;
        m.build(&s, 1, 10.);
        assert!(m.faces.is_empty());
        assert_eq!(m.points.len(), 3);
    }
}
