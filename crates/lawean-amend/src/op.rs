//! 改め文の操作（docs/08-amendment.md §3）。

use lawean_source::ArticleNum;

/// 項の指し方。改め文の 1 文の中では、繰り下げ等の前の番号（改正前）で指す
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParaRef {
    Num(u32),
}

/// 改正対象の位置
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loc {
    pub article: ArticleNum,
    pub paragraph: Option<ParaRef>,
    /// 号（「第三号の二」= `3_2`）。あれば字句の置換をその号（とその下の号）に限る
    pub item: Option<String>,
    /// 「ただし書」「本文」「前段」「後段」「各号列記以外の部分」。あれば字句の置換をその文に限る
    pub part: Option<SentencePart>,
    /// 「附則第七条第六項」: 原始附則の条（本則ではない。id の世界には載せず、文書の側だけ改める）
    pub suppl: bool,
    /// 号の下のイロハ（「同号ロ」= `ロ`）。あれば字句の置換をその細目に限る
    pub sub: Option<String>,
}

impl Loc {
    pub fn new(article: ArticleNum, paragraph: Option<ParaRef>) -> Self {
        Loc {
            article,
            paragraph,
            item: None,
            part: None,
            suppl: false,
            sub: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// 「目次中「A」を「B」に改める」
    ReplaceToc { from: String, to: String },
    /// 「第N条[第M項]中「A」を「B」に改める」— 対象内の全出現
    Replace { at: Loc, from: String, to: String },
    /// 「第N条[第M項]中「A」の下に「B」を加える」
    InsertAfterPhrase {
        at: Loc,
        anchor: String,
        text: String,
    },
    /// 「第N条に次の一項を加える」
    AppendParagraph {
        article: ArticleNum,
        text: Vec<String>,
    },
    /// 「第N条第M項の次に次の一項を加える」「同項の次に次の一項を加える」
    InsertParagraphAfter {
        article: ArticleNum,
        after: ParaRef,
        text: Vec<String>,
    },
    /// 「同条に第一項として次の一項を加える」: 条の先頭に項を置く（「同条を同条第二項とし」の後）
    InsertParagraphFirst {
        article: ArticleNum,
        text: Vec<String>,
    },
    /// 「第N章に次の一条を加える」「第一章第八節に次の七条を加える」。`path` は外側から (章/節/款/目, 番号)
    AppendArticle {
        path: Vec<(lawean_source::ContainerKind, String)>,
        text: Vec<String>,
    },
    /// 「附則に次の二条を加える」「附則に次の見出し及び二条を加える」+ 条の行: 原始附則の末尾に条を足す（文書の側だけ）
    AppendSupplArticles { text: Vec<String> },
    /// 「本則に次の一章を加える」「第二編第二章に次の一節を加える」+ 章・節の行: 本則（`path` が空）または容器の末尾に容器を足す
    AppendContainers {
        path: Vec<(lawean_source::ContainerKind, String)>,
        text: Vec<String>,
    },
    /// 「第一章第五節中第十七条の前に次の三条を加える」+ 条の行
    InsertArticleBefore {
        before: ArticleNum,
        text: Vec<String>,
        suppl: bool,
    },
    /// 「第三十八条の表第七十条第二項の項中「A」を「B」に改め」: 条・項の中の表（読替え表）の行（上欄が `row`）の字句
    ReplaceTableRow {
        at: Loc,
        row: String,
        from: String,
        to: String,
    },
    /// 「同項に次の表を加える」+ 欄の行: 項の末尾に表を足す（欄は「第」で始まる行から行を組む）
    AppendTable { at: Loc, text: Vec<String> },
    /// 「別表第一及び別表第二を削る」
    DeleteAppdx { tables: Vec<String> },
    /// 「同項後段を削る」「同項ただし書を削る」
    DeleteSentencePart { at: Loc, part: SentencePart },
    /// 「第一章第八節の節名中「A」を「B」に改める」「第N章の章名中…」。`path` は外側から (章/節/款/目, 番号)
    ReplaceContainerTitle {
        path: Vec<(lawean_source::ContainerKind, String)>,
        from: String,
        to: String,
    },
    /// 「第N章の次に次の一章を加える」「第一章中第五節の次に次の二節を加える」+ 内容
    /// （「第M章　題名」「第一節　…」「（見出し）」「第K条　本文」…）。`path` は外側から (章/節/款/目, 番号)
    InsertContainersAfter {
        path: Vec<(lawean_source::ContainerKind, String)>,
        text: Vec<String>,
    },
    /// 「同節の前に次の一節を加える」+ 内容
    InsertContainersBefore {
        path: Vec<(lawean_source::ContainerKind, String)>,
        text: Vec<String>,
    },
    /// 「第N章を第M章とする」「第一章中第八節を第十節とする」。番号は「2」「2_2」（第二章の二）
    RenumberContainer {
        path: Vec<(lawean_source::ContainerKind, String)>,
        to: String,
    },
    /// 「第N条の次に次の一条を加える」
    InsertArticleAfter {
        after: ArticleNum,
        text: Vec<String>,
        /// 「附則第一条の次に次の一条を加える」: 原始附則の条（文書の側だけ）
        suppl: bool,
    },
    /// 「第N条第M項（各号列記以外の部分）に後段として次のように加える」— 項の文の末尾に文を足す
    AppendSentence { at: Loc, text: Vec<String> },
    /// 「第N条を次のように改める」
    ReplaceArticle {
        article: ArticleNum,
        text: Vec<String>,
    },
    /// 「第N条第M項を次のように改める」+ 本文（号を含んでよい）。項の本文の全部の差し替え
    ReplaceParagraph { at: Loc, text: Vec<String> },
    /// 「同項第三号を次のように改める」+「三　本文」（イロハを含んでよい）。号の全部の差し替え
    ReplaceItem { at: Loc, text: Vec<String> },
    /// 「題名を次のように改める」+ 題名の行
    SetTitle { text: Vec<String> },
    /// 「題名の次に次の目次を付する」+ 目次の行（「目次」「第一章　総則（第一条）」…「附則」）。目次の無い法律に目次を足す
    SetToc { text: Vec<String> },
    /// 「第三章の章名を削る」: 題名の無くなった章は前の章に併合される（中の条は前の章の末尾に）
    DeleteContainerTitle {
        path: Vec<(lawean_source::ContainerKind, String)>,
    },
    /// 「第三章第二節から第五節までを削る」
    DeleteContainers {
        path: Vec<(lawean_source::ContainerKind, String)>,
        kind: lawean_source::ContainerKind,
        from: u32,
        to: u32,
    },
    /// 「第百二条及び第百三条を次のように改める」+「第百二条及び第百三条　削除」: 複数の条を 1 つの「削除」の条に
    ReplaceArticles {
        articles: Vec<ArticleNum>,
        text: Vec<String>,
    },
    /// 「別表第二X法（…）の項中「A」を「B」に改める」: 別表の行（上欄が `row` で始まる行）の字句。
    /// 「の下に「B」を加える」は `to = A + B`
    ReplaceAppdxRow {
        table: String,
        row: String,
        from: String,
        to: String,
    },
    /// 「第四章及び第五章を次のように改める」+ 章の内容
    ReplaceContainers {
        paths: Vec<Vec<(lawean_source::ContainerKind, String)>>,
        text: Vec<String>,
    },
    /// 「第二章の章名を次のように改める」+「第二章　題名」の行
    SetContainerTitle {
        path: Vec<(lawean_source::ContainerKind, String)>,
        text: Vec<String>,
    },
    /// 「同項中第二十一号を第三十七号とし」「第四号を同条第七号とし」（`at` は条・項）。号の番号は「3」「3_2」
    RenumberItem { at: Loc, from: String, to: String },
    /// 「第十二号から第二十号までを十六号ずつ繰り下げ」
    ShiftItems {
        at: Loc,
        from: u32,
        to: u32,
        by: i32,
    },
    /// 「同項第一号の次に次の一号を加える」「同号の次に次の五号を加える」+「二　本文」…
    InsertItemAfter {
        at: Loc,
        after: String,
        text: Vec<String>,
    },
    /// 「同号の前に次の一号を加える」
    InsertItemBefore {
        at: Loc,
        before: String,
        text: Vec<String>,
    },
    /// 「同項に次の一号を加える」
    AppendItem { at: Loc, text: Vec<String> },
    /// 「同項各号を次のように改める」+ 号の行（全部の号の差し替え）
    ReplaceItems { at: Loc, text: Vec<String> },
    /// 「同号ロを同号ハとし」「同号中ヘをトとし」: 号の下のイロハの番号の付け替え（`at` は号）
    RenumberSubitem { at: Loc, from: String, to: String },
    /// 「ハからホまでをニからヘまでとし」: イロハの範囲を `by` だけずらす
    ShiftSubitems {
        at: Loc,
        from: String,
        to: String,
        by: i32,
    },
    /// 「同号イの次に次のように加える」+「ロ　本文」: イロハの挿入
    InsertSubitemAfter {
        at: Loc,
        after: String,
        text: Vec<String>,
    },
    /// 「第N条第M項第A号及び第B号を次のように改める」「第A号から第C号までを次のように改める」+ 号の行: 挙げた号だけの差し替え。
    /// `items` は号の番号（「3_2」）、`range` は「から…まで」の形（描画用）
    ReplaceItemSet {
        at: Loc,
        items: Vec<String>,
        range: bool,
        text: Vec<String>,
    },
    /// 「第N条中第P項を第Q項とし」「同項を同条第D項とし」
    RenumberParagraph {
        article: ArticleNum,
        from: ParaRef,
        to: u32,
    },
    /// 「第A項から第B項までをK項ずつ繰り下げ」
    ShiftParagraphs {
        article: ArticleNum,
        from: u32,
        to: u32,
        by: i32,
    },
    /// 「第N条[第M項]を削る」
    Delete { at: Loc },
    /// 「第N条を第M条とする」「同条を第M条とし」— 条ずれ。本文は触らない
    RenumberArticle {
        from: ArticleNum,
        to: ArticleNum,
        /// 「同条を附則第一条の三とし」: 原始附則の条
        suppl: bool,
    },
    /// 「第N条から第M条までをK条ずつ繰り下げ」— 範囲の条（枝番も含む）の基数を K 動かす
    ShiftArticles { from: u32, to: u32, by: i32 },
    /// 「第N条の見出し中「A」を「B」に改め」
    ReplaceCaption {
        article: ArticleNum,
        from: String,
        to: String,
    },
    /// 「第N条の見出しを「（X）」に改め」
    SetCaption { article: ArticleNum, text: String },
    /// 「第N条の前に見出しとして「（X）」を付し」: 見出しの無い条に見出しを付ける（結果は `SetCaption` と同じ。字面が違う）
    AttachCaption { article: ArticleNum, text: String },
    /// 「第N条の見出しを削り」「第N条の前の見出しを削り」
    DeleteCaption { article: ArticleNum },
    /// 「第N条[第M項]後段を次のように改める」— 項の本文の前段／後段を差し替える。続く行のうち最初の 1 行が文で、
    /// 残り（読替え表の欄など）は文には入れない
    ReplaceSentencePart {
        at: Loc,
        part: SentencePart,
        text: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SentencePart {
    Front,
    Back,
    /// ただし書
    Proviso,
    /// 本文（ただし書を除く文）
    Main,
    /// 各号列記以外の部分（項の文。号を除く）
    Chapeau,
}

impl Op {
    /// この操作が触る被改正法の条（目次・章末への追加は None）
    pub fn article(&self) -> Option<&ArticleNum> {
        match self {
            Op::ReplaceToc { .. }
            | Op::SetToc { .. }
            | Op::AppendSupplArticles { .. }
            | Op::AppendContainers { .. }
            | Op::AppendArticle { .. }
            | Op::InsertContainersAfter { .. }
            | Op::InsertContainersBefore { .. }
            | Op::RenumberContainer { .. }
            | Op::ReplaceContainerTitle { .. }
            | Op::SetTitle { .. }
            | Op::SetContainerTitle { .. }
            | Op::DeleteContainerTitle { .. }
            | Op::DeleteContainers { .. }
            | Op::ReplaceContainers { .. }
            | Op::ReplaceAppdxRow { .. }
            | Op::DeleteAppdx { .. } => None,
            Op::ReplaceArticles { articles, .. } => articles.first(),
            Op::Replace { at, .. }
            | Op::InsertAfterPhrase { at, .. }
            | Op::AppendSentence { at, .. }
            | Op::Delete { at } => Some(&at.article),
            Op::AppendParagraph { article, .. }
            | Op::InsertParagraphAfter { article, .. }
            | Op::InsertParagraphFirst { article, .. }
            | Op::ReplaceArticle { article, .. }
            | Op::RenumberParagraph { article, .. }
            | Op::ShiftParagraphs { article, .. } => Some(article),
            Op::InsertArticleAfter { after, .. } => Some(after),
            Op::InsertArticleBefore { before, .. } => Some(before),
            Op::RenumberArticle { from, .. } => Some(from),
            Op::ShiftArticles { .. } => None,
            Op::ReplaceCaption { article, .. }
            | Op::SetCaption { article, .. }
            | Op::AttachCaption { article, .. }
            | Op::DeleteCaption { article } => Some(article),
            Op::ReplaceSentencePart { at, .. }
            | Op::DeleteSentencePart { at, .. }
            | Op::ReplaceParagraph { at, .. }
            | Op::ReplaceItem { at, .. }
            | Op::RenumberItem { at, .. }
            | Op::ShiftItems { at, .. }
            | Op::InsertItemAfter { at, .. }
            | Op::InsertItemBefore { at, .. }
            | Op::AppendItem { at, .. }
            | Op::ReplaceItems { at, .. }
            | Op::ReplaceItemSet { at, .. }
            | Op::ReplaceTableRow { at, .. }
            | Op::AppendTable { at, .. }
            | Op::RenumberSubitem { at, .. }
            | Op::ShiftSubitems { at, .. }
            | Op::InsertSubitemAfter { at, .. } => Some(&at.article),
        }
    }

    /// この操作が触る項（位置に項があるもの）
    pub fn paragraph(&self) -> Option<u32> {
        match self {
            Op::Replace { at, .. }
            | Op::InsertAfterPhrase { at, .. }
            | Op::AppendSentence { at, .. }
            | Op::Delete { at }
            | Op::ReplaceSentencePart { at, .. }
            | Op::DeleteSentencePart { at, .. }
            | Op::ReplaceParagraph { at, .. }
            | Op::ReplaceItem { at, .. }
            | Op::RenumberItem { at, .. }
            | Op::ShiftItems { at, .. }
            | Op::InsertItemAfter { at, .. }
            | Op::InsertItemBefore { at, .. }
            | Op::AppendItem { at, .. }
            | Op::ReplaceItems { at, .. }
            | Op::ReplaceItemSet { at, .. }
            | Op::ReplaceTableRow { at, .. }
            | Op::AppendTable { at, .. }
            | Op::RenumberSubitem { at, .. }
            | Op::ShiftSubitems { at, .. }
            | Op::InsertSubitemAfter { at, .. } => match at.paragraph {
                Some(ParaRef::Num(n)) => Some(n),
                _ => None,
            },
            _ => None,
        }
    }

    /// 続く条文（インデント 1 の行）を受け取る操作か
    pub fn takes_content(&self) -> bool {
        matches!(
            self,
            Op::AppendParagraph { .. }
                | Op::InsertParagraphAfter { .. }
                | Op::InsertParagraphFirst { .. }
                | Op::AppendArticle { .. }
                | Op::InsertContainersAfter { .. }
                | Op::InsertContainersBefore { .. }
                | Op::InsertArticleAfter { .. }
                | Op::AppendSentence { .. }
                | Op::ReplaceArticle { .. }
                | Op::ReplaceParagraph { .. }
                | Op::ReplaceItem { .. }
                | Op::InsertItemAfter { .. }
                | Op::InsertItemBefore { .. }
                | Op::AppendItem { .. }
                | Op::ReplaceItems { .. }
                | Op::ReplaceItemSet { .. }
                | Op::InsertSubitemAfter { .. }
                | Op::AppendSupplArticles { .. }
                | Op::AppendContainers { .. }
                | Op::InsertArticleBefore { .. }
                | Op::AppendTable { .. }
                | Op::SetTitle { .. }
                | Op::SetToc { .. }
                | Op::SetContainerTitle { .. }
                | Op::ReplaceArticles { .. }
                | Op::ReplaceContainers { .. }
                | Op::ReplaceSentencePart { .. }
        )
    }

    pub fn push_content(&mut self, line: String) {
        match self {
            Op::AppendParagraph { text, .. }
            | Op::InsertParagraphAfter { text, .. }
            | Op::InsertParagraphFirst { text, .. }
            | Op::AppendArticle { text, .. }
            | Op::InsertContainersAfter { text, .. }
            | Op::InsertContainersBefore { text, .. }
            | Op::InsertArticleAfter { text, .. }
            | Op::AppendSentence { text, .. }
            | Op::ReplaceArticle { text, .. }
            | Op::ReplaceParagraph { text, .. }
            | Op::ReplaceItem { text, .. }
            | Op::InsertItemAfter { text, .. }
            | Op::InsertItemBefore { text, .. }
            | Op::AppendItem { text, .. }
            | Op::ReplaceItems { text, .. }
            | Op::ReplaceItemSet { text, .. }
            | Op::InsertSubitemAfter { text, .. }
            | Op::AppendSupplArticles { text, .. }
            | Op::AppendContainers { text, .. }
            | Op::InsertArticleBefore { text, .. }
            | Op::AppendTable { text, .. }
            | Op::SetTitle { text, .. }
            | Op::SetToc { text, .. }
            | Op::SetContainerTitle { text, .. }
            | Op::ReplaceArticles { text, .. }
            | Op::ReplaceContainers { text, .. }
            | Op::ReplaceSentencePart { text, .. } => text.push(line),
            _ => {}
        }
    }
}

/// 改め文の 1 文（複数の操作が「、」で連なる）。同じ文の中の項番号は改正前の番号で解釈する
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub text: String,
    pub ops: Vec<Op>,
}

/// 改正単位: 1 つの被改正法令に対する改め文のまとまり（改正法の 1 条）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmendUnit {
    /// 改正法の条（「第三十五条」）
    pub article_of_amending_law: String,
    /// 被改正法令の題名（「借地借家法」）
    pub target_title: String,
    pub instructions: Vec<Instruction>,
}

impl AmendUnit {
    /// 単位を、`locs` の位置（附則の号「〜の改正規定」）に当たる改正規定（文）とそれ以外に分ける。
    /// 附則の号が 1 つの条の一部だけを別の日に施行するとき（令3-49 第6条「医師法第十六条の十一第一項の改正規定を除く」）、
    /// 分けた 2 つの単位を施行日の順に当てる。文の中のどれかの操作が位置に当たれば（条が同じで、項が言われていれば項も同じ）その文
    pub fn split_by_locs(&self, locs: &[Loc]) -> (AmendUnit, AmendUnit) {
        let hits = |ins: &Instruction| {
            ins.ops.iter().any(|op| {
                locs.iter().any(|l| {
                    op.article() == Some(&l.article)
                        && match (&l.paragraph, op.paragraph()) {
                            (None, _) | (Some(_), None) => true,
                            (Some(ParaRef::Num(p)), Some(q)) => *p == q,
                        }
                })
            })
        };
        let (a, b): (Vec<Instruction>, Vec<Instruction>) =
            self.instructions.iter().cloned().partition(hits);
        let mk = |instructions| AmendUnit {
            article_of_amending_law: self.article_of_amending_law.clone(),
            target_title: self.target_title.clone(),
            instructions,
        };
        (mk(a), mk(b))
    }
}
