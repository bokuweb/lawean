// WASM 用: Lean のランタイムが参照する libuv の 4 関数のスタブ。
// 一時ファイル・OS エラー文字列のためのもので、純粋関数（applyUnit 等）は呼ばない。呼ばれたら失敗を返す。
#include <stddef.h>
const char* uv_strerror(int err) { (void)err; return "libuv is not available in wasm"; }
int uv_os_tmpdir(char* buffer, size_t* size) { (void)buffer; (void)size; return -1; }
int uv_fs_mkstemp(void* loop, void* req, const char* tpl, void* cb) { (void)loop; (void)req; (void)tpl; (void)cb; return -1; }
int uv_fs_mkdtemp(void* loop, void* req, const char* tpl, void* cb) { (void)loop; (void)req; (void)tpl; (void)cb; return -1; }
