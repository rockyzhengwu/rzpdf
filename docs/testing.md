# 解析回归与异常输入验证

## 运行

```sh
cargo test --workspace --locked
cargo test -p pdf --test parser_robustness --locked
```

`pdf/tests/parser_robustness.rs` 不引入随机库或额外工具链：固定种子与生成算法保证失败可复现。

- 1,024 组手写语法/内容种子的删除、插入和替换扰动，在严格/兼容模式下检查对象解析和内容操作符解析不 panic。对象解析成功必须推进输入位置。
- 256 组现有 PDF 的截断/单字节替换，在两个模式下经过完整文档入口；打开成功时继续验证页数、页面访问和 DisplayList 构建。
- 2,048 组 Flate/LZW/ASCIIHex/ASCII85/RunLength 编码扰动，使用 0–64 字节输出限额；成功输出必须不超过限额。另有 Predictor 非法类型、零尺寸及非法位深专项回归。
- 使用小 Limits 约束 token、深度、流和对象数量；接受明确解析错误，不要求异常输入成功。

语法种子由项目测试代码生成，不含外部样本文本。完整文件测试复用仓库既有 `tests/path-test.pdf`，来源及许可仍需追溯；没有新增第三方 PDF。该测试不证明所有异常输入均安全，不检测视觉正确性，也不能代替覆盖率引导模糊测试。

## 待补齐的验收

1. 为真实样本登记来源、许可、支持特性和预期行为。CCITT 原缺失样本测试已改为程序生成 Group4 条纹数据，检查逐像素解码和 BlackIs1 两种极性，不再忽略；仍需独立真实编码样本以避免编解码器共享缺陷。
2. 已新增独立 [fuzz 工作区](../fuzz/README.md)，分别覆盖语法、完整文档、内容解析/解释和五类流解码；四目标本地 AddressSanitizer 短时覆盖率引导结果见 [验证记录](fuzz-smoke-validation.md)。更多种子登记、失败输入最小化和持续执行仍需验收，短时无崩溃不代表完整安全性。
3. CI 已配置 60 分钟任务超时，包含原生依赖构建；仍需远端执行验证并按运行数据调整。本地确定性测试不使用线程内超时，不能据此宣称所有死循环都已排除。
4. 增加独立结构工具和阅读器验证，及固定渲染对比样本。

P0-12 是否完成以 TODO 的全部验收为准，而不是单个冒烟测试通过。按用户要求，fuzz 实现、执行及 CI 接入暂缓，保留已有文件和历史记录。

## 完整工作区验证（2026-10-07）

```sh
cargo test --workspace --locked --quiet
```

同一次进程完整运行后退出码0；pdf单元515项、39个pdf集成目标合计230项、render测试30项，合计775项全部通过、0失败/忽略。日志共45条测试目标汇总，包含零测试目标；不把零测试目标计为额外测试用例。最近的Stitching/Type4参数/Lab与颜色工作流回归，以及当前工作区其余测试均包含在本次运行内。

平台Darwin x86_64；rustc 1.96.0 (ac68faa20 2026-05-25)，cargo 1.96.0 (30a34c682 2026-05-25)。git diff --check通过。构建输出仍有未使用项及本机`.cargo/config`弃用警告，未改用户全局配置或系统依赖。

本次只验证本地默认工作区测试，不运行独立fuzz工作区，不触发远端CI，也不代表独立阅读器视觉/颜色管理、全部平台或TODO全部功能验收。`.github/workflows/ci.yml`仍配置Ubuntu原生依赖与同类工作区命令，远端执行证据待补；P0-01保持未完成。本轮未提交。

## Stitching 函数回归（2026-10-07）

实现依据为 [ISO 32000-1:2008 §7.10.4 / Table 41](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf)：Type3 拼接单输入子函数，统一输出维度，按区间映射输入并裁剪到可选 Range。内部边界归右侧区间；末界等于 Domain 上界时以该子函数 Encode 起值求值。规范搜索摘要可检索，链接直接读取当前返回404；没有把失败下载计为独立验证。

```sh
cargo test -p pdf --lib --locked --quiet
```

最终504项单元测试全部通过、0失败/忽略，包含6项Stitching回归和1项Separation颜色转换回归。覆盖非0–1域、上下界输入裁剪、分段不连续边界、反向Encode、单子函数/嵌套多输出、Range裁剪、端点Bounds、宽域插值、非法输入/默认对象、子函数维度/Range拒绝、Sampled子函数、间接参数及子函数错误传播。Separation样本保存后严格重开，红/蓝/白RGB端点与预期一致。

