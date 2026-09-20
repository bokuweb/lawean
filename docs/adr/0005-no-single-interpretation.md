# 0005. IR に「唯一の正しい解釈」を持たせない

状態: accepted

## 文脈

法令の意味は文理・判例・行政解釈・学説で変わる。IR が「この条文の意味はこれ」と 1 つに決めると、
それは変換者の解釈であって法律ではない。しかし全 Rule に複数解釈を持たせるのは重い。

## 判断

`Rule.interpretations: Vec<Interpretation { expression, authority, confidence }>` を持てるようにし、
**空なら文理解釈のみ**とする。解釈が分かれる箇所だけ複数入れる。`Unknown(Ambiguous)` は「分かれることは分かっているが未記入」。

## 理由

- 多くの条文は文理で一意に読める。全部に解釈を付けるとコストが見合わない
- 検証は「解釈 A のもとで性質 P が成り立つ」の形になる。解釈を選ぶのは検証時のパラメータ

## 結果

- Provenance の `confidence` と Interpretation は別物。confidence は「この変換が原文に忠実か」、Interpretation は「原文が何を意味するか」
- 判例・行政解釈の出典を Provenance に持てる必要がある → `Author::Precedent(case_id)` などを後で追加

## 関連

[art-05.md](../03-examples/art-05.md) 第2項「前項と同様」がただし書きを引き継ぐか
