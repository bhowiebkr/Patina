use patina_sdk::node;

/// The NAND gate. The other gates of the example are subgraphs of NAND gates.
#[node(color = "#71344f", symbol = "nand")]
pub fn nand(a: &bool, b: &bool) -> bool {
    !(*a && *b)
}
