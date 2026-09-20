/-!
# Semantic IR の意味論（ADR-0016、docs/10-lean-semantics.md §2）

法令は Lean の**データ**（`Model`）、意味論はこのファイルの評価器 1 つ。docs/07 の SMT 写像をそのまま Lean に書いたもの:

    applies R  ⇔  cond_R  ∧  ¬ applies_X   (X ∈ exceptions_of R = R を overrides する Rule)
    効果は applies R を前件にした帰結（`holds`）。世界 `w` が全 Rule の帰結を満たすとき `consistent`

再帰は燃料ではなく**層化**で止める: Rust（`ResolvedModel`）が Rule を「例外 → 原則」「参照先 → 参照元」の順に並べ、
評価器はその順に `applies` を積む（`run`）。順序が守られているかは `Model.stratified` で検査する。
上書きの循環（`lawean-space` が検出する異常）があると層化できず、その時点で弾く。
-/

namespace Lawean.Sem

abbrev RuleId := String
/-- 述語名・属性名・変数名。Rust の `pred_name` / `a:属性` と同じ文字列 -/
abbrev Name := String

/-- 値。期間は月数、金額は円（Rust 側で Int に落とす） -/
inductive Value
  | int (n : Int)
  | var (x : Name)
  /-- 他 Rule の `set` が定める値（「これより長い」）。1 段だけ展開する -/
  | ruleValue (r : RuleId)
  | add (a b : Value)
  | sub (a b : Value)
deriving Repr, DecidableEq, Inhabited

inductive CmpOp
  | eq | ne | lt | le | gt | ge
deriving Repr, DecidableEq, Inhabited

inductive Expr
  | tt
  | ff
  | and (es : List Expr)
  | or (es : List Expr)
  | not (e : Expr)
  /-- 述語。不透明で、引数は名前に畳んである（`p:更新する(by=D:借地権者)`）。同じ名前なら同じ Bool 変数 -/
  | pred (name : Name)
  | cmp (a : Value) (op : CmpOp) (b : Value)
  /-- 他 Rule が適用されること -/
  | ref (r : RuleId)
  /-- 自由な Bool 変数（評価概念・Unparsed）。性質の前提に現れる -/
  | unknown (key : Name)
deriving Repr, Inhabited

inductive Effect
  | set (attr : Name) (v : Value)
  | deem (fact : Name)
  | void (target : Name)
  | preserve (target : Name)
  /-- 「この限りでない」「適用しない」。主張は無し（overrides が担う） -/
  | exception (r : RuleId)
  /-- 「前項と同様とする」「〜とみなして〜の規定を適用する」: r の効果をこの Rule の前件で再主張 -/
  | sameAs (r : RuleId)
  /-- Obligation / Permission / … の印。性質からは「その効果が生じた」としてだけ参照できる -/
  | mark (kind : Name)
deriving Repr, DecidableEq, Inhabited

structure Rule where
  id        : RuleId
  cond      : Expr
  effect    : Effect
  /-- この Rule が例外・特則として上書きする Rule -/
  overrides : List RuleId := []
  /-- この Rule を上書きする Rule（`overrides` の逆引き）。Rust が層化順で計算して出す。`Model.wf` が整合を検査する -/
  exceptions : List RuleId := []
  /-- Provenance.stable_id。改正 × 意味の frame 定理で使う -/
  source    : String := ""
  /-- 確度 0..100 -/
  conf      : Nat := 100
deriving Repr, Inhabited

/-- 法令（の一部）の意味。Rule は層化された順 -/
structure Model where
  rules : List Rule
deriving Repr, Inhabited

/-- 世界 = 自由変数への割り当て（Z3 のモデルに当たる）。`bools` は述語・unknown・事実・void・印、`ints` は変数と属性 -/
structure World where
  bools : Name → Bool
  ints  : Name → Int

-- 補助 ------------------------------------------------------------

def Model.rule? (m : Model) (r : RuleId) : Option Rule := m.rules.find? (·.id = r)

/-- r を上書きする Rule（例外）の id -/
def Model.exceptionsOf (m : Model) (r : RuleId) : List RuleId :=
  (m.rules.filter (·.overrides.contains r)).map (·.id)

/-- 積んだ applies の表 -/
abbrev Env := List (RuleId × Bool)

def Env.get (env : Env) (r : RuleId) : Bool :=
  match env.find? (·.1 = r) with
  | some (_, b) => b
  | none => false

-- 評価 ------------------------------------------------------------

/-- 値の評価。`ruleValue` は対象 Rule の `set` を 1 段だけ展開する（Rust の SMT 写像と同じ） -/
def evalV (m : Model) (w : World) : Value → Int
  | .int n => n
  | .var x => w.ints x
  | .ruleValue r =>
    match m.rule? r with
    | some { effect := .set _ v, .. } => evalV1 w v
    | _ => w.ints ("rulevalue:" ++ r)
  | .add a b => evalV m w a + evalV m w b
  | .sub a b => evalV m w a - evalV m w b
