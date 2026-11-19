//! Node definitions. Every node is one WGSL file under `kernels/nodes/`; its `//!` header lines
//! declare the node:
//!
//! ```text
//! //! name: Levels
//! //! category: Adjustment
//! //! description: Remaps the input range (any line without a key continues this).
//! //! input: input Color "Input" default=(0,0,0,1) primary
//! //! input: material Material hidden   (fed only by graph flattening, never drawn)
//! //! output: output Color
//! //! param: levelinlow float(0,1) = 0 "In low"
//! //! param: intensity float(0,100) = 1 "Intensity" soft
//! //! param: mode enum(Copy|Add|Add Sub) = add_sub "Mode"
//! //! passes: log2+1        (a sum of N, log2 or int param terms, each optionally K*; default 1)
//! //! stats                 (an atomic u32 buffer for min/max/histograms)
//! //! atoms: 5              (that many atomic u32 per pixel, zeroed before the first pass)
//! //! paint                 (a hand-painted R32Float mask, read through `scratch_in`)
//! //! domain: mesh          (mesh | texture | inherit, the default)
//! //! defaults: 2           (revision that adopts this definition's updated defaults)
//! //! input-defaults: 5     (optional separate revision for updated input defaults)
//! ```

use thiserror::Error;

/// Most inputs a node may declare (bindings `input0..input15`).
pub const MAX_INPUTS: usize = 16;
/// Most passes a multipass node may run.
pub const MAX_PASSES: u32 = 256;
/// Most stops a gradient holds (some materials use all 256).
pub const GRADIENT_STOPS: usize = 256;
pub const CURVE_POINTS: usize = 16;
/// Largest Curve control table currently supported by measured source fixtures.
pub const CURVE_CONTROL_POINTS: usize = 256;
mod gradient;
mod passes;
use passes::parse as parse_passes;

#[derive(Debug, Error)]
#[error("{file}:{line}: {message}")]
pub struct DefError {
    pub file: String,
    pub line: usize,
    pub message: String,
}

/// What travels along a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Socket {
    Gray,
    Color,
    /// Base colour, roughness, metallic, height, AO and normal.
    Material,
}

impl Socket {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "gray" | "grayscale" => Some(Self::Gray),
            "color" => Some(Self::Color),
            "material" => Some(Self::Material),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Gray => "Grayscale",
            Self::Color => "Color",
            Self::Material => "Material",
        }
    }

    /// Texture layers the socket's data takes.
    pub fn layers(self) -> u32 {
        match self {
            Self::Gray | Self::Color => 1,
            Self::Material => 3,
        }
    }

    /// Gray and colour convert into each other; a material links only to a material.
    pub fn accepts(self, from: Socket) -> bool {
        (self == Self::Material) == (from == Self::Material)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParamType {
    Float {
        min: f32,
        max: f32,
    },
    Int {
        min: i32,
        max: i32,
    },
    Bool,
    Enum(Vec<String>),
    /// `float2`, `float3`, `float4`.
    Vector {
        n: usize,
        min: f32,
        max: f32,
    },
    /// sRGB RGBA.
    Color,
    /// Stop count, then position, r, g, b, a per stop (sorted by position).
    Gradient,
    /// Stop count, then position, r, g, b, a and midpoint bias per stop.
    GradientControls,
    /// Legacy fixed16 layout: count, then x, y, left/right handle x/y per point.
    Curve,
    /// The same point format with an explicit capacity; does not change private legacy layouts.
    CurvePoints {
        capacity: usize,
    },
    /// A string, kept in the node's text map; it takes no value slots.
    Text(TextKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    /// `text`: one line.
    Line,
    /// `lines`: several lines.
    Lines,
    /// `file`: a file path, absolute or relative to the project.
    File,
    /// `code`: WGSL appended to the kernel, so the node compiles its own pipeline.
    Code,
}

/// An image the app rasterises on the CPU and binds at binding 6 (`host_*` in the kernel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    /// The image file named by the `path` param.
    Bitmap,
    /// The `text` param in the `font` param's font, `fontsize` points on a 512-point canvas,
    /// lines aligned by `alignment` (left, center, right): a coverage mask.
    Text,
    /// The SVG file named by the `path` param, premultiplied.
    Svg,
}

impl Host {
    /// The params the host reads, with the types they need.
    fn needs(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Bitmap | Self::Svg => &[("path", "file")],
            Self::Text => &[
                ("text", "text"),
                ("font", "file"),
                ("fontsize", "float"),
                ("alignment", "enum"),
            ],
        }
    }
}

