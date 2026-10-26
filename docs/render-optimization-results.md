# 渲染修复与优化验收

日期：2026-10-26。[实施清单](render-optimization-plan.md)中的首轮实现与复测已完成。
本记录取代历史报告中的 MuPDF 比较结论；没有改变资源上限或三方像素门禁。

后续的[第二轮优化](render-performance-round2.md)已完成，最新比例、CPU／RSS 及
前后数据以该记录为准；本文件保留第一轮结果。

## 修复及实现

| 项目 | 实现及边界 |
|---|---|
| RF-01 Type1 失败 | 确认输入实际为普通 Type1 程序包装成 CIDFontType0，编码使用物理 GID。Compatible 模式仅对 FreeType 确认的 Type1 格式恢复有界 GID 映射，并记录 `FontProgram` 结构诊断；Strict 明确拒绝此非标准组合。真正 CID-keyed CFF 继续使用 charset，不推断 identity。 |
| RF-02／RP-01 曲线与矢量 | 将规范化后的轮廓按全局 cell 建立有界空间索引；每个 cell 只提交相关轮廓，仍在同一覆盖操作中保留 winding／even-odd、孔洞和重叠。索引存储、构建和访问计费；覆盖预算在绘制前预检。原生及浏览器密集曲线／矩形都在原预算内完成。 |
| RP-02 图片 | ICC 色彩转换改为 128 像素的固定栈批次，减少逐像素锁定／查询变换缓存与派发。保留 Decode、Range、渲染意图和转换语义；Matte、蒙版、显式 Alternate 回退及其他位深仍使用原有通路。长度、输出预算与异常数据继续校验。 |
| RP-03 RGBA | 在实际扫描确认 surface 完全不透明后，使用 opaque RGBA 读回，省去无用的浮点反预乘；含透明像素的 surface 保留通用转换。保持 owned buffer 和输出格式。 |
| RP-00 诊断 | 对图片、曲线、空白页分别完成 macOS `sample` CPU 采样；原生 worker 新增首次和重复回放／RGBA 读回拆分数据，正式总计时继续包含二者。 |
| 比较工具 | 修复 MuPDF pixmap 的 separations 构造，新增与标准 PyMuPDF DeviceRGB 输出对照的回归。原来非空的空 separations 会进入额外转换路径，不能当作普通 RGB 基线。 |

同尺寸 surface 复用未纳入最终实现：它影响既有保存栈／裁剪契约，已撤回。
不通过新增长期 RGBA 缓存、提高预算或替换字体后端换取本轮分数。首次字体加载
与画布初始化仍有后续优化空间；本轮公共读回优化也降低了文字页面的回放成本。

CPU 采样中，原小 JPEG 样本的 752 个主线程采样里，656 个位于图片 RGBA 转换，
可见 ICC 逐像素变换缓存锁。空白页主要落在 `read_pixels` 的浮点反预乘与白底
合成；曲线样本主要落在每个 cell 重复构造整页路径的 Skia 边。采样仅用于定位，
没有并入正式性能计时。

## 正确性结果

- 原 7 个报错组合全部恢复渲染：Type1 三类 × 两个 DPI，加曲线 144 DPI。
- 正式复测 **420 次计时全部成功，无 trap／超时／渲染报错**。
- 三方图像门禁由历史 16/28 提升到 **22/28**。统计中排除两组空白基线，
  当前速度比例使用 20 个通过的非空组合。参考适配器也已修正，新增通过数不能
  全部当作 rzpdf 像素算法变更的贡献。
- 原来能够渲染的 **21 个组合，所有选中页面的 RGBA 与旧输出逐字节一致**。
  原始报错输入没有可比的旧图，改由独立引擎、局部回归和浏览器检查验证。
- 新恢复的稀疏／多页 Type1、144 DPI 密集文字和曲线通过三方门禁。
  新恢复的密集 Type1 72 DPI 可渲染，但因字形覆盖差异仍未通过门禁。
- 已目视检查新恢复文字、密集文字及 144 DPI 曲线的三引擎 contact sheet。

## 性能前后对比

同一 macOS x86_64 机器、同一组生成文件及 owned RGBA 计时边界；没有其他构建或
测试与正式性能任务并行。原基线三次试验，本次五次试验；每次两次预热、十次重绘。
下表是 rzpdf 重绘中位数，单位 ms；降低比例是本次相对旧值，不是置信区间。

| 场景 | 优化前 | 优化后 | 耗时降低 |
|---|---:|---:|---:|
| 曲线，72 DPI | 157.96 | 58.20 | 63% |
| 密集矩形，144 DPI | 150.05 | 53.03 | 65% |
| 小 JPEG，144 DPI | 154.91 | 91.76 | 41% |
| Flate 图片，72 DPI | 141.19 | 82.26 | 42% |
| 空白页，144 DPI | 11.65 | 5.84 | 50% |
| TrueType 文字，144 DPI | 11.54 | 6.86 | 41% |

