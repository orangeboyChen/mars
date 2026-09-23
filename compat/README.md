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
   - 两边的**记录结构**（magic / seq / tailer，以及除 async zstd 外的块长度）必须一致；
   - 确定性用例额外要求**编码字节完全一致**：zlib 路径始终严格比对（现在两边都是 zlib 语义、字节相同），zstd 路径在 C++ 侧也用同版本 zstd 时用 `--strict-bytes` 严格比对。

输入记录覆盖了 ASCII、UTF-8、0x01–0xFF 全字节、超过一个 TEA 分组的长记录、以及紧贴 8 字节 TEA 分组边界的 1/7/8 字节记录。

## 已知差异

1. **zstd：仓库 vendored 的是 1.4.4，Rust 侧是 1.5.7**。同一条记录压缩后长度可能差 1 字节（实测 4096 字节那条：C++ 18 字节、Rust 17 字节），其余结构（magic / seq / tailer / 块数）完全一致，互解也完全正常。把 C++ 侧换成同版本 zstd 后**字节完全一致**——已验证：

   ```bash
   SYSTEM_ZSTD=1 ./compat/cpp/build.sh          # 链接系统 libzstd（1.5.x），跳过 mars/zstd
   python3 compat/run.py --strict-bytes         # 32/32，zlib 与 zstd 全部 identical
   ```

   也就是说差异只来自 C++ 侧依赖的 zstd 版本，不是端口逻辑。要彻底消除就把 `mars/zstd` 升到 1.5.7。`build.sh` 支持 `ZSTD_LIB_DIR` / `ZSTD_INCLUDE_DIR` 指定非默认路径。

2. **C++ 解码器在 async zstd 上会丢尾巴**：`decode_log_file.c` 的 `zstdDecompress` 一旦把输入读完就跳出循环，不去 drain `ZSTD_decompressStream`，于是每个块的最后几个字节被丢掉（单块丢 9 字节，每记录分块时只剩 431/4510 字节）。这是 C++ 侧解码器的问题，Rust 解码器能完整还原，所以这条路径只校验 Rust 侧的还原结果。

3. **C++ 在非加密模式下会把 64 字节未初始化内存写进 header**：`LogCrypt` 没配置 server 公钥时直接 return，`client_pubkey_` 从没被初始化，而 `SetHeaderInfo` 照样把它拷进每个 header——日志头里写的是当时堆上的内容（macOS 上是全 0，Linux 上不是）。Rust 端口写 0。解码器在非加密 magic 下不读这个字段，功能无影响，但严格说这是把 64 字节堆内存泄进日志文件。比对时该字段会被屏蔽。

4. **`is_compress=false` 的 async 路径不是可用组合**：magic 字节写着 zlib/zstd，所有解码器（包括仓库自己的）都会去解压一段没压缩过的数据。这条路径只比对两个编码器的字节；另外 C++ 把 `is_compress_` 当作 `__GetSeq()` 的 sync/async 参数传进去，未压缩的 async buffer 拿到 seq=0，Rust 端口故意传 `true`（见 `LogBuffer::reset` 的注释），所以比对时 seq 字段会被屏蔽。

> 之前 flate2 用 `miniz_oxide` 后端时，zlib 压缩字节和 C++ 的系统 zlib 差 3 字节（566 vs 569）。改成 `zlib-rs` 后端并显式用 `new_with_window_bits(-15)`（对应 `deflateInit2(..., -MAX_WBITS, ...)`）后，两边字节完全一致（566 vs 566）。
