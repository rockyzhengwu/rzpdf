# TrueType 字符映射验证

边界修正版有限验收结果（2026-10-09）：闭包5项、组装/公共注册前缀10项均通过；后者直接检查保存前及严格重开后的FontBBox。两个真实示例已重生成，qpdf检查及MuPDF精确文本提取通过，96DPI三页PNG逐页查看无缺字、重叠或裁切。提取字体/tmp/rzpdf-bounds-extract.QYG20U/font-0006.ttf、font-0008.ttf经python3.11 -O独立脚本与七固定负例均通过（含错误整体边界但有效校验和）；Arial为17标量/26GID/5真实复合，Noto为44/45/0。MuPDF直接显示FontBBox：Arial[-1.4648438,-205.5664,768.5547,869.1406]（设计单位按2048缩放），Noto[16,-229,974,880]。预算化闭包只联合有效字形头，不重新验证所有简单轮廓/提示指令语义；有限样本不代表全面字体符合性。修正版完整workspace及自动新增字符选区编辑尚未完成；下方“进行中”与首轮记录保留为历史。

边界修正版进度（2026-10-09）：首轮冻结版workspace已退出0，1027项通过；该结果不覆盖随后修正。GlyphClosure现在于同一预算化依赖遍历中计算保留有轮廓字形头联合，忽略空复合，全空为零框，写入head；闭包5项及组装前缀9项专项已通过。独立检查器新增整体边界检查，旧Arial产物在-O下明确拒绝；新增校验和修复后的错误边界负例，待新样本运行七负例。公共注册/严格重开FontBBox测试会话46394、Arial生成90747、中文生成53660均未终态，不把旧产物或旧绿色门禁当修正版验收。下文首轮记录为历史字段范围，真实重生成/独立核验/逐页视觉完成前不宣称边界修复闭环或全面字体符合性。

