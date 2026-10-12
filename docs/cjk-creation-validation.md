# 中英文整字体构建验证

## 跨Form调用真实字体选区（2026-10-08）

连续路径索引版复验（当前冻结源码）：原51039两模式生成退出0，`output/pdf/rzpdf-cjk-mixed-font-indexed.pdf`及`output/pdf/rzpdf-cjk-same-chain-indexed.pdf`严格重开、qpdf、MuPDF精确文字/后缀坐标和-O负例通过；mixed核对三段Noto/Helvetica绑定与字号，七负例拒绝，same-chain四负例拒绝。八页72DPI PNG逐模式与旧基线cmp字节一致；mixed四页及same-chain新第一页目视正常，其余页与已查看基线一致。只验证固定水平嵌入CJK+标准14字体及同链夹子Form，当前完整回归99029仍在运行。

混合字体检查加固：检查脚本逐一核对固定基线三段的文字、字体和字号（Noto Sans SC/24、Helvetica/18、Noto Sans SC/24），不只检查两种字体名称出现。新增子字体误绑定、子字号错误和父字体误绑定三负例；当前mixed-font XML普通/-O均拒绝全部七负例，默认/partial/same-chain仍各四负例通过。此为固定样本独立检查，不是通用字体符合性验证；PDF生产引擎未修改。

混合字体验收：`compare_cjk_span --mixed-font`父Form采用嵌入Noto Sans SC/24Tf，夹子Form使用Helvetica/18Tf/2Tc。session27044生成退出0，产物`output/pdf/rzpdf-cjk-mixed-font-span.pdf`；qpdf结构、MuPDF精确两句中文夹Hello PDF基线与编辑你好！ PDF、独立后缀坐标、四负例及python3 -O通过。XML明确区分NotoSansSC和Helvetica，检查脚本要求两者存在。第一页面目视字体/字距正常，第二至四页PNG与已验收same-chain样本cmp字节一致；按PDF技能使用MuPDF验证。仅真实嵌入水平CJK+标准14字体，不是双嵌入字体、真实竖排或完整字体矩阵。

```sh
cargo run -p pdf --example compare_cjk_span --locked -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-mixed-font-span.pdf --mixed-font
mutool draw -F stext -o tmp/pdfs/rzpdf-cjk-mixed-font-span.xml output/pdf/rzpdf-cjk-mixed-font-span.pdf
python3 -O docs/check-cjk-span.py tmp/pdfs/rzpdf-cjk-mixed-font-span.xml --mixed-font --self-test
```

同链夹子Form真实字体样本：`compare_cjk_span --same-chain`构建父Form两个中文事件，中间夹子Form事件；基线三行，编辑后精确文字`你好！ PDF`。端点同链/子事件不同链由示例校验，普通/补偿两种编辑页严格重开。生成session23177退出0，qpdf结构、MuPDF精确文字/末尾坐标及普通/-O四负例通过；默认与same-chain检查模式互用均因精确基线文字不符退出1。四页72 DPI PNG逐页目视无缺字/重叠/裁切，原第二页图片PNG与unified基线cmp一致；补偿页尾PDF保持原坐标，普通页左移。仅固定水平Noto Sans SC，不代表真实竖排或全字体矩阵。

```sh
cargo run -p pdf --example compare_cjk_span --locked -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-same-chain-span.pdf --same-chain
qpdf --check output/pdf/rzpdf-cjk-same-chain-span.pdf
mutool draw -F stext -o tmp/pdfs/rzpdf-cjk-same-chain-span.xml output/pdf/rzpdf-cjk-same-chain-span.pdf
python3 -O docs/check-cjk-span.py tmp/pdfs/rzpdf-cjk-same-chain-span.xml --same-chain --self-test
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-same-chain-span-%d.png output/pdf/rzpdf-cjk-same-chain-span.pdf
```

公共span统一入口后最新复验：session95710/74173生成两种unified产物均退出0；独立检查session95377退出0，两份qpdf结构、MuPDF精确文字/后缀坐标及四负例通过，部分模式使用python3 -O。72 DPI八页PNG与reuse版逐页cmp字节一致，部分模式两编辑页目视无缺字/重叠/裁切。此固定不同调用链样本只证明最新入口无回归，不替代同链夹子Form真实字体视觉矩阵。按PDF技能完成MuPDF独立渲染，未改系统依赖。

