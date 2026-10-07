# rzpdf

`replace_page_form_draw(page, &snapshot, operation, &form_handle)` 用同文档Form替换顶层一次Do调用，事务注册页面局部资源名并复制变化内容流，保留重复调用和共享页面。外文档句柄与过期快照拒绝，跨文档Form须先导入；不修改嵌套叶内容或自动匹配替换Form尺寸/Matrix。实色路径Skia像素回归验证单次颜色/尺寸替换、邻接及共享页整图保持和严格保存重开。

`remove_page_form_draw(page, &snapshot, operation)` 删除顶层Contents中一次Form Do调用，保留共享Form对象、重复调用及其他页面；要求当前页面快照，拒绝图片Do和offline叶流快照。它不是嵌套Form内容编辑或安全擦除，已有事务回滚及严格保存重开回归。

`transform_page_form_draw(page, &snapshot, operation, matrix)` 在当前用户空间变换一次顶层Form调用，通过q/cm/Do/Q恢复后续状态，不修改Form自身Matrix或共享流。不是显示空间编辑；已有平移、旋转、非均匀缩放、反射与Form Matrix/父CTM组合、邻接绘制隔离及严格保存重开回归。Skia路径像素专项覆盖旋转/缩放/反射、BBox裁剪与删除，共享页整图保持；其他内容类型及独立阅读器验收仍待补齐。

路径参数编辑可用`replace_page_path_numbers(page, &snapshot, operation, &values)`，支持m/l/c/v/y/re并保留邻接操作；图片移动/缩放/旋转可用`transform_page_image_draw(page, &snapshot, operation, matrix)`，隔离单次Do并恢复后续图形状态。两者使用当前用户空间，不自动反算显示坐标或提供命中测试，仍需当前来源快照。图片平移与矩形编辑已有保存前后的Skia中心像素回归，完整复杂状态/独立阅读器验收未完成。

图片编辑已有两种入口：`resources().replace_image_handle(page, name, &handle)`替换目标页的整个图片名称绑定；`replace_page_image_draw(page, &snapshot, operation, &handle)`只替换顶层Contents中一次图片Do。使用同文档图片句柄，保持共享原图及其他页，后者保持原有绘制变换并事务注册资源/复制变化流。单次接口拒绝过期/错页快照，编辑回滚后须重新获取快照；不定位Form/内联图片，不自动缩放或安全擦除。详见 [内容编辑接口](docs/content-editing.md)。

保存已增加透明功能及已编码流最低版本检查：alpha/BM相关ExtGState、图像SMask和透明组需有效1.4或更高；保留流的Flate、JBIG2、JPX、Crypt也检查各自最低版本。不满足时明确拒绝，使用SaveOptions显式升级或有效Catalog覆盖，不自动更改源文档。此为定向检查，不是所有PDF功能版本/符合性审计，也不新增上述编码的解码支持。

Unicode 文档替换可用 `replace_page_text` / `replace_page_text_array_item`，按目标操作的有效Tf/q/Q/ExtGState Font绑定已有字体编码；未知前置操作、缺少字体或无效文字对象状态明确拒绝。ExtGState已接通Font、常规线型参数、ca/CA常量alpha及16种标准BM到Skia的映射，路径/文字/图片使用对应alpha/模式；RGB像素回归覆盖常见组合，不等于全部色彩管理或透明模型验收。图形软蒙版及透明组等未支持条目仍明确拒绝或报告诊断，不新增字体覆盖或补偿布局。

内容文字编辑支持 Tj、两类引号操作和 TJ 单项字符串替换，`replace_page_text_range` 按解码字符码边界提供局部 Unicode 替换/插入/删除；沿用目标有效字体，不自动补字体覆盖或补偿字宽。文档提交校验页面快照身份/修订号并采用事务和流复制写入。`encode_text_for_font` 可按显式字体资源的既有Unicode映射编码；复合字体要求已有ToUnicode/Encoding并验证字符码长度及读回。TJ 保留其他片段、显式间距与注释，不等于完整排版编辑或安全删除。详见 [内容编辑接口](docs/content-editing.md)。

`remove_page_text_draw(page, &snapshot, operation)` 清空整个顶层文字事件的字符串，支持Tj/TJ/两类引号，不依赖字体编码；保留引号状态与TJ数值/注释，批量TJ仅一次扫描和程序重解析。它取消原字形推进，不保证连续文本布局保持，也不清除旧内容对象中的残留数据。

顶层文字、图片及路径绘制带来源操作位置；文字字符另带 TJ 项索引和解码字符码范围。图片/文字报告及 DisplayList 可检查同文档/同页/同修订配对与当前性。`image_geometry_hits` 提供显示空间的图片几何候选，`transform_page_image_draw_display` 支持显示空间仿射编辑；不判断裁剪、透明和遮挡，也不覆盖 Form/内联来源或文字路径命中。

