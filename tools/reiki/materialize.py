#!/usr/bin/env python3
"""lawean の `fixtures/reiki/cases/`（改め文と出典だけ）に、改正前・改正後の条文を手元で足して `fixtures/reiki/full/` に書く。

    python3 tools/reiki/materialize.py [fixtures/reiki/cases] [fixtures/reiki/full] [--cache ~/.cache/lawean/reiki/jorei]

出典の条例Webの版（`source.jorei_web_before` / `jorei_web_after`）を `build_cases.py` と同じ手順で行と表に起こし、
変わった所とその前後の条を構造付き plain text にする（totoro の `fixtures/reiki` と同じ `old` / `new`）。
版はキャッシュから読み、無ければ条例Webアーカイブから取る（1 秒に 1 件）。`fixtures/reiki/full/` は git に入れない
"""
import argparse
import glob
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(__file__))
import build_cases as B  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cases", nargs="?", default="fixtures/reiki/cases")
    ap.add_argument("out", nargs="?", default="fixtures/reiki/full")
    ap.add_argument("--cache", default=str(Path.home() / ".cache/lawean/reiki/jorei"))
    a = ap.parse_args()
    cache = Path(a.cache)
    os.makedirs(a.out, exist_ok=True)
    total = 0
    for f in sorted(glob.glob(os.path.join(a.cases, "*.jsonl"))):
        with open(os.path.join(a.out, os.path.basename(f)), "w") as w:
            for line in open(f):
                c = json.loads(line)
                s = c["source"]
                old_bs = B.blocks_of(B.content(s["jorei_web_before"], cache))
                new_bs = B.blocks_of(B.content(s["jorei_web_after"], cache))
                keys, so, sn = B.changed(old_bs, new_bs)
                keys = [k for k in keys if not B.AMENDING_SUPPL.match(k)]
                want = B.context(keys, so, sn)
                c["old"] = B.render(s["target_title"], want, so)
                c["new"] = B.render(s["target_title"], want, sn)
                w.write(json.dumps(c, ensure_ascii=False, separators=(",", ":")) + "\n")
                total += 1
    print(f"materialized {total} cases into {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
