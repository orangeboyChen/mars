#!/usr/bin/env bash
# Builds `compat_tool`, the C++ half of the differential format test.
#
# It compiles only the xlog sources the test needs (no JNI, no ObjC, no
# appender): the crypt + buffer classes for encoding, and the repo's own
# decode_log_file.c for decoding. zstd comes from the vendored copy in
# mars/zstd so the result does not depend on a system libzstd.
#
# usage: compat/cpp/build.sh [output-path]

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="${1:-$here/compat_tool}"
build_dir="${BUILD_DIR:-$here/build}"

CXX="${CXX:-c++}"
CC="${CC:-cc}"
CXXFLAGS="${CXXFLAGS:--std=c++14 -O1}"
CFLAGS="${CFLAGS:--O1}"

mkdir -p "$build_dir"

cpp_sources=(
  "$here/compat_tool.cc"
  "$here/assert_stub.cc"
  "$root/mars/xlog/crypt/log_crypt.cc"
  "$root/mars/xlog/src/log_base_buffer.cc"
  "$root/mars/xlog/src/log_zlib_buffer.cc"
  "$root/mars/xlog/src/log_zstd_buffer.cc"
  "$root/mars/comm/autobuffer.cc"
  "$root/mars/comm/ptrbuffer.cc"
)

c_sources=(
  "$root/mars/xlog/crypt/decode_log_file_c_impl/decode_log_file.c"
  "$root/mars/xlog/crypt/decode_log_file_c_impl/micro-ecc-master/uECC.c"
)

# Vendored zstd: common + compress + decompress are enough for streaming.
shopt -s nullglob
zstd_sources=(
  "$root"/mars/zstd/lib/common/*.c
  "$root"/mars/zstd/lib/compress/*.c
  "$root"/mars/zstd/lib/decompress/*.c
)
shopt -u nullglob

includes=(
  -I"$root"
  -I"$root/mars"
  # The decoder's micro-ecc copy is the one that gets linked, so its header
  # has to win over mars/xlog/crypt/micro-ecc-master/uECC.h.
  -I"$root/mars/xlog/crypt/decode_log_file_c_impl"
  -I"$root/mars/xlog/crypt"
  -I"$root/mars/zstd/lib"
  -I"$root/mars/zstd/lib/common"
)

objects=()
compile() {
  local compiler="$1" flags="$2" src="$3" obj
  obj="$build_dir/$(echo "${src#$root/}" | tr '/' '_').o"
  if [ "$src" -nt "$obj" ]; then
    echo "  CC/CXX $(basename "$src")"
    "$compiler" $flags "${includes[@]}" -c "$src" -o "$obj"
  fi
  objects+=("$obj")
}

for src in "${cpp_sources[@]}"; do
  compile "$CXX" "$CXXFLAGS" "$src"
done

for src in "${c_sources[@]}" "${zstd_sources[@]}"; do
  # decode_log_file.c brings its own `main`; rename it so it can be linked.
  if [ "$(basename "$src")" = "decode_log_file.c" ]; then
    if [ "$src" -nt "$build_dir/decode_log_file.o" ]; then
      echo "  CC $(basename "$src")"
      "$CC" $CFLAGS -Dmain=decode_log_file_main \
        -include "$here/decode_log_file_shim.h" "${includes[@]}" -c "$src" \
        -o "$build_dir/decode_log_file.o"
    fi
    objects+=("$build_dir/decode_log_file.o")
    continue
  fi
  compile "$CC" "$CFLAGS" "$src"
done

echo "  LINK $out"
"$CXX" $CXXFLAGS "${objects[@]}" -o "$out" -lz -lpthread
echo "built $out"
