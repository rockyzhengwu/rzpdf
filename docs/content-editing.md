# 内容编辑

## 显式 TJ 调整构建（验收中）

`ContentBuilder::show_adjusted_encoded_text(codes, adjustment)`及带Limits版本输出`[<字符码> adjustment] TJ`，不推断字体、不编码Unicode。调整数值采用PDF TJ的千分之一字号单位：正值沿写字轴后退，负值向前。它只改变文字矩阵，不应拿Td代替（Td还会改变行矩阵）。`adjusted_encoded_text_with_budget(codes, adjustment, limits, budget)`返回持有实际Vec容量租约的BudgetedBytes；数字格式化使用64字节栈缓冲，不另分配String。非有限数值、字符串/数字token超限、输出超限或额度不足返回错误，普通builder已有字节不变。

这只是自动推进补偿的构建原语。水平文字若希望替换后的结束位置不变，调用方应完整计算新旧推进差，再以`1000 * (新推进 - 旧推进) / 字号`得到TJ数值（推进未乘Tz）；字符间距和只作用于合法单字节空格码的词间距也须计入。不能只按Unicode字符数估计宽度，不能把Tz/页面CTM重复乘入数值；零字号、竖排、缺失/不可信字体度量与状态恢复仍需明确策略。

自动补偿接入（验收中）：`replace_page_text_preserving_advance(page, &snapshot, operation, text)`借用同池文本度量/字体所有者，逐字符码计算新旧宽度及Tc/Tw差，以TJ调整保留原写字轴结束位置。接受Tj和两类引号；Tj转TJ，单引号转T*/TJ，双引号转Tw/Tc/T*/TJ，不改变行矩阵语义。零字号无法通过TJ补偿间距，明确拒绝；溢出/不可编码字符与旧快照按既有规则拒绝。保留结束位置不等于字形能装进旧绘制边界，较长文本可能与后续字形重叠；不是自动排版。

`ContentProgram::with_adjusted_text_codes_with_budget`是对应低层字符码重建，不编码Unicode/计算度量；目标操作内部词法会重写，周围流片段与流数量沿用既有局部替换契约。调整字节、引号前缀、校验及重建程序同池持有租约，数字格式化不分配临时String，前缀完整长度在分配前检查。顶层编辑持有字体/编码所有者至提交返回，不产生裸Font深Clone。

新入口专项原session66501退出0，adjusted_text四项通过、0失败/忽略、0.52秒，日志`/tmp/rzpdf-automatic-text-advance.log`。覆盖Courier单字符串三种操作符、不同新旧长度/空文本、字符/词间距、正负Tz、文本/页面仿射矩阵、后续字符及下一行位置、严格重开和共享页隔离/错误回滚/快照重试；当前生产版本全部PDF单元原session43804退出0，635项通过、0失败/忽略、1.80秒。新加预算重建专项原session78609尚待终态。TJ数组/跨项及嵌套Form补偿、竖排/真实字体专项、完整unwind和独立渲染仍待接入或验收，不能据此勾选完整P3。新旧字符码遍历采用字体visit_chars_limited，不生成额外字符数组；Tw只作用于单字节0x20，Tz/页面CTM不重复乘入调整数值。

新增adjusted_text专项检查数字解析、输出/额度拒绝以及Courier已知度量下后续文字位置；原session65546退出0，两项通过、0失败/忽略，日志`/tmp/rzpdf-adjusted-text.log`。该版本PDF单元原session92362退出0，633项通过、0失败/忽略；不覆盖之后新增的文本度量跟踪，也不证明自动补偿或所有布局状态。

文本度量状态接入：内部BudgetedTextMetrics将BudgetedFont和字号/Tc/Tw/Tz值一起返回，复用已有前缀字体解析。q/Q槽位保存同一个不可变字体引用及标量状态，并按新槽位大小计费；Tf和ExtGState Font更新字号，双引号在目标绘制之前设置Tw/Tc。BT/ET不重置这些图形状态标量。字体状态专项9项和该版本全部PDF单元635项均退出0；它们不覆盖随后新增的自动补偿入口。此状态暂只从页面顶层前缀获取，尚未传递嵌套Form的继承字号/间距。

预算重建验收更新：原session78609退出0，adjusted_text当前五项通过、0失败/忽略、0.47秒，日志`/tmp/rzpdf-adjusted-program-ownership-final.log`，取代上文新增预算专项待确认状态。覆盖三种原操作的峰值差1拒绝、保留邻居、双结果合计拒绝且首结果保持、释放后重试、返回程序所有者unwind释放；生产代码未变，635项PDF单元证据仍对应当前生产版本。这不代替公共编辑链所有失败点/unwind或完整AR-03验收。

竖排合成补验（增强版验收中）：Identity-V与显式W2分别给A/B/空格负推进，字体字号10、Tc=2、Tw=7；空格源码为双字节0020，不应加Tw。检查生成的TJ数值分别为-1000（A→AB）、800（A→空串）、700（A＋空格→B），确认实际WritingMode::Vertical及保存重开后后续字符/下一行位置。初版六项通过；新增独立数值/方向断言的原session25333尚待终态，不能沿用初版结果。字体只提供合成编码与度量，不是实际嵌入竖排字形或独立视觉验证。

后续验收更新：竖排增强版原session25333退出0，六项通过、0失败/忽略、1.36秒，日志`/tmp/rzpdf-vertical-text-compensation-verified.log`，取代上文待确认状态。真实静态Noto Sans SC水平整Tj补偿/无补偿四页对照也已通过本库严格重开、MuPDF独立P/D/F坐标、归一化文本和四页目视检查；交换对照页的四页负例在坐标检查下拒绝，详见[中文构建验证](cjk-creation-validation.md)。这是固定矩阵证据，不表示真实竖排、全部字体、TJ/嵌套补偿或完整P3完成。

