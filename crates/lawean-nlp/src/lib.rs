//! 層 1.5: 文の主体・客体・行為を GiNZA の係り受け（jewel、Python 不要）から取る。
//!
//! 規則（層 1）は文末の効果と条件節の形までしか見ない。「誰が」「何を」は格助詞の正規表現より
//! 係り受け（UD の `nsubj` / `obj` / `obl`）で取る方が、長い連体修飾や並列に強い。
//! モデルは `JEWEL_GINZA_BUNDLE`（jewel の `tools/export_spacy_model.py` で出した `ja_ginza.spacy-rs`）。
//! 無ければ `Parser::load` が `Err` を返し、呼ぶ側は層 1 だけで続ける（この crate は WASM には入れない）。
//!
//! 出力は `lawean-extract::candidate::Candidate`（原文根拠つき）。値は確定しない。

use jewel_core::{Bundle, Doc, StringStore, TokenData};
use jewel_yoyogi::GinzaPipeline;
use lawean_extract::candidate::{Candidate, Confidence, Field};
use lawean_source::StableId;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum NlpError {
    #[error("GiNZA bundle not available: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Bundle(#[from] jewel_core::BundleError),
    #[error(transparent)]
    Ginza(#[from] jewel_yoyogi::GinzaError),
}

pub struct Parser {
    pipeline: GinzaPipeline,
}

/// 係り受け木の 1 語（表示・検査用に文字列へ戻したもの）
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Word {
    pub i: usize,
    pub text: String,
    /// Sudachi の品詞（「名詞-普通名詞-一般」）。既知のものだけ
    pub tag: Option<&'static str>,
    /// UD の関係（`nsubj` など）。既知のものだけ。`_bunsetu` は落としてある
    pub dep: Option<&'static str>,
    /// 文節の主辞（parser の `_bunsetu` 付きの関係だった）
    pub bunsetu_head: bool,
    pub head: usize,
    /// 文の本文の UTF-8 バイト範囲
    pub start: usize,
    pub end: usize,
}

const DEPS: &[&str] = &[
    "ROOT",
    "nsubj",
    "obj",
    "iobj",
    "obl",
    "nmod",
    "acl",
    "advcl",
    "advmod",
    "amod",
    "case",
    "cc",
    "conj",
    "mark",
    "aux",
    "cop",
    "det",
    "compound",
    "fixed",
    "flat",
    "punct",
    "dep",
    "csubj",
    "ccomp",
    "xcomp",
    "nummod",
    "appos",
    "dislocated",
    "discourse",
    "list",
    "parataxis",
    "orphan",
    "reparandum",
    "vocative",
    "goeswith",
    "clf",
    "expl",
    "obl:tmod",
    "nmod:poss",
    "acl:relcl",
    "nsubj:pass",
    "csubj:pass",
    "aux:pass",
    "compound:prt",
    "obl:npmod",
];
const TAGS: &[&str] = &[
    "名詞-普通名詞-一般",
    "名詞-普通名詞-サ変可能",
    "名詞-普通名詞-副詞可能",
    "名詞-普通名詞-形状詞可能",
    "名詞-普通名詞-助数詞可能",
    "名詞-固有名詞-一般",
    "名詞-固有名詞-人名-一般",
    "名詞-固有名詞-地名-一般",
    "名詞-数詞",
    "名詞-助動詞語幹",
    "動詞-一般",
    "動詞-非自立可能",
    "形容詞-一般",
    "形容詞-非自立可能",
    "形状詞-一般",
    "形状詞-助動詞語幹",
    "助詞-格助詞",
    "助詞-係助詞",
    "助詞-副助詞",
    "助詞-接続助詞",
    "助詞-終助詞",
    "助詞-準体助詞",
    "助動詞",
    "接尾辞-名詞的-一般",
    "接尾辞-名詞的-サ変可能",
    "接尾辞-名詞的-助数詞",
    "接頭辞",
    "連体詞",
    "副詞",
    "接続詞",
    "補助記号-句点",
    "補助記号-読点",
    "補助記号-括弧開",
    "補助記号-括弧閉",
    "補助記号-一般",
    "記号-一般",
    "空白",
];

fn name_of(table: &'static [&'static str], id: u64) -> Option<&'static str> {
    table.iter().copied().find(|s| StringStore::id(s) == id)
}

