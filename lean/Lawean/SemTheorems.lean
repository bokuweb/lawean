import Lawean.Sem

/-!
# 評価器のメタ定理（docs/10）

性質の証明を**局所的**にするための定理。`consistent` を全 Rule について `simp` で展開すると Rule 数の 2 乗で膨らむので、
「wf な Model では、各 Rule の applies はその条件と例外だけで決まる」（`applies_spec`）を一度証明し、
個々の性質は関係する Rule だけを取り出して展開する。
-/

namespace Lawean.Sem

-- Env ------------------------------------------------------------

def Env.keys (env : Env) : List RuleId := env.map (·.1)

theorem Env.get_append_of_mem (env e2 : Env) (x : RuleId) (h : x ∈ env.keys) :
    (env ++ e2).get x = env.get x := by
  unfold Env.get
  rw [List.find?_append]
  cases hf : env.find? (·.1 = x) with
  | some v => simp
  | none =>
    exfalso
    rw [List.find?_eq_none] at hf
    simp only [Env.keys, List.mem_map] at h
    obtain ⟨⟨k, b⟩, hm, rfl⟩ := h
    exact absurd (by simp : (fun p : RuleId × Bool => decide (p.1 = k)) (k, b) = true) (by simpa using hf (k, b) hm)

theorem Env.get_append_single (env : Env) (k : RuleId) (b : Bool) (h : k ∉ env.keys) :
    (env ++ [(k, b)]).get k = b := by
  unfold Env.get
  rw [List.find?_append]
  have : env.find? (·.1 = k) = none := by
    rw [List.find?_eq_none]
    intro p hp
    simp only [Env.keys, List.mem_map] at h
    simpa using fun e => h ⟨p, hp, e⟩
  simp [this]

theorem Env.keys_append (env e2 : Env) : (env ++ e2).keys = env.keys ++ e2.keys := by
  simp [Env.keys]

-- evalE は参照する id の値にしか依存しない ------------------------------------------------------------

mutual
  theorem evalE_congr (m : Model) (w : World) (env1 env2 : Env) :
      ∀ e : Expr, (∀ x ∈ e.refs, env1.get x = env2.get x) → evalE m w env1 e = evalE m w env2 e
    | .tt, _ => rfl
    | .ff, _ => rfl
    | .and es, h => by simp only [evalE, Expr.refs] at *; exact evalAll_congr m w env1 env2 es h
    | .or es, h => by simp only [evalE, Expr.refs] at *; exact evalAny_congr m w env1 env2 es h
    | .not e, h => by simp only [evalE, Expr.refs] at *; rw [evalE_congr m w env1 env2 e h]
    | .pred _, _ => rfl
    | .cmp _ _ _, _ => rfl
    | .ref r, h => by simp only [evalE]; exact h r (by simp [Expr.refs])
    | .unknown _, _ => rfl
  theorem evalAll_congr (m : Model) (w : World) (env1 env2 : Env) :
      ∀ es : List Expr, (∀ x ∈ refsAll es, env1.get x = env2.get x) → evalAll m w env1 es = evalAll m w env2 es
    | [], _ => rfl
    | e :: es, h => by
      simp only [evalAll, refsAll, List.mem_append] at *
      rw [evalE_congr m w env1 env2 e (fun x hx => h x (Or.inl hx)),
          evalAll_congr m w env1 env2 es (fun x hx => h x (Or.inr hx))]
  theorem evalAny_congr (m : Model) (w : World) (env1 env2 : Env) :
      ∀ es : List Expr, (∀ x ∈ refsAll es, env1.get x = env2.get x) → evalAny m w env1 es = evalAny m w env2 es
    | [], _ => rfl
    | e :: es, h => by
      simp only [evalAny, refsAll, List.mem_append] at *
      rw [evalE_congr m w env1 env2 e (fun x hx => h x (Or.inl hx)),
          evalAny_congr m w env1 env2 es (fun x hx => h x (Or.inr hx))]
end