公共补偿外层unwind补验通过：Tj/两类引号成功修改后注入panic，检查两共享页源码与Contents流ID恢复、旧快照拒绝、刷新重试与严格保存重开后的文本/后续字符及下一行位置。增强版要求panic载荷精确匹配指定注入消息，防止其他断言失败误认成成功清理；原session71814退出0，七项通过、0失败/忽略、1.36秒，日志`/tmp/rzpdf-compensation-outer-unwind-verified.log`。尚不代表所有公共入口和每一个失败点的完整unwind矩阵。

## Form 调用编辑与分层来源

以下入口只编辑顶层页面 Contents 中选定的一次 Form `Do`，要求当前文档、当前 revision 和同页的 `ContentProgram`：

- `remove_page_form_draw(page, &snapshot, operation)` 删除一次调用，不删除共享 Form 对象或资源，不是安全擦除。
- `transform_page_form_draw(page, &snapshot, operation, matrix)` 用 `q/cm/Do/Q` 在当前用户空间施加相对变换，恢复后续图形状态；不改 Form 自身 Matrix，不提供显示空间绝对定位。新增包装需有图形状态深度头寸。
- `replace_page_form_draw(page, &snapshot, operation, &handle)` 用同文档 Form 句柄替换一次调用，事务注册页面局部资源并复制变化内容流，不自动匹配新 Form 的尺寸、BBox 或 Matrix。跨文档句柄须先通过 `import_resource_from(..., ResourceKind::Form)` 导入完整依赖图，再获取新的页面快照。

每次成功提交或事务回滚后都需重新取得快照。资源注册及内容提交可由外层 `edit_transaction` 一起回滚；保存到外部的字节不随文档事务回滚。Form 导入已有源释放后两层路径资源及私有 Helvetica/RGB 依赖回归；不代表所有字体、图片过滤器或透明组支持。

Form 内路径、文字和图片使用 `nested_source`，其顶层 `source` 为 None。`NestedDrawSource.invocations` 从外到内记录每层父流 Do 索引、字节位置及可选文档局部 Form ID；末端 `operation_index`、`operator_byte_offset` 属于叶流，路径另有叶流 `geometry_operation_indices`。文本/图片的几何索引为空。ID及索引不是持久化或跨修订句柄。

`page.nested_content_program(&root, &nested_source)` 只读验证当前页面快照、每层资源绑定/Do位置、深度/循环/逻辑工作额度和叶操作位置，返回无 stamp 的离线叶程序；路径索引还检查递增顺序及路径操作类型。它不认证完整路径语义，不授予修改共享 Form 的权限，叶索引绝不能直接用于顶层编辑。上述三个文档入口拒绝叶程序快照。

`remove_nested_path_draw(page, &root_snapshot, &nested_source)` 为嵌套路径提供专用入口：验证调用链后将叶绘制操作改为n，从内到外复制Form并用新的局部资源名只重绑选定Do，最后复制页面变化流；原对象和其他调用不改。复制时物化有效Resources（包括旧式省略资源时的页面fallback），保留Matrix/BBox及几何/挂起裁剪。整个资源注册与提交受同一事务保护。已有两层重复调用、共享页隔离、外层回滚、严格重开及路径删除像素回归；复杂资源、独立阅读器与峰值预算仍未完成。

`replace_nested_path_numbers(page, &root_snapshot, &nested_source, geometry_operation, values)` 修改来源几何索引列表中的一个m/l/c/v/y/re操作，保留操作名，拒绝参数错数、非有限数值及不在列表内的索引。复用调用链复制与事务提交，坐标是叶操作当前用户空间，不自动反算显示坐标或联动整个路径。两层重复调用矩形修改已有邻接操作/共享页隔离、失败回滚及严格保存重开回归；六类数值几何操作另有单层Form重复调用、邻接参数及严格重开专项，两层矩形修改已有挂起裁剪/邻接路径像素回归；几何来源列表重新核对paint/n边界，未知前缀和固定操作数错数拒绝，仍非全部图形状态语义验证。来源列表不是不可伪造的授权凭据，不将用户构造的列表视为可信命中结果。

顶层单次删除/变换/替换已有共享页隔离、错误回滚与严格保存重开回归；Skia实色路径专项覆盖旋转/缩放/反射与BBox裁剪、删除及单次颜色/尺寸替换。它们不是独立阅读器、所有复杂内容或完整可视化编辑验收。

## 嵌套内容编辑

多色图片回归还覆盖变换后再次刷新来源并替换绑定：四象限图片先旋转、镜像或缩小平移，再换成另一组四种颜色，整页像素与独立矩形参考一致；第二调用、共享页和严格重开结果保持。此证据限于本库后端的轴对齐样本，不证明任意仿射插值、透明纹理或独立阅读器兼容性。

`replace_nested_text_range(page, &root_snapshot, &nested_source, item, range, text)` 在一个Tj/引号字符串或一个TJ字符串项内，按解码后PDF字符码字节范围替换Unicode文字。使用调用链继承的有效字体验证两端为完整字符码边界，验证替换后整体字符数量/字节额度，再复制提交。范围不是Unicode索引，不跨TJ字符串项，不补偿字形推进。WinAnsi局部替换已有严格重开、共享页及Helvetica像素回归；合成Type0/CMap样本验证混合单/双字节边界拒绝、总字符额度和中文提取读回，但不证明真实中文字体字形渲染。复杂字体、跨字符串范围及连续推进补偿仍待补齐。