叶节点规划程序复用后复验：默认/部分模式生成会话58000/86700均退出0，产物为`output/pdf/rzpdf-cjk-cross-span-reuse.pdf`及`output/pdf/rzpdf-cjk-cross-span-partial-reuse.pdf`。两份qpdf结构、MuPDF精确文字/独立后缀坐标和四个变异负例通过（部分模式使用python3 -O）；72 DPI八页PNG与owner版本逐页cmp字节一致，部分模式编辑两页目视复核无缺字/重叠/裁切。按PDF技能渲染复验，沿用MuPDF，不改系统依赖；仅覆盖本节固定水平真实字体样本。

单片段表复用后的最新复验：生成`output/pdf/rzpdf-cjk-cross-span-owner.pdf`及`output/pdf/rzpdf-cjk-cross-span-partial-owner.pdf`均退出0；两份qpdf、独立精确文字/后缀坐标及四负例通过。72 DPI八页PNG与下述生命周期产物逐页cmp字节一致，部分保留编辑两页目视复核正常。只证明固定样本的最新生产版本输出保持，不代表全字体、性能或最新完整workspace验收。

最新生命周期优化后复验：重新运行示例两种模式，分别生成`output/pdf/rzpdf-cjk-cross-span-lifetime.pdf`与`output/pdf/rzpdf-cjk-cross-span-partial-lifetime.pdf`，运行均退出0。qpdf、独立精确文字/坐标及脚本普通/-O四负例全部通过；MuPDF 72 DPI八页PNG分别与优化前下述产物逐字节一致，目视复核未见缺字/重叠/裁切或图片变化。此证据覆盖最新生产引擎对本节固定样本的输出，不代表完整工作区或通用字体性能验收。

固定示例`compare_cjk_span`复用已有两页`create_cjk --images`字体资源，构建同一叶Form的两层、两次调用。四页分别为基线、原第二页、补偿编辑和普通编辑。选区从首次调用句首到第二次调用句尾，保留第二次调用后续` PDF`事件；结果为`你好！ PDF`，补偿页末尾位置不变，普通页左移。

```sh
cargo run -p pdf --example compare_cjk_span --locked -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-cross-span.pdf
qpdf --check output/pdf/rzpdf-cjk-cross-span.pdf
mutool draw -F stext -o tmp/pdfs/rzpdf-cjk-cross-span.xml output/pdf/rzpdf-cjk-cross-span.pdf
python3 docs/check-cjk-span.py tmp/pdfs/rzpdf-cjk-cross-span.xml --self-test
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-cross-span-%d.png output/pdf/rzpdf-cjk-cross-span.pdf
```

本库生成运行退出0，严格重开四页解释通过；qpdf结构检查通过。MuPDF四页精确字符文本、末尾P/D/F独立x/y坐标保持（容差0.002点），普通控制左移至少10点且y保持通过。脚本拒绝交换编辑页、错误坐标、错误文本和缺页四负例；普通及`python3 -O`模式均验证通过，未知选项退出2。四页72 DPI图像已目视，无缺字、裁切或重叠。系统Poppler因缺libtiff.5.dylib无法启动，使用MuPDF独立渲染，未修改系统依赖。

仅验收固定静态Noto Sans SC水平共享Form跨调用缩短，不包含真实竖排、不同字体混合、新增字形、TJ选区、通用搜索或排版适配。检查脚本只接受该固定样本，不是通用PDF验证器；源字体不新增入库，fuzz继续暂缓。

部分结束事件模式使用`--partial`：选区结束在第二次调用中文前缀之后，保留同一Tj的`Hello PDF`和下一事件的` PDF`，精确结果为`你好！Hello PDF PDF`。

```sh
cargo run -p pdf --example compare_cjk_span --locked -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-cross-span-partial.pdf --partial
qpdf --check output/pdf/rzpdf-cjk-cross-span-partial.pdf
mutool draw -F stext -o tmp/pdfs/rzpdf-cjk-cross-span-partial.xml output/pdf/rzpdf-cjk-cross-span-partial.pdf
python3 docs/check-cjk-span.py tmp/pdfs/rzpdf-cjk-cross-span-partial.xml --partial --self-test
python3 -O docs/check-cjk-span.py tmp/pdfs/rzpdf-cjk-cross-span-partial.xml --partial --self-test
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-cross-span-partial-%d.png output/pdf/rzpdf-cjk-cross-span-partial.pdf
```

