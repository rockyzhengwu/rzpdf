# 中英文整字体构建验证

## 两层Form真实中文推进补偿（2026-10-08）

`compare_cjk_advance`新增可选`--nested`：先把含中文/图片和后续PDF事件的第一页包装为两层Form，再复制补偿/无补偿页，调用嵌套Unicode补偿/普通替换入口。固定输出仍为基线、原第二页、补偿、无补偿；使用原嵌入静态Noto Sans SC字体，不用合成字形替代真实字体。本例由rzpdf生成，独立工具只用于核验。

```sh
cargo run -p pdf --example compare_cjk_advance --locked -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-nested-advance.pdf --nested
bash docs/check-cjk-advance.sh output/pdf/rzpdf-cjk-nested-advance.pdf --nested
mutool draw -r 96 -o tmp/pdfs/rzpdf-cjk-nested-advance-%d.png output/pdf/rzpdf-cjk-nested-advance.pdf
```

最终运行原session82446退出0，日志`/tmp/rzpdf-real-cjk-nested-advance-final.log`；首轮示例编译将Option字典转换当Result，修正后重新生成。保存/释放/严格重开后的补偿P/D/F位置及无补偿水平差均通过；生成文件SHA-256为`d29a38641d0b10e054144501d98bda5dbffc5a8264ce660982b1e9400165b7f9`，不承诺再次生成字节一致。

qpdf 11.2.0结构与MuPDF 1.27.0独立stext坐标/归一化文字通过，日志`/tmp/rzpdf-cjk-nested-independent-final.log`。脚本检查共享Type0/ToUnicode/FontFile2、RGB/软遮罩，以及三份不同外Form/三份不同叶Form的字型和图片共用关系。复制编辑保留未使用的旧资源别名，因此按唯一对象引用计数；首次脚本重复计数拒绝正常文件，已修正后复验，并在原顶层样本上通过回归。无嵌套标志拒绝嵌套文件、嵌套标志拒绝顶层文件；交换第三/四页的同结构四页文件通过结构阶段、随后坐标检查退出1，日志`/tmp/rzpdf-cjk-nested-swapped-check.log`。

四页96 DPI PNG均已目视：真实中文字形、标题和图片/透明度清晰，无缺字/裁切，补偿页空隙为保留后续位置的预期结果，无补偿页PDF左移。另将顶层对照同样渲染到`tmp/pdfs/rzpdf-cjk-advance-check-%d.png`，四页分别与嵌套PNG进行cmp，全部字节一致，确认包装本身不改变这组页面的渲染。原先72 DPI PNG与新96 DPI图不能直接比较尺寸；没有将首次不同分辨率的cmp失败解释为引擎问题。

按PDF技能完成渲染检查；本机Poppler仍因缺libtiff.5.dylib无法启动，使用已有MuPDF，未修改系统依赖。仅证明这款真实水平字体的两层整Tj缩短补偿，不代替真实竖排/TJ范围/所有资源作用域/全部字体或完整P3；合成混合字符码横竖排另有四项嵌套专项通过，但不证明实际竖排字形视觉。fuzz未运行。

扩展示例的原两参数默认CLI也已复跑，原session25317退出0，生成`output/pdf/rzpdf-cjk-advance-regression.pdf`并经新检查脚本通过，日志`/tmp/rzpdf-cjk-advance-default-regression.log`和`/tmp/rzpdf-cjk-advance-default-regression-check.log`；不只使用旧PDF来证明默认生成路径兼容。

## 真实字体自动推进补偿与不补偿对照（2026-10-08）

新增`compare_cjk_advance`，输入为已有两页`create_cjk --images`样本。它在第一页正文后插入独立的“ PDF”文字事件，再复制出两页：第三页将整句缩短为“你好！”并自动补偿，第四页作相同替换但不补偿。输出顺序为基线、原第二页、补偿、无补偿；不修改输入文件，输出路径应与输入不同。