`replace_nested_image_draw(page, &root_snapshot, &nested_source, &handle)` 使用同文档Image句柄替换一次嵌套图片Do，在复制叶Form的局部Resources添加新绑定，再复制祖先；保留已有变换，不自动匹配图片尺寸。跨文档图片必须先导入再获取当前快照和句柄。两层重复调用/共享页、仿射后矩阵保持、主动错误回滚、跨文档导入后源释放及严格保存重开已有回归，实色RGB替换另有整页RGBA参考比较；新图入口校验流/Subtype而非完整解码，复杂图像与独立阅读器仍待验收。

`transform_nested_image_draw(page, &root_snapshot, &nested_source, matrix)` 对选定嵌套图片Do施加相对当前用户空间的六参数变换，使用q/cm/Do/Q隔离并复制调用链；不反算显示坐标，不接受Form Do或内联图片。两层Form已有平移、旋转、非均匀缩放及镜像矩阵、共享页隔离、回滚和严格重开回归；实色图片的缩放平移、旋转和镜像另有整页RGBA与直接矩形参考比较。四象限2×2多色图片的旋转、镜像和缩小平移已有两层重复调用/共享页隔离、严格重开及独立矩形参考整页RGBA回归。插值、任意父Matrix/仿射组合及独立阅读器仍待验收。

`replace_nested_text(page, &root_snapshot, &nested_source, text)` 对嵌套Tj/引号进行Unicode替换；`replace_nested_text_array_item(..., item, text)` 对TJ字符串项替换。沿每层父Do之前的Tf/gs及q/Q传递实际有效字体，再扫描叶前缀，使用该字体的现有编码；缺字体、未知前缀或不可编码字符拒绝，不新增字体覆盖/形状化/补偿布局。已验证两层同名字体作用域、无Tf叶继承和局部q/Q恢复，以及TJ/引号清空后重新插入；Helvetica两层重复调用的Tj、TJ、引号删除及WWWW/é€替换已有实际字形和整页RGBA直接参考回归，包括非零leading/字符间距。复杂字体、跨字符串范围、连续推进补偿及独立阅读器仍待补齐。

空Tj不产生字符提取事件；仍可使用解析后父Do/叶Tj的操作索引、字节位置及Form对象ID构造`NestedDrawSource`，由编辑入口重新验证调用链。空literal/hex Tj继承调用者Helvetica插入é€已有严格保存重开、重复调用及共享页隔离回归；不代表已提供自动空事件定位或可视化命中接口。

`replace_nested_text_codes(page, &root_snapshot, &nested_source, codes)` 替换嵌套Tj/引号字符串；`replace_nested_text_array_item_codes(..., item, codes)` 替换TJ完整字符串项（项索引包括数值）。输入是原始PDF字符码，不是UTF-8/Unicode；不自动校验既有字体是否能解码这些字节，也不形状化/补偿推进。调用链复制和事务隔离与嵌套删除共用，修改后重新获取root及来源；数值TJ项不能作为字符串目标。

`remove_nested_image_draw(page, &root_snapshot, &nested_source)` 删除Form叶流中的一次Image Do，在有效叶资源scope中验证Subtype，不接受Form Do或内联图片。它复制并重绑选中的调用链，不删除图片资源，不改变其他调用或邻接路径；已有两层局部scope、重复图片/重复Form调用、外层回滚及严格重开回归，实色RGB删除另有BBox/邻接路径整页像素回归；复杂图像和独立阅读器仍待验收。

`remove_nested_text_draw(page, &root_snapshot, &nested_source)` 清空选中Form叶流的一次文字事件，复用逐层复制、局部Do重绑与整体事务提交。沿用Tj/TJ/两类引号的字符串清空规则，保留TJ数值、换行及间距状态，不补偿字形推进，不要求Unicode编码；不是安全擦除。已有两层Tj/TJ/引号、缺省资源、重复调用/共享页、外层回滚和严格重开回归；Helvetica两层重复文字另有BBox/图片混合像素回归。连续事件推进补偿、跨字符串范围及复杂字体仍未完成。

## 多步编辑事务

可在外层 `edit_transaction` 中组合文字局部替换、图片绘制删除及路径绘制取消；每次成功修改后需重新取得ContentProgram快照，不能重复使用旧修订快照。公共编辑入口的嵌套提交仍受外层回滚保护。已有Err/panic回归验证：事务内保存能读到全部暂存编辑，外层失败恢复原内容流引用和共享页，回滚后保存严格读回仍正确；revision单调推进，因此回滚也会使旧快照失效。事务中保存生成的字节不会被文档回滚撤销，不应提前作为已提交结果发布。

## 删除路径绘制

`remove_page_path_draw(page, &snapshot, painting_operation)` 将S/s/f/F/f*/B/B*/b/b*改为n，不删除几何，也不改变路径结束位置；挂起的W/W*裁剪仍在此结束处提交。源几何、后续绘制和共享页面保留，clip语义不作为删除目标。可使用同修订PathDrawSource的painting_operation_index选择；不做整条几何清理或安全擦除。

已有九绘制操作严格重开回归，以及本库Skia像素回归：删除红填充而保留其W裁剪后，后续蓝矩形只显示裁剪内部分，外部保持白色，保存前后整页RGBA一致。当前像素样本是矩形，不代表所有曲线/闭合/奇偶裁剪或独立阅读器验收。

## 清空单次文字绘制

`remove_page_text_draw(page, &snapshot, operation)` 清空顶层Tj、TJ、单引号或双引号事件中的所有字符串。保留原操作符，双引号的字距/词距和换行、单引号的换行继续生效，TJ数值项及注释保留；不删除状态操作或字体资源，也不要求字体解码/Unicode映射。无效TJ项、非文字目标、过期或跨页快照拒绝，提交使用既有事务和流复制后修改机制。

