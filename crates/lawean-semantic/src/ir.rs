use lawean_source::StableId;

// ---------------------------------------------------------------- ID

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefinitionId(pub String);

// ---------------------------------------------------------------- Model

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticModel {
    /// どの Source IR（version_id）に対する意味か
    pub document: String,
    pub definitions: Vec<Definition>,
    pub rules: Vec<Rule>,
    /// どの Rule / Definition にも当てはめられなかった文
    pub unknowns: Vec<UnknownNode>,
}

// ---------------------------------------------------------------- Rule

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: RuleId,
    pub subject: Option<EntityRef>,
    pub condition: Expr,
    pub effect: Effect,
    /// この Rule が例外・特則として上書きするもの。宣言する側が持つ（ADR-0004）
    pub overrides: Vec<Override>,
    pub temporal: Option<RuleTemporal>,
    /// 空なら文理解釈のみ（ADR-0005）
    pub interpretations: Vec<Interpretation>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Override {
    Rule(RuleId),
    /// 「契約の条件にかかわらず」— 法令が特約に優先する
    Contract,
}

// ---------------------------------------------------------------- Effect

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Obligation(Action),
    Prohibition(Action),
    /// 「できる」（許容）、「もって足りる」
    Permission(Action),
    /// 「できる」（法律関係を変える権限）。行使したときの効果は `exercise` に
    Power {
        action: Action,
        exercise: Option<Box<Effect>>,
    },
    /// 「〜は、〜とする」
    Set {
        attribute: String,
        value: Value,
    },
    /// 「みなす」— 反証を許さない
    Deem(Fact),
    /// 「推定する」— 反証を許す
    Presume(Fact),
    /// 「無効とする」
    Void(Target),
    /// 「この限りでない」「適用しない」
    Exception(RuleId),
    /// 「前項と同様とする」
    SameAs(RuleId),
    /// 「A を B とみなして、〜の規定を適用する」
    DeemAndApply {
        from: Fact,
        to: Fact,
        apply: RuleId,
    },
    /// 「〜の規定は、〜にも適用する」
    Apply(RefTarget),
    /// 「なお従前の例による」— 廃止・改正前の法令を適用する
    ApplyExternal {
        law: String,
        topic: String,
    },
    /// 「〜について準用する」— 主体を置換して適用
    ApplyMutatis {
        rules: Vec<RuleId>,
        substitute: Vec<(EntityRef, EntityRef)>,
    },
    /// 「〜の効力を妨げない」「なお〜の効力を有する」
    Preserve(Target),
    Unknown(UnknownExpr),
}

/// 行為。`args` は自由語彙（by / to / content / timing / form …）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub verb: String,
    pub args: Vec<(String, Arg)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub pred: Predicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// 特約・契約条項
    Contract(String),
    /// 他 Rule の効果
    RuleEffect(RuleId),
    Fact(Fact),
}

// ---------------------------------------------------------------- Expr

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    True,
    False,
    And(Vec<Expr>),
    Or(Vec<Expr>),
    /// 原文に否定がある場合だけ使う。例外は `overrides` で表す（ADR-0004）
    Not(Box<Expr>),
    Pred(Predicate),
    Cmp(Value, CmpOp, Value),
    Ref(RefTarget),
    Time(TimeCond),
    Unknown(UnknownExpr),
}

/// 述語。`name` と `args` は自由語彙（v0.1）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predicate {
    pub name: String,
    pub args: Vec<(String, Arg)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    Entity(EntityRef),
    Value(Value),
    Expr(Box<Expr>),
    Ref(RefTarget),
    Text(String),
    List(Vec<Arg>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Duration(Duration),
    Period(Period),
    PeriodValue(PeriodValue),
    /// 年利など。分母 1000 の千分率（年一割 = 100）
    RatePerMille(u32),
    /// 事実として外から与えられる変数（「契約で定めた期間」「既払額」）
    Var(String),
    /// 他 Rule の effect が定める値（「これより長い」の「これ」）
    RuleValue(RuleId),
    Add(Box<Value>, Box<Value>),
    Sub(Box<Value>, Box<Value>),
    Interest {
        principal: Box<Value>,
        rate: Box<Value>,
        from: Event,
    },
    Unknown(UnknownExpr),
}

// ---------------------------------------------------------------- Entity

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityRef {
    /// 定義語（「借地権者」）
    Definition(DefinitionId),
    /// 未定義の一般概念（「当事者」「第三者」「裁判所」）
    Named(String),
}

// ---------------------------------------------------------------- Reference

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefTarget {
    Rule(RuleId),
    Definition(DefinitionId),
    /// 「この節」「この法律」→ 構造パス
    Scope(StableId),
    /// 「第十六条」→ 構造パス。Resolved IR で Rule 集合に展開
    Provision(StableId),
    External {
        law: String,
        path: Option<String>,
    },
    /// 「前項」「次条第一項」。Source IR の位置から機械的に解決できるので未解決のままでよい
    Relative(RelativeRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeRef {
    /// 原文の表記（「前項」「前条」「前二項」「前項前段」「同項」）
    pub text: String,
}

// ---------------------------------------------------------------- Temporal

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Hour,
    Day,
    Week,
    Month,
    Year,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    pub length: u32,
    pub unit: Unit,
}

/// 起算点となる事象。値は Fact として外から与える
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Period {
    pub from: Event,
    pub length: Duration,
    pub direction: Direction,
}

/// 「一年前から六月前までの間」
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub from: Period,
    pub to: Period,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeriodValue {
    Definite(Period),
    /// 「期間の定めがない」
    Indefinite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeCond {
    Before(Event),
    After(Event),
    Within(Window),
    /// 「〜の日から六月を経過した後」
    Elapsed(Period),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleTemporal {
    pub effective_from: Option<Event>,
    pub effective_to: Option<Event>,
    /// 経過措置: この時点より前に生じた事実にも（またはだけ）適用する
    pub facts_before: Option<Event>,
}

// ---------------------------------------------------------------- Definition

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub id: DefinitionId,
    pub term: String,
    /// 「この法律において」→ [law root]、「この条において」→ [art:6]、飛び飛びもある（第22条）
    pub scope: Vec<StableId>,
    pub body: DefinitionBody,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionBody {
    Expr(Expr),
    Window(Window),
    Unknown(UnknownExpr),
}

// ---------------------------------------------------------------- Unknown

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownKind {
    /// 法律が意図的に開放的にしている（正当事由、相当、遅滞なく）
    Intentional,
    /// 変換器が理解できなかった
    Unparsed,
    /// 複数解釈があり未選択
    Ambiguous,
    /// 他法令・政令・判例に委ねられている
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownExpr {
    pub kind: UnknownKind,
    /// 該当箇所の原文
    pub text: String,
}

/// Rule にも Definition にもならなかった文
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownNode {
    pub unknown: UnknownExpr,
    pub provenance: Provenance,
}

// ---------------------------------------------------------------- Provenance / Interpretation

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Author {
    Human(String),
    Llm(String),
    Parser(String),
    Precedent(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// Source IR の構造パス。テキストオフセットではない
    pub source: StableId,
    pub confidence: Confidence,
    pub by: Author,
    /// IR に写さなかった譲歩表現など
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authority {
    Literal,
    Precedent(String),
    Administrative(String),
    Doctrine(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interpretation {
    pub authority: Authority,
    pub statement: String,
    pub confidence: Confidence,
}
