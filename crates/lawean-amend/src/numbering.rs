//! 条の連番の検査。溶け込み後（と発射台）の本則で、条番号が文書順に「次の番号」になっているか。
//!
//! 「次の番号」の規則（法制執務の枝番）:
//! - 第N条 の次は 第N+1条、または 第N条の二（枝番を 1 段深く始める。最初は「の二」）
//! - 第N条のM の次は 第N条のM+1、第N条のMの二（さらに深く）、または上の段に戻って 第N+1条
//! - 第N条のMのK の次は 第N条のMのK+1、または 第N条のM+1、または 第N+1条
//! - 「第N条から第M条まで　削除」（Num="N:M"）は N から M を占める
//!
//! 発射台にもともとある崩れ（e-Gov のデータの都合）は、溶け込みで新しく生じたものと区別して報告する。

use lawean_source::{ArticleNum, LegalDocument, Provision};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberingIssue {
    /// 直前の条
    pub prev: String,
    /// 続く条（ここが「次の番号」でない）
    pub next: String,
    pub message: String,
}

/// 「第十二条の二」「第三条から第五条まで」
pub fn label(n: &ArticleNum) -> String {
    use lawean_resolve::numeral::to_kanji;
    match n {
        ArticleNum::Single { base, branch } => {
            let mut s = format!("第{}条", to_kanji(*base));
            for b in branch {
                s.push_str(&format!("の{}", to_kanji(*b)));
            }
            s
        }
        ArticleNum::Range { from, to } => format!("{}から{}まで", label(from), label(to)),
        ArticleNum::Other(s) => s.clone(),
    }
}

/// `prev` の次に `next` が来てよいか
pub fn succeeds(prev: &ArticleNum, next: &ArticleNum) -> bool {
    let prev = match prev {
        ArticleNum::Range { to, .. } => to.as_ref(),
        p => p,
    };
    let next = match next {
        ArticleNum::Range { from, .. } => from.as_ref(),
        n => n,
    };
    let (
        ArticleNum::Single {
            base: pb,
            branch: pbr,
        },
        ArticleNum::Single {
            base: nb,
            branch: nbr,
        },
    ) = (prev, next)
    else {
        // 読めない表記は判定しない
        return true;
    };
    if *nb == pb + 1 && nbr.is_empty() {
        return true;
    }
    if nb != pb {
        return false;
    }
    // 1 段深く始める: 枝番は「の二」から
    if nbr.len() == pbr.len() + 1 && nbr[..pbr.len()] == pbr[..] && nbr[pbr.len()] == 2 {
        return true;
    }
    // 同じ段か上の段で +1
    for k in 1..=pbr.len() {
        if nbr.len() == k && nbr[..k - 1] == pbr[..k - 1] && nbr[k - 1] == pbr[k - 1] + 1 {
            return true;
        }
    }
    false
}

/// 本則の条を文書順に
pub fn article_nums(doc: &LegalDocument) -> Vec<ArticleNum> {
    fn walk(ps: &[Provision], out: &mut Vec<ArticleNum>) {
        for p in ps {
            match p {
                Provision::Container(c) => walk(&c.children, out),
                Provision::Article(a) => out.push(a.num.clone()),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&doc.main_provision, &mut out);
    out
}

/// 条番号の列の崩れ
pub fn check_sequence(nums: &[ArticleNum]) -> Vec<NumberingIssue> {
    let mut out = Vec::new();
    if let Some(first) = nums.first() {
        if !matches!(first, ArticleNum::Single { base: 1, branch } if branch.is_empty()) {
            out.push(NumberingIssue {
                prev: String::new(),
                next: label(first),
                message: format!("本則が{}から始まる（第一条でない）", label(first)),
            });
        }
    }
    for w in nums.windows(2) {
        if !succeeds(&w[0], &w[1]) {
            out.push(NumberingIssue {
                prev: label(&w[0]),
                next: label(&w[1]),
                message: format!("{}の次が{}（連番でない）", label(&w[0]), label(&w[1])),
            });
        }
    }
    out
}

/// 文書の本則の条の連番
pub fn check_document(doc: &LegalDocument) -> Vec<NumberingIssue> {
    check_sequence(&article_nums(doc))
}

/// `IdentRevision` の描画（条の番号の列。`art` は "142_4" / "153:160" / "toc"）から
pub fn check_art_strings<'a>(arts: impl IntoIterator<Item = &'a str>) -> Vec<NumberingIssue> {
    let mut nums = Vec::new();
    let mut last: Option<&str> = None;
    for a in arts {
        if a == "toc" || a == "main" || last == Some(a) {
            continue;
        }
        last = Some(a);
        nums.push(ArticleNum::parse(a));
    }
    check_sequence(&nums)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> ArticleNum {
        ArticleNum::parse(s)
    }

    #[test]
    fn branch_rules() {
        assert!(succeeds(&n("1"), &n("2")));
        assert!(succeeds(&n("2"), &n("2_2")));
        assert!(succeeds(&n("2_2"), &n("2_3")));
        assert!(succeeds(&n("2_3"), &n("2_3_2")));
        assert!(succeeds(&n("2_3_2"), &n("2_3_3")));
        assert!(succeeds(&n("2_3_4"), &n("2_4")));
        assert!(succeeds(&n("2_3_4"), &n("3")));
        assert!(succeeds(&n("2_9"), &n("3")));
        assert!(succeeds(&n("153:160"), &n("161")));
        assert!(succeeds(&n("152"), &n("153:160")));
        // 崩れ
        assert!(!succeeds(&n("1"), &n("3")));
        assert!(!succeeds(&n("2"), &n("2_3")), "枝番は「の二」から");
        assert!(!succeeds(&n("2_2"), &n("2_2_3")));
        assert!(!succeeds(&n("2_3_4"), &n("2_5")));
        assert!(!succeeds(&n("2_3_4"), &n("4")));
        assert!(!succeeds(&n("3"), &n("2")));
        assert!(!succeeds(&n("2_2"), &n("2")));
    }

    #[test]
    fn sequence_report() {
        let issues = check_sequence(&[n("1"), n("2"), n("2_2"), n("4"), n("4_2_2")]);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].message, "第二条の二の次が第四条（連番でない）");
        assert_eq!(
            issues[1].message,
            "第四条の次が第四条の二の二（連番でない）"
        );
        assert!(check_sequence(&[n("2")])
            .iter()
            .any(|i| i.message.contains("第一条でない")));
    }
}
