//! The parser of the header lines: tokens, labels, numbers, param types and defaults.
use super::{
    CURVE_CONTROL_POINTS, GRADIENT_STOPS, ParamType, Socket, TextKind, const_name, curve_values,
};

pub(super) fn push_line(text: &mut String, line: &str) {
    if !text.is_empty() {
        text.push(' ');
    }
    text.push_str(line);
}

/// `\n` and `\t` in a quoted default, and `\\` for a backslash.
pub(super) fn unescape(s: &str) -> String {
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

pub(super) fn is_ident(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Splits on whitespace outside quotes and parentheses; `=` is a token of its own.
pub(super) fn tokens(line: &str) -> Vec<String> {
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

pub(super) fn label(token: &str) -> Option<String> {
    token
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .map(str::to_owned)
}

pub(super) fn id_socket(t: &[String]) -> Result<(&str, Socket), String> {
    let (Some(id), Some(socket)) = (t.first(), t.get(1)) else {
        return Err("expected `<id> <Gray|Color|Material>`".into());
    };
    let socket = Socket::parse(socket).ok_or(format!("unknown socket `{socket}`"))?;
    Ok((id, socket))
}

/// `0.5`, `(0.5, 0.5, 1)` or `0.5,0.5,1`.
pub(super) fn numbers(s: &str) -> Result<Vec<f32>, String> {
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

pub(super) fn parse_type(s: &str) -> Result<ParamType, String> {
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

pub(super) fn parse_default(ty: &ParamType, t: &[String]) -> Result<Vec<f32>, String> {
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
