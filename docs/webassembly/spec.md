# WebAssembly PDF 实现规范

状态：2026-10-22，用户已授权按本规范推进 WebAssembly PDF 开发；当前 WA-00～03 的 P0 最小闭环已在 Chromium 验收；稳定 API、Worker、完整字体服务及其他浏览器仍按后续阶段推进。原生基线为 `7480541`。本专题不改产品版本，不将原生测试结论外推为浏览器支持。

本文件定义需求与架构决策；任务及依赖顺序见 [实施计划](plan.md)，用例和当前证据见 [验收规范](acceptance.md)。实现、测试、证据必须关联任务及用例 ID。未通过的门禁保持待完成，不以“能够编译”替代页面或功能验收。

## 1. 目标与边界

在浏览器本地完成 PDF 字节加载、页面解析、文字提取、字体注册、页面渲染，并分阶段开放既有核心支持的编辑及字节保存。默认不上传 PDF，不执行 PDF JavaScript、附件或外部动作。核心继续遵守既有格式限制、权限、文档身份、revision、事务和保存保护。

| 范围 | 首轮要求 | 后续阶段或排除项 |
|---|---|---|
| P0/P1 最小闭环 | 字节输入、页数/页面尺寸、RGBA 输出、文字提取、错误与释放接口；在浏览器显示真实引擎输出 | 完整阅读器交互、编辑面板、全部格式组合不是 P0 退出条件 |
| 字体 | 嵌入字体、Base14、显式注册替代字体；保留字形与 Unicode 的独立处理 | P2 完成多字体回退、按需加载与完整冻结字体矩阵 |
| 阅读器 | P3 实现 Worker、缩放/分块、调度、缓存和取消 | 首版不引入共享内存、多线程、WebGPU 或浏览器系统字体枚举 |
| 编辑 | P4 选择已有支持的来源驱动文字/图片操作、页面操作及保存闭环 | 复杂文字自动排版、任意字体子集、OCR、安全脱敏不随 Web 移植扩大范围 |
| 加密/签名 | 原生密码与权限规则不得绕过；随机数依赖必须明确适配 | 首轮不承诺浏览器签署、在线信任或完整签后编辑；未交付入口返回明确不支持 |
| 平台 | 单线程 `wasm32-unknown-emscripten`，CPU Skia 栅格渲染作为首条验证路径 | `wasm32-unknown-unknown`/Canvas 后端为评估分支，不混记为已支持路线 |

所有阶段的静默丢字、错误字形、错误图层/变换、跨文档污染、输入触发崩溃和失败后部分编辑均为验收阻塞。损坏输入的明确拒绝、缺失 Unicode 和已声明的受限功能需分别记录。

## 2. 架构与 crate 边界（WA-ARC）

计划保持四个 Rust crate：

```text
pdf/          解析、文档模型、解释器、字形映射、提取、编辑、保存
font-data/    内置字体、度量、CMap 与资源元数据
render/       现有 Skia 后端及内存像素输出
web-wasm/     Emscripten 导出 ABI、句柄表、参数校验、资源生命周期
web/          TypeScript 包装、Worker、异步资源加载、演示阅读器
scripts/      构建预检、浏览器与语料验收驱动
```

上述四 crate 与前端目录已在 WA-03 落地；目录存在不代表后续各阶段功能已实现。`web/` 是前端包，不属于 Cargo workspace。依赖方向为 `web-wasm → pdf/render`、`render → pdf`、`pdf → font-data`；`pdf` 不依赖 DOM、JavaScript、网络或 Skia。

- WA-ARC-01：继续复用 `Device`、`DisplayList`、owned `Document` 和字节读写入口，不重写解析器。
- WA-ARC-02：字体、过滤器、安全等现有核心模块保持模块形式；只有独立构建/复用需求明确时才增加 crate。
- WA-ARC-03：JS 绑定只属于 `web-wasm`；浏览器网络和持久缓存属于 `web`。Emscripten 首轮使用导出 C ABI 和 TypeScript 包装，不同时依赖 wasm-bindgen。
- WA-ARC-04：签名等较重能力按实际依赖审计决定 feature 边界；关闭功能必须有明确能力报告和错误，不能留下隐式调用缺口。不得为移植改坏原生默认能力或取消有效输入检查。

## 3. 工具链与原生依赖（WA-BUILD）

P0 必须验证 Rust/Emscripten ABI 和 Skia 的实际组合，固定 Rust、emsdk 版本/提交、Skia 锁定版本、编译参数、Node、浏览器及操作系统。只安装一个 Rust target 不等于 C/C++ 工具链就绪。首轮组合未通过前，不写“已支持 Emscripten”，也不选择未经运行的 emsdk 版本作为已验证配置。

| 依赖 | 必须验证的工作 |
|---|---|
| Skia/skia-safe | 无系统 Fontconfig 的字节字体加载、CPU surface、像素读取、跨编译资源与链接；不得误链接宿主库 |
| FreeType | Wasm 构建、Type1/CFF/TrueType、集合索引、CID 服务和回调 ABI；不依赖宿主 pkg-config 的库 |
| mozjpeg | JPEG/CMYK 路径、编译器与 SIMD/汇编选项；宿主汇编代码不能进入 Wasm |
| jbig2dec | 现有仓库内固定源码的 Wasm 编译；分配/释放/重分配回调、Globals、对齐与预算失败释放 |
| JPX/ICC 等 Rust 后端 | 目标支持、运行结果、工作区估计和 Wasm 侧峰值；保留有界检查 |
| getrandom/ring | 分别盘点直接与传递版本；适配浏览器随机数和实际开放的加密能力；不可仅修顶层 getrandom 就宣布完成 |
| font-data | 构建时生成与运行时访问分开；资源哈希、许可和压缩后体积单独测量 |

WA-BUILD-01：P0 同时需要 core 检查、最终链接和真实浏览器绘制证据；Node 执行仅为补充。

WA-BUILD-02：使用锁定依赖；记录 `.wasm`、JS 包装和资源清单哈希及压缩前后大小。正常构建不得修改第三方源或宿主原生依赖选择。

WA-BUILD-03：检查脚本不安装软件、不修改系统配置、不自动下载语料；缺工具返回 blocked。用户明确要求继续配置工具链时，可在 ignored `output/` 内准备固定版本 SDK；不改系统默认工具链或 shell 配置。构建驱动仍不自动安装工具。

WA-BUILD-04：核心与渲染器不得混合不同配置的同名 FreeType 静态符号。Wasm 首版让 Skia 从源码构建并使用核心的完整 FreeType，验证 Type1 等驱动可用；GN 中的 system FreeType 必须显式指向目标依赖头文件，不代表可使用宿主库。异常处理、longjmp 与栈参数必须在最终运行探针中验证。

### 读取兼容补充

Standard V=4/R=4 AES-128 的读取可兼容空用户密码，认证沿用原权限语义；非空密码暂仅原生 ASCII 接口支持。AESV2 使用首个文件 ID 参与口令验证，并按对象号/代数派生对象密钥，不能复用 R6 文件级 AES-256 密钥。R4 目前只读，完整保存和有变更的增量保存明确拒绝，不降级或移除保护；新加密输出仍为 R6。其他旧修订和 RC4 内容过滤器保持未支持。错误应包含实际 handler/V/R。