/// GiNZA の parser は文節の主辞に `_bunsetu` を付けた関係（`nsubj_bunsetu`）を出し、Python 側の
/// `bunsetu_recognizer` がそれを落として文節境界に変える。jewel の抽出プロファイルはその部品を持たないので、
/// ここで落とす（文節の主辞かどうかは `Word::bunsetu_head`）
fn dep_of(id: u64) -> (Option<&'static str>, bool) {
    if let Some(d) = name_of(DEPS, id) {
        return (Some(d), false);
    }
    for d in DEPS {
        if StringStore::id(&format!("{d}_bunsetu")) == id {
            return (Some(d), true);
        }
    }
    (None, false)
}

impl Parser {
    /// `JEWEL_GINZA_BUNDLE` から読む
    pub fn from_env() -> Result<Self, NlpError> {
        let p = std::env::var("JEWEL_GINZA_BUNDLE")
            .map_err(|_| NlpError::Unavailable("JEWEL_GINZA_BUNDLE is not set".into()))?;
        Self::load(Path::new(&p))
    }

    pub fn load(path: &Path) -> Result<Self, NlpError> {
        if !path.exists() {
            return Err(NlpError::Unavailable(format!(
                "{} does not exist",
                path.display()
            )));
        }
        let bundle = Bundle::load(path)?;
        Ok(Parser {
            pipeline: GinzaPipeline::load(&bundle)?,
        })
    }

    pub fn doc(&self, text: &str) -> Result<Doc, NlpError> {
        Ok(self.pipeline.process(text)?)
    }

    /// 係り受け木を文字列に戻したもの
    pub fn words(&self, text: &str) -> Result<Vec<Word>, NlpError> {
        let doc = self.doc(text)?;
        Ok(words_of(&doc, text))
    }
}

/// 括弧書きを落とした本文と、落とした後のバイト位置 → 元のバイト位置の対応。
/// 法令の文は括弧書き（「（第二百一条の十五第一項において準用する場合を含む。）」）が深く、係り受けを壊すので、
/// 解析は落とした本文で行い、根拠の位置は元に戻す
pub struct Stripped {
    pub text: String,
    /// `orig[i]` = 落とした本文のバイト i が元の本文のどのバイトか（`orig[text.len()]` は末尾）
    orig: Vec<usize>,
}

impl Stripped {
    pub fn new(text: &str) -> Self {
        let mut out = String::with_capacity(text.len());
        let mut orig = Vec::with_capacity(text.len() + 1);
        let mut depth = 0usize;
        for (b, c) in text.char_indices() {
            match c {
                '（' => depth += 1,
                '）' => depth = depth.saturating_sub(1),
                _ if depth == 0 => {
                    for k in 0..c.len_utf8() {
                        orig.push(b + k);
                    }
                    out.push(c);
                }
                _ => {}
            }
        }
        orig.push(text.len());
        Stripped { text: out, orig }
    }
    /// 落とした本文の範囲を元の範囲に
    pub fn to_orig(&self, start: usize, end: usize) -> (usize, usize) {
        let last = self.orig[self.orig.len() - 1];
        let s = self.orig.get(start).copied().unwrap_or(last);
        // 終端は「最後のバイトの次」。落とした括弧書きの手前で止める
        let e = if end == 0 {
            s
        } else {
            self.orig.get(end - 1).map(|b| b + 1).unwrap_or(last)
        };
        (s, e.max(s))
    }
}

/// トークンのバイト範囲（`idx` は文字オフセット）
fn byte_range(text: &str, t: &TokenData) -> (usize, usize) {
    let mut it = text.char_indices();
    let start = it.by_ref().nth(t.idx).map(|(b, _)| b).unwrap_or(text.len());
    let n = t.text.chars().count();
    let end = if n == 0 {
        start
    } else {
        it.nth(n - 1).map(|(b, _)| b).unwrap_or(text.len())
    };
    (start, end)
}

pub fn words_of(doc: &Doc, text: &str) -> Vec<Word> {
    doc.tokens()
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let (start, end) = byte_range(text, t);
            let (dep, bunsetu_head) = dep_of(t.dep);
            Word {
                i,
                text: t.text.to_string(),
                tag: name_of(TAGS, t.tag),
                dep,
                bunsetu_head,
                head: (i as i64 + t.head as i64).max(0) as usize,
                start,
                end,
            }
        })
        .collect()
}

