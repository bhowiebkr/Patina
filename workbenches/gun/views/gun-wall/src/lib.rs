//! The gun wall of the gun workbench: the side views of the guns of eight seeds, next to the
//! graph. A click on a gun gives its seed to the `gun` node.

use std::fmt::Write;

use geometry::Seed;
use guns::{Class, Element, Maker, Outline, Pick, Rarity, generate, side_view};
use patina_sdk::{Canvas, Graph, Param, Pointer, Ui};

/// The guns on one page.
const PAGE: u64 = 8;
/// The longest path that the editor draws (4.3).
const MAX_PATH: usize = 4096;
/// The height of the label under each gun, in pixels.
const LABEL: f32 = 20.0;
// The colors of the GUI guide.
const FIELD: [u8; 3] = [0x19, 0x19, 0x19];
const BORDER: [u8; 3] = [0x45, 0x45, 0x45];
const ACCENT: [u8; 3] = [0xdf, 0xa6, 0x4b];
const MUTED: [u8; 3] = [0xb2, 0xb2, 0xad];
const INK: [u8; 3] = [0x17, 0x15, 0x11];

/// Shows the side views of the guns of eight seeds, with the class, the maker, the rarity and
/// the element of the `gun` node. Click a gun to give its seed to the `gun` node. Click Previous or Next for other seeds.
#[patina_sdk::view]
pub struct GunWall {
    /// The ID of the `gun` node.
    gun: String,
    /// The first seed of the page. The first call sets it from the seed of the `gun` node.
    first: Option<u64>,
    tiles: Vec<Tile>,
}

/// The seed, the class, the maker, the rarity and the element of a gun.
type Key = (u64, Pick<Class>, Pick<Maker>, Pick<Rarity>, Pick<Element>);

/// One gun of the wall.
#[derive(Default)]
struct Tile {
    /// The gun of the tile, or `None` before the first gun.
    key: Option<Key>,
    label: String,
    outlines: Vec<Outline>,
    /// The paths in pixels for the place `place`, with the color of each.
    paths: Vec<([u8; 3], String)>,
    place: [f32; 4],
}

impl GunWall {
    pub fn init() -> Self {
        GunWall { gun: String::new(), first: None, tiles: (0..PAGE).map(|_| Tile::default()).collect() }
    }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        ui.label("GUN / WALL");
        let Some(id) = graph.nodes().find(|id| graph.folder(id) == Some("nodes/gun")) else {
            ui.label("Add a gun node. The wall shows its guns for eight seeds.");
            return;
        };
        if self.gun != id {
            self.gun.clear();
            self.gun.push_str(id);
        }
        let inputs = read::<Seed>(graph, id, "seed").and_then(|seed| {
            let picks = (read(graph, id, "class")?, read(graph, id, "maker")?, read(graph, id, "rarity")?, read(graph, id, "element")?);
            Ok((seed, picks))
        });
        let (seed, (class, maker, rarity, element)) = match inputs {
            Ok(inputs) => inputs,
            Err(error) => return ui.label(&error),
        };
        let first = self.first.get_or_insert(seed.0 / PAGE * PAGE);
        ui.columns(110.0, |ui| {
            ui.column(|ui| {
                if ui.button("Previous").clicked() {
                    *first = first.saturating_sub(PAGE);
                }
            });
            ui.column(|ui| {
                if ui.button("Next").clicked() {
                    *first = first.saturating_add(PAGE).min(u64::MAX - PAGE);
                }
            });
        });
        let first = *first;
        ui.label(&format!("Seeds {first} to {}. Click a gun to use its seed.", first + PAGE - 1));

        // One new gun at each call, because a gun takes some milliseconds.
        let wanted = |i: usize| Some((first + i as u64, class, maker, rarity, element));
        if let Some((i, tile)) = self.tiles.iter_mut().enumerate().find(|(i, tile)| tile.key != wanted(*i)) {
            *tile = Tile::new((first + i as u64, class, maker, rarity, element));
        }

