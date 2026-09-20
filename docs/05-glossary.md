# 05. Glossary — 法令用語 ↔ IR 用語

IR の命名の揺れを防ぐための対応表。英語名は Rust の型名・フィールド名にそのまま使う。

## 構造（Source IR）

| 法令用語 | XML | IR | 備考 |
|---|---|---|---|
| 法令 | `Law` | `LegalDocument` | |
| 法令番号 | `LawNum` | `law_num` | 「平成三年法律第九十号」 |
| 題名 | `LawTitle` | `title` | |
| 本則 | `MainProvision` | `main_provision` | |
| 附則 | `SupplProvision` | `SupplProvision` | 原始附則と改正法附則がある |
| 抄 | `@Extract="true"` | `extract` | 改正法附則のうち本法に関係する部分のみ収録 |
| 編・章・節・款・目 | `Part / Chapter / Section / Subsection / Division` | 同名 | |
| 条 | `Article` | `Article` | |
| 見出し | `ArticleCaption` | `caption` | 「（借地権の存続期間）」 |
| 枝番（第二十二条の二） | `Article @Num="22_2"` | `ArticleNum` | 表記は要確認 |
| 項 | `Paragraph` | `Paragraph` | |
| 号 | `Item` | `Item` | |
| イ・ロ・ハ | `Subitem1` | `Item`（再帰） | |
| 本文 | `Sentence @Function="main"` | `SentenceFunction::Main` | |
| ただし書 | `Sentence @Function="proviso"` | `SentenceFunction::Proviso` | |
| 前段 / 後段 | `Sentence @Num=1 / 2`（Function 無し） | `sentences[0] / [1]` | 「前項前段」のような参照がある |
| 各号列記 | `Item` 列 | `items` | |
| 定義規定の欄 | `Column` | `Column` | 用語 / 定義 のペア |
| 別表 | `AppdxTable` | `Appendix` | v0.1 未着手 |
| 改め文 | `AmendProvision` | — | v0.2 |

## 意味（Semantic IR）

| 法令用語 | 原文の典型表現 | IR | ADR |
|---|---|---|---|
| 義務 | 〜しなければならない | `Effect::Obligation` | |
| 禁止 | 〜してはならない | `Effect::Prohibition` | |
| 許容 | 〜することができる（しても違法でない） | `Effect::Permission` | 0003 |
| 権限・形成権 | 〜することができる（法律関係を変える） | `Effect::Power` | 0003 |
| 法定 / 効果の内容 | 〜は、〜とする | `Effect::Set` | |
| みなす（擬制） | 〜とみなす | `Effect::Deem` | 0002 |
| 推定 | 〜と推定する | `Effect::Presume` | 0002 |
| 無効 | 〜は、無効とする | `Effect::Void` | |
| 例外 | ただし、〜 / この限りでない / 適用しない | `Rule.overrides` + `Effect::Exception` | 0004 |
| 特則 | 〜の規定にかかわらず | `Rule.overrides` | 0004 |
| 準用 | 〜について準用する | `Effect::ApplyMutatis` | |
| 対抗 | 〜をもって第三者に対抗することができる | `Effect::Power`（暫定） | 0003 |
| 足りる | 〜をもって足りる | `Effect::Permission` | |
| 限る | 〜場合に限る / 〜ときに限り | condition（必要条件）。`OnlyIf` 案あり | |
| 譲歩 | 〜ても / 〜がなくても / 〜にかかわらず（契約） | condition に入れない。`Provenance.note` または `Override::Contract` | | |
| 読み替え | 〜を〜とみなして〜を適用する | `Effect::DeemAndApply` | |
| 同様 | 前項と同様とする | `Effect::SameAs`（仮） | |
| 強行規定 | 〜に反する特約で〜に不利なものは無効 | 通常の `Rule` + `Void` | |
| 経過措置 | 施行前に〜 / なお従前の例による | `Temporal`（TODO） | |
| 施行 | この法律は、〜から施行する | `enforced_on` | |
| 定義 | 〜とは、〜をいう / 次の各号に掲げる用語の意義は | `Definition` | |
| 主体 | 借地権者、当事者、第三者 | `Entity` / `EntityRef` | |
| 期間 | 三十年、六月、一年前から | `Temporal::Duration { from }` | |
| 起算点 | 〜の日から | `Duration.from: Event` | |
| 正当事由・相当・遅滞なく | 評価的概念 | `Unknown(Intentional)` | 0001 |
| 解釈 | 文理 / 判例 / 行政解釈 / 学説 | `Interpretation.authority` | 0005 |
| 出所 | — | `Provenance` | |

## 改正（v0.2）

| 法令用語 | IR |
|---|---|
| 改正法 | `AmendingLaw` |
| 改め文 | `Patch` |
| 新旧対照表 | `Diff(old, new)` |
| ハネ改正（他法令の条番号ずれ等に伴う改正） | `CascadedAmendment` |
| 一部改正 / 全部改正 / 廃止 | `PatchKind` |