BG/UCR 及 BG2/UCR2 的标量函数须解析、保留并计入内容预算；现代条目覆盖旧条目，Default 与 q/Q 正确恢复。它们用于 RGB 到 CMYK 的印刷转换，不在 RGB 栅格输出时误用；transfer/halftone 的非中性值继续明确拒绝。MP/DP 标记点须验证操作数并保留源操作，不开启 marked-content 栈或改变后续文字/可见性。

## 4. Web API 与生命周期（WA-API）

WA-03 使用临时 ABI 版本 1 验证最小闭环，公开契约在 WA-04 冻结。句柄为单模块内不复用的 u32，0 表示失败；类型错误、释放后使用和重复释放拒绝。输入缓冲区、文档、RGBA 缓冲区分别持有句柄；JS 同步复制 RGBA 后释放，任何内存增长后重新读取 HEAPU8。错误信息为 UTF-8，指针/长度只供下一次操作前复制。trap/abort 使该模块内所有句柄失效，由调用方重建实例；当前没有 Worker 取消承诺。

P0 历史限额（PDF 输入现已按 WA-INPUT 调整）：输入最多 8 MiB，最多 8 个活跃句柄，输入、输出与已注册字体程序合计最多 64 MiB，图像/页面最多 4,000,000 像素，单 RGBA 最多 16,000,000 字节，DPI 1～288。解析操作预算 64 MiB、解码流 16 MiB，线性内存最多 256 MiB。它们不构成包含 JS/Skia/C 分配的统一内存硬上限。首版严格解析，不开放编辑、密码输入和文字提取；现支持显式注册一个最多 32 MiB 的本地替代字体，按可靠 Unicode 选字形并保留 PDF 字宽，不按源 GID 猜测。字体对象跨页复用，失败注册保留原字体，清除后可重新注册。其余完整字体服务按 P1/P2 补充，不绕过已有核心保护。


以下是逻辑契约，具体 Rust/C 导出名称在 P1 实现前冻结；JS 包装提供 Promise，但不因此假定 Wasm 同步调用能在中途处理 JS 消息。

| 操作 | 契约 |
|---|---|
| `open(bytes, options)` | 首版整文件 `Uint8Array` 输入；接受限额、Strict/Compatible 和显式密码；返回不透明文档句柄 |
| `getPageCount` / `getPageInfo` | JS 页号从 0 开始；尺寸单位、旋转及用户坐标到显示坐标的变换明确返回 |
| `getFontRequests` / `registerFont` | 返回缺失字体请求和注册结果；见 WA-FONT，不能暴露 Rust 借用或宿主路径 |
| `extractText` | 返回文本、字符几何、坐标系、Unicode 缺失及诊断；不把绘制顺序承诺为阅读顺序 |
| `renderPage` | 返回尺寸、stride、RGBA8、alpha 语义和诊断；首轮输出为白底不透明 RGBA，后续透明输出单独定义 |
| `save` | P4 开放；返回有上限的 PDF 字节，浏览器层负责 Blob 下载；失败不得返回半成品 |
| `close` / `dispose` | 释放文档、页缓存和绑定资源；重复 close 幂等，关闭后其他操作返回无效句柄 |

- WA-API-01：ABI 使用有界整数、地址和长度；不跨 ABI 传递 Rust enum、引用、String/Vec 布局。验证长度、溢出、句柄类别和所有权后再读取缓冲区。
- WA-API-02：文档句柄带 generation 和 Worker session 身份；旧句柄不能访问复用槽或重建 Worker。编辑选择绑定 revision，成功编辑后使旧选择失效；回滚沿用核心 stamp 规则。
- WA-API-03：区分文档、输入/输出缓冲区和任务句柄；明确每个输出的释放函数。第一版对 JS 返回有所有权的复制，禁止把可增长 Wasm 内存的裸 view 长期暴露给用户。
- WA-API-04：错误至少区分格式、密码、权限、不支持、缺字体、预算、无效句柄、取消和内部故障；附阶段/页号/任务 ID，不泄露密码或原始敏感内容。
- WA-API-05：按实际 panic 配置处理 trap；不能承诺 `catch_unwind` 可捕获 abort/OOM。异常终止时废弃该 Worker session，由新 Worker 重开文档，不能继续复用其句柄。

## 5. 字体语义与来源（WA-FONT）

```text
PDF 字符码 → CID/GID → 字形程序 → 定位/绘制
     └────────────→ Unicode → 复制/搜索/无障碍文本
```

- WA-FONT-01：两条映射链分开。GID 仅在具体 face 内有效；不能把源字体 GID 直接用于替代字体，不能将 CID、私用区或自定义字形名猜成 Unicode。显式 ToUnicode 的权威性沿用核心规则。
- WA-FONT-02：优先嵌入字体及子集，其次既有 Base14；非标准字体按名称别名、衬线、字重、倾斜、语言和实际字形覆盖选择显式注册的替代字体。找不到可靠映射时返回缺失诊断。
- WA-FONT-03：保留 PDF 的 Widths/W/DW/W2、TJ、文字矩阵及水平/竖排推进。显示已有页面不通过 `fillText` 重排 Unicode，也不使用替代字体的普通浏览器行布局覆盖 PDF 定位。
- WA-FONT-04：支持矩阵按真实路径区分 Type1/PFB、CFF、CID-keyed CFF、TrueType、OpenType/CFF、集合索引和核心已支持的 Type3；Type3 继续解释 CharProcs。存在解析器不等于已通过字形渲染。
- WA-FONT-05：现有 `set_substitute_font` 是显式单字体基线，不当作完整回退链。新 resolver 应统一预检与实际绘制的选择，避免提取/渲染使用不同字体身份。
- WA-FONT-06：缓存身份至少包含字体内容哈希、集合索引、相关 face 配置；文档字体身份还要包含文档及字体对象/资源作用域，不按 BaseFont 字符串合并。关联 revision、预算和释放策略。

实现先在 `pdf/src/font` 中明确 resolver/provider/fallback 职责；按实际公共边界确定文件，不强制增加字体 crate。Resolver 决定选择与请求；Provider 提供已经注册的字体字节；Skia/FreeType 消费字形和字体程序。无浏览器路径由核心偷偷下载字体。

## 6. 异步字体与资源包（WA-ASSET）

- WA-ASSET-01：采用有界的“发现字体请求 → Web 异步加载 → 校验/注册 → 重新渲染”协议。首次预检可增量发现嵌套 Form/Type3 字体，限制总请求数和重试轮次，不能要求网络 await 穿过当前同步 Rust 解释器。
- WA-ASSET-02：请求带文档/session 身份和请求 ID；请求合并、超时、失败、离线、取消、过期返回都可观察。迟到结果不得污染已关闭文档或新 session。
- WA-ASSET-03：网络仅访问应用配置的资源根或显式回调，不将 PDF 字体名称解释为 URL/路径。资源使用清单、内容哈希、版本、格式、集合索引、许可和大小上限；哈希不符不注册。
- WA-ASSET-04：标准字体、CMap 与 CJK/其他文字系统的替代字体分开计量和缓存；CMap 是编码/语义数据，不是字形字体。首版先注册完整字体，再评估分片；不能仅按 Unicode 切割未知自定义编码字体。
- WA-ASSET-05：内嵌数据保留为原生默认路径；Web 可以按实际体积评估外置资源。必须证明打包后未无意重复携带内嵌与外置全量字体，不从仓库目录大小推算 Wasm 下载体积。
- WA-ASSET-06：新输入复杂文字的 shaping、字体覆盖和嵌入/子集属于编辑能力，和已有字形绘制分开。首轮不新增自动 shaping 承诺。