        let width = ui.available_size().map_or(480.0, |size| size[0]).max(160.0);
        let columns = [8, 4, 2].into_iter().find(|&c| width / c as f32 >= 180.0).unwrap_or(1);
        let size = [width / columns as f32, (width / columns as f32 * 0.55).max(90.0)];
        let mut canvas = ui.canvas("Gun wall", [width, size[1] * (PAGE as usize / columns) as f32]);
        for (i, tile) in self.tiles.iter_mut().enumerate() {
            let at = [(i % columns) as f32 * size[0], (i / columns) as f32 * size[1]];
            let ready = tile.key == wanted(i);
            tile.draw(&mut canvas, [at[0], at[1], size[0], size[1]], ready, ready && first + i as u64 == seed.0);
        }
        for pointer in canvas.pointer() {
            if let Pointer::Up([x, y]) = *pointer {
                let index = (y / size[1]) as usize * columns + ((x / size[0]) as usize).min(columns - 1);
                if index < PAGE as usize {
                    graph.set_param(&self.gun, "seed", first + index as u64);
                }
            }
        }
    }
}

impl Tile {
    fn new(key: Key) -> Tile {
        let (seed, class, maker, rarity, element) = key;
        let (label, outlines) = match generate(Seed(seed), class, maker, rarity, element) {
            Ok(gun) => (format!("{seed}  {}, {} {}", gun.name, gun.maker.title(), gun.class.title()), side_view(&gun)),
            Err(error) => (format!("{seed}  {error}"), Vec::new()),
        };
        Tile { key: Some(key), label, outlines, ..Tile::default() }
    }

    /// Draws the tile at `place`: x, y, width and height. `current` marks the seed of the `gun` node.
    fn draw(&mut self, canvas: &mut Canvas, place: [f32; 4], ready: bool, current: bool) {
        let ([x, y], [w, h]) = ([place[0] + 2.0, place[1] + 2.0], [place[2] - 4.0, place[3] - 4.0]);
        canvas.rect([x, y], [w, h], FIELD, true);
        canvas.rect([x, y], [w, h], if current { ACCENT } else { BORDER }, false);
        let text = [x + 8.0, y + h - 7.0];
        if !ready {
            return canvas.text(text, MUTED, "Making the gun...");
        }
        if self.place != place {
            self.paths = paths(&self.outlines, [x, y, w, h - LABEL]);
            self.place = place;
        }
        for (color, data) in &self.paths {
            canvas.path(data, *color, true);
            canvas.path(data, INK, false);
        }
        canvas.text(text, if current { ACCENT } else { MUTED }, &self.label);
    }
}

/// The parameter `key` of node `id`, or its default.
fn read<T: Param>(graph: &Graph, id: &str, key: &str) -> Result<T, String> {
    graph.param(id, key).map_or_else(|| Ok(T::default_value()), T::from_text)
}

/// The outlines as one SVG path for each color, fit into the box: x, y, width and height. +Z is up.
fn paths(outlines: &[Outline], [x, y, w, h]: [f32; 4]) -> Vec<([u8; 3], String)> {
    let (mut low, mut high) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in outlines.iter().flat_map(|o| o.outlines.iter().flatten()) {
        for k in 0..2 {
            (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k]));
        }
    }
    if !(low[0] < high[0] && low[1] < high[1]) {
        return Vec::new();
    }
    let pad = 8.0;
    let scale = ((f64::from(w) - 2.0 * pad) / (high[0] - low[0])).min((f64::from(h) - 2.0 * pad) / (high[1] - low[1]));
    let center = [f64::from(x + w / 2.0), f64::from(y + h / 2.0)];
    let mid = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0];
    let pixel = |p: &[f64; 2]| [center[0] + (p[0] - mid[0]) * scale, center[1] - (p[1] - mid[1]) * scale];
    let mut kept = Vec::new();
    outlines
        .iter()
        .filter_map(|o| {
            let color = [16, 8, 0].map(|shift| (o.color >> shift) as u8);
            // A coarser step for a path that is too long for the editor.
            let data = [0.75, 1.5, 3.0, 6.0, 12.0].into_iter().map(|step| path(&o.outlines, &pixel, step, &mut kept)).find(|d| d.len() <= MAX_PATH)?;
            (!data.is_empty()).then_some((color, data))
        })
        .collect()
}

