# PDFium 语料缺陷修复计划

日期：2026-10-22。目标是在既有原生实现与 WebAssembly 架构上，定位并修复本轮公开语料暴露的内容错误及兼容性缺口；不自动推进编辑、JavaScript、XFA 或发布。本文不罗列实现文件。

## 基线与判断边界

本地 PDFium 测试资源固定于 `d20ccc016932c9ab97044835980592bedb2658cb`，模板展开后按来源去重共 615 个样本。参照使用已安装的 PDFium 149.0.7825.0 / pypdfium2 5.8.0，并非该 checkout 编译的同版本引擎。运行配置、输入/字体/二进制哈希及原始证据保存在 ignored `output/pdfium-content-20261022/`。

| 检查 | 基线 |
|---|---|
| 文档解释 | 332 正常、57 警告、226 报错 |
| 首页/中页/末页抽样渲染 | 657 页次；431 成功、226 报错 |
| 参照抽样渲染 | 648 成功、9 报错 |
| 两端均成功的像素比较 | 431 页次，其中 15 页触发人工检查 |
| 已取得页数的两端对照 | 未发现页数不一致；不包含未取得页数的打开失败 |
| 崩溃与超时 | 本轮均为 0 |

上述不是兼容率或已确认缺陷数。解释会扫描页面，图像对照只抽样；参照无法打开的文档可能没有进入渲染计数。警告包含结构恢复与 Unicode 映射诊断，不一律要求消除。现有归一化图像指标仅用于筛查，可能漏掉小面积错误。

## 执行规则

- 先复现和归因，再调整实现；每个独立根因建立自有最小回归或固定来源/哈希的上游回归。
- 合法支持范围内的画面错误优先；相同根因影响多样本时集中处理，不逐样本打补丁。
- Strict 保留格式与安全约束。Compatible 的容错必须有明确边界、诊断和预算，不以吞错、猜字符、忽略真实绘图状态来提升通过率。
- 将案例标为：确认缺陷、已声明未支持、可恢复损坏、应拒绝、外部资源/密码缺失、测试配置问题、待调查。未调查案例不得直接标记预期失败。
- PDFium 输出是独立参照，不是唯一正确答案；争议用例核对上游断言/预期图、PDF 规则及必要的第二引擎结果。
- 核心功能变更先更新相应支持说明/spec；涉及 Web 字体和 ABI 的内容沿用 WebAssembly spec 的 WA-FONT/WA-ASSET/WA-API，保持既有阶段门禁。

## 阶段与退出条件

以下为初始阶段定义；实际执行状态与尚未完成项见末尾执行记录。

| 阶段 | 工作 | 退出条件 |
|---|---|---|
| PF-00 基线与裁决 | 对全部打开/解释/渲染失败和 15 页差异建立案例账本；关联上游测试意图，区分 JS/XFA、密码、损坏和实际页面绘制；固定两端配置 | 每个异常有初步分类、证据、负责人阶段和可复跑 ID；两端批注/背景/缩放/字体范围明确 |
| PF-01 无报错但画错 | 优先检查大片空白、颜色排列、裁剪/透明和图案描边文字；对 15 页逐项裁决并修复确认缺陷 | 15 页均有结论；确认缺陷的最小回归及原始页复测通过；未修复项明确保留，不通过扩大阈值关闭 |
| PF-02 图像与颜色 | 按实际图像语义处理 JPX 多阶段过滤链、调色板/分量/alpha、颜色覆盖；定位 JBIG2 尺寸、样本长度、Mask/SMask 和 inline image 边界；再处理函数/色彩空间组合 | 每个启用子功能有正例、截断/冲突/越界负例和预算验证；相关组全量复测；支持矩阵明确未支持分支 |
| PF-03 字体与文字 | 定位缺省/异常 Widths、CMap、Type3 状态与绘制、描边图案、CID 类型及无 Unicode 替代；按需进入既有多字体 resolver 设计 | 度量与绘制分别验证；保留 PDF 推进量；不混用 CID/GID/Unicode；正常无 ToUnicode 的可绘制文档不因提取诊断失败；无法恢复映射有明确结果 |
| PF-04 有界兼容恢复 | 对合法但被拒绝的情况先纠错；再评估 xref、stream 边界、页树计数/类型、重复节点与缺失资源的兼容策略；优先处理可证明安全的恢复 | 每项恢复有 Strict/Compatible 对照、恢复诊断及扫描/循环预算；循环、歧义引用和错误资源不会无限遍历或静默猜测；密码/旧加密扩展另有明确决定 |
| PF-05 双端验收与持续回归 | 重跑 615 样本；把确认修复及差异页扩展到全文/定向高 DPI；将所有修复根因的代表页纳入真实浏览器；重复、乱序和关闭重开验证 | 无未解释的新增回归；目标支持范围内确认缺陷关闭；原生/Wasm 与独立参照证据分开；超时/资源拒绝不伪装为成功 |

