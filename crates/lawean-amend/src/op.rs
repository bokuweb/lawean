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

/// 見出しの操作（項の見出しにも）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptionEdit {
    /// 「見出し中「A」を「B」に改め」（削りは `to` が空、「の下に」は `to = A + B`）
    Replace { from: String, to: String },
    /// 「見出しを「（X）」に改め」「見出しを次のように改める」+「（X）」
    Set(String),
    /// 「見出しとして「（X）」を付する」
    Attach(String),
    /// 「見出しを削る」
    Delete,
}

/// 表の在りか
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableRef {
    /// 別表（「別表第一」「別表甲号」「別表第一号表」。列挙「別表第一から第四まで」は字面のまま）
    Appdx(String),
    /// 条・項の中の表（「第十三条第一項の表」）
    InArticle(Loc),
    /// 前文（「前文のうち第五項中」）。表ではないが字面の位置で改める
    Preamble,
}

/// 表の中の位置への操作
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableAction {
    /// 「中「A」を「B」に改め」（削りは `to` が空、「の下に「B」を加え」は `to = A + B`）
    Phrase { from: String, to: String },
    /// 「を次のように改める」+ 内容
    Replace { text: Vec<String> },
    /// 「を削る」
    Delete,
    /// 「の次に次の…を加える」+ 内容
    InsertAfter { text: Vec<String> },
    /// 「の前に次の…を加える」+ 内容
    InsertBefore { text: Vec<String> },
    /// 「に次の…を加える」「に次のように加える」+ 内容
    Append { text: Vec<String> },
    /// 「を…とする」
    Renumber { to: String },
    /// 「第三号から第五号までを一号ずつ繰り上げる」: 位置の範囲の番号を `by` 動かす
    Shift { by: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// 「目次中「A」を「B」に改める」
    ReplaceToc { from: String, to: String },
    /// 「本則中「A」を「B」に改める」、古い法律の位置を書かない「「勅令」を「政令」に改める」: 本則の全部の条で置き換える
    ReplaceAll { from: String, to: String },
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
    /// 「題名中「A」を「B」に改める」「題名中「A」の下に「B」を加える」（`to = A + B`）
    ReplaceTitle { from: String, to: String },
    /// 「目次を削る」
    DeleteToc,
    /// 「題名及び目次を次のように改める」+ 題名の行と目次の行
    SetTitleAndToc { text: Vec<String> },
    /// 「題名を削る」
    DeleteTitle,
    /// 「附則を附則第一条とし」「附則第一項を附則第一条とし」: 項だけの附則の項を条に（原始附則）
    /// 「附則第十六項を附則第二十六条第一項とし」: 条の中の項に（`para`）
    ParagraphToArticle {
        from: u32,
        to: ArticleNum,
        para: Option<u32>,
    },
    /// 「第二章の二を削る」: 枝番の容器 1 つ（`path` は外側から、最後が消す容器）
    DeleteContainer {
        path: Vec<(lawean_source::ContainerKind, String)>,
    },
    /// 「第六章を第七章とし、以下順次一章ずつ繰り下げ」: 容器（`path` の中の `kind`）の番号の範囲を `by` 動かす
    ShiftContainers {
        path: Vec<(lawean_source::ContainerKind, String)>,
        kind: lawean_source::ContainerKind,
        from: u32,
        to: u32,
        by: i32,
    },
    /// 「第十三条の十一から第十三条の十五までを一条ずつ繰り下げ」: 枝番の条の範囲の枝番を `by` 動かす
    ShiftBranchArticles {
        base: u32,
        from: u32,
        to: u32,
        by: i32,
    },
    /// 「第三章中「第五節　収容」を「第五節　収容及保管」に改める」: 容器の中の字句（容器の題名・条の本文）。
    /// 「本則（第九条…を除く。）中」「第三章（第四十九条…を除く。）中」は `except` の条を除く（`path` が空なら本則）
    ReplaceInContainer {
        path: Vec<(lawean_source::ContainerKind, String)>,
        except: Vec<Loc>,
        /// 除く容器の題名（「本則（第四章の章名…を除く。）中」）
        except_titles: Vec<Vec<(lawean_source::ContainerKind, String)>>,
        from: String,
        to: String,
    },
    /// 「第四章の章名及び同章第一節の節名を次のように改める」+ 題名の行（順に各容器へ）
    SetContainerTitles {
        paths: Vec<Vec<(lawean_source::ContainerKind, String)>>,
        text: Vec<String>,
    },
    /// 「第一条の条名を削る」「附則第一条の見出し及び条名を削る」: 残った 1 条の「第N条」を外す（条の無い本則・附則に）
    DeleteArticleTitle { article: ArticleNum },
    /// 「第七十一条の付記中「A」を「B」に改める」: 条の付記（罰則の注記）の字句
    ReplaceSupplNote {
        article: ArticleNum,
        from: String,
        to: String,
    },
    /// 「第百一条の付記を削る」
    DeleteSupplNote { article: ArticleNum },
    /// 「同条を第四章第三節中第三十六条とする」: 条を別の容器（`path`）へ移して番号を付け替える
    MoveArticle {
        from: ArticleNum,
        path: Vec<(lawean_source::ContainerKind, String)>,
        to: ArticleNum,
    },
    /// 「第四章の二第三節を第四章の三第一節とする」: 容器を別の容器の中へ移す
    MoveContainer {
        from: Vec<(lawean_source::ContainerKind, String)>,
        to: Vec<(lawean_source::ContainerKind, String)>,
    },
    /// 「同条第二項を第百十三条の二の六とする」「同項を附則第二十一条第一項とし」: 項を条（の項）にする
    MoveParagraph {
        article: ArticleNum,
        paragraph: u32,
        to: String,
    },
    /// 「第二編第三章の章名及び第八十一条から第八十八条までを次のように改める」「同条ただし書及び各号を次のように改める」:
    /// 種類の違う位置をまとめて改める。`scope` は位置の字面
    ReplaceStructure { scope: String, text: Vec<String> },
    /// 「第六十五条の付記を次のように改める」+ 付記の行
    SetSupplNote {
        article: ArticleNum,
        text: Vec<String>,
    },
    /// 「第五号の二から第五号の四までを一号ずつ繰り下げ」: 号（`at` の項の中）の枝番の範囲を `by` 動かす
    ShiftBranchItems {
        at: Loc,
        base: u32,
        from: u32,
        to: u32,
        by: i32,
    },
    /// 「第十八条中X法第四十二条の三の改正規定を次のように改める」「…の改正規定を削る」「…の改正規定の次に次のように加える」
    /// 「第九条に次の改正規定を加える」: 改正法の改正規定そのものの操作。`target` は位置の字面
    AmendmentEdit { target: String, action: TableAction },
    /// 「第二条のうち、X法目次の改正規定中「A」を「B」に改める」: 改正法の改正規定の中の字句
    ReplaceInAmendment {
        article: ArticleNum,
        target: String,
        from: String,
        to: String,
    },
    /// 「題名の次に次の目次を付する」+ 目次の行（「目次」「第一章　総則（第一条）」…「附則」）。目次の無い法律に目次を足す
    SetToc {
        text: Vec<String>,
        /// 「目次を次のように改める」（目次の差し替え）。false は目次の無い法律に付ける形
        replace: bool,
    },
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
        /// 行の中の細目（「同表の一六の項イ中」の「イ」）。あればその文だけ
        sub: Option<String>,
        from: String,
        to: String,
    },
    /// 「別表第一の八の項を次のように改める」+ 行の内容: 別表の行（rowspan で続く行も含む）の差し替え
    ReplaceAppdxRowWhole {
        table: String,
        row: String,
        text: Vec<String>,
    },
    /// 「別表第一中九の項及び一〇の項を削り」「別表第一建築士法（…）の項を削り」。
    /// 数えられない行の範囲（「Aの項からBの項まで」）は `A〜B`
    DeleteAppdxRows { table: String, rows: Vec<String> },
    /// 「同項を同表の九の項とし」「同表中一二の項を一一の項とし」: 別表の行の番号（上欄）の付け替え
    RenumberAppdxRow {
        table: String,
        from: String,
        to: String,
    },
    /// 「四十四の三の項の次に次のように加える」+ 行の内容
    InsertAppdxRowsAfter {
        table: String,
        after: String,
        text: Vec<String>,
    },
    /// 「附則の次に次の別表を加える」+ 別表の行（題は「別表第二（第三条関係）」の行）
    AppendAppdx { text: Vec<String> },
    /// 「同表を別表第三とし」: 別表の題の付け替え
    RenameAppdx { from: String, to: String },
    /// 「別表第一の次に次の一表を加える」+ 題と行
    InsertAppdxAfter { after: String, text: Vec<String> },
    /// 「同表の一一の二の項中ハを削り」: 別表の行の中の細目（イロハ）の削除
    DeleteAppdxRowSub {
        table: String,
        row: String,
        sub: String,
    },
    /// 「ニをハとし」: 別表の行の中の細目の記号の付け替え
    RenumberAppdxRowSub {
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
    /// 「同項に第一号として次の一号を加える」: 項の先頭に号を置く（号ずれの後）
    InsertItemFirst { at: Loc, text: Vec<String> },
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
    /// 「附則第三項を附則第四項とし」「附則第二条中第一項を…」: 原始附則に向けた操作。中の操作を、原始附則を本則に見立てて当てる
    /// （項だけの附則は仮の条（第0条）に束ねる）。文書の側だけ改める（id の世界には載せない）
    Suppl(Box<Op>),
    /// 「同項（第三号を除く。）及び同条第三項第一号中「A」を「B」に改める」: 除く位置のある字句の操作（`except` は位置の中で除くところ）
    Except { op: Box<Op>, except: Vec<Loc> },
    /// 「第八条の次に次の二章を加える」「第十条の次に次の一款を加える」+ 章・款の内容: 条の後ろに容器を置く
    InsertContainersAfterArticle {
        after: ArticleNum,
        text: Vec<String>,
    },
    /// 「第一条の前に次の章名を加える」「第五条の前に次の目次及び章名を加える」「題名の次に次の目次及び章名を附する」
    /// + 目次・章名の行: 条の前に容器の題名を置く（その条から次の題名までがその容器）。`before` が None なら本則の最初
    InsertHeadingsBefore {
        before: Option<ArticleNum>,
        /// 「第三条の次に次の章名を付する」: 条の後ろ（次の条の前）
        after: bool,
        with_toc: bool,
        text: Vec<String>,
    },
    /// 「同号イからニまでを次のように改める」「同号イ及びロを削る」: 号（`at` の item）の下のイロハの列挙。
    /// `text` が None なら削る
    SubitemsEdit {
        at: Loc,
        subs: Vec<String>,
        text: Option<Vec<String>>,
    },
    /// 「第N条中次の表の上欄に掲げる字句を同表の下欄に掲げる字句に改める」+ 字句の対の表。`scope` は位置の字面
    ReplacePairs { scope: String, text: Vec<String> },
    /// 「附則第十四項の見出し中「A」を「B」に改め」「附則第六項の前の見出しを削る」: 項の見出し
    ParagraphCaption { at: Loc, edit: CaptionEdit },
    /// 表の中の位置（「第四表名称の欄」「第二類第五号」「備考」「A及びBの項」）への操作。位置は改め文の字面のまま（空は表の全部）
    TableEdit {
        table: TableRef,
        path: String,
        action: TableAction,
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
            | Op::ReplaceAll { .. }
            | Op::SetToc { .. }
            | Op::AppendSupplArticles { .. }
            | Op::AppendContainers { .. }
            | Op::AppendArticle { .. }
            | Op::InsertContainersAfter { .. }
            | Op::InsertContainersBefore { .. }
            | Op::RenumberContainer { .. }
            | Op::ReplaceContainerTitle { .. }
            | Op::SetTitle { .. }
            | Op::ReplaceTitle { .. }
            | Op::SetContainerTitle { .. }
            | Op::DeleteContainerTitle { .. }
            | Op::DeleteContainers { .. }
            | Op::ReplaceContainers { .. }
            | Op::ReplaceAppdxRow { .. }
            | Op::ReplaceAppdxRowWhole { .. }
            | Op::DeleteAppdxRows { .. }
            | Op::RenumberAppdxRow { .. }
            | Op::InsertAppdxRowsAfter { .. }
            | Op::AppendAppdx { .. }
            | Op::RenameAppdx { .. }
            | Op::InsertAppdxAfter { .. }
            | Op::DeleteAppdxRowSub { .. }
            | Op::RenumberAppdxRowSub { .. }
            | Op::DeleteAppdx { .. }
            | Op::InsertHeadingsBefore { .. }
            | Op::DeleteToc
            | Op::MoveContainer { .. }
            | Op::ReplaceStructure { .. }
            | Op::AmendmentEdit { .. }
            | Op::SetTitleAndToc { .. }
            | Op::ReplacePairs { .. }
            | Op::DeleteTitle
            | Op::ParagraphToArticle { .. }
            | Op::DeleteContainer { .. }
            | Op::ShiftBranchArticles { .. }
            | Op::ShiftContainers { .. }
            | Op::ReplaceInContainer { .. }
            | Op::SetContainerTitles { .. }
            | Op::Suppl(_)
            | Op::Except { .. } => None,
            Op::InsertContainersAfterArticle { after, .. } => Some(after),
            Op::MoveArticle { from, .. } => Some(from),
            Op::DeleteArticleTitle { article }
            | Op::DeleteSupplNote { article }
            | Op::MoveParagraph { article, .. }
            | Op::SetSupplNote { article, .. }
            | Op::ReplaceSupplNote { article, .. }
            | Op::ReplaceInAmendment { article, .. } => Some(article),
            Op::TableEdit { table, .. } => match table {
                TableRef::InArticle(at) => Some(&at.article),
                TableRef::Appdx(_) | TableRef::Preamble => None,
            },
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
            | Op::InsertItemFirst { at, .. }
            | Op::ReplaceItems { at, .. }
            | Op::ReplaceItemSet { at, .. }
            | Op::ReplaceTableRow { at, .. }
            | Op::AppendTable { at, .. }
            | Op::RenumberSubitem { at, .. }
            | Op::ShiftSubitems { at, .. }
            | Op::InsertSubitemAfter { at, .. }
            | Op::SubitemsEdit { at, .. }
            | Op::ShiftBranchItems { at, .. }
            | Op::ParagraphCaption { at, .. } => Some(&at.article),
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
            | Op::InsertItemFirst { at, .. }
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

    /// 位置（`at`）を持つ操作の位置
    pub fn loc_mut(&mut self) -> Option<&mut Loc> {
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
            | Op::InsertItemFirst { at, .. }
            | Op::ReplaceItems { at, .. }
            | Op::ReplaceItemSet { at, .. }
            | Op::ReplaceTableRow { at, .. }
            | Op::AppendTable { at, .. }
            | Op::RenumberSubitem { at, .. }
            | Op::ShiftSubitems { at, .. }
            | Op::InsertSubitemAfter { at, .. }
            | Op::SubitemsEdit { at, .. }
            | Op::ShiftBranchItems { at, .. }
            | Op::ParagraphCaption { at, .. } => Some(at),
            Op::TableEdit {
                table: TableRef::InArticle(at),
                ..
            } => Some(at),
            _ => None,
        }
    }

    /// 字句（置換の前後・加える字句・表の中の位置…）に `f` を当てる（字句の中で伏せた「」を戻すのに使う）
    pub fn map_strings(&mut self, f: &dyn Fn(&str) -> String) {
        let m = |x: &mut String| *x = f(x);
        match self {
            Op::ReplaceToc { from, to }
            | Op::ReplaceAll { from, to }
            | Op::Replace { from, to, .. }
            | Op::ReplaceTitle { from, to }
            | Op::ReplaceCaption { from, to, .. }
            | Op::ReplaceContainerTitle { from, to, .. }
            | Op::ReplaceSupplNote { from, to, .. }
            | Op::ReplaceInContainer { from, to, .. }
            | Op::ReplaceAppdxRow { from, to, .. }
            | Op::ReplaceInAmendment { from, to, .. } => {
                m(from);
                m(to);
            }
            Op::ReplaceTableRow { row, from, to, .. } => {
                m(row);
                m(from);
                m(to);
            }
            Op::InsertAfterPhrase { anchor, text, .. } => {
                m(anchor);
                m(text);
            }
            Op::ParagraphCaption {
                edit: CaptionEdit::Replace { from, to },
                ..
            } => {
                m(from);
                m(to);
            }
            Op::TableEdit { path, action, .. } => {
                m(path);
                if let TableAction::Phrase { from, to } = action {
                    m(from);
                    m(to);
                }
            }
            Op::AmendmentEdit { target, .. } => m(target),
            Op::SetCaption { text, .. } | Op::AttachCaption { text, .. } => m(text),
            Op::Suppl(inner) => inner.map_strings(f),
            Op::Except { op, .. } => op.map_strings(f),
            _ => {}
        }
    }

    /// 原始附則に向けた操作（`suppl`）を `Op::Suppl` に包む。附則を自分で扱う操作（字句の置換・条の追加・条ずれ）はそのまま
    pub fn in_suppl(mut self, suppl: bool) -> Op {
        let suppl = match self.loc_mut() {
            Some(l) => l.suppl,
            None => suppl,
        };
        if !suppl || self.article().is_none() {
            return self;
        }
        match self {
            Op::Replace { .. }
            | Op::InsertAfterPhrase { .. }
            | Op::InsertArticleAfter { .. }
            | Op::InsertArticleBefore { .. }
            | Op::RenumberArticle { .. } => self,
            mut op => {
                if let Some(l) = op.loc_mut() {
                    l.suppl = false;
                }
                Op::Suppl(Box::new(op))
            }
        }
    }

    /// 続く条文（インデント 1 の行）を受け取る操作か
    pub fn takes_content(&self) -> bool {
        if let Op::Suppl(inner) = self {
            return inner.takes_content();
        }
        // 「第二十四条の見出しを次のように改める」+「（見出し）」
        if let Op::SetCaption { text, .. } = self {
            return text.is_empty();
        }
        if let Op::SubitemsEdit { text, .. } = self {
            return text.is_some();
        }
        if let Op::ReplacePairs { .. }
        | Op::SetSupplNote { .. }
        | Op::SetTitleAndToc { .. }
        | Op::ReplaceStructure { .. } = self
        {
            return true;
        }
        if let Op::ParagraphCaption {
            edit: CaptionEdit::Set(text),
            ..
        } = self
        {
            return text.is_empty();
        }
        if let Op::TableEdit { action, .. } | Op::AmendmentEdit { action, .. } = self {
            return matches!(
                action,
                TableAction::Replace { .. }
                    | TableAction::InsertAfter { .. }
                    | TableAction::InsertBefore { .. }
                    | TableAction::Append { .. }
            );
        }
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
                | Op::InsertItemFirst { .. }
                | Op::ReplaceItems { .. }
                | Op::ReplaceItemSet { .. }
                | Op::InsertSubitemAfter { .. }
                | Op::AppendSupplArticles { .. }
                | Op::AppendContainers { .. }
                | Op::InsertArticleBefore { .. }
                | Op::AppendTable { .. }
                | Op::ReplaceAppdxRowWhole { .. }
                | Op::InsertAppdxRowsAfter { .. }
                | Op::AppendAppdx { .. }
                | Op::InsertAppdxAfter { .. }
                | Op::SetTitle { .. }
                | Op::SetToc { .. }
                | Op::SetContainerTitle { .. }
                | Op::ReplaceArticles { .. }
                | Op::ReplaceContainers { .. }
                | Op::ReplaceSentencePart { .. }
                | Op::InsertContainersAfterArticle { .. }
                | Op::InsertHeadingsBefore { .. }
                | Op::SetContainerTitles { .. }
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
            | Op::InsertItemFirst { text, .. }
            | Op::ReplaceItems { text, .. }
            | Op::ReplaceItemSet { text, .. }
            | Op::InsertSubitemAfter { text, .. }
            | Op::AppendSupplArticles { text, .. }
            | Op::AppendContainers { text, .. }
            | Op::InsertArticleBefore { text, .. }
            | Op::AppendTable { text, .. }
            | Op::ReplaceAppdxRowWhole { text, .. }
            | Op::InsertAppdxRowsAfter { text, .. }
            | Op::AppendAppdx { text, .. }
            | Op::InsertAppdxAfter { text, .. }
            | Op::SetTitle { text, .. }
            | Op::SetToc { text, .. }
            | Op::SetContainerTitle { text, .. }
            | Op::ReplaceArticles { text, .. }
            | Op::ReplaceContainers { text, .. }
            | Op::ReplaceSentencePart { text, .. } => text.push(line),
            Op::Suppl(inner) => inner.push_content(line),
            Op::SetCaption { text, .. } if text.is_empty() => *text = line.trim().to_string(),
            Op::SubitemsEdit {
                text: Some(text), ..
            }
            | Op::ReplacePairs { text, .. }
            | Op::SetSupplNote { text, .. }
            | Op::SetTitleAndToc { text }
            | Op::ReplaceStructure { text, .. } => text.push(line),
            Op::ParagraphCaption {
                edit: CaptionEdit::Set(text),
                ..
            } if text.is_empty() => *text = line.trim().to_string(),
            Op::InsertContainersAfterArticle { text, .. }
            | Op::InsertHeadingsBefore { text, .. }
            | Op::SetContainerTitles { text, .. } => text.push(line),
            Op::TableEdit {
                action:
                    TableAction::Replace { text }
                    | TableAction::InsertAfter { text }
                    | TableAction::InsertBefore { text }
                    | TableAction::Append { text },
                ..
            }
            | Op::AmendmentEdit {
                action:
                    TableAction::Replace { text }
                    | TableAction::InsertAfter { text }
                    | TableAction::InsertBefore { text }
                    | TableAction::Append { text },
                ..
            } => text.push(line),
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
