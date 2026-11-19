use city::Streets;
use patina_sdk::node;

/// Removes the dead ends again and again, until each street is on a loop. The highways stay.
#[node(color = "#3a3c40")]
pub fn prune(streets: &Streets) -> Streets {
    city::prune(streets)
}
