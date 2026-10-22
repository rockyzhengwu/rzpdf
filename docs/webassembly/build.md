# WA-02 目标构建

构建使用 [固定配置](toolchain.json)，Node 运行版本为 22.15.1，SDK 内部 Node 为 24.19.0，不改变仓库的原生默认工具链。Emscripten 3.1.64、Skia 0.93.1 与指定 nightly 是本轮验证组合，不表示任意 SDK/Rust 版本互相兼容。

## 准备工具

检查与构建脚本都不安装工具。以下是用户要求配置工具链时的显式准备命令，SDK 和缓存置于 ignored `output/`。Rust 工具链安装不更改默认值，且需要 `rust-src`；标准库由同一次构建重编，不要求安装预编译 Emscripten target。

```sh
git clone https://github.com/emscripten-core/emsdk.git output/webassembly/toolchains/emsdk
git -C output/webassembly/toolchains/emsdk checkout d7b4130688bfa348392b921ab04015aa15c4f154
python3 output/webassembly/toolchains/emsdk/emsdk.py install 3.1.64
python3 output/webassembly/toolchains/emsdk/emsdk.py activate 3.1.64
rustup toolchain install nightly-2026-08-26 --profile minimal --component rust-src
```

准备命令需要网络。首次构建还会获取 Cargo 缓存缺少的包、Emscripten zlib port 和 Skia 固定版本源码及其依赖。Cargo 使用 `--locked`；不更新仓库锁文件。SDK 环境仅由子进程加载，不写 shell 启动文件。

## 构建与运行

```sh
python3 scripts/build_webassembly.py --package pdf --output output/webassembly/wa02-core.json
python3 scripts/build_webassembly.py --package render --example wasm_build_probe --run --output output/webassembly/wa02-link.json
```

可以用 `--emsdk` 指定 SDK 目录，用 `--toolchain` 指定已有 nightly 的本地名称；驱动核对 rustc 完整提交、SDK 提交、emcc 版本及锁定 Skia 版本，名称不同不能绕过版本检查。本机复用已有 nightly 的 `rzpdf-wasm-2026-08-26` 别名，实际执行时加 `--toolchain rzpdf-wasm-2026-08-26`。

驱动覆盖目标专用 `CC/CXX/AR`，避免全局宿主编译器抢占；Emscripten 目标启用 FreeType `bundled`，C 编译和最终链接启用 SDK zlib。原生目标仍使用原有系统 FreeType 配置。

Skia 通过 `FORCE_SKIA_BUILD=1` 从源码构建，GN 显式启用 external FreeType，头文件取自锁定 freetype-sys 的源码目录；实际链接由核心的 bundled FreeType 提供。C 依赖使用 `-O2`，Skia 通过本次 Cargo 命令的 profile 配置固定 opt-level=2，避免 dev 模式下未优化的色彩转换函数超过旧版 Binaryen 的局部变量数限制；不修改原生 profile。GN 参数名含 system，但不会探测或链接宿主 FreeType。这避免预构建 Skia 的裁剪版 FreeType 与核心完整 FreeType 的同名符号发生混用。

当前 Rust 的链接器默认参数启用 Wasm 异常处理，而本组合的 C 依赖使用 Emscripten 的 setjmp/longjmp 模式。因此固定 Rust `panic=abort`、关闭目标 exception-handling，并在最终链接显式 `-fno-exceptions`，保持 C 库已有的 longjmp 支持。未定义符号仍是链接错误。固定 nightly 对该目标特性发出 unstable 警告，后续调整 Rust/SDK/Skia 时必须重跑探针，不能直接去掉参数。

探针使用 8 MiB 栈、32 MiB 初始内存、256 MiB 最大线性内存和内存增长；启用栈溢出检查。它们是本轮工具链探针的参数，不是产品已实现的资源预算承诺。生产预算、分配失败与 Worker 恢复仍按 spec 实现。

## 证据边界

