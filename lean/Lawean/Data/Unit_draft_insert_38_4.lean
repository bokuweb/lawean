import Lawean.Ident

/-!
自動生成: `cargo run -p lawean-lean --example gen`。手で編集しない。

自作の改正案 `fixtures/amendments/drafts/insert-38-4.txt`（docs/09 計画 3）: 第38条第3項の次に 1 項を挿入し、以降を繰り下げる。現行 `rev_403AC0000000090_20260521` に束縛。施行令の「第三十八条第四項」がずれる
-/

namespace Lawean.Data
open Lawean.Ident

def unit_draft_insert_38_4 : AmendUnit :=
  [ .insertAfter "403AC0000000090/main/chap:3/sec:3/art:38/para:3" "draft/insert-38-4/art:38/new:1" "38" "建物の賃貸人は、前項の規定による説明をしたときは、その内容を記録した書面を保存しなければならない。" ]

end Lawean.Data
