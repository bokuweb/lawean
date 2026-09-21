//! 期間の計算（民法第140〜143条）を Z3 の線形算術に落とす（docs/03-examples/minpo-140-143.md の実装）。
//!
//! 日付は (年, 月, 日) の Int の三つ組。暦は SMT-LIB の `define-fun` で書く（閏年、月の日数、翌日、n 月後の応当日）。
//! 規則:
//! - 第140条 初日不算入: 起算日 = 事象の日の翌日（午前零時から始まるときは当日）
//! - 第143条第2項本文: 月・年で定めた期間は、最後の月の起算日に応当する日の**前日**に満了
//! - 同ただし書き: 応当する日が無ければその月の末日に満了
//! - 第142条（休日）は扱わない。遡り（「一年前から六月前まで」）は応当日で逆算する（判例・実務の扱い。docs の論点）
//!
//! ここは Semantic IR とは独立した**期間計算の仕様**で、`TimeCond::Within` 等を日付の制約に展開する土台。
//! `date_expr` の返す式は Z3 の `Int` で、性質はそのまま `check` に足せる。

use std::fmt::Write;

/// 暦の定義（SMT-LIB）。`declare` の後、`assert` の前に一度だけ入れる
pub fn calendar_defs() -> String {
    let mut s = String::new();
    // 閏年
    s.push_str("(define-fun leap ((y Int)) Bool (or (and (= (mod y 4) 0) (not (= (mod y 100) 0))) (= (mod y 400) 0)))\n");
    // 月の日数
    s.push_str(
        "(define-fun dim ((y Int) (m Int)) Int (ite (= m 2) (ite (leap y) 29 28) (ite (or (= m 4) (= m 6) (= m 9) (= m 11)) 30 31)))\n",
    );
    // 日付の妥当性
    s.push_str("(define-fun valid ((y Int) (m Int) (d Int)) Bool (and (>= m 1) (<= m 12) (>= d 1) (<= d (dim y m))))\n");
    // 翌日（年・月・日それぞれ）
    s.push_str("(define-fun next_y ((y Int) (m Int) (d Int)) Int (ite (and (= m 12) (= d 31)) (+ y 1) y))\n");
    s.push_str("(define-fun next_m ((y Int) (m Int) (d Int)) Int (ite (= d (dim y m)) (ite (= m 12) 1 (+ m 1)) m))\n");
    s.push_str(
        "(define-fun next_d ((y Int) (m Int) (d Int)) Int (ite (= d (dim y m)) 1 (+ d 1)))\n",
    );
    // 前日
    s.push_str(
        "(define-fun prev_y ((y Int) (m Int) (d Int)) Int (ite (and (= m 1) (= d 1)) (- y 1) y))\n",
    );
    s.push_str("(define-fun prev_m ((y Int) (m Int) (d Int)) Int (ite (= d 1) (ite (= m 1) 12 (- m 1)) m))\n");
    s.push_str("(define-fun prev_d ((y Int) (m Int) (d Int)) Int (ite (= d 1) (dim (prev_y y m d) (prev_m y m d)) (- d 1)))\n");
    // n 月後の年・月（n は負でもよい）: 通算月 = y*12 + (m-1) + n
    s.push_str(
        "(define-fun add_y ((y Int) (m Int) (n Int)) Int (div (+ (* y 12) (- m 1) n) 12))\n",
    );
    s.push_str(
        "(define-fun add_m ((y Int) (m Int) (n Int)) Int (+ (mod (+ (* y 12) (- m 1) n) 12) 1))\n",
    );
    // 応当日（無ければ月末に丸める = 第143条第2項ただし書き）
    s.push_str("(define-fun corr_d ((y Int) (m Int) (d Int) (n Int)) Int (ite (<= d (dim (add_y y m n) (add_m y m n))) d (dim (add_y y m n) (add_m y m n))))\n");
    // 日付の順序（辞書順）
    s.push_str("(define-fun date_lt ((y1 Int) (m1 Int) (d1 Int) (y2 Int) (m2 Int) (d2 Int)) Bool (or (< y1 y2) (and (= y1 y2) (< m1 m2)) (and (= y1 y2) (= m1 m2) (< d1 d2))))\n");
    s.push_str("(define-fun date_le ((y1 Int) (m1 Int) (d1 Int) (y2 Int) (m2 Int) (d2 Int)) Bool (or (date_lt y1 m1 d1 y2 m2 d2) (and (= y1 y2) (= m1 m2) (= d1 d2))))\n");
    s
}