JSON 记录源码身份、工具版本、命令、参数、构建日志、耗时和目标 native archive 的对象格式抽查；抽查覆盖缓存目录中每个 archive 的一个对象成员，不冒充所有成员或所有编解码器的运行验证。链接例子另记录 JS/Wasm SHA-256，`--run` 记录 Node 退出码及输出。

例子在内存中生成单页 PDF，解析后经 FreeType/Skia 绘制，再编码和解码 PNG；断言 96×96 尺寸、文字深色像素、红色几何和透明叠色。Emscripten 文件操作发生在虚拟文件系统。原生可使用同一例子作对照：

```sh
cargo run -p render --example wasm_build_probe --locked --offline -- output/webassembly/wa02-native-probe.png
```

构建或 Node 成功都不表示浏览器验收通过。WA-01/02 及构建驱动仍保留 `p0_complete: false`；只有真实浏览器驱动通过必选 P0 探针后记录 P0 完成，且不外推稳定 API、多浏览器、Worker 和完整字体服务。


## WA-03 浏览器最小闭环

```sh
npm --prefix web ci --ignore-scripts
python3 scripts/build_webassembly.py --package web-wasm --output output/webassembly/wa03-build.json
npm --prefix web run build
python3 -m http.server 8000 --bind 127.0.0.1 --directory web
```

在浏览器访问 `http://127.0.0.1:8000`，选择本地 PDF。构建产物和 TS 输出位于 ignored `web/dist/`，本地处理，不上传输入。当前严格解析，支持 Standard AES-128/R4 空用户密码读取；输入文件仍最多 8 MiB。文件/图像/句柄/DPI 上限见 spec。需要继续使用已有 nightly 别名时，在构建命令加上 WA-02 中的 `--toolchain` 参数。

Web 构建仅在该目标命令追加 ES module、模块化和导出符号参数；不修改原生 profile。浏览器包装使用 owned Uint8ClampedArray，RGBA8/sRGB/straight alpha/逐行紧密排列。关闭文档可重复调用；模块 trap 后必须重建。缺少内嵌字体时，可选择本地 TTF/OTF/TTC 替代程序（最多 32 MiB）；程序仅在浏览器内使用，不打包或上传。仅有可靠 Unicode 映射的字符可替代，且替代字体必须实际覆盖这些字符。P0 演示在主线程同步执行，Worker 与取消在 WA-08 实现。

浏览器回归使用已准备的 Chromium 可执行文件，不自动下载浏览器。首次准备 Node 开发依赖和 Python Pillow 是显式环境步骤；固定 npm 依赖由 lock 文件管理。当前 Python 生成器需要 Pillow，仅生成项目自有图案，不修改原有二进制测试资产。

```sh
python3 scripts/prepare_webassembly_fixtures.py
cargo build -p render --example rgba_reference --locked --offline
RZPDF_CHROMIUM=/absolute/path/to/chrome node web/tests/browser.mjs
```

驱动自动生成原生 RGBA 基线，起仅监听 loopback 的临时服务器，并在 Chromium 中检查真实 Canvas、各页像素、坏输入、句柄上限、反复释放及 memory.grow 后副本。合成回归的像素阈值固定为零差异。私有清单可逐样本显式设置 `pixel_threshold`（`max_channel_difference` 和每页 `differing_channels`）；报告逐页记录实际误差和门限，不修改合成基线。结果及截图放在 `output/webassembly/wa03-browser/`。输入通过独立清单寻址，报告记录哈希；测试资产不复制进 Web 发布目录。字体资产沿用仓库已有的来源和许可证，生成器新增图形采用仓库许可。

