use geometry::Sketch;
use patina_sdk::node;

/// The shape inside the line through the points, for example "0 0, 2 0, 1 1.5". Write each
/// point as two numbers, with a comma between the points.
#[node]
pub fn polygon(points: &str) -> Result<Sketch, String> {
    Sketch::polygon(&corners(points)?).checked()
}

fn corners(points: &str) -> Result<Vec<[f64; 2]>, String> {
    let mut corners = Vec::new();
    for point in points.split(',').map(str::trim).filter(|point| !point.is_empty()) {
        let numbers: Option<Vec<f64>> = point.split_whitespace().map(|w| w.parse().ok().filter(|n: &f64| n.is_finite())).collect();
        match numbers.as_deref() {
            Some(&[x, y]) => corners.push([x, y]),
            _ => return Err(format!("write the point {point:?} as two numbers")),
        }
    }
    if corners.len() < 3 {
        return Err(format!("write 3 points or more, not {}", corners.len()));
    }
    Ok(corners)
}

#[cfg(test)]
mod tests {
    use super::corners;

    #[test]
    fn the_points_are_pairs_of_numbers() {
        assert_eq!(corners("0 0, 2 0,1 1.5, "), Ok(vec![[0.0, 0.0], [2.0, 0.0], [1.0, 1.5]]));
        assert_eq!(corners("0 0, 2 0"), Err("write 3 points or more, not 2".into()));
        assert_eq!(corners("0 0, 2, 1 1"), Err("write the point \"2\" as two numbers".into()));
        assert!(corners("0 0, 2 inf, 1 1").is_err());
    }
}