theorem all_congr' {α : Type} (f g : α → Bool) : ∀ (l : List α), (∀ x ∈ l, f x = g x) → l.all f = l.all g
  | [], _ => rfl
  | x :: xs, h => by
    simp only [List.all_cons]
    rw [h x (by simp), all_congr' f g xs (fun y hy => h y (by simp [hy]))]

theorem applies1_congr (m : Model) (w : World) (env1 env2 : Env) (r : Rule)
    (hc : ∀ x ∈ r.cond.refs, env1.get x = env2.get x)
    (he : ∀ x ∈ r.exceptions, env1.get x = env2.get x) :
    applies1 m w env1 r = applies1 m w env2 r := by
  unfold applies1
  rw [evalE_congr m w env1 env2 r.cond hc]
  congr 1
  apply all_congr'
  intro x hx
  rw [he x hx]

-- run の仕様 ------------------------------------------------------------

def runFrom (m : Model) (w : World) (env : Env) (rs : List Rule) : Env :=
  rs.foldl (fun env r => env ++ [(r.id, applies1 m w env r)]) env

theorem run_eq_runFrom (m : Model) (w : World) : run m w = runFrom m w [] m.rules := rfl

theorem runFrom_cons (m : Model) (w : World) (env : Env) (r : Rule) (rs : List Rule) :
    runFrom m w env (r :: rs) = runFrom m w (env ++ [(r.id, applies1 m w env r)]) rs := rfl

theorem runFrom_keys (m : Model) (w : World) : ∀ (rs : List Rule) (env : Env),
    (runFrom m w env rs).keys = env.keys ++ rs.map (·.id)
  | [], env => by simp [runFrom]
  | r :: rs, env => by
    rw [runFrom_cons, runFrom_keys m w rs]
    simp [Env.keys_append, Env.keys]

/-- 積んだ後は、前からある id の値は変わらない -/
theorem runFrom_get_old (m : Model) (w : World) : ∀ (rs : List Rule) (env : Env) (x : RuleId),
    x ∈ env.keys → (runFrom m w env rs).get x = env.get x
  | [], env, x, _ => rfl
  | r :: rs, env, x, hx => by
    rw [runFrom_cons, runFrom_get_old m w rs _ x (by rw [Env.keys_append]; exact List.mem_append_left _ hx)]
    exact Env.get_append_of_mem env _ x hx

/-- 層化された順に積むと、各 Rule の値は最終的な表に対する `applies1` と一致する -/
theorem runFrom_spec (m : Model) (w : World) : ∀ (rs : List Rule) (env : Env),
    stratifiedFrom env.keys rs = true →
    idsNodup rs = true →
    (∀ r ∈ rs, r.id ∉ env.keys) →
    ∀ r ∈ rs, (runFrom m w env rs).get r.id = applies1 m w (runFrom m w env rs) r
  | [], _, _, _, _, _, h => by simp at h
  | r :: rs, env, hs, hn, hfresh, r', hr' => by
    simp only [stratifiedFrom, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq] at hs
    obtain ⟨⟨hexc, hrefs⟩, hs'⟩ := hs
    simp only [idsNodup, Bool.and_eq_true, Bool.not_eq_true', decide_eq_false_iff_not, List.mem_map,
      not_exists, not_and] at hn
    obtain ⟨hn1, hn2⟩ := hn
    have hfr : r.id ∉ env.keys := hfresh r (by simp)
    rw [runFrom_cons]
    generalize henv' : env ++ [(r.id, applies1 m w env r)] = env'
    have hkeys' : env'.keys = env.keys ++ [r.id] := by rw [← henv']; simp [Env.keys_append, Env.keys]
    have hfresh' : ∀ r'' ∈ rs, r''.id ∉ env'.keys := by
      intro r'' h''
      rw [hkeys', List.mem_append, List.mem_singleton]
      rintro (h | h)
      · exact hfresh r'' (by simp [h'']) h
      · exact hn1 r'' h'' h
    have hs'' : stratifiedFrom env'.keys rs = true := by rw [hkeys']; exact hs'
    -- 前から積んである id の値は最終表でも同じ
    have hold : ∀ x ∈ env.keys, (runFrom m w env' rs).get x = env.get x := by
      intro x hx
      rw [runFrom_get_old m w rs env' x (by rw [hkeys']; exact List.mem_append_left _ hx)]
      rw [← henv']
      exact Env.get_append_of_mem env _ x hx
    rcases List.mem_cons.mp hr' with rfl | hr'
    · -- r 自身
      have hget : (runFrom m w env' rs).get r'.id = applies1 m w env r' := by
        rw [runFrom_get_old m w rs env' r'.id (by rw [hkeys']; simp)]
        rw [← henv']
        exact Env.get_append_single env r'.id _ hfr
      rw [hget]
      apply applies1_congr
      · intro x hx
        exact (hold x (hrefs x hx)).symm
      · intro x hx
        exact (hold x (hexc x hx)).symm
    · exact runFrom_spec m w rs env' hs'' hn2 hfresh' r' hr'

/-- **applies の仕様**: wf な Model では、Rule の applies は「条件が最終表で成り立ち、例外がどれも適用されない」に等しい -/
theorem applies_spec (m : Model) (w : World) (hwf : m.wf = true) (r : Rule) (hr : r ∈ m.rules) :
    applies m w r.id = (evalE m w (run m w) r.cond && r.exceptions.all fun x => !(applies m w x)) := by
  simp only [Model.wf, Bool.and_eq_true] at hwf
  obtain ⟨⟨hn, _⟩, hs⟩ := hwf
  have := runFrom_spec m w m.rules [] hs hn (by simp [Env.keys]) r hr
  simp only [applies, run_eq_runFrom]
  rw [this]
  rfl

/-- **consistent の仕様**: 適用される Rule の帰結は成り立つ -/
theorem consistent_rule (m : Model) (w : World) (h : consistent m w = true) (r : Rule) (hr : r ∈ m.rules)
    (ha : applies m w r.id = true) : holds m w r.effect = true := by
  simp only [consistent, List.all_eq_true] at h
  have := h r hr
  simp only [applies] at ha
  simpa [ha] using this

end Lawean.Sem