## 7. Worker、内存与显示（WA-RUN）

- WA-RUN-01：P0 可使用最小主线程演示；P3 前迁入单 Worker，生产渲染不得长期阻塞 UI。消息带协议版本、session、任务 ID；旧响应不能覆盖新页/缩放结果。
- WA-RUN-02：首版硬取消可终止 Worker，随后所有句柄失效并显式重开。普通 cancel 消息不能中断同一 Worker 正在执行的同步 Wasm；协作取消须有解释/解码检查点和独立证据后才声明支持。
- WA-RUN-03：为 Web 设置专用输入、解码、操作、字体缓存、页面像素和输出预算，与核心 Limits 取交集。32 位长度及 RGBA 乘法先检查；不能直接照搬原生 512 MiB 预算作为浏览器承诺。
- WA-RUN-04：JS 输入、Wasm 源、解码样本、DisplayList、Skia surfaces/软蒙版/mipmap、RGBA 复制及缓存分别计量。OperationBudget tracked peak 不等于 Wasm heap 或全浏览器 RSS。
- WA-RUN-05：完整关闭/淘汰须释放可跟踪资源。Wasm 线性内存可能保持高水位，不能以 heap 大小未缩小判为所有权泄漏；反之 close 返回也不能证明已经释放。
- WA-RUN-06：缩放、旋转、DPR、页面/显示坐标和 tile 边界统一；缩小采样、遮罩、透明组和有界分块不得静默降级。每页/tile 存储及并发数单独限制。

浏览器专用数值上限和包体积/耗时目标在 P0/P1 实测后冻结到验收记录；冻结前使用显式传入的保守测试配置并记录数值，不能把未测阈值写成产品 SLA。

## 8. 编辑与保存（WA-EDIT）

P4 先冻结来源驱动文字/图片编辑、选页操作和全量保存的子矩阵，再开放入口。新 Unicode 文字需确认目标字体能表达，缺字体/缺映射返回明确错误；替代显示不自动授权写入替代字体。

编辑继续采用核心事务、copy-on-write、revision 失效和权限/签名保护；失败重试无部分更新。保持原生既有签名、恢复文档和受限增量保存的白名单。Wasm 导出层不得绕过这些规则，也不将下载成功作为 PDF 保存成功的证明。

输出需在 Web 和原生严格重开并独立检查；验证文字、引用关系及渲染。兼容读修复、内容流边界和相邻绘制的状态恢复加入编辑保存回归。

## 9. 测试、证据与交付（WA-VERIFY）

- WA-VERIFY-01：每个任务绑定验收用例；记录工具版本、构建选项、源码/二进制/字体/样本哈希、命令、退出码、浏览器版本、DPI/DPR、耗时和内存口径。
- WA-VERIFY-02：Chromium、Firefox、WebKit 分开记录；浏览器内成功与 Node/原生成功分开。P0 至少一个真实浏览器，P5 完成桌面三引擎冻结矩阵；移动设备只在实际测试后声明支持。
- WA-VERIFY-03：复用固定两批 60 文档的去重清单，对可读文档检查全页解析/提取与抽样渲染；损坏源单列负例。不在文档中枚举语料文件名或固化用户私有数据集路径。
- WA-VERIFY-04：原生同源码渲染为移植一致性对照，MuPDF 为独立参考；像素阈值只做筛查。冻结图片误差/墨迹/尺寸阈值，差异页目视复核，不能用双空白图通过验收。
- WA-VERIFY-05：新增合成可分发字体/页面和预算/句柄/取消/失败重试测试；覆盖已有 JPEG/JPX/JBIG2、显式 Mask、透明组、流边界、CJK 与 Type3。私有语料只通过本地清单提供，不进入包或远端 CI。
- WA-VERIFY-06：文档、依赖升级、工具链变更和新失败推动 spec/测试更新；不得跳过或弱化门禁来宣布阶段完成。不自动提交、推送、发布或扩大历史版本平台声明。

## 10. 借鉴来源与决策边界

核对日期：2026-10-22。下列官方资料用于设计借鉴；依赖组合仍需本项目实测，不复制未经验证的支持结论。

