//! Source IR の型。docs/02-source-ir.md の写像表に対応する。
//! ここでは意味解析をしない。XML の構造を型に写すだけ。

use crate::xml::Element;

/// 法令内で構造ノードを一意に指すパス。例: `main/art:3/para:1/sent:2`
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StableId(pub String);

impl StableId {
    pub fn child(&self, seg: impl AsRef<str>) -> StableId {
        StableId(format!("{}/{}", self.0, seg.as_ref()))
    }
}

impl std::fmt::Display for StableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `Article @Num`。`3` / `121_2`（第百二十一条の二）/ `155:157`（第百五十五条から第百五十七条まで 削除）
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArticleNum {
    Single {
        base: u32,
        branch: Vec<u32>,
    },
    Range {
        from: Box<ArticleNum>,
        to: Box<ArticleNum>,
    },
    /// 上のどれにも解釈できなかった表記。lossless のため原文を保持
    Other(String),
}

impl ArticleNum {
    pub fn parse(s: &str) -> ArticleNum {
        if let Some((a, b)) = s.split_once(':') {
            return ArticleNum::Range {
                from: Box::new(Self::parse(a)),
                to: Box::new(Self::parse(b)),
            };
        }
        let mut parts = s.split('_');
        let base = parts.next().and_then(|p| p.parse().ok());
        let branch: Option<Vec<u32>> = parts.map(|p| p.parse().ok()).collect();
        match (base, branch) {
            (Some(base), Some(branch)) => ArticleNum::Single { base, branch },
            _ => ArticleNum::Other(s.to_string()),
        }
    }

    /// XML の `Num` 属性表記に戻す
    pub fn to_num_string(&self) -> String {
        match self {
            ArticleNum::Single { base, branch } => {
                let mut s = base.to_string();
                for b in branch {
                    s.push('_');
                    s.push_str(&b.to_string());
                }
                s
            }
            ArticleNum::Range { from, to } => {
                format!("{}:{}", from.to_num_string(), to.to_num_string())
            }
            ArticleNum::Other(s) => s.clone(),
        }
    }
}

impl LegalDocument {
    /// 文書内の全構造ノードの stable_id（文書自身・本則・附則・編章節条項号文・欄）
    pub fn stable_ids(&self) -> Vec<&StableId> {
        let mut out = vec![&self.stable_id];
        fn sents<'a>(ss: &'a [Sentence], out: &mut Vec<&'a StableId>) {
            out.extend(ss.iter().map(|s| &s.stable_id));
        }
        fn item<'a>(i: &'a Item, out: &mut Vec<&'a StableId>) {
            out.push(&i.stable_id);
            match &i.body {
                ItemBody::Sentences(ss) => sents(ss, out),
                ItemBody::Columns(cs) => {
                    for c in cs {
                        out.push(&c.stable_id);
                        sents(&c.sentences, out);
                    }
                }
                ItemBody::Mixed(_) | ItemBody::None => {}
            }
            for c in &i.children {
                if let ItemChild::Subitem(s) = c {
                    item(s, out);
                }
            }
        }
        fn para<'a>(p: &'a Paragraph, out: &mut Vec<&'a StableId>) {
            out.push(&p.stable_id);
            sents(&p.sentences, out);
            for c in &p.children {
                if let ParagraphChild::Item(i) = c {
                    item(i, out);
                }
            }
        }
        fn prov<'a>(p: &'a Provision, out: &mut Vec<&'a StableId>) {
            match p {
                Provision::Container(c) => {
                    out.push(&c.stable_id);
                    c.children.iter().for_each(|c| prov(c, out));
                }
                Provision::Article(a) => {
                    out.push(&a.stable_id);
                    for c in &a.children {
                        if let ArticleChild::Paragraph(p) = c {
                            para(p, out);
                        }
                    }
                }
                Provision::Paragraph(p) => para(p, out),
                Provision::Raw(_) => {}
            }
        }
        self.main_provision.iter().for_each(|p| prov(p, &mut out));
        for s in &self.suppl_provisions {
            out.push(&s.stable_id);
            for c in &s.children {
                match c {
                    SupplChild::Provision(p) => prov(p, &mut out),
                    SupplChild::Paragraph(p) => para(p, &mut out),
                    SupplChild::Raw(_) => {}
                }
            }
        }
        out
    }
}

/// 項単位にまとめた文（号・欄の文も含む）。抽出・参照解決の入力に使う
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentenceGroup<'a> {
    /// 項（附則直下の項も含む）の stable_id
    pub paragraph: &'a StableId,
    pub sentences: Vec<SentenceRef<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentenceRef<'a> {
    pub sentence: &'a Sentence,
    /// 号（イロハ含む）の中の文
    pub in_item: bool,
    /// 定義欄（Column）の中の文
    pub in_column: bool,
}

