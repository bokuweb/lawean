import Lawean.Ident

/-!
自動生成: `cargo run -p lawean-lean --example gen`。手で編集しない。

自作の改正案 `fixtures/amendments/drafts/delete-28.txt`（docs/09 計画 5）: 第28条を削る。`rev_403AC0000000090_20220518` に束縛。高齢者居住安定確保法第58条の「借地借家法第二十八条」が参照切れになる
-/

namespace Lawean.Data
open Lawean.Ident

def unit_draft_delete_28 : AmendUnit :=
  [ .delete "403AC0000000090/main/chap:3/sec:1/art:28/para:1" ]

end Lawean.Data
