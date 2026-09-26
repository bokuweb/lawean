#!/usr/bin/env python3
"""自治体が公開する「公布した条例」の PDF（横書き）から、一部改正条例の改め文を切り出す。

    python3 tools/reiki/pdf_aratamebun.py <pdf のディレクトリ> --city 伊勢崎市 --out amendments.jsonl

- `pdftotext -bbox-layout` で行ごとの座標と語を取り、語を空白なしでつなぐ（両端揃えで字間が空く PDF がある）
- 段落は字下げで組み直す: 左端から始まり、前の行が右端まで埋まっている行は前の段落の続き
- 公布文（「…をここに公布する。」）と「○○市条例第N号」で条例を分け、柱書き（「X条例（…）の一部を次のように改正する。」）
  から附則の前までを改め文とする。「第N条　X条例（…）の一部を次のように改正する。」で複数の例規を改めるものは例規ごとに分ける
- 1 行が 1 件（改正する例規ごと）: {pdf, page, city, ordinance, promulgated, title, target_title, target_num, lines}
"""
import argparse
import html
import json
import re
import subprocess
import sys
from pathlib import Path

ZEN = str.maketrans("０１２３４５６７８９", "0123456789")
KANJI = {c: i for i, c in enumerate("〇一二三四五六七八九")}


GAZETTE_HEADER = re.compile(
    r"^(?:第[0-9０-９,，]+号)?(?:号外)?.{0,8}(?:市報|公報|区報|町報)(?:号外)?(?:第[0-9０-９,，]+号)?(?:[（(]?(?:令和|平成)[0-9０-９元]+年[0-9０-９]+月[0-9０-９]+日[）)]?)?(?:第[0-9０-９,，]+号)?$"
)


def columns(lines):
    """段の頭の x 座標。行頭が多く集まる位置が 2 つ以上あり、どの行もその段の中に収まれば段組み"""
    if len(lines) < 20:
        return [0.0]
    starts = {}
    # 表の升目（短い）は段の頭に数えない
    long = [l for l in lines if len(l[3]) >= 10]
    if len(long) < 20:
        return [0.0]
    for _, x0, _, _ in long:
        k = round(x0 / 10) * 10
        starts[k] = starts.get(k, 0) + 1
    peaks = sorted(k for k, n in starts.items() if n >= len(long) * 0.12)
    heads = []
    for k in peaks:
        if not heads or k - heads[-1] > 100:
            heads.append(k)
    if len(heads) < 2:
        return [0.0]
    heads = [h - 12 for h in heads]
    inside = sum(1 for _, x0, x1, _ in lines if column_of(heads, x1 - 1) == column_of(heads, x0))
    return heads if inside >= len(lines) * 0.9 else [0.0]


def column_of(cols, x):
    return max([i for i, h in enumerate(cols) if x >= h] or [0])