清空字符串会取消原字形推进，不补偿后续文字的位置；TJ数值调整仍推进。旧内容流可能保留，逻辑删除不是安全擦除。TJ清空借用原源字节进行一次逐项词法扫描，统一替换字符串为`<>`后仅一次完整程序重解析，保留其余字节；替换缓冲预检查原始/解码字节上限并可失败分配。这消除了逐字符串复制/重解析，不代表所有编辑的性能或聚合峰值预算已经验收。

本库Skia像素专项覆盖四类操作：目标AB原先确有墨迹、清空后区域全白，显式独立Tm定位的邻接C和蓝矩形逐像素不变，共享页保留原整图，保存前/严格重开整页RGBA一致。邻接文字使用独立Tm，不据此证明连续文本的原推进或布局保持；当前为Helvetica自编样本，不代表真实字体与独立阅读器矩阵验收。

## 删除单次图片绘制

`remove_page_image_draw(page, &snapshot, operation)` 仅移除顶层Image XObject的一次Do，保留图片资源、其他绘制及共享页。无需图片像素解码，旧资源仍须存在且为Image；不支持Form/内联删除。底层 `ContentProgram::without_operation` 支持零操作替换，仍比较所有邻接操作/操作数并保留流基数和外部字节；它是语法工具，删除q/Q、字体或路径状态操作可能破坏语义，高层图片入口不接受这些目标。不以n或其他状态操作替代删除。

旧流对象和图片数据仍可能保存于文档，逻辑删除不是安全擦除或涂黑；若有隐私删除要求，须另行实现对象回收/残留审计，不能使用此接口宣称数据不可恢复。

## 字符码内部范围

范围编辑内部通过字体受限访问器检查字符码边界及数量，不为这些检查保留完整CharCode向量。SimpleFont仍验证宽度，Type0仍应用有效CMap及水平/竖排度量；最终替换缓冲、字体加载、编码候选表和事务对象克隆不因此获得统一峰值计费。

`replace_page_text_array_range(page, &snapshot, operation, range, text)` 和 `replace_nested_text_array_range(page, &root, &nested_source, range, text)` 支持同一次TJ内跨字符串项的Unicode范围替换。`range`为`(起始数组项, 解码字符码字节偏移)..(结束数组项, 解码字符码字节偏移)`，不是Unicode索引；两端必须为字符串项及完整字符码边界，允许同项零长度插入。文字用有效字体编码后放入起始项，中间选中字符串清空，末项保留范围后缀，数字间距/注释及未选项保留。检查原TJ总字符数、结果总字符数和总字符码字节额度；不跨文字事件，不补偿推进或保证原位置不变。顶层及嵌套重复调用/共享页已有WinAnsi提取和严格读回回归；嵌套入口另验证非法端点无修改、外层失败恢复Resources/根流ID与文字、旧快照拒绝及刷新后重试；合成Type0/CMap另验证跨项双字节两端边界拒绝、总字符超限、单字节替换及严格重开/共享调用码长保持，不证明真实中文字体渲染；Helvetica两层Form的跨两字符串WWWW/é€替换另有BBox/图片混合整页RGBA直接参考，保留TJ数字、清空末项，严格重开/共享调用保持。真实复杂字体渲染、连续推进补偿及独立阅读器仍待验收。

`ContentProgram::with_replaced_text_array_items_codes(operation, replacements, options)` 在一次源码扫描中替换多个TJ字符串项。`replacements`为严格递增的`(数组索引, 字符码字节切片)`，索引包含数字项但目标必须为字符串；未选字符串、数字及注释保持原始字节。字符码由调用方提供，不验证字体，不提供跨项Unicode范围映射或字宽补偿。单项字符码入口复用此实现；输出长度受额度限制，但不代表操作级聚合峰值预算。

局部编辑还会用同一字体校验最终拼接字符串的字符码和max_text_chars，避免旧字符串与插入文本各自合格但合并后超限。超限在写入前返回ResourceLimit，不推进revision；这一检查仅覆盖目标字符串，不等于全页字符合计、操作级聚合内存或布局预算已实现。

局部编辑回归包含混合一/两字节CMap（非固定宽假设），拒绝两字节码中间的替换及零长度插入，合法替换前后严格读回Unicode与原字符码；另覆盖TJ空项插入、局部删除和清空，保留数组基数及正负间距。此为自编字体映射样本，不替代真实嵌入字体/任意CMap完整验收。

局部编辑的边界校验已改用目标有效字体的chars_limited，不再依赖整页绘制/图片解码。字体选择沿用q/Q、Tf及ExtGState Font前缀跟踪，旧字符码须完整解析；新Unicode用同一字体编码。无关后续未支持绘制可保留，但不修复其资源或保证页面可渲染；前缀未知操作仍保守拒绝。早期整页解释实现的限制以此更新为准。

`replace_page_text_range(page, &snapshot, operation, array_item, range, text)` 按解码字符码范围替换部分字符串，Unicode替换文本用目标有效字体编码。TJ必须指定Some(完整数组项索引)，Tj/两类引号必须None；范围起止必须是完整字符码边界，允许零长度插入或空文本删除。目标字体直接验证边界，文档入口校验来源快照当前性与页面身份。仅目标字符串重新编码写出，字符串前后字符码、TJ数值/其他项、引号行为、邻接字节与共享页保留；不保证原字符串内部转义拼写不变，也不补偿变化后的字宽。

`CharItem::decoded_code_range()` 与 `ExtractedCharacter::decoded_code_range` 提供字符码在其PDF字符串解码字节中的半开区间。单字节字体按一字节递增，CID按CMap解析出的实际字符码长度递增；TJ每个字符串项独立从零计数，并结合array_item_index定位。手动构造字符没有范围。它不是Unicode字符索引、UTF-8范围或字形边界，也不是原始PDF转义源码偏移；字面量八进制/括号转义、十六进制和压缩内容必须先解码。编辑仍需同修订来源校验及字体字符码边界验证，不能把该范围直接切入ContentProgram源码。