/// SMT の日付（3 つの Int の式）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateExpr {
    pub y: String,
    pub m: String,
    pub d: String,
}

impl DateExpr {
    pub fn var(name: &str) -> Self {
        DateExpr {
            y: format!("|{name}.y|"),
            m: format!("|{name}.m|"),
            d: format!("|{name}.d|"),
        }
    }
    pub fn lit(y: i32, m: u32, d: u32) -> Self {
        DateExpr {
            y: y.to_string(),
            m: m.to_string(),
            d: d.to_string(),
        }
    }
    pub fn declare(name: &str) -> String {
        format!("(declare-const |{name}.y| Int)\n(declare-const |{name}.m| Int)\n(declare-const |{name}.d| Int)\n(assert (valid |{name}.y| |{name}.m| |{name}.d|))\n")
    }
    fn args(&self) -> String {
        format!("{} {} {}", self.y, self.m, self.d)
    }
    /// 翌日
    pub fn next(&self) -> Self {
        DateExpr {
            y: format!("(next_y {})", self.args()),
            m: format!("(next_m {})", self.args()),
            d: format!("(next_d {})", self.args()),
        }
    }
    /// 前日
    pub fn prev(&self) -> Self {
        DateExpr {
            y: format!("(prev_y {})", self.args()),
            m: format!("(prev_m {})", self.args()),
            d: format!("(prev_d {})", self.args()),
        }
    }
    /// n 月後の応当日（無ければ月末）。n は負でもよい（遡り）
    pub fn corresponding(&self, months: i64) -> Self {
        DateExpr {
            y: format!("(add_y {} {} {months})", self.y, self.m),
            m: format!("(add_m {} {} {months})", self.y, self.m),
            d: format!("(corr_d {} {months})", self.args()),
        }
    }
    pub fn eq(&self, o: &DateExpr) -> String {
        format!(
            "(and (= {} {}) (= {} {}) (= {} {}))",
            self.y, o.y, self.m, o.m, self.d, o.d
        )
    }
    pub fn lt(&self, o: &DateExpr) -> String {
        format!("(date_lt {} {})", self.args(), o.args())
    }
    pub fn le(&self, o: &DateExpr) -> String {
        format!("(date_le {} {})", self.args(), o.args())
    }
}

/// 起算日（第140条）。`from_midnight` なら初日算入
pub fn start_day(event: &DateExpr, from_midnight: bool) -> DateExpr {
    if from_midnight {
        event.clone()
    } else {
        event.next()
    }
}

/// 月・年で定めた期間の満了日（第143条第2項）: 起算日の n 月後の応当日の前日。応当日が無い月はその月の末日
pub fn expiry(start: &DateExpr, months: i64) -> DateExpr {
    // 応当日が無い（起算日の日 > 最後の月の日数）ときは、corr_d が月末に丸めるので「その前日」にしない
    let corr = start.corresponding(months);
    let target_dim = format!("(dim {} {})", corr.y, corr.m);
    let no_corresponding = format!("(> {} {target_dim})", start.d);
    let prev = corr.prev();
    DateExpr {
        y: format!("(ite {no_corresponding} {} {})", corr.y, prev.y),
        m: format!("(ite {no_corresponding} {} {})", corr.m, prev.m),
        d: format!("(ite {no_corresponding} {} {})", corr.d, prev.d),
    }
}

/// 「事象の日から n 月」の満了日（初日不算入）
pub fn expiry_from_event(event: &DateExpr, months: i64) -> DateExpr {
    expiry(&start_day(event, false), months)
}

/// 「満了の n 月前」（遡り。応当日で逆算。docs の論点どおり判例・実務の扱い）
pub fn before(event: &DateExpr, months: i64) -> DateExpr {
    event.corresponding(-months)
}

/// 完全な SMT-LIB: 暦の定義 + 宣言 + 主張。`(check-sat)` で sat なら主張を満たす世界がある
pub fn script(decls: &[String], asserts: &[String]) -> String {
    let mut s = String::from("(set-option :produce-models true)\n(set-logic ALL)\n");
    s.push_str(&calendar_defs());
    for d in decls {
        s.push_str(d);
    }
    for a in asserts {
        let _ = writeln!(s, "(assert {a})");
    }
    s.push_str("(check-sat)\n(get-model)\n");
    s
}
