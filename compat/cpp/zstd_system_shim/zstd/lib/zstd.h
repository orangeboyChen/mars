// Lets `log_zstd_buffer.h` (#include "zstd/lib/zstd.h") resolve to the
// system libzstd headers when the harness is built with SYSTEM_ZSTD=1.
#include <zstd.h>