可运行 `cargo run -p pdf --example edit_content -- output/pdf/rzpdf-edit-content-example.pdf` 生成两页原稿/编辑稿对照（已有输出拒绝覆盖）；qpdf/MuPDF结构、提取及渲染验证记录见 [内容编辑样本验证](docs/content-edit-example-validation.md)。

生成后运行 `bash docs/check-content-edit-example.sh output/pdf/rzpdf-edit-content-example.pdf`，重复检查共享资源、页面结构、精确标题及绘制事件/图片矩阵（需qpdf、mutool、jq、python3）；它不是通用PDF符合性或视觉验收器。

`PdfPage::extract_characters_paint_order()` 返回绘制顺序下的字符码/码长、可选 Unicode、声明字号、BaseFont 名、字体书写模式、位置/矩阵和对象索引；位置为旋转/裁剪后的左上原点显示空间，矩阵为文字运行初始变换。ToUnicode 沿用已有 Cmap 映射支持，缺失映射不伪装成 U+FFFD；简单字体没有 ToUnicode 时沿用编码回退。BaseFont 名不是稳定字体身份，不推断阅读顺序，也未完成所有复杂映射或段落布局。字符数和复制的 Unicode/字体名总字节受限，解释结果与返回事件并存的整体峰值尚未聚合计费，P2-01 仍在实施。

只读提取示例：`cargo run -p pdf --example extract_text -- INPUT.pdf`，按文字运行分隔输出，页间保留分页符，缺失 Unicode 映射或内容诊断报错；不重建阅读顺序，后续页失败时前面页可能已输出。

推荐 `extract_characters_paint_order_report()` 保留字符和同次解析诊断；结果包含 Tz 单位因子，位置/矩阵包含 Ts 抬升。报告的 `single_column_reading_order_indices(tolerance, &limits)` 提供显式单栏、显示水平、从左到右空间排序，返回原绘制事件索引；不是多栏/bidi/Tagged PDF 语义阅读顺序。详见 [文本提取接口](docs/text-extraction.md)。

图片可用 `image_resource_names()` 发现有效页命名资源，`extract_image_source(name)` 保留原始流/编码字典和尺寸，`extract_decoded_image(name)` 返回规范化样本模型（`rgba_buffer()` 应用软遮罩）。`extract_images_paint_order_report()` 返回实际图片绘制及解析诊断，保留同图重复绘制与显示空间矩阵。原始提取允许未知编码，解码仍可失败；内联/Form 路径未完整支持。详见 [图片提取接口](docs/image-extraction.md)。

`PdfPage::extract_text_runs_paint_order()` 将字符事件按解释后的文字对象分组，输出 `ExtractedTextRun`（对象索引和字符数组）；字符只移动、不重复复制。文字运行不是行/段落或原始操作跨度，空文字对象不输出运行；不替代阅读顺序推断或 ContentProgram 来源映射。

A Rust library for **PDF data extraction** and **rendering**.

## Development plan

`PdfDocument::new_with_version(PdfVersion::V2_0)` / `Document::new_with_version(...)` 显式选择新文档头版本，`new_with_version_and_options(version, options)` 同时设置策略/限额；默认 new/new_with_options 仍为 1.7。只选择版本，不是 PDF/A 或 PDF 2.0 全功能符合性声明。

`target.merge_document(&source)` 原子追加源文档全部页；`source.extract_pages(&[3, 0])` 将选中页按给定顺序输出为新文档，保留源有效版本和 ParseOptions。二者基于批量导入，不合并/复制 Info、XMP、书签或命名目标；非空注释/标记结构等限制仍适用，PDF 2.0 选页保持有效版本，Catalog 升版成为输出头版本。合并仍要求目标版本不低于源，merge_documents 示例预先读取全部输入并按最高有效版本创建目标。可运行 `merge_documents OUTPUT.pdf INPUT1.pdf INPUT2.pdf` 或 `extract_pages OUTPUT.pdf INPUT.pdf 3 0` 示例（均通过 `cargo run -p pdf --example ... --locked -- ...`）；保存会替换指定现有普通输出文件。实际中英文/图片合并拆分验证见 [合并与选页验证](docs/merge-extract-validation.md)。

`target.import_pages_from(&source, &[2, 0], insertion_index)` 按指定顺序批量导入不重复的源页；同批次使用一份对象映射，共享字体/图片/内容只复制一次，选中页之间的对象引用会指向对应目标页。整批事务化，后续页失败会回滚已插入页和资源；重复/越界索引拒绝，合法位置的空集合不修改文档。同文档批量导入暂不支持，请使用 `copy_page`；其余注释/标记结构/版本/加密等限制与单页导入一致，未选中页面引用及源命名目标/书签树仍不自动导入。

