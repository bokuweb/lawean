//! 整数 → 漢数字（法令の条・項番号の表記）。`lawean-resolve::numeral::kanji_to_u32` の逆

pub fn to_kanji(n: u32) -> String {
    if n == 0 {
        return "〇".into();
    }
    const D: [&str; 10] = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    let mut s = String::new();
    let (th, rest) = (n / 1000, n % 1000);
    let (hu, rest) = (rest / 100, rest % 100);
    let (te, on) = (rest / 10, rest % 10);
    if th > 0 {
        if th > 1 {
            s.push_str(D[th as usize]);
        }
        s.push('千');
    }
    if hu > 0 {
        if hu > 1 {
            s.push_str(D[hu as usize]);
        }
        s.push('百');
    }
    if te > 0 {
        if te > 1 {
            s.push_str(D[te as usize]);
        }
        s.push('十');
    }
    s.push_str(D[on as usize]);
    s
}

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
