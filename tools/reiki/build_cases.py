#!/usr/bin/env python3
"""例規の改め文（`pdf_aratamebun.py` の出力）に、条例Webアーカイブの改正前・改正後の版を組み合わせ、
totoro のテストデータ（`fixtures/shugiin_egov` と同じ形）を作る。

    python3 tools/reiki/build_cases.py amendments.jsonl --out cases.jsonl [--cache ~/.cache/lawean/reiki/jorei]

- 条例Webアーカイブ（https://jorei.slis.doshisha.ac.jp/）は、全国の例規集を 2016 年 7 月から半年ごとに集めている。
  版ごとに改正の沿革（`reiki_numbers`: 「2025/03/26 条例第6号」）を持つ
- 改正前 = その改正の番号が沿革に無い最後の版、改正後 = 初めて沿革に入った版。
  間に入った改正がその 1 件だけの組を使う（ほかの改正が混ざると、改め文と差分が合わない）
- 本文の HTML は行（段落）と表に起こす。沿革の注記（「一部改正〔平成30年条例30号〕」「(令4条例4・一部改正)」）は落とす
- 変わった所だけを出す: 行の差分が当たった条（見出しから次の条まで）、目次、附則、別表
- 整合の検査: 改め文の「A」を「B」に（削る・加えるも）の A が改正前、B が改正後の変わった所に現れること
  （`verification`: `consistent` / `inconsistent`）。lawean で当てて確かめる仕組みはまだ無い

条例Webの利用規約は「条例Webデータベースの全部または一部を改変して再公開しないこと」。取ってきた版は手元のキャッシュに置く。
条例の本文そのものは著作権法第 13 条により権利の目的とならない
"""
import argparse
import difflib
import hashlib
import html
import json
import re
import sys
import time
import urllib.parse
import urllib.request
from html.parser import HTMLParser
from pathlib import Path

API = "https://jorei.slis.doshisha.ac.jp/api/reiki/select"
UA = "Mozilla/5.0 (lawean reiki dataset; research)"
ZEN = str.maketrans("０１２３４５６７８９．，", "0123456789.,")
_last = [0.0]


def get_json(params, cache: Path):
    key = hashlib.sha1(json.dumps(params, sort_keys=True).encode()).hexdigest()
    f = cache / f"{key}.json"
    if f.exists():
        return json.loads(f.read_text())
    wait = 1.0 - (time.time() - _last[0])
    if wait > 0:
        time.sleep(wait)
    url = API + "?" + urllib.parse.urlencode(params)
    for attempt in range(3):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": UA}), timeout=60) as r:
                data = json.loads(r.read())
            break
        except Exception as e:  # noqa: BLE001
            if attempt == 2:
                raise
            time.sleep(5)
    _last[0] = time.time()
    cache.mkdir(parents=True, exist_ok=True)
    f.write_text(json.dumps(data, ensure_ascii=False))
    return data


def versions(city: str, title: str, cache: Path):
    """同じ例規の版（条例Webの収集時点ごと）を古い順に"""
    # 「・」「、」を含む題名は語句の検索で何も返らない: 記号で区切った一番長い部分で探し、題名の一致は手元で見る
    part = max(re.split(r"[・、，（）()「」　 ]", title), key=len)
    q = f'city:"{city}" AND title:"{part}"'
    data = get_json(
        {"q": q, "rows": 300, "wt": "json", "fl": "id,collection,title,reiki_numbers,updated_date,original_url,municipality_id"},
        cache / "query",
    )
    docs = [d for d in data["response"]["docs"] if d.get("title", "").replace("　", "") == title]
    docs.sort(key=lambda d: d["id"].split("-")[0])
    return docs


def content(doc_id: str, cache: Path) -> str:
    data = get_json({"q": f'id:"{doc_id}"', "wt": "json", "fl": "id,content"}, cache / "content")
    c = data["response"]["docs"][0].get("content", "")
    return c[0] if isinstance(c, list) else c


def norm_number(s: str) -> str:
    """「2003/03/31 条例第15号〕」→「2003/03/31 条例第15号」"""
    return s.translate(ZEN).strip().rstrip("〕").strip()


