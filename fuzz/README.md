# 覆盖率引导模糊测试

2026-10-04：按用户要求暂缓此专项，不继续实现或运行；普通 CI 已移除 fuzz 步骤，手动工作流任务也已禁用。已有代码与历史结果保留。fonts 新入口的三项共享测试曾通过，但插桩运行尚未验收，不作为当前交付主线。

独立 Cargo 工作区，不影响根目录正常测试。`Cargo.lock` 单独锁定工具依赖。
目标采用 [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz.html) 的 cargo-fuzz / libFuzzer 接口。

四个目标：

- `syntax`：严格/兼容语法解析，成功读取必须推进位置。
- `document`：打开完整 PDF，继续访问最多 8 页并解释内容。
- `content`：内容操作符解析及新建页面上的内容解释；预注册 F1 Helvetica 和 Im1 RGBA，让合法文字/图像操作可以进入对应解释路径。
- `filters`：首字节选择 Flate/LZW/ASCIIHex/ASCII85/RunLength；成功解码必须遵守输出限额。

每个输入上限 64 KiB；配置小对象数、深度、解码、文本、路径和诊断限额。
不捕获 panic：panic 应由 fuzzer 保存为失败输入。明确的 PDF 错误是允许的结果。
这些限额不等同于进程峰值内存或单次输入的执行时间限制。

## 运行

需要 nightly 与 cargo-fuzz（工具安装由开发者按本机环境执行）：

```sh
cargo install cargo-fuzz --version 0.13.2 --locked
cargo test --manifest-path fuzz/Cargo.toml --locked --lib
cargo check --manifest-path fuzz/Cargo.toml --locked --bins
cd fuzz
cargo +nightly fuzz run syntax -- -max_len=65536 -max_total_time=60 -timeout=5 -rss_limit_mb=1024
cargo +nightly fuzz run document -- -max_len=65536 -max_total_time=60 -timeout=5 -rss_limit_mb=1024
cargo +nightly fuzz run content -- -max_len=65536 -max_total_time=60 -timeout=5 -rss_limit_mb=1024
cargo +nightly fuzz run filters -- -max_len=65536 -max_total_time=60 -timeout=5 -rss_limit_mb=1024
```

也可安装到本项目忽略目录，避免改全局工具：

```sh
cargo install cargo-fuzz --version 0.13.2 --locked --root "$PWD/fuzz/target/tools"
cd fuzz
RUSTUP_TOOLCHAIN=nightly ./target/tools/bin/cargo-fuzz fuzz run syntax corpus/syntax --sanitizer address -- -seed=72657664 -max_len=65536 -max_total_time=60 -timeout=5 -rss_limit_mb=1024
```

先准备 corpus。不要把 `seeds/` 作为第一个语料目录：libFuzzer 会在第一个目录写入新增语料，应保留自编种子和演化语料的边界。runner 的 run 命令不支持 --locked；先用上方 locked 检查确认依赖，再检查运行是否改变锁文件。

正式验收前应添加来源/许可明确的有效与异常种子，不能只用空语料证明深入覆盖。
`seeds/` 中语法、内容和 ASCIIHex 种子由本项目编写，无第三方样本文本；它们不是完整覆盖样本集。
运行前可将对应种子复制到 `corpus/<target>/`。完整文档种子应由项目创建示例生成，或使用登记许可的外部样本，不自动复制来源未明确的既有 PDF。

项目自编完整文档种子包含两页、标准字体文本、路径、RGB 图像和 SMask：

```sh
mkdir -p fuzz/corpus/document
cargo run --manifest-path fuzz/Cargo.toml --locked --example generate_document_seed -- fuzz/corpus/document/created.pdf
```

生成器使用 create_new，拒绝覆盖已有种子；共享 harness 测试要求种子在小 Limits 下严格/兼容模式均能读出两页及每页三个绘制对象。仍需更多 xref/对象流/过滤器与异常文件种子。
崩溃保存在 `artifacts/<target>/`，语料在 `corpus/<target>/`；两者默认忽略。
失败输入使用 `cargo +nightly fuzz tmin <target> <artifact>` 最小化，并转成普通回归测试。
记录工具链、依赖锁、目标、运行时间、种子来源及结果；短时无崩溃不证明所有输入安全。

## 验收边界

共享 harness 的普通测试只证明入口可以执行；`cargo check` 不证明插桩成功。
覆盖率引导运行、sanitizer、原生库插桩及远端 CI 的状态须分别记录。
当前 targets 未覆盖全部图像解码器、字体程序或所有编辑状态转换，后续继续扩充。

本地短时插桩运行与 PDF 种子独立检查见 [验证记录](../docs/fuzz-smoke-validation.md)。

保留的 `Fuzz smoke checks` 工作流仅支持手动触发，任务当前禁用；恢复后计划各目标分别运行 60 秒，显式使用 AddressSanitizer，保留失败输入和演化语料。本地/远端执行结果须单独登记，工作流配置本身不是运行成功的证据。原生依赖是否插桩仍须检查构建参数，不能由 Rust sanitizer 选项推断全部 C/C++ 路径已覆盖。