impl ParamType {
    pub fn slots(&self) -> usize {
        match self {
            Self::Float { .. } | Self::Int { .. } | Self::Bool | Self::Enum(_) => 1,
            Self::Vector { n, .. } => *n,
            Self::Color => 4,
            Self::Gradient => 1 + GRADIENT_STOPS * 5,
            Self::GradientControls => 1 + GRADIENT_STOPS * 6,
            Self::Curve => 1 + CURVE_POINTS * 6,
            Self::CurvePoints { capacity } => 1 + capacity * 6,
            Self::Text(_) => 0,
        }
    }

    pub fn curve_capacity(&self) -> Option<usize> {
        match self {
            Self::Curve => Some(CURVE_POINTS),
            Self::CurvePoints { capacity } => Some(*capacity),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Param {
    pub id: String,
    pub label: String,
    pub ty: ParamType,
    pub default: Vec<f32>,
    /// A text param's default.
    pub text: String,
    /// First slot in the node's value buffer.
    pub offset: usize,
    /// Float slider bounds guide dragging; numeric entry may exceed them.
    pub soft_range: bool,
    /// Missing values in pre-versioned nodes retain this definition's previous default.
    pub legacy_default: Option<Vec<f32>>,
}

impl Param {
    /// The value buffer slots for `stored` (a saved value), falling back to the default.
    pub fn pack(&self, stored: Option<&[f32]>) -> Vec<f32> {
        let slots = self.ty.slots();
        match (&self.ty, stored) {
            (ParamType::GradientControls, Some(v)) if !v.is_empty() => gradient::pack(v),
            (ParamType::Gradient | ParamType::Curve | ParamType::CurvePoints { .. }, Some(v))
                if !v.is_empty() =>
            {
                let mut v = v.to_vec();
                if self.ty == ParamType::Gradient {
                    sort_stops(&mut v);
                }
                v.resize(slots, 0.0);
                v
            }
            (_, Some(v)) if v.len() == slots => v.to_vec(),
            _ => {
                let mut v = self.default.clone();
                v.resize(slots, 0.0);
                v
            }
        }
    }
}

/// Sorts a packed gradient's stops by position.
pub fn sort_stops(v: &mut [f32]) {
    let count = (v.first().copied().unwrap_or(0.0) as usize).min((v.len().max(1) - 1) / 5);
    let mut stops: Vec<[f32; 5]> = (0..count)
        .map(|i| {
            let s = &v[1 + i * 5..6 + i * 5];
            [s[0], s[1], s[2], s[3], s[4]]
        })
        .collect();
    stops.sort_by(|a, b| a[0].total_cmp(&b[0]));
    for (i, s) in stops.iter().enumerate() {
        v[1 + i * 5..6 + i * 5].copy_from_slice(s);
    }
}

#[derive(Clone, Debug)]
pub struct Input {
    pub id: String,
    pub label: String,
    pub socket: Socket,
    /// Read when nothing is linked (gray inputs use `.r`).
    pub default: [f32; 4],
    /// Fallback for pre-versioned nodes, when this definition changed its input default.
    pub legacy_default: Option<[f32; 4]>,
    /// The input a pass-through or size inherits from.
    pub primary: bool,
    /// Linked only when a host flattens the graph (a material instance's referenced
    /// material); the editor shows no socket for it.
    pub hidden: bool,
    /// Extra image sockets may collapse while unused; their indices and bindings stay fixed.
    pub optional: bool,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub id: String,
    pub label: String,
    pub socket: Socket,
    /// First layer in the node's output array texture.
    pub layer: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PassBase {
    Fixed(u32),
    /// log2 of the texture side.
    Log2,
    /// An int or enum parameter's value.
    Param(String),
    /// Whether an integer/enum parameter equals this value (0 or 1).
    Equal(String, i32),
}

/// The pass count: a sum of terms, each a factor times a base.
#[derive(Clone, Debug, PartialEq)]
pub struct Passes {
    pub terms: Vec<(i32, PassBase)>,
}

impl Passes {
    fn single() -> Self {
        Self {
            terms: vec![(1, PassBase::Fixed(1))],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    /// Evaluated over the asset's UV atlas with its mesh maps.
    Mesh,
    /// A tileable square image.
    Texture,
    /// Mesh when any linked input is, else texture.
    Inherit,
}

#[derive(Clone, Debug)]
pub struct Def {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    /// Node defaults older than this revision use any declared legacy values.
    pub defaults_version: u32,
    pub input_defaults_version: Option<u32>,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub params: Vec<Param>,
    pub passes: Passes,
    pub stats: bool,
    /// Atomic u32s per pixel in `atoms` (binding 22), for union-find and per-shape sums.
    pub atoms: u32,
    /// Reads a painted mask (the Paint tab's brush target) at binding 6.
    pub paint: bool,
    pub host: Option<Host>,
    pub domain: Domain,
    /// Path under `kernels/`.
    pub path: String,
    /// The whole kernel file.
    pub source: String,
}

impl Def {
    pub fn input(&self, id: &str) -> Option<(usize, &Input)> {
        self.inputs.iter().enumerate().find(|(_, i)| i.id == id)
    }

    pub fn output(&self, id: &str) -> Option<&Output> {
        self.outputs.iter().find(|o| o.id == id)
    }

    pub fn param(&self, id: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.id == id)
    }

    /// The param holding the node's own WGSL (Pixel and Value Processor).
    pub fn code_param(&self) -> Option<&Param> {
        self.params
            .iter()
            .find(|p| p.ty == ParamType::Text(TextKind::Code))
    }

    /// Value buffer length.
    pub fn slots(&self) -> usize {
        self.params.iter().map(|p| p.ty.slots()).sum()
    }

    pub fn layers(&self) -> u32 {
        self.outputs.iter().map(|o| o.socket.layers()).sum()
    }

    /// The graph's terminal node: its outputs feed the viewport, not links.
    pub fn is_output(&self) -> bool {
        self.id == "output"
    }

    pub fn multipass(&self) -> bool {
        self.passes != Passes::single()
    }

    /// Passes to run at texture side `size` with packed `values`.
    pub fn pass_count(&self, size: u32, values: &[f32]) -> u32 {
        let term = |base: &PassBase| match base {
            PassBase::Fixed(n) => *n as i32,
            PassBase::Log2 => size.max(1).ilog2() as i32,
            PassBase::Param(id) => self
                .param(id)
                .and_then(|p| values.get(p.offset))
                .map_or(1, |v| *v as i32),
            PassBase::Equal(id, expected) => i32::from(
                self.param(id)
                    .and_then(|p| values.get(p.offset).or_else(|| p.default.first()))
                    .is_some_and(|v| v.round() as i32 == *expected),
            ),
        };
        let total: i32 = self.passes.terms.iter().map(|(k, b)| k * term(b)).sum();
        total.clamp(1, MAX_PASSES as i32) as u32
    }

    /// Parses a kernel file's header. `path` is relative to `kernels/`; the id is its stem.
    pub fn parse(path: &str, source: &str) -> Result<Self, DefError> {
        let id = path
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".wesl"))
            .unwrap_or(path)
            .to_owned();
        let folder = path.rsplit('/').nth(1).unwrap_or("other");
        let mut def = Def {
            name: title_case(&id),
            category: title_case(folder),
            id,
            description: String::new(),
            defaults_version: 1,
            input_defaults_version: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            params: Vec::new(),
            passes: Passes::single(),
            stats: false,
            atoms: 0,
            paint: false,
            host: None,
            domain: Domain::Inherit,
            path: path.to_owned(),
            source: source.to_owned(),
        };
        let mut ids: Vec<String> = Vec::new();
        for (n, line) in source.lines().enumerate() {
            let Some(text) = line.trim_start().strip_prefix("//!") else {
                continue;
            };
            let fail = |message: String| DefError {
                file: path.to_owned(),
                line: n + 1,
                message,
            };
            let text = text.trim();
            let (key, rest) = match text.split_once(char::is_whitespace) {
                Some((k, r)) => (k, r.trim()),
                None => (text, ""),
            };
            let Some(key) = key
                .strip_suffix(':')
                .or(matches!(key, "stats" | "paint").then_some(key))
            else {
                if !text.is_empty() {
                    push_line(&mut def.description, text);
                }
                continue;
            };
            let mut new_id = |id: &str| -> Result<String, DefError> {
                if !is_ident(id) {
                    return Err(fail(format!("`{id}` is not a valid id")));
                }
                if ids.iter().any(|i| i == id) {
                    return Err(fail(format!("`{id}` is declared twice")));
                }
                ids.push(id.to_owned());
                Ok(id.to_owned())
            };
            match key {
                "name" => def.name = rest.to_owned(),
                "category" => def.category = rest.to_owned(),
                "defaults" | "input-defaults" => {
                    let revision = rest.parse().ok().filter(|v| *v > 0).ok_or_else(|| {
                        fail("defaults revision must be a positive integer".into())
                    })?;
                    if key == "defaults" {
                        def.defaults_version = revision;
                    } else {
                        def.input_defaults_version = Some(revision);
                    }
                }
                "description" => push_line(&mut def.description, rest),
                "stats" => def.stats = true,
                "paint" => def.paint = true,
                // Measured closeness notes; ignored at runtime.
                "parity" => {}
                "domain" => {
                    def.domain = match rest {
                        "mesh" => Domain::Mesh,
                        "texture" => Domain::Texture,
                        "inherit" => Domain::Inherit,
                        _ => return Err(fail(format!("unknown domain `{rest}`"))),
                    }
                }
                "passes" => def.passes = parse_passes(rest).map_err(fail)?,
                "host" => {
                    def.host = Some(match rest {
                        "bitmap" => Host::Bitmap,
                        "text" => Host::Text,
                        "svg" => Host::Svg,
                        _ => return Err(fail(format!("unknown host `{rest}`"))),
                    })
                }
                "atoms" => match rest.parse() {
                    Ok(n @ 1..=8) => def.atoms = n,
                    _ => return Err(fail(format!("atoms must be 1 to 8, not `{rest}`"))),
                },
                "input" => {
                    let t = tokens(rest);
                    let (id, socket) = id_socket(&t).map_err(fail)?;
                    let id = new_id(id)?;
                    let mut input = Input {
                        label: title_case(&id),
                        id,
                        socket,
                        default: [0.0, 0.0, 0.0, 1.0],
                        legacy_default: None,
                        primary: false,
                        hidden: false,
                        optional: false,
                    };
                    let mut i = 2;
                    while i < t.len() {
                        match t[i].as_str() {
                            "primary" => input.primary = true,
                            "hidden" => input.hidden = true,
                            "optional" => input.optional = true,
                            "default" | "legacy" if t.get(i + 1).is_some_and(|s| s == "=") => {
                                let v = numbers(t.get(i + 2).map_or("", String::as_str))
                                    .map_err(fail)?;
                                let value = match v.as_slice() {
                                    [g] => [*g, *g, *g, 1.0],
                                    [r, g, b] => [*r, *g, *b, 1.0],
                                    [r, g, b, a] => [*r, *g, *b, *a],
                                    _ => {
                                        return Err(fail("default takes 1, 3 or 4 numbers".into()));
                                    }
                                };
                                if t[i] == "legacy" {
                                    input.legacy_default = Some(value);
                                } else {
                                    input.default = value;
                                }
                                i += 2;
                            }
                            s => match label(s) {
                                Some(l) => input.label = l,
                                None => return Err(fail(format!("unexpected `{s}`"))),
                            },
                        }
                        i += 1;
                    }
                    if input.socket == Socket::Material && input.legacy_default.is_some() {
                        return Err(fail(
                            "legacy input defaults require a gray or color socket".into(),
                        ));
                    }
                    def.inputs.push(input);
                    if def.inputs.len() > MAX_INPUTS {
                        return Err(fail(format!("more than {MAX_INPUTS} inputs")));
                    }
                }
                "output" => {
                    let t = tokens(rest);
                    let (id, socket) = id_socket(&t).map_err(fail)?;
                    let id = new_id(id)?;
                    let label = t.get(2).and_then(|s| label(s)).unwrap_or(title_case(&id));
                    let layer = def.layers();
                    def.outputs.push(Output {
                        id,
                        label,
                        socket,
                        layer,
                    });
                }
                "param" => {
                    let t = tokens(rest);
                    let (Some(id), Some(ty)) = (t.first(), t.get(1)) else {
                        return Err(fail("param needs an id and a type".into()));
                    };
                    let id = new_id(id)?;
                    let ty = parse_type(ty).map_err(fail)?;
                    let mut values: &[String] = &[];
                    let mut label_text = title_case(&id);
                    let mut text = String::new();
                    if matches!(ty, ParamType::Text(_)) && t.get(2).is_some_and(|s| s == "=") {
                        text = t
                            .get(3)
                            .and_then(|s| label(s))
                            .map(|s| unescape(&s))
                            .ok_or(fail("a text default is a quoted string".into()))?;
                        if let Some(l) = t.get(4).and_then(|s| label(s)) {
                            label_text = l;
                        }
                    } else if t.get(2).is_some_and(|s| s == "=") {
                        let end = t.iter().skip(3).position(|s| s.starts_with('"'));
                        let end = end.map_or(t.len(), |e| e + 3);
                        values = &t[3..end];
                        if let Some(l) = t.get(end).and_then(|s| label(s)) {
                            label_text = l;
                        }
                    } else if let Some(l) = t.get(2).and_then(|s| label(s)) {
                        label_text = l;
                    }
                    let default = parse_default(&ty, values).map_err(fail)?;
                    let label_at = t.iter().rposition(|s| label(s).is_some());
                    let modifiers = label_at.map_or(&[][..], |at| &t[at + 1..]);
                    let mut soft_range = false;
                    let mut legacy_default = None;
                    let mut modifier = 0;
                    while modifier < modifiers.len() {
                        match modifiers[modifier].as_str() {
                            "soft" => soft_range = true,
                            "legacy" => {
                                if modifiers.get(modifier + 1).is_none_or(|s| s != "=") {
                                    return Err(fail("legacy default needs = and a value".into()));
                                }
                                let value = modifiers
                                    .get(modifier + 2)
                                    .ok_or_else(|| fail("missing legacy default".into()))?;
                                if matches!(ty, ParamType::Text(_)) {
                                    return Err(fail(
                                        "legacy defaults require a numeric parameter".into(),
                                    ));
                                }
                                legacy_default = Some(
                                    parse_default(&ty, std::slice::from_ref(value))
                                        .map_err(fail)?,
                                );
                                modifier += 2;
                            }
                            _ => {}
                        }
                        modifier += 1;
                    }
                    if label_at.is_none() && t.windows(2).any(|t| t[0] == "legacy" && t[1] == "=") {
                        return Err(fail(
                            "legacy default requires a quoted parameter label".into(),
                        ));
                    }
                    if soft_range
                        && !matches!(ty, ParamType::Float { .. } | ParamType::Vector { .. })
                    {
                        return Err(fail(
                            "soft range requires a float or vector parameter".into(),
                        ));
                    }
                    let offset = def.slots();
                    def.params.push(Param {
                        id,
                        label: label_text,
                        ty,
                        default,
                        text,
                        offset,
                        soft_range,
                        legacy_default,
                    });
                }
                _ => return Err(fail(format!("unknown key `{key}:`"))),
            }
        }
        let fail = |message: &str| DefError {
            file: path.to_owned(),
            line: 1,
            message: message.to_owned(),
        };
        if def.outputs.is_empty() {
            return Err(fail("a node needs at least one `output:`"));
        }
        if let Some(host) = def.host {
            if def.multipass() || def.paint {
                return Err(fail("a `host:` node can't be multipass or painted"));
            }
            for (id, ty) in host.needs() {
                let ok = match def.param(id).map(|p| &p.ty) {
                    Some(ParamType::Text(TextKind::File)) => *ty == "file",
                    Some(ParamType::Text(TextKind::Line | TextKind::Lines)) => *ty == "text",
                    Some(ParamType::Float { .. }) => *ty == "float",
                    Some(ParamType::Enum(_)) => *ty == "enum",
                    _ => false,
                };
                if !ok {
                    return Err(fail(&format!("`host:` needs a `{id}` {ty} param")));
                }
            }
        }
        for (_, base) in &def.passes.terms {
            if let PassBase::Param(p) | PassBase::Equal(p, _) = base
                && !matches!(
                    def.param(p).map(|p| &p.ty),
                    Some(ParamType::Int { .. } | ParamType::Enum(_))
                )
            {
                return Err(fail("`passes:` names a param that is not an int"));
            }
        }
        Ok(def)
    }
}

fn push_line(text: &mut String, line: &str) {
    if !text.is_empty() {
        text.push(' ');
    }
    text.push_str(line);
}

/// `\n` and `\t` in a quoted default, and `\\` for a backslash.
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(c) => out.push(c),
            None => out.push('\\'),
        }
    }
    out
}