where
  /-- 展開先の値。さらに `ruleValue` が出たら変数扱い -/
  evalV1 (w : World) : Value → Int
    | .int n => n
    | .var x => w.ints x
    | .ruleValue r => w.ints ("rulevalue:" ++ r)
    | .add a b => evalV1 w a + evalV1 w b
    | .sub a b => evalV1 w a - evalV1 w b

def CmpOp.eval : CmpOp → Int → Int → Bool
  | .eq, a, b => a == b
  | .ne, a, b => a != b
  | .lt, a, b => a < b
  | .le, a, b => a ≤ b
  | .gt, a, b => a > b
  | .ge, a, b => a ≥ b

mutual
  /-- 条件式の評価。`ref` は積んである applies を引く（無ければ false） -/
  def evalE (m : Model) (w : World) (env : Env) : Expr → Bool
    | .tt => true
    | .ff => false
    | .and es => evalAll m w env es
    | .or es => evalAny m w env es
    | .not e => !(evalE m w env e)
    | .pred name => w.bools name
    | .cmp a op b => op.eval (evalV m w a) (evalV m w b)
    | .ref r => env.get r
    | .unknown key => w.bools key
  def evalAll (m : Model) (w : World) (env : Env) : List Expr → Bool
    | [] => true
    | e :: es => evalE m w env e && evalAll m w env es
  def evalAny (m : Model) (w : World) (env : Env) : List Expr → Bool
    | [] => false
    | e :: es => evalE m w env e || evalAny m w env es
end

/-- 1 つの Rule の applies: 条件が成り立ち、かつ例外がどれも適用されない -/
def applies1 (m : Model) (w : World) (env : Env) (r : Rule) : Bool :=
  evalE m w env r.cond && r.exceptions.all fun x => !(env.get x)

/-- 層化された順に applies を積む -/
def run (m : Model) (w : World) : Env :=
  m.rules.foldl (fun env r => env ++ [(r.id, applies1 m w env r)]) []

/-- Rule r が世界 w で適用されるか -/
def applies (m : Model) (w : World) (r : RuleId) : Bool := (run m w).get r

/-- 効果の帰結。`sameAs` は 1 段だけ辿る -/
def holds (m : Model) (w : World) : Effect → Bool
  | .set attr v => w.ints attr == evalV m w v
  | .deem f => w.bools f
  | .void t => w.bools ("void:" ++ t)
  | .preserve t => w.bools ("preserve:" ++ t)
  | .exception _ => true
  | .sameAs r =>
    match m.rule? r with
    | some { effect := .sameAs _, .. } => w.bools ("eff:" ++ r)
    | some r' => holds1 m w r'.effect
    | none => w.bools ("eff:" ++ r)
  | .mark kind => w.bools ("eff:" ++ kind)
where
  holds1 (m : Model) (w : World) : Effect → Bool
    | .set attr v => w.ints attr == evalV m w v
    | .deem f => w.bools f
    | .void t => w.bools ("void:" ++ t)
    | .preserve t => w.bools ("preserve:" ++ t)
    | .exception _ => true
    | .sameAs r => w.bools ("eff:" ++ r)
    | .mark kind => w.bools ("eff:" ++ kind)

/-- 世界 w が法令 m の模型である: 適用される Rule の帰結がすべて成り立つ -/
def consistent (m : Model) (w : World) : Bool :=
  let env := run m w
  m.rules.all fun r => !(env.get r.id) || holds m w r.effect

-- 層化の検査 ------------------------------------------------------------

mutual
  /-- 式が参照する Rule -/
  def Expr.refs : Expr → List RuleId
    | .and es => refsAll es
    | .or es => refsAll es
    | .not e => e.refs
    | .ref r => [r]
    | _ => []
  def refsAll : List Expr → List RuleId
    | [] => []
    | e :: es => e.refs ++ refsAll es
end

/-- Rule の並びが評価順として正しいか: 各 Rule の例外と参照先が、その Rule より前（`seen`）に積まれている -/
def stratifiedFrom (seen : List RuleId) : List Rule → Bool
  | [] => true
  | r :: rest =>
    r.exceptions.all (fun x => decide (x ∈ seen)) && r.cond.refs.all (fun x => decide (x ∈ seen)) &&
    stratifiedFrom (seen ++ [r.id]) rest

def Model.stratified (m : Model) : Bool := stratifiedFrom [] m.rules

/-- id に重複が無いか -/
def idsNodup : List Rule → Bool
  | [] => true
  | r :: rest => !decide (r.id ∈ rest.map (·.id)) && idsNodup rest

/-- id に重複が無く、`exceptions` が `overrides` の逆引きと一致し、層化されている -/
def Model.wf (m : Model) : Bool :=
  idsNodup m.rules &&
  m.rules.all (fun r => r.exceptions == m.exceptionsOf r.id) &&
  m.stratified

end Lawean.Sem
