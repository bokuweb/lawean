import Lawean.SemTheorems

/-!
# Frame 定理 — 改正 × 意味（docs/10 §4、ADR-0016）

改正の前後で Model が変わっても、性質が依存する Rule の集合 S（依存で閉じている）が**同じレコードのまま**両方に入っていれば、
S の Rule の applies と帰結は前後で一致する。したがって S だけから証明した性質は、改正後に**再証明せずに**成り立つ。

S の Rule のレコードが変わる = 改正がその項の本文を変えた（層 2 が抽出し直す）か、新しい Rule がそれを上書きするようになった
（`exceptions` が変わる）か。どちらも「再検証が要る」場合で、そのとき `Sub S m'` が成り立たなくなる。

改正単位 `u` との結びつきは Rust 側（`lawean-lean`）が作る改正後の Model にある: `u` が触らない項の Rule はレコードごと写される。
ここでは「S が両方に同じレコードで入っている」ことをデータについて `rfl` で確かめ、残りをこの定理が引き受ける。
-/

namespace Lawean.Sem

/-- S が依存で閉じている: S の各 Rule の依存先（例外・参照・ruleValue・sameAs）が S の中にある -/
def closed (S : List Rule) : Bool :=
  S.all fun r => r.deps.all fun d => decide (d ∈ S.map (·.id))

/-- S の Rule が m に同じレコードで入っている -/
def Sub (S : List Rule) (m : Model) : Prop := ∀ r ∈ S, m.rule? r.id = some r

/-- S の Rule についてだけの consistent -/
def consistentOn (m : Model) (S : List Rule) (w : World) : Bool :=
  S.all fun r => !(applies m w r.id) || holds m w r.effect

-- rule? ------------------------------------------------------------

theorem rule?_some {m : Model} {id : RuleId} {r : Rule} (h : m.rule? id = some r) : r ∈ m.rules ∧ r.id = id := by
  unfold Model.rule? at h
  refine ⟨List.mem_of_find?_eq_some h, ?_⟩
  have := List.find?_some h
  simpa using this

theorem find_id_of_nodup : ∀ (rs : List Rule) (r : Rule), idsNodup rs = true → r ∈ rs →
    rs.find? (·.id = r.id) = some r
  | [], r, _, h => by simp at h
  | x :: xs, r, hn, h => by
    simp only [idsNodup, Bool.and_eq_true, Bool.not_eq_true', decide_eq_false_iff_not, List.mem_map,
      not_exists, not_and] at hn
    obtain ⟨hn1, hn2⟩ := hn
    rcases List.mem_cons.mp h with rfl | h
    · simp [List.find?_cons]
    · have hne : ¬ (x.id = r.id) := fun e => hn1 r h e.symm
      simp only [List.find?_cons, hne, decide_false]
      exact find_id_of_nodup xs r hn2 h

theorem rule?_of_mem (m : Model) (hwf : m.wf = true) (r : Rule) (hr : r ∈ m.rules) : m.rule? r.id = some r := by
  simp only [Model.wf, Bool.and_eq_true] at hwf
  exact find_id_of_nodup m.rules r hwf.1.1 hr

theorem consistentOn_of_consistent (m : Model) (S : List Rule) (w : World) (hS : Sub S m)
    (h : consistent m w = true) : consistentOn m S w = true := by
  simp only [consistent, consistentOn, List.all_eq_true] at *
  intro r hr
  exact h r (rule?_some (hS r hr)).1

-- Model を取り替えても、参照先が同じなら評価は同じ ------------------------------------------------------------