fn is_ident(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `levelinlow` -> `Levelinlow`, `edge_width` -> `Edge Width`.
pub fn title_case(id: &str) -> String {
    id.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Upper-case WGSL constant name for an enum option: `Add Sub` -> `ADD_SUB`, `F2-F1` ->
/// `F2_MINUS_F1` (`*` and `/` become `TIMES` and `OVER`, so labels stay distinct).
pub fn const_name(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let word = match c {
            '-' => "_MINUS_",
            '*' => "_TIMES_",
            '/' => "_OVER_",
            '+' => "_PLUS_",
            c if c.is_ascii_alphanumeric() => {
                out.push(c.to_ascii_uppercase());
                continue;
            }
            _ => "_",
        };
        for w in word.chars() {
            if !(w == '_' && (out.is_empty() || out.ends_with('_'))) {
                out.push(w);
            }
        }
    }
    out.trim_end_matches('_').to_owned()
}

/// Splits on whitespace outside quotes and parentheses; `=` is a token of its own.
fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut depth = 0u32;
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if !cur.is_empty() {
            out.push(std::mem::take(cur));
        }
    };
    for c in line.chars() {
        match c {
            '"' if depth == 0 => {
                quoted = !quoted;
                cur.push(c);
            }
            '(' if !quoted => {
                depth += 1;
                cur.push(c);
            }
            ')' if !quoted => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            '=' if !quoted && depth == 0 => {
                flush(&mut cur, &mut out);
                out.push("=".into());
            }
            c if c.is_whitespace() && !quoted && depth == 0 => flush(&mut cur, &mut out),
            c => cur.push(c),
        }
    }
    flush(&mut cur, &mut out);
    out
}

