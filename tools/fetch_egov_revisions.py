#!/usr/bin/env python3
"""改正単位（`bench_apply list` の出力）ごとに、e-Gov の被改正法の「直前の版」と「直後の版」を決めて取ってくる。

    python3 tools/fetch_egov_revisions.py units.tsv pairs.tsv [--cache ~/.cache/lawean/egov] [--no-xml] [--max-bytes N]

キャッシュ（再開できる）:
  <cache>/laws.json                 法令番号 → 法令 ID
  <cache>/revisions/<法令ID>.json   改正履歴（e-Gov API v2 law_revisions）
  <cache>/rev/<版ID>.xml            版の本文（law_data、response_format=xml）

組み合わせ（被改正法 L と改正法 A ごと）:
  L の版を日付順に並べ、A による版を取り出す。
  - A による版が無い: no_revision（e-Gov に無い・施行前・廃止済みなど）
  - A による最初の版が L の最初の版: no_base
  - 単位の数と版の数が同じ: k 番目の単位 ↔（k 番目の版の直前の版, k 番目の版）
  - 単位が 1 つで版が続いて複数（段階施行）: （最初の版の直前の版, 最後の版）
  - それ以外: ambiguous
"""
import json
import os
import sys
import time
import urllib.parse
import urllib.request

API = "https://laws.e-gov.go.jp/api/2"
args = sys.argv[1:]


def flag(name, default=None):
    if name in args:
        i = args.index(name)
        v = args[i + 1]
        del args[i : i + 2]
        return v
    return default


cache = os.path.expanduser(flag("--cache", "~/.cache/lawean/egov"))
max_bytes = int(flag("--max-bytes", "0"))
no_xml = "--no-xml" in args
if no_xml:
    args.remove("--no-xml")
units_path, pairs_path = args
os.makedirs(os.path.join(cache, "revisions"), exist_ok=True)
os.makedirs(os.path.join(cache, "rev"), exist_ok=True)


def get(url, binary=False):
    for attempt in range(5):
        try:
            with urllib.request.urlopen(url, timeout=120) as r:
                data = r.read()
            time.sleep(0.2)
            return data if binary else json.loads(data)
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return None
            time.sleep(2 * (attempt + 1))
        except Exception:  # noqa: BLE001
            time.sleep(2 * (attempt + 1))
    return None


units = []
with open(units_path, encoding="utf-8") as f:
    header = f.readline()
    for ln in f:
        c = ln.rstrip("\n").split("\t")
        units.append(dict(page=c[0], block=int(c[1]), unit=int(c[2]), law=c[3], title=c[5], num=c[6]))

# 法令番号 → 法令 ID
laws_path = os.path.join(cache, "laws.json")
laws = json.load(open(laws_path)) if os.path.exists(laws_path) else {}
for num in sorted({u["num"] for u in units if u["num"]} - set(laws)):
    d = get(f"{API}/laws?" + urllib.parse.urlencode({"law_num": num}))
    ids = [x["law_info"]["law_id"] for x in (d or {}).get("laws", [])]
    laws[num] = ids[0] if ids else ""
    json.dump(laws, open(laws_path, "w"), ensure_ascii=False)
print(f"laws: {sum(1 for v in laws.values() if v)} / {len(laws)} resolved", file=sys.stderr)


def revisions(law_id):
    p = os.path.join(cache, "revisions", f"{law_id}.json")
    if not os.path.exists(p):
        d = get(f"{API}/law_revisions/{law_id}")
        json.dump(d or {}, open(p, "w"), ensure_ascii=False)
    d = json.load(open(p))
    # API は新しい順。古い順に（同じ日の中の順は API のまま逆に）
    return list(reversed(d.get("revisions", [])))


# (被改正法, 改正法) ごとに単位をまとめる
groups = {}
for u in units:
    law_id = laws.get(u["num"], "")
    groups.setdefault((law_id, u["law"]), []).append(u)

rows = []
need = set()
for (law_id, amending), us in groups.items():
    us.sort(key=lambda u: (u["page"], u["block"], u["unit"]))
    if not law_id:
        rows += [(u, "no_target", "", "") for u in us]
        continue
    chron = revisions(law_id)
    idx = [i for i, r in enumerate(chron) if r.get("amendment_law_id") == amending]
    if not idx:
        rows += [(u, "no_revision", "", "") for u in us]
        continue
    if idx[0] == 0:
        rows += [(u, "no_base", "", "") for u in us]
        continue
    rid = lambda i: chron[i]["law_revision_id"]  # noqa: E731
    if len(us) == len(idx):
        for u, i in zip(us, idx):
            rows.append((u, "ok", rid(i - 1), rid(i)))
    elif len(us) == 1 and idx[-1] - idx[0] == len(idx) - 1:
        rows.append((us[0], "ok", rid(idx[0] - 1), rid(idx[-1])))
    else:
        rows += [(u, "ambiguous", "", "") for u in us]

for u, st, a, b in rows:
    if st == "ok":
        need.update([a, b])

with open(pairs_path, "w", encoding="utf-8") as f:
    f.write("page\tblock\tunit\tstatus\tprev\tafter\ttitle\n")
    for u, st, a, b in sorted(rows, key=lambda r: (r[0]["page"], r[0]["block"], r[0]["unit"])):
        f.write(f"{u['page']}\t{u['block']}\t{u['unit']}\t{st}\t{a}\t{b}\t{u['title']}\n")

stats = {}
for _, st, _, _ in rows:
    stats[st] = stats.get(st, 0) + 1
print("pairs:", stats, f"revisions needed: {len(need)}", file=sys.stderr)

if not no_xml:
    total = 0
    todo = sorted(r for r in need if not os.path.exists(os.path.join(cache, "rev", f"{r}.xml")))
    for k, r in enumerate(todo):
        data = get(f"{API}/law_data/{r}?response_format=xml", binary=True)
        if data is None:
            continue
        with open(os.path.join(cache, "rev", f"{r}.xml"), "wb") as f:
            f.write(data)
        total += len(data)
        if k % 50 == 0:
            print(f"  {k}/{len(todo)} {total / 1e6:.0f} MB", file=sys.stderr)
        if max_bytes and total > max_bytes:
            print("  --max-bytes reached", file=sys.stderr)
            break
