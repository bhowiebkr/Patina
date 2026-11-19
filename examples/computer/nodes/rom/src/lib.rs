use patina_sdk::node;

const WORDS: usize = 256;

/// A ROM of 256 words of 16 bits, as the ROM 256x16 chip of Digital Logic Sim 2. It gives the word
/// at `address`: the high byte on `out0` and the low byte on `out1`. `words` is the text of the
/// words from address 0, with spaces, commas or lines between them. A word is decimal, hexadecimal
/// with `0x` or binary with `0b`, and `_` can split its digits. An incorrect word is an error.
#[node(color = "#655219")]
pub struct Rom {
    text: String,
    words: Result<[u16; WORDS], String>,
}

impl Rom {
    pub fn init() -> Self {
        Rom { text: String::new(), words: Ok([0; WORDS]) }
    }

    pub fn process(&mut self, address: &u8, words: &str) -> Result<(u8, u8), String> {
        if self.text != words {
            words.clone_into(&mut self.text);
            self.words = parse(words);
        }
        let [high, low] = self.words.as_ref().map_err(String::clone)?[usize::from(*address)].to_be_bytes();
        Ok((high, low))
    }
}

/// Reads the words of `text`, from address 0. The other words are 0.
fn parse(text: &str) -> Result<[u16; WORDS], String> {
    let mut words = [0; WORDS];
    for (n, part) in text.split(|c: char| c.is_whitespace() || c == ',').filter(|p| !p.is_empty()).enumerate() {
        let digits = part.replace('_', "").to_ascii_lowercase();
        let word = match (digits.strip_prefix("0x"), digits.strip_prefix("0b")) {
            (Some(hex), _) => u16::from_str_radix(hex, 16),
            (_, Some(bits)) => u16::from_str_radix(bits, 2),
            _ => digits.parse(),
        };
        let word = word.map_err(|_| format!("The word {n} \"{part}\" is not a 16-bit number"))?;
        *words.get_mut(n).ok_or(format!("The ROM has {WORDS} words, and the text has more"))? = word;
    }
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_decimal_hex_and_binary_words() {
        let mut rom = Rom::init();
        let words = "258, 0x1F_02\n0b1_0000_0011";
        assert_eq!([0, 1, 2, 3].map(|a| rom.process(&a, words)), [Ok((1, 2)), Ok((0x1F, 2)), Ok((1, 3)), Ok((0, 0))]);
        assert_eq!(rom.process(&0, "1 0x10000"), Err("The word 1 \"0x10000\" is not a 16-bit number".to_owned()));
        assert!(parse(&"1 ".repeat(257)).is_err());
    }
}