fn label(token: &str) -> Option<String> {
    token
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .map(str::to_owned)
}

fn id_socket(t: &[String]) -> Result<(&str, Socket), String> {
    let (Some(id), Some(socket)) = (t.first(), t.get(1)) else {
        return Err("expected `<id> <Gray|Color|Material>`".into());
    };
    let socket = Socket::parse(socket).ok_or(format!("unknown socket `{socket}`"))?;
    Ok((id, socket))
}

/// `0.5`, `(0.5, 0.5, 1)` or `0.5,0.5,1`.
fn numbers(s: &str) -> Result<Vec<f32>, String> {
    let s = s.trim().trim_start_matches('(').trim_end_matches(')');
    s.split([',', ' '])
        .filter(|v| !v.trim().is_empty())
        .map(|v| {
            v.trim()
                .parse::<f32>()
                .map_err(|_| format!("`{v}` is not a number"))
        })
        .collect()
}

/// `float(0,1)` -> ("float", ["0", "1"]).
fn type_args(s: &str) -> (&str, Vec<&str>) {
    match s.split_once('(') {
        Some((name, rest)) => {
            let args = rest.trim_end_matches(')');
            let sep = if args.contains('|') { '|' } else { ',' };
            (name, args.split(sep).map(str::trim).collect())
        }
        None => (s, Vec::new()),
    }
}

