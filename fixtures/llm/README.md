# fixtures/llm

層 2（`lawean-llm`）のオフラインテスト用。

| ファイル | 内容 |
|---|---|
| `art5-para1.sample-response.json` | **手書きのサンプル**。`POST /v1/messages` のレスポンスの形（`content[0].text` に JSON、`usage`、`stop_reason`）に合わせて、docs/06 の出力例を入れたもの。実 API の出力ではない |

実 API で取得したレスポンスは `examples/extract_paragraph` が `<paragraph>.response.json` として保存する。取得したら上のサンプルを差し替える。
