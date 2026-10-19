# PDF 核心跨版本差距审计

2026-10-19，代码基线为 v0.14 提交 `64be233`，任务整理基线为 `613044b`。结论是：创建、解析、提取、编辑、渲染及安全保存的主要模块已有实现和分期验收；下一阶段应补明确的兼容性缺口、稳定接口与工程验证，不再按旧 TODO 从头开发这些模块。

这是一轮按已交付版本边界进行的源码与测试索引审计，不是对所有 PDF 特性、每条输入路径或所有字体的穷尽证明。表中“缺口”与“失败缺陷”不同：一个合法场景被明确拒绝，可能正是当前支持契约。下面给出源码入口、已有断言或最小补验配方；未执行的配方不记为通过或新发现的运行失败。

## 已有能力不能再列为未开发

| 已交付模块 | 当前实现或断言 | 后续应如何描述 |
|---|---|---|
| 新建与资源 | [PageBuilder 测试](../pdf/tests/create_document.rs)、[资源测试](../pdf/tests/resources.rs)、[PNG](../pdf/tests/png_embedding.rs)、[JPEG](../pdf/tests/jpeg_embedding.rs) | 扩大输入兼容，不是缺少构建器或资源管理器 |
| 文字与来源编辑 | [ContentProgram](../pdf/tests/content_program.rs)、[跨事件范围](../pdf/tests/text_event_range.rs)、[嵌套编辑](../pdf/tests/nested_text_edit.rs)、[子集](../pdf/tests/v03_font_edit.rs) | 不能把旧跨事件／嵌套／新增字符待办直接判为仍未实现 |
| 页面与对象图 | [关系迁移](../pdf/tests/v09_page_relationships.rs)、[结构迁移](../pdf/tests/v010_structure_transfer.rs)、[AF 页面操作](../pdf/tests/v014_associated_files.rs) | 明确哪些关系不能迁移，不能称没有导入／复制能力 |
| 图像／字体／图形 | [内联图像](../pdf/tests/v06_inline.rs)、[解码器](../pdf/tests/v06_codecs.rs)、[字体映射](../pdf/tests/v07_font_mapping.rs)、[Pattern](../pdf/tests/v08_pattern.rs)、[Type7](../pdf/tests/v08_tensor.rs) | 按编码、字体种类和图形组合列限制 |
| 批注与表单 | [批注](../pdf/tests/v09_annotations.rs)、[字段类型](../pdf/tests/v09_form_types.rs) | Unicode 字段已有，不能混同为 FreeText Unicode 也已支持 |
| 保存、安全与恢复 | [ObjStm](../pdf/tests/v012_object_streams.rs)、[加密增量](../pdf/tests/v011_encrypted_incremental.rs)、[受控更新](../pdf/tests/v011_controlled_updates.rs)、[恢复](../pdf/tests/v012_recovery.rs) | 区分 R6 和旧 handler、受控修改和任意签后编辑、只读恢复和修复写回 |

## 当前功能缺口

“未实现”描述能力，“明确拒绝”描述入口行为，两者可以同时成立。优先级按对现有 PDF 核心工作流的潜在影响排序，不代表已测得用户文件中的出现频率，也不把所有项目都设为 v1.0 必选项。

