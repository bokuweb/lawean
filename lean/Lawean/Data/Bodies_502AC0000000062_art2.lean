import Lawean.Refs

/-!
自動生成: `cargo run -p lawean-lean --example gen`。手で編集しない。

令2-62 第2条が加える第178条・第180条の本文を、起草時の版に当てたリビジョンで id の参照にしたもの（`body::body_of`）。
「第二十八条第五項」「第四項」「第六項」は第28条の項の id を指す
-/

namespace Lawean.Data
open Lawean.Ident

def bodies_502AC0000000062_art2 : Bodies :=
  [ ("502AC0000000062/art2/art:178/new:1", [.text "第二十八条の規定は組合の総会の招集について、第二十九条の規定は組合の総会の議事について、それぞれ準用する。この場合において、", .ref "414AC0000000078/main/chap:2/sec:1/subsec:2/div:3/art:28/para:5" .absoluteArt, .text "中「", .ref "414AC0000000078/main/chap:2/sec:1/subsec:2/div:2/art:9/para:1" .absoluteArt, .text "」とあるのは「", .ref "502AC0000000062/art2/art:168/new:1" .absoluteArt, .text "」と、", .ref "414AC0000000078/main/chap:2/sec:1/subsec:2/div:3/art:29/para:3" .absoluteArt, .text "中「次条」とあるのは「第百七十九条」と読み替えるものとする。"]),
    ("502AC0000000062/art2/art:180/new:1", [.text "組合員の数が五十人を超える組合は、総会に代わってその権限を行わせるために総代会を設けることができる。"]),
    ("502AC0000000062/art2/art:180/new:2", [.text "総代会は、総代をもって組織するものとし、総代の定数は、組合員の総数の十分の一を下らない範囲内において定款で定める。ただし、組合員の総数が二百人を超える組合にあっては、二十人以上であることをもって足りる。"]),
    ("502AC0000000062/art2/art:180/new:3", [.text "総代会が総会に代わって行う権限は、次の各号のいずれかに該当する事項以外の事項に関する総会の権限とする。一理事及び監事の選挙又は選任二前条の規定に従って議決しなければならない事項"]),
    ("502AC0000000062/art2/art:180/new:4", [.ref "414AC0000000078/main/chap:2/sec:1/subsec:2/div:3/art:28/para:1" .absoluteArt, .text "から", .ref "414AC0000000078/main/chap:2/sec:1/subsec:2/div:3/art:28/para:4" .absolute, .text "まで及び", .ref "414AC0000000078/main/chap:2/sec:1/subsec:2/div:3/art:28/para:6" .absolute, .text "並びに第二十九条（", .ref "414AC0000000078/main/chap:2/sec:1/subsec:2/div:3/art:29/para:3" .absolute, .text "ただし書を除く。）の規定は組合の総代会について、", .ref "414AC0000000078/main/chap:2/sec:1/subsec:2/div:3/art:31/para:5" .absoluteArt, .text "の規定は総代会が設けられた組合について、それぞれ準用する。"]) ]

end Lawean.Data