def rows_of(pdf: Path):
    """ページごとの行（y の近い語の塊をまとめたもの）: (page, x0, x1, text)"""
    out = subprocess.run(
        ["pdftotext", "-bbox-layout", str(pdf), "-"], capture_output=True, text=True
    ).stdout
    for pno, page in enumerate(re.findall(r"<page (.*?)</page>", out, re.S), 1):
        lines = []
        split = []
        for x0, y0, x1, y1, body in re.findall(
            r'<line xMin="([\d.]+)" yMin="([\d.]+)" xMax="([\d.]+)" yMax="([\d.]+)">(.*?)</line>',
            page,
            re.S,
        ):
            words = [
                (float(a), float(b), html.unescape(t))
                for a, b, t in re.findall(r'<word xMin="([\d.]+)" yMin="[\d.]+" xMax="([\d.]+)" yMax="[\d.]+">(.*?)</word>', body)
            ]
            text = "".join(w[2] for w in words)
            lines.append((float(y0), float(x0), float(x1), text))
            split.append((float(y0), words))
        cols = columns(lines)
        if len(cols) > 1:
            # pdftotext の行が段をまたぐことがある: 語の位置で段ごとの行に分け直す
            lines = []
            for y0, words in split:
                seg = []
                for w in words:
                    if seg and column_of(cols, w[0]) != column_of(cols, seg[-1][0]):
                        lines.append((y0, seg[0][0], seg[-1][1], "".join(x[2] for x in seg)))
                        seg = []
                    seg.append(w)
                if seg:
                    lines.append((y0, seg[0][0], seg[-1][1], "".join(x[2] for x in seg)))
        # 段組み（公報の 2 段・3 段）は段ごとに上から読む
        lines.sort(key=lambda l: (column_of(cols, l[1]), l[0], l[1]))
        rows = []
        for y, x0, x1, text in lines:
            if rows and abs(rows[-1][0] - y) < 3 and column_of(cols, rows[-1][1]) == column_of(cols, x0):
                r = rows[-1]
                # 同じ高さの離れた塊（表の升目、「附　則」）は全角の空白でつなぐ
                gap = "　" if x0 - r[2] > 8 else ""
                rows[-1] = (r[0], min(r[1], x0), max(r[2], x1), r[3] + gap + text)
            else:
                rows.append((y, x0, x1, text))
        for _, x0, x1, text in rows:
            # ページ番号（「－ 3 －」「3」）
            if re.fullmatch(r"[－\-‑—]?\s*[0-9０-９]+\s*[－\-‑—]?", text.strip()):
                continue
            # 公報の柱（「第24号　静　岡　市　報　令和３年４月１日」）
            if GAZETTE_HEADER.search(re.sub(r"[\s\u3000]", "", text)):
                continue
            yield pno, x0, x1, text


# 段落の頭: 見出し、条、項、号、細目
PARA_HEAD = re.compile(
    r"^(（[^）]{1,40}）$|第[0-9０-９]+条(の[0-9０-９]+)*\u3000|[0-9０-９]+\u3000|[⑴-⒇]|\([0-9０-９]+\)\u3000|[ア-ン]\u3000|[附付]\u3000*則)"
)


def paragraphs(pdf: Path):
    """段落: (page, text)。左端と右端は PDF 全体の行から決める"""
    rows = list(rows_of(pdf))
    if not rows:
        return []
    left = min(r[1] for r in rows)
    # 右端: 長い行の右端で一番多いもの（表やページ番号が右にはみ出しても引きずられない）
    ends = [round(r[2]) for r in rows if len(r[3]) >= 15] or [round(r[2]) for r in rows]
    right = max(set(ends), key=lambda x: sum(1 for e in ends if abs(e - x) <= 3))
    paras = []
    prev_full = False
    for page, x0, x1, text in rows:
        # 前の行が右端まで埋まっていて「。」で終わらず、この行が段落の頭の形でなければ続き
        cont = (
            paras
            and prev_full
            and not paras[-1][1].endswith("。")
            # 題名（「…を改正する条例」）の次は柱書き
            and not re.search(r"(改正する|制定する|定める|関する)(条例|規則)$", paras[-1][1])
            and not PARA_HEAD.match(text)
        )
        if cont:
            paras[-1] = (paras[-1][0], paras[-1][1] + text)
        else:
            paras.append((page, text))
        prev_full = x1 > right - 16
    return paras


def to_int(s: str) -> int | None:
    s = s.translate(ZEN)
    if s.isdigit():
        return int(s)
    if s == "元":
        return 1
    return None


def western(date: str) -> str | None:
    """「令和８年６月３０日」→ 2026/06/30"""
    m = re.match(r"(令和|平成)([0-9０-９元]+)年([0-9０-９]+)月([0-9０-９]+)日", date)
    if not m:
        return None
    base = {"令和": 2018, "平成": 1988}[m.group(1)]
    y, mo, d = to_int(m.group(2)), to_int(m.group(3)), to_int(m.group(4))
    return f"{base + y:04d}/{mo:02d}/{d:02d}"


