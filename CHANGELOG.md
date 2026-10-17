# 变更记录

本文件只记录交付变化，不替代长期任务池或原始测试日志。版本范围见 [ROADMAP](ROADMAP.md)。

## Unreleased — v0.11 保存与安全扩展（限定范围已验收）

2026-10-17，V11-00～13 全部按限定范围收口：最终完整工作区 85 组、1,501 项、联合 156 项、四组独立门禁共 120 案例、八组精确像素及目视复核通过；609 项冻结输入前后内容和列表一致。仅本地 macOS x86_64 验收，未提交、推送、改包版本或发布；不等于完整 PDF/PAdES 合规。具体证据见[验收记录](docs/releases/v0.11-acceptance.md)。

新增层级/多 Widget 签名字段、既有空字段签署和 Unicode 可见外观；复用源字体及 AP 管线，不引入自动布局或身份信任背书。新增 DocMDP/FieldMDP 签署版本约束读取、实际 revision 修改分类，以及既有 Tx/checkbox 填值、既有空字段签署和新增 Square 的受控工作流。不可变计划绑定来源并在完成时重验全部旧签和实际变更，不开放普通编辑或保存；未知组合继续拒绝。接口与边界见[受控工作流指南](docs/signed-workflows.md)。

修复字符串相等判断受 literal/hex 写法影响的问题，以及简单字体 Widths 范围外未使用 MissingWidth 导致独立签名 AP 文字重叠的问题；显式零宽不改变。新增跨 MuPDF/Skia 的原生产者文字范围检查，补足同引擎像素参考可能共享错误的盲点。