PF-00 → PF-01 → PF-02 → PF-03 → PF-04 → PF-05 为默认执行顺序。PF-01 定位时若落到图像或字体模块，先完成该根因的最小修复再回到队列；每批修复后运行受影响组，不必等待最终阶段才回归。PF-04 的高风险容错在画面错误之后处理。

## 首批调查队列

以下 ID 来自本轮固定清单，仅用于定位，不代表已确认根因。

1. PF-01：`0388` 大面积纯色不同、`0406` 图像颜色布局不同、`0348` 大片内容缺失；先排除损坏输入、默认行为和参照配置差异，再形成断言。
2. PF-01/PF-03：`0365`、`0366` 描边/图案文字差异；核对字体、文字渲染模式与图案坐标变换。
3. PF-02：JPX 链限制 10 页次、调色板/分量/alpha 限制 5 页次、JBIG2 字典/头部不一致 5 页次，以及图像长度/遮罩错误组。计数为抽样页次，组间按独立案例去重。
4. PF-03：字体宽度错误 8 页次、CMap 整数错误 3 页次、Type3 矩阵/CharProc 相关组。先判断格式缺损与允许缺省，再决定容错。
5. PF-04：xref 无恢复锚点 22 页次、页树类型/Count/重复节点、缺失资源组。它们可能来自故意损坏的回归文档，不能承诺全部接受。

旧式加密、非空密码、JavaScript、动态 XFA、批注交互及超预算页面独立列入范围决策；本计划不把它们自动变成必须成功打开或执行的需求。页面自身的静态内容错误仍正常归因。

## 验收方法

- 案例账本记录上游 ID/哈希、模式、失败阶段、预期行为、根因组、修复状态和证据位置；统计文档数、唯一页面数和页次时明确单位。
- 差分统一页面盒、旋转、DPI、白底和批注开关；当前主线是仅页面内容、48 DPI。差异页以更高 DPI 复核，字体相关对照显式固定可比较的字体策略。
- 不要求两个独立栅格器逐字节一致。全局指标用于定位；修复验收增加局部形状、颜色、覆盖或字形断言，不依赖一个宽松 MAE 阈值。
- 每次验收同时报告已解决、剩余确认缺陷、声明未支持、预期拒绝及未裁决数量。原始错误清单保留，允许列表必须有逐例理由与输入哈希。
- 单根因修复运行定向核心测试及受影响样本；最终运行既有安全/字体/图像/图形回归、615 样本和相关 Wasm 浏览器回归。出现新变更或失败时再扩大测试。
- 本轮完成标准是目标支持范围的确认缺陷关闭且无未裁决异常；不是让 615 个文件不分性质地全部显示成功。需要新增大功能时单列 spec/工作项，不冒充修复完成。

## 复跑入口

先构建当前 release 批测程序，再执行以下命令。每次输出目录使用新名称，避免覆盖既有基线。

```sh
cargo build --release -p pdf --example check_pdf -p render --example check_render
RZPDF_SUBSTITUTE_FONT=/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc \
python3 scripts/check_pdfium_corpus.py /home/zhengwu/workspace/Github/pdfium \
  --output output/pdfium-retest-NEW --count 0
```

现有执行器覆盖模板展开、全页解释、首/中/末页渲染及 HTML 对照。案例账本、固定失败清单重跑、受影响文档全页渲染、资源峰值/内存上限和独立评分门禁在 PF-00/PF-05 补齐；当前诊断脚本正常退出不表示语料全部通过。

## 第一批执行记录（2026-10-22）

计划尚未全部完成，以下为实际交付，不将未裁决失败当作预期成功。

| 阶段 | 实际进度 |
|---|---|
| PF-00 | 已建立 615 案例的可复跑账本和错误分组；保留 226 个仍有解释/渲染异常的文档，尚须逐组裁决，未生成自动放行名单 |
| PF-01 | 修复 5 个已确认的画面根因案例；另有 4 个参照分歧、3 个栅格差异和 3 个待审案例。待审为极细描边、高频图片采样、近相切径向渐变 |
| PF-02 | 支持 JPX 前置字节过滤器；保留高分辨率显式遮罩；按 Interpolate 选择采样。JPX 调色板/alpha/更多颜色及其余编码组未宣称完成 |
| PF-03 | 修复标准字体未定义字符的 MissingWidth/.notdef 行为；CMap/Type3 其余组仍需裁决 |
| PF-04 | 源页树 Count 可在 Compatible 中按有界遍历结果恢复，编辑后的错误 Count 仍拒绝；旧 hybrid 补充流不再覆盖增量表的新值，Strict 规则不变。其余恢复组未关闭 |
| PF-05 | 已重跑 615 个样本和 26 页 Chromium 回归；这是一批修复的验证，不是全计划验收 |