/// 主語・目的語・述語の三つ組（1 文に複数あってよい: 並列や従属節）
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Frame {
    /// 文の主述語（`ROOT`）か
    pub is_root: bool,
    /// 述語の範囲。「戸別訪問をした」のように「サ変名詞 + する」なら名詞の側
    pub predicate: (usize, usize),
    pub predicate_text: String,
    /// 主語の名詞句（連体修飾を含む部分木）
    pub subject: Option<(usize, usize)>,
    pub subject_text: Option<String>,
    /// 主語の格（「は」「が」）。主述語に主語が無く、従属節の「は」の主語を引き継いだときは "は（主題）"
    pub subject_case: Option<String>,
    pub object: Option<(usize, usize)>,
    pub object_text: Option<String>,
    /// 「に」「に対し」「から」の斜格（相手方）
    pub obliques: Vec<((usize, usize), String, String)>,
}

/// 部分木のバイト範囲（左端から右端まで。連体修飾・複合語を含む。格助詞と読点は除く）
fn subtree(words: &[Word], root: usize) -> (usize, usize) {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); words.len()];
    for w in words {
        if w.head != w.i && w.head < words.len() {
            children[w.head].push(w.i);
        }
    }
    let mut stack = vec![root];
    let mut lo = words[root].start;
    let mut hi = words[root].end;
    let mut seen = vec![false; words.len()];
    while let Some(i) = stack.pop() {
        if seen[i] {
            continue;
        }
        seen[i] = true;
        let w = &words[i];
        if i != root && matches!(w.dep, Some("case") | Some("punct")) {
            continue;
        }
        lo = lo.min(w.start);
        hi = hi.max(w.end);
        stack.extend(children[i].iter().copied());
    }
    (lo, hi)
}

fn case_of(words: &[Word], noun: usize) -> Option<String> {
    let parts: Vec<&str> = words
        .iter()
        .filter(|w| w.head == noun && w.dep == Some("case"))
        .map(|w| w.text.as_str())
        .collect();
    (!parts.is_empty()).then(|| parts.join(""))
}

/// 複合語の範囲（「戸別」+「訪問」）: `compound` で係る語を左に含める
fn compound_span(words: &[Word], i: usize) -> (usize, usize) {
    let mut lo = words[i].start;
    let mut stack = vec![i];
    while let Some(h) = stack.pop() {
        for w in words
            .iter()
            .filter(|w| w.head == h && w.i != h && w.dep == Some("compound"))
        {
            lo = lo.min(w.start);
            stack.push(w.i);
        }
    }
    (lo, words[i].end)
}

fn is_suru(w: &Word) -> bool {
    w.tag == Some("動詞-非自立可能")
        && matches!(w.text.as_str(), "し" | "する" | "す" | "さ" | "せ")
}

