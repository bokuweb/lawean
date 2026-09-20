# 0013. 改正単位は identity で書く。独立なら可換、衝突は値、依存は半順序

状態: accepted（2026-09-20）。Lean は実装済み（`lean/Lawean/Ident.lean`、`IdentExamples.lean`）。Rust 側の束縛は未（[TODO](../TODO.md)）

## 文脈

[ADR-0010](0010-amendment-first.md) の `Op` は「第N条第M項」という**番号**で対象を指す。可換性の定理 `applyOp_comm` は
「触る条が違えば可換」までで、同じ条の中の項の挿入は番号を動かすので可換にならない。

施行順序は事後には一意に決まるが、起案時には決まらない。「公布の日から起算して…政令で定める日」の未確定施行日と、
他法令の整備法が先に同じ法令を改正する**割り込み**が現実にある（[08](../08-amendment.md) §1 の「発射台の特定が負担」はこれ）。
割り込みがある以上、改正単位の可換性は「あれば列挙が省ける」性質ではなく、**発射台がずれても同じ項に当たる**ために要る性質である。

CRDT を検討した。収束型 CRDT（RGA / Yjs / Automerge）は位置を一意 ID で表す点は同じだが、衝突をタイムスタンプ等で**黙って解決する**。
法令では順序依存そのものが検出したい異常であり、解消は人が調整規定を書くことで行う。欲しいのは Pijul / Darcs 流のパッチ理論、
すなわち「独立なら可換、衝突は値として残る」構造である。

## 判断

1. **対象は stable_id で指す。** `Op.replace (id) (expected) (new)` / `insertAfter (anchor) (newId) …` / `delete (id)` / `resolve (id) (text)`。
   番号は状態に持たず、描画時に計算する（`paraNum`）。新しい id は改正法 ID と位置から決定的に振る（誰が計算しても同じ）
2. **独立なら可換。** 独立 = 触る id（読む・書く・作る）が交わらない。`applyOp_comm` / `applyUnit_comm` を Lean で証明した。
   全ての発射台・全ての操作について成り立つので、独立な改正単位の施行順序は列挙しなくてよい
3. **衝突は失敗ではなく値。** `replace` は期待した本文（Pijul の context に相当）を持ち、違えば `Node.conflicts` に新本文を積む。
   溶け込みは成功し、検査 `hasConflict` が衝突を報告する。解消は `resolve`（調整規定 = 両方に依存する patch）
4. **依存は半順序。** B が A の作った id を触るなら B は A に依存する（`dependsOn`）。施行スケジュールはその線形拡張でなければならない（`scheduleOk`）。
   依存先が未施行なら対象の id が無く `none`（発射台に無いものを触っている）
5. **改め文は表層構文。** 番号ベースの `Op`（`Lawean.Basic` / `Apply`、Rust の `lawean-amend`）は残し、
   パースは「発射台リビジョン R0 に対して番号を id に束縛する」工程にする。束縛に失敗すれば発射台の不一致がその場で出る
6. **CRDT は中核に採らない。** 複数の起案者が同じ改正案を同時に編集する要件が出たら、エディタ層（Automerge / Yjs）に限定して置き、
   確定した改正単位からは本 ADR の代数で検査する

## 理由

- 番号は改正で動くが identity は動かない。[ADR-0007](0007-stable-id-and-version-id.md) の stable_id はすでにこの役割を持っている
- 独立性は `decide` で判定でき、定理が残りを引き受ける。順序が要る場所（依存・衝突）だけが型に現れる
- 衝突を値にすると「どちらの順でも溶け込むが結果が違う」ケースが失敗にも黙殺にもならず、審査資料として出せる
- 参照を id で持てば、項の挿入で参照は壊れない。ハネ改正の手当ては**探すものではなく生成するもの**になり（[ADR-0012](0012-structured-authoring.md)）、
  成立した改め文の検査は「生成した手当てが含まれているか」の差分になる

## 結果（トレードオフ）

- Lean: `Ident.lean`（型・apply・検査・定理）、`IdentExamples.lean`（割り込み・独立・依存・衝突の実行例）。`#print axioms` は `propext` と `Quot.sound`
- Rust に要るもの: `lawean-amend` の `Op` を発射台に対して stable_id に束縛する `bind`、Source IR の参照を id 参照にする、番号の描画
- 「「A」を「B」に改める」が複数の項に当たる場合は、束縛時に id ごとの `replace` に展開する
- 「次のように改める」（全部改正）は `delete` + `insertAfter` に落とす。旧 id が消えるので他法令からの参照切れが正しく出る
- 同じ anchor への 2 つの挿入は独立ではない（順序が番号を決める）。これは正しく、順序を要求する
- id の重複は代数では検査しない。`Revision.wf` を改正単位の終わりで検査する（Rust の項番号の連続性検査と同じ位置）
- 順序依存が消えるわけではない。残るのは依存と衝突だけで、そこが人が調整規定を書く場所

## 関連

[ADR-0010](0010-amendment-first.md)、[ADR-0011](0011-lean-as-reference-for-consolidation.md)、[ADR-0012](0012-structured-authoring.md)、
[08-amendment.md](../08-amendment.md) §5、`lean/README.md`
