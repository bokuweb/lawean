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

`crates/lawean-amend/tests/reiki_fixtures.rs` は、置いたデータに条文が紛れ込んでいないことを見る。`full/` があれば、
旧・新がそろっていることと、`parse_units` で読める改め文の数（いまは 918 件中 61 件。算用数字の横書きは法律の書式と違う）を出す。

## 整合

改め文の字句の組（「A」を「B」に、「A」を削る、「A」の次に「B」を加える）が、どれも改正前・改正後の変わった所に現れる組だけ。
lawean で改め文を当てて確かめてはいない（例規の書式を読めるようにしたら、法律の `bench_apply` と同じく当てて確かめたい）。

改め文（条例・規則の本文）は著作権法第 13 条により権利の目的とならない。
