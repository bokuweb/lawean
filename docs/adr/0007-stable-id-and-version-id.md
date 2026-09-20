# 0007. ID は `stable_id + version_id`

状態: accepted

## 文脈

改正で条番号が繰り下がる（第22条 → 第22条の2 が挿入される等）。条番号を ID にすると改正のたびに参照が壊れる。
一方、e-Gov の `law_revision_id`（`{law_id}_{施行日}_{改正法令ID}`）は「ある時点の法令」を一意に指す。

## 判断

- `version_id` = e-Gov の `law_revision_id` をそのまま使う
- `stable_id` = そのバージョン内での構造パス（`main/art:3/para:1/sent:2`）
- 改正をまたいだ同一性（lineage）は v0.2 で `stable_id` の上に別途持つ。v0.1 は構造パスのみ

## 理由

- v0.1 は 1 バージョンしか扱わないので構造パスで十分。lineage を先に設計すると改正 patch の設計を先取りすることになる
- e-Gov の ID を捨てないことで、他ツールとの突合が楽

## 結果

- Provenance も Reference も `(version_id, stable_id)` の組を指す。同一 version 内なら stable_id だけ
- 附則の stable_id は `suppl:{index or AmendLawNum}/art:N`。原始附則と本則で条番号が衝突するため

## 関連

[02-source-ir.md](../02-source-ir.md) の stable_id 節
