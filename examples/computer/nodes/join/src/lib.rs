use patina_sdk::node;

/// Joins 8 bits into a byte. An unwired bit is a parameter.
#[node]
#[allow(clippy::too_many_arguments)]
pub fn join(b7: &bool, b6: &bool, b5: &bool, b4: &bool, b3: &bool, b2: &bool, b1: &bool, b0: &bool) -> u8 {
    [b7, b6, b5, b4, b3, b2, b1, b0].iter().fold(0, |n, &&b| n << 1 | u8::from(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_b7_at_the_top() {
        assert_eq!(join(&true, &false, &false, &false, &false, &true, &false, &true), 0b1000_0101);
    }
}