```sh
cargo run -p pdf --example compare_cjk_advance --locked --offline -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-advance.pdf
bash docs/check-cjk-advance.sh output/pdf/rzpdf-cjk-advance.pdf
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-advance-%d.png output/pdf/rzpdf-cjk-advance.pdf
```

最终源码运行原session30593退出0，日志`/tmp/rzpdf-real-cjk-advance-final.log`；首次示例编译的索引Result错误已修正，未改变生产引擎。示例检查共享基线源码不变、补偿页后续P/D/F与基线位置差小于0.002、无补偿页水平差大于10，并在保存、释放原文档、严格重开后重复检查及解释四页。

qpdf 11.2.0结构检查通过；MuPDF 1.27.0的stext XML由xmllint逐一读取四页末尾P/D/F原点，独立确认第三页位置保持、第四页明显左移且纵坐标保持。脚本另检查四页共享Type0/ToUnicode及RGB图片关系、第二页软遮罩，核对移除空白后的中文/拉丁文本与页序；归一化空白不是精确行布局验收。三页旧样本被拒绝；用本库extract_pages将补偿/无补偿页交换（0/1/3/2）后，四页结构仍通过但坐标检查退出1，避免仅检查页数误认成功。

四页72 DPI PNG已目视检查：中文字形清晰、图片/透明度保持，第三页的空隙是保留后续位置的预期结果，第四页PDF自然左移；无重叠、裁切或缺字。第二页PNG与原第二页`cmp`字节一致。输出SHA-256为`ad379753907057b7b4de9c7f52e628317164b88a4d3d1f320bda069726a14eca`，不承诺重复生成PDF字节一致。沿用MuPDF渲染，未修改现有Poppler缺失依赖。

只验证这款静态真实字体的水平整Tj缩短与后续事件位置，不代表竖排真实字形、TJ数组、嵌套Form、自动排版或完整P3。较长替换仍可能侵占后续字形区域；补偿只保留结束位置，不保证适配旧边界。未运行fuzz、未再次运行整个工作区。

## 真实CID字体范围替换与事务回滚（2026-10-08）

新增固定样本`edit_cjk_text INPUT.pdf OUTPUT.pdf`：读取两页`create_cjk --images`文件，复制第一页到第三页，以现有字体反向编码计算字符码字节范围，将第二个Tj中的“世界”替换为“你好”。范围不是Unicode字符索引，也不是转义源码偏移；此示例只面向原创建样本的直接Tf/Tj，不提供通用文字搜索或任意图形状态字体推断。

```sh
cargo run -p pdf --example edit_cjk_text --locked --offline -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-text-edit.pdf
bash docs/check-cjk-page-edit.sh output/pdf/rzpdf-cjk-text-edit.pdf --text-edit
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-text-edit-%d.png output/pdf/rzpdf-cjk-text-edit.pdf
```

最终源码对应运行原session45065退出0，日志`/tmp/rzpdf-cjk-text-edit-final.log`。示例同时检查：未映射火箭字符拒绝且stamp/源码不变；外层事务在成功替换后注入错误，核对实际返回为该注入错误而非提前失败，并确认源码恢复；重新获取快照后重试成功；共享原页源码不变。保存、释放原文档后严格读回三页并解释通过。

qpdf 11.2.0结构检查与MuPDF 1.27.0六行精确文本通过：第三页为“你好，你好！Hello PDF”，前两页不变；三页仍共享原嵌入字体，第一/三页共享RGB图片，第二页保留RGBA遮罩。旧页面重排产物在`--text-edit`模式拒绝，文字编辑产物在默认重排模式拒绝。前两页渲染PNG与原图`cmp`字节一致，第三页已目视检查中文替换清晰、正文与图片无重叠或裁切。输出SHA-256为`e22274a43e3452273fd6ff75fb41d8e5316d0d4e0ce0016abeb63b60d7369c0d`，不承诺重复生成PDF字节一致。

