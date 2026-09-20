# 0003. 「できる」を Permission と Power で分ける

状態: accepted

## 文脈

「〜することができる」には、(a) 許容（してもよい。しても違法でない）と、(b) 権限・形成権（それをすると法律関係が変わる。
請求すると相手に義務が生じる、解除すると契約が消滅する）の 2 つがある。Hohfeld の privilege と power に対応する。

## 判断

`Effect::Permission(Action)` と `Effect::Power(Action)` を別にする。Power は「行使したときに生じる効果」を持つ。

## 理由

- Permission は制約の緩和（禁止の否定）で、他の Rule に影響しない。Power は行使が新しい Fact / Obligation を生む。検証モデルが違う
- 借地借家法は Power だらけ（更新請求、買取請求、増減請求、異議、解約申入れ）。ここを潰すと法律の骨格が消える

## 結果

- 「できる」の分類は変換時に人が判断する。迷ったら `Unknown(Ambiguous)` で複数解釈を残す
- Power の「行使したときの効果」は別 Rule への参照でよい（第13条 買取請求 → 売買が成立したものとみなす）

## 関連

[art-05.md](../03-examples/art-05.md)、第13条（未作成）
