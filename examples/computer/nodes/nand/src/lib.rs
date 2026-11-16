use patina_sdk::node;

/// The NAND gate. The other gates of the example are subgraphs of NAND gates.
#[node]
pub fn nand(a: &bool, b: &bool) -> bool {
    !(*a && *b)
}