`target.import_page_from(&source, source_index, insertion_index)` 导入一页及其可达内容/资源，返回目标文档页句柄；目标插入位置允许页数，源页与源对象不修改。固化源页继承资源/页面框/旋转，重映射共享和循环引用，事务内更新目标树及 Count；同文档调用转为 `copy_page`。目前拒绝非空注释、标记结构/文章归属、加密源和较低目标有效版本；不导入书签、页标签、结构树或其他页面，保留字体数据不意味着重新授予许可。源对象/容器克隆的整体峰值预算和完整页面关系导入仍待完善。

`target.import_resource_from(&source, &source_handle, ResourceKind::Font)` 导入字体（Image 同理），返回目标文档句柄，再用目标 `resources().register_handle` 绑定页资源名。每次导入重映射可达对象 ID，保持同次共享引用/循环、保留原编码流，不按不同调用自动去重。拒绝错误句柄/类型、较低目标有效版本、加密源、外部文件流及资源图指向 Page/Pages/Catalog；限额或引用失败会回滚目标写入。节点/深度、token、单流及累计载荷有限额，但源对象克隆/容器开销不是完整峰值内存预算。这不是完整页面导入或字体许可重新授权；外层事务回滚后的新句柄不得继续使用。

`copy_page(source_index, insertion_index)` 在同一文档复制页面，两个索引均按插入前页序解释，插入位置允许页数（追加）。新页有独立 ID，固化继承资源/页面框/旋转，共享原有内容流和资源对象；用页级内容替换/资源注册接口编辑可保持原页不变，低层修改共享对象则会影响两页。非空注释、StructParents 标记结构及文章 B 归属目前显式拒绝；不复制或重写这些关系，也不是跨文档导入。外层事务回滚后新返回的 ID 不得继续使用。

`set_page_rotation(index, degrees)` 设置绝对顺时针页旋转，`rotate_page(index, degrees)` 在当前有效角度（含继承值）上相对旋转；均要求 90 的倍数并归一化到 0/90/180/270。`set_page_crop_box(index, Some(rect))` 设置未旋转用户空间中的 CropBox，验证有限正面积及与 MediaBox 相交。传 `None` 会在当前页显式写入当时的有效 MediaBox，覆盖祖先裁剪（不是删除属性后恢复继承）。这些入口只修改该页并支持事务回滚，不变换内容、注释或其他页面框。

显示坐标以左上角为原点、Y向下。90°/270°页面矩阵已修正遗漏的Y翻转，旧版本这两个角度的提取/绘制坐标可能呈镜像；应重新解释页面，不复用旧DisplayList。路径重放同时区分纯填充与填充加描边，纯填充不再添加黑色轮廓。

`PdfPage::geometry()` 独立返回 `PageGeometry`：有效可见框（MediaBox与CropBox交集）、归一化旋转角度、显示宽高及用户空间到显示空间矩阵。不读取Contents、不加载字体；页面内容无法解释时仍可查询合法几何。查询使用最新继承/间接坐标值，不改文档修订号；返回值为快照，编辑后应重新查询。

`PageGeometry::display_to_user_point(&point)` 将显示坐标转回未旋转PDF用户空间，不截断到页框，也不是内容命中测试。底层 `Matrix::checked_inverse()` 使用f64中间计算，拒绝非有限、奇异矩阵或超出f32范围的逆矩阵；输入点同样需要有限。

`ExtractedCharacter::em_to_display_matrix()` 以当前字符的基线位置为原点，将局部em坐标映射到显示空间，包含字号、Tz、文字/页面变换和UserUnit；`display_to_em_point()` 提供逆转换，零水平缩放等奇异变换拒绝。这是文字命中测试的坐标基础，不提供字形轮廓、边界框、裁剪可见性或垂直字形指标推断。

页面 `UserUnit`（PDF 1.6）已纳入几何尺寸与显示矩阵，`PageGeometry.user_unit` 返回叶页值，缺失/null默认1，不沿父节点继承。`PageBuilder::new().user_unit(unit)` 可直接构建指定单位的页面；插入前检查有效版本与缩放几何，省略时不写字段。`set_page_user_unit(index, Some(unit))` 设置叶页单位比例，要求文档有效版本至少1.6、有限正数及有限正显示尺寸；`None` 删除叶页字段并恢复默认1。修改支持事务回滚，不改写内容、批注或页面框的源坐标。保存含非null UserUnit 的Page需要有效版本至少1.6，不自动升级。像素回归已覆盖非零裁剪框、四种旋转下整数对齐实色路径的2倍缩放、共享隔离、严格保存重读和清除恢复；另覆盖Helvetica文字、RGB图片和实色路径在0.5/1.5/2.5比例及72×72、72×144 DPI下与提高/降低DPI的逐像素等价。不代表任意比例、字体/图片类型或独立阅读器矩阵已验收。