部分模式生成退出0，qpdf及独立检查通过；验证范围扩至全部13个保留字符，包括空格，而不只末尾PDF。四个变异负例均拒绝，默认/部分模式互用均因精确文字不符退出1。编辑两页72 DPI目视无缺字、重叠或裁切；前两页PNG与已验收默认基线逐字节一致。按PDF技能完成独立渲染，沿用MuPDF以避开本机Poppler缺失依赖，不修改系统环境。仅增加示例/检查脚本，不改变引擎生产代码，不宣称完整字体矩阵完成。

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
## 真实中文跨事件范围对照（两层 Form）

可重复的正负对照门禁：`bash docs/check-cjk-event-negatives.sh output/pdf/rzpdf-cjk-event-advance.pdf output/pdf/rzpdf-cjk-nested-event-advance.pdf`。脚本先确认两个正常输入通过，独立qpdf交换每份输出的第3/4页，确认交换后的PDF语法合法，再要求比较脚本退出1且已到MuPDF四页坐标提取阶段，避免把依赖缺失/结构错误算作正确拒绝。原输入不修改，只清理脚本自身新建的临时文件。2026-10-08原session59505退出0，日志`/tmp/rzpdf-cjk-event-negatives.log`；两个负例均在坐标比较处拒绝。此为固定对照样本测试，不是通用PDF符合性或fuzz。

`compare_cjk_advance`新增可组合的`--events`和`--nested`。固定两页`create_cjk --images`输入沿用实际嵌入的静态Noto Sans SC字体，把原句拆为中文和英文两Tj事件；范围模式将两事件一起替换为“你好！”。四页为基线、原第二页、保持推进、普通替换，不引入新字形或重新排版。

```sh
cargo run -p pdf --example compare_cjk_advance --locked -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-nested-event-advance.pdf --nested --events
bash docs/check-cjk-advance.sh output/pdf/rzpdf-cjk-nested-event-advance.pdf --nested
mutool draw -r 96 -o tmp/pdfs/rzpdf-cjk-nested-event-%d.png output/pdf/rzpdf-cjk-nested-event-advance.pdf
```

2026-10-08 示例原session27878退出0，内置坐标/基线隔离/displaylist/严格重开自检通过，日志`/tmp/rzpdf-cjk-nested-event-advance.log`。独立检查退出0，日志`/tmp/rzpdf-cjk-nested-event-independent.log`：qpdf结构及字体嵌入/图像/Form复制关系、MuPDF尾部P/D/F坐标和归一化文本通过。四页96DPI PNG逐页查看，标题、中文、图片与透明页正常；补偿页保留原尾部位置，普通替换左移，补偿后的间隙为预期而非自动布局。系统Poppler因缺失libtiff.5.dylib无法启动，使用现有MuPDF独立渲染，未修改系统依赖。

产物`output/pdf/rzpdf-cjk-nested-event-advance.pdf`的SHA-256为`da41b67db3a02b2ea18605ac294591cecba60de76a7978e8f91c85db92ec7050`。此证据仅覆盖固定真实水平字体、相邻两Tj和同一两层Form调用内范围；不代表真实竖排、跨不同Form、所有TJ/字体切换矩阵或完整P3验收。

顶层`--events`原session39731及原默认CLI原session67694也已退出0，分别生成`output/pdf/rzpdf-cjk-event-advance.pdf`与`output/pdf/rzpdf-cjk-advance-cli-regression.pdf`。两文件都通过`bash docs/check-cjk-advance.sh FILE.pdf`独立结构/坐标/文本复验，日志`/tmp/rzpdf-cjk-event-independent.log`与`/tmp/rzpdf-cjk-event-default-independent.log`。顶层跨事件四页另以96DPI MuPDF渲染，对应PNG逐页与已查看的嵌套跨事件PNG用cmp完全一致；原session17875退出0。原默认单事件CLI仍有效，不能据此宣称所有新模式或错误参数组合完整验收。

