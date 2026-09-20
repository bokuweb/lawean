# fixtures

| ファイル | 内容 | 取得元 | 取得日 |
|---|---|---|---|
| `403AC0000000090.xml` | 借地借家法（令和4年法律第48号による改正後、施行 2026-05-21） | `https://laws.e-gov.go.jp/api/2/law_data/403AC0000000090?response_format=xml` | 2026-09-20 |

`law_data_response` でラップされている（`law_info` / `revision_info` / `law_full_text`）。法令本体は `law_full_text/Law` 以下。
| `129AC0000000089.xml` | 民法（令和8年法律第45号による改正後、施行 2026-06-24） | 同 API、`law_id=129AC0000000089` | 2026-09-20 |

民法は対象法ではないが、(1) 第138〜143条（期間の計算）が Temporal モデルの前提、(2) 借地借家法に無い要素
（`Part` / `Subsection` / `Division` / `Subitem1` / `ParagraphCaption` / `Ruby` / 枝番 `Num="121_2"` / 削除範囲 `Num="155:157"` / `EnactStatement`）
を含むのでパーサのカバレッジ用に置く。