`overlay_page_content(index, &ContentBuilder)` 在原内容之后添加覆盖内容，用独立图形状态隔离原页面的变换、裁剪及颜色，并清除遗留路径。原内容作用域不配对、遗留未执行裁剪或内联图片暂不支持时，在修改前拒绝；未知操作仍保持不透明，不代表完整语义验证。合并内容受字节、操作数及深度限制，页面内容替换不影响共享原流的其他页；字体/图片须另行注册。该接口保留原内容，不是安全涂黑；整体峰值内存预算及独立视觉验收仍待实现。

`overlay_page_text(index, document::page_overlay::TextOverlay { font_name, codes, font_size, matrix, color })` 为文字水印/页码提供定位入口；字体须先注册到该页，`codes` 必须由对应字体编码器产生，不能直接传中文 UTF-8。matrix 使用未旋转默认用户空间，可表达平移或旋转，color 是 0–1 RGB。检查有限矩阵、正字号及颜色范围后委托隔离覆盖接口；不自动居中、不计算显示旋转后的布局、不提供透明度或批量页码格式模板。调用方需保证字体名称与编码匹配。

文字覆盖还会解析页面有效继承资源，要求字体名称存在、字体对象为字典且 Type 为 Font；缺失资源在修改前拒绝。这不是完整字体合法性或字符码/字体匹配验证，调用方仍须使用对应编码器。

`overlay_pages_text(&[(page_index, TextOverlay), ...])` 按选择顺序原子地覆盖多页，每页可使用不同字体名称和编码后的页码；重复/越界页在写入前拒绝，空集合不修改。后续页失败会恢复之前页的内容与资源引用。字体注册若也需要整批回滚，应与此调用一起包在外层 `edit_transaction`；该接口尚未聚合计算所有事务快照/内容副本的峰值内存。

`PageNumberFormat { start: 1, prefix: "第 ", total_separator: Some(" 页 / 共 "), suffix: " 页" }.label(ordinal, selected_count, limits)` 生成 Unicode 页码标签，ordinal 从 0 开始，按选中页序计数；总数表示选择长度而非末尾编号。支持纯数字/前后缀/总页数，检查溢出与文本额度。标签仍需通过标准字体或嵌入 CID 字体的编码器转换成 codes，再调用覆盖接口；它不自动注册字体或修改 PDF。

`number_pages_standard(&pages, &format, StandardFont::Helvetica, font_size, matrix, color)` 将标签生成、WinAnsi 编码、逐页字体注册和覆盖组合在一个事务内；按 pages 顺序编号，后续页失败会回滚字体注册及之前页的覆盖。仅支持所选标准字体的可打印 WinAnsi 字符，中文标签明确拒绝；中文请使用嵌入 CID 字体编码和批量文字覆盖。定位为未旋转页面用户空间，不自动居中或适配旋转/CropBox。

`number_pages_cid(&pages, &format, &registered_cid_font, font_size, matrix, color)` 复用本文件已嵌入的 CID 字体添加中文页码；预先嵌入的固定字符集必须包含前后缀、数字和总数分隔符。检查字体句柄归属，按页注册同一字体对象并编码标签，任一步失败整体回滚，不重复嵌入字体文件。该新组合入口尚待专门中文样本验收。

`watermark_pages_standard(&pages, "DRAFT", StandardFont::Helvetica, font_size, matrix, color)` 在选中页添加同一文字水印，逐页字体注册与覆盖整体事务化；matrix 决定位置与旋转，浅色 RGB 不等于透明度。仅支持可打印 WinAnsi，不自动居中、不改变原内容，也不是安全涂黑；中文水印通过嵌入 CID 字体和批量覆盖实现。

`watermark_pages_cid(&pages, "草稿", &registered_cid_font, font_size, matrix, color)` 使用本文件已有 CID 字体编码水印并跨页注册，同一事务内覆盖，不重复嵌入字体文件。固定字符集须包含全部文字；缺字与外来句柄拒绝。此接口的真实中文渲染验收仍待完成。

`create_cjk` 示例在原有五个参数后可追加 `--overlay`，预先将水印及页码字符加入固定字符集，再为两页添加“草稿”和“第 1 页 / 共 2 页”中文覆盖。未传此选项时保留原来的正文示例行为；带选项路径已通过编译，实际外部验收待补。

组合示例：`cargo run -p pdf --example overlay_document -- INPUT.pdf OUTPUT.pdf`。同一外层事务为全部页面添加斜向 DRAFT 和 `Page 1 / N` 页脚；先严格读回内存保存结果并解释全部页，再保存目标文件。示例使用固定位置，适合默认用户空间的 A4 页面演示，不自动处理旋转/裁剪/小尺寸页面；请使用与输入不同的新输出文件名，独立视觉验收仍待完成。