## 图片显示坐标平移

显示空间仿射另有本库Skia像素回归：40×40页面将两像素红/绿横纹图旋转为竖纹图，验证纹理方向、旧区域白色及邻接蓝图；严格保存重开前后整页RGBA缓冲一致。这是自编样本和本库后端证据，不替代独立阅读器、任意仿射采样/裁剪/透明组合验收。

`translate_page_image_draw_display(page, &snapshot, operation, dx, dy)` 接受页面显示空间位移（左上原点、向下为正），解释当前页面并核对来源，通过该次图片CTM与页面旋转/裁剪变换的线性部分反算当前用户空间位移，再用q/cm/Do/Q隔离提交。图片归一化的y反射在反算中移除；不直接把ImageObject矩阵的位移方向当PDF用户空间。原图尺寸/线性变换和后续绘制保持。

非有限位移、非图片目标、奇异变换和反算后f32溢出在写入前拒绝；精度仍受f32限制。此入口完整解释并解码页面图片，暂不支持仅从原始资源无解码移动未知编码图片；页面内其他不支持内容也可能导致失败。不实现绝对定位、文字路径坐标编辑或像素可见性。相对用户空间变换仍用 `transform_page_image_draw`。

`transform_page_image_draw_display(..., [a,b,c,d,e,f])` 将显示空间仿射变换作用于当前图片的显示结果，可用于旋转、缩放、剪切、镜像和带支点的组合。平移入口复用该实现。内部设C为当前CTM与页面矩阵组合、D为请求变换，计算用户空间变换 `C * D * inverse(C)`，移除图片归一化时同时处理y反射和位移；乘积和逆矩阵采用f64中间值，最终写入f32。绕某显示点的支点变换须由调用方组合相应平移，不自动选择中心；奇异的原C拒绝，有限但退化的请求D允许将图片压缩为零面积。

## 图片几何命中

`DisplayList::image_geometry_hits(x, y)` 在页面显示坐标（左上原点、向下为正）中返回图片单位方形的仿射命中候选，后绘制优先，结果含DisplayList对象索引与图片引用；查询本身无分配。矩阵反算使用f64中间值，包含方形边界，奇异矩阵没有二维命中区域。单图片可用 `geometry_contains_display_point`；非有限坐标或图片矩阵产生错误，列表迭代按每个图片返回该错误，空列表不执行图片判定。

这是几何候选，不是可见性或像素命中：不判断裁剪、透明、软蒙版、图片采样及文字/路径/其他图片遮挡，也不提供文字/路径命中。选择后必须校验DisplayList当前stamp及与ContentProgram配对，再把图片来源操作索引用于编辑。不要把显示坐标直接作为相对图片变换的当前用户空间参数。

## 来源身份统一契约

图片报告、字符报告、文字run报告及解释器产生的DisplayList共用同页/同文档/同修订配对规则。文字两类报告提供 `source_stamp()` 与 `matches_program()`；DisplayList的stamp可选，手动构造没有来源且不匹配任何ContentProgram。路径可通过其所属DisplayList验证配对，再使用路径操作索引。先验证stamp当前性，再验证快照配对；两个过期结果仍可能互相匹配。

直接取Vec或消费DisplayList为对象会丢弃集合身份，单个绘制对象仍只有位置，不是稳定句柄。报告公开的事件载荷允许调用方修改，配对只证明提取时的文档状态，不认证用户修改过的载荷。文字报告新增私有身份字段，外部字面量构造需改用页面提取入口。报告与root配对不验证嵌套叶索引；Form分层来源须另用上文的只读定位接口校验。

## 文字绘制来源

顶层 `TextObject::source()`、提取字符及文字run的 `source` 对应同修订 ContentProgram 的操作索引和操作符字节偏移，覆盖 Tj/TJ/单引号/双引号。TJ字符另有 `array_item_index`，按完整数组计数（包含数值与空字符串），可选择 `replace_page_text_array_item` 的字符串目标；非TJ字符无数组索引。空字符串没有字符事件，需直接查询ContentProgram定位。

顶层字符来源还提供解码字符码范围，不是转义PDF字符串的源字节范围；不提供完整字形命中/布局补偿或跨修订句柄。Form文字另使用上文的nested_source。`replace_page_text`及字符码入口支持Tj和两类引号，保留原操作符、间距与注释；TJ用单独数组项入口。引号换行/间距继续生效，变化字宽影响后续位置。手动TextObject应使用TextObject::new，提取结构手动字面量需补新增字段。

## 路径绘制来源

顶层解释所得 `PathObject::source()` 返回可选 `PathDrawSource`，记录显式 m/l/c/v/y/re/h 的操作索引，以及最终绘制操作索引和字节偏移。路径可跨内容流、穿插图形状态操作；q/Q不保存或清空当前路径，n或绘制结束清空下一路径的来源。s/b/b*隐式关闭属于绘制操作，不额外生成虚构的h来源。

这些索引可对应同修订 ContentProgram 并选择几何参数编辑，但不包含CTM历史、裁剪来源或显示坐标反算，也不是跨修订稳定句柄；手动构造的路径无来源。新增来源向量受内容操作数上限限制并使用可失败分配，不代表操作级聚合内存预算已完成。

## 图片绘制来源

`ImageExtractionReport` 另保留私有文档修订及页面身份，`source_stamp()` 可用于文档当前性检查，`matches_program(&program)` 校验报告与ContentProgram同文档/同修订/同页，拒绝离线程序及重开文档。配对前应同时检查 `doc.is_stamp_current(report.source_stamp())` 和 `report.matches_program(&program)`；两个旧快照彼此匹配不代表仍是当前状态。报告中的绘制索引/字节偏移本身仍不携带身份，便捷Vec提取入口丢弃报告身份和诊断。报告新增私有字段，外部手动字面量构造不再支持，使用页面提取入口获取报告。