- [rust-skia](https://github.com/rust-skia/rust-skia#supported-platforms)：优先验证其 Emscripten 路线；软件栅格及内存字体仍需本项目构建验证。
- [Rust Emscripten 目标](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-emscripten.html)、[emsdk](https://emscripten.org/docs/getting_started/downloads.html)：交叉工具链和 ABI 匹配。
- [PDF.js 字体处理](https://github.com/mozilla/pdf.js/blob/master/src/core/fonts.js)、[字体加载](https://github.com/mozilla/pdf.js/blob/master/src/display/font_loader.js)、[资源与 Worker API](https://github.com/mozilla/pdf.js/blob/master/src/display/api.js)：字体修复/转换、加载失败回退及资源独立配置。首轮不照搬其浏览器字体渲染路线。
- [MuPDF Wasm 字体回调](https://github.com/ArtifexSoftware/mupdf/blob/master/platform/wasm/lib/mupdf.c)：按普通字体、CJK/文字系统提供字体的接口划分。
- [PDFium 字体映射](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/core/fxge/cfx_fontmapper.cpp)：名称归一化、风格和替代匹配的设计参考。
- [getrandom 0.4.3](https://docs.rs/getrandom/0.4.3/getrandom/)：不同 Wasm 目标的随机数适配；另需检查 ring 所带的 0.2 系列，不能混用 feature 名称。

工程借鉴不改变第三方字体、数据、代码的许可。新增资产须登记来源与许可；不因为可以下载或本机已安装就默认可以随 Web 包分发。


### Corpus repair parity follow-up

Native and Wasm continue to share the bounded core behavior for JPX opacity,
CMYK, PDF Indexed palette precedence and low-bit indices; independently sized
image soft masks; direct inline Group 4 framing; and the documented Type3 alpha
and gray-only transparency-group subsets. Browser regression fixtures must check
source pixel/color/opacity formulas in addition to native/Wasm equality.
Compatibility diagnostics do not count as evidence that malformed input became
valid, and resource/unsupported paths must remain explicit across both targets.

The Web viewer applies the document's default optional-content View configuration
and supported usage rules through the existing visibility model. Native browser
references and corpus workers use the same selection. This does not add layer
authoring or a layer-management UI, and legacy core paint-order APIs stay opt-in.

Certified patch point queries retain unresolved-geometry errors. Display may
fall back to bounded forward tessellation with 1/8-device-pixel control-hull
flatness, parameter-space colors evaluated before functions/conversion, greatest
v/u fold precedence and later-patch precedence. Common-v ties require a geometry
identity, not epsilon merging. Depth, scratch and raster-work limits still fail
explicitly; no partial raster is committed on error. Browser tests must include
an analytic folded patch and compare native/Wasm pixels without relaxed parity.

Parsed Compatible input may recover underdeclared OCProperties and the exact
joined stream-ending token `endstreamendobj`, with structure diagnostics and
unchanged resource checks. Strict mode retains both refusals; this does not
change the viewer's current Strict parsing policy. Compatible Form content
warnings must retain the full outer-to-inner invocation path and leaf operation
index/byte offset. Page-level diagnostic count and byte limits include warnings
from every Form invocation and the retained source paths. Strict unsupported
operators and fatal child/resource errors must still fail explicitly.

## Web 功能补齐（WA-WEB，2026-10-22）

用户已授权实现 Web 功能缺口，完整目标保留如下；阶段通过不代替整个目标完成。

| 要求 | 交付与验收 |
|---|---|
| WA-WEB-01 | ABI/TS 结构化错误、带显式密码及 Strict/Compatible 的打开、页几何、诊断、session 和释放；错误密码后可重试且不泄露密码 |
| WA-WEB-02 | 默认 View 可见文本、字符坐标、选择与跨页搜索；未知 Unicode 保留 null，未知字符不被拼接成虚假的跨字符命中；旋转/CropBox/UserUnit 坐标一致 |
| WA-WEB-03 | 书签树、页内/命名目标及链接交互；外部 URI 必须由用户动作触发并校验协议，其他动作不执行 |
| WA-WEB-04 | 常用批注与 AcroForm 的读取、显示、编辑及保存重开；复用原生权限、签名和事务保护，未支持种类明确返回错误 |
| WA-WEB-05 | 已有文字/图片/页面编辑子集和有上限的 PDF 字节保存/下载；选择绑定文档/revision，失败无部分修改；不冒充自动排版或安全脱敏 |
| WA-WEB-06 | 有界多字体 fallback、缺字体请求、显式配置的按需加载与离线缓存；哈希/许可、取消/迟到/同名冲突、缺映射不得猜字 |
| WA-WEB-07 | Worker 异步调用、任务调度、硬取消/重建及会话失效；主线程不运行同步重任务，旧任务结果不能覆盖新画面 |
| WA-WEB-08 | 阅读器接入上述功能、缩放/DPR/旋转和必要分块；完整关闭/释放流程、真实浏览器正负例及原生对照 |

JSON 元数据输出使用独立的只读缓冲句柄，调用方复制后释放，不得作为 PDF 输入
或字体输入使用。所有大小/句柄/页面和权限检查在调用原生入口前完成；输出序列化
同样受字节限额约束。API 返回明确的能力及坐标语义，不将估计的 em 字符框称为
实际字形轮廓，也不将绘制顺序称为阅读顺序。密码不写入错误、日志或持久缓存。

### WA-WEB 首批接口实现说明

上文 WA-03 的 ABI 1、仅严格打开和无密码/文字接口描述保留为 P0 历史。
当前实现使用 ABI 2，新增有界 JSON 查询、显式模式/密码和错误代码；TS/阅读器
接口见 [当前 API](api.md)。默认仍为 Strict，Compatible 必须显式选择。当前
capabilities 明确把未接入的批注/表单/编辑/保存标为 false；完整 session、Worker、
字体服务及后续编辑阶段仍待完成。JSON 输出不得作为输入重用，PDF 来源字节
也计入保留缓冲额度。禁止复制的用户认证文档可渲染，但文字接口拒绝且界面禁用。

### Worker 调度与取消契约（WA-WEB-07 / WA-T11）

主线程使用异步 Worker 引擎；底层同步接口保留给 Worker 与诊断。每个 Worker
持有独立模块，会话 ID 与单调任务 ID 绑定请求和响应；每次只派发一个任务，
普通任务（含已派发）最多 32 项，另留最多 8 个释放控制项保证队列满时仍可 close；
传输中的输入合计最多 64 MiB。输入复制后转移，调用方
原始字节不被 detach；RGBA 输出转移其独立 ArrayBuffer。

AbortSignal 取消尚未派发的任务时只移除该任务。取消已经派发的任务时终止
Worker，使本会话全部文档/字体和待执行任务失效；下一次 open 显式创建新会话，
不保存密码或自动重开文档。超时、Worker 错误或 Wasm trap 同样废弃会话，迟到
响应按 session/任务 ID 丢弃。普通格式/密码/权限错误不废弃 Worker。

文档 close 立即禁止新调用并撤销该文档的排队任务；若该文档正在执行，采用同样
的硬取消并使整会话失效。关闭其他文档可排队完成释放。dispose 幂等终止引擎并
拒绝所有未完成任务；之后不再自动重建。取消/关闭/页面切换不得将旧结果提交到
Canvas、文字层或搜索结果。同步底层 dispose 释放所有跟踪资源和字体，不重用旧
句柄；不承诺 Wasm 线性内存收缩或浏览器立即回收全部 RSS。

### 批注、表单及保存（WA-WEB-04 / WA-T14/15）

Web 开放原生已支持的类型化批注读/增/改/删与 AcroForm 字段填写，保留未知类型
为不可编辑记录，禁止执行 PushButton/JavaScript/XFA 动作。混合字段中不支持的
类型不能使其他可读字段消失。批注/Widget 外观可显式参与渲染，默认阅读器启用；
图层选择、Screen 意图、缩放及 includeWidgets 语义沿用核心。

每次修改携带模块实例身份、文档句柄及十进制 revision 字符串；对象必须仍属于
该页/字段树。跨文档/跨会话/旧 revision 请求明确拒绝。整次修改使用核心事务，
预留回复句柄/字节额度后才修改，返回新 revision；失败回滚且不得返回部分结果。
revision 不是提交次数，回滚后旧选择也会失效。常用编辑界面使用 PDF 用户坐标，
通过页矩阵与 Canvas 位置互转，不把显示坐标直接写入 Rect/QuadPoints。

保存保持现有安全策略，默认请求输出最多 64 MiB（实际受剩余缓冲预算限制），也可请求
更低额度。返回独立 PDF 字节及其状态 stamp，用户点击后才下载 Blob。输出超限、
只读 R4、签名来源及其他核心保护均明确失败，不自动解密、降级或移除保护。
必须验证编辑后原生/Web 严格重开、值/关系/外观与独立工具结果；旧视图和字段
选择在成功修改或回滚后重新读取。

页面 Widget 交互必须用 userToDisplay 与缩放投影 PDF Rect，CSS 控件自身的向下
坐标与 PDF 的向上坐标分别处理；CropBox、UserUnit、四向 Rotate、NoZoom 和
NoRotate 不得混用。隐藏、NoView、默认 View 图层不可见的 Widget 不创建页面
输入控件；只读字段/Widget 禁止填写。页面与侧栏控件共享同一字段请求和事务，
更新/回滚后一起重读，翻页/缩放/会话失效立即撤销旧控件。表单外观仍由引擎绘制，
浏览器控件只承担焦点与输入。可编辑组合框允许输入未列出的值并交由核心校验。

阅读器提供显式页面绘制模式：便笺、矩形、椭圆、自由文字、矩形高亮、线段和
手写笔迹。默认模式保留文字选择/字段填写；绘制使用 Pointer capture、页面逆
矩阵、有界笔迹点数及预览，pointercancel 不提交，成功/失败后重新获取快照。

### 来源内容编辑与页面操作（WA-WEB-05 / WA-EDIT）

content(page) 返回默认 View 可见内容的 revision 快照：字符带可验证的原生来源，
图片带本次列表索引、显示矩阵及几何四边形。外部只提交字符范围或图片列表索引，
不得提交任意原生指针、流地址或伪造的调用链。应用时在同 revision 内重新解释并
定位；字符范围的首批界面限同一绘制事件（含 TJ 和嵌套 Form），按原字体编码，
默认补偿原推进，删除不宣称安全脱敏。跨事件范围不隐式扩大，明确拒绝。

图片支持顶层/嵌套来源的删除、显示空间仿射变换和 PNG/JPEG 替换。图片字节独立
传输，最多 8 MiB，先计入 Worker 队列预算再复制/转移；JSON 保持 64 KiB 上限。
新资源、调用链隔离、内容更新和回复构成一个事务；失败不能留下孤立新资源或
部分页面修改。嵌套变换须消除图像归一化矩阵，再从显示空间换回当前用户空间。

页面提供旋转、复制（显式名称前缀）、删除（默认拒绝被引用页）及完整排列重排。
操作后的 pageCount、revision、书签/链接/字段及当前页 UI 一起刷新；不通过重写
页面编号绕过原生关系/权限/签名保护。跨文档导入和带新字体的子集写入后续继续
接入既有原生接口，不将当前首批路径当作整个 WA-WEB-05 已完成。

内容编辑元数据最多 8 MiB，超出时只禁用该页内容编辑，保留已有页面显示、页面
操作和保存入口；文字层仍遵守其自身限额。编辑完成与翻页/换文件竞争时，仅刷新
仍打开的目标文档，页数从新状态读取，旧错误不能覆盖新文件状态，异步刷新错误
必须被处理。协议 2 的 apply 单独传输可选资产，不把资产数组塞入 JSON。清理
资产输入前必须读取结构化错误，普通坏图片不能被误判为内部故障并销毁会话。

任何成功编辑或回滚后，旧搜索结果/高亮、链接、书签和文字选择立即失效，再从
当前 revision 重建；编辑期间的旧渲染结果不得回写，换文件不得出现旧编辑错误。

### 跨文档页面导入（WA-WEB-05 / WA-EDIT）

导入引用同模块/Worker 中已打开的源文档及其精确 revision，不将裸对象编号跨文档
复用。目标和源 stamp 均验证；源失效、关闭或旧 revision 明确拒绝。目标操作要求
修改和组装权限，源复制权限沿用安全策略。原生拒绝的加密/较新版本来源、关系和
签名组合不自动降级、解密或丢弃关系。页面选择非空且有界，名称冲突用显式前缀
处理，导航迁移必须显式选择。整个导入与回复使用一个目标事务，失败无部分修改，
源内容不修改；源关闭后导入页仍应可渲染、编辑和保存重开。

阅读器使用独立源文件/密码输入，不替换当前文档；读取和打开来源后再次验证当前
目标视图，过期结果只关闭临时来源。密码不保留，导入后及时释放来源；保存仍保留
目标加密策略。公开接口不因上传/读取文件而自动提交编辑，导入由用户按钮触发。

### 新字形子集写入与显式跨度（WA-WEB-05 / WA-FONT-06）

replaceText 可显式提供有授权的原始静态 TrueType 字节与 FontDescriptor 元数据，
复用原生原始 GID 子集校验和嵌入权利检查；不能把任意替代显示字体当作原字体
源写回。字体资产最多 32 MiB，与 JSON 分开传输并计入共享队列/事务预算。字体
不匹配、缺字、不允许嵌入/子集或预算不足均整体回滚，不能遗留字体资源。
子集前缀由当前文档内的有界扫描生成，包含直接/间接声明并避免重复，不要求
调用方猜测六字母标签。布局/辅助表省略必须显式选择，不自动 shaping/重排。

replaceTextSpan 为显式跨绘制事件/调用链跨度，仍用字符记录半开索引和当前 stamp。
默认 View 选择若跨越隐藏图层文字则拒绝，不能把可见字符范围悄悄扩大到隐藏内容。
非文字绘制保留，未选择的共享调用/页面隔离；普通 replaceText 继续限制同一事件。

### 多字体注册与选择契约（WA-FONT-02/05/06）

显式内存 provider 最多 128 个 face，每个源最多 32 MiB、源合计最多 64 MiB，
同时受 Web 总保留额度约束。身份为源字节 SHA-256 与集合索引；同身份和相同
元数据重复注册幂等，同身份不同元数据拒绝。同名不同字节保留独立身份。
选择元数据包括 family、最多 16 个 aliases、weight（1～1000）、italic、serif
和最多 16 个语言提示；family/alias 最多 256 字节、语言提示最多 64 字节。
只允许原字体证实的单 Unicode 字符映射进入回退；未映射或需多字符 shaping
时明确失败。先检查实际字符覆盖，再按名称别名、语言、倾斜、衬线与字重选择，
同分按注册顺序稳定选择。原嵌入字体与 Base14 路径优先；旧单字体入口保留为
最后显式回退。同一绘制事件可使用多个 face，保持原 PDF 的位置及推进。
注册不修改 PDF，字体随引擎释放/Worker 失效清除；无隐式系统字体或网络访问。

### 字体预检与文档约束注册（WA-WEB-06 / WA-ASSET-01/02）

fontRequirements(page, renderOptions) 使用默认 View 与相同批注/Widget/缩放选择，
只读遍历绘制事件及使用的图案、软蒙版，Type3 沿用已解释的实际图形。字体按本次
页面快照中的实际 face 身份分组，fontId 仅在 stamp/page/选择内有效；不同资源
即使 BaseFont 相同也不合并。返回缺字、缺 Unicode 和需要 shaping 的独立诊断，
以及已选注册字体身份，选择规则与绘制共用。遍历深度/操作/字符/字体数有上限。
无复制权限时不向 JS 返回 Unicode 码点，仍可按名字/样式配置完整字体资源。
预检完成仅证明字体需求已满足，不替代页面渲染或其他功能支持的验证。

异步加载完成后通过 document.registerFont(bytes, metadata, stamp) 注册；在同一
原生调用中验证目标仍存活且 revision 相同，防止状态预检与注册之间关闭/编辑的
竞态。Worker 消息绑定当前文档与 session，排队字节副本计入原有预算；已关闭、
跨模块或过期来源不改变字体注册。显式 engine.registerFont 仍可供应用预注册。

预检 stamp 还携带单调字体清除代次 fontEpoch；清空多字体或成功替换/清空旧单字体
会使之前的字体请求失效，追加注册不使同一批请求互相失效。文档约束注册对携带
该字段的请求在原生调用内校验，避免清除缓存后迟到加载把旧字体重新放回。

Type3 验收区分已支持的 CharProc 图形与原生明确不支持的操作：未使用的字体资源
和 Type3 提取元数据不产生字体请求；CharProc 内 BT 保持原生 unsupported 负例，
不通过预检静默忽略，也不把它当作已有 Type3 图形正例。

### 显式字体资源服务（WA-ASSET-01～06 / WA-WEB-06）

资源清单版本 1，最多 128 个 face。每项包含 id、资源版本、格式、SHA-256、精确
字节数（1～32 MiB）、集合/选择元数据及非空许可标识，可附许可说明/链接与覆盖
范围提示。同一哈希/集合身份不能配置冲突项。清单、URL 和回调都来自应用显式
配置，PDF 名称仅用于选择，不拼接成网络地址。HTTP 加载限制在配置根下，拒绝
跨根与重定向，省略凭据；自定义加载回调显式提供 AbortSignal。

服务最多同时接收 16 个页面请求、启动 2 个资源读取，单次最多尝试 16 项（可配置
1～128），每项超时默认 15 秒（可配置 100 ms～60 秒）。相同配置身份的资源读取
合并，取消一个订阅者不取消其他订阅者，最后订阅者取消才中止读取。每轮注册后
重新预检，无法映射或需要 shaping 的项目不靠加载猜测；没有候选时保留未解决
报告。注册必须使用原始文档/revision/fontEpoch，变更后失败而非自动采用新身份。

JS 字节缓存和读取暂存合计使用独立的 64 MiB 保守额度，先计量再读取/拼接，超限
淘汰 LRU 或拒绝；不冒充 Wasm/浏览器总内存。缓存键包括清单版本、内容、集合、
配置和许可身份，内存与 CacheStorage 命中均复验哈希/长度/格式；校验失败不注册。
离线模式只使用已有缓存，不触发网络/回调。持久缓存独立命名，限定本服务写入的
数据与淘汰额度，浏览器仍可自行回收或拒绝存储；不能将配置持久缓存视为离线已
可用。只有成功的文档注册才能提交新缓存，取消/迟到/失效结果不提交。清缓存按
顺序等待已有缓存写入再删除，避免迟到写回；dispose 取消并释放本服务拥有的
资源，不隐式清除其他文档共用的原生注册。请求/命中/注册/失败/取消等事件可观察。

HTTP 清单字节数与 SHA-256 针对 Fetch 解码后的字体程序；传输压缩的 Content-Length
不是字体长度，不能据此拒绝有效 gzip/br 响应。逐块读取限制解码后字节数，最多
100,000 次流读取并定期让出事件循环；不保存无限零长度分块或先收集不受限列表。

同一缓存命名空间的服务实例共享串行读写队列；环境支持 Web Locks 时协调其他
上下文中的本服务实例。旧实例取消写入后的清理先结束，新实例才读取/写入，避免
旧清理删除新配置的缓存。该协调不替代浏览器存储配额和自主淘汰规则。

### 语言上下文与字体选择（WA-FONT-02/05/06）

绘制快照携带 Catalog Lang 默认值及 BDC Lang 的最内层覆盖，EMC 恢复外层，
BMC 不更改语言；q/Q 不结束 marked-content 范围。Form、图案、软蒙版及 Type3
解释子程序继承调用处上下文，局部覆盖不泄漏到调用者。空语言明确表示未知；
无效或过长语言不猜测区域，源元数据仍保留。语言只影响可证实 Unicode 的替代
face 排名，不更改编码/GID、字宽、提取的 ActualText 或引入 shaping。

预检按实际字体资源与语言分组，报告 language；同一字体在不同语言范围中不合并
需求。实际绘制和异步候选选择使用相同的大小写不敏感语言排名：精确、兼容前缀、
同主语言兼容脚本、未知、冲突脚本/语言依次退后，覆盖仍是前提。显式别名优先于
语言提示，同分按注册顺序。语言字符串与栈深/操作/快照保留均计入既有有界路径。

预检同时返回全部可证实单标量 codepoints 与缺字 missingCodepoints；无复制权限时
两者均清空。selected 的 preference 使用原生名称/语言/样式排名。资源服务在覆盖
已满足后仍可有界尝试清单中排名更好的语言字体，每次注册后重新预检；没有更优
候选即结束。complete 表示映射和覆盖满足，不承诺配置之外的语言字形或排版。

### 字体格式与集合验证（WA-T06/07/08）

全部 14 个 Base14 face 逐项验证。使用现有独立生成的八页字体矩阵验证嵌入 Type1、CFF、OpenType/CFF、TrueType、
非 Identity CID/W2、不同字节长度和 Unicode 独立于绘制编码；稀疏 CID CFF 另行
检查 CID 42 到 GID 2。注册路径逐格式通过资源服务的格式/哈希检查，再与显式
单字体和原生参考像素对照；TTC 同字节的两个索引必须独立且实际形状不同。
错误程序或索引不能改变原注册，PDF 字宽、CJK 的 W2 竖排推进和保存后的来源结构保持。CFF 文字
轮廓裁剪继续为明确拒绝，不以覆盖预检成功代表该绘制能力已支持。

真实简繁中、日、韩字体从应用显式提供的同一有许可集合派生有界测试子集，
记录原集合/索引、命名字体、子集哈希、FontTools 字符到 GID 和许可副本。生成
工具不下载或查找系统字体，运行时仍只接受配置字节。未提供该集合的环境必须
报告真实 CJK 未验证，不能用合成语言标签替代。当前矩阵固定零像素差异，
所有格式须非空白；既有其他 CJK 样本的已记录边缘差异不由本矩阵自动关闭。

### 有界像素区域渲染（WA-RUN-06 / WA-T12/13 / WA-WEB-08）

新增 renderRegion(page, region, dpi, options)，region 为该 DPI 下、已有页面旋转/
CropBox/UserUnit 后的左上角整数像素 x/y/width/height。区域必须完全位于 ceil
后的整页像素尺寸内，不隐式裁切或缩放；Web 单边最多 1024，单块最多 1,048,576
像素，逻辑整页单边最多 32768。当前分块 DPI 为 1～1152，整页仍为 1～288；
现有绘制选项、权限及资源限额保持。后续资源扩展及未完成的浏览器验收见
[资源计划](resource-plan.md)和[策略](resources.md)，不将旧门禁外推为新实现已验收。
输出为独立 RGBA 副本，附区域 x/y；不返回跨 memory.grow 的借用视图。

渲染器只分配区域大小的颜色/透明组/软蒙版/页面空间图案 surface，使用整数平移
保留全局采样相位；独立重复单元的局部图案不继承页面裁切。嵌套页面空间图案的
超采样同时放大区域坐标和尺寸。每块仍按现有预算解释页面，不能假称只解析可见
对象；复杂单元、原图解码及解释预算超限仍显式失败，不静默降分辨率。

同步/Worker 均校验并复制参数，保持任务取消和会话失效语义。拼接分块在整数倍率
须与同 DPI 整页逐字节一致；非整数倍率单通道最多 1 级浮点舍入。曲线的平坦区域
和 alpha 必须一致，记录全部差异数量/最大值，不能隐藏几何空隙或整块缺失。
覆盖文字/图片、非整数及非均匀 DPI、旋转/裁剪、图案、软蒙版及透明组。超出
整页 surface 限额的页面须有小块正例；越界/零尺寸/过大区域和失败释放重试须
有负例。
该底层接口是后续可见区域调度、有限缓存和
fit/视图旋转的基础，不以单个接口关闭整个阅读器显示需求。

小 surface 的抗锯齿裁切可能改变接近块边界的矩形角覆盖率。每块分配最多两像素
边缘留白，输出直接读取请求区域；留白及所有离屏 surface 计入原有 aggregate
像素/字节限额，不返回留白字节。页面空间子渲染器继承实际分配区域，不重复
添加留白；超采样同时放大该区域，末端取整产生的尾行/列以透明零贡献处理。
非整数倍率的 1 级门限仅用于浮点采样舍入，不允许用它掩盖块边缘的裁切错误。

补片渐变的显示统一使用已有的有界前向栅格器（几何平直误差最多 1/8 像素），
不再根据当前区域内是否碰到难以认证的反向采样点切换显示算法。三角重心和
采样位置保持全局坐标，区域原点单独传入，仅分配该区域的 roots/output；
认证点采样公共 API 及其失败语义保持。整页与分块必须使用同一显示算法。

区域描边按全局 raster 变换展开轮廓，再以整数原点绘制，shader 同步转换坐标；
曲线细分不随块原点改变。回归包含旋转/UserUnit 下的单选圆形、虚线图案描边及
软蒙版描边。字体局部位置也先以全局比例求取，再施加分块平移。已知中文原生/
Wasm 字形边缘差异单独固定输入、范围和误差上限并目视复核，不放宽同一 Wasm
整页与分块的门限；Base14 Widget 另有严格原生对照。

曲线路径、描边轮廓和曲线裁剪的全局细分仍在试验。正权有理 Bernstein
控制凸包将弦误差约束为 1/64 像素，再吸附到 1/256 像素网格；相对于输入
轮廓的几何误差小于 1/32 像素，有独立曲线采样和预算负例。细分深度最多
24，每个 renderer 每页工作额度为 max_operation_bytes/16，超限明确失败。
扩大到细线/描边文字后仍有扫描转换差异，尚未通过验收；不得以几何误差证明
替代像素一致性门禁。试验中的 2 级边缘门限未通过，已撤回；正式整数倍率零
差异和非整数最多 1 级的目标保持，区域接口标为 experimental。

曲线路径覆盖率采用固定全局 64×64 像素单元、各侧两像素留白；
颜色/透明度/混合与原 shader 在目标上应用覆盖率，同一对象的不同单元
不得重复覆盖边界像素。不以整页离屏再裁剪替代区域分配。单元临时
surface、快照及既有图案 surface 计入像素/字节预算；单元数乘轮廓
点数计入本页工作额度，超限在该对象绘制前拒绝。该方案仍需浏览器
和独立视觉复验，不因定向原生门禁通过取消 experimental 状态。

固定覆盖窗口同样用于直线路径展开后的描边轮廓，以覆盖矩形批注
拐角等扫描转换场景；覆盖图使用 Alpha8，绘制时保持原 paint 的
shader、颜色、透明度和混合。预算暂按两个 RGBA 临时平面保守预留，
不把 Alpha8 节省量当作扩大公开上限的依据。

对无 shader 的纯色 Normal/SrcOver，Alpha8 覆盖率按连续同值水平段
绘制，源 alpha 以 `(alpha * coverage + 127) / 255` 量化到 8 位，
段绘制关闭自身抗锯齿但保留现有 canvas 裁剪。由此避免 A8 图像浮点
合成在原生 SIMD 与 Wasm 标量后端间的 1 级差异。扫描像素计入单元
工作额度；图案、软蒙版 shader 及其他混合模式维持原有覆盖路径。

曲线裁剪使用相同 64×64 全局覆盖窗口及两像素留白。覆盖字节只组装
当前 surface 范围的 Alpha8 蒙版，再保持现有裁剪栈相交；嵌套的
W/W*、洞、图案与透明混合须独立复测。蒙版副本、单元 scratch、
仍被 canvas shader 持有的 Alpha8 图像合计检查像素/字节预算。
图像账本按 Skia 引用计数在 restore 后清除，父组仍持有的图像
继续计费；子图案/软蒙版分配须扣除父裁剪存活量。全部生成工作
在应用新裁剪前检查限额，错误由调用方恢复画布，不能返回半块图像。

RGBA8 渲染入口对已验证的常量填充/描边、图像、渐变及组透明度
统一先取最近的 8 位 alpha（半级向上）；形状采集仍强制 255。
软蒙版采样已为 8 位，不重复改变其值。此契约避免裁剪 shader
切入浮点管线时，让常量 0.5 的中间颜色由平台舍入方式决定。
曲线裁剪与半透明 Multiply 的内部像素须同时满足独立颜色断言、
原生/Wasm 零差异及整页/分块原门限，不能增加平坦区域豁免。

固定覆盖窗口适用于所有非轴对齐矩形的填充与裁剪，以及所有描边；
不能只检查原始路径中是否存在曲线。原始矩形经过倾斜变换后也要
纳入，空裁剪保持空集合语义；轴对齐矩形沿用直接路径。回归须含
斜边多边形、倾斜矩形的填充/裁剪及 TrueType 文字轮廓裁剪。

缓存字形的原点须先在全局 raster 平面取 1/256 像素网格，再减区域
整数原点；不能先减去远处窗口原点再决定字形亚像素相位。每个字形
仍使用原 face/GID/线性变换和现有 hinting。超过 Skia 仿射缓存
256 像素阈值的矢量字形使用统一路径覆盖，位图/空字形保留原回退。
字形原点变化须反向补偿页面空间 shader；轴对齐矩形也先确定全局
几何再直接绘制，不额外生成整页中间图。

验证中曲线附加的平坦区域/alpha 检查按实际曲线记录判定，不能
再以归一化工作额度是否消耗作为曲线标记；矩形现在同样消耗额度。
数值门限不变，非曲线的非整数复合组差异仍完整记录在原 1 级门限内。

隔离的 Emscripten 模块初始化时，Skia 字形缓存采用 2 MiB/2 个
strike，隐式 typeface 缓存关闭，资源缓存 4 MiB；模块 dispose
清除未使用的后端缓存。strike 字节账本不包含其持有的字体程序和
FreeType face，因此必须同时限制条目数。已注册字体由 provider
独立持有，不因缓存条目数而减少可用字体；宿主原生进程默认配置
不由 Web ABI 修改。此策略不是完整原生 RSS 硬额度，Wasm 最大
内存仍为 256 MiB。连续区域压力输入保持同一 engine/模块，
同步端大字形场景的线性内存扩展不得超过预设 128 MiB 回归限额，
Worker 同样跑完整序列，不以重建模块或减少次数消除分配失败。
FreeType 明确的 OutOfMemory 归为资源错误，不归为字体损坏。

### 阅读器适合模式（WA-WEB-08 / WA-09）

适合页宽使用滚动容器扣除 padding 后的可用宽度，适合整页同时约束
容器的可用最大高度；沿用 10%～400% 缩放边界，并在控件中显示实际值。
容器宽度或窗口高度变化后重新计算；手动输入缩放与 XYZ 显式 zoom
退出适合模式。页面几何沿用 CropBox/UserUnit/文档旋转后的 display points，
文字、链接、搜索和表单使用同一缩放。异步旧结果不得覆盖新模式。

### 额外视图旋转（WA-WEB-08 / WA-09）

阅读器提供 0/90/180/270 度顺时针视图角度，不修改页面 Rotate 或文档 revision。
画面、文字、链接、搜索高亮和交互层共用同一 CSS 矩阵，外框及适合模式
使用旋转后的尺寸；绘制指针先逆变换为 display points，再换算 PDF 用户坐标。
RenderOptions 新增 viewRotation（默认 0），只描述调用方在输出之外施加的
角度，不旋转 RGBA 或改变 region 坐标；用于 NoRotate 外观与预检补偿。
ABI flags 的低两位保持原注释语义，位 2～3 为额外角度除以 90，位 4 为可选的 target 图片解码策略；
拒绝其他位和非法角度。NoRotate 控件及外观抵消页面与额外视图旋转，
NoZoom 保持原 viewZoom 语义。Worker 快照参数、旧结果隔离和释放规则不变。

### 阅读器可见区域与像素缓存（WA-RUN-04/05/06 / WA-WEB-08）

阅读器使用 512×512 全局 raster 单元串行请求区域，不分配整页画布。
当前视口经额外视图旋转逆变换后，向外对齐单元并尝试预取一圈；
可见块优先，再按距视口中心排序。视口画布独立上限 32 MiB，
预取超限先去除预取，实际可见区域仍超限则明确资源错误，
不静默降低 DPI。单个在途块最多 1 MiB，JS RGBA LRU 缓存上限
16 MiB 且最多 128 项；来源键最多 2048 字符，避免微小块的
元数据随交互次数无限增长。三者分别计量，不冒充整个浏览器或 Wasm 的 RSS 上限。

滚动替换尚未派发的需求，已派发块自然完成；同一调度器至多
一个区域请求在途。旧视口结果只能进入仍有效的源缓存，不能覆盖
新视口；文档／generation／revision／字体 epoch／DPI／缩放／
额外旋转共同区分缓存。页面切换和缩放清空显示，编辑、字体重配、
文件关闭／替换和会话失效清空缓存。resize 和 DPR 改变刷新对应
窗口或重建来源，pagehide 释放显示／缓存和观察任务。

补充所有权约束：首屏就绪等待跟随同一有效来源的最新视口，不能
因滚动替换计划而跳过文字层／编辑面板。字体服务每次成功注册
记录本地注册序号，与原生清除 epoch 一起区分像素缓存，防止
新增更优 fallback 后复用旧页像素。新窗口先提交已有缓存块，
再接收新块，避免新到达块提前淘汰本窗口尚未提交的缓存。
画布两轴先重置到 1，再设置新尺寸；中间分配同样不得越过
32 MiB 上限。仍在同一窗口内的滚动也重排未派发块的可见优先级。

### 字体与 CMap 资源交付清单（WA-ASSET-03/04/05 / WA-T10）

资源打包仅使用显式本地输入，不发现系统字体或下载资源。内嵌
Base14、AFM 与 CMap 分组记录源字节、构建变换后的字节、哈希
和原始许可／来源说明；CMap 按构建器相同规则去除整行百分号
注释。对应 Wasm 必须匹配已通过的 WA-03 构建且结构校验通过，
逐项检查载荷的精确字节序列只出现一次，记录此检查的范围，
不将它扩展为对任意编码形式的重复分析。源字体／映射／许可
均进入构建源码清单。

可选字体以显式资源描述、本地文件和完整许可文本输入；复用
真实运行时元数据校验，另核对大小／哈希／格式头，不冒充原生
face 或字形覆盖验证。每源最多 32 MiB、独立程序合计 64 MiB、
最多 128 个资源；相同内容的集合 face 只复制一次，输出 URL
以内容哈希命名，保留各自索引。重复内嵌程序默认拒绝，只能
显式选择并记录；不顺带复制已内嵌的整套字体或映射。

资源包保留许可原始字节并记录指纹，不写入本地来源路径。
记录真实原始／gzip 9 测量体积，不把目录大小当下载大小；
压缩体积不代表服务器已经提供压缩响应。打包使用新目录和
暂存验证，不覆盖已有目录，验证失败不提交目标。此包只处理
字体／CMap，不代表所有依赖许可、完整 SDK 或发布已验收。

## 页面解释复用与优化构建（WA-PERF）

- WA-PERF-01：浏览器默认使用独立的 `wasm-release` profile，Rust opt-level=2、关闭调试信息与调试断言，并对最终 Emscripten 链接启用 O2。保留显式 dev 构建用于定位；原生默认 profile 不变。优化产物必须经过像素和交互回归，不能用构建成功替代验证。
- WA-PERF-02：页面准备与区域绘制分开。`prepare_page(page, annotations)` 返回带文档身份和 revision 的不可变快照，区域坐标不参与解释缓存键；渲染专用准备的 DPI 和图片解码策略参与缓存键。默认可选内容选择参与准备；批注 intent、widgets、viewZoom 和 viewRotation 必须区分。跨文档、成功编辑和回滚后的旧快照均拒绝回放。
- WA-PERF-03：解释器按原始间接字体资源对象身份共享已解析字体及其 CMap，禁止按 `/F1` 或 BaseFont 名称跨资源作用域共享。直接字体保持局部解析；Type3 保持原来的操作预算及 revision 约束，不纳入持久字体缓存。
- WA-PERF-04：每文档页面缓存最多两项、32 MiB 估计存储；字体缓存最多 32 项、16 MiB 估计存储，且均受 max_operation_bytes 截断。估计包括 Rust 容器、字形映射和程序数据，并非统一 allocator/RSS 硬限额。超大快照、容量不足或缓存分配失败回退为不保留；失败解析不缓存。revision 在下次访问时淘汰旧缓存；显式清除和关闭文档释放缓存引用，调用方自行持有的快照由其管理。
- WA-PERF-05：以相同输入、区域、倍率和浏览器比较冷／热耗时；优化前后像素必须对照。对缓存命中、LRU、拒绝保留、文档隔离、编辑／回滚失效以及批注参数分别验证。本阶段不以调整分块调度或增加线程掩盖重复解释成本。


## PDF 输入预算调整（WA-INPUT）

打开文件、拖放、Worker 协议、同步接口和页面导入均移除独立的 8 MiB PDF 大小限制，界面不再显示该限制。输入分配仍在 64 MiB 共享缓冲预算内；打开期间输入与源码副本的同时保留、字体及其他文档均计入预算。容量不足返回资源错误，不宣称文件大小无限制。能力报告使用 `maxInputBytes: null`、`maxBufferBytes: 67108864` 表示这一语义。

默认保存请求同步使用共享缓冲预算作为上界，实际分配受剩余额度约束，显式较小 maxBytes 继续生效。替换图片和内容元数据的 8 MiB 限制、字体程序、解码、绘制与模块线性内存限制保持各自语义。验证必须包含超过 8 MiB 的有效 PDF 在同步、Worker 和阅读器中的打开、非空白渲染、保存与重新打开，以及共享预算拒绝后的资源释放。


### CFF 字体冗余 CIDToGIDMap 兼容

CIDToGIDMap 仅在 CIDFontType2 上解释和验证。CIDFontType0／OpenType-CFF 即使带有该冗余条目，也继续使用嵌入 CFF 的 charset 映射，不将 CID 当成 GID，也不将无关条目作为 TrueType 映射流解码。同步、操作预算和 Web 路径应保持一致。稀疏 CID 用例必须证明 CID 42 仍选择物理 GID 2，Type2 映射的两字节条目、大小与错误校验继续生效。

### 裁剪遮罩的有效范围

非矩形裁剪遮罩仅存储与当前光栅区域相交的路径边界，保留全局原点；离屏空交集使用透明遮罩。64 像素全局规范单元及 halo 继续用于覆盖计算，裁剪边界缩小不能改变逐像素 alpha 或分块接缝。遮罩按实际尺寸计入工作、活跃像素和字节预算，不通过提高预算处理大量小圆角裁剪。每页可复用最多四项、至多 2 MiB 且受绘制预算进一步约束的遮罩缓存，键包含规范路径和填充规则；区域／页面切换清空缓存，缓存持有的图像仍计入活跃遮罩账本。

### 书签读取错误与正文隔离

阅读器加载书签遇到格式错误（包括非法 UTF-16 和损坏的 PDF 字符串边界）或不支持的导航类型时，清空不可用书签、在书签栏及阅读状态中显示明确提示，继续正文绘制。低级 `outlines` 接口和 PDF 文本字符串严格解码规则保持原语义，不对原字节补零、猜测恢复或自动重写。资源、权限、内部、取消及会话失效等错误继续传播，不伪装成普通元数据警告。文件切换、重开和编辑刷新均须隔离旧请求的警告状态，正常 Unicode 书签和导航不得退化。