对原来同一批 **14 个通过的非空组合**逐项配对计算几何均值，rzpdf 首次耗时约降低
**28%**，重绘约降低 **45%**。这批固定子集相对 PDFium 的耗时比例从首次
4.73×／重绘 3.08×，变为首次 **3.42×**／重绘 **1.76×**。
改善明显，但尚未追平 PDFium。

本轮扩大到 **20 个当前通过的非空组合**后，修正适配器的结果如下。不能将它与旧
14 个组合的几何均值直接当作同范围改善量。

| 操作 | rzpdf／MuPDF | rzpdf／PDFium |
|---|---:|---:|
| 首次页面 | 3.02× | 4.12× |
| 显式保留页面重绘 | 2.94× | 1.85× |
| 后续页面 | 2.91× | 2.07× |

大于 1 表示 rzpdf 更慢。后续页面只有两类多页输入、两个 DPI，共四个配对组合。
MuPDF 旧倍率因适配器问题已作废，本表是重跑结果；绑定／后端差异和首次字体成本
仍计入操作耗时。不能外推为浏览器端的速度比例。

未观察到旧有可测组合的首次或重绘中位数超过 10% 的退化。小 JPEG 144 DPI 的
rzpdf 整个 worker 峰值 RSS 约 34.78→34.82 MiB，大扫描图约 118.07→118.24 MiB；
未通过显著扩大驻留内存换取这些收益。RSS 不是纯引擎分配量或浏览器线性内存。

## 仍保留的六组图像差异

| 输入／DPI | 已查明的方向与处置 |
|---|---|
| `jpeg-scan`，72／144 | 源图／输出中心采样对照显示 rzpdf 选择最近源像素，另两引擎在高缩小比下有平滑采样。`Interpolate=false` 的现有最近邻策略与启用插值路径有独立回归。保持当前策略，未声称每个 JPEG 解码／ICC 舍入差异都已消除。 |
| `cjk-dense`，72／144 | 墨迹位置基本一致，覆盖／字重不同；另外两引擎彼此也未通过 RGB 门禁。浏览器还确认 CoreText／FreeType 位图文字存在平台差异。保留原字体与图像，不靠替换字体或放宽门槛纳入排名。 |
| `rectangles`，72 | 发现并修复参考适配器颜色通路后，仍存在低 DPI 覆盖差异。rzpdf 明确有将亚像素非零描边提升为单像素 hairline 的[显示策略](transparency.md)，因此不能期待与保留分数像素线宽的引擎一致。 |
| `text-dense`，72 | 原字体报错已修复，新增可渲染输出显现低 DPI 字形覆盖差异，未通过 RGB 门禁。144 DPI 已通过。 |

这些输入继续出现在 `quality.json` 和逐项报告中，**不进入等输出速度比例**。
其中的策略选择和位图后端差异没有被包装成“所有 PDF 都渲染正确”。如需改变
最小描边或图片缩小策略，应另定产品语义和质量门禁；本轮不默默改变既有策略。

## 验证与复现

- 原生定向测试：渲染库 92 项、颜色 133 项、字体 181 项、图片 26 项通过；
  图片另有 1 项依赖私有 Financial 文件的既有 ignored 测试，未计为通过。
- 工具测试 5 项通过，覆盖质量门禁、统计纳入规则及 MuPDF 适配器颜色回归。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`、格式和 diff 检查通过。
- 固定工具链优化 WASM 构建与 TypeScript 构建通过。
- Chromium 直接调用／真实 Worker 共 **56 个组合检查通过**：同一 WASM 两条调用
  路径输出逐字节一致；向量、普通图片等要求与原生一致，文字验证墨迹范围和面积；
  大 JPEG 因浏览器资源策略执行受限降采样，单列验证尺寸、非空及直接／Worker 一致性。
- 既有 ICC 混合空间、预算错误恢复、稀疏 CID CFF、循环颜色引用和 Type3 原／反向
  FontBBox 的真实浏览器两条通路回归通过。

本轮是定向验证，没有重跑全量工作区集成测试，也没有发布或推送。
复现原生命令见[基准指南](benchmarking.md)。浏览器检查在生成 native 报告、构建
优化 WASM 和 TypeScript 后运行：

```sh
RZPDF_CHROMIUM='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
RZPDF_BENCHMARK_CORPUS="$PWD/output/benchmark-corpus-v2" \
RZPDF_BENCHMARK_NATIVE="$PWD/output/benchmark-optimized-20261026/quality.json" \
node web/tests/render-benchmark-regression.mjs
```

使用项目已安装的 Node 22.15.1 与浏览器路径。结果分别位于：

- `output/benchmark-optimized-20261026/`：完整性能报告、原始 420 次计时、质量检查、
  输入／二进制指纹、PNG／RGBA 和 contact sheet。
- `output/render-profiles-before/`：三类热点的采样记录。
- `output/render-difference-oracles/`：用于区分参考颜色通路和描边覆盖的独立小样本。
- `output/webassembly/render-benchmark-regression/report.json`：56 项浏览器结果与产物指纹。
- `output/webassembly/render-optimization-grid-build.json`、`icc-browser/report.json`：
  优化 WASM 构建及既有专项浏览器回归。