最新抽样仍为 657 页次：渲染成功从 431 增至 444，报错从 226 降至 213；解释正常/警告/报错为 333/69/213。10 个文档的整条已测路径由失败恢复；另有部分局部恢复。新增 3 个页数差异来自源 Count 声明大于实际 Kids 数，报告保留双方数值，不以声明数伪造页面。10 页仍触发原有视觉筛查阈值，人工裁决单独保存，不扩大筛查阈值。

证据位于 ignored `output/pdfium-repair-full2/`、`output/pdfium-repair-ledger/` 和 `output/pdfium-visual-independent-repaired/`，包含原始错误、逐例状态及 72/144 DPI 三方对照。渲染 67 项、核心 825 项通过（1 项忽略），安全/编码定向集和检查脚本通过。Chromium 156.0.8078.4 的 26 页与原生零差异，另检查页面混合指定颜色及细遮罩覆盖。原生与 Wasm 构建源码指纹一致；浏览器版本与此前不同，保留实际版本信息。

继续执行时先关闭 PF-01 的 3 个待审案例，再按既定 PF-02～PF-04 分组处理剩余项目；不能将本批记录作为“所有缺陷修复完成”的证明。

### 第二批执行记录

补齐最小设备细线策略、Compatible Type4 栈顶输出，以及单索引、最多 256 项、8 位灰度/RGB JPX 调色板及顺序映射。调色板逐样本颜色与失败预算释放回归通过；超出此限定的 JPX alpha/颜色/编码仍保留限制。

最终本批语料证据为 `output/pdfium-repair-full3/`，账本为 `output/pdfium-repair-ledger-final/`：449 页次渲染成功、208 报错，相比初始基线增加 18 个成功页次；13 个文档的已测整条路径从异常恢复。解释结果仍为 333 正常、69 警告、213 报错。无新增解释/渲染失败，无崩溃和超时。视觉筛查剩余 8 页，保留原始标记及人工裁决；原 15 页中的 6 项确认画面问题已修复，4 项参照分歧、3 项栅格差异，图片采样与近相切径向渐变两项仍待审。

核心 826 项通过（1 项忽略）、渲染 68 项、安全及编码定向 30 项、检查器 8 项通过。更新后的 26 页 Chromium 与原生零差异，指定混合颜色、细遮罩及亚像素描边覆盖断言通过。账本仍有 223 个带异常的文档，其中 193 个尚未最终裁决、26 个声明未支持、2 个密码要求、2 个资源拒绝；这些分类不是豁免，也不是新增缺陷数。因此 PF-01～PF-05 仍未整体关闭，不能宣称完整计划已经完成。

## PF-01 收口：采样与近相切渐变

PF-01 现已完成。本轮仅增加回归与裁决证据，没有修改生产渲染行为，也没有扩大视觉筛查阈值。原始 15 个差异案例最终为 6 个已修复、5 个参照分歧、4 个栅格差异；原始 8 个筛查标记继续保留，不能与未裁决案例混为一谈。PF-02～PF-05 及文档异常账本仍未整体完成。

- 高频图片：独立解码确认原始图像为 64×64 的 1 位黑白交替行，Interpolate 为 false。使用像素中心最近邻的独立公式，对当前固定批测程序在 48/72/144 DPI 的输出逐像素核对，差异均为 0。另补完整页面回归，覆盖 36/48/72/96/144 DPI；恰落在源像素边界时允许等距相邻样本，仍拒绝灰色混合、空白、错相位和透明输出。
- 近相切径向渐变：原始坐标经 f32 存储后，起点到终止圆心的距离平方减半径平方约为 `2.471031175e-7`，实际略在圆外。沿外向射线 `P=k*S, k>1`，任意非负参数 t 对应圆的距离减半径为 `(k-1)*|S|+t*(|S|-1)>0`，应保持未覆盖；圆心处的最大交点参数 `|S|/(|S|-1)>1`，应取扩展终点色。新增回归分别验证原始几何、略外、精确相切与略内，保留小系数的正负号，避免人为 epsilon 抹平真实几何。

证据：ignored `output/pf01-closure.json`、`output/pf01-oracle-verification-final/`，以及最新账本的逐例裁决。两个 Rust 定向回归和 4 个独立检查器测试通过。复跑：