/// 述語ごとの主語・目的語。述語は `ROOT` と、それに `advcl` / `acl` / `conj` / `ccomp` で係る動詞・形容詞・サ変名詞
pub fn frames(words: &[Word], text: &str) -> Vec<Frame> {
    let mut out = Vec::new();
    // 主題（「は」の主語）: 主述語に主語が無いとき、文の最初の「は」の主語を引き継ぐ
    // parser が「裁判所は、」を独立した ROOT にすることがある（主題の遊離）ので、ROOT / dislocated の名詞も見る
    let topic = words
        .iter()
        .find(|x| {
            matches!(
                x.dep,
                Some("nsubj") | Some("nsubj:pass") | Some("dislocated") | Some("ROOT")
            ) && x
                .tag
                .is_some_and(|t| t.starts_with("名詞") || t.starts_with("代名詞"))
                && case_of(words, x.i).is_some_and(|c| c.contains('は'))
        })
        .map(|x| x.i);
    // 号の「〜した者」「〜したとき」: ROOT が名詞で、それに acl で係る述語が文の主述語
    let root = words.iter().find(|w| w.dep == Some("ROOT"));
    let root_is_noun = root.is_some_and(|r| {
        r.tag
            .is_some_and(|t| t.starts_with("名詞") && t != "名詞-普通名詞-サ変可能")
    });
    let acl_of_root: Option<usize> = match (root, root_is_noun) {
        (Some(r), true) => words
            .iter()
            .rev()
            .find(|x| x.head == r.i && x.dep == Some("acl"))
            .map(|x| x.i),
        _ => None,
    };
    for w in words {
        let is_pred = matches!(
            w.dep,
            Some("ROOT") | Some("advcl") | Some("conj") | Some("acl") | Some("ccomp")
        ) && w.tag.is_some_and(|t| {
            t.starts_with("動詞") || t.starts_with("形容詞") || t == "名詞-普通名詞-サ変可能"
        });
        if !is_pred {
            continue;
        }
        let is_root = w.dep == Some("ROOT") || acl_of_root == Some(w.i);
        let subj = words.iter().find(|x| {
            x.head == w.i && matches!(x.dep, Some("nsubj") | Some("nsubj:pass") | Some("csubj"))
        });
        let mut obj = words.iter().find(|x| x.head == w.i && x.dep == Some("obj"));
        // 「戸別訪問をした」: する の目的語がサ変名詞なら、それが述語
        let pred_span = match obj {
            Some(o) if is_suru(w) && o.tag == Some("名詞-普通名詞-サ変可能") => {
                obj = None;
                compound_span(words, o.i)
            }
            _ => compound_span(words, w.i),
        };
        let span = |i: usize| subtree(words, i);
        let (s, case) = match subj {
            Some(x) => (Some(span(x.i)), case_of(words, x.i)),
            None if is_root => match topic {
                Some(t) => (Some(span(t)), Some("は（主題）".to_string())),
                None => (None, None),
            },
            None => (None, None),
        };
        let o = obj.map(|x| span(x.i));
        let obliques = words
            .iter()
            .filter(|x| x.head == w.i && x.dep == Some("obl"))
            .filter_map(|x| {
                let c = case_of(words, x.i)?;
                matches!(c.as_str(), "に" | "に対し" | "に対して" | "から" | "へ")
                    .then(|| (span(x.i), c))
            })
            .map(|((a, b), c)| ((a, b), text[a..b].to_string(), c))
            .collect();
        out.push(Frame {
            is_root,
            predicate: pred_span,
            predicate_text: text[pred_span.0..pred_span.1].to_string(),
            subject: s,
            subject_text: s.map(|(a, b)| text[a..b].to_string()),
            subject_case: case,
            object: o,
            object_text: o.map(|(a, b)| text[a..b].to_string()),
            obliques,
        });
    }
    out
}

impl Parser {
    /// 文の主体・客体・行為を候補で。括弧書きは落として解析し、位置は元の本文に戻す。
    /// 文が長すぎる（4000 バイト超）ときは解析せず空
    pub fn candidates(&self, sentence: &StableId, text: &str) -> Result<Vec<Candidate>, NlpError> {
        if text.len() > 4000 || text.trim().is_empty() {
            return Ok(Vec::new());
        }
        let st = Stripped::new(text);
        if st.text.trim().is_empty() {
            return Ok(Vec::new());
        }
        let words = self.words(&st.text)?;
        let mut out = Vec::new();
        for f in frames(&words, &st.text) {
            let conf = if f.is_root {
                Confidence::Medium
            } else {
                Confidence::Low
            };
            let (a, b) = st.to_orig(f.predicate.0, f.predicate.1);
            out.push(
                Candidate::new(
                    Field::Act,
                    sentence,
                    text,
                    a,
                    b,
                    if f.is_root {
                        "ginza:root"
                    } else {
                        "ginza:clause"
                    },
                )
                .confidence(conf),
            );
            if let Some((a, b)) = f.subject {
                let (a, b) = st.to_orig(a, b);
                let mut c = Candidate::new(
                    Field::Subject,
                    sentence,
                    text,
                    a,
                    b,
                    if f.subject_case.as_deref() == Some("は（主題）") {
                        "ginza:topic"
                    } else {
                        "ginza:nsubj"
                    },
                )
                .label(f.predicate_text.clone())
                .confidence(conf);
                if let Some(k) = &f.subject_case {
                    c = c.role(k.clone());
                }
                out.push(c);
            }
            if let Some((a, b)) = f.object {
                let (a, b) = st.to_orig(a, b);
                out.push(
                    Candidate::new(Field::Object, sentence, text, a, b, "ginza:obj")
                        .label(f.predicate_text.clone())
                        .role("を")
                        .confidence(conf),
                );
            }
            for ((a, b), _, case) in &f.obliques {
                let (a, b) = st.to_orig(*a, *b);
                out.push(
                    Candidate::new(Field::Object, sentence, text, a, b, "ginza:obl")
                        .label(f.predicate_text.clone())
                        .role(case.clone())
                        .confidence(conf),
                );
            }
        }
        Ok(out)
    }
}
