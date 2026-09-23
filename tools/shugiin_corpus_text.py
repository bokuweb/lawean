#!/usr/bin/env python3
"""キャッシュの制定法律 HTML（tools/fetch_shugiin.py）を全部平文にする（tools/shugiin_text.py と同じ変換）。

    python3 tools/shugiin_corpus_text.py [--cache ~/.cache/lawean/shugiin]

出力: <cache>/txt/<14 桁>.txt（HTML より新しければ作り直さない）
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
n = 0
for f in sorted(os.listdir(src)):
    if not f.endswith(".htm"):
        continue
    out = os.path.join(dst, f[:-4] + ".txt")
    if os.path.exists(out) and os.path.getmtime(out) >= os.path.getmtime(os.path.join(src, f)):
        continue
    try:
        text = shugiin_text.convert(open(os.path.join(src, f), "rb").read())
    except Exception as e:  # noqa: BLE001
        print(f"skip {f}: {e}")
        continue
    with open(out, "w") as fh:
        fh.write(text)
    n += 1
print(f"converted {n}")
