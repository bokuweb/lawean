// Lean の C 出力を Rust から呼ぶための薄い皮。lean.h の inline 関数をここで使い、Rust には char* だけを見せる。
#include <lean/lean.h>
#include <stdlib.h>
#include <string.h>

// ランタイムの初期化（lean.h には宣言が無い。Lean が生成する main と同じ呼び方）
extern void lean_initialize_runtime_module(void);
// Lean の外で作ったスレッドから呼ぶには、スレッドごとにヒープの初期化が要る
extern void lean_initialize_thread(void);
extern void lean_finalize_thread(void);
// Lean が生成する初期化関数（Lawean.Ffi とその依存モジュール）
extern lean_object* initialize_Lawean_Ffi(uint8_t builtin, lean_object* w);
// @[export] した関数。引数は所有権が移る（lean_obj_arg）、戻りは所有（lean_obj_res）
extern lean_object* lawean_apply_unit(lean_object* rev, lean_object* unit);
extern lean_object* lawean_check_unit(lean_object* rev, lean_object* unit);
extern lean_object* lawean_relation(lean_object* a, lean_object* b);

static int initialized = 0;

int lawean_leanrt_init(void) {
    if (initialized) return 0;
    lean_initialize_runtime_module();
    lean_object* res = initialize_Lawean_Ffi(1 /* builtin */, lean_io_mk_world());
    if (lean_io_result_is_ok(res)) {
        lean_dec_ref(res);
    } else {
        lean_io_result_show_error(res);
        lean_dec(res);
        return 1;
    }
    lean_io_mark_end_initialization();
    initialized = 1;
    return 0;
}

void lawean_leanrt_thread_init(void) { lean_initialize_thread(); }
void lawean_leanrt_thread_fini(void) { lean_finalize_thread(); }

// 結果の文字列を malloc したバッファに写して返す。呼ぶ側が lawean_leanrt_free で解放する
static char* take_string(lean_object* s) {
    size_t n = lean_string_size(s); // NUL を含むバイト数
    char* out = (char*)malloc(n);
    memcpy(out, lean_string_cstr(s), n);
    lean_dec(s);
    return out;
}

char* lawean_leanrt_apply_unit(const char* rev, const char* unit) {
    return take_string(lawean_apply_unit(lean_mk_string(rev), lean_mk_string(unit)));
}

char* lawean_leanrt_check_unit(const char* rev, const char* unit) {
    return take_string(lawean_check_unit(lean_mk_string(rev), lean_mk_string(unit)));
}

char* lawean_leanrt_relation(const char* a, const char* b) {
    return take_string(lawean_relation(lean_mk_string(a), lean_mk_string(b)));
}

void lawean_leanrt_free(char* p) { free(p); }