解释器产生的顶层 `ImageObject` 可通过 `source()` 取得 `ImageDrawSource`：`operation_index` 是 ContentProgram 中从零开始的操作索引（包括未知操作），`operator_byte_offset` 是拼接后的解码 Contents 中 `Do` 操作符自身的字节位置。来源直接来自解析事件，不通过绘制顺序反推；支持注释、重复绘制及跨流拆分操作。

来源仅对应产生绘制的页面修订，不携带文档身份或修订校验。编辑必须使用同页同修订的 ContentProgram 快照，由文档编辑 API 校验；内容变化或重新打开后重新提取，不能将旧索引当稳定句柄。此处顶层source不覆盖Form内部；Form内图片另有nested_source，内联图片仍未支持。

手动通过 `ImageObject::new` 构造的对象没有来源。新增私有来源字段后，外部结构体字面量构造需迁移到该构造函数；原公开字段仍可访问，但修改绘制对象不会编辑原文档。

## 路径几何参数编辑

`document.replace_page_path_numbers(page, &snapshot, operation, &values)`替换选定`m/l/c/v/y/re`操作的有限f32数值参数，保持原操作名称及所有邻接操作/注释。参数个数分别为2/2/6/4/4/4，旧操作参数也须为合法有限数值；负矩形宽高允许。提交校验当前文档/revision/页面并事务复制变化流，其他共享页不变。

坐标属于该操作执行时的当前用户空间，不是裁剪/旋转后的显示坐标；不自动反算CTM、不做命中测试、整体路径移动或控制点联动，不改变填充/描边/裁剪规则。f32入口不承诺任意精度的几何转换。六类操作严格保存读回及DisplayList无诊断、矩形注释/共享隔离、非法参数/错页拒绝已有回归，不等于独立像素或完整路径状态验收。

Skia像素回归另外验证30×10页面中仅修改首个红色填充矩形的位置：原区域中心为白色、新区域红色，邻接蓝色矩形不变；公共编辑后保存前与严格重开中心RGBA精确一致。render16项通过，此证据不覆盖曲线轮廓/描边/裁剪的全部像素或独立阅读器。

## 页面图片资源替换

已有非均匀CTM下的旋转/缩放/镜像组合专项按提取矩阵的图片左上归一化及页面显示转换核对六个分量，后续绘制矩阵保持，保存前后均验证。Skia像素专项在30×10页面中验证已有10倍缩放下局部平移1单位使红图移动10页面单位：原区域白、新区域红、后续蓝图区域不变，保存前/严格读回后中心RGBA一致。render15项已通过；这些样本不是独立阅读器或所有仿射/裁剪/颜色空间验收。

图片变换另外检查全Contents中的q/Q零操作数、配对及配置深度，要求目标处有额外一层状态保存的额度；异常或超限在提交前拒绝。深度8的专项验证原层数7可加入隔离层并在相同限制严格保存读回解释，原层数8拒绝且revision/内容不变。此为q/Q结构与深度检查，不代替其他图形状态、文字对象、未知操作或原生状态克隆的聚合预算验收。

`document.transform_page_image_draw(page, &snapshot, operation, matrix)`为选中的顶层图片Do添加隔离的q/cm/Do/Q变换；`[1,0,0,1,dx,dy]`表示平移。矩阵相对于原Do执行时的当前用户空间，保持原资源/图片数据，并恢复图形状态以免影响后续绘制。它不是显示坐标下的绝对定位；已有缩放/旋转会作用于该相对变换，不自动反算CTM。单次替换增加3个操作，沿用来源校验、流COW、额度和邻接操作/操作数不变检查。NaN/非Do拒绝、两个绘制仅首个平移/第二个矩阵不变、共享另一页及保存读回已有回归；全局图形状态/深度边界、复杂裁剪、独立像素与跨阅读器仍需补验。

`document.replace_page_image_draw(page, snapshot, operation, &image_handle)`替换顶层Contents中选定的一次图片Do。校验来源文档/revision/页面和现有Image资源，注册新的或复用已有图片资源名称，再局部替换操作；仅变化流复制写入，保留其他操作/注释、共享原流及绘制变换。资源注册和内容更新处于同一事务，提交内容时使用最新页字典，避免覆盖新资源。错页/过期/离线快照、非Do或Form、外部图片句柄拒绝；不定位Form内部或内联图片，不自动缩放到指定尺寸，不是安全擦除。单次蓝图/其他绘制红图/共享另一页不变、原注释保留、严格保存读回及外层回滚恢复资源和内容已有集成回归。

若新名称注册之后操作替换超出内容额度，整个编辑回滚：资源名称、原流ID和原始内容恢复。精确额度少1字节专项及严格保存读回已覆盖该路径。回滚遵循单调revision契约，旧快照随之失效，调用方须重新读取快照；不要把“对象状态已恢复”误解为revision回到原值。初始过期快照在事务前拒绝，不推进revision。该测试验证逻辑内容额度，不证明注册/克隆的聚合峰值预算。

若目标图片已有页面资源名称，注册会复用该名称；写入Do时使用PDF名称转义，不直接拼接资源名。含空格/斜线/#的`A /#`名称回归确认输出`/A#20#2F#23`、操作数量及资源名集合不变，严格读回仅目标绘制改变、邻接绘制和共享原页保持。

`document.resources().replace_image_handle(page, resource_name, &handle)` 将目标页已有图片名称绑定到同文档另一个图片对象。先校验句柄、页号及旧/新Subtype Image流，再复制有效继承Resources/XObject字典并事务写回目标页，不修改原图片或共享资源字典。句柄可通过`object_handle(id)`取得；其他文档图片需先导入，不能直接使用外部句柄。

