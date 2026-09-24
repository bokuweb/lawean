#!/usr/bin/env python3
"""改正単位（`bench_apply list` の出力）ごとに、e-Gov の被改正法の「直前の版」と「直後の版」を決めて取ってくる。

    python3 tools/fetch_egov_revisions.py units.tsv pairs.tsv [--cache ~/.cache/lawean/egov] [--no-xml] [--xml-since 20190501]

--xml-since: 版の本文は、公布日（ページの 4〜11 桁目）がこの日以後の改正法の単位の分だけ取る

キャッシュ（再開できる）:
  <cache>/laws.json                 法令番号 → 法令 ID
  <cache>/revisions/<法令ID>.json   改正履歴（e-Gov API v2 law_revisions）
  <cache>/rev/<版ID>.xml.gz         版の本文（law_data、response_format=xml、gzip）

組み合わせ（被改正法 L と改正法 A ごと）:
  L の版を日付順に並べ、A による版（施行前の版も含む）をすべて候補にする。候補は（その版の直前の版, その版）。
  A の版が続いていれば（最初の版の直前の版, 最後の版）も候補にする（単位の中の段階施行）。
  同じ法律を改める単位が複数あるとき、どの版にどの単位が入ったかは当てて確かめる（`bench_apply run`）。
  - A による版が無い: no_revision（e-Gov に無い・廃止済み・古すぎるなど）
  - 候補がどれも直前の版を持たない: no_base
"""
import gzip
import json
import os
import sys
from concurrent.futures import ThreadPoolExecutor
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
xml_since = flag("--xml-since", "")
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
WORKERS = 8


def resolve(num):
    d = get(f"{API}/laws?" + urllib.parse.urlencode({"law_num": num}))
    if d is None:  # 通信の失敗はキャッシュしない（次の実行で取り直す）
        return num, None
    ids = [x["law_info"]["law_id"] for x in d.get("laws", [])]
    return num, (ids[0] if ids else "")


todo_nums = sorted({u["num"] for u in units if u["num"]} - set(laws))
with ThreadPoolExecutor(WORKERS) as ex:
    for k, (num, law_id) in enumerate(ex.map(resolve, todo_nums)):
        if law_id is not None:
            laws[num] = law_id
        if k % 50 == 0:
            json.dump(laws, open(laws_path, "w"), ensure_ascii=False)
            print(f"  laws {k}/{len(todo_nums)}", file=sys.stderr)
json.dump(laws, open(laws_path, "w"), ensure_ascii=False)
print(f"laws: {sum(1 for v in laws.values() if v)} / {len(laws)} resolved", file=sys.stderr)


def revisions(law_id):
    p = os.path.join(cache, "revisions", f"{law_id}.json")
    if not os.path.exists(p):
        d = get(f"{API}/law_revisions/{law_id}")
        if d is None:
            return []
        json.dump(d, open(p, "w"), ensure_ascii=False)
    d = json.load(open(p))
    # API は新しい順。古い順に（同じ日の中の順は API のまま逆に）
    return list(reversed(d.get("revisions", [])))


# 改正履歴を先にまとめて取る
with ThreadPoolExecutor(WORKERS) as ex:
    list(ex.map(revisions, sorted({v for v in laws.values() if v})))

# (被改正法, 改正法) ごとに単位をまとめる
groups = {}
for u in units:
    law_id = laws.get(u["num"], "")
    groups.setdefault((law_id, u["law"]), []).append(u)

rows = []
need = set()
for (law_id, amending), us in groups.items():
    if not law_id:
        rows += [(u, "no_target", []) for u in us]
        continue
    chron = revisions(law_id)
    # 改正法は法令番号で照らす（法令 ID は閣法 AC00…・衆法 AC01…・参法 AC10… で形が違う）
    idx = [i for i, r in enumerate(chron) if r.get("amendment_law_num") == amending]
    if not idx:
        rows += [(u, "no_revision", []) for u in us]
        continue
    cands = [(chron[i - 1]["law_revision_id"], chron[i]["law_revision_id"]) for i in idx if i > 0]
    # 単位の中の段階施行（附則で一部を後から施行）: A の版が続いていれば、最初の版の直前から最後の版まで
    if len(idx) > 1 and idx[0] > 0 and idx[-1] - idx[0] == len(idx) - 1:
        cands.append((chron[idx[0] - 1]["law_revision_id"], chron[idx[-1]]["law_revision_id"]))
    if not cands:
        rows += [(u, "no_base", []) for u in us]
        continue
    for u in us:
        rows.append((u, "ok", cands))
        if u["page"][3:11] >= xml_since:
            for a, b in cands:
                need.update([a, b])

with open(pairs_path, "w", encoding="utf-8") as f:
    f.write("page\tblock\tunit\tstatus\tcandidates\ttitle\n")
    for u, st, cands in sorted(rows, key=lambda r: (r[0]["page"], r[0]["block"], r[0]["unit"])):
        cs = ";".join(f"{a}>{b}" for a, b in cands)
        f.write(f"{u['page']}\t{u['block']}\t{u['unit']}\t{st}\t{cs}\t{u['title']}\n")

stats = {}
for _, st, _ in rows:
    stats[st] = stats.get(st, 0) + 1
print("pairs:", stats, f"revisions needed: {len(need)}", file=sys.stderr)

if not no_xml:
    total = 0
    todo = sorted(r for r in need if not os.path.exists(os.path.join(cache, "rev", f"{r}.xml.gz")))

    def fetch(r):
        data = get(f"{API}/law_data/{r}?response_format=xml", binary=True)
        if data is not None:
            tmp = os.path.join(cache, "rev", f"{r}.xml.gz.tmp")
            with gzip.open(tmp, "wb") as f:
                f.write(data)
            os.replace(tmp, os.path.join(cache, "rev", f"{r}.xml.gz"))
        return len(data or b"")

    with ThreadPoolExecutor(WORKERS) as ex:
        for k, n in enumerate(ex.map(fetch, todo)):
            total += n
            if k % 50 == 0:
                print(f"  {k}/{len(todo)} {total / 1e6:.0f} MB", file=sys.stderr)
