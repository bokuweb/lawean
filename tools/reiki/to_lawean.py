#!/usr/bin/env python3
"""`build_cases.py` の出力から、整合した組を lawean の `fixtures/reiki/cases/<市>.jsonl` に書く。

    python3 tools/reiki/to_lawean.py ~/.cache/lawean/reiki/work fixtures/reiki/cases

lawean は公開リポジトリなので、**条例Webアーカイブの版から取った改正前・改正後の条文（`old` / `new`）は入れない**
（条例Webの利用規約は「全部または一部を改変して再公開しないこと」）。入れるのは、自治体が公布した改め文（PDF から）、
出典（改め文の PDF の URL、条例Webの版の ID）、変わった所の名前、整合の検査の結果。
旧・新の条文は `materialize.py` で手元に組み立てる（`fixtures/reiki/full/`、git に入れない）
"""
import collections
import glob
import json
import os
import sys

ORDER = ["case_id", "verification", "source", "checks", "changed", "aratamebun"]


def main():
    src, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    for f in glob.glob(os.path.join(out, "*.jsonl")):
        os.remove(f)
    counts = collections.Counter()
    for f in sorted(glob.glob(os.path.join(src, "*_cases.jsonl"))):
        city = os.path.basename(f)[: -len("_cases.jsonl")]
        seen = set()
        keep = []
        for c in sorted((json.loads(line) for line in open(f)), key=lambda c: c["case_id"]):
            if c["verification"] != "consistent" or c["case_id"] in seen:
                continue
            seen.add(c["case_id"])
            keep.append({k: c[k] for k in ORDER})
        if not keep:
            continue
        with open(os.path.join(out, f"{city}.jsonl"), "w") as w:
            for c in keep:
                w.write(json.dumps(c, ensure_ascii=False, separators=(",", ":")) + "\n")
        counts[city] = len(keep)
    for city, n in sorted(counts.items()):
        print(f"{city}\t{n}")
    print(f"total\t{sum(counts.values())}")


if __name__ == "__main__":
    main()