`document.reorder_pages(&[2, 0, 1])?` 按当前 0 起始页索引的完整排列重排三页；重复、缺失、越界索引拒绝，恒等排列不修改 revision。保留页面对象 ID、页面树槽位及 Count；跨父节点先固化继承资源/MediaBox/CropBox/Rotate，整个修改事务化，不删除共享对象。尚不自动调整页标签、书签的页序语义或结构树，也不等同于页面复制/跨文档导入。

TrueType cmap 已增加 Format 12 与 `get_gid_u32`；整字体嵌入可用 `resources().embed_truetype_font`，生成 CIDFontType2/Identity-H、CIDToGIDMap、字宽与 ToUnicode。返回字体的 `encoding().encode_text` 产生两字节字符码，`register_on_page` 可跨页复用。要求有嵌入许可的静态 TrueType、固定字符集合和显式描述指标；尚不支持子集、CFF、可变实例或 shaping。真实 Noto Sans SC 中英文创建/显示/提取验证见 [中文构建验证](docs/cjk-creation-validation.md)，其他边界见 [字符映射验证](docs/font-cmap-validation.md)。

`PdfStream::data_state()` 明确区分 RawEncoded（按字典编码的原始载荷，无 Filter 即恒等编码）与 Decoded（显式替换后的解码字节）。`from_decoded_data`/`with_decoded_data` 清理 Filter/DecodeParms/DL 并重建 Length；新建页面内容与 Metadata 共用该入口。保存重新读取后属于 RawEncoded，状态不写入 PDF；对象相等仍比较字典和字节，不比较来源标记。直接页面追加/替换及创建检查单流原始/解码字节限额，不代表多个流或事务克隆的聚合峰值已受限。

新增无 Reader 泛型入口 `pdf::document::Document`：`new`、`open`、`from_bytes`、`from_reader` 返回同一种类型，各有 mode/options 变体，可直接使用既有页面、资源、事务与保存服务。自定义 Reader 需满足 `Read + Seek + 'static`；借用 Reader 继续使用兼容的 `PdfDocument<R>`。新建不构造空 Reader 或解析伪造 PDF。当前通过 Deref 委托旧服务，部分底层返回类型仍暴露擦除后的 Reader，并非全部 API 已消除泛型。

DeviceGray/RGB/CMYK 的 PDF 图像以及同尺寸 DeviceGray SMask 支持 1/2/4/8/16 位采样：处理每行填充位、16 位大端样本，并在原始精度上应用 Decode 后量化为显示用 8 位。输入字节长度和输出大小受检查。此能力不改变 PNG 导入的 16 位拒绝策略，也不扩展其他颜色空间；新增位深的独立阅读器像素对照仍待验收。

DeviceGray/RGB/CMYK 图像和灰度 SMask 已支持显式 Decode 数组，包括默认和反转映射；验证分量数量、有限数值，并按设备范围裁剪。其他颜色空间的 Decode、Matte 与嵌套蒙版仍不支持；此能力尚未完成全部独立像素对照。

`document.resources().embed_png_image(page, encoded)` 使用 png 0.18.1 解码 PNG，展开调色板/透明信息并复用 RGB/RGBA 注册；RGB、RGBA、灰度、灰度 alpha 和调色板+tRNS 已有保存/解释回归。检查原始文件字节、像素、转换输出大小及解码内部限额；校验 CRC 和结束数据，无效页号在解码前拒绝。APNG、16 位和 ICC 转换暂不支持，gamma/色彩管理未实现；多缓冲聚合峰值不在单项限额内。`create_transparency` 示例增加 `--png` 模式，基础 RGBA 导入已通过 MuPDF 中心像素对照，其他变体仍需独立验收。

`document.resources().embed_jpeg_image(page, encoded)` 可原样嵌入 8 位灰度或 RGB/YCbCr JPEG，使用 DCTDecode，不重新编码。检查 JPEG 标记/头部及压缩字节、像素、RGBA 预算；不完整解码验证压缩数据，不处理 EXIF 旋转、ICC 或 CMYK/YCCK 反转策略，后者显式拒绝。输入损坏的熵编码仍可能在显示时失败；文件读取和图像显示由调用方另行处理。

透明图片示例可用 `cargo run --example create_transparency --locked -- output/pdf/rzpdf-transparency-example.pdf` 生成；同尺寸 8 位 SMask 已通过 Skia 回归及 MuPDF 独立中心像素检查（每通道容差 1）。复现命令和支持边界见 [透明图片验证](docs/transparency-example-validation.md)。

