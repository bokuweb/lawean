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

## 構造化記述（[ADR-0012](adr/0012-structured-authoring.md)）

- [x] `Op` → 改め文（`lawean-render::amend`、往復テスト）
- [x] Semantic IR → 日本語（`lawean-render::semantic`、原文と並べる）
- [ ] 人が書く構造化記述の入力形式（Rust 構築子か専用記法か）。改正案を IR patch として書き、改め文・新旧対照表・溶け込み条文を生成する
- [ ] 述語・引数の日本語化を自然文に近づける（今は `更新を請求した（by=借地権者）`）。語彙表と格助詞の逆変換
- [ ] 証明結果（Z3 の反例、Lean の定理名）に Provenance を引きずって表示する

## Lean を溶け込みの正にする（[ADR-0011](adr/0011-lean-as-reference-for-consolidation.md)）

- [ ] `lawean-lean` crate: Source IR → `def rev_… : Revision`、改め文の `Op` 列 → `def unit_… : AmendUnit` を `.lean` として出力（空白・全角数字の正規化を Rust 側と揃える）
- [ ] `lean/Lawean/Data/` に出力を置き、`theorem consolidates_r3_37_art35 : applyUnit rev_20210519 unit_r3_37_art35 = some rev_20220518 := by native_decide`
- [ ] Lean の `Revision` に目次と条の挿入・繰り下げを足す
- [ ] 3 段施行の順序依存を `applyUnit … = none` の定理に
- [ ] Rust `apply_unit` と Lean `applyUnit` の一致をテスト（同じ Op 列）
- [ ] （後）Semantic IR の Lean 化: `applies_R` を Bool 関数、期間を Int、07 の 6 性質を `omega` / `decide` で

## identity patch（[ADR-0013](adr/0013-identity-patches.md)）

- [x] Lean: `Ident.lean`（id ベースの `Op`、衝突を値に、`dependsOn` / `scheduleOk`、`paraNum`）と `applyOp_comm` / `applyUnit_comm`
- [ ] Rust: `lawean-amend` の番号 `Op` を発射台リビジョンに対して stable_id に束縛する `bind`（「「A」を「B」に改める」の複数箇所は id ごとに展開、全部改正は delete + insert）
- [ ] Source IR の参照を id 参照にし、番号を描画で出す。ハネ手当ての改め文を生成し、成立した改め文との差分で検査する
- [ ] id の決定性: 改正法 ID + 位置から振る規則を Rust と Lean で揃える
- [ ] 3 段施行（令和4年法律第48号）を `dependsOn` / `scheduleOk` の実データで検査
- [ ] `Ident` の `Revision` にも目次と条を足し、ADR-0011 の `consolidates` を `Ident.applyUnit` で行う

## 他法令への波及（[docs/09](09-cross-law-impact.md)）

- [x] 法令名 → law_id の対応表、`cross_refs(B, A)`
- [x] `provision_mapping`（条・項の移動、本文の変化、新設）
- [x] `impact(space, u)`: 参照切れ・ずれ・意味変化・時期不整合
- [x] 高齢者居住安定確保法 第52条 / 借地借家法 第30条の Semantic IR（テスト内の手書き）と上書き循環の検出
- [x] テスト計画 1〜5
- [ ] Lean: `LawSpace` と §4 の定理形
- [ ] 略称の解決（「同法」「新借地借家法」「旧法」）。附則の「（以下「新法」という。）」を拾う
- [ ] 政令委任（「政令で定めるところにより」）を A → C の依存として参照グラフに足す
- [ ] `impact` の結果を `lawean-render` で日本語の報告書にする

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