该入口改变这一页对该名称的所有使用，不改Contents字节或变换，不定位单次Do、Form内部或内联图片。仅校验图片资源类型，不重新解码全部过滤器或自动重采样；调用方负责新图的尺寸/编码/颜色语义，保存约束仍独立检查。旧对象字节保留，不是安全擦除，资源克隆的整体峰值预算仍待统一。共享页隔离、两次绘制、严格保存读回、外部句柄/缺失名称/非法页及外层事务回滚已有集成回归。

继承回归另外覆盖两页共享父节点Resources，以及间接Resources/间接XObject映射：仅目标页生成局部Resources，父节点和另一页字典不变，原Contents引用和Font资源保留；保存前后目标页蓝图、另一页红图的解码数据一致。此为资源隔离/本库解码回归，不是独立阅读器视觉验收。

类型失败回归覆盖新对象为null、普通字典、缺少Subtype流或Form流，以及旧名称实际绑定Form的情况：全部拒绝，目标页/共享原图及revision不被失败调用改变。不将此入口当作任意XObject替换接口。

## 内容流文字编辑

当前接口编辑已有字体字符码，不把 Unicode 字符串直接写入内容流。

| 场景 | 离线接口 | 文档事务接口 |
|---|---|---|
| 单字符串 Tj | `ContentProgram::with_replaced_text_codes` | `replace_page_text_codes` |
| TJ 数组中的一个字符串 | `ContentProgram::with_replaced_text_array_item_codes` | `replace_page_text_array_item_codes` |

直接传入Unicode可使用文档接口 `replace_page_text(page, snapshot, operation, text)`（Tj）及 `replace_page_text_array_item(page, snapshot, operation, item, text)`（TJ单项）。提交前追踪目标操作之前的Tf、q/Q恢复和ExtGState Font设置，按该有效字体的既有映射编码；不选择新的回退字体。BT/ET只改变文字对象状态，不清除字体选择。状态栈受深度和逻辑字节额度限制。

Unicode接口在缺少有效字体、目标不在BT/ET内、前面存在未知操作、Q无对应q或文字对象嵌套错误时拒绝，不猜测字体状态。只追踪顶层Contents的字体选择，不提供Form内部定位或完整图形/布局验证。

ExtGState支持Font、LW、LC、LJ、ML、D、FL、ca、CA、BM及可选Type，Font/字号应用到文字提取与绘制，并受q/Q恢复；数值及资源引用进行检查。虚线项数/逻辑字节和分配受限。ca/CA分别设置非描边/描边alpha，默认为1，只接受有限的[0,1]数值。路径填充与描边、文字Tr0～2及图片绘制使用对应alpha；图片非描边alpha与已有图像SMask相乘，透明文字仍可提取。填充加描边分别绘制，使用各自颜色/alpha。

BM支持16种标准名称（Normal、Multiply、Screen、Overlay、Darken、Lighten、ColorDodge、ColorBurn、HardLight、SoftLight、Difference、Exclusion、Hue、Saturation、Color、Luminosity），Compatible按Normal处理；名称数组选择首个可用模式，验证全部元素及额度，没有识别到模式时回退Normal（包括空数组），遵循 [ISO 32000-1 §11.6.3](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf)。单个未知名称仍产生未支持诊断或严格拒绝，不部分应用其他参数。路径、文字和图片均映射到Skia混合模式，q/Q恢复Normal或之前的模式。像素回归覆盖9种常见模式的RGB/常量alpha组合，以及Multiply文字/带图像软遮罩的图片；未据此验收全部非可分离模式、任意混合颜色空间、色彩管理或透明组。

尚不支持的图形状态软蒙版、透明组等条目使整个gs在严格模式拒绝、兼容模式产生诊断且不部分应用；Unicode字体绑定遇到这类gs或未知混合模式也拒绝，以免编码与解释采用不同状态。缺失资源或非法已支持参数报错，不伪装成完整的透明模型。

