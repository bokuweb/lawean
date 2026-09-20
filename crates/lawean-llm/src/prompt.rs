//! system prompt と、項ごとの user message の組み立て。

use lawean_extract::{EffectKind, Skeleton};
use lawean_resolve::Resolution;
use lawean_semantic::Definition;
use lawean_source::{LegalDocument, SentenceFunction};

pub const SYSTEM: &str = r#"あなたは日本の法令を機械可読な中間表現（Legal IR）に変換する補助をします。
入力は法令の 1 つの項（本文・ただし書き・号）と、規則ベースの前処理（層 1）が既に決めた情報です。
あなたの仕事は、層 1 が決めた枠の中で、条件節の中身と効果の引数を構造化することです。

## 守ること
1. 効果の種別（kind）は入力に示された層 1 の種別に従う。`can` は permission（許容）か power（法律関係を変える権限。請求権・解除権・形成権）に振り分ける。層 1 が unknown 系なら自由に判断してよい。
2. 参照（kind=ref）は入力に列挙された stable_id だけを使う。新しい参照を作らない。
3. 定義語は入力の「有効な定義語」にあるものを kind=definition, value=D:ID で使う。無い語は kind=entity で名詞のまま。
4. 「正当の事由」「相当」「遅滞なく」「不利」「やむを得ない事情」のような評価概念は判定せず、unknowns に kind=intentional で入れる。
5. 譲歩（「〜ても」「〜がなくても」「〜の有無にかかわらず」）は条件に入れない。note に書く。
6. 「ただし〜この限りでない」「〜の規定にかかわらず」の上書き関係は層 1 が処理済み。条件だけを書く。
7. 否定（negated=true）は原文に「〜でない」「〜しなかった」「〜を除き」がある場合だけ。例外を否定条件に書き換えない。
8. 括弧書きの場合分け（「（最初の更新にあっては、二十年）」）や増額/減額のような対称は、suffix を付けて別 Rule にする。
9. 期間は kind=duration で '30 year' / '6 month' / '2 year' の形。起算点は temporal に。金額は kind=money。数値が原文に無いなら kind=var で変数名。
10. confidence は medium が上限。迷ったら low。判断の根拠や捨てた情報は note に。
11. 文ごとに最低 1 つの Rule を返す。構造化できない文は condition を空、effect.kind を層 1 の種別、unknowns に kind=unparsed で原文を入れる。

## 述語の書き方
name は原文の動詞句を短く正規化する（「借地権者が契約の更新を請求したときは」→ name「更新を請求した」, args by=D:借地権者）。
主語・目的語・相手方は args に key=by / object / to で。時間条件は key=when, kind=text か temporal で。
"#;

pub struct ParagraphInput<'a> {
    pub paragraph: &'a str,
    pub heading: String,
    pub skeletons: Vec<&'a Skeleton>,
    pub definitions: Vec<&'a Definition>,
}

impl<'a> ParagraphInput<'a> {
    pub fn new(
        doc: &'a LegalDocument,
        skeletons: &'a [Skeleton],
        definitions: &'a [Definition],
        paragraph: &'a str,
    ) -> Self {
        let sk: Vec<&Skeleton> = skeletons
            .iter()
            .filter(|s| s.paragraph.0 == paragraph)
            .collect();
        let defs = definitions
            .iter()
            .filter(|d| {
                d.scope.iter().any(|s| {
                    lawean_resolve::model::is_under(&lawean_source::StableId(paragraph.into()), s)
                })
            })
            .collect();
        let title = doc
            .title
            .as_ref()
            .map(|t| lawean_source::inline_text(&t.text))
            .unwrap_or_default();
        ParagraphInput {
            paragraph,
            heading: format!("{title} {paragraph}"),
            skeletons: sk,
            definitions: defs,
        }
    }

    /// 文ラベル（`sent:1`、号なら `item:2/sent:1`）
    pub fn label(&self, s: &Skeleton) -> String {
        s.sentence
            .0
            .trim_start_matches(self.paragraph)
            .trim_start_matches('/')
            .to_string()
    }

