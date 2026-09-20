# TODO

後回しにしたもの。着手するときはここから消して docs / issue に移す。

## 層 2: grande による判定（[ADR-0009](adr/0009-layer2-decisions-via-grande.md)）

→ 着手順と完了条件は [docs/11](11-layer2.md) に移した。ここは細目だけ残す。

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

- [x] `lawean-lean` crate: Source IR → `def rev_… : Revision`、束縛した改め文 → `def unit_… : AmendUnit`（`Ident` 版。正規化は `para_text` の空白除去だけ）
- [x] `lean/Lawean/Data/` に出力を置き、`Consolidate.lean` で `consolidates_…`（`render` の一致）を `native_decide`
- [x] 目次は `toc` ノード。条の挿入は `insertAfter`（`AppendArticle`）で足りる。条の繰り下げ（条ずれ）は未（令和5年法律第53号）
- [x] 3 段施行の順序依存を `applyUnit … = none` の定理に（`art74_before_art73_fails`）
- [x] Rust `ident::apply_unit`（Lean の写し）と `apply_unit`（Source IR 版）と Lean の三者一致。Rust 側は `tests/ident_binding.rs`、Lean 側は `Consolidate.lean`。Lean → C 抽出で Rust の写しを不要にするのは未
- [ ] 条の見出し・条名の変更は `render` に入っていない（`snapshot_main` と同じ。全部改正で見出しが変わるケースは検査できない）
- [ ] `checkUnit : Revision → AmendUnit → Bool` と正しさの定理（[ADR-0014](adr/0014-proofs-at-build-time-editor-runs-verified-code.md)）。エディタから呼ぶ判定関数
- [ ] WASM 化の経路: **Lean → C** を本線に決めた（[ADR-0015](adr/0015-service-architecture.md)、[ADR-0016](adr/0016-lean-as-semantic-backend.md)、[docs/10](10-lean-semantics.md) M4）。C 呼び出しができたら `ident::apply_unit` を消す
- [ ] 証跡の形式（[ADR-0015](adr/0015-service-architecture.md) §5）: 発射台リビジョンのハッシュ、改正単位、結果のハッシュ、WASM のバージョン。ハッシュ対象の正規化を Rust / Lean / WASM で揃える
- [ ] エディタの各判定に裏付けの定理名を添える（`applyUnit_comm` / `scheduleOk` 等）
- [ ] Z3 の WASM ビルドで自法令内の性質検査をブラウザで閉じられるか測る。法令空間（他法令への波及）はサーバー側
- [ ] Semantic IR の Lean 化 → [docs/10](10-lean-semantics.md) に移した（M1・M2 済み、M3〜M5）

## identity patch（[ADR-0013](adr/0013-identity-patches.md)）

- [x] Lean: `Ident.lean`（id ベースの `Op`、衝突を値に、`dependsOn` / `scheduleOk`、`paraNum`）と `applyOp_comm` / `applyUnit_comm`
- [x] Rust: `lawean-amend::ident::bind`（複数箇所は id ごとに展開、全部改正は旧第1項に anchor した insert + delete、繰り下げは消える）
- [ ] Source IR の参照を id 参照にし、番号を描画で出す。ハネ手当ての改め文を生成し、成立した改め文との差分で検査する
- [x] id の決定性: `<改正法ID>/art<条>/art:<条>/new:<連番>`。Lean は id を計算しない（Rust が出したものを使う）ので揃える対象は Rust だけ
- [x] 3 段施行（令和4年法律第48号）を `dependsOn` / `scheduleOk` の実データで検査（Rust と Lean の両方）
- [x] `Ident` の `Revision` に目次（`toc`）を足し、ADR-0011 の `consolidates` を `Ident.applyUnit` で行う。`Node.art` は `String`（枝番 `42_2` のため）
- [x] 改正法が振った id と e-Gov の id の対応表: `ident::id_map`（当てた結果と e-Gov の版を文書順で突き合わせる）。`touched_egov_ids` / `modified_egov_ids` が Semantic IR の provenance と交わるかを frame 定理で使う

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
