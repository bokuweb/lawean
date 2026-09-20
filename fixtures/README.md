# fixtures

| ファイル | 内容 | 取得元 | 取得日 |
|---|---|---|---|
| `403AC0000000090.xml` | 借地借家法（令和4年法律第48号による改正後、施行 2026-05-21） | `https://laws.e-gov.go.jp/api/2/law_data/403AC0000000090?response_format=xml` | 2026-09-20 |

`law_data_response` でラップされている（`law_info` / `revision_info` / `law_full_text`）。法令本体は `law_full_text/Law` 以下。
| `129AC0000000089.xml` | 民法（令和8年法律第45号による改正後、施行 2026-06-24） | 同 API、`law_id=129AC0000000089` | 2026-09-20 |

民法は対象法ではないが、(1) 第138〜143条（期間の計算）が Temporal モデルの前提、(2) 借地借家法に無い要素
（`Part` / `Subsection` / `Division` / `Subitem1` / `ParagraphCaption` / `Ruby` / 枝番 `Num="121_2"` / 削除範囲 `Num="155:157"` / `EnactStatement`）
を含むのでパーサのカバレッジ用に置く。

## revisions/

借地借家法の過去・未来のリビジョン（`https://laws.e-gov.go.jp/api/2/law_data/{law_revision_id}`、取得日 2026-09-20）。
一覧は `https://laws.e-gov.go.jp/api/2/law_revisions/403AC0000000090`。2017 年以降の 9 版のうち現行以外の 8 版を置く（現行は `403AC0000000090.xml`）。

## amendments/

改正法本文のうち借地借家法を改正する条。e-Gov の law_data は溶け込み後の本文しか持たないので、衆議院「制定法律」ページ（Shift_JIS の HTML）から抽出した。

| ファイル | 出典 |
|---|---|
| `503AC0000000037_art35.txt` | デジタル社会の形成を図るための関係法律の整備に関する法律（令和3年法律第37号）第35条 |
| `504AC0000000048_art73-74.txt` | 民事訴訟法等の一部を改正する法律（令和4年法律第48号）第73条・第74条 |