此结果仅覆盖已映射字符的单Tj范围替换、固定真实字体及共享隔离/错误回滚；不代表新增字形/ToUnicode更新、字体子集、shaping、连续文字推进补偿、嵌套Form或全部复杂字体矩阵完成。生产引擎源码未变，未再次运行整个工作区，未运行fuzz。

## 合并与逆序拆分组合（2026-10-08）

现有`extract_pages`和`merge_documents`示例在复制完成后新增显式释放源文档，然后保存和严格读回，避免以源读取器仍存活掩盖资源所有权缺陷。使用上述创建及页面编辑样本执行：

```sh
cargo run -p pdf --example extract_pages --locked --offline -- output/pdf/rzpdf-cjk-extracted.pdf output/pdf/rzpdf-cjk-images.pdf 1 0
cargo run -p pdf --example merge_documents --locked --offline -- output/pdf/rzpdf-cjk-merged.pdf output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-page-edit.pdf
bash docs/check-cjk-transfer.sh extract output/pdf/rzpdf-cjk-extracted.pdf
bash docs/check-cjk-transfer.sh merge output/pdf/rzpdf-cjk-merged.pdf
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-extracted-%d.png output/pdf/rzpdf-cjk-extracted.pdf
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-merged-%d.png output/pdf/rzpdf-cjk-merged.pdf
```

本库两条命令均退出0，保存后严格读回并解释全部页；qpdf 11.2.0检查通过。MuPDF 1.27.0提取及固定检查脚本确认拆分顺序为B/A（两页），合并顺序为A/B/A/B/B（五页）。脚本按页面树Kids顺序检查字体与图片，拆分保留一份共享嵌入字体，跨两个源文件合并保留两份字体对象，不声称跨源字体去重；每页RGB尺寸/位深及RGBA软遮罩顺序正确。错误顺序的原两页输入和错误页数的三页输入分别退出1。

七份72 DPI渲染PNG逐一`cmp`与已目视验收的对应原页字节一致；另目视检查拆分第一页与合并末页，无缺字、裁切、图片翻转或透明度变化。输出SHA-256：拆分`acc71de6f348dd18d15514f3d1a6f83cd7e9152e0fc3293c46bc843219a3b1f0`，合并`9272547f30d4214265199d366fc77f273a8d65ca28b65c2a03d53ff57d0433d5`，仅标识本次产物。

只验证固定静态字体/RGB/RGBA样本与源释放后使用，不覆盖批注、书签、Catalog元数据合并、任意页面树或全部失败回滚矩阵。`check-cjk-transfer.sh`仅接受上述固定样本布局，不是通用PDF验证器。生产引擎源码未变，未重复跑整个工作区，未运行fuzz。

## 跨文档导入、复制与重排组合（2026-10-08）

新增`edit_page_order INPUT.pdf OUTPUT.pdf`示例：将源页逆序导入新文档，复制导入后的首个页面，再重排为原始页序＋最后一页副本。保留源有效PDF版本；释放源文档后解释所有目标页，保存并释放目标，再严格重开并解释所有页。只复制页面对象图，不合并源Catalog元数据、书签或批注。

以本节后述两页中英文/RGB/RGBA样本运行：

```sh
cargo run -p pdf --example edit_page_order --locked --offline -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-page-edit.pdf
bash docs/check-cjk-page-edit.sh output/pdf/rzpdf-cjk-page-edit.pdf
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-page-edit-%d.png output/pdf/rzpdf-cjk-page-edit.pdf
```