首轮宽域样本使用指数数字语法导致解析失败，已改为PDF合法完整十进制并重新运行全部pdf单元。此处为确定性数值与库内往返，不是独立阅读器或完整Shading渲染验收；Type4求值、Pattern/Shading和操作级聚合预算仍未完成。本轮未重跑集成目标/全工作区，也未运行fuzz。

## Type4 参数构造回归（2026-10-07）

`cargo test -p pdf --lib --locked --quiet` 最终508项全部通过、0失败/忽略，包含新增4项Type4构造回归：

- 间接Domain/Range及间接数值，保存后严格重开保留二输入/一输出维度；求值仍返回UnsupportedFeature。
- 非法/缺失参数在未知滤镜解码前返回FunctionError；合法参数的未知滤镜保留FilterError契约。
- 参数单数组请求字节8成功、7拒绝，4个参数条目在max_objects=3时拒绝；解码失败后短输入可重试，文档修订号不变。
- 数组数值引用成环时可重复返回错误，随后合法构造恢复，修订号不变。

首轮只有未知滤镜错误类型断言失败（507通过/1失败）；按现有FilterError契约修正后重跑全部pdf单元通过。此处验证参数构造与库内往返，不验证Type4程序语法/执行、独立渲染或聚合内存；两数组与解码结果共存仍未纳入OperationBudget。本轮未运行集成目标、全工作区或fuzz，未提交。

## Lab 颜色数值回归（2026-10-07）

`cargo test -p pdf --lib --locked --quiet` 最终513项全部通过、0失败/忽略，包含新增5项Lab数值/往返回归：D50、D65与等能白点的黑/白/中灰及低亮度；D65的RGB主色（8位通道允许1级误差）；L/a/b及自定义Range裁剪；组件数量/非有限输入、非零BlackPoint和不可适配白点错误；间接WhitePoint/数值/参数字典保存严格重开后中灰为119/119/119。

