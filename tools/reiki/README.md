# tools/reiki — 例規の改め文と、改正前・改正後の条文の組を集める

totoro のテストデータ（`fixtures/reiki`）を作る。法律（`bench_apply export`）と違い、例規には改め文を当てて確かめる仕組みがまだ無いので、
改め文の字句の組が改正前・改正後の変わった所に現れるか（整合）で選ぶ。

| スクリプト | すること |
|---|---|
| `fetch_pdfs.py <市>` | 自治体の「公布した条例」・公報のページを辿って PDF を取る（1 秒に 1 件）。市ごとの設定は `CITIES` |
| `pdf_aratamebun.py` | PDF（横書き）から一部改正の条例・規則の改め文を切り出す。行の座標で段落と段組みを組み直す |
| `build_cases.py` | 条例Webアーカイブの版から改正前・改正後を選び、変わった所（と前後の条）を構造付き plain text にし、整合を見る |
| `to_totoro.py` | 整合した組を `fixtures/reiki/cases/<市>.jsonl` に書く |

```sh
python3 tools/reiki/fetch_pdfs.py kitakyushu
python3 tools/reiki/pdf_aratamebun.py ~/.cache/lawean/reiki/kitakyushu/pdf --city 北九州市 \
  --urls ~/.cache/lawean/reiki/kitakyushu/pdfs.tsv --out ~/.cache/lawean/reiki/work/kitakyushu_amend.jsonl
python3 tools/reiki/build_cases.py ~/.cache/lawean/reiki/work/kitakyushu_amend.jsonl --out ~/.cache/lawean/reiki/work/kitakyushu_cases.jsonl
python3 tools/reiki/to_totoro.py ~/.cache/lawean/reiki/work <totoro>/fixtures/reiki/cases
```

PDF の書式の違い（公布文の前後の条例番号、「制定する」、「次に掲げる条例を公布する。」の一覧、行頭のページ番号、公報の柱、3 段組み、
「付則」）は `pdf_aratamebun.py` で吸収している。条例Webの版は `~/.cache/lawean/reiki/jorei/` にキャッシュする（1 秒に 1 件）。

条例Webアーカイブの利用規約は「改変して再公開しないこと」。取ってきた版は手元に置き、作ったデータを公開する前に先方に確認する。
条例・規則の本文と改め文そのものは著作権法第 13 条により権利の目的とならない。
