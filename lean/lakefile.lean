import Lake
open Lake DSL

package lawean where
  -- 依存なし（core のみ）。Mathlib は要らない

@[default_target]
lean_lib Lawean where
  roots := #[`Lawean]
