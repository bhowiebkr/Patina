//! Resolves the type parameters of a generic node from the types of its wires (spec 4.5). The
//! types are the texts of `node::type_text`: a space only between two words and after a comma.

/// Splits a type text into words, for example `Vec` or `'a`, and single punctuation characters.
fn tokens(ty: &str) -> Vec<&str> {
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '\'';
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in ty.char_indices() {
        match (word(c), start) {
            (true, None) => start = Some(i),
            (true, Some(_)) => {}
            (false, s) => {
                if let Some(s) = s {
                    out.push(&ty[s..i]);
                    start = None;
                }
                if c != ' ' {
                    out.push(&ty[i..i + c.len_utf8()]);
                }
            }
        }
    }
    out.extend(start.map(|s| &ty[s..]));
    out
}

/// The number of tokens of the first whole type in `tokens`, up to a comma or a close at depth 0.
fn one_type(tokens: &[&str]) -> usize {
    let mut depth = 0usize;
    for (i, &t) in tokens.iter().enumerate() {
        match t {
            "<" | "(" | "[" => depth += 1,
            ">" if i > 0 && tokens[i - 1] == "-" => {}
            ">" | ")" | "]" if depth == 0 => return i,
            ">" | ")" | "]" => depth -= 1,
            "," if depth == 0 => return i,
            _ => {}
        }
    }
    tokens.len()
}

/// Joins tokens with a space between two words and after a comma, as `node::type_text` does.
fn join<'a>(tokens: impl IntoIterator<Item = &'a str>) -> String {
    let mut text = String::new();
    for t in tokens {
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '\'');
        if word(text.chars().last()) && word(t.chars().next()) || text.ends_with(',') {
            text.push(' ');
        }
        text.push_str(t);
    }
    text
}

/// Matches `pattern` against `actual`, and binds each parameter of `params` that it finds. Returns
/// a text for the user when a parameter already has another type. A difference in the other parts
/// binds nothing: the compiler reports it.
pub fn unify(pattern: &str, actual: &str, params: &[String], bound: &mut [Option<String>]) -> Result<(), String> {
    let (pattern, actual) = (tokens(pattern), tokens(actual));
    let (mut p, mut a) = (0, 0);
    let mut found = Vec::new();
    while p < pattern.len() && a < actual.len() {
        if let Some(k) = params.iter().position(|n| n == pattern[p]) {
            let len = one_type(&actual[a..]);
            found.push((k, join(actual[a..a + len].iter().copied())));
            a += len;
        } else if pattern[p] == actual[a] {
            a += 1;
        } else {
            return Ok(());
        }
        p += 1;
    }
    if p < pattern.len() || a < actual.len() {
        return Ok(());
    }
    for (k, ty) in found {
        match &bound[k] {
            Some(old) if *old != ty => return Err(format!("`{}` is `{old}` and `{ty}`", params[k])),
            _ => bound[k] = Some(ty),
        }
    }
    Ok(())
}

/// True when the type text names one of `params`.
pub fn mentions(ty: &str, params: &[String]) -> bool {
    tokens(ty).iter().any(|t| params.iter().any(|p| p == t))
}

/// Puts the bound type of each parameter into `ty`.
pub fn substitute(ty: &str, params: &[String], bound: &[String]) -> String {
    join(tokens(ty).into_iter().map(|t| params.iter().position(|n| n == t).map_or(t, |k| &bound[k])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_parameters_and_puts_them_back() {
        let params = ["T".to_owned(), "U".to_owned()];
        let mut bound = [None, None];
        unify("Vec<T>", "Vec<HashMap<u8, f64>>", &params, &mut bound).unwrap();
        unify("(U, T)", "(&'static str, HashMap<u8, f64>)", &params, &mut bound).unwrap();
        assert_eq!(bound, [Some("HashMap<u8, f64>".to_owned()), Some("&'static str".to_owned())]);
        assert!(unify("T", "f64", &params, &mut bound).is_err());
        unify("Option<U>", "Vec<f64>", &params, &mut bound).unwrap();
        unify("fn(T)->U", "fn(HashMap<u8, f64>)->&'static str", &params, &mut bound).unwrap();
        let bound = bound.map(Option::unwrap);
        assert_eq!(substitute("Vec<(T, U)>", &params, &bound), "Vec<(HashMap<u8, f64>, &'static str)>");
        assert_eq!(substitute("Box<dyn Fn(T)>", &params, &bound), "Box<dyn Fn(HashMap<u8, f64>)>");
        assert!(mentions("Vec<(u8, U)>", &params) && !mentions("Vec<Tx>", &params));
    }
}
