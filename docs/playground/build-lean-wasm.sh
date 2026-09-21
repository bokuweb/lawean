#!/usr/bin/env bash
# Lean の C 出力を Emscripten で WASM にする（ADR-0017、docs/12 §4）。
# 要るもの: emcc、Lean の wasm32 版ツールチェーン（GitHub release の lean-<ver>-linux_wasm32.zip。初回に取ってくる）
# 出力: docs/playground/lean/lawean_lean.{js,wasm}
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VER="$(sed 's/.*://' "$ROOT/lean/lean-toolchain")"   # v4.15.0
CACHE="${LEAN_WASM_CACHE:-$HOME/.cache/lawean/lean-wasm}"
W="$CACHE/lean-${VER#v}-linux_wasm32"
if [ ! -d "$W" ]; then
  mkdir -p "$CACHE"
  echo "downloading lean-${VER#v}-linux_wasm32.zip"
  gh release download "$VER" -R leanprover/lean4 -p "lean-${VER#v}-linux_wasm32.zip" -D "$CACHE" --clobber
  (cd "$CACHE" && unzip -q -o "lean-${VER#v}-linux_wasm32.zip")
fi
OUT="$(mktemp -d)"
(cd "$ROOT/lean" && lake build Lawean.Ffi >/dev/null)
for m in Ident Check Refs Ffi; do
  emcc -O2 -I "$W/include" -c "$ROOT/lean/.lake/build/ir/Lawean/$m.c" -o "$OUT/$m.o"
done
emcc -O2 -I "$W/include" -c "$ROOT/crates/lawean-leanrt/csrc/shim.c" -o "$OUT/shim.o"
emcc -O2 -c "$ROOT/crates/lawean-leanrt/csrc/uv_stubs.c" -o "$OUT/uv_stubs.o"
mkdir -p "$ROOT/docs/playground/lean"
emcc -O2 "$OUT"/*.o -L"$W/lib/lean" -lInit -lleanrt \
  -o "$ROOT/docs/playground/lean/lawean_lean.js" \
  -sMODULARIZE=1 -sEXPORT_ES6=1 -sALLOW_MEMORY_GROWTH=1 \
  -sEXPORTED_FUNCTIONS=_lawean_leanrt_init,_lawean_leanrt_apply_unit,_lawean_leanrt_check_unit,_lawean_leanrt_relation,_lawean_leanrt_free,_malloc,_free \
  -sEXPORTED_RUNTIME_METHODS=ccall,cwrap,UTF8ToString,stringToUTF8,lengthBytesUTF8
rm -rf "$OUT"
ls -la "$ROOT/docs/playground/lean/"