# --- 本文の HTML → 行と表 -------------------------------------------------

NOTE = re.compile(
    r"^[（(〔][^（）()〔〕]*(?:条例|規則|訓令|告示)[^（）()〔〕]*(?:改正|追加|削除|全改|繰下|繰上|旧)[^（）()〔〕]*[）)〕]$"
    r"|^(?:全部改正|一部改正|追加|削除|本条追加|本条全部改正|旧[^、]*)〔[^〕]*〕(?:、.*)?$"
)


class Blocks(HTMLParser):
    """div / p を行に、table を表に。D1-Law の沿革の注記（id が h:hW）は落とす"""

    def __init__(self):
        super().__init__()
        self.out = []  # ("p", text) | ("table", rows)
        self.buf = []
        self.skip = 0
        self.stack = []
        self.table = None
        self.row = None
        self.cell = None

    def flush(self):
        # 全角の空白（「第１条　」）は残し、HTML の字下げの空白と改行だけを落とす
        t = re.sub(r"[ \t\r\n\xa0]+", "", "".join(self.buf)).strip(" \u3000")
        self.buf = []
        if t and self.table is None:
            self.out.append(("p", t))

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag in ("div", "p", "br", "li"):
            if self.cell is not None:
                if tag != "br" and self.cell:
                    self.cell.append("\n")
            else:
                self.flush()
        if tag == "div":
            hw = (a.get("id") or "").startswith("h:hW")
            self.stack.append(hw)
            if hw:
                self.skip += 1
        if tag in ("rt", "rp", "script", "style"):
            self.skip += 1
        if tag == "table":
            self.flush()
            self.table = []
        elif tag == "tr" and self.table is not None:
            self.row = []
        elif tag in ("td", "th") and self.row is not None:
            self.cell = []

    def handle_endtag(self, tag):
        if tag == "div":
            if self.stack and self.stack.pop():
                self.skip -= 1
            if self.cell is None:
                self.flush()
        elif tag in ("p", "li") and self.cell is None:
            self.flush()
        if tag in ("rt", "rp", "script", "style"):
            self.skip -= 1
        if tag in ("td", "th") and self.cell is not None and self.row is not None:
            self.row.append(re.sub(r"[ \t\r\n]+", "", "".join(self.cell)).strip("\u3000"))
            self.cell = None
        elif tag == "tr" and self.row is not None and self.table is not None:
            if any(self.row):
                self.table.append(self.row)
            self.row = None
        elif tag == "table" and self.table is not None:
            if self.table:
                self.out.append(("table", self.table))
            self.table = None

    def handle_data(self, data):
        if self.skip:
            return
        if self.cell is not None:
            self.cell.append(data)
        else:
            self.buf.append(data)


def blocks_of(html_text: str):
    p = Blocks()
    p.feed(html_text)
    p.flush()
    out = []
    for kind, v in p.out:
        if kind == "p":
            v = html.unescape(v)
            if NOTE.match(v):
                continue
        out.append((kind, v))
    return out


# --- 変わった所 -------------------------------------------------------------

ARTICLE = re.compile(r"^第[0-9０-９]+条(?:の[0-9０-９]+)*(?:　|$)")
CAPTION = re.compile(r"^[（(][^（）()]{1,60}[）)]$")
CONTAINER = re.compile(r"^第[0-9０-９]+(?:編|章|節|款|目)(?:の[0-9０-９]+)?　")
APPENDIX = re.compile(r"^(別表|様式|別記)")


