//! Midpoints travel with their keys during sorting and survive old five-word gradients.
use super::GRADIENT_STOPS;

pub(super) fn pack(stored: &[f32]) -> Vec<f32> {
    let count = (stored[0] as usize).min(GRADIENT_STOPS);
    let legacy = stored.len() == 1 + count * 5 || stored.len() == 1 + GRADIENT_STOPS * 5;
    let width = if legacy { 5 } else { 6 };
    let count = count.min((stored.len() - 1) / width);
    let mut keys: Vec<[f32; 6]> = stored[1..]
        .chunks_exact(width)
        .take(count)
        .map(|key| std::array::from_fn(|i| key.get(i).copied().unwrap_or(0.)))
        .collect();
    keys.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let mut packed = vec![count as f32];
    packed.extend(keys.into_iter().flatten());
    packed.resize(1 + GRADIENT_STOPS * 6, 0.);
    packed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midpoint_bias_stays_with_its_sorted_key() {
        let packed = pack(&[2., 1., 1., 1., 1., 0., -0.75, 0., 0., 0., 0., 1., 0.25]);
        assert_eq!(
            &packed[..13],
            &[2., 0., 0., 0., 0., 1., 0.25, 1., 1., 1., 1., 0., -0.75]
        );
    }

    #[test]
    fn compact_and_padded_legacy_values_gain_neutral_midpoints() {
        let compact = [2., 1., 1., 0.5, 0.25, 0., 0., 0.1, 0.2, 0.3, 1.];
        let expected = [2., 0., 0.1, 0.2, 0.3, 1., 0., 1., 1., 0.5, 0.25, 0., 0.];
        for values in [compact.to_vec(), {
            let mut padded = compact.to_vec();
            padded.resize(1 + GRADIENT_STOPS * 5, 0.);
            padded
        }] {
            assert_eq!(&pack(&values)[..13], &expected);
        }
    }
}
