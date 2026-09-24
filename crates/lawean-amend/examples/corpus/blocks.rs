//! 制定法律の平文から改正単位の塊（「第N条　X法（…）の一部を次のように改正する。」から次の単位まで）を切り出す。
//! `survey_corpus` と `bench_apply` で共有する

/// 法令ごとに本則（附則より前）を改正単位に切る: 「第N条　X法（…）の一部を次のように改正する。」（整備法）、
/// 「X法（…）の一部を次のように改正する。」（単独の一部改正法）、「第N条　次に掲げる法律の規定中…」（列挙形）
pub fn blocks(text: &str) -> Vec<String> {
    let numbered = regex::Regex::new(r"^第[一二三四五六七八九十百千]+条(?:の[一二三四五六七八九十百千]+)*　(?:.+の一部を次のように改正する。|次に掲げる法律の規定中.+)$").unwrap();
    let single = regex::Regex::new(r"^.+の一部を次のように改正する。$").unwrap();
    let other_art =
        regex::Regex::new(r"^第[一二三四五六七八九十百千]+条(?:の[一二三四五六七八九十百千]+)*　")
            .unwrap();
    let mut out: Vec<Vec<&str>> = Vec::new();
    let mut numbered_mode = false;
    let mut open = false;
    for line in text.lines() {
        let indent = line.chars().take_while(|c| *c == '\u{3000}').count();
        let t = line.trim_start_matches('\u{3000}').trim_end();
        if t == "附\u{3000}則" {
            break;
        }
        if indent == 0 && numbered.is_match(t) {
            out.push(vec![line]);
            numbered_mode = true;
            open = true;
            continue;
        }
        if indent <= 1 && !t.starts_with('第') && single.is_match(t) && !t.starts_with('（') {
            out.push(vec![line]);
            numbered_mode = false;
            open = true;
            continue;
        }
        // 整備法の改正でない条（「第三条　次に掲げる法律は、廃止する。」）で単位は終わる
        if numbered_mode && indent == 0 && other_art.is_match(t) {
            open = false;
            continue;
        }
        if open {
            if let Some(b) = out.last_mut() {
                b.push(line);
            }
        }
    }
    out.into_iter().map(|b| b.join("\n")).collect()
}