impl LegalDocument {
    /// 本則・附則の全文を項ごとに（文書順で）返す
    pub fn sentence_groups(&self) -> Vec<SentenceGroup<'_>> {
        let mut out = Vec::new();
        fn item<'a>(i: &'a Item, g: &mut SentenceGroup<'a>) {
            match &i.body {
                ItemBody::Sentences(ss) => g.sentences.extend(ss.iter().map(|s| SentenceRef {
                    sentence: s,
                    in_item: true,
                    in_column: false,
                })),
                ItemBody::Columns(cs) => {
                    for c in cs {
                        g.sentences.extend(c.sentences.iter().map(|s| SentenceRef {
                            sentence: s,
                            in_item: true,
                            in_column: true,
                        }));
                    }
                }
                ItemBody::Mixed(_) | ItemBody::None => {}
            }
            for c in &i.children {
                if let ItemChild::Subitem(s) = c {
                    item(s, g);
                }
            }
        }
        fn para<'a>(p: &'a Paragraph, out: &mut Vec<SentenceGroup<'a>>) {
            let mut g = SentenceGroup {
                paragraph: &p.stable_id,
                sentences: Vec::new(),
            };
            g.sentences.extend(p.sentences.iter().map(|s| SentenceRef {
                sentence: s,
                in_item: false,
                in_column: false,
            }));
            for c in &p.children {
                if let ParagraphChild::Item(i) = c {
                    item(i, &mut g);
                }
            }
            out.push(g);
        }
        fn prov<'a>(p: &'a Provision, out: &mut Vec<SentenceGroup<'a>>) {
            match p {
                Provision::Container(c) => c.children.iter().for_each(|c| prov(c, out)),
                Provision::Article(a) => {
                    for c in &a.children {
                        if let ArticleChild::Paragraph(p) = c {
                            para(p, out);
                        }
                    }
                }
                Provision::Paragraph(p) => para(p, out),
                Provision::Raw(_) => {}
            }
        }
        self.main_provision.iter().for_each(|p| prov(p, &mut out));
        for s in &self.suppl_provisions {
            for c in &s.children {
                match c {
                    SupplChild::Provision(p) => prov(p, &mut out),
                    SupplChild::Paragraph(p) => para(p, &mut out),
                    SupplChild::Raw(_) => {}
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalDocument {
    pub stable_id: StableId,
    /// e-Gov 法令 ID。`403AC0000000090`
    pub law_id: Option<String>,
    /// e-Gov `law_revision_id`。`403AC0000000090_20260521_504AC0000000048`
    pub version_id: Option<String>,
    /// `Law` 要素の属性（Era / Year / Num / LawType / Lang / Promulgate*）。順序込みで保持
    pub law_attrs: Vec<(String, String)>,
    /// `LawNum` の本文。「平成三年法律第九十号」
    pub law_num: Option<String>,
    pub title: Option<LawTitle>,
    pub enact_statements: Vec<Element>,
    pub toc: Option<Element>,
    pub preamble: Option<Element>,
    pub main_provision: Vec<Provision>,
    pub suppl_provisions: Vec<SupplProvision>,
    /// AppdxTable / AppdxNote / AppdxStyle / AppdxFormat / AppdxFig / Appdx。v0.1 では raw
    pub appendices: Vec<Element>,
    /// `LawBody` 直下の順序を復元するための並び。lossless 用
    pub body_order: Vec<BodySlot>,
    /// `Law` 直下で LawNum / LawBody 以外の要素（現状は無いはず）
    pub law_extra: Vec<Element>,
    /// `MainProvision` の属性（`Extract` など）
    pub main_attrs: Vec<(String, String)>,
}

/// `LawBody` の子要素の並び
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodySlot {
    Title,
    EnactStatement(usize),
    Toc,
    Preamble,
    MainProvision,
    SupplProvision(usize),
    Appendix(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawTitle {
    pub text: Vec<Inline>,
    pub attrs: Vec<(String, String)>,
}

/// 編・章・節・款・目・条
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provision {
    Container(Container),
    Article(Article),
    /// 条を持たない政令・省令の本則に直に並ぶ項（`MainProvision/Paragraph`）
    Paragraph(Paragraph),
    /// 予期しない要素。lossless 用
    Raw(Element),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerKind {
    Part,
    Chapter,
    Section,
    Subsection,
    Division,
}

impl ContainerKind {
    pub fn element_name(self) -> &'static str {
        match self {
            ContainerKind::Part => "Part",
            ContainerKind::Chapter => "Chapter",
            ContainerKind::Section => "Section",
            ContainerKind::Subsection => "Subsection",
            ContainerKind::Division => "Division",
        }
    }

    pub fn id_prefix(self) -> &'static str {
        match self {
            ContainerKind::Part => "part",
            ContainerKind::Chapter => "chap",
            ContainerKind::Section => "sec",
            ContainerKind::Subsection => "subsec",
            ContainerKind::Division => "div",
        }
    }

    pub fn from_element_name(name: &str) -> Option<Self> {
        Some(match name {
            "Part" => ContainerKind::Part,
            "Chapter" => ContainerKind::Chapter,
            "Section" => ContainerKind::Section,
            "Subsection" => ContainerKind::Subsection,
            "Division" => ContainerKind::Division,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    pub stable_id: StableId,
    pub kind: ContainerKind,
    pub num: Option<String>,
    pub title: Option<Vec<Inline>>,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Provision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article {
    pub stable_id: StableId,
    pub num: ArticleNum,
    /// 「（借地権の存続期間）」。括弧込み。無いことがある
    pub caption: Option<Vec<Inline>>,
    /// 「第三条」
    pub title: Option<Vec<Inline>>,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<ArticleChild>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArticleChild {
    Paragraph(Paragraph),
    /// SupplNote など
    Raw(Element),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paragraph {
    pub stable_id: StableId,
    pub num: String,
    pub caption: Option<Vec<Inline>>,
    /// `ParagraphNum` の本文。第1項は空、第2項以降は「２」
    pub num_text: Option<Vec<Inline>>,
    /// `ParagraphSentence / Sentence*`
    pub sentences: Vec<Sentence>,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<ParagraphChild>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParagraphChild {
    Item(Item),
    /// TableStruct / FigStruct / List / AmendProvision など
    Raw(Element),
}

/// 号（Item）およびイロハ（Subitem1..10）。`depth` 0 が Item
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub stable_id: StableId,
    pub depth: u8,
    pub num: Option<String>,
    /// 「一」「イ」
    pub title: Option<Vec<Inline>>,
    pub body: ItemBody,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<ItemChild>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemBody {
    Sentences(Vec<Sentence>),
    /// 定義規定など。`Column1 = 用語 / Column2 = 定義`
    Columns(Vec<Column>),
    /// Sentence と Column 以外が混じる。lossless 用に要素ごと保持
    Mixed(Element),
    /// ItemSentence / SubitemNSentence が無い
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemChild {
    Subitem(Item),
    Raw(Element),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub stable_id: StableId,
    pub num: Option<String>,
    pub sentences: Vec<Sentence>,
    pub attrs: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SentenceFunction {
    /// `Function="main"` 本文
    Main,
    /// `Function="proviso"` ただし書き
    Proviso,
    /// 属性なし。1 文だけの項など。`Main` に潰さない
    Unspecified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sentence {
    pub stable_id: StableId,
    pub num: Option<String>,
    pub function: SentenceFunction,
    pub text: Vec<Inline>,
    /// Num / Function 以外の属性（WritingMode など）
    pub attrs: Vec<(String, String)>,
}

impl Sentence {
    pub fn plain_text(&self) -> String {
        inline_text(&self.text)
    }
}

/// 文中の内容。v0.1 では Ruby 等を型付けせず raw で保持する
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Raw(Element),
}

pub fn inline_text(inl: &[Inline]) -> String {
    let mut s = String::new();
    for i in inl {
        match i {
            Inline::Text(t) => s.push_str(t),
            // Ruby はふりがな（Rt）を落として本体だけ（「禁錮」。e-Gov が後の改正でルビを外しても本文は同じ）
            Inline::Raw(e) if e.name == "Ruby" => s.push_str(&ruby_base(e)),
            Inline::Raw(e) => s.push_str(&e.text()),
        }
    }
    s
}

fn ruby_base(e: &crate::xml::Element) -> String {
    let mut s = String::new();
    for c in &e.children {
        match c {
            crate::xml::Node::Text(t) => s.push_str(t),
            crate::xml::Node::Element(x) if x.name == "Rt" => {}
            crate::xml::Node::Element(x) => s.push_str(&x.text()),
        }
    }
    s
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplProvision {
    pub stable_id: StableId,
    /// 無ければ原始附則
    pub amend_law_num: Option<String>,
    /// 「抄」
    pub extract: bool,
    pub label: Option<Vec<Inline>>,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<SupplChild>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupplChild {
    Provision(Provision),
    Paragraph(Paragraph),
    /// SupplProvisionAppdxTable など
    Raw(Element),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_num_forms() {
        assert_eq!(
            ArticleNum::parse("3"),
            ArticleNum::Single {
                base: 3,
                branch: vec![]
            }
        );
        assert_eq!(
            ArticleNum::parse("121_2"),
            ArticleNum::Single {
                base: 121,
                branch: vec![2]
            }
        );
        assert_eq!(ArticleNum::parse("155:157").to_num_string(), "155:157");
        assert_eq!(ArticleNum::parse("22_2_3").to_num_string(), "22_2_3");
        assert!(matches!(ArticleNum::parse("x"), ArticleNum::Other(_)));
    }
}
