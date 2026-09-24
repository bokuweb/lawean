#!/usr/bin/env python3
"""自治体の「公布した条例」ページから、一部改正の条例・規則の PDF を取ってくる。

    python3 tools/reiki/fetch_pdfs.py isesaki [--cache ~/.cache/lawean/reiki]

市ごとに、一覧のページと PDF の選び方を `CITIES` に書く。`levels` は一覧から辿るページの段（年 → 月 → 号など）の URL の形。
PDF は `<cache>/<市>/pdf/` に置き、
PDF のファイル名と元の URL を `<cache>/<市>/pdfs.tsv` に書く。1 秒に 1 件
"""
import argparse
import re
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

UA = "Mozilla/5.0 (lawean reiki dataset; research)"

CITIES = {
    # 最近公布した条例（議会ごとに 1 本、平成26年9月から）
    "isesaki": {
        "city": "伊勢崎市",
        "index": ["https://www.city.isesaki.lg.jp/gyoseijoho/jorei_kisoku_yoko/16071.html"],
        "pdf": r"/jourei[^/]*\.pdf$",
        "skip": r"gaiyou",
    },
    # これまでに公布した条例、規則（年ごとのページ、条例・規則ごとに 1 本、平成25年から）
    "yamato": {
        "city": "大和市",
        "index": ["https://www.city.yamato.lg.jp/gyosei/soshik/70/jorei_kisoku/koremadenikofushitajorei_kisoku/index.html"],
        "levels": [r"koremadenikofushitajorei_kisoku/\d+\.html$"],
        "pdf": r"\.pdf$",
        "text": r"改正",
    },
    # 川崎市公報（年度 → 号。告示・公告を含めて 1 件ごとに PDF。令和6年度から）
    "kawasaki": {
        "city": "川崎市",
        "index": [
            "https://www.city.kawasaki.jp/koho/Curr_1.html",
            "https://www.city.kawasaki.jp/koho/2025_1.html",
            "https://www.city.kawasaki.jp/koho/2024_1.html",
        ],
        "levels": [r"/koho/\d{8}_(?:rg|sp)\.html$"],
        "pdf": r"\.pdf$",
        "text": r"改正する(?:条例|規則)",
    },
    # 静岡市報（年度 → 号の PDF。令和3年度から）
    "shizuoka": {
        "city": "静岡市",
        "index": ["https://www.city.shizuoka.lg.jp/p000309.html"],
        "levels": [r"/s4873/s\d+\.html$"],
        "level_text": [r"静岡市報"],
        "pdf": r"\.pdf$",
        "text": r"静岡市報",
    },
    # 横須賀市報（号の PDF が 1 ページに並ぶ）
    "yokosuka": {
        "city": "横須賀市",
        "index": ["https://www.city.yokosuka.kanagawa.jp/1210/shihou.html"],
        "pdf": r"\.pdf$",
        "text": r"横須賀市報",
    },
    # 世田谷区公報（号の PDF）
    "setagaya": {
        "city": "世田谷区",
        "index": ["https://www.city.setagaya.lg.jp/02252/6185.html"],
        "pdf": r"\.pdf$",
        "text": r"第[0-9０-９]+号",
    },
    # 北九州市公報（年 → 月 → 日ごとの号の PDF。平成29年から）
    "kitakyushu": {
        "city": "北九州市",
        "index": ["https://www.city.kitakyushu.lg.jp/shisei/menu05_0008.html"],
        "levels": [r"/shisei/menu05_\d+\.html$", r"/contents/160_\d+\.html$"],
        "level_text": [r"北九州市公報", r"北九州市公報"],
        "pdf": r"/files/\d+\.pdf$",
        "text": r"^第\s*[0-9０-９]+\s*号",
    },
}

_last = [0.0]


def get(url: str) -> bytes:
    wait = 1.0 - (time.time() - _last[0])
    if wait > 0:
        time.sleep(wait)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=60) as r:
        data = r.read()
    _last[0] = time.time()
    return data


def links(url: str):
    s = get(url).decode("utf-8", "replace")
    for h, t in re.findall(r'href="([^"]+)"[^>]*>(.*?)</a>', s, re.S):
        yield urllib.parse.urljoin(url, h), re.sub(r"<[^>]+>", "", t).strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("city", choices=sorted(CITIES))
    ap.add_argument("--cache", default=str(Path.home() / ".cache/lawean/reiki"))
    a = ap.parse_args()
    conf = CITIES[a.city]
    base = Path(a.cache) / a.city
    (base / "pdf").mkdir(parents=True, exist_ok=True)
    pages = list(conf["index"])
    frontier = list(conf["index"])
    for depth, pat in enumerate(conf.get("levels", [])):
        text_pat = (conf.get("level_text") or [None] * 9)[depth]
        nxt = []
        for p in frontier:
            for h, t in links(p):
                h = h.split("#")[0]
                if re.search(pat, h) and (text_pat is None or re.search(text_pat, t)) and h not in pages:
                    pages.append(h)
                    nxt.append(h)
        frontier = nxt
    # PDF は最後の段のページ（段が無ければ一覧）から
    leaf = frontier if conf.get("levels") else pages
    found = {}
    for p in leaf:
        for h, t in links(p):
            if not re.search(conf["pdf"], h):
                continue
            if "skip" in conf and re.search(conf["skip"], h):
                continue
            if "text" in conf and not re.search(conf["text"], t):
                continue
            found[h] = t
    rows = []
    for url, text in sorted(found.items()):
        name = re.sub(r"[^A-Za-z0-9._-]", "_", urllib.parse.urlparse(url).path.rsplit("/", 1)[-1])
        f = base / "pdf" / name
        if not f.exists():
            try:
                f.write_bytes(get(url))
            except Exception as e:  # noqa: BLE001
                print(f"fail {url}: {e}", file=sys.stderr)
                continue
        rows.append(f"{name}\t{url}\t{text}")
    (base / "pdfs.tsv").write_text("\n".join(rows) + "\n")
    print(f"{a.city}: {len(rows)} pdfs from {len(pages)} pages", file=sys.stderr)


if __name__ == "__main__":
    main()