`document.resources().embed_rgba_image(page, width, height, pixels)` 可嵌入交错 8 位、非预乘 RGBA，生成独立 DeviceGray SMask 并事务注册 RGB 图像；精确输入长度为 width×height×4，受文档像素/字节限额约束，要求有效 PDF 版本≥1.4。本库已解析同尺寸、8 位 DeviceGray SMask 并合并 alpha，Skia 使用 Unpremul 缓冲格式；零/半/全透明白底像素已覆盖保存前后及 MuPDF 独立检查。不同尺寸重采样、Matte、嵌套蒙版等仍显式拒绝；不代表完整软蒙版实现。

从零创建两页 A4、英文正文、RGB 图片、中文 Info 标题和原始 XMP packet 的示例：

```bash
cargo run --example create_document --locked -- output/pdf/rzpdf-creation-example.pdf
```

先创建目标目录，也可传入其他文件路径；现有普通文件会按保存策略替换。可用 `bash docs/check-creation-example.sh output/pdf/rzpdf-creation-example.pdf` 独立检查结构、中文 Info、Metadata XML/作者和两页文本（需 qpdf、mutool、jq、xmllint）。示例不嵌入中文正文字体，不代表完整 P1 验收。独立检查记录见 [构建示例验证](docs/creation-example-validation.md)。

中英文两页及嵌入字体跨页复用示例（字体由调用方提供，指标为 1000 单位）：

```sh
cargo run -p pdf --example create_cjk --locked -- FONT.ttf POSTSCRIPT_NAME CAP_HEIGHT STEM_V output/pdf/rzpdf-cjk-example.pdf
```

字体源许可、静态实例准备、参数与独立验证方法见 [中文构建验证](docs/cjk-creation-validation.md)。示例保存后还会严格读回并构建两页 DisplayList；可运行 `bash docs/check-cjk-example.sh output/pdf/rzpdf-cjk-example.pdf` 检查结构、字体跨页复用及精确提取文本（需 qpdf/mutool/jq）。这不是所有字体或排版情况的验收，视觉检查仍需单独进行。

Info/XMP 默认独立编辑，不自动修改另一侧。`set_info_and_metadata(Some(info), Some(packet))` 可在同一事务替换两者；任一失败回滚全部修改，None 解除对应引用。该 API 只保证原子提交，语义一致性和合法 XML 由调用方负责，不自动映射字段或覆盖未知 XMP 属性。

`metadata_packet()` 读取文档 Catalog 的 Metadata 流并返回解码字节；`set_metadata_packet(Some(bytes))` 注册新的 Type Metadata/Subtype XML 流，None 解除引用。写入要求有效版本至少 1.4，原始/解码字节受文档 Limits 限制；支持事务回滚。接口不解析 XML、不验证 XMP，也不自动同步 Info，调用方负责提供合法 packet；保守保留旧对象，不用于敏感数据彻底清除。

`InfoTextField` 位于 `pdf::document::metadata`，覆盖 Title、Author、Subject、Keywords、Creator、Producer。`info_text(field)` 解码字段，`set_info_text(field, Some(text))` 使用 UTF-16BE 设置，传入 None 删除单字段并保留其他条目。接口使用文档 Limits；UTF-8 BOM 读取要求有效版本为 2.0。日期/Trapped 和 XMP 同步仍待实现。

`document.effective_version()` 返回源 Header 与最新 Catalog /Version 中较高的版本，非法类型或未知版本返回错误；`version()` 仍仅返回源 Header。保存与资源写入复用有效版本判断；此接口不保证对应版本的符合性。

`document.info_dict()` 读取最新文档信息字典，`set_info_dict(dict)` 替换，`remove_info_dict()` 解除引用；写入纳入文档事务，错误和 unwind 回滚 Trailer 与对象分配。文本值建议使用 `PdfString::from_text`。这是低层字典接口，不校验全部 Info 字段/日期，也不自动同步 XMP；替换保守保留旧对象，不用于敏感信息彻底清除。低层 `PDFContext::trailer()` 现在返回借用守卫，调用方需注意借用生命周期。

`PdfString::from_text` 将元数据/书签等 PDF 文本编码为带 BOM 的 UTF-16BE；`text_content` 解码 PDFDocEncoding、UTF-16BE 或带 BOM 的 UTF-8（后者要求调用方确认 PDF 2.0）。两者都有 Limits 版本；它们不是页面字体编码器。旧 `get_content` 仍只按原始 UTF-8 解码。Info/XMP 高层接口尚未完成。

可用 `PdfDocument::new()?` 从零创建 PDF 1.7 文档，再调用 `add_page`、内容编辑和 `save_to_bytes`。该入口不解析模板或构造输入 Reader；高层字体/图片资源管理仍未完成。`new_with_options` 可设置解析策略及资源限制。

