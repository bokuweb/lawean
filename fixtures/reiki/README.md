# reiki — 例規（条例・規則）の改め文と出典

自治体が公布した例規（横書き）の一部改正の改め文 918 件（5 市、例規 671 本）。
作り方は [`tools/reiki/`](../../tools/reiki/README.md)。totoro の `fixtures/reiki`（改め文と改正前・改正後の条文の組）と同じ case。

| 自治体 | 件数 | 公布の年 | 改め文の出典 |
|---|---|---|---|
| 北九州市 | 255 | 2023–2026 | 北九州市公報 |
| 大和市 | 229 | 2016–2022 | これまでに公布した条例、規則 |
| 静岡市 | 226 | 2021–2026 | 静岡市報 |
| 伊勢崎市 | 122 | 2017–2026 | 最近公布した条例 |
| 川崎市 | 86 | 2025–2026 | 川崎市公報 |

## ここに置くもの・置かないもの

- `cases/<市>.jsonl`（git に入れる）: `case_id`、`verification`、`source`（自治体、改正条例の番号、改め文の PDF の URL、
  被改正の例規の題名と番号、条例Webアーカイブの版の ID）、`checks`、`changed`、`aratamebun`
- `full/<市>.jsonl`（**git に入れない**）: 上に改正前・改正後の条文（`old` / `new`、構造付き plain text）を足したもの。
  条例Webアーカイブの利用規約（「条例Webデータベースの全部または一部を改変して再公開しないこと」）により、公開のリポジトリには置かず、手元で組み立てる:

```sh
python3 tools/reiki/materialize.py        # fixtures/reiki/cases → fixtures/reiki/full（条例Webの版はキャッシュか、無ければ 1 秒に 1 件取る）
cargo test -p lawean-amend --test reiki_fixtures -- --nocapture
```

## 当てて確かめる

`crates/lawean-amend/src/reiki.rs`:
- `to_law_style`: 例規の改め文を法律の書き方に写して `parse_units` で読む（「」の外の数字を漢数字に、号「(1)」「⑴」を「一」に。
  「」の中の字句と加える条文の本文はそのまま）
- `document`: 例規の条文（構造付き plain text）を e-Gov の形に起こす（条・項・号・細目、ただし書、附則、目次、別表）
- `canonical`: 数字・括弧・位取りの幅を揃える（公布された改め文は全角、例規集は 2 桁以上を半角）
- `check`: 改め文を改正前に当て、改正後と突き合わせる

```sh
cargo run --release -p lawean-amend --example reiki_apply -- --out result.tsv
```

| 結果 | 件数 |
|---|---|
| **当てて改正後と一致** | **415（45.2%）** → `apply_baseline.txt` |
| 当てたが改正後と違う | 146 |
| 読めない（様式の改正、PDF の切り出しの崩れ など） | 171 |
| 当てられない（字句・位置・表の行が見つからない） | 123 |
| 未対応の操作 | 63 |

`crates/lawean-amend/tests/reiki_fixtures.rs`:
- 置いたデータに条文（`old` / `new`）が紛れ込んでいないこと（いつも）
- `full/` があれば、全件を当てて、`apply_baseline.txt` の case が 1 件でも一致しなくなったら失敗する（回帰の gate）。
  新しく一致した case は `LAWEAN_REIKI_UPDATE_BASELINE=1` で baseline に足す

## 整合

改め文の字句の組（「A」を「B」に、「A」を削る、「A」の次に「B」を加える）が、どれも改正前・改正後の変わった所に現れる組だけを入れている。
そのうち 415 件は、上のとおり lawean で当てて確かめた。

改め文（条例・規則の本文）は著作権法第 13 条により権利の目的とならない。
