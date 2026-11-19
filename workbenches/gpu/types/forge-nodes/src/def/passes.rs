//! Integer pass expressions; a mode test adds a setup pass only when needed.
use super::{PassBase, Passes, is_ident};

pub(super) fn parse(s: &str) -> Result<Passes, String> {
    let mut terms = Vec::new();
    for term in s.split('+') {
        let (mul, base) = match term.split_once('*') {
            Some((k, b)) => (
                k.trim()
                    .parse::<i32>()
                    .map_err(|_| format!("bad pass factor `{k}`"))?,
                b.trim(),
            ),
            None => (1, term.trim()),
        };
        let base = if let Some((name, value)) = base.split_once("==") {
            if !is_ident(name.trim()) {
                return Err(format!("bad pass mode `{name}`"));
            }
            PassBase::Equal(
                name.trim().to_owned(),
                value
                    .trim()
                    .parse()
                    .map_err(|_| format!("bad pass mode value `{value}`"))?,
            )
        } else if base == "log2" {
            PassBase::Log2
        } else if let Ok(n) = base.parse::<u32>() {
            PassBase::Fixed(n)
        } else if is_ident(base) {
            PassBase::Param(base.to_owned())
        } else {
            return Err(format!("bad passes `{s}`"));
        };
        terms.push((mul, base));
    }
    Ok(Passes { terms })
}

#[cfg(test)]
mod tests {
    use crate::Def;

    #[test]
    fn only_the_selected_mode_adds_a_setup_pass() {
        let def = Def::parse(
            "x.wesl",
            "//! output: out Color\n//! param: mode enum(A|B|C) = b\n//! passes: 1+mode==2",
        )
        .unwrap();
        assert_eq!(def.pass_count(64, &[]), 1);
        for (mode, passes) in [(0., 1), (1., 1), (2., 2)] {
            assert_eq!(def.pass_count(64, &[mode]), passes);
        }
        for header in [
            "//! passes: 1+unknown==2",
            "//! param: mode float\n//! passes: 1+mode==2",
            "//! param: mode int(0,2)\n//! passes: 1+mode==x",
        ] {
            assert!(Def::parse("bad.wesl", header).is_err());
        }
    }
}