此前新增 `prepare_additional_signature()`：在严格不可变、未加密 PDF 2.0 的限定扁平字段输入上追加不可见审批签名；保留源前缀和旧签覆盖，准备及完成均逐签重开验证。该旧入口仍拒绝历史 MDP/字段锁、共享归属及未知组合；新的受控入口不扩大它的授权范围。V11-07 历史检查点为完整工作区 1,483 项、联合 57 项、独立多签 21 案例（开启 diff-policy）、历史 CMS 42 案例与 R6 30 案例通过，471 个输入前后指纹一致。接口见[追加审批签名指南](docs/advanced-saving.md#additional-approval-signatures)。

新增历史签名模型和 `verify_signatures()`：按真实 xref/Prev、EOF 和原始 Contents token 绑定签署版本与字段，保留被替换/删除签名字典的报告；公开区分密码学成功、失败、格式错误和不支持。复用现有 CMS/DER/ring，新增 PDF 2.0 的限定 PSS/P-256 支持；完成新签也支持这些算法，不增加依赖。源保护补强为历史普通对象及当前/历史 ObjStm 成员扫描。V11-04～06 子范围验收时完整工作区 1,474 项、联合 48 项、独立签名 42 案例与 R6 30 案例等门禁通过，接口见[验签指南](docs/signature-verification.md)。

新增 authenticated R6 增量保存：校验直接/间接 Encrypt 和原始 ID，保留原文件前缀、密钥/保护参数及第一 ID，实际追加更新第二 ID；只加密变更对象，支持原 ObjStm 成员的独立更新。沿用 metadata/Identity 例外和共享预算，保护参数变更及签名来源仍拒绝。新增 8 项集成和 3 项内部保护测试，修复 Encrypt/Perms 与签名权限字典混淆；R2～5 遗留解密明确暂不纳入。阶段 A 历史检查点为完整工作区 1,464 项、联合专项 39 项、独立追加 30 个案例及源码一致性门禁通过；最终交付证据见[v0.11 验收记录](docs/releases/v0.11-acceptance.md)。

## Unreleased — v0.10 显式结构与可选内容（限定范围已验收）

新增显式只读 `structure_associations()`：按页面/Form 分离 MCID，扫描真实标记及 Properties、Form 调用，校验 MCR/Stm、批注/Widget 及 XObject OBJR、ParentTree 双向所有权/null 槽/键冲突和 NextKey；累计解码、调用、操作和深度有限额。Form 缺省资源按规范回退到页面，跨页歧义拒绝。

新增只读 `structure_tree()` 层级、文档限定句柄/快照、P/K 检查、RoleMap、页面及标题/Lang/Alt/ActualText 元数据；内容和 ParentTree 仍由显式归属接口校验。扩展 PDF 2.0 NS/Namespaces/RoleMapNS 限定端点解析，以及 ClassMap/A/C、本地 owner/key 优先关系与修订模型；PDF 1.x 重复属性返回歧义，未知流返回 Opaque，不猜测布局继承或扩展语义。`StructureElement/StructureTree` 新增模型字段；`mapped_role` 仅保留无 NS 元素的旧式端点，命名空间身份和环状态使用 `resolved_role`。

新增显式 `extract_marked_text()` 和 `extract_text_structure_order()`：支持 Properties、Span/Artifact ActualText、Unicode/空替代、跨操作及嵌套 Form 来源；过滤 Artifact，按声明 K 顺序提取并应用元素 ActualText。替代保留来源但不伪造字形位置，不猜测无标签阅读顺序；跨所有者原子替代及结构 Form 多实例明确拒绝，未关联单位/未提取 OBJR 显式计数。共享工作限额覆盖跨 Form/跨页及被过滤或覆盖的文本。接口及边界见[逻辑提取指南](docs/marked-text.md)。

新增[结构编辑接口](docs/structure-editing.md)：节点创建/元数据与本地属性更新、混合 K 移动、空节点删除或子项提升；同步 P/Pg/ParentTree，保留已有 Form MCR/OBJR 及 NS/C/ClassMap，属性写入使用当前 R。支持 RoleMap 设置/移除、快照绑定的页面操作标记、自动 MCID/StructParents/NextKey 和可选 ActualText；新内容流隔离共享来源，错误和外层 panic 回滚。要求 PDF 1.4+、修改权限及无签名来源；不直接标记 Form 内部、不递归删除正文子树，未知反向扩展明确拒绝。

新增[结构感知的页面迁移](docs/structure-transfers.md)：复制/导入/合并/选页按 K 分支投影，维护页面/Form 域、MCR/OBJR 和 ParentTree；隔离共享资源，删除裁剪分支，重排保留声明顺序，Form Pg 仅在调用页唯一时重绑。正文快照编辑前后校验所有权，无法维护的结构 Form 修改回滚；显式 ActualText 不由字形自动改写。保留角色映射/类/命名空间定义，冲突及捕获双方未映射角色/未定义类时拒绝；图层配置迁移仍明确拒绝，不因阶段 C 的读取能力而放开。

新增[可选内容](docs/optional-content.md)模型和显式图层接口：默认/替代配置、BaseState/ON/OFF、四种 OCMD 策略、有界 VE、显式 View/Print、支持的 AS 状态类别及临时覆盖。BDC、Form/image、批注可见性接入既有绘制/提取；隐藏仍执行状态并消耗预算，不删除源内容、不改操作地址。显示列表和逻辑报告新增可选的 selection 缓存标识；旧接口不静默筛选。未知规则、循环、无法分层的原子替代及未支持的组合明确拒绝，保留页面迁移安全保护。

2026-10-17，V10-01～09 与阶段 A/B/C 在限定范围内全部验收：27 项新增图层测试、14 组 206 项联合回归、七组独立门禁、三配置图层像素/目视对照，以及最终源码完整工作区 80 组、1,453 项全部通过。457 个 Rust/Cargo/Python 输入在回归前后指纹及列表一致，证据见[验收记录](docs/releases/v0.10-acceptance.md)。可盘点 v0.11，不自动开工；不承诺完整 Tagged PDF/PDF-UA。未修改包版本、提交、推送或发布。

## Unreleased — v0.9 文档交互与对象关系（限定范围已验收）

新增[页面关系操作](docs/page-relationships.md)：复制和跨文档导入维护导航、Popup/IRT、AP/资源、层级字段及多 Widget；源标签默认迁移，目录导航显式启用，命名与 DR 冲突不覆盖。删除可显式重定向目标，并解除离页 Widget、空字段及祖先组关系；未知活跃反向引用和未知标签树扩展拒绝并回滚。2026-10-16，V09-01～11 在限定范围内关闭：最终源码完整工作区 73 组、1,312 项及五组独立门禁通过，23 页视觉/像素基线复核通过。新增 14 项页面关系用例。边界见[支持矩阵](docs/releases/v0.9-support.md)，源码指纹和最终证据见[验收记录](docs/releases/v0.9-acceptance.md)。未修改包版本、提交、推送、标签或发布。

阶段 C 新增统一字段定义与详情接口、自定义 Checkbox 状态/Unicode 导出标签、Radio NoToggleToOff/InUnison/明确 Widget 选择、Choice Combo/List 和导出/显示/I 索引维护、无永久值 PushButton 与 caption。字段文字复用嵌入字体、DA/DR、ToUnicode 与宽度，支持中文显式单/多行和左/中/右对齐，保存重开可继续填值；外观、缺字/溢出、限额、权限及签名保护共用管线。修复真实 Base14 Type1 Differences 缺省基础编码和全局 Q 默认值；保留 Ff 继承并避免浮点丢失标志低位。`FormValue` 新增变体，穷尽匹配需要迁移。该阶段完整工作区 72 组、1,298 项通过，四组独立门禁及视觉复核通过，历史检查点保留在验收记录中。边界见[表单指南](docs/forms.md)。

字段关系里程碑新增层级 AcroForm 树、继承检查和文档限定句柄；支持完整路径新增、部分名称改名、字段子树移除、合并 Widget 拆分、多页呈现增删与同步填值。未知字段可检查，动作/锁定/所有权/碰撞明确拒绝并回滚；旧 `form_fields()` 的页号表示第一个 Widget。该里程碑的 9 项新增测试、四页独立门禁和完整工作区 71 组、1,286 项通过，历史证据保留在阶段记录。

阶段 B 新增 15 类限定批注模型与稳定 CRUD、Popup/回复关系及显式级联移除；Square 和现有 AcroForm 复用自包含 AP 构建。增加可选批注/Widget 渲染、Screen/Print、AS/BBox/Matrix、NoRotate/NoZoom、整体透明度与 Highlight Multiply，正文提取与编辑来源保持隔离。FreeText 外观仅为 ASCII Courier、显式换行/对齐。该阶段完整工作区 70 组、1,277 项、独立批注/导航门禁及六页视觉检查通过。MuPDF 特殊 NoRotate 组合差异单独保留。限制和接口见 [批注指南](docs/annotations.md)，证据见 [阶段记录](docs/releases/v0.9-acceptance.md)。

导航阶段新增共用 Destination/View，保留坐标与 null；层级书签、稳定句柄 CRUD、命名目标改名与引用维护、GoTo/URI 链接、共用平衡 NameTree/NumberTree、嵌套附件树，以及随页面维护的 PageLabels。该阶段定向 122 项文档模块测试、19 项集成测试和 qpdf/pikepdf/MuPDF 门禁通过；整版结论以上方最终记录为准。接口及限制见[导航指南](docs/navigation.md)，全部任务及收口范围见[v0.9 计划](docs/releases/v0.9.md)。`LinkTarget` 新增带浮点视图的 Destination 变体，不再实现 Eq。

## Unreleased — v0.8 图形颜色与透明度（限定范围已验收）

新增实验性平铺及 Shading Pattern 解释、资源注册/导入和显示快照；Skia 路径、文字和 stencil 绘制可使用图案来源。图案保留局部资源和默认坐标系，支持正负步距、单元裁剪及非着色基空间色值；Pattern Background 与直接 sh 的规则分开。加入累计工作/显示载荷和离屏表面限制、模型/像素测试及独立互操作脚本。

新增 Type7 张量积补片，保留实际内部控制点，复用认证区间求根与累计工作上限；接通解析、函数颜色、显示、注册、导入与严格保存。透明度新增显式 DisplayList/Device 组边界，RGB 隔离/非隔离/挖空、独立 shape/opacity、嵌套背景校正、图形 Alpha/Luminosity SMask 及 TrueType Tr4～7 的 ET 轮廓裁剪。透明填充加描边按隐式挖空组处理，失败先丢弃再恢复。

颜色新增固定版本纯 Rust moxcms ICC 后端、真实 Gray/RGB/CMYK 配置转换、四种 rendering intent、显式 RGB 数值输出、有限共享缓存及显式 Alternate 回退。非法/空 ICC 不再静默使用 Alternate；依赖及许可见 [清单](docs/releases/v0.8-dependencies.md)。

接口变化：`ResourceKind` 新增 Pattern；`Pattern::Shading` 携带实际模型；`TilingPattern` 不再提供零步距 Default；`PageObject` 新增 GroupBegin/GroupEnd，`ShadingGeometry` 新增 TensorMesh；GeneralState 新增软蒙版/意图。自定义 Device 须处理组边界，不能静默展开。详见 [Pattern](docs/patterns.md)、[透明度](docs/transparency.md)、[颜色管理](docs/color-management.md)及 [支持矩阵](docs/releases/v0.8-support.md)。2026-10-15，最终未提交候选完整工作区门禁退出 0：68 组、1,251 passed、0 failed/ignored/filtered；八组独立门禁及 91 张主样本视觉复核通过。V08-00～17 在限定范围内关闭，源码及证据见 [验收记录](docs/releases/v0.8-acceptance.md)。未提交、推送、标签或发布，未修改包版本；fuzz 暂缓。

## Unreleased — v0.7 字体与文字解释（限定范围已验收）

新增 Type3 字体检查及受限不透明图形字形执行，接通 d0/d1、FontMatrix、资源、文字提取/ToUnicode 和来源隔离。修复带旋转 FontMatrix 的文字推进：只用水平分量，字形形状仍完整变换。字形内绘图与文字元数据分开，避免重复绘制；资源、循环、累计工作量、R6、共享页编辑和保留预算有回归。非默认 Tr、透明度和字形内文字仍明确拒绝。

新增字体检查快照和显式 `GlyphRun`/`PositionedGlyph` 页面写入：PDF 字符码、位置、可选 GID/Unicode 断言，支持冻结的水平/竖排字体组合；事务、共享资源、全量/增量保存及权限保护有回归。CMap 查找和反向编码保留源字节长度，新增 `cid_for_char`；旧无长度 CID 查询遇歧义返回 0。补齐 Type1、CFF、OpenType/CFF、TrueType 固定读绘及独立验证矩阵。

兼容变化：`Font` 增加 `Type3(Rc<Type3Font>)` 变体；自定义设备直接消费显示列表时应跳过 `TextObject::is_type3_metadata()` 文字元数据。`ExtractedCharacter` 新增 `vertical_origin`，下游字面量构造需迁移；竖排 em/轮廓辅助几何正确应用 W2，原 `position` 仍是文字原点。详见 [字形运行指南](docs/glyph-runs.md)与 [Type3 指南](docs/type3-fonts.md)。

2026-10-15，最终候选 `8a901e0` 的完整工作区回归退出 0：64 组、1,208 项通过，0 失败/忽略；字体、公开文件/子集、Type3 独立门禁及视觉复核通过。范围见 [矩阵](docs/releases/v0.7-font-matrix.md)，证据与保留限制见 [验收记录](docs/releases/v0.7-acceptance.md)。未推送、打标签或发布；包版本未修改，fuzz 继续暂缓。

## Unreleased — v0.6 内容流与图像基础（限定范围已验收）

- 新增类型化内联图像、精确 BI/ID/EI 边界及字典/载荷来源范围；接通构建、解释、提取和限定来源编辑，保留邻接内容、共享页及 Form 调用隔离。
- 新增进程内 JPX Gray/RGB 基础解码与 JBIG2 Generic/Pattern-Halftone 基础解码；保存/导入保留原编码及真实共享 Globals。无 JPX/JBIG2 编码器；JPX Alpha/ICC、JBIG2 symbol-coded 等组合明确不支持。
- 补齐同池 Globals/解码结果预算、结构/尺寸预检和失败重试；修复 CCITT 展开样本与图像打包样本契约不一致的问题，覆盖行补齐、极性、Decode 和 stencil。
- 新增独立生产者样本及许可/hash、图像指南和独立像素/结构/渲染门禁。`Operator` 的内联载荷不再是普通操作数，BI/ID/EI 按一个操作索引；最低 Rust 要求为 1.92，实际验证版本为 1.96。

2026-10-15，最终源码 `0f647af` 的完整工作区回归退出 0：61 组、1,165 项通过、0 失败/忽略；独立九页解码像素差均为 0，保存/导入、R6、共享 Globals 和视觉复核通过。具体范围见 [输入矩阵](docs/releases/v0.6-matrix.md)，迁移入口见 [图像指南](docs/image-codecs.md)，证据及预算限制见 [验收记录](docs/releases/v0.6-acceptance.md)。未推送、打标签或发布，Cargo 包版本未修改，fuzz 继续暂缓。

## Unreleased — v0.5 高级保存与安全（限定范围已验收）

新增三个独立工作流，默认保存仍是全量重写：

- 显式增量保存：严格 owned-bytes 输入、传统/流式/混合引用、多级 Prev 链及普通对象新增/替换/删除；保留原字节，不隐式 rebase，不提供源文件原地追加。
- Standard AES-256 / R=6 密码保护：用户/所有者认证、权限报告、对象级解密及全量 Preserve/Remove/Set；默认保存保留保护，已有保护的移除或替换要求所有者授权。
- 外部 detached CMS 签名：不可变 prepare/complete 两阶段、单个不可见批准签名、SHA-256 / RSA PKCS#1 v1.5 完整性检查、准备令牌失效/重复使用保护及已签来源编辑拒绝。核心不处理私钥，不将结构报告当成证书信任验证。

加密增量保存、加密签名、多签追加、R=2–5、可见签名、PAdES 和在线信任服务不在本版范围。混合引用历史可增量保存，但签名前须显式全量规范化并重开。增量删除不等于安全擦除。

2026-10-14，最终源码 `5e9bc93` 的锁定依赖完整工作区回归退出 0：59 组、1,139 项通过、0 失败/忽略；三组独立互操作门禁及最终视觉复核通过。范围外能力不因此转为正式支持。

新增独立 qpdf/libqpdf、MuPDF、pypdf、pyHanko 互操作门禁、预算/权限/回滚负例和固定 1/10/100 MiB 保存基线。升级锁定的 quick-xml、tar、bitstream-io，修复审查中发现的既有依赖问题；新增密码学依赖许可保持独立。`FilterContext` 增加解密状态字段，旧占位 SecurityHandler 改为真实实现；迁移及错误契约见 [高级保存指南](docs/advanced-saving.md)，精确范围见 [支持矩阵](docs/releases/v0.5-support.md)，实测结果见 [验收记录](docs/releases/v0.5-acceptance.md)。未打标签、推送或发布，Cargo 包版本未修改。

## Unreleased — 项目自有代码许可证变更

2026-10-14，根据维护者确认的代码来源，将项目自有代码从 GPLv3 改为 `AGPL-3.0-only`，同步根 LICENSE、README 和 Cargo 包元数据。第三方字体、度量、CMap 及其他独立授权材料的许可不变；此前版本的授权不撤销，历史提交不重写。范围与来源见 [许可证变更记录](docs/licensing.md)。

## Unreleased — v0.4 文档交互限定范围（已验收）

新增扁平书签与命名目标、页/命名/URI 链接、带可见外观的 Square 批注、文档级附件和基础文本/复选框 AcroForm API。新增 ReportLab、pypdf、qpdf 独立生产者样本与保存读回/失败回滚测试；页面关系尚未重映射的操作保持明确拒绝。限定支持类型和不支持的结构见 [v0.4 支持矩阵](docs/releases/v0.4-support.md)。2026-10-14 用户确认修复附件限额测试后的完整工作区回归全部通过，结合定向测试和独立结构/视觉检查完成限定范围验收。证据来源与已知限制见 [v0.4 验收记录](docs/releases/v0.4-acceptance.md)。尚未打标签或发布。

## Unreleased — v0.3 固定字体与公开文件矩阵

在冻结的静态字体和两份公开 PDF 范围内，补充 Courier/WinAnsi、中文
Identity-H/V、复合字形子集编辑及保存重开的独立校验；缺失字体和字形不再
静默退回或跳过，字体来源可区分嵌入、Core 14、显式替代和缺失。支持 MCID-only
标记内容的视觉/普通文字遍历，并改善细路径抗锯齿。`font-data`
改为从只读输入向 Cargo `OUT_DIR` 生成数据，不再写回 `src`。具体支持边界、
原始中文 PDF 的兼容模式限制和验证证据见 [v0.3 支持矩阵](docs/releases/v0.3-support.md)
及 [验收记录](docs/releases/v0.3-acceptance.md)。未打标签或发布。

## v0.2 限定范围已验收

2026-10-13 实现候选 `89b98a3` 的 V02-01～07 全部通过：锁定依赖完整回归 1,069 项通过，v0.1 基础门禁复验通过，qpdf/MuPDF 独立结构、文字、位置和画面检查通过。新增来源驱动的内容编辑、中文、四类文字操作、独立生产者及一层 Form 示例与 v0.2 自动门禁；核心库生产代码未改动。正式范围与限制见 [支持矩阵](docs/releases/v0.2-support.md)，过程与归档见 [验收记录](docs/releases/v0.2-acceptance.md)。尚未打标签或发布。

## v0.1 已验收，未单独发布

尚未打标签或发布。2026-10-13实现候选`89c714a`在冻结v0.1范围内完成8项验收：完整门禁退出0，1069项测试通过，独立结构/文本检查及12页视觉验证通过。可进入v0.2；实验能力和未验证平台不因此转为正式支持。详见 [验收记录](docs/releases/v0.1-acceptance.md)。

### 文档与交付流程

- 建立 v0.1～v1.0 分版本路线，区分正式支持候选、实验能力和未支持范围。
- 建立 v0.1 必选场景及发布门槛，将 TODO 保留为长期技术任务池和历史记录。

### 实现变更

- 将11个无许可声明的标准CMap替换为固定Adobe官方发行版的BSD-3-Clause数据，保留原声明及来源/哈希；移除5个仅供旧拆分表使用的非标准 `*-X` 辅助表。标准编码的有效映射/方向/编码范围保持一致；直接请求这些非标准辅助名称不再支持。新增24个受影响标准编码的离线指纹检查、Rust解析回归及CI门禁。
- 增加独立MuPDF生产者的两页兼容性fixture及离线回归，验证文字位置、旋转、压缩图片提取与保存重开；保留生成源和样本哈希。
- 补齐内置PFB字体的确切源包/逐文件对照和上游许可文本，归档中文验证字体OFL副本；新增冻结的v0.1候选支持矩阵。
- 完整门禁增加fixture与脚本校验及源文件列表对照，防止运行期间新增/删除输入未被发现。

- 新增 v0.1 公共 API 组合工作流回归，覆盖页面操作、合并/选页、两种保存策略及源文档隔离。
- 新增完整自动验收入口 `docs/check-v01.sh`，保留工作区指纹、测试日志和独立检查/渲染证据；中文输入必需，不静默跳过。
- 普通 CI 增加英文创建样本的独立结构/文本检查、渲染及产物归档；中文门禁和人工验收仍须单独执行。

实现基线为`89c714a`，验收结论仅覆盖支持矩阵所列范围。标签与对外发布单独执行，不将未来计划或现存实验能力写成已交付功能。