`add_page_with_builder(index, PageBuilder::new().size(width, height))` 可配置页面；PageBuilder 位于 `pdf::page::page_builder`，还支持 MediaBox/CropBox、旋转、原始 Resources 字典和初始内容。默认尺寸为 612×792 点（Letter，与既有 add_page 一致）；页面索引从 0 开始。资源字典不自动注册字体/图片，原始内容也不自动校验操作符配对。

`document.resources().register_existing(page_index, ResourceKind::Font, object_id)` 可将同文档已有字体对象注册到页面，返回供内容流使用的名称；Image 类型注册到 XObject。ResourceKind 位于 `pdf::document::resource_manager`。重复 ID 复用现有名称，新对象避开名称冲突；当前仅检查基本对象类型，不嵌入文件、不校验完整字体/图像结构，也不验证对象 ID 的跨文档来源。

`document.resources().embed_rgb_image(page_index, width, height, pixels)` 可嵌入交错排列的 8 位 RGB 原始像素，并返回图像资源名称。数据长度必须为 width×height×3；尺寸和字节受 Limits 约束，注册失败回滚新增对象。当前不压缩、不去重，不读取 PNG/JPEG 文件，也不处理 alpha 通道；内容流仍需使用返回名称执行 Do。

`ContentBuilder::draw_image(name, [a, b, c, d, e, f])?` 用矩阵将图像单位方框映射到页面坐标，生成配对的 q/cm/Do/Q，转义资源名并拒绝非有限数值。`draw_image_with_limits` 可限制名称和累计内容字节；失败不追加部分内容。该接口不验证资源是否注册，旧 builder 方法也尚未统一状态校验。

`ContentBuilder::try_build()` 在生成结果前检查语法、已知操作符固定参数数量、q/Q 与 BT/ET 配对及部分文字操作作用域；带 limits 版本还检查内容大小、操作数量和状态深度。文档的 replace/append_page_content_builder 已自动使用此校验，可能拒绝此前允许的未知 raw_operator 或未闭合片段。旧 build() 仍不校验；此校验不是完整 PDF 内容验证，也不保证文字编码或资源正确。

`ContentBuilder::show_encoded_text(&codes)?` 用十六进制字符串写入已按当前字体编码的字符码，可保留二进制和多字节 CID 码；带 limits 版本限制输出 token 和累计内容大小。它不把 Unicode 转为字体字符码。旧 `show_text(&str)` 只是原样写入 UTF-8 字节，不能保证中文或任意 Unicode 正确显示；中文静态 TrueType 可使用已实现的嵌入字体编码器，复杂字体/shaping/子集化仍未完成。

新内容建议使用 `try_set_font(name, size)?`：它转义字体资源名、拒绝非有限字号并支持限额版本；旧 `set_font` 不转义名称。两者都不会验证资源是否已注册。

`document.resources().register_standard_font(page_index, StandardFont::Helvetica)?` 可创建标准 Type1 字体并返回资源名；还支持 TimesRoman/Courier。字体不嵌入，编码为 WinAnsi；同页已有完全相同字典会复用。使用字体的 WinAnsi 编码器后传给 `show_encoded_text`；此标准字体入口不支持中文，中文使用静态 TrueType 嵌入入口，也不保证 PDF/A 符合性。

标准字体可用 `StandardFont::Helvetica.encode_text("Café €")?` 将可表示的 Unicode 转为 WinAnsi 字符码，再传给 `show_encoded_text`。无法表示的字符与控制字符返回错误，不替换成问号；不做 Unicode 归一化，也不支持中文。

颜色空间对象可通过 `document.resources().register_handle(page_index, ResourceKind::ColorSpace, &handle)?` 注册，返回如 `CS1` 的页内名称，用于内容中的 `cs/CS`；名称冲突会避让，同一对象已有绑定会复用。注册校验颜色空间解析，不保证所有颜色空间都可转换/渲染。`import_resource_from(&source, &handle, ResourceKind::ColorSpace)` 可先复制跨文档颜色空间对象图，再注册返回的目标文档句柄；不要直接注册外文档句柄。新增枚举分支可能需要下游穷举match同步增加ColorSpace处理。

也可直接调用 `document.resources().register_color_space(page_index, color_space_object)?`，原子创建/注册对象并复用页内相等值；间接根会先在当前文档解析。比较是PDF对象值相等，不进行不同对象图的语义等价推断。内部裸引用必须属于当前文档，跨文档请使用上述导入与句柄注册路径。

`encode_text_with_limits(text, limits)` 在分配前限制字符数和编码输出字节；`encode_text` 使用默认 Limits。该额度按编码后的一字节字符计，不按输入 UTF-8 字节计。

`document.save_to_writer(&mut output)?` 支持任意实现 Write 的输出：先完整序列化，再处理短写直至完成，不主动 flush/close。I/O 失败可能留下部分输出；当前仍在内存构造完整 PDF，不是流式节省内存的保存。

