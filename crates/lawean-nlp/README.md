# lawean-nlp

層 1.5: 文の主体・客体・行為を GiNZA の係り受けから、原文根拠付きの候補（`lawean-extract::candidate::Candidate`）で出す。
ランタイムは [jewel](https://github.com/bokuweb/jewel)（Python 不要）。

## モデル

jewel の exporter で `ja_ginza` 5.2.0 を bundle にする（Python は export のときだけ要る）:

```sh
cd /path/to/jewel
uv run --python 3.11 --with "spacy==3.7.5" --with "ginza==5.2.0" --with "ja-ginza==5.2.0" \
  --with "numpy==1.26.4" --with "click>=8.1,<8.2" --with safetensors \
  python tools/export_spacy_model.py ja_ginza ~/.cache/lawean/ja_ginza.spacy-rs --profile ner --japanese-tokenizer sudachi
export JEWEL_GINZA_BUNDLE=~/.cache/lawean/ja_ginza.spacy-rs
```

bundle は 307MB（Sudachi の辞書込み）。無ければ `Parser::from_env()` が `Err` を返し、テストはスキップする（CI には無い）。

```sh
cargo run --release -p lawean-nlp --example parse -- "借地権者は、借地権設定者に対し、建物の買取りを請求することができる。"
cargo run --release -p lawean-nlp --example subjects -- fixtures/403AC0000000090.xml   # 主語の取れた割合
```
