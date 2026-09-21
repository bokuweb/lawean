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
}

impl Loc {
    pub fn new(article: ArticleNum, paragraph: Option<ParaRef>) -> Self {
        Loc {
            article,
            paragraph,
            item: None,
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
    /// 「第N章に次の一条を加える」
    AppendArticle { chapter: u32, text: Vec<String> },
    /// 「第N章の次に次の一章を加える」+ 章の内容（「第M章　題名」「第一節　…」「（見出し）」「第K条　本文」…）
    InsertChapterAfter { after: u32, text: Vec<String> },
    /// 「第N章を第M章とする」
    RenumberChapter { from: u32, to: u32 },
    /// 「第N条の次に次の一条を加える」
    InsertArticleAfter {
        after: ArticleNum,
        text: Vec<String>,
    },
    /// 「第N条第M項（各号列記以外の部分）に後段として次のように加える」— 項の文の末尾に文を足す
    AppendSentence { at: Loc, text: Vec<String> },
    /// 「第N条を次のように改める」
    ReplaceArticle {
        article: ArticleNum,
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
    RenumberArticle { from: ArticleNum, to: ArticleNum },
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
}

impl Op {
    /// この操作が触る被改正法の条（目次・章末への追加は None）
    pub fn article(&self) -> Option<&ArticleNum> {
        match self {
            Op::ReplaceToc { .. }
            | Op::AppendArticle { .. }
            | Op::InsertChapterAfter { .. }
            | Op::RenumberChapter { .. } => None,
            Op::Replace { at, .. }
            | Op::InsertAfterPhrase { at, .. }
            | Op::AppendSentence { at, .. }
            | Op::Delete { at } => Some(&at.article),
            Op::AppendParagraph { article, .. }
            | Op::InsertParagraphAfter { article, .. }
            | Op::ReplaceArticle { article, .. }
            | Op::RenumberParagraph { article, .. }
            | Op::ShiftParagraphs { article, .. } => Some(article),
            Op::InsertArticleAfter { after, .. } => Some(after),
            Op::RenumberArticle { from, .. } => Some(from),
            Op::ShiftArticles { .. } => None,
            Op::ReplaceCaption { article, .. } | Op::SetCaption { article, .. } => Some(article),
            Op::ReplaceSentencePart { at, .. } => Some(&at.article),
        }
    }

    /// 続く条文（インデント 1 の行）を受け取る操作か
    pub fn takes_content(&self) -> bool {
        matches!(
            self,
            Op::AppendParagraph { .. }
                | Op::InsertParagraphAfter { .. }
                | Op::AppendArticle { .. }
                | Op::InsertChapterAfter { .. }
                | Op::InsertArticleAfter { .. }
                | Op::AppendSentence { .. }
                | Op::ReplaceArticle { .. }
                | Op::ReplaceSentencePart { .. }
        )
    }

    pub fn push_content(&mut self, line: String) {
        match self {
            Op::AppendParagraph { text, .. }
            | Op::InsertParagraphAfter { text, .. }
            | Op::AppendArticle { text, .. }
            | Op::InsertChapterAfter { text, .. }
            | Op::InsertArticleAfter { text, .. }
            | Op::AppendSentence { text, .. }
            | Op::ReplaceArticle { text, .. }
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