def segments(bs):
    """(鍵, [ブロック]): 目次、条（見出しから）、附則、別表。鍵は「目次」「第34条の7」「附則」「別表第1…」"""
    segs = []
    key = "前文"
    cur = []
    i = 0
    while i < len(bs):
        kind, v = bs[i]
        text = v if kind == "p" else ""
        nxt = bs[i + 1][1] if i + 1 < len(bs) and bs[i + 1][0] == "p" else ""
        new = None
        if text == "目次":
            new = "目次"
        elif key == "目次" and not (ARTICLE.match(text) or CAPTION.match(text)):
            # 目次の行（「第１章　総則（第１条―第５条）」「第２章　市営住宅の管理」「附則」）は目次のうち。
            # 最初の条か見出しで終わる（その前の本文の章名は目次に入る）
            pass
        elif text.replace("　", "") in ("附則", "付則") or re.match(r"^[附付]　*則　*[（(]", text):
            m = re.search(r"[（(].*[）)]$", text)
            new = "附則" + (m.group(0).replace("(", "（").replace(")", "）") if m else "")
        elif APPENDIX.match(text) and len(text) < 80:
            new = text[:20]
        elif CONTAINER.match(text) and not key.startswith("附則"):
            new = "章:" + text
        elif CAPTION.match(text) and ARTICLE.match(nxt):
            new = ("附則" if key.startswith("附則") else "") + re.match(r"^(第[0-9０-９]+条(?:の[0-9０-９]+)*)", nxt).group(1)
        elif ARTICLE.match(text) and not (cur and CAPTION.match(cur[-1][1] if cur[-1][0] == "p" else "")):
            new = ("附則" if key.startswith("附則") else "") + re.match(r"^(第[0-9０-９]+条(?:の[0-9０-９]+)*)", text).group(1)
        if new is not None and cur:
            segs.append((key, cur))
            cur = []
        if new is not None:
            # 同じ鍵が 2 度出たら番号を付ける（改正の附則が並ぶ例規集、同じ番号の条）
            n, base = 2, new
            while any(k == new for k, _ in segs):
                new = f"{base}#{n}"
                n += 1
            key = new
        cur.append((kind, v))
        i += 1
    if cur:
        segs.append((key, cur))
    return segs


def flat(bs):
    return ["P:" + v if k == "p" else "T:" + json.dumps(v, ensure_ascii=False) for k, v in bs]


AMENDING_SUPPL = re.compile(r"^附則([（(].*(条例|規則)第|#)")


def changed(old_bs, new_bs):
    """変わった所の鍵（文書の順）と、改正前・改正後の (鍵, ブロック) の列"""
    so, sn = segments(old_bs), segments(new_bs)
    do, dn = dict(so), dict(sn)
    keys = []
    for k, _ in so + sn:
        if k.startswith("章:") or k == "前文":
            continue
        if flat(do.get(k, [])) != flat(dn.get(k, [])) and k not in keys:
            keys.append(k)
    return keys, so, sn


def context(keys, so, sn):
    """文脈として出す鍵: 変わった所と、その前後の条（「第６条の次に次の１条を加える」の「第６条」）。
    旧・新の両方の並びから選び、両方に同じものを出す（片方だけに出ると追加・削除に見える）"""
    want = set(keys)
    for segs in (so, sn):
        order = [k for k, _ in segs]
        for i, k in enumerate(order):
            if k not in keys:
                continue
            for j in (i - 1, i + 1):
                if 0 <= j < len(order):
                    n = order[j]
                    if n != "前文" and n != "目次" and not n.startswith("章:") and not AMENDING_SUPPL.match(n):
                        want.add(n)
    return want


ITEM_LABEL = re.compile(r"^(?:\(([0-9０-９]+)\)|（([0-9０-９]+)）|([⑴-⒇]))[\u3000 ]?")


def item_label(text: str) -> str:
    """行頭の号の番号を「（２）　」に揃える（例規集は「(２)」、公布された改め文は「⑵」と書く。本文の字句は変えない）"""
    m = ITEM_LABEL.match(text)
    if not m:
        return text
    n = int((m.group(1) or m.group(2) or "0").translate(ZEN)) if not m.group(3) else ord(m.group(3)) - 0x2474 + 1
    full = str(n).translate(str.maketrans("0123456789", "０１２３４５６７８９"))
    return f"（{full}）\u3000" + text[m.end() :]


def render(title, want, segs):
    """文書の順に、出す鍵のブロックを並べる。附則の中の条・項だけを出すときは「附　則」の見出しを付ける"""
    out = [{"type": "paragraph", "text": title}]
    suppl_heading = "附則" in want
    for k, blocks in segs:
        if k not in want:
            continue
        if k.startswith("附則") and not suppl_heading:
            out.append({"type": "paragraph", "text": "附\u3000則"})
            suppl_heading = True
        for kind, v in blocks:
            if kind == "p":
                out.append({"type": "paragraph", "text": item_label(v)})
            else:
                out.append({"type": "table", "rows": v})
    return out