本库示例退出0。qpdf 11.2.0检查通过；固定样本脚本按页面树Kids顺序验证三页、共享Type0/CIDFontType2嵌入字体与ToUnicode、第二/三页共享图片及DeviceGray SMask，并核对六行正文及页序。原始两页输入在此脚本下退出1。MuPDF 1.27.0的三页PNG已目视检查，文字、四象限方向及透明度不变；`cmp`逐一确认输出第1/2页与原始渲染PNG字节一致，第3页与第2页一致。本次输出SHA-256为`3e8ea0d61f13871638fade949d046fb5d3c48042ac13b88216452d77a0ad8bb2`，不承诺多次生成PDF字节一致。

这验证固定真实字体/软遮罩对象图导入、源释放后使用及重排/复制读回，不代表P2所有继承属性、循环、批注归属、旋转裁剪、合并拆分或失败回滚矩阵完成。脚本仅适用于此直接资源/平坦页面树样本，不是通用PDF一致性验证器；未运行fuzz。

## 中英文、RGB/RGBA插图与覆盖组合（2026-10-08）

`create_cjk` 新增可选 `--images`，与 `--overlay` 可独立使用或组合（顺序任意，重复/未知选项拒绝且不写输出）。不传选项保留原来的四行文字示例。第一页绘制2×2 RGB四象限图，第二页绘制同色RGBA图，alpha依次为255、192、128、0；位置为用户空间(48,500)，尺寸240×160。正文与跨页共享的CID字体保持不变。

使用下文同一静态字体（SHA-256 `0516f3d1fb52efaf83f1d7bc540d779d1eb405f37e0dbb7696537444a0931087`），本次验证命令：

```sh
cargo run -p pdf --example create_cjk --locked --offline -- FONT.ttf NotoSansSC-Thin 733 80 output/pdf/rzpdf-cjk-images.pdf --images
bash docs/check-cjk-example.sh output/pdf/rzpdf-cjk-images.pdf --images
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-images-%d.png output/pdf/rzpdf-cjk-images.pdf
```

结果：本库示例退出0，保存后严格读回并解释两页；qpdf 11.2.0无语法/流编码错误。脚本验证原字体/标题/四行精确文本，并额外检查两页RGB Image的尺寸/位深及一个DeviceGray SMask。无图片原样本在`--images`检查下退出1，避免误认组合样本；当前默认示例仍通过原检查。MuPDF 1.27.0渲染的两页PNG已目视检查：文字清晰无缺字/裁切，红绿在上、蓝黄在下；第二页蓝色变浅、黄色完全透明，其余透明度符合预期。输出PDF SHA-256为`8dc24e3623132152c285cfcdf41fb6c6bc17b6a0cf482425893281d047ccad2e`，仅标识本次产物，不承诺跨次字节一致。

另运行`--images --overlay`组合：本库严格读回、qpdf检查通过，MuPDF提取四行正文、两处“草稿”和两页对应页码；两页渲染图已目视检查，正文/图片/水印/页脚无重叠或裁切。原精确四行脚本不适用带覆盖的文件，不能以它证明覆盖路径。当前系统Poppler缺少libtiff.5.dylib不能启动，本次使用MuPDF，未修改系统依赖。

仅完成此字体、水平文字与固定RGB/RGBA示例的组合验证；不代表全部字体、JPEG/PNG输入、旋转裁剪、多阅读器矩阵或完整P1验收。生产引擎源码未改变；此前938项全量对应a270aca，此轮新增StoreEntries公开接口专项1项通过，没有再次运行整个工作区。未运行fuzz。

2026-10-04：新增 `pdf/examples/create_cjk.rs`，创建两页 A4 中英文，使用本项目字体注册、CID 编码、保存接口；第二页复用同一字体。保存后严格打开并构建两页 DisplayList。按 PDF 技能要求渲染检查，不使用外部库替代本项目构建。

## 字体来源和范围