```sh
cargo test -p pdf almost_tangent_radial_cones --lib
cargo test -p render packed_stripes_match --lib
python3 -m unittest discover -s scripts -p 'test_pf01_oracles.py' -v
python3 scripts/check_pf01_oracles.py --run output/pdfium-repair-full3 --output output/pf01-oracle-NEW
```

下一阶段按原顺序推进 PF-02：逐组裁决和补齐 JPX/JBIG2、图像长度、遮罩等剩余情况。不能以 PF-01 收口替代这些尚未完成的工作。


## PF-02 第一组：JPX 透明通道与 CMYK

本组接通顺序 cdef 定义的末尾非预乘透明通道和 CMYK 样本。PDF 层仅在
SMaskInData 为 1 时使用内嵌透明度，缺省/0 忽略它；2、异常通道关联、
颜色与通道数矛盾，以及内嵌透明度与显式遮罩同时使用仍明确拒绝。
通道拆分前检查同时保留的样本与目标缓冲区大小，失败预算释放有回归覆盖。

固定重跑 14 个原 JPX 失败文档，其中 3 个恢复解析和渲染，均未触发视觉
筛查；其余 11 个继续失败，主要涉及单 tile 包络以外、ICC、颜色空间覆盖、
非 8 位/分采样或截断输入。这是范围内定向复测，不能替代全部语料重跑。
证据为 ignored `output/pf02-jpx-retest-final/` 和
`output/pf02-jpx-closure.json`，保留逐例原因、输入及工作程序指纹。

核心 827 项通过（1 项忽略）、渲染 69 项、编码定向 15 项和脚本 8 项通过。
自造样本逐字节验证 Gray+alpha、RGB+alpha 与 CMYK 原始样本，并检查灰度/RGB
透明度使用、非法参数和预算失败释放。新增 4 页浏览器样本后，Chromium
30 页与原生输出零差异；Wasm 已重新构建，证据在 ignored
`output/webassembly/wa03-build.json` 和 `output/webassembly/wa03-browser/`。

PF-02 保持进行中，下一组继续处理 JPX tile/颜色包络与图像遮罩尺寸，随后
裁决 JBIG2 头与字典矛盾、图像长度及损坏流。尚未裁决的异常不能视为已修复。


## PF-02 第二组：单 tile 边界与等价组件编码

修复将“名义 tile 必须等于图像尺寸”作为单 tile 判断的错误。零原点下，
名义 tile 可以大于图像，后端按图像边界裁剪；实际像素与 scratch 额度仍按
图像计算。最大 u32 tile、非对称 tile 有逐样本及预算释放回归，零尺寸和
真正多 tile 输入仍拒绝。

另外接通组件索引有效、编码设置与主 COD 完全相同的 COC。不同分解级数、
无效组件索引和截断 COC 继续拒绝，不扩大非等价覆盖的解码包络。

固定 14 个 JPX 文档复测现为 7 个解析/渲染成功、7 个失败，比上一组新增
4 个恢复文档；无新增失败，通过项均未触发视觉差异筛查。剩余原因包括
颜色空间覆盖、ICC、非 8 位/分采样、非零原点/多 tile，以及截断流。
此为定向复测，并非全语料统计。ignored 证据：
`output/pf02-tile-coc-retest/` 和 `output/pf02-tile-coc-closure.json`。

编码定向 17 项、JPX 单元 1 项和脚本 8 项通过。Wasm 已重新构建；Chromium
31 页与原生零差异。新增的独立 OpenJPEG 编码样本同时覆盖大名义 tile 和
等价 COC，由 OpenJPEG 校验源像素，再以独立坐标公式逐像素校验原生页面，
最后与浏览器比较。证据位于 ignored `output/pf02-tile-independent.json`
和最新 `output/webassembly/wa03-browser/`。

PF-02 仍进行中。遮罩尺寸适配尚未实现，下一步继续处理原生分辨率软遮罩，
并保留 Matte 的源颜色空间反预混合及资源上限要求。


## 固定批次修复与归因收口

本次按既定顺序处理完固定语料中确认的目标范围缺陷，并完成剩余异常的
逐组归因。这里的“收口”指这批确认修复及输入裁决，不代表所有 PDF 均受支持；
旧加密等原计划单列范围，以及格式、资源、编码包络的明确拒绝仍然保留。
原始失败不改成通过，也不生成自动放行名单。