## 固定 TJ 数组补充验收（2026-10-08）

连续路径索引版最新复验：生成14322及独立验证69734均退出0，产物`output/pdf/rzpdf-cjk-tj-inside-indexed.pdf`严格重开、qpdf、MuPDF精确文字/13字符坐标、QDF原间距及-O四XML/三QDF负例通过。四页72DPI PNG逐页目视正常，并与优化前内部边界样本逐页cmp字节一致。只是当前固定水平真实字体内部TJ边界验证；完整工作区99029仍运行，不据此宣称全部字体或性能任务完成。

内部边界变体 `--tj-inside` 已通过：第一个数组字符串包含“你好，世界！Hello”，结束位置为中文编码长度，保留同字符串Hello及第二字符串“ PDF”。产物 `output/pdf/rzpdf-cjk-tj-inside-span.pdf`，生成会话22711退出0并严格重开；qpdf、MuPDF精确文字/13字符后缀坐标、Python -O四负例及四页PNG逐页目视通过。QDF核对原始及两种重建均保留-125/75数值。此变体只扩展一个内部字体码边界，不代表任意编码或完整TJ矩阵。

`compare_cjk_span INPUT OUTPUT --tj` 生成两次嵌套调用的真实 Noto Sans SC 样本。数组为 `[<中文> -125 <Hello PDF> 75] TJ`，结束端点位于第一个字符串末尾，保留英文及下一文本事件。产物为 `output/pdf/rzpdf-cjk-tj-span.pdf`。

生成会话47106退出0，保存后严格重开通过。qpdf结构检查、MuPDF文字与全13字符后缀坐标检查（`docs/check-cjk-span.py XML --partial --self-test`）及Python `-O`检查均通过，四负例均拒绝。QDF内容流核对原数值-125/75在普通和补偿重建中保留；四页PNG逐页目视无缺字、重叠或裁切。此处用引擎生成以验收引擎，Poppler依赖缺失沿用MuPDF渲染替代。

`--tj`只覆盖第一字符串末尾，`--tj-inside`补充其内部字体码边界；两者均不覆盖竖排或全部TJ排列。上述样本验收时生产代码未改，1005项门禁仅覆盖当时生产范围；后续单叶辅助函数清理须另行回归。

可复现内部边界验证（末尾边界将文件名和生成参数改为`--tj`）：

```sh
cargo run -p pdf --example compare_cjk_span --locked -- output/pdf/rzpdf-cjk-images.pdf output/pdf/rzpdf-cjk-tj-inside-span.pdf --tj-inside
qpdf --check output/pdf/rzpdf-cjk-tj-inside-span.pdf
mutool draw -F stext -o tmp/pdfs/rzpdf-cjk-tj-inside-span.xml output/pdf/rzpdf-cjk-tj-inside-span.pdf
python3 -O docs/check-cjk-span.py tmp/pdfs/rzpdf-cjk-tj-inside-span.xml --partial --self-test
mutool draw -r 72 -o tmp/pdfs/rzpdf-cjk-tj-inside-span-%d.png output/pdf/rzpdf-cjk-tj-inside-span.pdf
qpdf --qdf --object-streams=disable output/pdf/rzpdf-cjk-tj-inside-span.pdf tmp/pdfs/rzpdf-cjk-tj-inside-span.qdf.pdf
rg -a -n '^.*-125 .*75\] TJ' tmp/pdfs/rzpdf-cjk-tj-inside-span.qdf.pdf
```

XML检查验证可见文字和坐标；最后一条命令另核对实际内容流的原数值仍存在。PNG仍须人工复核，不能以文字检查代替视觉验收。

内容流数值现可自动检查：完成上述QDF生成后，使用 `python3 -O docs/check-cjk-span.py XML --partial --tj-qdf QDF.pdf --self-test`。固定样本的五个原始/重建间距数组必须各保留一次-125和75且顺序不变；独立尾事件可能另生成补偿TJ，不错误要求全部TJ只有五个。脚本额外拒绝负间距变更、正间距变更及操作符变更三个QDF负例；两种边界样本通过，混合字体和同链XML检查回归通过。这是固定qpdf输出语法检查，不是通用PDF解析器，也不自动认证两个输入来自同一PDF；调用者须从同一产物生成XML/QDF。
