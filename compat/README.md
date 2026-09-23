# C++ / Rust 差分格式测试

目标：证明 Rust 端口写出来的 xlog 文件和 C++ 写出来的**是同一种格式**——不是"我读了 C++ 代码所以应该对"，而是两边互相读写、逐字节比对。

## 怎么跑

```bash
./compat/cpp/build.sh                 # 编译 C++ 侧（只需 g++/clang + zlib）
cargo build -p mars-xlog-compat       # 编译 Rust 侧
python3 compat/run.py                 # 跑矩阵
```

`run.py` 会自己编译缺失的工具，`--keep` 保留中间产物，`--filter zstd` 只跑名字里含 zstd 的用例。

## 两侧分别是什么

| | C++ | Rust |
|---|---|---|
| 编码 | `LogZlibBuffer` / `LogZstdBuffer`（`mars/xlog/src/`），走 `Write()` + `Flush()`，和 `XloggerAppender` 对 mmap 缓存文件做的事完全一样 | 同一个流程的 Rust 版 `mars_xlog_buffer::LogBuffer` |
| 解码 | 仓库自带的 `mars/xlog/crypt/decode_log_file_c_impl/decode_log_file.c` | `rust/crates/mars-xlog-compat` |

C++ 侧只编译 xlog 的 crypt + buffer 这几个 .cc（没有 JNI / ObjC / appender），zstd 用仓库里 vendored 的 `mars/zstd`；解码器用的就是仓库自己的那份 C 实现，没有重新实现。两个小例外写在代码注释里：`decode_log_file.c` 里 `lastPos` 没声明（用 `-include` 补上），`mars/comm/assert` 依赖 `xlogger_*`（用 `assert_stub.cc` 顶掉）。

## 矩阵

`mode(zlib|zstd) × compress(1|0) × crypt(1|0) × sync/async × 单块/每记录一块` = 32 个用例。每条用例：

1. 同一批记录分别用 C++ 和 Rust 编码成两个 `.xlog`；
2. 两个文件各自过两个解码器，共 4 份明文；
3. 断言：
   - **Rust 解码器必须从两个文件里都还原出原始输入**（`rust→cpp文件`、`rust→rust文件`）；
   - 在 C++ 解码器无损的路径上（zlib async、所有 sync），它从 Rust 写的文件里也必须还原出原始输入；
   - 确定性用例（`nocrypt` 且 `sync` 或 `compress=0`）额外要求两边**编码字节完全一致**。

输入记录覆盖了 ASCII、UTF-8、0x01–0xFF 全字节、超过一个 TEA 分组的长记录、以及紧贴 8 字节 TEA 分组边界的 1/7/8 字节记录。

## 已知差异（测试通过，但值得知道）

1. **zlib 压缩字节不完全相同**：flate2 用的是 `miniz_oxide`（`rust_backend`），C++ 用的是系统 zlib。同一份输入两边差 3 字节（566 vs 569）。格式层面完全互通——互相都能解出原文——但字节流不逐字节相同。要逐字节一致得把 flate2 换成 zlib / zlib-rs 后端。
2. **zstd 基本一致**：单块用例两边字节完全相同；每记录分块时 4096 字节那条记录一边压成 18 字节、一边 17 字节（vendored zstd 与 crates.io zstd 1.5.7 的差异），同样不影响互解。
3. **C++ 解码器在 async zstd 上会丢尾巴**：`decode_log_file.c` 的 `zstdDecompress` 一旦把输入读完就跳出循环，不去 drain `ZSTD_decompressStream`，于是每个块的最后几个字节被丢掉（单块丢 9 字节，每记录分块时只剩 432/4510 字节）。这是 C++ 侧解码器的问题，Rust 解码器能完整还原，所以这条路径只校验 Rust 侧的还原结果。
4. **`is_compress=false` 的 async 路径不是可用组合**：magic 字节写着 zlib/zstd，所有解码器（包括仓库自己的）都会去解压一段没压缩过的数据。这条路径只比对两个编码器的字节；另外 C++ 把 `is_compress_` 当作 `__GetSeq()` 的 sync/async 参数传进去，未压缩的 async buffer 拿到 seq=0，Rust 端口故意传 `true`（见 `LogBuffer::reset` 的注释），所以比对时 seq 字段会被屏蔽。