# 柱書き: 「[第N条　|N　]X条例（…条例第N号）の一部を次のように改正する。」（二度目は法令番号を書かない）
UNIT_HEAD = re.compile(
    r"^(?:(第[0-9０-９]+条)\u3000?|([0-9０-９]+)\u3000)?(.+?)(?:（((?:平成|令和|昭和)[^（）]*?(?:条例|規則)第[0-9０-９]+号)）)?の一部を次のように改正する。$"
)


# 公布文（「…をここに公布する。」「…をここに制定する。」「次に掲げる条例を公布する。」は別の書式なので含めない）
PROMULGATE = re.compile(r"をここに(公布|制定)する。$")


BATCH = re.compile(r"次に掲げる(条例|規則)を公布する。$")


def batch_ordinances(paras, number):
    """「次に掲げる条例を公布する。」+ 番号と題名の一覧 + 題名から始まる本文（世田谷区公報）"""
    for i, (page, t) in enumerate(paras):
        if not BATCH.search(t):
            continue
        date = next((western(paras[j][1].replace("\u3000", "")) for j in range(i + 1, min(len(paras), i + 4)) if western(paras[j][1].replace("\u3000", ""))), None)
        listed = []
        j = i + 1
        while j < len(paras) and j < i + 200:
            m = number(paras[j][1])
            if m:
                # 題名は番号の次の行から「…条例」「…規則」で終わるまで（折り返しをつなぐ）
                title = ""
                k = j + 1
                while k < len(paras) and not number(paras[k][1]) and len(title) < 200:
                    title += paras[k][1].replace("\u3000", "")
                    k += 1
                    if re.search(r"(条例|規則)$", title):
                        break
                listed.append((m.group(1), int(m.group(2).translate(ZEN)), title))
                j = k
            elif listed:
                break
            else:
                j += 1
        # 本文: 一覧の後で題名と同じ行（折り返しをつないだもの）から次の題名まで
        texts = [x for _, x in paras]
        flat = [x.replace("\u3000", "") for x in texts]
        starts = []
        for kind, num, title in listed:
            for k in range(j, len(flat)):
                acc = ""
                for m_ in range(k, min(len(flat), k + 4)):
                    acc += flat[m_]
                    if acc == title:
                        starts.append((k, m_ + 1, kind, num, title))
                        break
                    if not title.startswith(acc):
                        break
                else:
                    continue
                if starts and starts[-1][4] == title:
                    break
        starts.sort()
        for n, (k, body_at, kind, num, title) in enumerate(starts):
            end = starts[n + 1][0] if n + 1 < len(starts) else len(texts)
            yield from units_of(texts[body_at:end], paras[k][0], num, date, title, kind)


def split_ordinances(paras, city):
    """公布文ごとに条例を分け、改め文の単位を返す。公布文の無い PDF（題名から始まる）は番号と日付なしで返す"""
    if not any(PROMULGATE.search(t) or BATCH.search(t) for _, t in paras):
        for i, (page, t) in enumerate(paras):
            title = t.replace("\u3000", "")
            if re.search(r"を改正する(条例|規則)$", title) and i + 1 < len(paras) and UNIT_HEAD.match(paras[i + 1][1]):
                end = next(
                    (k for k in range(i + 2, len(paras)) if re.search(r"を改正する(条例|規則)$", paras[k][1].replace("\u3000", "")) and k + 1 < len(paras) and UNIT_HEAD.match(paras[k + 1][1])),
                    len(paras),
                )
                kind = "規則" if title.endswith("規則") else "条例"
                yield from units_of([x for _, x in paras[i + 1 : end]], page, None, None, title, kind)
        return
    num_re = re.compile(rf"^{re.escape(city)}(条例|規則)第([0-9０-９]+)号$")

    def number(t):
        t = t.replace("\u3000", "")
        # 公報の行頭に付くページ番号（「6静岡市規則第１号」）
        t = re.sub(rf"^[0-9０-９]+(?={re.escape(city)})", "", t)
        return num_re.match(t)

    yield from batch_ordinances(paras, number)
    starts = [i for i, (_, t) in enumerate(paras) if PROMULGATE.search(t)]
    for n, i in enumerate(starts):
        end = starts[n + 1] if n + 1 < len(starts) else len(paras)
        prev_end = starts[n - 1] + 1 if n else 0
        page = paras[i][0]
        date = None
        num = None
        kind = None
        # 番号は公布文の後（伊勢崎市・大和市）か前（静岡市）
        for j in list(range(i + 1, min(end, i + 6))) + list(range(i - 1, max(prev_end, i - 4) - 1, -1)):
            m = number(paras[j][1])
            if m:
                kind, num = m.group(1), int(m.group(2).translate(ZEN))
                break
        for j in range(i + 1, min(end, i + 5)):
            d = western(paras[j][1].replace("\u3000", ""))
            if d:
                date = d
                break
        if num is None:
            continue
        # 題名: 公布文の後の「…条例」「…規則」で終わる行（番号・日付・市長名の後）
        title_at = next(
            (
                j
                for j in range(i + 1, min(end, i + 8))
                if re.search(r"(条例|規則)$", paras[j][1].replace("\u3000", "")) and not number(paras[j][1])
            ),
            None,
        )
        if title_at is None:
            continue
        title = paras[title_at][1].replace("\u3000", "")
        body = [t for _, t in paras[title_at + 1 : end]]
        # 次の条例の番号の行（公布文の前に置く書式）は本文ではない
        while body and number(body[-1]):
            body.pop()
        yield from units_of(body, page, num, date, title, kind)