/// An SVG path of the outlines in pixels, with no point nearer than `step` pixels to the point
/// before it. `kept` is a buffer.
fn path(outlines: &[Vec<[f64; 2]>], pixel: &impl Fn(&[f64; 2]) -> [f64; 2], step: f64, kept: &mut Vec<[f64; 2]>) -> String {
    let mut data = String::new();
    for outline in outlines {
        kept.clear();
        for p in outline.iter().map(pixel) {
            if kept.last().is_none_or(|q| (p[0] - q[0]).hypot(p[1] - q[1]) >= step) {
                kept.push(p);
            }
        }
        if kept.len() < 3 {
            continue;
        }
        for (i, p) in kept.iter().enumerate() {
            let _ = write!(data, "{} {:.1} {:.1} ", if i == 0 { "M" } else { "L" }, p[0], p[1]);
        }
        data.push_str("Z ");
    }
    data.truncate(data.trim_end().len());
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUN: &str = "9a70000000000001";

    fn widget(tree: &str, prefix: &str) -> usize {
        tree.lines().position(|line| line.starts_with(prefix)).unwrap_or_else(|| panic!("no {prefix} in {tree}"))
    }

    #[test]
    fn the_wall_shows_eight_guns_and_a_click_sets_the_seed() {
        let mut graph = Graph::new(&format!("node\t{GUN}\tnodes/gun\nparam\t{GUN}\tseed\t11\nparam\t{GUN}\tmaker\tvesper\n"));
        let mut wall = GunWall::init();
        let mut tree = String::new();
        for _ in 0..PAGE {
            let mut ui = Ui::new(2, "");
            wall.ui(&mut ui, &mut graph);
            tree = ui.tree().to_owned();
        }
        // The page starts at a multiple of eight, and each gun has the maker of the node.
        for (i, tile) in wall.tiles.iter().enumerate() {
            assert_eq!(tile.key, Some((8 + i as u64, Pick::Random, Pick::Of(Maker::Vesper), Pick::Random, Pick::Random)));
            assert!(tile.label.contains("Vesper") && !tile.paths.is_empty(), "{}", tile.label);
        }
        for (_, data) in wall.tiles.iter().flat_map(|t| &t.paths) {
            assert!(data.len() <= MAX_PATH && data.chars().all(|c| "MLZ .-0123456789".contains(c)), "{data}");
        }
        let canvas = tree.lines().nth(widget(&tree, "canvas\tGun wall")).unwrap();
        assert!(canvas.split('\t').filter(|s| s.starts_with("path")).count() >= 2 * PAGE as usize);
        assert!(graph.edits().next().is_none());
        // With no size, the wall is 480 pixels wide: two columns of 240 x 132. Seed 10 is the third gun.
        wall.ui(&mut Ui::new(2, &format!("{} up 10 140", widget(&tree, "canvas\tGun wall"))), &mut graph);
        let edits: Vec<_> = graph.edits().collect();
        assert!(edits.len() == 1 && edits[0].ends_with(&format!("{GUN}\tseed\t10")), "{edits:?}");
        wall.ui(&mut Ui::new(2, &format!("{} 1", widget(&tree, "button\tNext"))), &mut graph);
        assert_eq!(wall.first, Some(16));
    }

    #[test]
    fn a_long_outline_gets_fewer_points() {
        let circle: Vec<[f64; 2]> = (0..4000).map(|i| f64::from(i) * std::f64::consts::TAU / 4000.0).map(|a| [a.cos(), a.sin()]).collect();
        let outline = Outline { color: 0x2f8f8a, outlines: vec![circle] };
        let paths = paths(&[outline], [0.0, 0.0, 400.0, 400.0]);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].0, [0x2f, 0x8f, 0x8a]);
        assert!(paths[0].1.len() <= MAX_PATH && paths[0].1.starts_with("M ") && paths[0].1.ends_with('Z'));
    }
}