| 编号 | 缺口与当前行为 | 源码与已有证据 | 处置建议 |
|---|---|---|---|
| F01 | CID-keyed CFF 的 CID→GID 不支持；`CidFont::glyph_id` 对非 CIDFontType2 返回 None | [cid_font.rs](../pdf/src/font/cid_font.rs) 的 `glyph_id`；[type0.rs](../pdf/src/font/type0.rs) 的 `get_glyph`；[v0.7 矩阵](releases/v0.7-font-matrix.md) | 高优先核心候选。区分 PDF 码→CID、CFF charset→GID 和 Unicode，不用 Unicode cmap 代替 CID 映射 |
| F02 | Type1 缺显式 Encoding／ToUnicode 时，内建编码及 Unicode 回退仍有限；纯 Rust `Type1Program` 是拒绝入口 | [simple_font.rs](../pdf/src/font/simple_font.rs) 的 `load_encoding_inner`／`unicode_mapping`、[type1_program.rs](../pdf/src/font/type1_program.rs)；v0.7 登记 UnitaryMethod 缺失 234/296 映射 | 高优先兼容候选，但旧文件数字只是历史证据，需同 hash 重现。Type1 原生读绘已存在，不要求先写完整 Type1 解析器或子集器 |
| F03 | 文字裁剪走 TrueType 轮廓链，CFF／Type1／Type3 裁剪不在支持范围 | [text_clip.rs](../pdf/src/page/content_interpreter/text_clip.rs)、[pdf_font.rs](../pdf/src/font/pdf_font.rs) 的 `mapped_glyph_outline`、[true_type_program.rs](../pdf/src/font/truetype/true_type_program.rs)、[v0.8 矩阵](releases/v0.8-support.md) | 独立轮廓候选；不得用字框替代真实 clip；不混入 F01 的字符映射交付 |
| F04 | FreeText 新建及类型化编辑仅 ASCII Courier；AcroForm 的中文外观不自动扩展它 | [annotations/model.rs](../pdf/src/document/annotations/model.rs) 的 `Kind::FreeText`、[annotations/edit.rs](../pdf/src/document/annotations/edit.rs) 的 DA 限制；[v0.9 矩阵](releases/v0.9-support.md) | 中优先候选，可复用现有字段字体／外观机制，但需单独限定字体、显式换行、溢出及旧 AP 保留 |
| F05 | Info 与 XMP 只有原始／字段接口和原子配对更新，没有 XMP schema 解析、字段同步或日期语义验证 | [metadata.rs](../pdf/src/document/metadata.rs) 的 `set_info_and_metadata`／`set_metadata_packet`／`set_info_dict`；[metadata 测试](../pdf/tests/metadata.rs) | 中优先核心候选。必须显式选择冲突策略并保留未知 namespace；不能把原子写两边称为已同步 |
| F06 | 目录有 OCProperties 时页面复制／删除／导入仍拒绝，图层迁移不是 v0.13 已交付能力 | [page_transfer.rs](../pdf/src/document/page_transfer.rs) 的 `page_relationship_guard`；[结构迁移测试](../pdf/tests/v010_structure_transfer.rs) 的 `optional_content_configuration_and_orphan_ocg_import_do_not_silently_drop_visibility` | 独立关系迁移候选。需要组身份／配置／Properties／OCMD 一致性，不能简单删 guard |
| F07 | AF owner 只开放 Catalog／page；不开放 StructElem、内容序列、任意字典 owner 或外部载荷访问 | [associated_files.rs](../pdf/src/document/associated_files.rs) 的 `associated_file_owner`／写保护、[v0.14 边界](releases/v0.14-boundaries.md) | 当前明确范围限制，非 v0.14 遗漏项。按具体业务文件选择扩展；不自动下载外部文件 |
| F08 | Standard handler 仅 V=5／R=6；旧 R2～5、公钥等不支持 | [security/r6.rs](../pdf/src/security/r6.rs) 的 `try_new`、[security 测试](../pdf/tests/v05_security.rs) 的 `malformed_handler_filters_and_permissions_are_not_reported_as_wrong_password` | 兼容候选但不优先扩大全部密码算法。当前 UnsupportedSecurity 不能改报错误密码 |
| F09 | 恢复器只处理限定简单来源，不恢复 ObjStm／多 revision／加密／签名，不提供写回 | [recovery.rs](../pdf/src/document/recovery.rs)、[恢复负例](../pdf/tests/v012_recovery.rs) 的 `recovery_refuses_ambiguous_ownership_security_and_history` | 保留只读隔离。复杂恢复要独立设计来源与歧义规则，不让 Compatible 充当自动修复 |
| F10 | 通用可达性清理与安全脱敏未作为公共能力交付；解除链接和增量保存均不擦除原文 | [save_options.rs](../pdf/src/document/save_options.rs) 的 `IncrementalSaveOptions`、[保留测试](../pdf/tests/v012_preservation.rs) 的 `all_full_layouts_preserve_unknown_graph_trailer_af_xmp_and_unreachable_objects` | 可选大专题，不作为近期默认工作。垃圾回收不等于安全脱敏；两者不能共用一个“删除完成”承诺 |