theorem evalV_congr (m m' : Model) (w : World) : ∀ v : Value,
    (∀ t ∈ v.ruleValues, m.rule? t = m'.rule? t) → evalV m w v = evalV m' w v
  | .int _, _ => rfl
  | .var _, _ => rfl
  | .ruleValue r, h => by
    simp only [evalV]
    rw [h r (by simp [Value.ruleValues])]
  | .add a b, h => by
    simp only [evalV, Value.ruleValues, List.mem_append] at *
    rw [evalV_congr m m' w a (fun t ht => h t (Or.inl ht)), evalV_congr m m' w b (fun t ht => h t (Or.inr ht))]
  | .sub a b, h => by
    simp only [evalV, Value.ruleValues, List.mem_append] at *
    rw [evalV_congr m m' w a (fun t ht => h t (Or.inl ht)), evalV_congr m m' w b (fun t ht => h t (Or.inr ht))]

mutual
  theorem evalE_congr2 (m m' : Model) (w : World) (env1 env2 : Env) : ∀ e : Expr,
      (∀ x ∈ e.refs, env1.get x = env2.get x) → (∀ t ∈ e.ruleValues, m.rule? t = m'.rule? t) →
      evalE m w env1 e = evalE m' w env2 e
    | .tt, _, _ => rfl
    | .ff, _, _ => rfl
    | .and es, hr, hv => by
      simp only [evalE, Expr.refs, Expr.ruleValues] at *; exact evalAll_congr2 m m' w env1 env2 es hr hv
    | .or es, hr, hv => by
      simp only [evalE, Expr.refs, Expr.ruleValues] at *; exact evalAny_congr2 m m' w env1 env2 es hr hv
    | .not e, hr, hv => by
      simp only [evalE, Expr.refs, Expr.ruleValues] at *; rw [evalE_congr2 m m' w env1 env2 e hr hv]
    | .pred _, _, _ => rfl
    | .cmp a op b, _, hv => by
      simp only [evalE, Expr.ruleValues, List.mem_append] at *
      rw [evalV_congr m m' w a (fun t ht => hv t (Or.inl ht)), evalV_congr m m' w b (fun t ht => hv t (Or.inr ht))]
    | .ref r, hr, _ => by simp only [evalE]; exact hr r (by simp [Expr.refs])
    | .unknown _, _, _ => rfl
  theorem evalAll_congr2 (m m' : Model) (w : World) (env1 env2 : Env) : ∀ es : List Expr,
      (∀ x ∈ refsAll es, env1.get x = env2.get x) → (∀ t ∈ ruleValuesAll es, m.rule? t = m'.rule? t) →
      evalAll m w env1 es = evalAll m' w env2 es
    | [], _, _ => rfl
    | e :: es, hr, hv => by
      simp only [evalAll, refsAll, ruleValuesAll, List.mem_append] at *
      rw [evalE_congr2 m m' w env1 env2 e (fun x hx => hr x (Or.inl hx)) (fun t ht => hv t (Or.inl ht)),
          evalAll_congr2 m m' w env1 env2 es (fun x hx => hr x (Or.inr hx)) (fun t ht => hv t (Or.inr ht))]
  theorem evalAny_congr2 (m m' : Model) (w : World) (env1 env2 : Env) : ∀ es : List Expr,
      (∀ x ∈ refsAll es, env1.get x = env2.get x) → (∀ t ∈ ruleValuesAll es, m.rule? t = m'.rule? t) →
      evalAny m w env1 es = evalAny m' w env2 es
    | [], _, _ => rfl
    | e :: es, hr, hv => by
      simp only [evalAny, refsAll, ruleValuesAll, List.mem_append] at *
      rw [evalE_congr2 m m' w env1 env2 e (fun x hx => hr x (Or.inl hx)) (fun t ht => hv t (Or.inl ht)),
          evalAny_congr2 m m' w env1 env2 es (fun x hx => hr x (Or.inr hx)) (fun t ht => hv t (Or.inr ht))]
end

theorem holds_congr (m m' : Model) (w : World) (e : Effect)
    (h : ∀ t ∈ e.deps, m.rule? t = m'.rule? t) : holds m w e = holds m' w e := by
  cases e with
  | set attr v =>
    simp only [holds, Effect.deps] at *
    rw [evalV_congr m m' w v h]
  | sameAs r =>
    simp only [holds, Effect.deps] at *
    rw [h r (by simp)]
  | _ => rfl

-- applies の一致 ------------------------------------------------------------

/-- 閉じた S の依存先 t について、両 Model の `rule?` が一致する -/
theorem rule?_agree (m m' : Model) (S : List Rule) (hS : Sub S m) (hS' : Sub S m') (t : RuleId)
    (ht : t ∈ S.map (·.id)) : m.rule? t = m'.rule? t := by
  obtain ⟨s, hs, rfl⟩ := List.mem_map.mp ht
  rw [hS s hs, hS' s hs]

theorem closed_deps {S : List Rule} (hc : closed S = true) {r : Rule} (hr : r ∈ S) {d : RuleId} (hd : d ∈ r.deps) :
    d ∈ S.map (·.id) := by
  simp only [closed, List.all_eq_true, decide_eq_true_eq] at hc
  exact hc r hr d hd

theorem applies_agree_aux (m m' : Model) (hwf : m.wf = true) (hwf' : m'.wf = true) (S : List Rule)
    (hS : Sub S m) (hS' : Sub S m') (hc : closed S = true) (w : World) :
    ∀ (rs : List Rule) (seen : List RuleId),
      (∀ r ∈ rs, r ∈ m.rules) →
      stratifiedFrom seen rs = true →
      (∀ x ∈ seen, x ∈ S.map (·.id) → applies m w x = applies m' w x) →
      ∀ r ∈ rs, r ∈ S → applies m w r.id = applies m' w r.id
  | [], _, _, _, _, r, hr, _ => by simp at hr
  | r :: rs, seen, hmem, hs, hseen, r', hr', hr'S => by
    simp only [stratifiedFrom, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq] at hs
    obtain ⟨⟨hexc, hrefs⟩, hs'⟩ := hs
    -- r ∈ S なら、この段で一致を示せる
    have step : r ∈ S → applies m w r.id = applies m' w r.id := by
      intro hrS
      have hr_m : r ∈ m.rules := hmem r (by simp)
      have hr_m' : r ∈ m'.rules := (rule?_some (hS' r hrS)).1
      rw [applies_spec m w hwf r hr_m, applies_spec m' w hwf' r hr_m']
      have hdep : ∀ d ∈ r.deps, d ∈ S.map (·.id) := fun d hd => closed_deps hc hrS hd
      simp only [Rule.deps, List.mem_append] at hdep
      congr 1
      · apply evalE_congr2
        · intro x hx
          show applies m w x = applies m' w x
          exact hseen x (hrefs x hx) (hdep x (Or.inl (Or.inl (Or.inr hx))))
        · intro t ht
          exact rule?_agree m m' S hS hS' t (hdep t (Or.inl (Or.inr ht)))
      · apply all_congr'
        intro x hx
        rw [hseen x (hexc x hx) (hdep x (Or.inl (Or.inl (Or.inl hx))))]
    rcases List.mem_cons.mp hr' with rfl | hr'
    · exact step hr'S
    · -- 残りへ。seen に r.id を足す
      have hseen' : ∀ x ∈ seen ++ [r.id], x ∈ S.map (·.id) → applies m w x = applies m' w x := by
        intro x hx hxS
        rcases List.mem_append.mp hx with hx | hx
        · exact hseen x hx hxS
        · rw [List.mem_singleton] at hx
          subst hx
          -- r.id が S の id なら、nodup より r 自身が S に入っている
          obtain ⟨s, hs, hsid⟩ := List.mem_map.mp hxS
          have h1 := hS s hs
          have h2 := rule?_of_mem m hwf r (hmem r (by simp))
          rw [hsid] at h1
          rw [h1] at h2
          have : s = r := Option.some.inj h2
          subst this
          exact step hs
      exact applies_agree_aux m m' hwf hwf' S hS hS' hc w rs (seen ++ [r.id])
        (fun r'' h => hmem r'' (by simp [h])) hs' hseen' r' hr' hr'S

/-- **applies の一致**: 閉じた S が両 Model に同じレコードで入っていれば、S の Rule の applies は一致する -/
theorem applies_agree (m m' : Model) (hwf : m.wf = true) (hwf' : m'.wf = true) (S : List Rule)
    (hS : Sub S m) (hS' : Sub S m') (hc : closed S = true) (w : World) :
    ∀ r ∈ S, applies m w r.id = applies m' w r.id := by
  intro r hr
  have hstrat : stratifiedFrom [] m.rules = true := by
    simp only [Model.wf, Bool.and_eq_true] at hwf; exact hwf.2
  exact applies_agree_aux m m' hwf hwf' S hS hS' hc w m.rules [] (fun _ h => h) hstrat (by simp) r
    (rule?_some (hS r hr)).1 hr

/-- **Frame 定理**: 閉じた S が両 Model に同じレコードで入っていれば、S についての consistent は一致する。
S だけから証明した性質は、改正後にも再証明なしで成り立つ -/
theorem consistentOn_agree (m m' : Model) (hwf : m.wf = true) (hwf' : m'.wf = true) (S : List Rule)
    (hS : Sub S m) (hS' : Sub S m') (hc : closed S = true) (w : World) :
    consistentOn m S w = consistentOn m' S w := by
  unfold consistentOn
  apply all_congr'
  intro r hr
  rw [applies_agree m m' hwf hwf' S hS hS' hc w r hr]
  rw [holds_congr m m' w r.effect]
  intro t ht
  exact rule?_agree m m' S hS hS' t (closed_deps hc hr (by simp [Rule.deps, ht]))

/-- 性質の移送: m で `consistentOn m S w → P w` を示してあれば、m' の consistent から P が出る -/
theorem transfer (m m' : Model) (hwf : m.wf = true) (hwf' : m'.wf = true) (S : List Rule)
    (hS : Sub S m) (hS' : Sub S m') (hc : closed S = true) (P : World → Prop)
    (hP : ∀ w, consistentOn m S w = true → P w) :
    ∀ w, consistent m' w = true → P w := by
  intro w h
  apply hP
  rw [consistentOn_agree m m' hwf hwf' S hS hS' hc w]
  exact consistentOn_of_consistent m' S w hS' h

end Lawean.Sem
