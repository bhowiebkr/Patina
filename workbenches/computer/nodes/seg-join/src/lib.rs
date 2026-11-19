use patina_sdk::node;
use segments::Segments;

/// Joins segments a to g of a digit into one bus.
#[node(color = "#175c59", symbol = "bus")]
pub fn seg_join(a: &bool, b: &bool, c: &bool, d: &bool, e: &bool, f: &bool, g: &bool) -> Segments {
    Segments([a, b, c, d, e, f, g].iter().fold(0, |n, &&s| n << 1 | u8::from(s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_a_at_the_top() {
        assert_eq!(seg_join(&true, &false, &false, &false, &false, &true, &true), Segments(0b100_0011));
    }
}
