#!/usr/bin/env python3
"""`build_cases.py` の出力（市ごとの `<市>_cases.jsonl`）から、整合した組だけを totoro の `fixtures/reiki/cases/<市>.jsonl` に書く。

    python3 tools/reiki/to_totoro.py ~/.cache/lawean/reiki/work ../../willinckii/totoro/fixtures/reiki/cases

- `verification` が `consistent` の組だけ（改め文の字句の組が、どれも改正前・改正後の変わった所に現れる）
- 同じ `case_id` は 1 件（同じ改正条例が同じ例規を 2 度改める段階施行は、1 件目だけ）
- キーの順を揃え、1 行 1 件、`case_id` の順
"""
import collections
import glob
import json
import os
import sys

ORDER = ["case_id", "verification", "source", "checks", "changed", "aratamebun", "old", "new"]


def main():
    src, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    for f in glob.glob(os.path.join(out, "*.jsonl")):
        os.remove(f)
    counts = collections.Counter()
    for f in sorted(glob.glob(os.path.join(src, "*_cases.jsonl"))):
        city = os.path.basename(f)[: -len("_cases.jsonl")]
        cases = [json.loads(line) for line in open(f)]
        seen = set()
        keep = []
        for c in sorted(cases, key=lambda c: c["case_id"]):
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
