# 0004. 例外・特則を `Not(condition)` に潰さない

状態: accepted

## 文脈

「A のとき B。ただし C のときはこの限りでない」は論理的には `A ∧ ¬C → B` と同値。
これを 1 つの Rule にまとめると簡潔だが、原文の「本文 / ただし書き」構造と、ただし書きの Provenance が消える。
「第九条の規定にかかわらず」のような特則も同じ。

## 判断

例外・特則は**別の Rule**にし、`overrides: Vec<RuleId>` で関係を宣言する。`Not` は原文に否定がある場合だけ使う。
論理的な正規化（`A ∧ ¬C → B`）は Verification IR で行う。

## 理由

- 改正は「ただし書きを削る / 足す」単位で起きる。Rule が分かれていれば改正 patch が局所的になる
- ただし書き自体に別の例外が付くことがある（例外の例外）。ネストは `overrides` の連鎖で自然に表せる
- 「かかわらず」は複数 Rule を一括で上書きする。`Not` では表せない（[art-09.md](../03-examples/art-09.md)、第22条）

## 結果

- Rule 数は増える。1 条から 3〜5 Rule 出るのが普通
- `overrides` の逆向き（`exceptions`）は resolver が張る。Semantic IR は片方向だけ持つ
- 括弧書きによる場合分け「（〜にあっては、二十年）」も同じ構造で扱えるか検討中（[art-04.md](../03-examples/art-04.md)）

## 関連

[art-03.md](../03-examples/art-03.md)
