#!/usr/bin/env python3
"""衆議院「制定法律」ページ（Shift_JIS の HTML）を fixtures/amendments の平文にする。

    python3 tools/shugiin_text.py <html> [--article 第七条] > out.txt

- 段落は 1 行。字下げは全角空白（衆議院の体裁のまま: 改め文は 2 字下げ、加える条文は 1 字下げ）
- 縦書きの大きな「 」（複数行の目次の字句など）は rowspan の表で組まれているので、中の行をつないで 1 行の「…」にし、
  前の行（「…を」「…に」）と次の行（「に改める。」）と合わせて 1 つの改め文にする
- ルビの (かな) は残す（lawean-amend の normalize_source_text が落とす）
- --article を与えると、その条（「第七条　X法（…）の一部を次のように改正する。」から次の条・附則の前まで）だけ
"""
import html
import re
import sys
from html.parser import HTMLParser


class P(HTMLParser):
    def __init__(self):
        super().__init__()
        self.lines = []  # 出力の行
        self.cur = []  # 今の段落の文字
        self.in_table = 0
        self.table_rows = []
        self.pending_rows = []  # 閉じていない「 の表の行
        self.row = None
        self.cell = None
        self.skip = 0  # script/style
        self.stack = []  # 入れ子の表: 外の表の (行の列, 行, 欄)

    def flush(self):
        t = "".join(self.cur)
        self.cur = []
        if self.in_table and self.cell is not None:
            self.cell.append(t)
            return
        if t.strip("　 \n\r\t"):
            # 閉じない「 の表の後に本文が来た: 続きの表ではなかったので、ためた行はそのまま出す
            if self.pending_rows and not self.in_table:
                self.lines.extend(self.table_lines(self.pending_rows))
                self.pending_rows = []
            self.lines.append(t.rstrip())

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self.skip += 1
        if tag == "table":
            self.flush()
            if self.in_table and self.cell is not None:
                # 欄の中の表（「 」の欄の中に目次の行を組んだ表など）: 外の欄の字句にする
                self.stack.append((self.table_rows, self.row, self.cell))
            self.in_table += 1
            self.table_rows = []
        elif tag == "tr" and self.in_table:
            self.row = []
        elif tag == "td" and self.in_table:
            self.cell = []
        elif tag in ("p", "div", "br", "h1", "h2", "h3", "h4", "li"):
            self.flush()

    def handle_endtag(self, tag):
        if tag in ("script", "style"):
            self.skip -= 1
        if tag in ("p", "div", "h1", "h2", "h3", "h4", "li"):
            self.flush()
        elif tag == "td" and self.in_table and self.cell is not None:
            self.flush()
            if self.row is None:  # <tr> の無い <td>（古いページ）
                self.row = []
            self.row.append("".join(self.cell).strip("\n\r\t "))
            self.cell = None
        elif tag == "tr" and self.in_table:
            if self.row:
                self.table_rows.append(self.row)
            self.row = None
        elif tag == "table" and self.in_table and self.stack:
            self.flush()
            if self.row:
                self.table_rows.append(self.row)
            ws = "\u3000 \xa0\n\r\t"
            inner = "".join(c.strip(ws) for r in self.table_rows for c in r)
            self.table_rows, self.row, self.cell = self.stack.pop()
            self.cell.append(inner)
            self.in_table -= 1
        elif tag == "table" and self.in_table:
            self.in_table -= 1
            rows = self.pending_rows + self.table_rows
            self.table_rows = []
            # 「 が閉じないまま表が終わる（行ごとに表を分けて組んだ「 」）: 」の表まで続けて 1 つの字句に
            ws = "\u3000 \xa0\n\r\t"
            cells = [c.strip(ws).replace("｢", "「").replace("｣", "」") for r in rows for c in r]
            if cells.count("「") > cells.count("」"):
                self.pending_rows = rows
                return
            self.pending_rows = []
            self.lines.extend(self.table_lines(rows))

    def handle_data(self, data):
        if self.skip:
            return
        self.cur.append(data.replace("\r", "").replace("\n", ""))

    @staticmethod
    def table_lines(rows):
        """rowspan の「 」の表なら 1 行の「…」に。それ以外（読替え表など）は欄ごとに 1 行（lawean-amend は表の欄を行で受け取る）"""
        ws = "\u3000 \xa0\n\r\t"
        cells = [c.strip(ws).replace("｢", "「").replace("｣", "」") for r in rows for c in r]
        opens = [c for c in cells if c == "「"]
        closes = [c for c in cells if c == "」"]
        body = [c for c in cells if c not in ("「", "」", "")]
        if opens and closes:
            return ["「" + "".join(body) + "」"]
        return [c for c in cells if c]