    pub fn user_message(&self) -> String {
        let mut m = String::new();
        m.push_str(&format!("## 位置\n{}\n\n## 文\n", self.heading));
        for s in &self.skeletons {
            let f = match s.function {
                SentenceFunction::Main => "Main",
                SentenceFunction::Proviso => "Proviso",
                SentenceFunction::Unspecified => "-",
            };
            m.push_str(&format!(
                "[{}] ({f}, {}) {}\n",
                self.label(s),
                effect_label(s.effect),
                s.text
            ));
            if !s.conditions.is_empty() {
                let c: Vec<&str> = s.conditions.iter().map(|c| c.text.as_str()).collect();
                m.push_str(&format!("  条件節: {}\n", c.join(" / ")));
            }
            let refs: Vec<String> = s
                .references
                .iter()
                .filter_map(|(t, r)| match r {
                    Resolution::Internal(ids) => Some(format!(
                        "{t} → {}",
                        ids.iter()
                            .map(|i| i.0.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                    Resolution::External { law, num, .. } => {
                        Some(format!("{t} → 他法令 {law} 第{}条", num.to_num_string()))
                    }
                    Resolution::Unresolved(_) => None,
                })
                .collect();
            if !refs.is_empty() {
                m.push_str(&format!("  参照: {}\n", refs.join(" ; ")));
            }
            if !s.concessions.is_empty() {
                let c: Vec<&str> = s.concessions.iter().map(|c| c.text.as_str()).collect();
                m.push_str(&format!("  譲歩（条件に入れない）: {}\n", c.join(" / ")));
            }
        }
        m.push_str("\n## この位置で有効な定義語\n");
        if self.definitions.is_empty() {
            m.push_str("（なし）\n");
        } else {
            let d: Vec<String> = self
                .definitions
                .iter()
                .map(|d| format!("{} {}", d.id.0, d.term))
                .collect();
            m.push_str(&d.join(" / "));
            m.push('\n');
        }
        m
    }
}

pub fn effect_label(k: EffectKind) -> &'static str {
    match k {
        EffectKind::Obligation => "obligation",
        EffectKind::Prohibition | EffectKind::CanNot => "prohibition",
        EffectKind::Can => "can",
        EffectKind::Deem => "deem",
        EffectKind::Presume => "presume",
        EffectKind::Exception => "exception",
        EffectKind::NotApply => "not_apply",
        EffectKind::Apply => "apply",
        EffectKind::ApplyMutatis => "apply_mutatis",
        EffectKind::DeemAndApply => "deem_and_apply",
        EffectKind::Void => "void",
        EffectKind::FormerExample => "former_example",
        EffectKind::Preserve => "preserve",
        EffectKind::Lapse => "lapse",
        EffectKind::Suffice => "suffice",
        EffectKind::Follow => "follow",
        EffectKind::Limit => "limit",
        EffectKind::Set => "set",
        EffectKind::Enforce
        | EffectKind::Definition
        | EffectKind::Repeal
        | EffectKind::Fragment => "unknown",
    }
}

/// 層 1 の種別に対して LLM が返してよい kind
pub fn allowed_kinds(k: EffectKind) -> &'static [&'static str] {
    match k {
        EffectKind::Can => &["permission", "power"],
        EffectKind::Prohibition | EffectKind::CanNot => &["prohibition"],
        EffectKind::Obligation => &["obligation"],
        EffectKind::Deem => &["deem"],
        EffectKind::Presume => &["presume"],
        EffectKind::Exception => &["exception"],
        EffectKind::NotApply => &["not_apply"],
        EffectKind::Apply => &["apply"],
        EffectKind::ApplyMutatis => &["apply_mutatis"],
        EffectKind::DeemAndApply => &["deem_and_apply"],
        EffectKind::Void => &["void"],
        EffectKind::FormerExample => &["former_example"],
        EffectKind::Preserve => &["preserve"],
        EffectKind::Lapse => &["lapse"],
        EffectKind::Suffice => &["suffice"],
        EffectKind::Follow => &["follow"],
        EffectKind::Limit => &["limit"],
        EffectKind::Set => &["set"],
        EffectKind::Enforce
        | EffectKind::Definition
        | EffectKind::Repeal
        | EffectKind::Fragment => &[
            "obligation",
            "prohibition",
            "permission",
            "power",
            "set",
            "deem",
            "presume",
            "void",
            "exception",
            "not_apply",
            "apply",
            "apply_mutatis",
            "deem_and_apply",
            "former_example",
            "preserve",
            "lapse",
            "suffice",
            "follow",
            "limit",
            "unknown",
        ],
    }
}
