# 0001. `Opaque` ではなく `Unknown`。種類を持つ

状態: accepted

## 文脈

部分形式化を前提にすると、IR に「形式化していない部分」が必ず残る。Lean の `opaque` のように単に「中身は見ない」とすると、
「変換器が理解できなかった」と「法律が意図的に曖昧にしている（正当事由、相当、遅滞なく）」の区別が消える。

## 判断

`Unknown { kind, text, provenance }` を Expr / Effect の一級の値にする。`kind` は
`Intentional`（意図的に開放的）/ `Unparsed`（変換器が理解できなかった）/ `Ambiguous`（複数解釈があり未選択）/
`External`（他法令・政令・判例に委ねられている）。

## 理由

- `Unparsed` は減らすべき負債、`Intentional` は減らせない仕様。混ぜると進捗が測れない
- 検証時の扱いが違う。`Intentional` は Z3 に自由変数として渡せるが、`Unparsed` は「この Rule は未検証」と報告すべき
- `External` は参照解決の対象で、他法令を取り込めば消える

## 結果

- Unknown の数と種類が、対象法の形式化率の指標になる
- 変換器（人手・LLM）は「分からなければ Unknown(Unparsed) を出す」ことが許される。無理に構造化しない

## 関連

[art-05.md](../03-examples/art-05.md) の「遅滞なく」、[art-09.md](../03-examples/art-09.md) の「不利」
