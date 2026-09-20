# TODO

後回しにしたもの。着手するときはここから消して docs / issue に移す。

## 層 2: grande による判定（[ADR-0009](adr/0009-layer2-decisions-via-grande.md)）

- [ ] 格助詞パーサ: 条件節 → 節末動詞（述語）+ が／を／に／から／まで の格（引数候補）。形態素解析は vibrato / lindera。数量・期間は正規表現
- [ ] `lawean-decide` crate: state = 項の骨組み、questions = ADR-0009 の表、grande の `POST /v1/systemone` を叩く。確率 → `Confidence`
- [ ] 「できる」44 文（借地借家法）の Permission / Power を人がラベル → E4B ゼロショットの精度を測る → 足りなければ pointer head を学習（`tools/train_head.py`）
- [ ] 述語同一性: 同じ条・参照関係にあるペアに絞って `noul`
- [ ] `Author::Model(String)` を Provenance に追加（現状 Human / Llm / Parser / Precedent）
- [ ] grande 側に欲しい機能があれば issue にする（候補: state 内 span を選択肢にする型）
- [ ] `lawean-llm` の README に「残余・レビュー補助のみ」と書く

## 層 2: eval

- [ ] 手書き例 8 条分（`lawean-semantic/examples`）を正解にして、規則 + grande の出力との一致率を測る

## 改正（[docs/08](08-amendment.md)）

- [ ] Lean と Rust の対応: Rust の apply を Lean の定義に対してテストで突き合わせる（同じ Op 列を両方で実行して比較）。将来的には Lean から C へ抽出して Rust から呼ぶ
- [ ] Lean: `deletePara` / `appendPara` を含む可換性、条の挿入（条ずれ）と参照索引の更新の定理（ハネの完全性）
- [ ] 令和4年法律第48号の 3 段施行をシナリオとして検査（§6 の 5）
- [ ] 令和5年法律第53号（2028 施行、第46〜48条の挿入 = 条ずれ）の改め文を取得して条の挿入・繰り下げに対応
- [ ] 他法令へのハネ（被改正法令を参照する他法令）
- [ ] 改め文の語彙の拡張: 「第N条の次に一条を加える」「第N条を第M条とし」「〜を削り、〜を〜とする」「別表」「様式」

## その他

- [ ] 03-examples の残り: 第7条（建物再築）、第13条第2項（裁判所が主体）
- [ ] `law_revisions` API の調査（過去版取得）→ 改正 patch
- [ ] 民法第142条（休日）と遡り計算（「一年前から」）の規則
