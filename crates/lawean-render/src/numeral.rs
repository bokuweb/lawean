//! 整数 → 漢数字（法令の条・項番号の表記）。`lawean-resolve::numeral::kanji_to_u32` の逆

pub use lawean_resolve::numeral::to_kanji;

pub fn fullwidth(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|c| char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lawean_resolve::numeral::kanji_to_u32;

    #[test]
    fn roundtrip() {
        for n in [1, 10, 11, 20, 38, 100, 133, 604, 1200, 2026] {
            assert_eq!(kanji_to_u32(&to_kanji(n)), Some(n), "{n}");
        }
        assert_eq!(to_kanji(38), "三十八");
        assert_eq!(to_kanji(133), "百三十三");
    }
}