透明功能在 [PDF 1.4引入](https://opensource.adobe.com/dc-acrobat-sdk-docs/library/pdfmark/pdfmark_Basic.html)。保存时检查有效Header/Catalog及显式输出版本：页面/Pages/Form资源中的透明ExtGState参数、声明Type的透明ExtGState、Image SMask和透明Group，低于1.4明确拒绝，调用方可显式选择SaveOptions版本升级；不自动改写原文档。已有有效Catalog版本覆盖也可满足最低版本。普通自定义字典的同名ca/BM字段不被猜测为透明功能。

保存还对准备好的输出流检查Filter，包含保留原编码的路径，支持名称、数组及间接引用。按 [ISO 32000-1 表6](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf)，FlateDecode最低1.2、JBIG2Decode最低1.4、JPXDecode和Crypt最低1.5；Fl名称也按1.2检查。不足时要求显式升级，不更改原流字节。未知过滤器仍保留，不据此证明其版本兼容或解码可用。普通自定义字典的Filter字段不作为流过滤器检查。

回归使用真实Flate字节验证升级后编码字节保持且可解压；其他过滤器的占位负载只验证版本/字典和原字节保留，不是有效编码或解码验收。外部流FFilter共用上述编码版本检查（名称/数组/间接引用）；非null的F/FFilter/FDecodeParms字段本身要求至少1.2，见 [PDF 1.5 流字典](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.5_v6.pdf)。升级保存只保留字典和内联字节，不读取外部载荷，不验证外部文件存在/编码正确；未知过滤器的特性版本及其他功能最低版本尚未全面覆盖。

声明Type为Metadata的流（包括间接Type）保存时要求有效版本至少1.4，遵循 [ISO 32000-1 §14.3.2](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf)。高层Metadata接口已有版本限制，此检查额外覆盖低层插入/导入和未使用的已序列化流；不把普通字典的同名Type视为Metadata流，不验证XML/XMP语义或所有组件Metadata关联。

Subtype为Image的流若BitsPerComponent为整数16（支持间接值），保存要求至少1.5，参见 [PDF 1.5 图像字典](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.5_v6.pdf)。测试使用单像素16位DeviceGray样本，升级保存后原样本字节不变且解码为RGBA[128,128,128,255]；普通自定义流的同名位深字段不被判为图像。此为最低版本检查，不是位深/过滤器匹配、非法图片字典或所有图像编码的完整符合性审计。

Type为ObjStm或XRef的保留流同样要求有效1.5，见 [PDF 1.5 §3.4.6–3.4.7](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.5_v6.pdf)。全量保存仍生成新的传统xref表；最低版本回归覆盖直接/间接Type及未使用的空结构流保存读回，不表示输出采用xref流、自动清理旧结构流或完成活动压缩对象/xref链的独立验收。普通字典的同名Type不作为结构流判断。

`structural_rewrite`另外覆盖自编活动XRef/ObjStm单页文件（四个压缩对象）：原始/Flate载荷与两种保存策略组合，修改压缩对象和页内容后严格重开，再保存重开仍保留修改。压缩对象流内索引不作为generation，字典在限定对象边界内尾部空白合法。此为本库闭环，不是独立阅读器或真实样本完整结构链验收。

事务回归另外覆盖未显式预读压缩对象便修改、随后页面懒加载和事务内保存；普通错误及panic均恢复源值和原空页面内容，回滚后严格保存读回仍保持恢复结果，revision单调推进。该夹具不证明全部缓存/预算/输入源异常组合已验收。

压缩对象删除回归覆盖原始/Flate源、预加载/未预加载：删除后懒展开旧ObjStm和全量保存都不让该对象复活，严格重开后即使主动解析保留的旧ObjStm，已释放对象仍不能解析；第二次保存保持删除。调用方另行移除示例Catalog的入向引用，以免留下活动悬空引用。旧结构流字节仍保留，逻辑删除不是安全擦除或涂黑，本库没有据此实现自动清理所有入向引用或敏感数据残留。

此检查覆盖已序列化对象（包括未使用的已插入对象），不是一般PDF功能版本或符合性审计；其他功能最低版本、一般透明组及跨阅读器验收仍待收口。检查的临时资源解析与保存缓冲整体峰值仍未统一计费。不据此声称低版本转换、所有透明PDF或PDF/A可用。

通过 `document.get_page(page)?.content_program()?` 获取快照，再选择操作索引。TJ 的 `item_index` 按全部数组元素计数，包括数值项；只允许替换字符串，不能把数值项当作文字。字符码必须由已有字体的编码器产生。不能用 UTF-8 或 PDF 元数据字符串编码代替字体编码。

`page.encode_text_for_font(resource_name, text)` 为显式选定的已有字体提供 Unicode→字符码转换。简单字体优先反向使用该字体 ToUnicode；只有 ToUnicode 不存在时才使用 Encoding/Differences 回退。支持一个字符码映射多个 Unicode 标量（包括非BMP字符），先确认剩余文本也可编码，再选择最长匹配；相同映射选择较小字符码，使结果确定。不可编码文本明确失败，不插入 U+FFFD、不改写字体映射。不推断操作处的有效 Tf，调用方须确保选定字体就是目标操作使用的字体。

复合字体要求已有 ToUnicode 和 Encoding CMap。候选字符码检查1～4字节长度、ToUnicode codespace（存在时）、Encoding实际解码长度和非零CID，避免整数转字节时截断或直接选择 `.notdef`。相同Unicode映射优先较短字符码，再按较小源字符码确定选择；最终输出用Encoding重新解码并验证Unicode完整读回。没有ToUnicode、映射到CID0、长度矛盾或不可表示的文本明确失败。不新增CID/字形/字体映射，也不保证CID对应可用的可见字形。

编码输入字符数、映射项数及动态规划表/输出等逻辑工作额度受 Limits 限制；简单字体还计费复制的映射字节，复合字体借用既有映射并计费候选表、动态规划表与输出，编码读回使用CMap受限访问器，不再创建读回字符数组。这不是字体加载、原生字体后端或整个编辑流程的统一峰值预算。匹配工作量随输入长度及候选映射数量增长，未提供独立CPU预算。

离线接口返回新程序，不修改文档，且清除来源身份。事务接口接收原始页面快照，校验文档身份、修订号、页面身份、当前字节、流边界及对象ID；跨文档、错页、过期及离线快照被拒绝。事务回滚恢复内容，但修订号保持单调递增，旧快照必须重新获取。

提交保留内容流数量和空流，未变化流保留原引用与原始编码。变化流复制为新对象，保留非编码字典属性，清理原 Filter/DecodeParms/DL 并更新 Length，不直接覆盖其他页面共享的流。操作跨流时，替换内容放入操作起始流；来源位置是解码后的偏移，不是文件偏移。

TJ 单项替换保留数组内其他字节，包括字符串词法形式、数值间距和注释。整个操作被重解析，前后操作的名称和操作数保持不变；替换尾部会增加换行，防止注释吞掉后续操作。

低层字符码接口不保证字体绑定；Unicode文档接口在可追踪的顶层程序中使用有效字体。所有接口均不保证字形覆盖、字宽不变或视觉布局不变。编码辅助接口遵循当前Unicode映射，但不验证字体映射本身的正确性。TJ 数值按原值保留，不自动补偿新文字的宽度。新字体选择、缺失字体映射构建、整段替换、布局补偿和安全删除尚未实现；替换也不等于清除旧对象或隐藏信息。

回归入口：`cargo test -p pdf --test content_program`；共享编码流专项：`cargo test -p pdf --lib text_commit_copies_shared`。fuzz 专项按用户要求暂缓。