# --- 整合の検査 --------------------------------------------------------------

PAIR = re.compile(r"「([^「」]+)」を「([^「」]*)」に")
DEL = re.compile(r"「([^「」]+)」を削")
ADD = re.compile(r"「([^「」]+)」の(?:次|下)に「([^「」]+)」を加え")


def fold(s: str) -> str:
    """空白を落とし、数字の全角・半角と括弧数字（「⑵」と「(２)」）を揃える"""
    s = re.sub(r"[⑴-⒇]", lambda m: f"({ord(m.group()) - 0x2474 + 1})", s)
    return re.sub(r"[\s\u3000]", "", s).translate(ZEN).replace("（", "(").replace("）", ")")


def consistent(lines, old_text, new_text):
    """改め文の字句の組が、改正前と改正後の変わった所に現れるか。(検査した数, 合わなかったもの)"""
    o, n = fold(old_text), fold(new_text)
    body = "".join(lines[1:])
    checks, bad = 0, []
    # 切り出しの崩れた改正の文（「」改める。」「に改める。」で始まるもの）
    for line in lines[1:]:
        t = line.lstrip("\u3000 ")
        if re.search(r"(改める|加える|削る|とする)。$", t) and re.match(r"^[」、。にをと]", t):
            bad.append(f"malformed: {t[:30]}")
    for a, b in PAIR.findall(body):
        checks += 1
        if fold(a) not in o or (b and fold(b) not in n):
            bad.append(f"「{a}」を「{b}」に")
    for a in DEL.findall(body):
        checks += 1
        if fold(a) not in o:
            bad.append(f"「{a}」を削")
    for a, b in ADD.findall(body):
        checks += 1
        if fold(a) not in o or fold(a + b) not in n and fold(b) not in n:
            bad.append(f"「{a}」の次に「{b}」")
    return checks, bad


def pair_for(key, docs, rn):
    """改正前 = その改正の番号が沿革に無い最後の版、改正後 = 沿革の差がその改正だけの版のうち一番遅いもの。
    例規集は公布の時に沿革へ番号を足し、条文は施行の時に直すので、遅い版のほうが施行の後の見込みが高い。
    組にならなければ理由（skip:…）を返す"""
    has = [key in r for r in rn]
    if True not in has:
        return "skip:not_in_history"
    first = has.index(True)
    if first == 0:
        return "skip:no_before"
    old_d = docs[first - 1]
    only = [d for d, r in zip(docs[first:], rn[first:]) if r - rn[first - 1] == {key}]
    if not only:
        return "skip:other_amendments"
    new_d = only[-1]
    # 例規集のシステムが替わった版（「reiki_honbun/…html」と「…_j.html」）をまたぐと、書式の差が全部の条に出る
    fam = lambda d: "honbun" if "reiki_honbun" in (d.get("original_url") or "") else "other"  # noqa: E731
    if fam(old_d) != fam(new_d):
        return "skip:format_changed"
    return old_d, new_d


def case_id(municipality_id: str, key: str, title: str) -> str:
    """reiki_{自治体コード}_{公布日}_{条例|規則}{番号}_{被改正の題名のハッシュ}"""
    m = re.match(r"(\d{4})/(\d{2})/(\d{2}) (.+?)第(\d+)号", key)
    date, kind, num = (m.group(1) + m.group(2) + m.group(3), m.group(4), m.group(5)) if m else ("", "", "")
    return f"reiki_{municipality_id}_{date}_{kind}{num}_{hashlib.sha1(title.encode()).hexdigest()[:6]}"