F01／F02 是字体读取兼容，F03 是轮廓解释，F04 是批注外观，不能合并成“支持 CFF／中文”一个无法验收的大任务。非 RGB 透明组、overprint、非零 Lab BlackPoint、AIS／TK 等额外组合仍按 [v0.8 矩阵](releases/v0.8-support.md)保留限制，其中专业印刷按既定决定暂缓。

## 最小复现及补验入口

以下为补验配方，不是本轮新执行结果。已有测试命名是精确定位依据；运行时应用 `--exact`，过滤结果不能作为完整工作区计数。

| 编号 | 最小场景或已有断言 | 补验退出条件 |
|---|---|---|
| F01 | 合成一个自有 CID-keyed CFF，令 CID 不等于 GID；PDF Encoding 映射码到该 CID，ToUnicode 故意独立。当前 `glyph_id` 非 Type2 分支是静态缺口证据；现有 geometry.cff 不因存在就算 CID-keyed 正例 | 至少两种独立原始输入、GID／轮廓／W 或 W2／文本／保存重开及原生绘制对照；缺映射和坏 charset 明确错误，预算失败释放 |
| F02 | 按 [v0.7 字体矩阵](releases/v0.7-font-matrix.md)重取同 hash UnitaryMethod；另造可分发的自有 Type1 内建 Encoding 样本，不依赖显式 ToUnicode | 原始字节、宽度和绘制不变；已知字符恢复，未知字符仍报告缺映射；不从像素推断文字 |
| F03 | 相同字形分别以 Tr=0 与 Tr=7 在 ET 后裁剪色块，字体选 name-keyed CFF／Type1；当前 TT 正例不能证明该组合 | 多轮廓／洞／矩阵／负字号和共享页测试；裁剪几何来自原字体，不做 Unicode 重映射 |
| F04 | `Kind::FreeText` 的文本改为“中文”，与同字体 AcroForm 字段对照 | 创建、编辑、重开后 AP 与提取一致；字体缺字、低版本、矩形溢出拒绝且回滚；旧自定义外观不被无声替换 |
| F05 | 设置 XMP dc:title=A，随后 `set_info_text(Title, B)`。按当前 API，packet 保持 A 是设计行为；`paired_info_metadata_update_is_atomic_on_second_step_failure` 仅证明原子性 | 新同步入口必须显式选择来源／冲突，更新后两边一致且保留私有节点；实体／深度／字节限额和失败原子性单列 |
| F06 | 复用 `optional_content_configuration_and_orphan_ocg_import_do_not_silently_drop_visibility` | 从当前原子拒绝扩展为可验证的选定配置迁移；导入前后显隐／像素正确，不污染目标配置 |
| F08 | 复用 `malformed_handler_filters_and_permissions_are_not_reported_as_wrong_password` 的 R=5 分支 | 若另行开放旧 handler，须有真实算法产生的独立输入，不能只把 R=6 字典改名当作正例 |
| F09／F10 | 复用恢复拒绝和不可达对象保留测试 | 当前应继续拒绝恢复写回、继续保留普通未知对象；不能为消除 TODO 而逆转安全契约 |

## 工程和验收缺口

