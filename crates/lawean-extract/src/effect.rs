//! 文末の効果表現の分類。法制執務の用語統一により語彙は閉じている（ADR-0008 層 1）。

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EffectKind {
    /// しなければならない
    Obligation,
    /// してはならない
    Prohibition,
    /// することができる。Permission と Power の区別は層 2（ADR-0003）
    Can,
    /// することができない
    CanNot,
    /// みなす
    Deem,
    /// 推定する
    Presume,
    /// この限りでない
    Exception,
    /// 適用しない
    NotApply,
    /// 適用する
    Apply,
    /// 準用する
    ApplyMutatis,
    /// 〜とみなして、〜の規定を適用する（読み替え + 適用）
    DeemAndApply,
    /// 無効とする / 効力を生じない
    Void,
    /// 従前の例による
    FormerExample,
    /// 効力を有する / 効力を妨げない / 効力を保存する
    Preserve,
    /// 効力を失う
    Lapse,
    /// もって足りる
    Suffice,
    /// その定めに従う / その期間による / 定めるところによる
    Follow,
    /// から施行する
    Enforce,
    /// 〜とは、〜をいう
    Definition,
    /// 廃止する
    Repeal,
    /// とする / ものとする / 〜する（法律関係の内容を定める平叙文）
    Set,
    /// 〜場合に限る（ただし書きの限定）
    Limit,
    /// 定義欄・号の断片など、文末が述語でない
    Fragment,
}

const TABLE: &[(&str, EffectKind)] = &[
    (r"なければならない。?$", EffectKind::Obligation),
    (r"してはならない。?$", EffectKind::Prohibition),
    (r"ことができない。?$", EffectKind::CanNot),
    (r"ことができる。?$", EffectKind::Can),
    (r"とみなす。?$", EffectKind::Deem),
    (r"と推定する。?$", EffectKind::Presume),
    (r"この限りでない。?$", EffectKind::Exception),
    (r"適用しない。?$", EffectKind::NotApply),
    (r"適用する。?$", EffectKind::Apply),
    (r"準用する。?$", EffectKind::ApplyMutatis),
    (
        r"(無効とする|効力を生じない|効力を有しない)。?$",
        EffectKind::Void,
    ),
    (r"従前の例による。?$", EffectKind::FormerExample),
    (
        r"(効力を有する|効力を妨げない|効力を保存する|妨げない)。?$",
        EffectKind::Preserve,
    ),
    (r"効力を失う。?$", EffectKind::Lapse),
    (r"もって足りる。?$", EffectKind::Suffice),
    (
        r"(定めに従う|期間による|定めるところによる|に従う)。?$",
        EffectKind::Follow,
    ),
    (r"から施行する。?$", EffectKind::Enforce),
    (r"廃止する。?$", EffectKind::Repeal),
    (r"をいう。?$", EffectKind::Definition),
    (r"に限る。?$", EffectKind::Limit),
    (
        r"(とする|する|ずる|生ずる|有する|後れる|定める|帰属する|存続する|終了する|消滅する|承継する|管轄する|指定する|組織する|支給する)。?$",
        EffectKind::Set,
    ),
    // 「消滅しない」のような否定の平叙文。適用しない・効力を生じない等は上で先に拾う
    (r"ない。$", EffectKind::Set),
];

fn table() -> &'static [(Regex, EffectKind)] {
    static T: OnceLock<Vec<(Regex, EffectKind)>> = OnceLock::new();
    T.get_or_init(|| {
        TABLE
            .iter()
            .map(|(p, k)| (Regex::new(p).unwrap(), *k))
            .collect()
    })
}

/// 文末から効果種別を判定し、マッチした末尾表現を返す
pub fn classify(text: &str) -> (EffectKind, Option<String>) {
    let t = text.trim();
    for (re, kind) in table() {
        if let Some(m) = re.find(t) {
            let tail = m.as_str().trim_end_matches('。').to_string();
            // 「A を B とみなして、〜の規定を適用する」は Apply ではなく読み替え + 適用
            if *kind == EffectKind::Apply && t.contains("とみなして") {
                return (EffectKind::DeemAndApply, Some(tail));
            }
            return (*kind, Some(tail));
        }
    }
    (EffectKind::Fragment, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tails() {
        assert_eq!(
            classify("借地権の存続期間は、三十年とする。").0,
            EffectKind::Set
        );
        assert_eq!(
            classify("従前の契約と同一の条件で契約を更新したものとみなす。").0,
            EffectKind::Deem
        );
        assert_eq!(
            classify("ただし、借地権設定者が遅滞なく異議を述べたときは、この限りでない。").0,
            EffectKind::Exception
        );
        assert_eq!(
            classify("その特約は、公正証書による等書面によってしなければならない。").0,
            EffectKind::Obligation
        );
        assert_eq!(
            classify("正当の事由があると認められる場合でなければ、述べることができない。").0,
            EffectKind::CanNot
        );
        assert_eq!(
            classify("この節の規定に反する特約で借地権者に不利なものは、無効とする。").0,
            EffectKind::Void
        );
        assert_eq!(
            classify("なお従前の例による。").0,
            EffectKind::FormerExample
        );
        assert_eq!(classify("借地権").0, EffectKind::Fragment);
        assert_eq!(classify("転借地権者がする土地の使用の継続を借地権者がする土地の使用の継続とみなして、前項の規定を適用する。").0, EffectKind::DeemAndApply);
        assert_eq!(
            classify("建物の所有を目的とする地上権又は土地の賃借権をいう。").0,
            EffectKind::Definition
        );
    }
}
