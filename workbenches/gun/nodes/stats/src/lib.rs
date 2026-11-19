use guns::Gun;
use patina_sdk::node;

/// The maker, the rarity, the stats and the parts of the gun as text, one item on each line. It
/// also writes the text to the standard output.
#[node(color = "#3a3c40")]
pub fn stats(gun: &Gun) -> String {
    let report = gun.report();
    print!("{report}");
    report
}