| 编号 | 当前证据 | 还需要什么 | 是否必须新增功能 |
|---|---|---|---|
| Q01 | README／pdf Cargo 声明 Rust 1.92；本机实际 rustc／cargo 为 1.99.1，1.92 不在已安装工具链中 | 明确 workspace 与各 crate 的 MSRV；在所声明最小版本验证核心与 renderer。离线 metadata 的 145 个解析 package 未发现声明高于 1.92，不等于 1.92 编译通过 | 否，先验证或收窄承诺；本轮不安装工具链 |
| Q02 | 最终证据只覆盖 macOS 13.7.8 x86_64；仓库有 Ubuntu CI 配置 | Linux／Windows／arm64 各自构建、原生库、文件替换及图像／字体验证。工作流配置不是运行证据 | 否，平台选择先确认；不自动开启远端 CI |
| Q03 | `Document` 通过 Deref 暴露 `PdfDocument`，公开低层模块很多；预算和 source 使用 Rc，ctx 使用 RefCell | 列稳定 facade／低层实验 API，约定 Send／Sync、错误分类、stamp／handle、公开结构字段及版本迁移。不要默认可跨线程共享 | 否，先形成 NEXT-02 契约 |
| Q04 | [AF 基线](releases/v0.14-performance.md) 1000 条逐项编辑约 64.85 秒；[图层基线](releases/v0.13-performance.md) 1000 组创建约 10.44 秒、配置／追加约 10.79 秒，均 debug／并发观察 | 同机隔离 debug／release，分开启动／构造／快照／校验／保存，再选择批量计划或单次快照复用。不能把重复校验直接当作已证实的唯一耗时原因 | 可能优化，但必须保留身份、权限、预算和回滚 |
| Q05 | 新接口有 OperationBudget；[font_program.rs](../pdf/src/font/font_program.rs) 明确 FreeType 原生分配不计池；各版列出缓存／store 排除项 | 按公开入口建立预算覆盖账本，区分调用方输入、瞬态克隆、常驻缓存及原生 RSS，不承诺一个 max_operation_bytes 限住整个进程 | 否，先登记与审计；全链路硬上限另定范围 |
| Q06 | 固定正负样本和十二组独立门禁已有；完整 1633 项是 v0.14 冻结结果 | 扩大真实文件及相互组合，按失败归因建立小回归；不能用测试总数换算 PDF 标准覆盖率 | 否；不要求 fuzz 或任意 PDF 均支持 |
| Q07 | 三个 Cargo package 均为 0.1.0，而实施里程碑已到 v0.14；许可／字体／依赖记录分别存在 | 决定产品里程碑与 crate 版本关系，核对安装／分发、迁移与第三方资料。没有发布动作不说明版本号是缺陷 | 否；不自动改版本、做法律结论或发布 |

本轮实际执行了 `cargo metadata --offline --locked --format-version 1`、本地工具链查询和源码／测试分支核对，没有重新运行完整工作区、获取新真实 PDF、进行新渲染或新增通过计数。`pdf/src` 与 `render/src` 搜索未发现 `todo!`／`unimplemented!`，但 `Type1Program` 等用 UnsupportedFeature 明确拒绝，因此不能据宏搜索宣布功能完整；也不能据 unwrap／unreachable 的文本命中直接认定输入漏洞。

## 暂缓和应用层能力

继续暂缓 fuzz、3D／多媒体、JavaScript／XFA 执行、专业印刷、页面切换／高级组合包／地理空间、线性化输出、时间戳／LTV／在线信任与全格式字体子集器。OCR、无标记表格／阅读顺序推断、自动排版与字体选择属于上层；显式结构解释仍属于核心。XFA 图存在时 [field_graph](../pdf/src/document/acroform/graph.rs) 的拒绝是既定边界，不算本轮待修缺陷。

## 后续顺序建议

先完成 NEXT-02：冻结稳定 API 与正式支持范围，再决定兼容性增量。已有已验收子集可以作为稳定版候选，不需要把 F01～F10 全做完才有资格稳定；但不能把明确不支持的合法 PDF 场景宣传为通用支持。

若继续做 PDF 规范基础功能，优先盘点一个“小版本字体读取兼容”候选：F01 为主线，F02 先恢复同 hash 证据并评估，独立决定是否纳入。需要先确认 CID-keyed CFF 的合法可分发原始样本和独立工具能力，再冻结 API／预算／映射／负例。F03 轮廓裁剪、F04 Unicode FreeText、F05 XMP 和 F06 图层迁移分别成题，不塞进同版。

工程上优先处理 Q01 的声明与验证、Q03 的契约，以及 Q04 的可重复性能分析；跨平台按实际发行目标安排。此次审计不批准 v0.15／v1.0 开工，也不改变任何已验收版本、来源保护、权限或输出默认。
