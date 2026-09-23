#!/usr/bin/env python3
"""衆議院「制定法律」の全件（第1回国会〜）を手元のキャッシュに取る。途中から再開できる。

    python3 tools/fetch_shugiin.py [--cache ~/.cache/lawean/shugiin] [--sessions 208-217]

- 国会ごとの一覧 kaijiNNN_l.htm から法律のページ（YYYYMMDD 付きの 14 桁 .htm）を集め、無いものだけ取る
- 取得の間隔は 0.3 秒（サーバーに負荷をかけない）
- 置き場所: <cache>/html/<14 桁>.htm と <cache>/index.tsv（国会、法律番号、URL、題名）
"""
import os
import re
import sys
import time
import urllib.request

BASE = "https://www.shugiin.go.jp/internet/itdb_housei.nsf/html/"


def get(url):
    for i in range(4):
        try:
            with urllib.request.urlopen(url, timeout=60) as r:
                return r.read()
        except Exception as e:  # noqa: BLE001
            if i == 3:
                raise
            time.sleep(2 * (i + 1))


def main():
    args = sys.argv[1:]
    cache = os.path.expanduser("~/.cache/lawean/shugiin")
    lo, hi = 1, 10**6
    if "--cache" in args:
        cache = os.path.expanduser(args[args.index("--cache") + 1])
    if "--sessions" in args:
        a, _, b = args[args.index("--sessions") + 1].partition("-")
        lo, hi = int(a), int(b or a)
    os.makedirs(os.path.join(cache, "html"), exist_ok=True)
    menu = get(BASE + "housei/menu_all.htm").decode("shift_jis", "replace")
    sessions = re.findall(r'html/(housei|houritsu)/kaiji(\d+)_l\.htm', menu)
    rows = []
    for d, n in sessions:
        if not (lo <= int(n) <= hi):
            continue
        idx_path = os.path.join(cache, f"idx_{d}_kaiji{n}_l.htm")
        if not os.path.exists(idx_path):
            with open(idx_path, "wb") as f:
                f.write(get(f"{BASE}{d}/kaiji{n}_l.htm"))
            time.sleep(0.3)
        t = open(idx_path, "rb").read().decode("shift_jis", "replace")
        for m in re.finditer(r'href="(\d{14})\.htm"[^>]*>([^<]*)<', t):
            rows.append((int(n), d, m.group(1), m.group(2).strip()))
    seen = set()
    rows = [r for r in rows if not (r[2] in seen or seen.add(r[2]))]
    rows.sort()
    todo = [r for r in rows if not os.path.exists(os.path.join(cache, "html", r[2] + ".htm"))]
    print(f"laws {len(rows)}, to fetch {len(todo)}", flush=True)
    for i, (n, d, lid, title) in enumerate(todo):
        path = os.path.join(cache, "html", lid + ".htm")
        try:
            b = get(f"{BASE}{d}/{lid}.htm")
        except Exception as e:  # noqa: BLE001
            print(f"skip {lid}: {e}", flush=True)
            continue
        with open(path + ".part", "wb") as f:
            f.write(b)
        os.replace(path + ".part", path)
        if i % 200 == 0:
            print(f"{i}/{len(todo)} {lid}", flush=True)
        time.sleep(0.3)
    with open(os.path.join(cache, "index.tsv"), "w") as f:
        for n, d, lid, title in rows:
            f.write(f"{n}\t{d}\t{lid}\t{title}\n")
    print("done", flush=True)


if __name__ == "__main__":
    main()