验证输入是 [Google Fonts Noto Sans SC](https://github.com/google/fonts/tree/main/ofl/notosanssc) 的 `NotoSansSC[wght].ttf`，采用 [SIL OFL 1.1](https://raw.githubusercontent.com/google/fonts/main/ofl/notosanssc/OFL.txt)，保留原许可。仅在临时验证目录下载，不把字体源加入仓库。FontTools 4.39.4 固定全部轴为 wght=400，输出静态 TrueType；这是外部样本准备，不是 rzpdf 已支持可变字体实例化。原实例仍保留 name ID 6 的 `NotoSansSC-Thin` 名称，因此传入该实际名称，不把文件名当作字体名称。不要据此安装或发布为官方 Regular 字体。

输入 variable SHA-256：`a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da`。
静态实例 SHA-256：`0516f3d1fb52efaf83f1d7bc540d779d1eb405f37e0dbb7696537444a0931087`。
实例有 31036 字形，fsType=0，OS/2 CapHeight 归一化后为 733；StemV=80 是本次显示冒烟采用的显式近似值，不声称已从轮廓测量或足以证明描述字典完全准确。真实指标自动推导仍待实施。OFL 保留名称为 Source；静态实例未使用该保留名称。

## 复现

先自行取得许可允许使用的字体，创建输出目录；保存会替换指定现有普通文件。

```sh
fonttools varLib.instancer 'NotoSansSC[wght].ttf' wght=400 --no-recalc-timestamp -o NotoSansSC-static.ttf
cargo run -p pdf --example create_cjk --locked -- NotoSansSC-static.ttf NotoSansSC-Thin 733 80 output/pdf/rzpdf-cjk-example.pdf
qpdf --check output/pdf/rzpdf-cjk-example.pdf
bash docs/check-cjk-example.sh output/pdf/rzpdf-cjk-example.pdf
mutool draw -F txt output/pdf/rzpdf-cjk-example.pdf
mutool draw -r 72 -o page-%d.png output/pdf/rzpdf-cjk-example.pdf
```

远端字体可能更新；以哈希和实际字体名称/指标为准，不盲用上述参数。交付输出本次 SHA-256 为 `8524d35a242f2248915b3b757fcf9659d21b68f5caec71f0b8e43a38beb53487`；文件哈希用于标识本次产物，不保证重复生成的字节顺序完全一致。

## 结果与修复

`check-cjk-example.sh` 是只读的精确示例验收脚本，依赖 qpdf/mutool/jq：检查两页、两页字体指向同一对象、Type0/Identity-H、CIDFontType2、FontDescriptor/FontFile2、字宽、映射流和 Unicode 标题，逐字核对四行及顺序（仅移除空行/分页符）。真实产物通过；既有英文创建样本被拒绝；缺参数返回 2。脚本只适用于本示例的直接页面资源结构，不是通用字体分析器，不验证完整字体轮廓或视觉质量；PNG 检查仍单独进行。

真实字体包含 cmap 4/12/14/6。运行发现旧读取对 14 误用 16 位长度并因额外未支持表拒绝字体，现按 OpenType 声明长度校验边界/最小头及累计预算，再跳过未实现表，保留 0/4/12；无受支持表仍显式拒绝。新回归验证辅助表共存和截断，不声称支持变体序列或未实现表内部语义。

qpdf 11.2.0：两页 PDF 1.7 无语法或流编码错误；不表示全部 PDF 语义正确。
MuPDF 1.27.0：文本提取包含以下四行（允许提取器增加空行/分页）：

```text
rzpdf - 中文字体嵌入
你好，世界！Hello PDF
第二页 - 同一字体跨页复用
中文可显示，也应正确提取。
```

两页 72 DPI PNG 均已目视检查：中文和拉丁文字可读，无缺字方框、重叠或裁切。Poppler 的既有 libtiff 依赖问题未修改系统，使用 MuPDF 替代。样本限于这款字体和水平简单文字，不覆盖所有中文/补充平面字形、shaping、变体、子集、CFF、竖排、PDF/A 或本库 P2 文本提取接口；P1-05/P1-10 完整验收仍不勾选。未运行 fuzz。