def merge_blocks(lines):
    """「…」だけの行を、前の行（…を／…に／…中）と次の行（に改める。／を…）につなぐ。
    表で組んだ字句の間の「及び」「並びに」だけの行も前後とつなぐ"""
    out = []
    i = 0
    while i < len(lines):
        l = lines[i]
        # 「…」」の後の「及び」「並びに」だけの行: 前の行につなぎ、次の「…」も続ける
        if out and out[-1].endswith("」") and l.strip("　 ") in ("及び", "並びに", "、"):
            out[-1] += l.strip("　 ")
            i += 1
            if i < len(lines) and lines[i].startswith("「"):
                out[-1] += lines[i]
                i += 1
                if i < len(lines) and re.match(r"^　?(に|を|、|と|及び)", lines[i]):
                    out[-1] += lines[i].strip("　")
                    i += 1
            continue
        if l.startswith("「") and l.endswith("」") and out and re.search(r"(を|に|中|、|及び|並びに)$", out[-1]):
            out[-1] += l
            # 次の行が続き（字下げ 1 の「に改める。」「を」+「…」の表 +「に改める。」）なら同じ行に。
            # つなぎの語と「…」の行が交互に続く間つなぐ
            j = i + 1
            while j < len(lines):
                nxt = lines[j]
                if nxt.startswith("「") and nxt.endswith("」") and re.search(r"(を|に|中|、|及び|並びに|」)$", out[-1]):
                    out[-1] += nxt
                elif re.match(r"^　?(に|を|、|と|及び|並びに)", nxt) and out[-1].endswith("」"):
                    out[-1] += nxt.strip("　")
                else:
                    break
                j += 1
            i = j
            continue
        out.append(l)
        i += 1
    return out


def convert(data, article=None):
    """HTML のバイト列 → 平文（本文の始まり「◎題名」の次から。`article` を与えればその条だけ）"""
    # meta は Shift_JIS と言っていても UTF-8 のページがある。UTF-8 として読めればそれ
    try:
        raw = data.decode("utf-8")
    except UnicodeDecodeError:
        raw = data.decode("shift_jis", errors="replace")
    p = P()
    p.feed(raw)
    lines = [html.unescape(l) for l in p.lines]
    lines = merge_blocks(lines)
    # 本文の始まり（「◎題名」の次）から
    start = next((i for i, l in enumerate(lines) if l.lstrip("　 ").startswith("◎")), -1) + 1
    lines = lines[start:]
    if article:
        s = next(i for i, l in enumerate(lines) if l.startswith(article + "　"))
        e = next(
            (i for i in range(s + 1, len(lines)) if re.match(r"^第[一二三四五六七八九十百]+条　", lines[i]) or "附　則" in lines[i]),
            len(lines),
        )
        lines = lines[s:e]
        # 直前の見出し「（X法の一部改正）」は含めない、末尾の見出しも落とす
        while lines and re.match(r"^　*(（.+）|第[一二三四五六七八九十]+[編章節]　[^（）]+)$", lines[-1]):
            lines.pop()
    return "\n".join(lines) + "\n"


def main():
    args = sys.argv[1:]
    article = None
    if "--article" in args:
        k = args.index("--article")
        article = args[k + 1]
        del args[k : k + 2]
    sys.stdout.write(convert(open(args[0], "rb").read(), article))


if __name__ == "__main__":
    main()