`save_to_file` 现在先写同目录独占临时文件，sync_all 成功后重命名替换，保留既有普通文件权限；拒绝符号链接和目录目标。失败时尝试清理临时文件，不承诺断电后的目录持久性、ACL/所有权保留或并发修改协调；替换依赖平台 rename 语义，当前验收平台为 macOS。

SaveOptions 位于 `pdf::document::save_options`；`save_to_bytes/writer/file_with_options` 可选择输出头版本。默认保留源 Header 与最新 Catalog /Version；显式输出版本不得低于两者中较高的有效版本，降级返回 UnsupportedFeature。保存前校验 Catalog 版本，非法类型/未知版本返回错误，不写出部分 Writer 内容。设置 2.0 头不代表 PDF 2.0 全功能或符合性验证。加密与增量模式尚未接入选项。

`compression: StreamCompression::FlateUnencoded` 可压缩未编码的间接流，已有 Filter 的流保留原字节；默认 Preserve。不修改内存中的文档对象，writer 更新 Filter/Length；未过滤流带非空 DecodeParms 时拒绝。Flate 输出的有效版本需至少 PDF 1.2，计入最新 Catalog 覆盖或显式 Header 升级。输入按原始/解码字节限额检查，压缩结果缓冲按 max_raw_stream_bytes 限制；压缩器内部开销及共存缓冲聚合峰值仍未预算，不提供压缩级别或预测器选项。

`SaveOptions::max_output_bytes` 限制最终 PDF 长度，默认 512 MiB。对象、xref、Trailer 与结尾逐项追加前检查剩余额度；叶子 token 分配前按转义后长度检查，流原始数据复制前检查。超限返回 ResourceLimit，Writer/文件入口尚未写入调用方输出。对象/字典克隆和多个缓冲共存仍未统一计费，故该限额不等于整个保存过程的峰值内存预算。

项目目标是逐步实现 PDF 构建、解析、读写、编辑和渲染。当前能力及实施任务见 [TODO.md](TODO.md)，架构调整与演进方案见 [docs/architecture.md](docs/architecture.md)。已有接口不代表完整兼容性，具体支持范围以阶段验收结果为准。

内容解释的兼容模式会跳过未知或未实现的操作符，并通过 `DisplayList::diagnostics()` 返回诊断；跳过可能导致显示差异。严格模式返回错误。诊断字节偏移相对于拼接后的页面内容。尚未实现的内联图像在两个模式下均返回明确错误。

读取可通过 `ParseOptions` 设置 `Limits`；Skia 渲染可用 `SkiaRender::with_limits(xdpi, ydpi, limits)` 复用限制配置。页面表面由 `max_render_pixels` 和 `max_render_bytes` 限制，与嵌入图像的 `max_image_pixels` 分开；这些限额不等于整个进程的峰值内存预算。

> **Note:** This library is currently under active development. Several features, such as Pattern colorspaces and Type3 fonts, are not yet implemented.

PDF Type 3 **Stitching functions**（不同于 Type3 字体）已支持单输入分段求值、Domain 裁剪、正向/反向 Encode、可选 Range 输出裁剪及嵌套函数；构造时校验子函数输入/输出维度，并保留深度和总节点限额。Type 4 PostScript 函数求值仍明确返回不支持。此功能不代表 Shading/Pattern 或操作级聚合内存预算已经完成。

Type 4 构造支持间接 Domain/Range 及数组内间接数值，先检查非空成对、有限有序参数和每数组条目/字节上限，再解码函数流；构造成功不代表代码已解析或可执行。数组检查仍是单项 Limits，不是两个参数数组与流解码的聚合内存预算。

Lab 已支持零 BlackPoint 的颜色转换：L/a/b 按定义域裁剪，经 XYZ 与 Bradford 白点适配到 D65，再使用 sRGB 传递函数和通道裁剪。非零 BlackPoint 补偿仍返回 UnsupportedFeature；不能适配的白点返回 ColorError。当前是固定 sRGB 显示策略，不包含 OutputIntent/ICC 渲染意图或感知色域映射，也未统一 CalRGB/CalGray 的现有转换策略。

---

## 🚀 Features

- **Data Extraction:** Efficiently traverse and extract page objects and document metadata.
- **Rendering:** Robust rendering support powered by `skia-safe` (Rust bindings for the Skia Graphics Engine).
- **Rust-Native:** Designed for safety, speed, and easy integration into the Rust ecosystem.

---

## 🛠 Examples

You can explore the library's capabilities by running the provided examples in the repository.

### 🔍 Document Tracing

Traverse the PDF structure and retrieve all page objects:

```bash
cargo run --example document
```
