#!/usr/bin/env python3
"""キャッシュの制定法律 HTML（tools/fetch_shugiin.py）を全部平文にする（tools/shugiin_text.py と同じ変換）。

    python3 tools/shugiin_corpus_text.py [--cache ~/.cache/lawean/shugiin]

出力: <cache>/txt/<14 桁>.txt（HTML と変換（shugiin_text.py）と誤記表より新しければ作り直さない）

衆議院のページの誤記は tools/shugiin_errata.tsv（ページの 14 桁、誤、正、注[、回数]）で直してから平文にする。
誤の字句はそのページにちょうど「回数」（省けば 1）回現れなければならない（違えば止める）
"""
import os
import subprocess
import sys

cache = os.path.expanduser("~/.cache/lawean/shugiin")
if "--cache" in sys.argv:
    cache = os.path.expanduser(sys.argv[sys.argv.index("--cache") + 1])
here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import shugiin_text  # noqa: E402

src = os.path.join(cache, "html")
dst = os.path.join(cache, "txt")
os.makedirs(dst, exist_ok=True)
errata_path = os.path.join(here, "shugiin_errata.tsv")
conv = max(os.path.getmtime(os.path.join(here, "shugiin_text.py")), os.path.getmtime(errata_path))
errata = {}
for ln in open(errata_path, encoding="utf-8"):
    if not ln.strip() or ln.startswith("#"):
        continue
    cols = ln.rstrip("\n").split("\t")
    lid, wrong, right = cols[:3]
    times = int(cols[4]) if len(cols) > 4 and cols[4] else 1
    errata.setdefault(lid, []).append((wrong, right, times))
n = 0
for f in sorted(os.listdir(src)):
    if not f.endswith(".htm"):
        continue
    out = os.path.join(dst, f[:-4] + ".txt")
    if os.path.exists(out) and os.path.getmtime(out) >= max(conv, os.path.getmtime(os.path.join(src, f))):
        continue
    try:
        text = shugiin_text.convert(open(os.path.join(src, f), "rb").read())
    except Exception as e:  # noqa: BLE001
        print(f"skip {f}: {e}")
        continue
    for wrong, right, times in errata.get(f[:-4], []):
        k = text.count(wrong)
        if k != times:
            sys.exit(f"errata {f[:-4]}: 「{wrong}」が {k} 回（{times} 回でなければならない）")
        text = text.replace(wrong, right)
    with open(out, "w") as fh:
        fh.write(text)
    n += 1
print(f"converted {n}")