def units_of(body, page, num, date, title, kind="条例"):
    """柱書きから次の柱書きまで。本則の単位は附則の前まで、附則の中の単位（「２　X条例（…）の一部を…」）は次の項まで"""
    base = {"page": page, "kind": kind, "ordinance": num, "promulgated": date, "title": title}
    known = {}
    in_suppl = False
    cur = None
    suppl_item = None
    for line in body:
        if line.replace("\u3000", "") in ("附則", "付則"):
            in_suppl = True
            if cur:
                yield cur
            cur = None
            continue
        m = UNIT_HEAD.match(line)
        if m and not re.search(r"「|」", m.group(3)):
            if cur:
                yield cur
            target = m.group(3)
            tnum = m.group(4) or known.get(target)
            if m.group(4):
                known[target] = m.group(4)
            suppl_item = to_int(m.group(2)) if m.group(2) else None
            cur = {
                **base,
                "in_suppl": in_suppl,
                "target_title": target,
                "target_num": tnum,
                "lines": [line],
            }
            continue
        if cur is None:
            continue
        if re.fullmatch(r"（.+の一部改正(に伴う経過措置)?）", line):
            continue
        # 附則の中の単位は、改正する条例の次の項（「３　…」）で終わる
        if in_suppl and suppl_item is not None and re.match(
            rf"^[{suppl_item + 1}{str(suppl_item + 1).translate(str.maketrans('0123456789', '０１２３４５６７８９'))}]\u3000", line
        ):
            yield cur
            cur = None
            continue
        if in_suppl and suppl_item is None and re.match(r"^（.+）$", line) and not cur["lines"][-1].endswith("改める。"):
            pass
        cur["lines"].append(line)
    if cur:
        yield cur


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pdf_dir")
    ap.add_argument("--city", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--urls", help="PDF のファイル名と元の URL（fetch_pdfs.py の pdfs.tsv）")
    a = ap.parse_args()
    urls = {}
    if a.urls:
        for line in open(a.urls):
            c = line.rstrip("\n").split("\t")
            if len(c) >= 2:
                urls[c[0]] = c[1]
    n = 0
    with open(a.out, "w") as w:
        for pdf in sorted(Path(a.pdf_dir).glob("*.pdf")):
            for u in split_ordinances(paragraphs(pdf), a.city):
                u = {"pdf": pdf.name, "pdf_url": urls.get(pdf.name), "city": a.city, **u}
                w.write(json.dumps(u, ensure_ascii=False) + "\n")
                n += 1
    print(f"{n} units", file=sys.stderr)


if __name__ == "__main__":
    main()