def text_of(bs_list):
    return "".join(v if isinstance(v, str) else json.dumps(v, ensure_ascii=False) for _, v in bs_list)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("amendments")
    ap.add_argument("--out", required=True)
    ap.add_argument("--cache", default=str(Path.home() / ".cache/lawean/reiki/jorei"))
    ap.add_argument("--pdf-base", default="", help="PDF の公開 URL の頭（出典に書く）")
    a = ap.parse_args()
    cache = Path(a.cache)
    counts = {}
    n_out = 0

    def count(k):
        counts[k] = counts.get(k, 0) + 1

    with open(a.out, "w") as w:
        for line in open(a.amendments):
            u = json.loads(line)
            if not u.get("target_num"):
                count("skip:no_number")
                continue
            docs = versions(u["city"], u["target_title"], cache)
            if not docs:
                count("skip:no_target")
                continue
            kind = u.get("kind") or ("規則" if "規則" in u["title"][-4:] else "条例")
            rn = [{norm_number(x) for x in d.get("reiki_numbers", [])} for d in docs]
            if u.get("promulgated") and u.get("ordinance"):
                key = f"{u['promulgated']} {kind}第{u['ordinance']}号"
                pair = pair_for(key, docs, rn)
                if isinstance(pair, str):
                    count(pair)
                    continue
            else:
                # 公布文の無い PDF: 間の改正が 1 件の隣り合う版の組を順に試し、改め文の字句がちょうど 1 組とだけ合えば使う
                fits = []
                for i in range(1, len(docs)):
                    added = rn[i] - rn[i - 1]
                    if len(added) != 1 or not rn[i - 1] <= rn[i]:
                        continue
                    o = blocks_of(content(docs[i - 1]["id"], cache))
                    n = blocks_of(content(docs[i]["id"], cache))
                    ks, so_, sn_ = changed(o, n)
                    do_, dn_ = dict(so_), dict(sn_)
                    ks = [k for k in ks if not AMENDING_SUPPL.match(k)]
                    if not ks:
                        continue
                    checks, bad = consistent(
                        u["lines"],
                        text_of([b for k in ks for b in do_.get(k, [])]),
                        text_of([b for k in ks for b in dn_.get(k, [])]),
                    )
                    if checks and not bad:
                        fits.append((docs[i - 1], docs[i], next(iter(added))))
                if len(fits) != 1:
                    count("skip:unnumbered_ambiguous" if fits else "skip:unnumbered_no_fit")
                    continue
                old_d, new_d, key = fits[0]
                pair = (old_d, new_d)
            old_d, new_d = pair
            old_bs = blocks_of(content(old_d["id"], cache))
            new_bs = blocks_of(content(new_d["id"], cache))
            keys, so, sn = changed(old_bs, new_bs)
            do, dn = dict(so), dict(sn)
            # 改正の附則（「附則（令和５年10月３日条例第28号）」）は改め文の外なので比べない
            keys = [k for k in keys if not AMENDING_SUPPL.match(k)]
            if not keys:
                count("skip:no_change")
                continue
            old_text = text_of([b for k in keys for b in do.get(k, [])])
            new_text = text_of([b for k in keys for b in dn.get(k, [])])
            checks, bad = consistent(u["lines"], old_text, new_text)
            verification = "consistent" if not bad else "inconsistent"
            count(f"ok:{verification}")
            title = u["target_title"]
            case = {
                "case_id": case_id(new_d.get("municipality_id", ""), key, title),
                "verification": verification,
                "source": {
                    "city": u["city"],
                    "municipality_id": new_d.get("municipality_id"),
                    "amending_title": u["title"],
                    "amending_num": key,
                    "amending_pdf": u.get("pdf_url") or a.pdf_base + u["pdf"],
                    "target_title": title,
                    "target_num": u["target_num"],
                    "jorei_web_before": old_d["id"],
                    "jorei_web_after": new_d["id"],
                    "original_url": new_d.get("original_url"),
                },
                "checks": {"phrases": checks, "mismatched": bad},
                "changed": keys,
                "aratamebun": u["lines"],
                "old": render(title, context(keys, so, sn), so),
                "new": render(title, context(keys, so, sn), sn),
            }
            w.write(json.dumps(case, ensure_ascii=False) + "\n")
            n_out += 1
    for k in sorted(counts):
        print(f"{k}\t{counts[k]}", file=sys.stderr)
    print(f"cases\t{n_out}", file=sys.stderr)


if __name__ == "__main__":
    main()