fn parse_type(s: &str) -> Result<ParamType, String> {
    let (name, args) = type_args(s);
    let range = |lo: f32, hi: f32| -> Result<(f32, f32), String> {
        match args.as_slice() {
            [] => Ok((lo, hi)),
            [a, b] => Ok((
                a.parse().map_err(|_| format!("bad range in `{s}`"))?,
                b.parse().map_err(|_| format!("bad range in `{s}`"))?,
            )),
            _ => Err(format!("`{s}` takes (min,max)")),
        }
    };
    Ok(match name {
        "float" => {
            let (min, max) = range(0.0, 1.0)?;
            ParamType::Float { min, max }
        }
        "int" => {
            let (min, max) = range(0.0, 16.0)?;
            ParamType::Int {
                min: min as i32,
                max: max as i32,
            }
        }
        "bool" => ParamType::Bool,
        "enum" if !args.is_empty() => ParamType::Enum(args.iter().map(|a| a.to_string()).collect()),
        "float2" | "float3" | "float4" => {
            let (min, max) = range(0.0, 1.0)?;
            let n = usize::from(name.as_bytes()[5] - b'0');
            ParamType::Vector { n, min, max }
        }
        "color" => ParamType::Color,
        "gradient" => ParamType::Gradient,
        "gradient_controls" => ParamType::GradientControls,
        "curve" if args.is_empty() => ParamType::Curve,
        "curve" => {
            let capacity = match args.as_slice() {
                [arg] => arg.parse::<usize>().ok(),
                _ => None,
            }
            .filter(|n| (2..=CURVE_CONTROL_POINTS).contains(n))
            .ok_or_else(|| {
                format!("`{s}` takes a point capacity from 2 to {CURVE_CONTROL_POINTS}")
            })?;
            ParamType::CurvePoints { capacity }
        }
        "text" => ParamType::Text(TextKind::Line),
        "lines" => ParamType::Text(TextKind::Lines),
        "file" => ParamType::Text(TextKind::File),
        "code" => ParamType::Text(TextKind::Code),
        _ => return Err(format!("unknown param type `{s}`")),
    })
}