Lab反变换、XYZ/sRGB矩阵和传递函数参照 [W3C CSS Color 4转换资料](https://www.w3.org/TR/css-color-4/#color-conversion-code)；Bradford白点变换核对 [LittleCMS原作者实现](https://raw.githubusercontent.com/mm2/Little-CMS/master/src/cmswtpnt.c)。Lab的源白点来自PDF字典，不固定为CSS Lab的D50。这里的回归是数值预期和库内往返，不是独立ICC色彩引擎/阅读器或视觉验证。

当前仅零BlackPoint，固定sRGB通道裁剪；非零BlackPoint补偿仍明确不支持。OutputIntent/ICC意图、感知色域映射和CalRGB/CalGray策略统一尚未完成。本轮未运行集成目标、全工作区或fuzz，未提交。

## Lab 参数路径收口（2026-10-07）

`cargo test -p pdf --lib --locked --quiet` 最终515项全部通过、0失败/忽略。新增2项回归验证WhitePoint/BlackPoint/Range的畸形长度数组包含循环引用时，固定数组读取器先返回ColorError、不进入成员引用解析；直接与上下文构造的默认值、合法参数及非法白点/黑点/Range验证一致。前轮间接参数保存严格重开与颜色数值测试同时继续通过。

此次去除参数字典回写和逐成员PdfArray重建，返回参数采用固定数组，普通resolve仍可能分配拥有副本；不表示整个颜色构造的聚合内存已计费。未运行集成目标、全工作区、独立视觉验证或fuzz，未提交。

## 颜色公共工作流集成回归（2026-10-07）

```sh
cargo test -p pdf --test color_workflow --locked --quiet
```

新增3项全部通过、0失败/忽略。使用PageBuilder注册页内Lab与Separation/Stitching颜色资源，真实内容操作符设置填充/描边、切换专色及q/Q恢复；DisplayList向自定义颜色设备回放得到灰119/119/119、蓝0/0/255和白255/255/255。Preserve/FlateUnencoded两种保存严格重开后结果一致，保存不改变修订号；复制页后只编辑副本保持原页灰色/副本白色。不支持的非零BlackPoint通过设备错误传播停止回放，后续红色路径不被绘制。

初次编译使用错误的SaveOptions引用参数，按现有按值API修正后重新执行成功。本轮未改生产代码，未重跑单元/全工作区，未做独立阅读器或Skia像素验证；自定义设备调用本库RGB转换，不能据此排除共享转换缺陷。未提交，fuzz仍暂缓。

## 共享颜色函数修订回归（2026-10-07）

`cargo test -p pdf --test color_workflow --locked --quiet` 最终4项全部通过、0失败/忽略。新增用例从严格打开的源文档加载两页共享间接Stitching函数；蓝色函数写入新红色函数后旧文档stamp失效，两页新解释均为红色，已有DisplayList设备回放仍为蓝色旧快照。再次保存严格重开后两页红色保持。

这里特意验证低层修改共享对象影响所有引用页，而不是复制后局部编辑；如仅修改单页，仍须先复制资源及重新绑定。已有DisplayList不会自动重新解释，应检查修订号并重新读取。测试使用本库自定义颜色设备，不是独立阅读器/像素验证。本轮生产代码未改，未重跑全工作区；前轮775项完整通过不包含本次新增用例。未提交，fuzz暂缓。

## Lab 数组标识校验（2026-10-07）

`cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出码0：pdf单元516项、颜色工作流集成4项全部通过，0失败/忽略。新增回归验证两种Lab构造入口拒绝非Lab名称、数字/null/字符串/布尔首项；正确直接名称保持兼容，上下文入口支持间接名称，直接入口不解析引用。间接名称修改为DeviceRGB或成环后拒绝，恢复为Lab可重新构造。参数字典入口仍保留。

本轮未重跑全工作区、render或独立视觉验证；先前775项完整通过记录不应视为包含此新改动。git diff --check通过，未提交，fuzz仍暂缓。

## 颜色空间资源注册与导入（2026-10-07）

`cargo test -p pdf --lib --test color_workflow --test resources --locked --quiet` 退出码0：pdf单元516项、color_workflow7项、resources14项全部通过，0失败/忽略。新增3项集成回归覆盖ColorSpace类别注册的CS1冲突避让与CS2复用、重复注册不改修订号、共享间接类别表不被修改；Lab数组及间接白点跨文档导入、外文档句柄拒绝、修改源参数不影响目标、同文档资源复用和严格保存重开；非法资源注册及外层事务失败不改变页面保存字节，失败后CS1仍可重试。

注册使用现有颜色空间解析器，不能据此证明所有类型可渲染。枚举新增ColorSpace分支，下游穷举match需要迁移。本轮未运行全工作区/render或独立视觉验证，不把先前775项完整通过当作本次变更验收；未提交，fuzz暂缓。

## 颜色空间导入失败边界（2026-10-07）

`cargo test -p pdf --test color_workflow --locked --quiet` 退出码0，8项全部通过、0失败/忽略。新增用例先以非法Lab白点验证ColorError拒绝不改目标，恢复白点后以max_objects=4触发间接参数复制中途ResourceLimit；两次失败均保持目标保存字节和源文档不变。随后DeviceRGB名称资源导入成功，对象号为3，证明此前临时对象和编号未泄漏；严格重开后对象值保持。

本轮仅新增确定性集成回归，不改生产代码；未重跑单元/全工作区或独立视觉验证，不将节点数量限制宣称为OperationBudget。未提交，fuzz仍暂缓。

## 颜色空间直接构建入口（2026-10-07）

`cargo test -p pdf --lib --test color_workflow --test resources --locked --quiet` 退出码0：pdf单元516项、color_workflow9项、resources14项全部通过，0失败/忽略。新增回归验证register_color_space原子创建Lab、重复相等值和间接根复用已有名称且不改修订、非法参数/页面保持字节、外层失败后资源字节/CS2名称恢复并可重试、保存严格重开中灰保持。

值复用采用现有PdfObject相等规则，不比较不同间接图的语义等价；裸引用须属于当前文档，不能用此入口替代跨文档导入。注册解析不表示全部颜色类型可渲染，普通对象副本与事务仍未完整计入OperationBudget。本轮未运行全工作区/render或独立视觉验证，未提交，fuzz暂缓。

## 直接颜色注册对象额度边界（2026-10-07）

`cargo test -p pdf --test color_workflow --locked --quiet` 退出码0，10项全部通过、0失败/忽略。新增用例在max_objects=16下创建页及DeviceRGB资源，再填满对象额度；重复注册已有值复用CS1且stamp保持，新DeviceGray两次创建均ResourceLimit拒绝，页资源字典与CS1原值保持不变，拒绝后已有值仍可复用。

此项验证数量上限下的复用/拒绝和资源状态，不证明OperationBudget、保存峰值或解除删除对象后额度回收。本轮未改生产代码、未重跑单元/全工作区/独立视觉验证，未提交，fuzz暂缓。

## 构建示例独立验证（2026-10-06）

使用现有 `create_document` 示例生成两页 PDF，再由独立工具检查结构、提取文字和渲染。样本为本项目生成，无新增第三方 PDF。

```sh
cargo run -p pdf --example create_document --locked --quiet -- output/pdf/build-validation.XQhHb5/created-document.pdf
qpdf --check output/pdf/build-validation.XQhHb5/created-document.pdf
qpdf --show-npages output/pdf/build-validation.XQhHb5/created-document.pdf
mutool draw -F txt -o - output/pdf/build-validation.XQhHb5/created-document.pdf
mutool draw -r 72 -o output/pdf/build-validation.XQhHb5/mupdf-page-%d.png output/pdf/build-validation.XQhHb5/created-document.pdf
```

- qpdf 11.2.0：结构和流编码检查通过，页数为 2；不等于完整规范符合性证明。
- MuPDF/mutool 1.27.0：两页文字均成功提取，标题 `rzpdf - Creation example` 及各页 `Page 1/2 | Text, image and metadata` 正确。
- 按 PDF 技能逐页人工查看 72 DPI 渲染图：标题、页码及红蓝图片可见，无明显裁切、重叠或异常黑块。
- PDF SHA-256：`d0a718f94de2d2a53917792b108b488cb138c78621363ea79d12b189c159e008`。

本地验证产物：[PDF](../output/pdf/build-validation.XQhHb5/created-document.pdf)、[第一页](../output/pdf/build-validation.XQhHb5/mupdf-page-1.png)、[第二页](../output/pdf/build-validation.XQhHb5/mupdf-page-2.png)。复现时请使用新目录，不覆盖既有产物。

环境限制：本机 Poppler 因缺少 `libtiff.5.dylib` 无法启动，Swift/PDFKit 回退因 JIT 缺少 `PDFDocument` 类符号失败，均不计为通过，未修改系统依赖。成功验证来自 qpdf 和 MuPDF。

范围仅为当前标准字体英文、RGB 图片及带 Info/XMP 的两页构建样本；未独立校验全部元数据内容，不覆盖真实复杂字体、编辑后文件、导入重排、加密签名或完整样本矩阵。整体任务保持未完成，本次未运行 fuzz。

## 编辑示例独立验证（2026-10-07）

使用本项目现有 `edit_content` 示例，生成原页/复制后编辑页的两页对照 PDF。样本完全由项目生成，没有引入第三方文件；没有覆盖既有验证产物。

```sh
cargo run -p pdf --example edit_content --locked --quiet -- output/pdf/edit-validation.Udmdci/edited-comparison.pdf
qpdf --check output/pdf/edit-validation.Udmdci/edited-comparison.pdf
qpdf --show-npages output/pdf/edit-validation.Udmdci/edited-comparison.pdf
mutool draw -F txt -o - output/pdf/edit-validation.Udmdci/edited-comparison.pdf
mutool draw -r 72 -o output/pdf/edit-validation.Udmdci/page-%d.png output/pdf/edit-validation.Udmdci/edited-comparison.pdf
```

- qpdf 11.2.0：无语法或流编码错误，页数为2，不代表完整规范符合性。
- MuPDF/mutool 1.27.0：第一页提取 `Original content`，第二页提取 `Edited content`。
- 按 PDF 技能逐页检查72 DPI渲染图：原页文字、两次红绿图片绘制和红/蓝矩形保持；编辑页文字替换正确，第一张图片旋转为竖向红上绿下，第二次图片绘制消失，红色路径绘制消失而蓝矩形保留。未见明显裁切、重叠、异常黑块或不可读字符。
- SHA-256：`b2712b6c78c5c1e2761a0c4aa4925e8b928873e8abc245b20304be9d9b6ee953`。

产物：[PDF](../output/pdf/edit-validation.Udmdci/edited-comparison.pdf)、[原页](../output/pdf/edit-validation.Udmdci/page-1.png)、[编辑页](../output/pdf/edit-validation.Udmdci/page-2.png)。复现时使用新目录；示例拒绝覆盖已有PDF。

Poppler仍因缺少 `libtiff.5.dylib` 无法启动，不计为通过；使用已安装MuPDF作为PDF技能渲染回退，未修改系统依赖。此验证覆盖固定Helvetica英文、顶层图片/路径与复制页隔离，不覆盖真实复杂字体、Form/内联图片、裁剪透明或完整P3验收。fuzz未运行。
