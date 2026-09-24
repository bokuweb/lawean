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
- [x] `checkUnit : Revision → AmendUnit → Bool` と正しさの定理（`lean/Lawean/Check.lean`: `checkUnit_iff`、`applyUnit_none_iff`）
- [x] Lean → C → Rust（ネイティブ）: `lawean-leanrt`（[ADR-0017](adr/0017-lean-to-c-is-the-runtime.md)）
- [x] Lean → C → WASM（`docs/playground/build-lean-wasm.sh`。Lean の wasm32 版ツールチェーン + libuv スタブ）。playground は Lean 経路
- [ ] `ident::apply_unit`（Rust の写し）を消す: `IdentOp` / `IdentRevision` を型だけの crate に出し、`id_map` 等も `lawean-leanrt` 経由にする。Lean が無い環境（CI の一部）の代役をどうするかも決める
- [ ] `Sem.applies` / `haneFixes` / `impact` の `@[export]` 入口（今は `applyUnit` / `checkUnit` / 依存の 3 つ）
- [ ] 証跡の形式（[ADR-0015](adr/0015-service-architecture.md) §5）: 発射台リビジョンのハッシュ、改正単位、結果のハッシュ、WASM のバージョン。ハッシュ対象の正規化を Rust / Lean / WASM で揃える
- [ ] エディタの各判定に裏付けの定理名を添える（`applyUnit_comm` / `scheduleOk` 等）
- [ ] Z3 の WASM ビルドで自法令内の性質検査をブラウザで閉じられるか測る。法令空間（他法令への波及）はサーバー側
- [ ] Semantic IR の Lean 化 → [docs/10](10-lean-semantics.md) に移した（M1・M2 済み、M3〜M5）

## 部分施行（`lawean-check::stage`、Lean `Stage.lean`）

- [x] 分割の保存則（部分列への分割、命令を落とさない）を Rust のテストに。本文の施行期日が読めないとき命令を落としていたのを修正
- [x] Lean: 独立な部分への分割は施行の順によらず単位全体と同じ（`applyUnit_interleave` / `staged_eq_whole` / `applyParts_eq`）。独立でない分割の反例
- [x] 別の日の部分が同じ条・項を触れば Enforcement に注意（`stage::overlaps`）
- [ ] 部分を `ident::bind` で id 操作にして、Lean の `isSplit` / `indepParts` を実データ（令3-49 第6条・第13条）で `native_decide`。`overlaps` は条・項の粒度の近似なので、id 粒度に置き換える
- [ ] 「…を改める部分に限る。」（文の一部だけを別の日に）: 今は文の単位でしか分けられない

## identity patch（[ADR-0013](adr/0013-identity-patches.md)）

- [x] Lean: `Ident.lean`（id ベースの `Op`、衝突を値に、`dependsOn` / `scheduleOk`、`paraNum`）と `applyOp_comm` / `applyUnit_comm`
- [x] Rust: `lawean-amend::ident::bind`（複数箇所は id ごとに展開、全部改正は旧第1項に anchor した insert + delete、繰り下げは消える）
- [x] ハネ手当ての生成（`hane::render_fix`、`lawean-check` の `suggested_fixes`）と、成立した改め文との突き合わせ。Lean `Refs.lean` で本文を id 参照で持つ `Body` と完全性。Source IR そのものを id 参照にする（層 2 の参照解決で `Body` を出す）のは未
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
- [x] Lean: `LawSpace` と分類の健全性・完全性（`Space.lean`）。施行日の区間（TimingGap）は Lean の外
- [ ] 略称の解決（「同法」「新借地借家法」「旧法」）。附則の「（以下「新法」という。）」を拾う
- [ ] 政令委任（「政令で定めるところにより」）を A → C の依存として参照グラフに足す
- [ ] `impact` の結果を `lawean-render` で日本語の報告書にする

## 改正（[docs/08](08-amendment.md)）

- [ ] Lean と Rust の対応: Rust の apply を Lean の定義に対してテストで突き合わせる（同じ Op 列を両方で実行して比較）。将来的には Lean から C へ抽出して Rust から呼ぶ
- [ ] Lean: `deletePara` / `appendPara` を含む可換性、条の挿入（条ずれ）。ハネの完全性は `Refs.lean` で済み（項の参照。条の参照は条ずれと一緒に）
- [ ] 令和4年法律第48号の 3 段施行をシナリオとして検査（§6 の 5）
- [ ] 令和5年法律第53号（2028 施行、第46〜48条の挿入 = 条ずれ）の改め文を取得して条の挿入・繰り下げに対応
- [ ] 他法令へのハネ（被改正法令を参照する他法令）
- [ ] 改め文の語彙の拡張: 「第N条の次に一条を加える」「第N条を第M条とし」「〜を削り、〜を〜とする」「別表」「様式」

## その他

- [ ] 03-examples の残り: 第7条（建物再築）、第13条第2項（裁判所が主体）
- [ ] `law_revisions` API の調査（過去版取得）→ 改正 patch
- [ ] 民法第142条（休日）。遡りは応当日で逆算する扱いで `temporal::before` に実装済み（判例・実務の扱い。要確認）
- [ ] `TimeCond::Within` / `Elapsed` を `temporal` の日付制約に自動展開し、`Value::Period` を日付で持つ（今は月数の Int と別）
- [x] 施行日の許容区間を附則から取る（`lawean-extract::suppl`、`lawean-check` の Enforcement）。廃止・経過措置の区間（`validity::Interval` の `to`）はまだ手で与える
- [ ] Lean の WASM: 大きな法令（公職選挙法 1167 項）で Emscripten の既定スタック（64KB）では落ちるので `-sSTACK_SIZE=32MB` にした。落ちたときは JS が Rust の写しに戻す。Lean 側の再帰の深さ（`List` の非末尾再帰）を減らせば既定に戻せる
- [ ] **新旧対照表の生成は後回し**（2026-09-22 の判断）。`Report.taisho_generated` は溶け込みから作れる最小形（項単位の新/旧、`check_taisho` の形式）で置いてあるが、見た目（欄の対照・傍線・条見出し）や部分改正の表現は詰めていない。優先は精度と実際の改正での検証
