// `zstdDecompress` in the vendored
// mars/xlog/crypt/decode_log_file_c_impl/decode_log_file.c reads and writes a
// `lastPos` variable that the file never declares, so the decoder does not
// compile as shipped. This header supplies the missing declaration (a
// file-scope counter, which is what the code treats it as) and is injected with
// `-include` so the vendored source stays untouched.
#include <stddef.h>

static size_t lastPos = 0;