| 工作组 | 本批完成内容 |
|---|---|
| PF-00/PF-01 | 保留固定输入、二进制、字体、参照引擎身份及原始视觉标记；全部当前标记已有裁决 |
| PF-02 | JPX 透明度/CMYK、PDF Indexed 调色板优先及低位索引、单 tile 边界/等价 COC；不同尺寸软遮罩与精度/Matte；有界尾行结束符恢复；直接内联 Group 4；JPEG 分配前尺寸校验；Gray-only 透明组及亮度软遮罩；复杂补片的有界显示栅格化 |
| PF-03 | 短 Widths 的 MissingWidth 尾项、损坏 ToUnicode 的兼容诊断与可靠映射回退、Type3 度量冲突保留推进及一致 alpha 整体合成；不猜任意替代字体 GID |
| PF-04 | 完整对象值后紧邻合法对象头时恢复缺失 endobj；从已验证 Kids 推导缺失源 Count；无歧义终端 DecodeParms 字典恢复；单个替代配置字典恢复；默认 View 图层配置接入查看器和比较工具 |
| PF-05 | 最终 615 输入重跑、58 个恢复文档全部 62 页、工作区回归、38 页真实浏览器、独立颜色/像素公式及补片 72/144 DPI 对照通过 |

最终原始统计为 657 页次中 496 渲染成功、161 报错，较最初 431 个成功页次
增加 65；58 个文档的已测完整路径从异常恢复。解释结果为 342 正常、102 警告、
171 报错。没有新增渲染失败、崩溃或超时。JPEG 矛盾输入从延迟绘制错误提前为
头部检查错误，这是分配前拒绝，不是新增可绘制文档回归。

剩余 178 个非通过文档已记录逐例理由和同输入哈希的独立元数据证据：
161 个格式/结构/值域或既有编码包络校验拒绝，7 个原计划单列的旧加密修订，
2 个需要密码，2 个达到资源限制，3 个缺少可靠字形解析资源，3 个源 Count
声明与真实 Kids 页数不符。最后一组的额外 PageNotExist 尝试仍保留在原始报告。
分类不等于完整符合性证明，也不要求其他阅读器必须采用相同容错边界。

原始视觉筛查仍标记 9 页，均有裁决，没有扩大差分筛查阈值。新增软遮罩
棋盘格差异在独立解码及 72/144 DPI 最近邻像素中心公式下无错误；精确落在
采样边界时保留等距相邻选择。复杂张量网格修复后的 72/144 DPI 对照未触发
视觉筛查。精确补片查询仍保留证书/未决错误，显示回退的 1/8 像素平坦度、
深度/工作量/内存限制及颜色、覆盖顺序另有正负回归，不将近似渲染冒充精确反解。

最终工作区 1,725 项通过、1 项忽略；检查脚本 12 项通过。Chromium
156.0.8078.4 的 38 页与原生逐通道零差异，构建源码指纹一致。所有恢复文档
的全部 62 页渲染通过，其中保留的一个筛查标记已有上述独立采样裁决。

证据位于 ignored `output/pdfium-repair-full9/`、
`output/pdfium-recovered-all-pages/`、`output/pdfium-repair-ledger-final9/`
及 `output/pdfium-batch-closure.json`。账本保留原始观察状态与裁决层，
`all_corpus_inputs_supported` 仍为 false；不能把本批收口宣传为生产级全格式支持。

### Real-document batch follow-up (2026-10-22)

The next pinned 10-document sample exposed three related compatibility failures:
underdeclared optional-content version, Form warnings without nested source
provenance, and the exact joined stream-ending token `endstreamendobj`. Parsed
Compatible input now diagnoses the version mismatch, carries Form warning call
paths into page reports with aggregate count/byte limits, and splits only the
complete joined token. Strict refusals and fatal/resource errors remain intact.
Project-authored regressions cover both modes, repeated nested invocations,
exact diagnostic limits, budgeted stream parsing and malformed token suffixes.

Full replay completed all 2,508 pages: page interpretation errors fell from 160
to zero and rendering errors from 376 to zero. Four documents were clean and six
retained warnings (including missing Unicode in four documents). All 2,132
previously successful page PNGs were byte-identical after the repairs. The 393
visual alerts at 48 DPI remain in the original report; every alerted page was
re-rendered against the independent reference at 144 DPI, with no alerts under
the unchanged metrics. This supports resolution-sensitive sampling differences,
not a claim of independent pixel equality or universal PDF support.

Workspace validation passed 1,729 tests, with one ignored test. The real browser
regression passed 38 pages with zero native/Wasm channel differences. Evidence
is retained in `output/sample-20261022-batch5-repaired/review.json` and its linked
reports. The earlier PDFium full-corpus report remains historical evidence;
this follow-up does not claim that corpus was rerun with the latest source.
