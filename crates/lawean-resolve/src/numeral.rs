//! 漢数字 → 整数。法令の条・項・号番号に出る範囲（〜千）だけ。

pub fn kanji_to_u32(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let mut total = 0u32;
    let mut current = 0u32;
    for c in s.chars() {
        match c {
            '〇' | '零' => {}
            '一' => current = 1,
            '二' => current = 2,
            '三' => current = 3,
            '四' => current = 4,
            '五' => current = 5,
            '六' => current = 6,
            '七' => current = 7,
            '八' => current = 8,
            '九' => current = 9,
            '十' => {
                total += if current == 0 { 10 } else { current * 10 };
                current = 0;
            }
            '百' => {
                total += if current == 0 { 100 } else { current * 100 };
                current = 0;
            }
            '千' => {
                total += if current == 0 { 1000 } else { current * 1000 };
                current = 0;
            }
            _ => return None,
        }
    }
    Some(total + current)
}

/// 整数 → 漢数字（法令の条・項番号の表記）。`kanji_to_u32` の逆
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numerals() {
        assert_eq!(kanji_to_u32("一"), Some(1));
        assert_eq!(kanji_to_u32("十"), Some(10));
        assert_eq!(kanji_to_u32("三十八"), Some(38));
        assert_eq!(kanji_to_u32("百三十三"), Some(133));
        assert_eq!(kanji_to_u32("六百四"), Some(604));
        assert_eq!(kanji_to_u32("千二百"), Some(1200));
        assert_eq!(kanji_to_u32("x"), None);
    }
}
