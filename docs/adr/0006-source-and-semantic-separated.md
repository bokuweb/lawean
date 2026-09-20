# 0006. 文章構造と意味構造を別ツリーで持つ

状態: accepted

## 文脈

条・項・号のツリー（Source IR）に意味（Rule）を埋め込むと、1 項 = 1 Rule になりがちだが、実際は
1 文から複数 Rule が出る、複数項で 1 Rule になる、別条の Rule が例外になる、など対応は多対多。

## 判断

Source IR と Semantic IR は別のデータ構造にし、`Provenance.source: StableId` でのみ結ぶ。
Source IR は e-Gov XML に対して lossless、Semantic IR は Source IR に対して部分的。

## 理由

- Source IR は意味解析ゼロで作れる。最初に安定させられる資産
- 改正は Source IR の patch として表現でき、その影響を受ける Rule は Provenance の逆引きで求まる
- Semantic IR を差し替えても（人手 → LLM、v0.1 → v0.2）Source IR は変わらない

## 結果

- 「この条文はどの Rule になったか」「この Rule はどの条文から来たか」の両方向の索引が要る
- Semantic IR がカバーしていない文は `unknowns` に置き、カバー率を測れるようにする

## 関連

[02-source-ir.md](../02-source-ir.md)、[04-semantic-ir.md](../04-semantic-ir.md)
