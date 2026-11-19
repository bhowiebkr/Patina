use city::{Plots, Zone};
use patina_sdk::node;

/// Only the plots that are parks, for example to put trees on them.
#[node]
pub fn parks(plots: &Plots) -> Plots {
    Plots(plots.0.iter().filter(|p| p.zone == Zone::Park).cloned().collect())
}