当前已确认的元数据缺口（2026-10-09）：首轮子集head仍沿用源字体整体边界；Noto为(-1002,-1048,2928,1808)，保留轮廓头联合实际为(16,-229,974,880)；Arial为(-1361,-665,4096,2060)，联合实际为(-3,-421,1574,1780)。按[官方head规范](https://learn.microsoft.com/en-us/typography/opentype/spec/head)应计算有轮廓字形联合、忽略无轮廓字形。现独立检查器通过的字段不包含整体head边界；渲染与逐字轮廓通过不能证明该字段正确。冻结版完整门禁结束后优先修正、补独立检查并重新生成验证；当前不宣称完整字体符合性。

2026-10-09 实际复合子集：`create_subset /System/Library/Fonts/Supplemental/Arial.ttf ArialMT 716 80 output/pdf/rzpdf-latin-subset.pdf`生成固定文本`Composite subset - é Å ö ñ ç é`，许可/fsType沿用源字体。本机源Arial没有复制入库，系统字体未修改。独立qpdf、MuPDF提取/96DPI视觉及`check-cjk-subset.py SOURCE.ttf EXTRACTED.ttf STEXT.xml --latin --self-test`在python3.11 -O下通过：17标量、26保留GID、5所选真实复合字形，校验和、组件原GID、展开轮廓、度量、指令、许可和六负例正确。辅助LTSH/hdmx/VDMX/PCLT仅在显式允许省略时删除并报告；不将陈旧缓存保留或未知表静默丢失。该样本不是全面字体符合性验证，最新workspace门禁另进行中。

2026-10-09 实验子集真实中文首轮验证：`create_cjk ... --subset`使用既有静态Noto源（上述来源/许可记录），生成`output/pdf/rzpdf-cjk-subset.pdf`。qpdf结构、MuPDF两页精确文本和96DPI逐页视觉检查通过；FontTools只读脚本`check-cjk-subset.py SOURCE.ttf EXTRACTED.ttf STEXT.xml`验证44标量、45保留GID、原GID映射、校验和、轮廓、水平/竖向度量、字形指令、未选空字形和版权/许可/fsType保持。源字体10,595,948字节，嵌入子集381,196字节，PDF384,596字节；构建池peak546,136字节。仅固定两页水平样本，不代表实际复合字体或新增字符编辑闭环；完整workspace及检查器负例仍待补。字体不新增入库，fuzz暂缓。

真实复合字形对照（2026-10-05后续）：示例现调用统一 `checked_glyph_outline`，只读比较脚本允许复合并提供 `--require-composite`，不能用全部简单样本冒充复合验收。使用本机 `/System/Library/Fonts/Supplemental/Arial.ttf`，无 fvar，SHA256 `525979822591a3447cfc49d943d6f7683508e25543407871c0ed8fed05fd2bd9`；未复制/发布系统字体。实际复合 é/Å/ö/ñ/ç 的 GID、独立 FontTools BoundsPen 边界和 PointInsidePen 非零绕数分类全部一致：1445 采样点，0 数值 Boundary 排除。复现：先构建示例，再运行 `python3 docs/check-glyph-outline.py /path/to/Arial.ttf --characters 'éÅöñç' --require-composite`（Python 需已有 FontTools，本机复用 3.11/4.39.4）。只含 A 的 require-composite 输入拒绝，空列表拒绝。这个有限字体、默认实例、未 hinting 网格不代替全面 flags/嵌套/点对齐/phantom、字体指标、PDF 映射/可见性或真实渲染验收；合成嵌套/点对齐测试仍是单独证据。

PDF 复合轮廓入口（2026-10-05后续）：`Font::glyph_outline` 按 PDF code/CID→GID 查询统一 SFNT 轮廓；`TextObject::character_outline_display` 及其边界/几何位置查询现支持上述默认实例复合轮廓。原 `Font::simple_glyph_outline` 行为不变。扩展既有测试：不含 Unicode cmap 的合成 FontFile2 通过显式 CIDToGIDMap 将字符65映射至复合GID0、66映射至空GID2；原文档和严格保存重读的 em 命令、独立显示坐标、顶点 Boundary 及空轮廓均通过。PDF 库335项单元测试通过。此合成共线轮廓只证明映射/坐标/边界查询链路，不证明内部填充或真实复合字体渲染；phantom/hinting/metrics/变体、透明/裁剪/可见性边界不变，下一步真实字体独立对照。

SFNT 复合 em 入口（2026-10-05后续）：`TrueTypeProgram::checked_glyph_outline` 接入展开点及二次命令并按同源 head.unitsPerEm 归一化。原 simple-only API 保持复合拒绝，两接口共享 SFNT/head 与归一化逻辑。四表三字形测试验证复合缩放与位移后的独立 em 坐标、简单入口等价、空/越界和深度拒绝；PDF 库 335 项单元测试通过。尚未更改 Font 或显示轮廓调用路径，不以独立 SFNT 测试代替 PDF code/CID→GID 或真实字体独立对照；phantom/hinting/metrics/变体仍未支持。

复合二次轮廓命令（2026-10-05后续）：`ExpandedGlyph::outline_commands` 直接使用 f64 展开点，保留分数变换与隐含中点，按独立轮廓输出闭合二次曲线，不取整/折线化。端点使用 usize，不受简单 glyph 单个 u16 端点范围限制；公共数据调用时重验严格递增、完整点归属和有限值。点、端点与命令逻辑存储合计受字节限额约束，另限制命令与对象数，可失败分配。测试验证两离曲线分数点生成精确隐含中点、非法范围/NaN/限额/空数据，以及三层展开样本的命令数和实际边界；PDF 库 334 项单元测试通过。尚未接入 SFNT/em 或 PDF 字体轮廓查询，真实字体、phantom/hinting/metrics/变体和整体容量峰值边界不变。

复合点展开（2026-10-05后续）：新增 `GlyphLocations::expanded_glyph`，经依赖检查后显式栈后序组装；按组件顺序重编号真实点与轮廓端点，保留 f64 分数值和 on-curve 标志。遵循 [OpenType glyf 变换与对齐规则](https://learn.microsoft.com/en-us/typography/opentype/spec/glyf)：完整矩阵先作用于子点，再对齐父/子控制点；scaled offset 也经矩阵，unscaled 或无标志时直接位移。没有 hinting、像素 grid rounding、phantom 点、USE_MY_METRICS 或变体实例；真实点之外的对齐索引明确报错，不能假装完成 phantom 对齐。图深度/访问数前置；输入、组件、栈帧、叶转换及父级合并做累计逻辑字节计费，最终点/轮廓数量受限，可失败分配；部分解析分配发生在聚合计费前，容器容量和整体峰值并未完整覆盖。TrueType 40 项通过，包含三层嵌套与点对齐的独立坐标预期、两种 offset、空字形、循环及限额。尚未生成复合轮廓命令、接入 PDF/em 查询或做真实字体独立对照；先前 simple API 仍仅支持简单字形。

复合依赖图检查（2026-10-05后续）：`GlyphLocations::validate_composite_dependencies` 使用显式栈，不依赖原生递归；活动路径重复 GID 拒绝，合法 DAG 的重复引用允许但分别计费。根深度为 1，空根也访问一次；`max_depth`、`max_objects` 和累计输入/逻辑组件/栈帧字节限制展开工作，返回访问次数及最大深度。原始输入与帧成本在组件解析前检查，组件逻辑成本在解析后累计；不是容器容量或全操作峰值预算。简单叶只校验公共头，不解码点，也不执行变换、点对齐、metrics 或 hinting。专项 38 项通过，覆盖 DAG、循环、三类额度、非法引用、截断和空根；此前记录中“未检查循环”已由此项补齐，复合轮廓展开仍待实现。

复合字形组件解析（2026-10-05）：新增CompositeGlyph/GlyphComponent及loca.composite_glyph，按[OpenType glyf组件规范](https://learn.microsoft.com/en-us/typography/opentype/spec/glyf)读取组件GID、signed字节/word位移或unsigned点索引、F2DOT14等比/分轴/2×2矩阵，保留原标志及借用指令。任意组件的指令标志都会触发尾部读取；首组件要求XY，保留位/互斥缩放/双offset标志/越界GID/截断拒绝。数量与逻辑组件存储限额前置、可失败分配，不等于容器容量/操作峰值计费。TrueType37项通过；这只是记录解析，尚未执行点对齐、offset变换、USE_MY_METRICS、递归/循环、hinting或轮廓展开，既有simple轮廓入口仍明确拒绝复合。

真实简单字形网格对照（2026-10-05后续）：inspect_glyph_outline新增可选`--grid`，在实际em边界内输出固定17×17单元中心的Outside/Inside/Boundary编码。只读`docs/check-glyph-outline.py`逐字核对FontTools GID、BoundsPen实际边界及PointInsidePen非零绕数分类，样本必须非空简单字形；空字符列表拒绝，不产生空验收。FontTools没有对应三态边界接口，脚本显式统计并排除本库Boundary，不隐藏排除数。上述同哈希静态Noto输入的O/A/B/中/一实际运行：五字形边界/GID一致，1445分类点全部一致，0边界点排除。复现：先`cargo build -p pdf --example inspect_glyph_outline --locked`，再用装有FontTools的Python执行`docs/check-glyph-outline.py /path/to/static-font.ttf`。本机已有Python3.11/FontTools4.39.4可用。此固定合法样本网格不是fuzz，不验证全部字体、曲线上精确边界或PDF可见性/编码。

真实静态字体对照（2026-10-05）：新增只读`inspect_glyph_outline`示例，接受字体路径和单个Unicode字符，输出GID、命令数、em边界及三个采样位置的几何分类。该示例针对独立字体的Unicode cmap，不替代PDF code/CID→GID路径。用MuPDF从既有rzpdf-cjk-example.pdf提取FontFile2到独立临时目录，得到静态NotoSansSC，SHA-256为`0516f3d1fb52efaf83f1d7bc540d779d1eb405f37e0dbb7696537444a0931087`，与既有许可/来源记录一致（见[cjk验证](cjk-creation-validation.md)）。字符O：GID48、2轮廓、28命令，实际em边界`[0.058,-0.013,0.684,0.746]`；中心Outside，左侧`[0.0893,0.3665]` Inside，框外Outside。Python3.11中的FontTools4.39.4 BoundsPen及PointInsidePen独立核对GID/边界/孔洞和左侧笔画一致。默认Python缺FontTools，复用已有3.11环境，未安装依赖或修改PDF。

复现：`cargo run -p pdf --example inspect_glyph_outline --locked -- /path/to/static-font.ttf O`。示例只报告位置，不将所有字符的框中心假定为内部；当前只验证一个真实简单字形，不证明复合/CFF/可变字体、PDF可见性或完整文字命中矩阵。

简单轮廓几何命中（2026-10-05后续）：`outline_hit::outline_location` / `TextObject::character_outline_location_display` 返回Inside/Outside/Boundary，先完整验证闭合命令，再按非零绕数求交；二次曲线按y极值拆成单调段，用固定64次二分求交，不折线化，半开y区间避免共享顶点/切线重复计数。直线、水平二次曲线与端点另做边界检查；全局归一化避免大数乘积溢出，Boundary采用归一化坐标下64×f64::EPSILON，不是精确算术判定。命令数受max_path_segments限制，无额外容器。回归覆盖曲线内/外/边界、孔洞、同向重叠非零绕数、切线、水平回折、镜像及1e-200/1e200尺度、非法输入/序列和PDF字符显示轮廓边界查询；TrueType35项通过。仍不判断真实可见性、描边、裁剪、透明、遮挡、hinting、复合字形或可变实例，真实字体完整矩阵待验收。

轮廓实际边界（2026-10-05后续）：`glyf::outline_bounds` 计算线段/二次曲线的轴向极值（含内部导数零点），不采用控制点包围框；`TextObject::character_outline_display_bounds` 先转换曲线再求显示边界，不旋转已有AABB。空命令None；未闭合/缺Move/非有限命令拒绝，命令数受max_path_segments限制，计算无额外容器。回归验证控制点y=20但曲线最高y=10、横向极值、大数、非法序列、额度、实际PDF推进字符显示边界与空字形；TrueType33项通过。边界仍不是填充命中或painted/visible bounds，不含描边、裁剪、hinting、透明与遮挡。

显示轮廓入口（2026-10-05后续）：`TextObject::character_outline_display(index, limits)` 在绘制对象持有的真实字体与CharCode上查询简单轮廓，按每个字符位置、Tf/Tz和文字/页面矩阵转换为f64显示坐标；非有限或超出f32显示范围的坐标拒绝。保留Quadratic控制点，不借助run初始平移替代推进后的字符原点。回归覆盖重复非空字形不同原点、空字形、非法索引、剪切/Ts/Tz及严格保存重读。没有将轮廓复制进所有ExtractedCharacter，调用方从DisplayList的Text对象按需查询；没有裁剪、透明、描边、遮挡、hinting、复合或可变实例，尚不等于命中或可见性。

PDF字体轮廓入口（2026-10-05后续）：`Font::simple_glyph_outline(&CharCode, limits)` 按字体已有code/CID→GID映射选择字形，再从可用字体程序读取em轮廓，不以Unicode cmap或字符码代替GID。未提供数字映射/字体程序及不支持的程序或复合字形返回错误。回归构造Type0/Identity-H＋CIDToGIDMap，把PDF字符码65映射至GID0、66映射至空GID1，保存严格重读后结果相符。该四表SFNT只用于解析测试，不证明真实字体显示或完整文字命中；尚未将轮廓挂接到ExtractedCharacter，复合与hinting仍未实现。

SFNT轮廓入口（2026-10-05后续）：`TrueTypeProgram::checked_simple_glyph_outline(data, gid, limits)` 接入同源maxp/head/loca/glyf，按head.unitsPerEm将简单轮廓原地转换为f64 em坐标。检查unitsPerEm为16～16384，空loca范围返回空命令，越界GID、复合字形和CFF拒绝。自编四表SFNT覆盖16/1000/2048/16384的实际坐标归一化及非法值，TrueType32项通过。没有从PDF字符/编码选GID、解析复合轮廓、应用可变实例或执行hinting；字体解析、点存储和命令存储的操作级聚合峰值仍待统一计费。

简单字形轮廓（2026-10-05后续）：`SimpleGlyph::outline_commands(&limits)` 已将点转换为Move/Line/Quadratic/Close，处理首点离曲线时的最后曲线点或首尾隐含中点、连续离曲线中点和闭合二次曲线，多轮廓独立闭合。使用f64设计单位保留半整数中点，不做折线近似。调用时再次检查公开端点/点数据的一致性，命令上界和字节额度前置限制，可失败分配；TrueType31项回归通过。这补齐下段此前未实现的二次曲线路径生成，但尚未接入PDF字体/GID映射、em归一化、渲染或命中，也未实现复合展开、hinting或可变实例。

简单字形点解码（2026-10-05）：`GlyphLocations::simple_glyph(gid, &limits)` / `SimpleGlyph::try_new` 读取轮廓端点、借用指令字节、展开重复标志、解码x/y短增量或有符号16位增量，保留on-curve标记。端点必须严格递增，重复数不得超过点数，截断/保留标志拒绝；点数使用max_path_segments及max_objects限制，端点/临时标志/点存储累计使用max_decoded_bytes限制，可失败分配。零长度loca范围None，零轮廓可只有头或附带指令。复合字形明确不支持；没有执行hinting、展开复合依赖、生成二次曲线路径或验证声明边界包住实际点，不能作为完整字体/命中验收。TrueType组30项通过。

字形头读取（2026-10-05）：`GlyphLocations::glyph_header(gid)` 在已验证loca范围内读取glyf十字节公共头，返回轮廓数与有符号设计单位边界；零长度范围返回None，零轮廓头仍返回Some。`GlyphHeader::is_composite()` 按负轮廓数判断，不强制仅接受推荐的-1。截断和倒置边界拒绝，相等边界允许。依据[OpenType glyf规范](https://learn.microsoft.com/en-us/typography/opentype/spec/glyf)，这些是声明的控制点边界，不保证紧贴曲线、不覆盖任意可变实例；尚未验证点数据、复合依赖或hinting，不能直接宣称准确文字命中。TrueType组28项回归通过。

跨页复用：RegisteredCidFont 保留文档归属 ObjectHandle，通过 register_on_page 在同一文档其他页绑定已有 Type0 对象，返回该页资源名；已有 F1 时可返回 F2，重复注册复用原名。不重嵌入 FontFile2/后代/映射；编码器改为只读 encoding()，避免替换编码表后与已写入字体对象不一致。object_handle() 用于现有归属检查接口；跨文档拒绝且不修改目标文档，非法页拒绝。回归用两页保存严格读回，并确认 FontFile2/CIDFontType2 各仅一个。外层事务回滚后的新对象句柄不得继续使用；通过低层对象编辑替换字体或编码相关对象的调用方仍须自行维护编码一致性。此项不是跨文档字体导入或按字体字节自动去重，也不取代真实中文渲染验收。

整字体注册入口：ResourceManager::embed_truetype_font 接受字体字节、固定 repertoire 与 TrueTypeEmbeddingOptions，返回资源名及编码器。按 PDF 32000-1 表 117/118/122/126 构造 Type0/Identity-H、CIDFontType2、FontDescriptor/FontFile2、CIDToGIDMap、W 和 ToUnicode，六个间接对象及页资源绑定在事务中完成。FontFile2 保留完整源字节；至少 PDF 1.3；拒绝 CFF/可变字体、缺必要指标或缺字，并按可编辑文档检查 fsType，不允许预览打印专用字体进入该接口。调用方显式提供真实 PostScript 名称、1000 单位 CapHeight/StemV 与 serif 分类，不推测缺失指标；许可条款仍由调用方确认。注册不执行轮廓点/复合依赖/hinting 完整验证，不是 PDF/A 或真实字体安全/显示验收。自编 SFNT 元数据样本验证保存严格读回、字体对象关键字段、缺字/受限许可前置拒绝，以及对象额度不足后的对象内容和下一对象号回滚；该样本不是可渲染字体，真实中文显示与文本提取验收仍待完成。

实验性子集入口：`embed_truetype_subset_font(page_index, source, repertoire, options, subset_tag, omit_layout_and_auxiliary, budget)` 不改变整字体接口。调用方提供文档内唯一的六个大写字母 tag；基础名称非空、至多56字节，使用旧入口允许的字母/数字/`-_.`，前缀后的名称同步用于 name ID6、FontDescriptor 和 BaseFont。只支持静态独立 TrueType、固定 Unicode 标量字集、直接 CID 映射；保持原GID与numGlyphs，未选字形为空，包含GID0及所选复合依赖，重建glyf/loca/cmap/head/name。name版本0/1的版权、许可和语言标签保持。`omit_layout_and_auxiliary=true`明确允许省略已知布局表及LTSH/hdmx/VDMX/PCLT辅助表，不授权省略未知表；DSIG不保留，省略表通过`RegisteredSubsetCidFont::omitted_tables()`报告。变量/CFF/颜色等未支持依赖明确拒绝。原句柄的固定字集不会自动扩充，编辑新增字符必须另建字体资源；shaping、变体序列和自动子集tag分配尚未实现。

子集预算跟踪重建输出、临时闭包/表清单和存活省略报告；借用源、现有字体解析器普通分配及文档持久对象不在此输出池范围。字体资源注册复用六对象事务。组装、公共保存/跨页提取、注册中途失败回滚和显式单事件新增字体隔离专项已有通过证据；固定真实中文/复合字形独立字体工具和渲染也已通过。最新辅助表策略后的完整工作区回归仍进行中，自动覆盖扩充与通用跨事件/嵌套选区入口未完成，不能据此宣称P3-05完成。

字体描述信息前置：按 [OpenType post 规范](https://learn.microsoft.com/en-us/typography/opentype/spec/post) 增加 PostMetrics 公共 32 字节头读取，支持版本 1/2/2.5/3，保留有符号 16.16 italicAngle 的分数、下划线位置/厚度及非零 isFixedPitch 语义。TrueTypeProgram 加载可选 post_metrics；缺表仍为 None，出现不支持版本或头截断则报错。单元测试覆盖负分数、非 1 固定宽标志、全部头截断及版本，SFNT 样本验证挂接。只读取公共头，不验证版本 2/2.5 字形名称尾部、不调整可变实例，也不据此伪造完整 FontDescriptor 或 StemV/CapHeight。

CID 构建编码前置：UnicodeCidEncoding::from_text 从已读取的 Unicode cmap/maxp/水平指标建立固定字符集合，按 Unicode scalar 排序分配 1..65535 的两字节 CID，0 留给 .notdef；重复字符复用 CID，但不同字符即使映射到同一 GID 也保留不同 CID，避免提取时歧义。提供 encode_text、CIDToGIDMap、1000 单位 widths 和 ToUnicode CMap 输出，缺字或超出集合拒绝。ToUnicode 按 [PDF 32000-1 第 9.10.3 节](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf) 使用 UTF-16BE（补充平面为代理对，无 BOM）；bfchar 每组最多 100 项。字符数、条目数、构建缓冲和输出字节按 Limits 检查，分配可失败。此接口只建立映射，不代表已嵌入字体、检查使用许可、执行 shaping/连字/双向排版或完成独立阅读器验收；资源注册、FontDescriptor 与中文端到端仍待接入。

字形范围前置：依据 [loca 规范](https://learn.microsoft.com/en-us/typography/opentype/spec/loca) 和 [glyf 规范](https://learn.microsoft.com/en-us/typography/opentype/spec/glyf)，新增零拷贝 GlyphLocations，检查 numGlyphs+1 项、短偏移乘二/长偏移、非递减及 glyf 边界；空字形合法，越界 GID 拒绝。TrueTypeProgram::checked_glyph_locations 从同一源数据的 maxp/head/loca/glyf 构造视图，缺表和非法格式拒绝，OTTO/CFF 显式不支持这个入口。自编样本覆盖两种偏移、截断、倒序、越界、空字形和 SFNT 挂接；不表示轮廓点、复合依赖、hinting、校验和或真实字体渲染已经验证。视图借用原始数据，不复制 glyf 或展开偏移数组。

2026-10-04：为 P1-05 字体构建补充 cmap Format 12 支持。格式依据 [Microsoft OpenType cmap 规范](https://learn.microsoft.com/en-us/typography/opentype/spec/cmap#format-12-segmented-coverage)，不重新定义 PDF 字符编码。

实现范围：读取 32 位长度、分组数量及字符/GID 范围；分组排序、重叠、长度和 GID 算术检查后存储，二分查找而非展开整个字符范围。TCmap::get_gid_u32 接受完整字符码；旧 get_gid(u16) 保持兼容，不能表达补充平面字符且不会截断超出 u16 的 GID。TrueTypeProgram 对 Format 12 按 32 位声明长度计入已有累计 cmap 字节限额；字符表选择优先 Windows 3/10、Unicode 0/4，再考虑既有 BMP/其他回退，符号字体仍优先 Windows 符号表。

确定性回归使用项目自编字节表，不依赖外部字体：BMP A/B、U+20000–U+20002、范围外查询、全部截断位置、倒序/重叠范围、GID 加法溢出、超大组数和空表；完整 SFNT+cmap 读取及 Unicode/符号优先选择。现有 Format 0/4 回归继续执行。

maxp 前置已补：依据 [OpenType maxp 规范](https://learn.microsoft.com/en-us/typography/opentype/spec/maxp)读取 0.5/1.0 版本的 numGlyphs，检查对应最小表长度、版本及非零数量。缺表时旧简化读取继续兼容；构建用 `unicode_glyph_id(char)` 则要求 maxp 和 Unicode cmap，缺字返回 None，映射 GID 超出 numGlyphs 返回错误，不截断补充平面字符。该查询按 Unicode 表优先级选择，不受符号显示模式影响。

确定性回归补版本/长度/零数量，以及 U+20002→102 在 numGlyphs=103 时有效、numGlyphs=102 时拒绝、缺字和缺表。当前只交叉验证请求字符的映射；不会因此宣称整个字体的全部 cmap、loca/glyf 或 maxp 其他字段已经验证。

嵌入标志前置已补：`FontEmbeddingRights` 读取 OS/2 的公共前缀和 fsType，依据 [官方 fsType 规范](https://learn.microsoft.com/en-us/typography/opentype/spec/os2#fstype)保留原值并解释版本差异。0/1 忽略未定义高位，0–2 组合权限按较宽许可解释，3–5 检查互斥；检查保留位。`validate_outline_embedding(subset, editable_document)` 拒绝受限嵌入、位图限定的轮廓嵌入、禁止子集时的子集请求，以及仅预览/打印字体用于可编辑文档。解析受限字体本身不因此失败，读取与嵌入分别判断。

TrueTypeProgram 保留缺 OS/2 的旧读取行为，但新嵌入验证缺表即报错，不推断拥有许可。当前仅检查标志前缀，不代表完整 OS/2 metrics 或其他表有效；技术标志不能替代字体许可证，也不会修改源字体权限。尚未接入真正的字体注册/嵌入入口，预览打印的只读文档生命周期也未实现。

水平字宽前置已补：依据 [head](https://learn.microsoft.com/en-us/typography/opentype/spec/head)、[hhea](https://learn.microsoft.com/en-us/typography/opentype/spec/hhea) 和 [hmtx](https://learn.microsoft.com/en-us/typography/opentype/spec/hmtx) 规范读取 unitsPerEm、字体包围盒、hhea ascent/descent 与水平 advance。检查表长度、版本/魔数、保留字段、unitsPerEm 范围、metric count 和 maxp 数量；后续字形复用最后一个 advance，同时验证其侧边距数据实际存在。按 GID 查询会拒绝越界；缩放到 1000 单位使用 f64 保留分数，不把缩放值提前取整。

回归覆盖末尾字形复用、分数缩放、负 descent、包围盒、截断和非法单位/计数。TrueTypeProgram 在 head/hhea/hmtx 齐全时加载指标；缺表的旧读取仍兼容并返回 None，不构造虚假字宽。自编 SFNT 指标样本验证表加载与字宽查询，不是可渲染字体。旧测试的内置 TimesNewRoman 实际为 Type 1 PFB，现通过 SFNT 签名检查明确拒绝，不再把它计作 TrueType 验收。签名依据 [OpenType 文件规范](https://learn.microsoft.com/en-us/typography/opentype/spec/otff#organization-of-an-opentype-font)，接受 TrueType、OTTO 与旧 true 包装，不等于支持全部轮廓格式。真实字体 cmap→有效 GID→字宽及独立工具验收仍待补齐。当前不计算字形轮廓包围盒、不验证完整字体校验和，不应用 kerning、GSUB/GPOS、竖排或非默认可变字体实例调整；hhea 指标也不冒充完整 FontDescriptor。

尚未完成：实际字体许可确认与嵌入入口接入、完整轮廓/字形引用验证、PDF 字宽数组与完整描述符、CIDToGIDMap、ToUnicode、高层 Unicode 编码以及独立阅读器显示/提取验收。此子表能力不代表中文 PDF 构建闭环或 P1-05 已完成；Format 13/14、可变字体及 CFF 等仍为后续范围。fuzz 保持用户要求的暂缓状态。