实现参考 [Emscripten 的直接调用与内存视图约定](https://emscripten.org/docs/porting/connecting_cpp_and_javascript/Interacting-with-code.html)；浏览器驱动使用 [Playwright BrowserType](https://playwright.dev/docs/api/class-browsertype)。当前仅验证 Chromium，Firefox/WebKit 及完整语料在 WA-11。


额外本地回归可通过 `RZPDF_PRIVATE_MANIFEST` 传入 JSON 对象，按任意私有 ID 记录 `path`、`sha256`，以及可选 `font_path`、`font_sha256`。路径只写入 ignored 清单；文档不列来源文件名。驱动用同一个显式字体生成原生基线并与浏览器比较，不自动查询或上传系统字体。R4 非空密码的原生认证、只读保存拒绝和 R6 回归由 `pdf/tests/r4_security.rs` 等安全测试覆盖；浏览器暂未交付密码输入界面。

## 字体格式矩阵

本地验证工具需 FontTools 4.62.1；它不进入 Rust/Wasm 运行时。先构建当前
原生 rgba_reference 与 Wasm/TS，再生成并运行矩阵：

```sh
python3 scripts/prepare_web_font_matrix.py
RZPDF_CHROMIUM=/absolute/path/to/chrome node web/tests/font-matrix.mjs
```

可显式传入 `--cjk-collection /absolute/path/to/collection.ttc` 和
`--cjk-license /absolute/path/to/license` 验证命名 Noto CJK JP/KR/SC/TC faces。
原字体、子集及许可只保留在 ignored output，报告包含原集合与各子集哈希。
不传入时仅验证已有项目资产，报告真实 CJK 未验证；不自动下载或选择平台字体。
所有新增正向场景固定原生/浏览器零像素差异，另比较注册与显式索引的像素、
PDF 度量和保存重开；CFF 轮廓裁剪明确作为拒绝项。

## 字体与 CMap 的可部署资源包

先完成当前 Wasm 构建、TypeScript 构建与构建报告。打包工具只使用
本地输入，验证 Wasm 与已通过的 WA-03 报告匹配；不安装或下载字体。
输出目录必须不存在。只打包内嵌数据的说明与清单：

```sh
node scripts/package_web_assets.mjs --output output/webassembly/font-assets-v1
```

默认报告路径沿用本指南的 WA-03 输出，也可用 `--build-report` 指定。
`--wasm` 可指定同一构建的模块。生成的内嵌清单分别记录 PFB、AFM、
CMap 的哈希、字节数、对应许可原文和精确载荷出现次数；不将这些
资源再次完整复制到外部，也不复制整个 Wasm。包内原始／gzip 9
大小为实际字节测量，不代表服务器已经启用压缩。

可选字体需要明确的本地配置。该配置用于打包，不能作为阅读器的
字体清单上传；`file` 和 `licenseFile` 相对于配置所在目录解析。
以下哈希及大小必须替换为实际字体值：

```json
{
  "version": 1,
  "entries": [{
    "resource": {
      "id": "my-font",
      "version": "1",
      "format": "truetype",
      "sha256": "<actual lowercase SHA-256>",
      "sizeBytes": 12345,
      "font": {"family": "My Font", "aliases": [], "weight": 400, "italic": false, "serif": false, "index": 0},
      "license": {"spdx": "<actual license identifier>"}
    },
    "file": "my-font.ttf",
    "licenseFile": "my-font-license.txt"
  }]
}
```

```sh
node scripts/package_web_assets.mjs --config /absolute/path/to/config.json --output output/webassembly/font-assets-with-fallback-v1
```

生成的 `fonts/manifest.json` 是实际 Web 资源清单。将整个输出目录
部署为静态资源，阅读器选择此清单，资源根填写该目录下 `fonts/`
的完整 HTTP(S) URL，并保留末尾斜杠。按需服务只读取清单指定的
内容哈希 URL，不读取本地配置路径。`fonts/licenses.json` 关联许可
原文；许可、来源说明和指纹需与资源一同部署。

同一 TTC 的多个索引使用多个资源描述，但只复制一份程序；内容
相同的文件按 SHA-256 去重。每源最多 32 MiB，独立程序合计
64 MiB，最多 128 个资源，生成的 Web 清单最多 1 MiB。
默认拒绝再次外置已内嵌字体；确需这么做时显式使用
`--allow-embedded-duplicates`，清单仍记录重复。

该包验证元数据、格式头、字节身份和复制，不代替字体注册时的
face／覆盖检查，也不表示所有第三方依赖许可、SDK 或发布已验收。
生成过程不覆盖已有目录，验证失败不留下已提交资源包。
