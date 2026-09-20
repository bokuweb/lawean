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
}

impl Op {
    /// 続く条文（インデント 1 の行）を受け取る操作か
    pub fn takes_content(&self) -> bool {
        matches!(
            self,
            Op::AppendParagraph { .. }
                | Op::InsertParagraphAfter { .. }
                | Op::AppendArticle { .. }
                | Op::ReplaceArticle { .. }
        )
    }

    pub fn push_content(&mut self, line: String) {
        match self {
            Op::AppendParagraph { text, .. }
            | Op::InsertParagraphAfter { text, .. }
            | Op::AppendArticle { text, .. }
            | Op::ReplaceArticle { text, .. } => text.push(line),
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