fn parse_default(ty: &ParamType, t: &[String]) -> Result<Vec<f32>, String> {
    let nums = || -> Result<Vec<f32>, String> {
        let mut v = Vec::new();
        for s in t {
            v.extend(numbers(s)?);
        }
        Ok(v)
    };
    Ok(match ty {
        ParamType::Bool => match t.first().map(String::as_str) {
            None | Some("false") | Some("0") => vec![0.0],
            Some("true") | Some("1") => vec![1.0],
            Some(s) => return Err(format!("`{s}` is not a bool")),
        },
        ParamType::Enum(options) => match t.first() {
            None => vec![0.0],
            Some(s) => {
                let key = const_name(s);
                let index = options
                    .iter()
                    .position(|o| const_name(o) == key)
                    .or_else(|| s.parse::<usize>().ok().filter(|i| *i < options.len()))
                    .ok_or(format!("`{s}` is not an option"))?;
                vec![index as f32]
            }
        },
        ParamType::Color => match nums()?.as_slice() {
            [] => vec![0.0, 0.0, 0.0, 1.0],
            [g] => vec![*g, *g, *g, 1.0],
            [r, g, b] => vec![*r, *g, *b, 1.0],
            [r, g, b, a] => vec![*r, *g, *b, *a],
            _ => return Err("a colour takes 1, 3 or 4 numbers".into()),
        },
        ParamType::Gradient | ParamType::GradientControls => {
            let width = if *ty == ParamType::GradientControls {
                6
            } else {
                5
            };
            let v = nums()?;
            let stops: Vec<f32> = if v.is_empty() {
                [0., 1.]
                    .into_iter()
                    .flat_map(|p| {
                        let mut stop = vec![p, p, p, p, 1.];
                        if width == 6 {
                            stop.push(0.);
                        }
                        stop
                    })
                    .collect()
            } else if v.len() % width == 0 && v.len() / width <= GRADIENT_STOPS {
                v
            } else {
                return Err(format!("a gradient takes {width} values per stop"));
            };
            let mut out = vec![(stops.len() / width) as f32];
            out.extend(stops);
            out
        }
        ParamType::Curve | ParamType::CurvePoints { .. } => {
            if t.len() == 1 && t[0] == "empty" {
                return Ok(vec![0.0]);
            }
            let v = nums()?;
            let points: Vec<[f32; 2]> = if v.is_empty() {
                vec![[0.0, 0.0], [1.0, 1.0]]
            } else if v.len() % 2 == 0 && v.len() / 2 <= ty.curve_capacity().unwrap_or(0) {
                v.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
            } else {
                return Err("a curve takes (x y) per point".into());
            };
            curve_values(&points)
        }
        _ => {
            let v = nums()?;
            let n = ty.slots();
            match v.len() {
                0 => vec![0.0; n],
                1 => vec![v[0]; n],
                l if l == n => v,
                _ => return Err(format!("expected 1 or {n} numbers")),
            }
        }
    })
}

/// Packs curve points with handles a third of the way to each neighbour (straight segments).
pub fn curve_values(points: &[[f32; 2]]) -> Vec<f32> {
    let mut out = vec![points.len() as f32];
    for (i, p) in points.iter().enumerate() {
        let prev = points.get(i.wrapping_sub(1)).unwrap_or(p);
        let next = points.get(i + 1).unwrap_or(p);
        let l = [p[0] + (prev[0] - p[0]) / 3.0, p[1] + (prev[1] - p[1]) / 3.0];
        let r = [p[0] + (next[0] - p[0]) / 3.0, p[1] + (next[1] - p[1]) / 3.0];
        out.extend([p[0], p[1], l[0], l[1], r[0], r[1]]);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "def/tests.rs"]
mod tests;
