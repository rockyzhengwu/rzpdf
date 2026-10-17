# PDF 引擎实施计划

更新日期：2026-10-17。本文是长期技术任务池与历史实施记录，不表示这些功能已经完成。

## v0.11 收口记录

[实施计划](docs/releases/v0.11.md)的 V11-00～13、阶段 A/B/C 和横向门禁已完成限定范围本地验收。最终完整工作区 85 组、1,501 项、联合 156 项、四组独立门禁 27/21/42/30 案例、八组精确像素和目视复核通过；609 项冻结输入前后内容及列表一致。范围见[安全矩阵](docs/releases/v0.11-security-matrix.md)，证据见[验收记录](docs/releases/v0.11-acceptance.md)，接口见[工作流指南](docs/signed-workflows.md)。一般签名来源编辑/保存仍拒绝，专用受控计划只开放已声明子集；长期父项不因此全部勾选。下一步可盘点 v0.12，不自动开工。本轮未提交、推送、改包版本或发布，历史记录保留。

## v0.10 验收历史记录

2026-10-17，[版本清单](docs/releases/v0.10.md)的 V10-01～09 已按三个必选阶段及横向门禁完成限定范围验收。结构读取/提取、节点编辑、页面迁移/正文保护，以及 OCG/OCMD 配置、显式图层渲染/提取完成；最终完整工作区 80 组、1,453 项，206 项联合回归、七组独立门禁及三配置像素/目视复核通过。边界见[可选内容指南](docs/optional-content.md)及[迁移指南](docs/structure-transfers.md)，457 个源码输入的前后指纹和具体证据见[验收记录](docs/releases/v0.10-acceptance.md)。不放宽未知扩展、原子替代分割和图层配置迁移的拒绝保护，不承诺完整 Tagged PDF/PDF-UA。可进入 v0.11 盘点，不自动开工或关闭长期 P0～P5 父项；未提交、推送、改包版本或发布，fuzz 继续暂缓。

## v0.9 收口记录

2026-10-16，[v0.9 清单](docs/releases/v0.9.md)的 V09-01～11 在限定范围内全部关闭：导航、批注、AcroForm、Unicode 外观及页面关系迁移完成。最终源码完整工作区 73 组、1,312 项通过，五组独立门禁和 23 页视觉/像素基线复核通过；14 项新增页面关系测试覆盖冲突、目标重定向、字段/批注维护、未知标签扩展、权限/签名、限额与回滚。边界见[支持矩阵](docs/releases/v0.9-support.md)，源码指纹和证据见[验收记录](docs/releases/v0.9-acceptance.md)。未提交、推送、标签或发布；不自动进入 v0.10，不关闭长期 P0～P5 父项，fuzz 继续暂缓。

## 当前版本管理规则

2026-10-15 v0.8 的 A1/A2/B/C 限定范围已完成本地工程验收：完整工作区 68 组、1,251 passed、0 failed/ignored/filtered；八组独立门禁及 91 张主样本视觉复核通过。Pattern、Type7、RGB 透明组/挖空/软蒙版、TrueType 裁剪与实际 ICC 转换已收口；V08-00～17 关闭，允许进入 v0.9 盘点，不自动关闭长期 P0～P5 父项。任务见 [版本计划](docs/releases/v0.8.md)，边界见 [支持矩阵](docs/releases/v0.8-support.md)，源码指纹和证据见 [验收记录](docs/releases/v0.8-acceptance.md)。本轮修改尚未提交；未推送、标签或发布，fuzz 继续暂缓。

2026-10-15 v0.7 的 A/B/C 冻结范围已完成本地工程验收：最终候选 `8a901e0` 的完整工作区 64 组、1,208 项通过，0 失败/忽略；字体映射/原生读绘、显式字形运行、公开文件/子集及 Type3 独立门禁和视觉复核通过。V07-00～09 关闭，允许进入 v0.8 范围盘点；不自动关闭 P0～P5 长期父项。任务见 [计划](docs/releases/v0.7.md)，边界见 [矩阵](docs/releases/v0.7-font-matrix.md)，证据与未解决输入见 [验收记录](docs/releases/v0.7-acceptance.md)。未推送、打标签或发布；fuzz 继续暂缓。

2026-10-14 v0.5：最终源码 `5e9bc93` 的 A/B/C 限定范围已完成本地工程验收。完整工作区门禁退出 0：59 组、1,139 项通过、0 失败/忽略；三组独立互操作门禁及最终视觉复核通过。最终范围和证据见 [v0.5 计划](docs/releases/v0.5.md)、[支持矩阵](docs/releases/v0.5-support.md)和 [验收记录](docs/releases/v0.5-acceptance.md)。本段取代下方旧版本的“当前阶段”快照；fuzz 继续暂缓，未推送、打标签或发布，不自动勾选完整 P0～P5 父任务。

2026-10-15 v0.6 已完成限定范围本地工程验收：最终源码 `0f647af`，完整工作区 61 组、1,165 项通过，0 失败/忽略；独立像素/结构及逐页视觉复核通过。内联读写/来源编辑、进程内 JPX/JBIG2 基础后端和真实共享 Globals 路径已收口。范围及未验收组合见 [输入矩阵](docs/releases/v0.6-matrix.md)，任务见 [计划](docs/releases/v0.6.md)，证据见 [验收记录](docs/releases/v0.6-acceptance.md)。未推送、打标签或发布，不自动关闭长期父任务；可进入 v0.7 范围盘点。OCR、推断和排版继续放入上层。

2026-10-14 范围收敛：按用户确认暂缓低优先级专题，清单见标准路线的“已确认暂缓的能力”；线性化改为可选后续专题，不阻塞 v0.12。v0.6 的 A/B/C 均为必选，支持范围按已冻结矩阵逐项验收；未开放的 JPX Alpha/ICC 和 JBIG2 symbol-coded 输入明确拒绝，不以空 Globals 或空白图片替代门禁。

2026-10-13 v0.3 的 V03-01～07 已在[冻结子范围](docs/releases/v0.3.md)内完成本地工程验收：锁定依赖的最终全工作区测试退出 0；静态 H/V 字体、Courier、子集/复合字形、两份真实公开 PDF、离线 `font_data` 打包及独立 qpdf/MuPDF/FontTools 对照通过。精确证据与限制见[验收记录](docs/releases/v0.3-acceptance.md)及[支持矩阵](docs/releases/v0.3-support.md)。P3-05/P4 长期父任务不自动随本版关闭，fuzz 继续暂缓；未打标签或发布。下方 v0.1/v0.2 及专项状态为历史记录。

2026-10-13 v0.2 最终验收：实现候选 `89b98a3` 的 V02-01～07 全部通过；完整门禁退出 0，53 组共 1,069 项测试通过、0 失败/忽略，新增工作流 12 页完成视觉或逐字节基线核对。正式范围与证据见 [v0.2 验收记录](docs/releases/v0.2-acceptance.md)及 [支持矩阵](docs/releases/v0.2-support.md)。未打标签或发布，fuzz 继续暂缓。下文较早的“当前/待验收”文字为历史快照。

2026-10-13最终验收：实现候选`89c714a`完整门禁退出0，53组共1069 passed、0 failed/ignored；12页渲染目视/逐字节对照通过。V01-01～08全部关闭，可进入v0.2常用内容编辑版，下一步先冻结编辑操作/字体/嵌套范围并盘点已有实现。证据与限制见 [最终验收记录](docs/releases/v0.1-acceptance.md)。尚未打标签或发布；远端没有CI，仅声明本地macOS已验证；fuzz继续暂缓。下文旧快照不代表当前状态。

当前CMap修复已验证：11个标准表替换为Adobe官方20190730发行版原文件，移除5个无剩余引用的非标准辅助表；24个受影响标准编码的CID映射、codespace和WMode经离线及Rust生产解析回归对照一致。65个现存CMap均有Adobe声明，官方BSD-3-Clause、上游提交及SHA-256已归档；不更改根许可、不重写历史。716项PDF单元及2项v01_workflow均退出0，证据 `output/cmap/v01-20261013`；新候选完整门禁仍待终态，计划目录 `output/pdf/v01-cmap-candidate-20261013-01`。原5da2e34门禁因实现变更主动停止（退出143），不再作为当前候选验收。fuzz继续暂缓，不打标签或发布。

以下同日期收口快照保留当时状态，不表示相关门禁现在仍运行；最新状态以本段、版本清单和验证记录为准。

2026-10-13 候选验收：实现已提交 `5da2e34`，用户已授权提交/推送。正在独立干净工作区执行完整门禁 `output/pdf/v01-candidate-5da2e34-02`，未终态前不勾选全部验收项；首次准备运行退出143的原因及干净检出构建步骤见验证记录。原工作区未知的 `test.png` 和缓存仍保留且不提交。GitHub CLI 凭证失效，远端 CI 尚未核实；不打标签或发布。

2026-10-13 v0.1继续收口：独立MuPDF生产者fixture及2项组合回归退出0；14个内置PFB与历史gsfonts源包逐字节匹配，来源/上游许可及Noto OFL副本已归档；候选支持矩阵已冻结。完整自动门禁运行于 `output/pdf/v01-20261013-workspace-01`，未终态前不宣布通过；本轮不变更被冻结的Rust/fixture/门禁输入，fuzz不实现、不运行。最终候选提交与远端操作另待用户确认。

v0.1 首批收口已落地：新增 `pdf/tests/v01_workflow.rs` 公共 API 组合回归、`docs/check-v01.sh` 完整自动门禁，普通 CI 增加独立英文创建检查/渲染归档；场景映射与实际专项结果见 [v0.1 验证记录](docs/releases/v0.1-validation.md)，来源/许可缺口见 [样本登记](docs/releases/v0.1-samples.md)。未将工作区专项当作最终候选提交验收，V01-01～08 仍以其完整条件决定勾选。

当前阶段为 **v0.1 已验收、尚未发布，允许进入v0.2范围盘点**。版本范围以 [ROADMAP.md](ROADMAP.md) 为准，v0.1证据以 [验收清单](docs/releases/v0.1.md) 为准；二者取代本文历史执行队列和 M0～M5 顺序作为当前工作入口。

P0～P5 保留稳定任务 ID 和历史证据，不删除已有代码，不因划入后续版本而撤回已有能力。任务的目标版本映射见 ROADMAP；跨版本任务只验收当期子范围，不提前勾选整个父项。版本进度按验收场景报告，不按全局 checkbox 比例报告。旧记录中的“当前”“下一步”“运行中”“源码冻结”描述当时状态，不自动代表现在状态。

2026-10-12 状态核对：最新全量日志 `/tmp/rzpdf-resource-owner-workspace.log` 记录52组、1066项通过、0失败、0忽略；旧会话句柄已过期，未取得总退出码，不将其描述为已观察到 EXIT 0 的发布门禁。对应 Rust/Cargo 指纹为 `3e62ae80a25b20f96f36d469fc34aa8dcba167688af39df506220adf0b6486e6`。这是历史核对结果，不能替代最终候选提交上的发布验收。当前本地仍有未提交实现；fuzz 暂缓，不实现、不运行，不阻塞本期正常功能验收。

以下为历史实施记录，较新状态以上述摘要及版本清单为准。

逐对象所有者公共终态：79126退出0，五组15/6/3/8/9共41 passed、0 failed/ignored/filtered；与715项PDF单元同一源码指纹`3e62ae80a25b20f96f36d469fc34aa8dcba167688af39df506220adf0b6486e6`。启动统一全工作区`cargo test --workspace --locked --quiet`，日志`/tmp/rzpdf-resource-owner-workspace.log`，尚未终态；源码继续冻结。ResolvedResource及泛型BM借用核心的专项和公共回归已通过，不等于完整AR-03；旧1065项全量属于此前操作聚合版本，不覆盖本次变更。

逐对象所有者整组PDF单元终态：43937退出0，`/tmp/rzpdf-resource-owner-full-unit.log`为715 passed、0 failed/ignored/filtered，95.57秒；源码指纹`3e62ae80a25b20f96f36d469fc34aa8dcba167688af39df506220adf0b6486e6`未变。公共79126首组nested_adjusted_text已15项通过，当前nested_text_edit，整体公共会话尚未结束，源码继续冻结；不将单元结果视为完整工作区验收。

混合模式专项97276退出0：1 passed、0 failed/ignored（714过滤），普通结果回调的偏好数组完整验证与预算失败清理通过。启动新版本整组PDF单元43937（`cargo test -p pdf --lib --locked`，`/tmp/rzpdf-resource-owner-full-unit.log`），公共79126仍在编译；两者尚未终态。源码指纹核对仍为`3e62ae80a25b20f96f36d469fc34aa8dcba167688af39df506220adf0b6486e6`，源码继续冻结，diff空白检查通过；完整工作区仍需新版本统一复验。

逐对象所有者定向终态：57433退出0，字体绑定15 passed、0 failed/ignored（700过滤），含新增BM逐元素生命周期及既有Tf/gs精确峰值邻居拒绝/重试、返回所有者、异池及unwind用例。启动混合模式专项97276（`/tmp/rzpdf-resource-blend-tests.log`）及五组公共文字编辑79126（`/tmp/rzpdf-resource-owner-public.log`），尚未终态。源码指纹仍为`3e62ae80a25b20f96f36d469fc34aa8dcba167688af39df506220adf0b6486e6`，暂时冻结；新版本整组PDF单元和全工作区仍待验收，旧1065项基线不覆盖本次生产变更。

逐对象所有者编译检查99268退出0（39.18秒）。新增resolved_resource_blend_elements_release_individually_and_retry：32元素逐次查询之间仅数组额度存活、峰值等于数组保留加单元素峰值、精确额度加1字节邻居拒绝/释放重试、支持模式后的非法元素仍报错、解析回调panic释放。字体绑定专项会话57433运行中，命令`cargo test -p pdf --lib --locked page::font_binding::tests`，日志`/tmp/rzpdf-resource-owner-tests.log`；源码指纹`3e62ae80a25b20f96f36d469fc34aa8dcba167688af39df506220adf0b6486e6`。尚未通过专项或新版本公共/全量回归；Tf/gs真实资源精确共存边界仍需补核。

资源副本逐对象所有权实现中：font_binding新增非Clone的ResolvedResource，字段按PdfObject/MemoryLease顺序拥有；font_scope_resource借用类别字典并返回资源所有者，ext_state_font的字号副本局部释放、数组仅保留至字体复制结束。混合模式核心改为泛型Borrow<PdfObject>结果，在持有数组所有者时逐个解析/释放元素，普通PdfObject解析回调保持兼容，并继续验证全部元素。编译检查会话99268（`cargo test -p pdf --lib --locked page:: --no-run`，日志`/tmp/rzpdf-resource-owner-build.log`）尚未终态；待补精确峰值/错误/unwind专项并复跑公共及全量，不能以此前1065项基线证明本次变更通过。

最新统一工作区门禁已通过：会话67632退出0，命令`cargo test --workspace --locked --quiet`，日志`/tmp/rzpdf-font-resource-workspace.log`；52组共1065 passed、0 failed/ignored/filtered，其中PDF单元714、PDF集成313、render单元38。结束时Rust/Cargo源码指纹仍为`054f697ab36f4a86ff191010c44fecbd24cb4c836204b565c2911403b8a5d53f`，解除本轮源码冻结。以下及子任务中的“67632待终态”描述由本条终态取代；不再引用旧1063项基线作为当前版本验收。未实现/运行fuzz，未提交或推送代码。下一步逐项收口所有权与对象表清单，再继续资源副本逐对象生命周期及其边界测试；全量通过不代表完整AR-03或P0–P5完成。

当前验收状态（以下过程记录按历史保留，不作为当前运行状态）：资源作用域定向59057退出0，14 passed、0 failed/ignored（700过滤）；整组PDF单元74329退出0，714 passed、0 failed/ignored/filtered；五组公共编辑89648退出0，15/6/3/8/9共41 passed、0 failed/ignored/filtered。日志分别为`/tmp/rzpdf-font-resource-warmup.log`、`/tmp/rzpdf-font-resource-full-unit.log`、`/tmp/rzpdf-font-resource-public.log`。源码指纹核对仍为`054f697ab36f4a86ff191010c44fecbd24cb4c836204b565c2911403b8a5d53f`。2/3/32次Tf容量预热后峰值稳定与保留额度一致通过；资源副本租约按操作作用域释放，操作内部仍保守聚合，未完成逐对象精确生命周期。下一门禁为同一源码版本的`cargo test --workspace --locked --quiet`；门禁期间冻结Rust/Cargo源码，完成前不以旧1063项基线代替当前验收。fuzz不实现、不运行；AR-03整体保持未完成。

反复Tf首轮77412退出101：13通过、1失败；2次/32次峰值136936/140520，返回保留均26661。槽位表第二次在构造后扩容，第三次起构造时已持有完整表容量，因此2次不能作稳定峰值基线。改为2/3/32对照：返回额度全部相同，第三次及32次峰值必须相等，第二次峰值不得更大；不放宽生产预算。复验会话日志`/tmp/rzpdf-font-resource-warmup.log`，尚未终态。

资源作用域首轮45744退出101：13通过、1失败，继承字体q/Q恢复测试旧断言要求返回额度增加，实际已不保留临时资源副本。改为返回保留额度与原继承所有者相等，并补真实前缀2次/32次同字体Tf的峰值和返回保留额度相等、释放归零对照；没有放宽预算。复验77412运行中（`/tmp/rzpdf-font-resource-repeat.log`），尚未通过。

资源解析副本租约收敛第一步：font_binding的resolve闭包及object_lease改为每个前缀操作的局部作用域，租约保留到Tf/gs匹配分支拥有对象全部销毁后释放，不再将整段前缀副本额度移交最终字体；最终结果仅转移SelectedFont派生租约。操作内部仍保守聚合临时对象，不是逐对象精确释放或RSS约束。定向45744运行中，日志`/tmp/rzpdf-font-resource-scope.log`，尚未验收；旧714+41证据不覆盖本次生产变更。

槽位版公共终态：17957退出0，五组15/6/3/8/9共41 passed、0 failed/ignored/filtered；与714项整组PDF单元同一源码指纹`def0a04d5973ab4f667a94763bf9aa8b99852106dc2f721dfcf63ae5ca2bc31b`。解除本轮源码冻结。下一步核对font_scope_resource/ext_state_font及通用resolve闭包：当前对象租约汇入object_lease，临时对象销毁后仍保留至结果释放；需要对象与租约联合拥有的解析结果，按真实共存生命周期释放，不影响返回字体派生租约。最新完整workspace仍待统一复验，不勾选AR-03。

槽位版整组PDF单元终态：91903退出0，`/tmp/rzpdf-font-arena-full-unit.log`为714 passed、0 failed/ignored/filtered、110.53秒；源码指纹`def0a04d5973ab4f667a94763bf9aa8b99852106dc2f721dfcf63ae5ca2bc31b`未变。公共会话17957仍在nested_adjusted_text，源码继续冻结；不能以714单元替代公共或完整工作区验收。

槽位计费定向终态：43982退出0，14 passed、0 failed/ignored（700过滤），真实布局/增长共存/复用/拒绝重试/unwind及修正后的栈边界全部通过。当前源码指纹`def0a04d5973ab4f667a94763bf9aa8b99852106dc2f721dfcf63ae5ca2bc31b`；启动整组PDF单元91903（`/tmp/rzpdf-font-arena-full-unit.log`）及五组公共文字编辑17957（`/tmp/rzpdf-font-arena-public.log`），两者未终态，源码冻结。仅验证槽位和租约子项，不代表资源副本计费或完整workspace/AR-03验收。

槽位版首轮定向78318退出101：12通过、1失败，font_stack_growth旧测试仍按Option<Rc<Font>>计算额度；已改为真实Option<usize>栈元素大小，未放宽生产额度。新增槽位控制存储专项：实际capacity×槽尺寸、旧新布局共存峰值、32次空槽复用、别名take拒绝、增长失败保持旧槽/内容及一字节邻居释放重试、unwind归零；字体派生数据另由预算构造/前缀测试覆盖。复验43982运行中，日志`/tmp/rzpdf-font-selection-arena-boundaries.log`，尚未通过。

共享控制计费实现中：生产字体绑定改为FontSelections索引槽位，不再使用Rc控制块；q/Q保存索引并维护别名数，最后别名释放派生字体租约，空槽复用。槽位Vec采用allocator-api2和现有TableAllocator，以真实分配Layout提前计费及保留旧新容量共存；函数退出统一销毁槽位，不新增unsafe实现。定向会话78318运行中（`/tmp/rzpdf-font-selection-arena.log`），尚未验收；栈元素由Option<Rc>改为Option<usize>，需核对现有精确栈额度测试并补槽位增长/复用/拒绝专项。此前713+41证据属于Rc版本，不覆盖此改动。

字体共享租约当前验收：公共会话45602退出0，nested_adjusted_text15/nested_text_edit6/same_invocation_span3/text_event_range8/text_source9共41 passed、0 failed/ignored/filtered；与713项整组PDF单元的源码指纹同为`a8ecae51efda123f5f77f07d719e94e8588a404b1cc91cf401018a8f3517daa7`，diff空白检查通过。解除本轮源码冻结；下一步处理Rc共享控制分配可准确计费的存储方案及资源副本额度。该结果不是最新完整workspace门禁，不能据此勾选整个AR-03；新实现后须再统一全量验证。

字体共享所有权整组PDF单元终态：76872退出0，`/tmp/rzpdf-font-owner-full-unit.log`为713 passed、0 failed/ignored/filtered、96.37秒，源码指纹`a8ecae51efda123f5f77f07d719e94e8588a404b1cc91cf401018a8f3517daa7`与启动时一致。公共回归45602的nested_adjusted_text已15项通过，当前nested_text_edit，后续三组及整体公共会话仍待终态；源码继续冻结，不宣告完整工作区或AR-03验收。

字体额度对照终态：90497退出0，13项定向通过、0失败（700过滤），包含实际前缀两入口继承字体替换后保留额度相等检查。当前源码指纹`a8ecae51efda123f5f77f07d719e94e8588a404b1cc91cf401018a8f3517daa7`；启动整组PDF单元76872（`/tmp/rzpdf-font-owner-full-unit.log`）及公共nested_adjusted_text/nested_text_edit/same_invocation_span/text_event_range/text_source回归45602（`/tmp/rzpdf-font-owner-public.log`）。两会话未终态，源码冻结，不把此前1063工作区基线计作此次生产变更验收。Rc控制块、资源副本额度及整体预算仍未完成。

继承字体租约定向终态：8889退出0，13项通过、0失败（700过滤）。随后扩展既有budgeted_inherited_font测试，对无q保存别名的局部Tf替换，比较传入/不传入继承字体的返回保留额度完全相同，并覆盖font_before_operation及text_state两条预算入口；会话90497运行中，日志`/tmp/rzpdf-font-inherited-release.log`。尚未完成新版本整组PDF/公共编辑回归，不据此勾选整体预算。

共享字体专项终态：69320退出0，13项字体绑定测试全部通过（700过滤），含新增最后别名释放/32次替换/额度拒绝重试/unwind专项。继而将跨Form继承字体的租约也移入SelectedFont，与局部Tf/gs同样按最后别名生命周期释放；入口仍通过同池absorb检查，不允许异池租约混入。新版本定向会话8889运行中，日志`/tmp/rzpdf-font-inherited-owner.log`，此变更尚未验收；Rc控制块及资源解析副本计费仍待完成，整体预算任务保持未勾选。

字体所有权第一步定向终态：9261退出0，12项字体绑定测试通过、0失败（700过滤），覆盖既有继承字体、q/Q、gs Font、双结果共存与unwind。随后新增SelectedFont共享租约专项：32次替换不累积派生数据额度、最后别名释放、峰值额度加一字节邻居拒绝/释放重试及panic清理；会话69320运行中，日志`/tmp/rzpdf-font-selection-lifetime.log`。专项直接检验共享字体拥有者，不等价于完整前缀资源副本或所有编辑入口验收，待测试终态及整组回归。

字体所有权收敛第一步（待验证）：font_binding新增SelectedFont，将每次Tf/gs新构造字体的租约放进同一个Rc拥有对象，q/Q仅共享该对象；退出的字体最后别名销毁即释放派生数据及租约，最终选中字体才将租约移交返回值。继承字体外层租约、Rc控制分配、资源解析副本额度仍未收敛，未勾选完整任务。定向`cargo test -p pdf --lib --locked page::font_binding::tests`会话9261运行中，日志`/tmp/rzpdf-font-selection-owner.log`；源码指纹`e1b9dd4ec6954efe78c640f00965bca897d0d72e27412b623f4d6c0a3f559cab`。1063项门禁属于改动前版本，不能证明这次变更通过；下一步补反复切换字体的峰值不累积、q/Q存活共存及错误/unwind定向验收。

最新完整回归终态：`cargo test --workspace --locked` 会话11693退出0；`/tmp/rzpdf-mapping-final-workspace.log` 共52个结果组、1063 passed、0 failed/ignored/filtered。验收前后Rust/Cargo指纹均为`5dde4dbf12dd85290ea8fb394301d7d2653d2c0c002b441c49b0245f61416c91`。此次门禁覆盖调用树leaf复用生产变更，解除该轮源码冻结；下方历史“运行中/待终态”已被本记录取代。P3-05、AR-03及整体P0–P5仍未完成；fuzz暂缓，未新增提交或推送。

后续字体预算核对：`page/font_binding.rs`已有不可Clone的BudgetedFont和持有字体的BudgetedEncodedText，q/Q使用Rc共享Font；SimpleFont/Type0已有预算构造，Descriptor FontFile已预算解码并转移租约，不能按下方旧描述从零重写。实际待收敛：font_before_operation_in_scope_tracked中Rc::new共享控制块尚未计费，font_lease汇总所有前缀字体直到返回，已退出字体的数据释放后额度仍保守保留；需将单字体租约与共享所有者绑定，使最后别名释放时归还，而非独立汇总租约。先确定可准确计费的共享存储布局，不能猜测Rc私有分配布局；补多次Tf、嵌套q/Q、gs Font、继承字体、错误/unwind和精确额度重试，保留现有来源认证和事务契约。此核对不勾选整体字体预算任务，也不把1063项旧门禁用于未来变更。

映射优化公共终态及实样对照：98516退出0，nested_adjusted_text15/same_invocation_span3/text_event_range8共26项通过，0失败/忽略/过滤。18133示例编译退出0；当前生产版正向双CID55137及反向3813生成均退出0，tracked peak4116746/2388976与旧样本相同，两个新mapping.pdf均与旧edit.pdf cmp逐字节相同、qpdf无结构错误。已启动最新完整workspace11693（/tmp/rzpdf-mapping-final-workspace.log），源码继续冻结在5dde4dbf12dd85290ea8fb394301d7d2653d2c0c002b441c49b0245f61416c91；待终态，不把旧1061门禁用于此次生产变更。

映射优化整组PDF单元终态：14384退出0，/tmp/rzpdf-mapping-full-pdf-unit.log为712 passed、0 failed/ignored/filtered、92.21秒，含最新映射身份及性能预算测试；Rust/Cargo指纹5dde4dbf12dd85290ea8fb394301d7d2653d2c0c002b441c49b0245f61416c91未变。80360过滤命令实际共3项（两映射测试及名称匹配的unicode_lookup_checks_selected_mapping_against_maxp）通过，不能称其只有两项结果。98516公共nested_adjusted_text已15项通过，当前same_invocation_span仍运行，text_event_range及完整工作区新版本门禁未终态，不宣布整体优化验收。

映射冻结版本整组PDF单元启动：14384（/tmp/rzpdf-mapping-full-pdf-unit.log），Rust/Cargo指纹5dde4dbf12dd85290ea8fb394301d7d2653d2c0c002b441c49b0245f61416c91与定向/公共启动时一致。80360最新两项映射专项退出0，含错误叶提示回退/偏移与父链拒绝及宽树重复测量预算；公共98516与整组14384仍待终态，不标记整体门禁通过。

映射优化公共回归启动：nested_adjusted_text、same_invocation_span、text_event_range同命令测试，日志/tmp/rzpdf-mapping-public-regression.log；覆盖嵌套/同链/跨事件公开编辑来源与共享隔离。selected_mapping_最新定向仍运行，未据启动认定通过；暂冻结Rust/Cargo防止这些门禁混版，先等待定向与公共终态，再补全量门禁或发现的问题。git diff --check通过。

映射优化测量终态19573退出0：同样width1/8events=23us、width128/1024events=160us、width1024/8192events=1256us；较旧单次1630/99157us约10.2/78.9倍，非统计基准/硬承诺。保留内存64/8192/65536字节和含七字节邻居峰值71/8199/65543均不变，精确/相邻额度重试通过。该结果不含后补身份测试；已启动最新selected_mapping_测试组，日志/tmp/rzpdf-mapping-provenance-suite.log。公共与全量新生产版回归尚待执行，不能沿用优化前1061项门禁证明此次变更。

映射优化身份专项补充：selected_mapping_leaf_hints_do_not_replace_provenance_checks新增四变体，叶顺序交换/越界提示必须回退得到原映射，调用byte offset更改/自循环父链必须按完整来源查找失败，七字节相邻租约保留且临时映射释放。尚未运行；当前19573为已编译的优化性能专项，后补测试不在其二进制内。git diff --check通过；待19573终态后执行最新映射测试组及公共回归。

映射基线36922退出0：width1/8events=15us、width128/1024events=1630us、width1024/8192events=99157us，映射保留64/8192/65536字节，独立池peak含七字节邻居71/8199/65543，精确额度/相邻拒绝/释放重试通过。随后map_to_tree增加既有leaf复用：只在叶数等于事件数且逐层反向完整FormInvocation匹配并准确终止合法root0时直接使用，不新增索引分配；其它树保留原通用扫描语义。优化定向日志/tmp/rzpdf-mapping-leaf-reuse.log运行中，仍需外来树/路径/offset/根终止负例、完整单元及公共回归，不标记性能任务完成。

映射性能基线测试新增：selected_mapping_wide_and_repeated_measurement_baseline构造宽度1/128/1024、每叶八次循环重复的原始选区，分别记录map_to_tree耗时、独立映射池tracked peak/retained，验证映射等于原tree.leaves、输入所有者不变、七字节邻居保留、精确额度与相邻一字节拒绝/释放重试、全部释放。生产算法未修改；36922定向运行（/tmp/rzpdf-mapping-baseline.log）待终态，时间不作为硬阈值/复杂度证明，后续优化须在同样样本复测并补身份/路径负例。

调用树性能当前源码复核：from_paths已经使用预算化(parent,operation_index)前缀哈希表并保留完整FormInvocation冲突校验、连续路径复用；旧待办“from_paths逐来源/逐节点扫描”是历史描述，不再据此重做索引。实际剩余扫描位于SpanSelection::map_to_tree（span_tree.rs约130行），每事件每层用tree.nodes.iter().position全arena搜索，宽树/重复事件仍可能放大。下一步为此映射测量建立独立证据，再利用from_selection已有leaf映射或预算化索引；必须验证完整路径/根终止/身份及byte offset，不能仅凭叶ID数量相同接受外来树，保留missing_tree失败/邻居预算/回滚语义，评估新增索引与输出共存额度。生产源码未修改，现有710单元/1061工作区证据不包含未来优化。

反向字体/视觉阶段：66059退出0，/tmp/rzpdf-reverse-span-fonts.log四TTF源GID/闭包/轮廓/指令/度量/全局边界/校验和/保留表/许可及三字体负例通过，新增é/Å两复合字形保留。按PDF技能渲染并实际逐页查看三张96dpiPNG，未见缺字/重叠/裁切，补偿竖排B尾段保留原首B空位。校验器--reverse正式接入--fonts并检查主源一致，联合命令日志/tmp/rzpdf-reverse-span-integrated.log待终态；PDF字典源度量仍待补，反向场景暂不勾选。

反向坐标负例终态：check_reverse补字体/line字符所有权计数，并为首源与尾源分别增加wmode/dir/字体/字号扰动，另验证文字、页数、补偿尾段、第三调用、NaN及全局平移拒绝。95571退出0，/tmp/rzpdf-reverse-span-negatives.log为14反向负例及源绝对坐标通过，git diff --check通过。仍缺反向PDF字典源度量/四抽取字体/渲染目视，不据坐标与负例宣称P3-05或完整反向场景完成。

反向混合生成进度：46442示例编译退出0，36066 Arial水平→NotoSC竖排B尾源双层三页生成退出0，tracked peak2388976，文件output/pdf/rzpdf-latin-subset-horizontal-vertical-nested-span-edit.pdf。check-subset-span新增独立--reverse PRIMARY TAIL坐标入口，按源hmtx水平24pt/1Tc及尾vmtx/glyf原点18pt/2Tc计算各段绝对位置；py_compile通过。此入口暂不带负例/字典/字体验证且与旧选项互斥，不能据代码称完整反向验收；后续补独立负例、四字体/字典及逐页渲染。

反向混合模式开始：edit_subset_span新增独立--tail-vertical（必须配--cid-tail），尾源显式使用Identity-V注册，首源继续由原--vertical控制；任一竖排时三个调用分列。示例编译46442，日志/tmp/rzpdf-reverse-span-build.log，待终态。后续固定Arial水平首源→NotoSC竖排B尾源，尾推进为源vmtx×18pt-2Tc，绝对原点需源hmtx/2及glyf yMax+tsb；校验器必须分别验证首源Identity-H/W及尾源Identity-V/W/W2，补两侧方向和反向补偿负例，再完成严格重开/共享隔离、四字体与逐页渲染。现有--vertical校验器不能直接证明反向模式，不勾选此场景。

双CID混合视觉/回归阶段：按PDF技能完成NotoSC竖排→Arial水平双层样本三页96dpi渲染并逐页实际查看，未见缺字/重叠/裁切；补偿页水平尾段留出原首B推进，第三列保持原位。四TTF已抽取至tmp/pdfs/vertical-cid-span，联合字体校验83631（/tmp/rzpdf-vertical-cid-span-fonts.log）仍运行，不能据已启动称字体通过。30337原单层纯竖排与双层Courier混合模式XML/字典/负例回归退出0；双CID场景仍待字体终态再勾选。

双CID混合竖排推进：79432生成NotoSC竖排→Arial水平双层跨调用样本退出0，output/pdf/rzpdf-cjk-subset-vertical-cid-nested-span-edit.pdf，日志/tmp/rzpdf-vertical-cid-span-generate.log。check_vertical_pdf扩展可选尾源，分别校验三个Identity-V的W/W2及TAILAA Identity-H的源W并拒绝水平W2，补尾编码/宽度/意外W2三字典负例；不漏查第四个Type0。qpdf无结构错误，独立校验19646日志/tmp/rzpdf-vertical-cid-span-independent.log；抽取四字体、渲染目视及校验器旧路径回归仍待执行，不勾选双CID场景。

混合书写跨调用推进：69259生成NotoSC竖排首源/Courier水平尾源双层Form三页退出0，tracked peak4036718，文件output/pdf/rzpdf-cjk-subset-vertical-mixed-nested-span-edit.pdf。校验器按各line实际尾字符区分wmode/dir，后缀补偿使用水平尾源18pt/2Tc推进，首源/第三列继续用源vmtx竖排绝对布局，并保留独立尾字体/字号/推进负例。qpdf结构无错误；独立校验日志/tmp/rzpdf-vertical-mixed-span-independent.log，后续需抽取字体、逐页渲染及纯竖排/旧水平校验回归，尚不勾选混合模式验收。

竖排跨调用绝对坐标补强终态：73314单层字体/字典联合校验退出0，三TTF语义/保留表/许可/校验和及三字体负例通过。校验器新增源hmtx/vmtx/glyf原点、24pt/Tc及三列固定变换的逐字符绝对坐标和整体平移负例；首轮漏计固定MuPDF stext下沿em锚点的24pt而失败，修正校验公式后69725退出0，单/双层各十一XML负例及四字典负例通过，日志/tmp/rzpdf-vertical-{span,nested-span}-absolute.log。未修改引擎/PDF，沿用已逐页查看的六页渲染；此证据仅针对固定NotoSC/em锚点场景，不推广到任意字体/shaping/混合模式。

单层竖排样本阶段验收：77678生成退出0，tracked peak4028732，qpdf无结构错误，三页96dpi已按PDF技能逐页查看无缺字/重叠/裁切，补偿空位符合后缀保持。三TTF已抽取至tmp/pdfs/vertical-span，联合校验73314（/tmp/rzpdf-vertical-span-independent.log）运行中；尚不勾选单层验收。检查发现现有跨调用坐标校验偏重相对关系，后续加入源hmtx/vmtx/glyf推导的绝对首字符及列坐标，并加入整体平移负例；先前双层证据只按实际相对坐标范围解释，不推广为绝对布局全部验证。

真实NotoSC双层竖排跨调用首阶段：81856生成退出0，tracked peak4031547；qpdf结构检查无错误，79481独立XML检查退出0，精确三页文字/字体/字号、竖排方向、源vmtx推进、补偿后缀及第三调用位置和十XML负例通过。按PDF技能渲染96dpi三页并逐页实际查看，无缺字/重叠/裁切，补偿第二列留空保持旧后缀位置。文件output/pdf/rzpdf-cjk-subset-vertical-nested-span-edit.pdf，XML/PNG/三TTF在tmp/pdfs/vertical-nested-span；抽取字体独立复核36980（/tmp/rzpdf-vertical-nested-span-independent.log）待终态。尚未验收竖排字典源度量、单层/混合模式及完整矩阵，P3-05保持未完成。

竖排跨调用独立校验扩展：示例编译71238退出0；check-subset-span.py新增--vertical SOURCE，核对wmode/dir、源vmtx归一化推进及Tc符号、自然后缀位移与补偿位置，新增方向/书写模式/源推进负例，py_compile通过但未据此宣布语义通过。暂明确拒绝竖排混合尾源校验，不沿用水平公式。已启动NotoSC双层Form跨调用三页生成，输出output/pdf/rzpdf-cjk-subset-vertical-nested-span-edit.pdf，日志/tmp/rzpdf-vertical-nested-span-generate.log；待严格重开、独立提取校验/负例、字典/字体及渲染验收。

真实竖排跨调用扩展开始：edit_subset_span新增显式--vertical选项，首源注册Identity-V，三个调用改为横向分列以避免竖排列重叠；保留--nested双层及水平旧路径，混合尾源仍保留其自身水平模式，不静默改尾字体。示例编译71238（/tmp/rzpdf-vertical-span-build.log）待终态。后续必须扩展check-subset-span.py的竖排源度量/方向/坐标与负例，再完成NotoSC单层/双层及混合模式样本的严格重开、qpdf、抽取字体和逐页渲染；现有水平校验器不能用于声称竖排通过，P3-05保持未完成。

W/W2预算收口验收：最终PDF单元47331退出0，/tmp/rzpdf-metric-final-pdf-unit.log为710 passed、0 failed/ignored/filtered、95.03秒，覆盖最终水平/竖排公开边界与全部度量所有权测试。生产代码与1061项工作区通过版本一致，之后只有此公开边界测试扩展。聚合checked_mul/显式Limits、固定内外数组capacity及数值字符串capacity、公开注册与PreparedSubsetEdit发布生命周期、精确/少一字节/相邻额度/错误及panic回滚释放重试、真实Arial/Noto固定输出不变均已有对应证据，勾选此预算子任务；完整字体解析和文档常驻成本仍按已声明边界排除，不声称整个字体流程/RSS全部池化。P3-05总项和其它未完成任务保持不勾选，fuzz继续暂缓。

竖排公开聚合预算最终专项95089退出0：/tmp/rzpdf-metric-vertical-boundary-final.log为1 passed、0 failed、709 filtered，覆盖同一测试中的水平/竖排公开入口独立峰值、精确额度、少一字节、相邻租约保留/释放重试、失败文档字节不变、返回live=0及严格重开；竖排分别检查Identity-V、/W与/W2。生产逻辑未变，完整workspace1061项结果仍对应此前生产版；后补测试的完整PDF单元门禁另行执行，不将专项过滤结果当作全量。

竖排公开聚合预算专项首轮69786退出0：public_subset_registration_budget_boundary_rolls_back_and_retries水平/竖排两分支通过，1 passed、0 failed、709 filtered（定向过滤，不代表全单元门禁）。随后发现原/W子串检查可能被/W2满足，已收紧为含分隔空格的/W键检查；最终专项95089（/tmp/rzpdf-metric-vertical-boundary-final.log）运行中，尚不勾选该验收项。

W/W2预算工作区回归终态：会话52792退出0，/tmp/rzpdf-metric-final-workspace.log共52组、1061 passed、0 failed/ignored/filtered；Rust/Cargo指纹dedc9ca914e3962e8a0c5d65c2502dc60b9c1913908efd8a6687ee090922f2b2与启动时一致，解除此次源码冻结。随后将public_subset_registration_budget_boundary_rolls_back_and_retries扩展为水平/竖排两个公开入口，各自测峰值并验证精确/少一字节/相邻额度/回滚/释放重试及严格重开；新增版本专项69786（/tmp/rzpdf-metric-vertical-boundary.log）待终态，1061结果不证明此后补测试，预算任务仍不勾选。

W/W2预算真实水平/竖排固定复验完成：竖排抽取字体10785退出0，三TTF源GID/闭包/轮廓/指令/度量/边界/校验和/保留表与三字体负例通过。Arial水平跨事件Form样本93516退出0、tracked peak1583210，qpdf无结构错误且与旧样本cmp相同；47223退出0，三页精确文字/坐标/字体与六XML负例、三TTF源语义及三字体负例通过。按PDF技能实际查看新渲染的水平三页96dpiPNG，无缺字/重叠/裁切；加上上一轮竖排三页，两个固定实样复验完成，不推出任意字体/完整竖排矩阵。最新workspace52792（/tmp/rzpdf-metric-final-workspace.log）已启动，冻结指纹dedc9ca914e3962e8a0c5d65c2502dc60b9c1913908efd8a6687ee090922f2b2，期间不改Rust/Cargo，预算任务最终门禁待终态。

预算联合PDF单元终态与真实竖排Form复验：86503退出0，710 passed、0 failed/ignored/filtered、100.07秒；15596生成退出0，tracked peak4020598。新竖排Form跨事件PDF与旧样本cmp逐字节相同，qpdf结构无错误；86786退出0，精确三页文字/字体/字号/源推进坐标、八XML负例、Identity-V/W/W2源度量及四PDF对象负例通过。按PDF技能重新渲染96dpi三页并实际逐页目视无缺字/重叠/裁切，补偿页空位为保留原后续位置。三个字体抽取至/tmp/rzpdf-metric-visual.S2VkQo，独立字体复核10785待终态；水平实样及新工作区回归仍未完成，不据710单元宣布完整门禁。

预算改动真实字体复验启动：当前edit_subset示例编译9886退出0，使用原完整NotoSC源生成独立新路径output/pdf/rzpdf-cjk-subset-vertical-form-events-budget-edit.pdf（15596，日志/tmp/rzpdf-metric-real-form.log），保留此前样本不覆盖。待生成终态后做严格重开/qpdf/Identity-V及W/W2源度量、XML精确坐标/负例、抽取字体和渲染目视。联合PDF单元86503仍运行；这些启动记录不代表实样或最新整组验收完成。

度量所有权生命周期专项终态：76720与76420均退出0，各一项通过，分别覆盖注册后内容失败/指定panic回滚与自动PreparedSubsetEdit准备成功后共享池饱和导致的实际内容发布失败；后者明确assert准备完成、度量租约存活、回滚字节不变和相邻租约保留，并刷新快照走公开接口重试、严格重开B。最新整组PDF单元日志/tmp/rzpdf-metric-lifecycle-consolidated.log已启动待终态，完整真实字体复验/工作区回归仍未完成，不勾选预算收口或P3-05。

自动选区度量所有权定向测试新增：subset_text_edit新增PreparedSubsetEdit真实准备/新字体注册与替换程序构建后，测试包装对象持有共享池全部剩余额度，使commit_page_content_edit后续发布失败；断言已完成准备、度量租约实际非零且仍计费、精确ResourceLimit、文档字节恢复与相邻租约保留，随后刷新快照走公开接口正常重试并严格重开B。专项76420日志/tmp/rzpdf-prepared-metric-lifecycle.log待终态；注册对象外层内容失败专项76720仍待终态，不据源码断言当作测试通过。

注册后内容失败生命周期验证新增：metric_array_tests复用仅测试可见的合成字体fixture，在外层文档事务中成功内部子集注册后断言度量租约仍计费、后续超额保留拒绝，再以无效页内容发布失败检查文档字节恢复；另检查有效页内容写入后的指定panic回滚、相邻七字节额度与正常重试/严格重开。专项76720日志/tmp/rzpdf-metric-owner-content.log待终态。该定向测试覆盖注册对象/外层事务所有权，不等同于自动选区PreparedSubsetEdit后续发布失败；后者仍需补证，不勾选完整预算收口。

度量预算失败所有权专项终态：构造器专项28753退出0，三项通过（705过滤），包含token超限不增加字符串计费、内层完成后外层额度不足释放、精确panic载荷确认且相邻七字节租约保持；公开注册重试79838退出0，一项通过（706过滤），覆盖精确峰值/少一字节/相邻额度失败文档不变与释放重试。日志分别为/tmp/rzpdf-metric-array-failure-tests.log与/tmp/rzpdf-public-metric-boundary-retry.log。新增断言不替代真实文档外层内容失败的定向生命周期验证，源码当前仍需联合复验。

公开字体预算专项首轮49031编译退出101：新增测试闭包遗漏PdfDocument读取器泛型，已改为Cursor<Vec<u8>>；重试日志/tmp/rzpdf-public-metric-boundary-retry.log待终态，首轮不计为通过。

W/W2联合单元终态：75274退出0，/tmp/rzpdf-metric-array-pdf-unit.log为706 passed、0 failed/ignored/filtered、92.33秒，覆盖生产接入与两个构造器额度测试及既有水平/竖排编辑回滚。随后补公开子集嵌入精确峰值/少一字节/相邻租约拒绝及释放重试、失败文档字节不变与严格重开，专项49031待终态；不将706结果用于后补测试。接口文档与content-editing预算范围同步，尚待自动编辑新增生命周期定向断言和实样复验，预算任务/P3-05保持未完成。

W/W2预算生产接入进度：水平W与竖排W2改为固定长度MetricArrayBuilder，聚合对象数/槽位字节预检、数字token限额及实际槽位/字符串capacity入池；非空外层两对象数组同样计费。子集入口传入共享池，EmbeddedCidFontObjects持有度量租约并经已有PreparedSubsetEdit/Fragment所有权链延续至内容发布；公开注册完成后转为文档常驻成本，旧整字体入口使用本地Limits池而非调用者共享池。58797首项专项退出0，但其后补聚合字节预检与第二项外层/相邻额度测试不在该结果内；最新PDF整组单元75274（/tmp/rzpdf-metric-array-pdf-unit.log）待终态。尚需完整公开嵌入/自动编辑边界、error/panic回滚生命周期和实样回归，不勾选预算收口或P3-05。

2026-10-09 竖排工作区门禁终态：会话64670退出0，/tmp/rzpdf-vertical-workspace.log全部52组终态合计1055 passed、0 failed/ignored/filtered（PDF库704、render库38及公共集成/文档测试）。Rust/Cargo集合指纹e38fb345f9b18df560ca86642ce055687943335ef856e82ae837b2f491478325与冻结起点一致，源码冻结解除。此结果不证明后续新改动或完整P3完成。

W/W2预算修复开始：embedded_font新增固定容量MetricArrayBuilder，检查对象数/容量字节/数值token上限，栈缓冲格式化并计入数字字符串实际capacity，合并租约随输出所有权转移。库编译检查70374退出0；边界专项96639仍待终态（测试启动后修正了字符串长度下界断言，需最新源码复验）。尚未接入W/W2生产路径、外层数组和公开/自动编辑所有权链，相关TODO保持未完成，不把1055项旧基线用于新增构造器验收。

本地提交检查点（2026-10-08）：此前提交85ff816保存了跨文本事件编辑、同叶嵌套端点及当时未接入的span_tree原型。后续统一展开选区、预算生命周期及调用树优化尚未提交。清理/索引优化前的完整工作区1005项通过，不代表当前版本已完整验收；当前门禁及范围见下文。最新索引版固定真实水平字体TJ内部边界、混合字体及同链样本独立复验已通过，不代表完整字体矩阵。整体P3及项目未完成，fuzz继续暂缓，来源不明的test.png不纳入提交，不推送远端。

## 当前基线

### 当前收敛任务

字体嵌入预算审查新增缺口：W/W2构造沿用PdfArray::try_add_obj，它只做try_reserve并传播分配失败，不按Limits聚合数值对象字节或将capacity计入OperationBudget。UnicodeCidEncoding的字集限额不等于三倍W2 PdfObject容量限额；源接口也复用旧完整TrueType解析，未声明其所有分配已入池。固定功能/峰值测试不证明嵌入预算完整。工作区门禁后优先补W/W2对象数及字节预检、实际容量所有权与相邻额度/失败释放；本轮不改冻结Rust源码。P0-10及P3完整验收仍保持未完成。

SFNT/复合解析任务证据收口：核对当前实现、704项PDF单元日志及实际字体独立输出后，将复合记录解析复用与静态原GID SFNT写出限定范围勾选；不自动勾选完整字体/字符扩充/竖排状态矩阵或P3-05。check-subset-edit加入竖排/PDF对象检查后，既有拉丁/中文单Tj、TJ、Form跨事件六组水平XML在Python3.11 -O下精确文字/坐标与各六负例回归全部通过。本轮不改Rust/Cargo，工作区64670已通过adjusted_text14项并进入color_workflow，仍待终态。

竖排PDF对象检查联合终态：会话49819退出0，单Tj、TJ和Form跨事件三份PDF的Identity-V/W/W2均与完整源逐CID度量一致，各四PDF对象负例及八XML负例通过。工作区门禁当前704项PDF单元已通过，进入adjusted_text集成；不是全量终态，指纹复核与冻结起点一致。

竖排PDF对象独立检查补强：check-subset-edit新增--pdf，qpdf JSON独立读取Type0/Descendant W/W2，按完整Noto源hmtx/vmtx/glyf yMax核对每CID宽度、负向推进及两维原点，同时核对Identity-V、字集/数组长度与三个字体身份。新增错误编码、推进、纵向原点和多余CID三元组四定向负例；当前单Tj样本已通过，TJ/Form两个样本联合命令会话49819待终态。不改Rust/Cargo，完整工作区64670仍运行，源码冻结保持。

真实竖排TJ/Form跨事件固定独立验收终态：抽取字体会话52742/48182均退出0，六份TTF按完整Noto源的原GID/闭包/轮廓/指令/水平与竖排度量/保留表/边界/校验和/许可声明及各三字体/八XML负例通过；目录`/tmp/rzpdf-vertical-tj-fonts.VaVMSs`、`/tmp/rzpdf-vertical-form-fonts.fHkiN2`。两组原/自然/补偿六页96dpi均实际逐页目视，无缺字/重叠/裁切；结合严格重开、TJ数值/注释保留、qpdf及源vmtx精确纵向坐标，本固定TJ跨项和单层Form跨事件实样验收完成。不代表多层跨调用/混合书写模式或全部竖排字形替换；工作区64670仍待终态，Rust/Cargo冻结保持。

竖排TJ/Form跨事件结构坐标终态：生成78866/98120均退出0，峰值3232667/4019318，示例严格重开精确文字及原TJ数值/注释保留检查通过。两份PDF经qpdf无结构错误、MuPDF stext和Python3.11 -O竖排检查，精确三页文字/字体/字号/前缀/补偿后续与源vmtx自然纵向位移、各八XML负例通过。尚待抽取字体/六页渲染目视，不宣称完整TJ/嵌套竖排矩阵。工作区门禁已完成构建并进入font_data测试，仍待终态，冻结保持。

竖排真实矩阵扩展进行中：工作区门禁64670仍运行，本轮复核Rust/Cargo指纹与冻结起点一致。使用已构建edit_subset示例（--vertical --cjk）启动TJ跨项和Form跨事件两个三页样本，不改Rust/Cargo；生成会话78866/98120，日志`/tmp/rzpdf-vertical-real-{tj,form-events}.log`。仅样本执行进度，尚未完成独立精确文字/源竖排度量坐标/抽取字体/逐页目视，不勾选完整TJ/嵌套竖排矩阵，也不沿用单Tj实样证明新路径通过。

当前工作区门禁源码冻结指纹：`e38fb345f9b18df560ca86642ce055687943335ef856e82ae837b2f491478325`（rg列举非忽略Rust/Cargo文件，排序后逐文件SHA256再汇总）。工作区会话64670、日志`/tmp/rzpdf-vertical-workspace.log`；终态时复核指纹，冻结期间只进行文档/独立输出验收，不改Rust/Cargo，不重启仍活跃的测试进程。

竖排联合PDF单元终态：会话24242退出0，`/tmp/rzpdf-vertical-consolidated-pdf-unit.log`704 passed、0 failed/ignored/filtered、91.58秒，包含当前竖排度量/源接口/Identity-V/W2整字体与子集/自动补偿替换/两页共享/tiny/error/panic回滚及刷新重试断言。真实Noto固定三页结构/精确纵向坐标/抽取字体/负例/目视也有独立证据；此记录不是工作区全量。已启动`cargo test --workspace --locked`，日志`/tmp/rzpdf-vertical-workspace.log`，验收期间冻结Rust/Cargo文件；尚待终态，完整竖排状态/多层混合矩阵及P3-05仍未完成。

能力摘要同步：README不再笼统声称没有子集，区分旧整字体固定字集与显式静态子集入口，并列出新增Identity-V/W2入口；docs/content-editing顶部补当前竖排实现、真实固定样本证据及无shaping/替代字形/自动排版限制。当前PDF单元会话24242仍待终态，本轮只改文档，不沿用旧704结果证明新增panic或竖排生产改动已联合通过。完整竖排矩阵、P3-05和总体计划保持未完成。

竖排共享/错误回滚终态与panic联合门禁：会话88616退出0，六项通过；两页共享AA/A、原字体对象不变、tiny拒绝、外层指定错误回滚及刷新重试/预算释放断言通过。新增外层panic在成功内层替换后触发，catch_unwind必须识别精确指定payload并核对保存字节/预算恢复，不能将内层unwrap错误当作成功panic测试。当前完整PDF单元回归`/tmp/rzpdf-vertical-consolidated-pdf-unit.log`已启动，覆盖本轮竖排生产实现及新panic断言，尚待终态；旧704全量早于这些改动，不冒充最新完整验收。

竖排预算测量终态与共享/回滚补验：会话16190退出0，六项通过；公共替换实际峰值2109084字节（另外三个拒绝场景0），<8MiB上限和最终live_bytes=0通过，前64KiB/1MiB失败记录保留而不冒充通过。新增两页共享原竖排子集：tiny拒绝保存字节/池不变，外层指定错误在成功内层编辑后回滚，刷新快照正常重试，严格重开AA/A及原字体对象不变；`/tmp/rzpdf-vertical-sharing-rollback.log`待终态。新断言不计入此前六项通过；panic、多层/混合状态、完整PDF单元/工作区门禁仍待完成。

真实竖排抽取字体独立终态：会话79745退出0，三份TTF按完整Noto源检查原GID/字形闭包/轮廓/指令/水平及竖排度量/保留表原字节/边界/校验和/许可声明通过，三个字体负例及八XML负例通过。结合前轮严格重开、qpdf、源vmtx精确纵向位移与三页目视，固定真实水平字形不替换的Identity-V输出检查已齐。合成自动替换预算专项仍未通过，因此不将输出检查代替公共语义/预算门禁或宣称完整竖排支持；测量复验16190待终态。

竖排预算复验仍失败：会话84579退出101，仍为公共替换operation live bytes（5通过/1失败），1MiB不能覆盖该流程，前一“扩大后待验收”不表示成功。改用16MiB测量池并新增实际峰值输出及<8MiB上限断言，最终live_bytes=0仍必须通过；不是移除预算限制。当前测量复验日志`/tmp/rzpdf-vertical-auto-edit-measured.log`待终态。真实样本抽取三字体目录`/tmp/rzpdf-vertical-fonts.EAvhik`，独立检查会话79745待终态，尚不勾选竖排验收。

真实竖排结构/坐标/视觉验收进度：rzpdf-cjk-subset-vertical-edit.pdf经qpdf无结构错误，MuPDF stext为wmode=1/dir=0 1；check-subset-edit新增--vertical SOURCE，按源vmtx/unitsPerEm计算好替换两个你的24pt自然纵向位移，精确三页文本/字体/字号/前缀/补偿后续位置与各八XML负例在Python3.11 -O通过。三页96dpi实际逐页目视，无缺字/重叠/裁切；补偿页空位为保留原后续位置。尚未抽取三字体做独立源语义/保留表校验，复验专项84579仍待终态；不勾选完整竖排实样或P3-05。

竖排自动替换专项失败与复验：会话76929退出101，5通过/1失败；新增公共替换在旧64KiB语义测试池下返回operation live bytes，不是竖排映射断言失败。该测试包括新增完整字体解析/CMap/编辑规划且不定义精确预算上限，扩大为1MiB，保留失败不变及最终live_bytes=0断言；预算实现未放宽。重试日志`/tmp/rzpdf-vertical-auto-edit-retry.log`待终态，不能把失败版计为通过。edit_subset新增--vertical，真实Noto三页原/自然/补偿样本日志`/tmp/rzpdf-vertical-real-edit.log`已记录生成峰值3231794；还需独立字体/坐标/逐页目视，不声明竖排验收完成。

竖排自动子集注册接入（待测试/实样）：subset_text_edit共用构造器根据原字体WritingMode选择内部with_mode注册，新字体保持Identity-V/W2，替换推进复用with_replacement_font及各字体真实竖排度量，不再固定水平注册。加入公共竖排A→AA补偿替换及严格重开精确文本断言，库检查会话56189退出0；尚未声明运行/真实竖排验收通过。此前公共竖排子集专项会话38898退出0六项通过，只覆盖自动替换接入前版本。当前专项日志`/tmp/rzpdf-vertical-auto-edit.log`已启动待终态；需要非零竖排推进、真实新字、自然/补偿坐标、共享资源/error/panic及独立视觉矩阵和新全量门禁。

显式竖排子集入口接入（待专项）：新增embed_truetype_vertical_subset_font及内部with_mode共用构造器，旧公开/内部子集入口保持水平；复用原GID闭包/SFNT/权限/预算写出Identity-V/W2，不合成缺失源度量。库检查会话33653退出0；新增公共子集写出/严格重开提取A、缺对/截断保存不变及预算释放断言。此前整字体竖排专项会话77808退出0，六项通过，不能证明后加子集路径；新`/tmp/rzpdf-vertical-public-subset.log`专项已启动待终态。真实竖排样本/新PDF全量/工作区门禁及自动竖排替换仍待完成。

显式竖排整字体嵌入接入（待验收）：新增embed_truetype_vertical_font，旧embed_truetype_font及子集内部仍水平；显式入口使用同源vhea/vmtx与checked_glyph_locations构造逐CID W2，Type0写Identity-V，无源度量明确拒绝，不伪造默认值。库cargo check日志`/tmp/rzpdf-vertical-embedding-check.log`退出0；新增公共写出Identity-V/W2、严格重开提取A与缺对/截断失败保存字节不变断言。三元组专项会话93754退出0，六项通过，证明嵌入接入前版本；当前公共嵌入专项`/tmp/rzpdf-vertical-public-embedding.log`已启动待终态。还没有竖排子集入口、自动竖排替换、真实竖排度量/像素独立验收或新workspace门禁。

竖排PDF三元组实现：VerticalMetrics::pdf_metric按已检查同源horizontal advance与glyf yMax构造[w1y,v1x,v1y]，1000-em缩放、负向推进、横向半宽原点、yMax+有符号bearing；i32求和防止i16溢出，新增1024-em精确小数/负bearing/上界/非法units断言。源接口专项会话32935退出0（六项通过），只覆盖三元组加入前版本。新版三元组专项`/tmp/rzpdf-vertical-pdf-triples.log`已启动待终态。尚未写入W2或开放竖排子集编辑。PDF三元组依据Adobe公开ISO32000-1 Table117，字体纵向原点依据OpenType vmtx；不应用BASE/shaping替换。

竖排源接口接入待验收：新增TrueTypeProgram::checked_vertical_metrics借用读取SFNT配对表；两表均无返回None，缺对/非法表返回错误，不合成PDF默认值或应用竖排替换。原子集竖排表测试新增源接口的无表、完整三GID以及缺对/截断断言。前一PDF单元会话95962退出0，日志`/tmp/rzpdf-vertical-preflight-pdf-unit.log`证明接口加入前版本，不自动证明新增源码。当前源接口专项`/tmp/rzpdf-vertical-source-interface.log`已启动，待终态；下一步Identity-V/W2与实际竖排编辑仍待实现。

竖排度量预检专项终态：会话5691退出0，`/tmp/rzpdf-vertical-metrics-integrated.log`六项通过、0失败、698过滤，包含新增两项解析器、原GID竖排表保留/缺对截断拒绝及既有W2回退/竖排缩放/Identity-V编码回归。真实静态Noto源vhea版本00011000、24..34字段全零，符合新增预检；这只是源字段核对，不是竖排输出验收。已启动当前PDF单元联合回归`/tmp/rzpdf-vertical-preflight-pdf-unit.log`，生产预检变更尚未新全量workspace门禁。下一步向嵌入路径提供借用竖排度量与PDF W2构造，保持竖排自动子集显式拒绝直至真实端到端验证。

竖排专项运行切换：旧解析器专项会话96814已退出0；它只证明启动时旧单测试，不计入新增版本边界/生产接入。已启动当前`cargo test -p pdf --lib --locked vertical`，日志`/tmp/rzpdf-vertical-metrics-integrated.log`，同时复验现有名称含vertical的度量/子集/推进测试；尚待终态。

竖排度量预检接入（待新版验收）：build_subset的vhea/vmtx配对预检改用共用VerticalMetrics，新增1.0/1.1版本、全部表头截断、保留字段及零/越界计数回归；原尾部复用/负bearing/数据截断测试保留。旧专项会话96814的测试二进制仍确认存活，不能用其结果证明本轮后补源码；待其终态后启动当前版本专项与原子集竖排表保留回归。尚未接入Identity-V/W2或自动竖排替换，完整workspace旧门禁也不证明这项生产变更。

竖排实现第一步：确认公共嵌入仅写Identity-H/水平W，自动子集入口仍显式拒绝竖排，不能删除拒绝就宣称支持。新增零分配借用VerticalMetrics解析器，按OpenType vhea/vmtx读取advanceHeight及有符号topSideBearing，尾部字形复用最后advance但保留各自bearing，检查版本/保留字段/计数/截断/GID；定向测试日志`/tmp/rzpdf-vertical-metrics.log`待终态。尚未接入TrueTypeProgram或Identity-V/W2写出/竖排编辑/独立渲染，不勾选竖排支持。规范来源：https://learn.microsoft.com/en-us/typography/opentype/spec/vmtx 和 /vhea。

真实双CID源固定验收完成：八份实际抽取字体按各自完整源（EDITAA/AB/AC首源，TAILAA尾源）独立核对原GID/闭包/轮廓/指令/度量/边界/校验和/保留表/许可声明，中英检查会话31066/15264均退出0，各三字体负例和十XML负例通过；抽取目录`/tmp/rzpdf-cid-latin-fonts.d6ESfD`、`/tmp/rzpdf-cid-cjk-fonts.M3rF6Z`。六页MuPDF 96dpi渲染均实际逐页目视，无缺字/重叠/裁切，补偿BB保留原起点。结合严格重开、原父子Form不变、qpdf及尾源真实推进，固定Arial→NotoSC B、NotoSC→Arial B双层水平Tj矩阵完成；不是任意字体、复杂shaping、竖排或完整P3-05。

真实双CID源结构/坐标终态：生成会话43489/88509均退出0，峰值1978136/3064158；严格重开及原父子Form隔离断言通过。两份cid-span PDF经qpdf与MuPDF stext、Python3.11 -O检查通过，独立由尾源FontTools hmtx/unitsPerEm计算18pt+2Tc的B推进，核对自然尾段精确位移及补偿位置，各十XML负例均拒绝；联合会话82505退出0。原Courier混合中英十负例回归通过。检查器新增四抽取字体/TAILAA独立尾源支持，但尚未实际抽取检查或逐页目视，不声明完整双CID矩阵完成。

真实双CID源混合实现（待验收）：edit_subset_span新增互斥`--cid-tail=FONT.ttf`与Courier `--mixed-tail`，固定配对Arial首段/NotoSC尾段及反向，尾段独立TAILAA子集BBB、18pt/2Tc、双字节端点，保留原自然/补偿三页及父子Form隔离断言。仅示例改动，不是通用字体选择接口；完整字体现有源路径由调用方提供。编译检查日志`/tmp/rzpdf-cid-tail-check.log`，生成日志`/tmp/rzpdf-cid-span-{latin,cjk}.log`；尚待运行终态及四份抽取字体/双源度量/视觉独立验收，不勾选双CID矩阵。

混合父Form隔离终态：中英生成会话48884/62506均退出0，新增原尾父Form精确不变断言和严格重开均通过；峰值1598289/2984130。`cmp`联合命令退出0，两个mixed-parent-span PDF分别与之前完成抽取字体/坐标/十负例及逐页目视的mixed-span PDF逐字节一致，故输出验收证据适用，无需把相同输出冒充新的视觉矩阵。生产实现未改。

混合矩阵隔离断言补强：独立XML检查新增将Courier尾段字体伪装为Arial及字号改为24的两个定向负例，中英各十XML负例通过；旧同字体中英七负例回归通过。示例新增尾段父Form原对象快照及编辑后精确不变断言，补齐此前仅验证尾叶Form的缺口；生成改用mixed-parent-span新文件，保留已验收mixed-span输出。检查日志`/tmp/rzpdf-mixed-parent-check.log`，中英生成日志`/tmp/rzpdf-mixed-parent-{latin,cjk}.log`；新增父对象断言尚待生成终态，不沿用旧样本证明新断言通过。

真实混合尾字体固定验收完成：两份mixed-span PDF各三份抽取TTF通过Python3.11 -O源GID/轮廓/组件/指令/度量/边界/校验和/原样保留表/许可声明检查，各三字体负例和八XML负例通过；中文会话26400退出0，抽取目录`/tmp/rzpdf-mixed-latin-fonts.1CvYfI`、`/tmp/rzpdf-mixed-cjk-fonts.RzLBB3`。MuPDF 96dpi六页均实际逐页目视，无缺字/重叠/裁切；补偿页BB起点右移为保留原末端位置，自然页BB回到原行起点。结合严格重开、原Form隔离和qpdf，此固定双层水平Tj的Arial/Noto→Courier混合范围完成；不代表竖排、两种嵌入CID字体之间的混合、自动排版或完整P3-05。

真实混合尾字体结构/坐标专项终态：中英生成会话80985/1066均退出0，跟踪峰值1598289/2984130字节；示例严格重开及原共享/尾Form不变检查通过。两份mixed-span PDF经qpdf无结构错误、MuPDF stext和Python3.11 -O独立检查，精确文字/字体/字号/有限坐标/补偿后缀/第三调用不变均通过；自然尾段位移精确-12.8pt（Courier600单位×18pt＋2Tc），新增伪造尾段位移负例与原七项均正确拒绝。同字体双层中英旧检查器回归也通过。尚待实际抽取字体和六页渲染目视，不勾选完整混合实样。

真实混合尾字体样本实现（待验收）：edit_subset_span新增可组合--mixed-tail，第二调用使用Courier/18pt/2Tc/单字节BBB，第一和第三调用保留真实子集/24pt/1Tc/双字节字码；可叠加--nested，两端按各自字码偏移选择，严格重开预期及原尾Form不变检查已接入。编译初版通过，移除新增unwrap后复验；中英生成日志`/tmp/rzpdf-mixed-span-{latin,cjk}.log`。尚未完成生成、独立混合字体坐标检查、抽取字体及视觉验收，不勾选真实混合矩阵。

SFNT独立检查补强（2026-10-09）：`check-cjk-subset.py`新增hhea/hmtx/maxp/OS2/post及hint/竖排/STAT保留表的存在性和源原始字节比较；`check-subset-edit.py`新增修改hhea ascent后修复校验和的负例，要求精确拒绝于preserved table bytes而非损坏校验和。双层Arial/Noto各三份抽取字体在Python3.11 -O下复验均退出0，各三个字体负例及七XML负例通过。仅检查实际源中存在的表，不证明这些字体没有的可选表或竖排编辑支持。生产Rust未改；完整字体矩阵和P3-05仍未完成。

当前状态摘要（2026-10-09，优先于下方历史进度）：真实双层水平Tj跨调用样本已完成独立验收；P3-05和整体P0–P5仍未完成。最新完整工作区证据为1048项，后补PDF单元联合证据为702项，不能合并为新全量。下一步核对权限预检/闭包/SFNT任务与现有实现，消除过时待实现描述，再补真实混合字体矩阵。fuzz暂缓；不新增提交或推送。下方按阶段记录的“运行中/待终态”不代表当前仍有活动进程。

真实双层字体验收终态：MuPDF从两份nested-span PDF各抽取三份TTF；Python3.11 -O `check-subset-span.py --fonts SOURCE DIR` 中英均退出0。Arial原字体1标量/2保留GID，新字体3标量/7保留GID/2复合；Noto原字体1标量/2保留GID，新字体2标量/3保留GID。独立源GID映射、轮廓/组件/指令、度量、边界、校验和及许可声明保留检查通过，各两字体负例与七XML负例通过。抽取目录`/tmp/rzpdf-nested-latin-fonts.5QPKp3`和`/tmp/rzpdf-nested-cjk-fonts.H8TRTv`。结合此前严格重开、共享父子Form不变、qpdf及六页目视，固定双层实样已验收；不是完整字体矩阵或许可法律授权。

真实两层跨调用结构/文本/视觉进度：生成会话50106/80043均退出0，跟踪峰值1593119/2978961字节；严格重开与共享父子Form原对象不变。`output/pdf/rzpdf-{latin,cjk}-subset-nested-span-edit.pdf` 均qpdf无结构错误，MuPDF stext经Python3.11 -O `check-subset-span.py`精确文字/字体/24pt/前后坐标及各七负例通过；六页96dpi渲染均实际逐页目视，无缺字/重叠/裁切，补偿页第二行前方空位为保留原末端位置。尚未对新两层PDF抽取字体做独立语义/校验和/许可检查，不勾选完整实样子项。

真实多层样本生成进度：`cargo check -p pdf --example edit_subset_span --locked` 会话74319退出0。使用既有本机Arial和已提取静态NotoSC源生成`output/pdf/rzpdf-{latin,cjk}-subset-nested-span-edit.pdf`，分别会话50106/80043，日志`/tmp/rzpdf-real-nested-span-{latin,cjk}.log`；当前生成仍运行，中文共用构建锁排队，不另起重复进程。qpdf、MuPDF、Python3.11可用；尚未独立提取/字体检查或逐页目视，不声明多层实样验收通过。

真实多层跨调用样本实现：`edit_subset_span`新增可组合`--nested`/`--cjk`选项，原单层CLI保留；两层共享父子Form、三次调用与原页/自然/补偿三页对照，检查源调用深度与编辑后原父子对象不变，严格重开文字沿用。编译检查会话74319待终态；尚未生成或独立验收多层中英样本，必须完成qpdf/独立提取坐标/字体检查及逐页渲染目视后才勾选实样矩阵。

最新PDF单元联合基线：`cargo test -p pdf --lib --locked` 会话25218退出0，`/tmp/rzpdf-subset-consolidation-pdf-unit.log` 为702 passed、0 failed/ignored/filtered，耗时85.48秒，包含本轮新增五项聚合额度/复合flags/共享GID非BMP/公共表策略及既有嵌套矩阵。源码指纹`1e5847e817cdeea1cb23431709bc96ecb20802ed42fc2c7fc5149c09444773ec`，差异检查通过。生产代码自1048项工作区基线后未改，仅测试和文档变化；不能把702项当作最新完整工作区验收。下一步收口范围规划器退出条件与真实字体剩余矩阵，P3-05及整体目标保持未完成。

公共子集表策略专项终态：会话33164退出0，`/tmp/rzpdf-subset-public-table-policy.log` 为1 passed、0 failed、701 filtered，12场景通过；未知/变量/颜色表不会被省略opt-in放行，GSUB显式省略及DSIG自动省略准确报告，拒绝保存字节/预算不变。此前新增聚合额度、复合flags、共享GID/非BMP专项也均已通过；生产实现未变，接下来以完整PDF单元回归联合验收新增测试，旧1048工作区基线仍只证明当时版本。

公共子集表策略补验（待终态）：新增 `public_subset_table_policy_is_explicit_and_rejections_are_atomic`，12场景覆盖ZZZZ/fvar/gvar/COLR/GSUB/DSIG各两种显式省略策略；未知/变量/颜色表不得因opt-in被放行，GSUB仅显式省略可接受，DSIG省略报告必须准确，拒绝时保存字节和预算不变。会话33164，日志`/tmp/rzpdf-subset-public-table-policy.log`，尚未声明通过；不代表这些不支持的字体表已实现。

共享GID公共编辑专项终态：会话61813退出0，`/tmp/rzpdf-subset-shared-gid-public.log` 为1 passed、0 failed、700 filtered。三标量共享原GID1在子集cmap与独立CID映射中保留；未选GID空轮廓、整体校验和、公共插入B𠀀后严格重开AB𠀀A/共享页AA、旧句柄不可编码新字与预算释放均通过。CID原GID映射子项据此核对完成；不是GID重编号实现或真实非BMP视觉验收，P3-05仍未完成。

共享GID公共编辑补验（待终态）：新增 `shared_gid_non_bmp_subset_survives_assembly_and_public_edit`，静态源A/B/𠀀各映射原GID1，CID仍各自独立；重建子集后核对三标量cmap、CIDToGIDMap、未选GID2空轮廓及整体校验和。公共单字符串子集编辑在共享A字体页插入B𠀀，要求严格重开AB𠀀A/另一页AA、旧字体句柄仍拒绝新字符及预算回基线。会话61813，日志 `/tmp/rzpdf-subset-shared-gid-public.log`，尚未声明通过；合成字形不替代真实字体视觉矩阵。

复合组件专项终态：会话 72341 退出 0，`/tmp/rzpdf-composite-flags-boundaries.log` 为 5 passed、0 failed、695 filtered；负轮廓计数/保留与互斥flags/首组件定位、截断各字段、非末组件指令、字/字节参数、完整矩阵、借用遍历及组件预算所有权均通过。新增一项测试不计入此前1048项全量；生产解析实现未改。继续核对CID映射与SFNT写出子项，P3-05保持未完成。

复合组件边界补验（待终态）：新增 `composite_negative_contours_and_flag_boundaries_are_explicit`，验证轮廓计数 -1/-2/i16::MIN 均按复合解析，0/正值拒绝；保留 flags、互斥变换、同时 scaled/unscaled offset 和首组件非 XY 的精确错误。与已有截断逐字段、非末组件指令、字/字节参数、完整矩阵及预算所有权测试联合运行；会话 72341，日志 `/tmp/rzpdf-composite-flags-boundaries.log`。CID已有共享GID独立CID/ToUnicode/宽度及非BMP测试和只读glyph_mappings接口，后续按完整SFNT/公共编辑证据核对，不自动勾选全部映射交付。

聚合字节测试修正待复验：会话 98511 退出 101，字符额度与原跨调用原子性两项通过；字节测试在超限拒绝后的减少调用重试阶段被 `Predefined CMap source bytes` 拒绝。Identity-H 内置源为 7887 字节，原填充产生的测试额度更小，未能到达预期成功路径。增大 Form 注释填充（每个 Form 超过该 CMap 源）以隔离展开额度，保持 `span expanded decoded work` 精确拒绝及状态不变断言；重试日志 `/tmp/rzpdf-subset-span-aggregate-limits-retry.log`，待终态，不将失败版计为字节验收通过。

聚合额度专项进度：字符测试会话 22808 已退出 0，四场景通过（1 passed、697 filtered）。新增 `automatic_subset_cross_call_decoded_work_counts_repeated_forms`，公共入口验证同一 Form 三次调用按 `root 字节＋3×Form 字节` 累计，精确上限与少 1 字节拒绝；失败字节/预算不变，显式减少调用后刷新快照重试，原共享 Form 不变及严格重开文字。包含原跨调用原子性测试的三项复验会话 98511，日志 `/tmp/rzpdf-subset-span-aggregate-limits.log`，待终态；字节专项尚未声明通过，1048 完整基线早于这两项新增测试。

聚合字符额度补验（待终态）：新增 `automatic_subset_cross_call_character_limits_are_aggregate_and_atomic`，公共自动子集入口覆盖两次共享 Form 各自 2 字符、累计输入 4 对上限 3/4，以及局部新字 3＋尾段保留 2、累计输出 5 对上限 4/5；拒绝时验证保存字节/预算/原 Form 不变，并刷新根快照以较窄范围重试、严格重开核对结果。专项会话 22808，日志 `/tmp/rzpdf-subset-span-aggregate-characters.log`；尚未声明通过，展开字节聚合边界仍待补齐。1048 项完整基线不覆盖这项新增测试。

最新完整基线（2026-10-09）：`cargo test --workspace --locked` 会话 75971 已退出 0，`/tmp/rzpdf-root-endpoint-workspace.log` 共 52 个成功组、1048 项通过，0 失败/忽略/过滤；包括 PDF 单元 697、render 单元 38、页面根端点公共集成 3 项及三个 crate 文档测试。Rust/Cargo 指纹 `eb8aef5e9cbad965bbf35703f195abeea4592e5c5eeeff0a77ce9859ed27ba8c` 与启动时一致，`git diff --check` 通过，本轮源码冻结解除。此基线覆盖当前双向根/Form 子集修改，不代表聚合额度新任务、完整 P3-05 或整个项目完成。下方“会话 75971 正在运行”是历史记录。

- [x] 范围规划器聚合额度确定性验收：公共跨调用自动子集入口覆盖多个调用各自未超限、累计输入/输出字符超限及精确上限；重复 Form 展开字节按每次调用累计，精确上限成功、少 1 字节拒绝。错误后保存字节/预算占用恢复，字符测试以较窄选区刷新重试，展开测试显式减少调用后刷新重试；原共享 Form 不变、严格重开文字均通过。证据 `/tmp/rzpdf-subset-span-aggregate-limits-retry.log` 会话 10513 退出 0，3 passed、696 filtered，包含原跨调用原子性回归。仅此固定矩阵，不代表所有预算层或完整 P3-05。

2026-10-09 当前验收状态：页面根/Form 双向六场景子集专项已终态通过，`/tmp/rzpdf-subset-span-root-bidirectional.log` 为 1 passed、0 failed、696 filtered；普通页面端点公共集成三项通过。当前源码启动 `cargo test --workspace --locked`，日志 `/tmp/rzpdf-root-endpoint-workspace.log`，会话 75971；验收期间冻结 Rust/Cargo 文件，指纹 `eb8aef5e9cbad965bbf35703f195abeea4592e5c5eeeff0a77ce9859ed27ba8c`。尚未取得最新全量终态，不能沿用旧 1046 项证明当前代码通过。接下来先完成全量门禁，再核对范围规划器/聚合预算退出条件及整理当前能力摘要；P3-05 和整体计划保持未完成，fuzz 暂缓，不新增提交或推送。

2026-10-09 页面端点公共集成终态与反向选区补验：会话 75814 退出 0，/tmp/rzpdf-page-root-endpoint-integration-retry.log 三项全部通过，显式根转换/普通 Form-only 空链拒绝/普通展开根内自然补偿与原 Form 选区均验收。子集专项扩展六场景：原四项 root→Form 同/异字体，加自然/补偿 Form→root（首段局部 Form 新字体、末段页面原字体后缀）；使用原绘制顺序来源，严格重开 ABBAAAA、未编辑调用索引相应移动，error/panic 回滚及字节位置拒绝沿用。/tmp/rzpdf-subset-span-root-bidirectional.log 待终态，未声明反向通过，生产基线最新全量仍为修改前 1046。

2026-10-09 普通展开选区根端点入口修正待验收：/tmp/rzpdf-page-root-endpoint-integration.log 终态 2 passed/1 failed，新公共契约在普通 replace_nested_text_span 外层仍受 validate_limits 非空链拒绝，原两项 Form 用例通过。移除该外层重复校验，统一由 expanded walker 验证两个端点、几何/调用深度/索引/字节位置及原快照，普通 Form-only 接口非空链限制不变。/tmp/rzpdf-page-root-endpoint-integration-retry.log 待终态，生产变更尚未全量验收，不把子集根专项通过等同普通入口已通过。

2026-10-09 页面根子集认证/panic 专项终态：会话 6696 退出 0，/tmp/rzpdf-subset-span-root-authentication.log 为 1 passed、0 failed，四场景在 root→Form 同 CID/Courier 范围基础上通过篡改字节偏移精确拒绝、指定 error/panic 发布后字节回滚、预算释放和刷新重试。公开页面端点契约集成会话 37152 待终态，最新生产修改尚未整体 workspace 验收，不用旧 1046 项代表当前全量。

2026-10-09 页面端点公共契约补验待终态：same_invocation_span 集成新增 page_text_endpoint_is_explicit_and_does_not_become_a_form_leaf，真实公开类型从顶层 source 转换，要求索引/字节偏移保留、空调用/几何链、nested_content_program 拒绝且文档不变；原编码普通展开选区自然/补偿根内 B→X 严格重开 AXC，另一共享页 ABC。/tmp/rzpdf-page-root-endpoint-integration.log 待终态；该组还复验原有两项 Form 展开选区。子集根认证/panic 会话 6696 仍待结果，未宣称通过。

2026-10-09 页面根端点修正专项终态：会话 91003 退出 0，/tmp/rzpdf-subset-span-root-endpoint-retry.log 为 1 passed、0 failed，四场景自然/补偿 × 同 CID/Courier 尾段验证公开根端点转换、页面新字体注册、Form 原字体后缀与共享调用/页坐标隔离、tiny 预算释放和外层指定错误字节回滚。后补精确端点位置认证与 panic 版会话 6696 仍待终态，不将这两项后补断言计入前版通过。docs/content-editing.md 已说明顶层 source 与 nested_source 的区别及空链仅展开选区可用。

2026-10-09 页面根端点认证与 unwind 补验待终态：旧混合会话 99313 退出 101，同样在未修正的顶层 nested_source 解包失败，未执行真实混合编辑。修正后的页面根四场景另补篡改 operator_byte_offset 精确 ObjectError（span endpoint byte position mismatch）且字节/预算不变，以及发布后指定 panic 完整回滚和刷新重试；/tmp/rzpdf-subset-span-root-authentication.log 待终态。上一修正版会话 91003 已编译仍待运行输出，不计入本轮后补断言通过证据。

2026-10-09 根端点缺口修正待验收：非混合根专项会话 2139 退出 101，测试误将顶层字符的 nested_source 解包（顶层仅 source）；展开选区原校验也拒绝空调用链，根首插入分支无法通过公开端点表达。新增 NestedDrawSource::from_page_text 显式转换，只有 visit_expanded_text_span 接受空调用链表示页面根，普通 nested validate_limits 非空约束保持，路径几何仍拒绝、事件索引/字节偏移/快照仍认证。专项改用显式页面端点，/tmp/rzpdf-subset-span-root-endpoint-retry.log 待终态；混合旧版 99313 尚待结果，不支持修正后通过声明。生产修改后旧 1046 全量不代表当前版本。

2026-10-09 根→Form 跨调用混合字体补充待验收：automatic_subset_span_inserts_in_root_and_deletes_in_form 扩展自然/补偿 × 同 CID/Courier 尾段四场景；首段仍完整 TrueType 来源并注册新子集，尾段 Courier 18pt/3Tc 单字节码独立删除、保留其原字体后缀，坐标及资源隔离/预算/error 回滚断言沿用。/tmp/rzpdf-subset-span-root-mixed-public.log 待终态；先前非混合版本会话 2139 仍存活待输出，不能覆盖本轮新增混合断言。

2026-10-09 根作用域跨调用专项补充待验收：中文真实字体会话 42495 已退出 0，固定中英实样子项勾选，文档同步完整 1046 门禁及实样闭环。新增 automatic_subset_span_inserts_in_root_and_deletes_in_form，覆盖页面根首事件新子集绑定、进入共享 Form 尾段旧字体删除/保留后缀、自然/补偿、tiny 预算释放、指定外层错误字节回滚及刷新重试、原 Form/另一页/未编辑调用与前缀坐标保持；/tmp/rzpdf-subset-span-root-public.log 编译测试待终态，不以旧 1046 门禁证明这项新增测试通过。

2026-10-09 真实跨调用结构/坐标/视觉验收：Arial/NotoSC 生成均退出 0，严格重开和原共享 Form 不变检查通过，跟踪峰值分别 1590304/2976146 字节。output/pdf/rzpdf-{latin,cjk}-subset-span-edit.pdf 均 qpdf 无结构错误，MuPDF stext 经 Python -O check-subset-span.py 精确文字/字体/24pt/起点/补偿末端/自然平移/第三调用位置及各七负例通过；六页 96dpi PNG 已全部实际目视，无缺字、裁切或重叠，补偿页第二行前方空位为保留原位置。抽取字体目录 /tmp/rzpdf-span-latin-fonts.pFjoeJ 与 /tmp/rzpdf-span-cjk-fonts.8crrQy，Arial 完整字体检查退出 0，NotoSC 会话 42495 待终态。旧 Arial 默认检查会话 48782 退出 0，未破坏默认字集规则；完整 P3-05 仍未完成。

2026-10-09 跨调用字体检查复用接入：check-subset-edit.py 的 check_fonts 增加仅关键字 repertoire_pair（默认字集/CLI 不变），check-subset-span.py --fonts SOURCE DIR 显式传 AAA/AéÅ 或 你你你/你好，复用三子集精确身份/原 GID/轮廓/复合/度量/校验和/许可及两个字体负例。旧中英 Form XML 在 Python -O 下各精确坐标和六负例复跑退出 0；旧 Arial 默认字体验收 /tmp/rzpdf-subset-font-checker-default-retry.log 待终态，新跨调用字体验收仍待实样。

2026-10-09 真实跨调用示例 check 终态：首轮 /tmp/rzpdf-real-subset-span-check.log 编译失败 E0624（示例误用私有 ctx），改为公开 resolve_object 后 /tmp/rzpdf-real-subset-span-check-retry.log 会话 41828 退出 0。已分别启动 Arial/NotoSC 生成命令，日志 /tmp/rzpdf-real-subset-span-{latin,cjk}.log，目标 output/pdf/rzpdf-{latin,cjk}-subset-span-edit.pdf；两生成与独立结构/文字/字体/视觉均待验收，不将编译成功算成实样通过。

2026-10-09 真实跨调用独立 XML 检查器待实样验收：新增 docs/check-subset-span.py，检查三页精确文字/ArialMT 或 NotoSansSC-Thin/24pt、所有坐标有限、保留首字符与新字插入起点、补偿第二调用两字符后缀、自然删除一致负向位移及第三调用逐字坐标保持；七个指定失败点负例包括错误文字/页数/字体/字号/补偿位置/未编辑调用位置/NaN，采用显式 require 不受 Python -O 绕过。--help 已退出 0，但尚无实际 XML 验证。生成器 check 会话 6046 仍待终态，不声明新示例或检查器通过真实验收。

2026-10-09 真实跨调用生成器接入待验收：新增 pdf/examples/edit_subset_span.rs，Arial AAA→éÅ / NotoSC 你你你→好，三页共享一个原 Form、每页三次调用以 -80/-160 cm 分行；选区跨前两调用各保留起末字符，第三调用不编辑，自然/补偿分别一次调用新子集 span API，检查原共享 Form 和严格重开精确文字。/tmp/rzpdf-real-subset-span-check.log 编译待终态，尚未生成 PDF 或独立/视觉验收；刚通过的 1046 门禁早于此新增示例，不覆盖它。

2026-10-09 最新跨调用完整门禁终态：会话 43104 退出 0，cargo test --workspace --locked 日志 /tmp/rzpdf-cross-call-workspace.log 共 52 个成功组、1046 passed、0 failed/ignored/filtered；PDF 单元 696、render 单元 38、全部公共集成及三个 crate 的零用例文档测试通过。Rust/Cargo 指纹 8adf9b0cb3cb065910b259ba5edad14a5a6870cc47f5ae26ac2a688452d1bbac 保持，源码冻结解除。该门禁覆盖最新跨调用入口、Owned 预算及八场景测试，不证明真实跨调用字体验收或完整 TODO 完成；下一步真实 Arial/NotoSC 样本，不做 fuzz、未提交。

2026-10-09 最新全量门禁阶段结果：/tmp/rzpdf-cross-call-workspace.log 的 PDF 单元组已 696 passed、0 failed、0 ignored、0 filtered（87.47s），包括跨调用八场景及同 leaf 72 场景，正在进入 adjusted_text 等公共集成。会话 43104 尚未终态，render 与完整集成/文档测试结果未收齐，因此不声明全量成功，不解除源码冻结。

2026-10-09 跨调用双层八场景专项终态：会话 35115 退出 0，/tmp/rzpdf-cross-call-subset-multilevel.log 为 1 passed、0 failed，覆盖自然/补偿 × 新 BB/空删除 × 单/双层共享 Form，并包含六种非法 CID 端点、预算/指定 error/panic 回滚、原父子对象不变、共享页/第三调用隔离与严格重开坐标断言。最新全量回归会话 43104 仍在运行，源码继续冻结；专项通过不等于全量或真实跨调用渲染完成。

2026-10-09 跨调用边界与空替换专项结果：/tmp/rzpdf-cross-call-subset-boundaries.log、/tmp/rzpdf-cross-call-subset-empty.log 均为 1 passed、0 failed；边界版会话 31899 已退出 0，空替换版日志已明确成功（会话终态待收）。双层八场景 /tmp/rzpdf-cross-call-subset-multilevel.log 仍待编译/测试终态。已启动最新 cargo test --workspace --locked，日志 /tmp/rzpdf-cross-call-workspace.log；门禁期间冻结 Rust/Cargo 源码，不以旧 1044 项门禁代替本次结果，文档记录不受冻结限制。

2026-10-09 跨调用双层共享 Form 补充待验收：专项矩阵为自然/补偿 × BB/空删除 × 单/双层共八场景；双层父 Form 显式 XObject 资源调用原 leaf，要求编辑后原父、子对象均未改变，第三调用与另一页仍隔离。/tmp/rzpdf-cross-call-subset-multilevel.log 编译/测试待终态。本轮 ps 已确认边界版和空替换版测试二进制均存活（0 CPU 的启动等待），未因无输出重启；它们不覆盖此最新双层修改。

2026-10-09 跨调用空替换补充待验收：automatic_subset_cross_call 专项扩展为空字符串删除的自然/补偿两场景，严格重开应为 AAAAAA，另一页仍 AAAAAAAAA，末端后缀和第三调用坐标按删除后索引验证；沿用非法 CID 端点、预算失败、指定 error/panic 回滚及原 Form 不变断言。/tmp/rzpdf-cross-call-subset-empty.log 待运行终态；上一版边界测试 /tmp/rzpdf-cross-call-subset-boundaries.log 已编译，进程仍待测试输出，不以此记录声明通过。

2026-10-09 跨调用专项终态与边界补充：/tmp/rzpdf-cross-call-subset-public-tests.log 与 /tmp/rzpdf-span-owned-plans-retry.log 均为 1 passed、0 failed；前者验证自然/补偿跨调用替换与事务回滚，后者验证 Owned 计划预算生命周期及明确换行字节。新增跨调用 CID 起末端六种非法偏移（切开双字节码、越过原字符串），要求失败且保存字节、预算占用不变；/tmp/rzpdf-cross-call-subset-boundaries.log 正在复验。此前 1044 项 workspace 门禁不覆盖这些最新修改，P3-05 仍未完成。

2026-10-09 跨调用新子集公共测试补齐待验收：新增automatic_subset_cross_call_is_atomic_and_keeps_unselected_invocations，两自然/补偿场景跨三次共享Form的前两调用，起末均保留字符，第三调用/另一页/原Form不变；tiny预算释放、指定error/panic字节回滚及刷新重试、严格重开ABBAAAAA/AAAAAAAAA、末端x/y与未编辑调用坐标检查。/tmp/rzpdf-cross-call-subset-public-tests.log待终态。Owned计划旧专项/tmp/rzpdf-span-owned-plans-test.log退出101，预算所有者检查之后源码字节断言错误漏掉重建换行；改为明确\n(B) Tj\n，/tmp/rzpdf-span-owned-plans-retry.log复跑待终态，不算已通过。跨调用入口库check此前退出0，不代替这轮公共验收。

2026-10-09 跨调用新子集公共入口实现待验收：replace_nested_text_span_with_subset_and_budget复用展开原选区、事件归属及调用树；原root认证后的预算事务回调内准备原节点，先生成所有非插入事件原字体删除片段并取得插入metrics，再注册首事件新子集并绑定复制局部scope（root事件使用页绑定），逐节点原索引合并，由Owned计划发布器一次bottom-up发布。SpanSubsetPublication持有字体/binding/scope租约到根提交，旧共享字体及Form不修改。/tmp/rzpdf-cross-call-subset-check.log库check日志已显示成功，句柄终态核对中；公共跨调用专项尚待补齐，不作功能验收。原发布器抽取版本/tmp/rzpdf-span-publication-extraction-tests.log退出0，15项nested_adjusted_text+2项same_invocation_span通过，编译早于Owned/新子集，不混算。Owned预算生命周期/tmp/rzpdf-span-owned-plans-test.log仍待终态，最新全量仍缺。

2026-10-09 跨调用计划所有权接入待验证：SpanNodeProgram增加OwnedPlanned，SpanPublicationPlans区分旧入口Borrowed与新入口所需Owned预算表；共享publisher先检查表及聚合字符，再逐节点转移计划。无子节点时直接保留计划及其程序/片段租约，不借用回调局部表，不为规避生命周期而复制叶程序；旧入口借用路径保留。/tmp/rzpdf-span-owned-plans-check.log编译检查待终态；此前/tmp/rzpdf-span-publication-extraction-tests.log是所有权分支加入之前的回归，不能混算。Owned分支及跨调用新子集尚待实际接入和验收。

2026-10-09 调用树发布抽取库检查终态：/tmp/rzpdf-span-publication-extraction-check.log退出0；nested_adjusted_text与same_invocation_span集成专项已启动，日志/tmp/rzpdf-span-publication-extraction-tests.log待终态。该重构尚未通过最新集成验收，跨调用子集入口仍待接入。

2026-10-09 跨调用子集发布边界收敛：现有commit_prepared_span的原事务/原root认证保持，bottom-up重建与资源绑定抽为build_prepared_span_publication；未来子集入口须在认证成功后的同一回调内注册字体并发布，不提前注册后刷新快照，不重复调用树发布器。内部发布器仍检查计划表大小/聚合字符限制，字体/资源租约需由调用者保留跨发布。/tmp/rzpdf-span-publication-extraction-check.log库check待终态；跨调用子集入口尚未实现，现有span集成回归待执行，不能据此前72场景或真实同leaf样本声称跨调用已支持。

2026-10-09 固定真实Form样本独立验收终态：/tmp/rzpdf-real-form-cjk-independent.log会话51410退出0，中文三份TTF及两字体负例通过，连同Arial验证完成中英两个固定Form跨Tj→TJ样本的严格重开/qpdf/独立精确文字与位置/抽取字体/六页目视闭环。该固定范围替换到末字符串结尾，不代替真实保留末端后缀、多层/混合真实字体或跨调用新子集；P3-05仍未完成，最新workspace未回归。

2026-10-09 真实Form三页样本验证进度：Arial及NotoSC生成日志/tmp/rzpdf-real-form-{latin,cjk}-fixture.log均退出0，输出output/pdf/rzpdf-{latin,cjk}-subset-form-events-edit.pdf严格重开正确；两份qpdf通过，MuPDF stext配合check-subset-edit.py的精确三页文字/字体/字号/原前缀及补偿后续坐标和六XML负例通过。tmp/pdfs对应两组三页96dpi PNG已全部目视，无缺字/裁切/重叠，补偿间隔符合预期。抽取字体目录/tmp/rzpdf-form-latin-fonts.aQlNgZ及/tmp/rzpdf-form-cjk-fonts.VxjN1h各三份TTF；/tmp/rzpdf-real-form-latin-independent.log退出0，轮廓/GID/闭包/度量/head边界/校验和/许可及两字体负例通过；中文/tmp/rzpdf-real-form-cjk-independent.log会话51410仍运行，不能宣称完整真实Form验收。跨调用及最新全量未完成。

2026-10-09 真实Form示例编译终态：/tmp/rzpdf-real-form-subset-example-check.log退出0。已启动Arial与NotoSC --form --events生成，日志/tmp/rzpdf-real-form-{latin,cjk}-fixture.log，输出output/pdf/rzpdf-{latin,cjk}-subset-form-events-edit.pdf；生成结果尚待确认，qpdf/MuPDF文字位置、抽取字体及六页目视尚未执行，不作真实Form验收。

2026-10-09 真实Form示例接入待验证：edit_subset增加--form（当前须同时--events），三页复用同一原Form，页面仅Do；页面1/2通过原nested provenance及leaf program调用同leaf跨事件新子集入口，分别自然/补偿，保留TJ数字/注释并严格重开检查原页及两编辑页文字。生成仍使用本项目引擎以验证实现，PDF技能要求后续独立结构/字体检查及渲染目视，不以内部提取代替视觉。/tmp/rzpdf-real-form-subset-example-check.log编译检查待终态，尚未生成或验收真实Form样本；跨调用及最新全量未完成。

2026-10-09 同leaf混合字体72场景专项终态：/tmp/rzpdf-nested-subset-events-mixed-retry.log退出0，automatic_subset七项通过、687过滤，Form测试内部72场景包含尾段Courier单字节编码/18pt/3Tc与首段CID码、保留末端原字体后缀及自然/补偿位置；沿用单/双层继承、名字冲突/页回退、预算拒绝、指定error/panic字节回滚和共享调用/页隔离。首轮样本名称假设已修正，实际跨字体编辑验证通过。该结果不是最新全量或真实字体渲染验收，跨调用自动子集仍待实现，P3-05未完成。

2026-10-09 72场景混合字体首轮状态纠正：/tmp/rzpdf-nested-subset-events-mixed-tests.log退出101，六项通过、Form一项在样本注册阶段失败；page0已含原CID字体F1，Courier为F2，page1尚无原字体所以Courier为F1，测试错误要求名称相同。已在混合样本page1注册同一原字体再注册Courier，建立一致资源而不修改生产注册规则。复跑/tmp/rzpdf-nested-subset-events-mixed-retry.log待终态，首轮不作72场景验收。

2026-10-09 同leaf混合字体公共矩阵补验：nested_show扩为72场景，在原48场景之外增加跨事件尾段Courier单字节编码、18pt及3Tc；首段仍原CID子集，结束端点按尾段1字节字符码选首字符并保留第二字符，检查末端原Courier绑定及各事件度量补偿。混合字体场景同样覆盖单层/双层继承、局部名称冲突/页回退、预算与指定error/panic回滚、共享页/调用及坐标隔离。/tmp/rzpdf-nested-subset-events-mixed-tests.log最新七项待终态，不计入前48场景通过；真实Form与跨调用自动子集仍待推进。

2026-10-09 同leaf跨事件公共专项终态：/tmp/rzpdf-nested-subset-events-public-tests.log退出0，automatic_subset七项通过、687过滤；nested_show内部48场景完成同事件/跨下一Tj保留末端后缀、自然/补偿、单层与双层继承/资源冲突/页回退、指定error/panic完整回滚重试及共享调用/页和坐标隔离。该结果覆盖共享范围规划器与新Form event_range入口，不宣称跨调用展开选区或真实Form字体验收；末段不同字体/状态变化矩阵仍待补齐，最新全量仍是这些生产修改之前1044基线。

2026-10-09 同leaf跨事件公共矩阵待验收：nested_show测试扩为48场景（四种首事件×自然/补偿×单层冲突/双层冲突/双层页回退×同事件/跨下一事件）。新公共event_range入口跨到下一Tj首字符末端，保留其第二字符；严格重开检查目标文字、未编辑四字符调用与另一页、后缀x/y补偿及资源名称，所有场景沿用tiny预算拒绝、指定error/panic字节回滚和刷新重试。/tmp/rzpdf-nested-subset-events-public-tests.log最新七项待终态，不算已通过。此前共享规划器重构/tmp/rzpdf-scoped-subset-range-tests.log退出0七项通过、687过滤，编译早于新Form入口及48场景。不同后段字体、状态变化、真实Form样本、跨调用新子集和最新全量仍待推进。

2026-10-09 同leaf跨事件自动子集入口实现待验收：replace_nested_text_event_range_with_subset_and_budget锚定原start source，认证root后预算准备调用链，每个原事件重建祖先继承状态及原leaf metrics；共享范围规划器先尾段删除再首段新子集，复制leaf scope独立绑定并持有scope/binding租约到inside-out一次发布。原scope不被新字体注册污染；中间子Form调用保持，不宣称展开选区/跨调用支持。/tmp/rzpdf-nested-subset-events-check.log库check退出0；公共跨事件测试仍待补齐，最新全量不覆盖。原/tmp/rzpdf-form-subset-scope-matrix.log退出0，单项内部24场景通过、693过滤，编译早于范围规划重构；重构七项/tmp/rzpdf-scoped-subset-range-tests.log仍运行，不混用结果。

2026-10-09 作用域无关范围规划器接入：页面跨事件入口保留原快照认证与外层事务，调用build_scoped_subset_event_range；该内部规划器接收原事件metrics解析回调及可选局部scope，先规划后段删除、首段子集注册、按原索引合并，不自行发布或刷新快照。PreparedSubsetRangeEdit额外持有局部binding预算租约到发布结束，为同leaf Form复用准备。/tmp/rzpdf-scoped-subset-range-check.log库检查退出0；格式化后/tmp/rzpdf-scoped-subset-range-tests.log七项专项待终态。原24场景/tmp/rzpdf-form-subset-scope-matrix.log编译早于此生产重构，仍待终态，不能替该重构验收。Form跨事件尚未公开接入，P3-05未完成。

2026-10-09 显式leaf名称冲突专项终态及回退分支保留：/tmp/rzpdf-form-subset-font-collision-retry.log退出0，1项通过、693过滤，内部16场景验证保留FontEdit1并使用FontEdit2，双层文字状态由parent继承。随后将无leaf Resources的页回退分支恢复为独立组合，现四种事件×自然/补偿×单层冲突/双层冲突/双层页回退共24场景，/tmp/rzpdf-form-subset-scope-matrix.log待终态；后补不计入先前通过。同leaf跨事件/跨调用自动子集仍待实施。

2026-10-09 名字冲突首轮失败纠正：/tmp/rzpdf-form-subset-font-collision.log退出101，继承样本无leaf Resources时实际使用页资源回退，父scope中的FontEdit1不属于leaf作用域，测试错误地要求父资源继承。无资源回退16场景此前/inherited-matrix.log已通过；本次冲突样本改为显式leaf资源、仍继承父Tf/Tc/TL，首轮未通过不作验收。复跑/tmp/rzpdf-form-subset-font-collision-retry.log待终态，未改变生产资源解析规则。

2026-10-09 双层继承专项终态及名字冲突补验：/tmp/rzpdf-form-subset-inherited-matrix.log退出0，1项通过、693过滤，内部16场景通过（leaf无资源及Tf/Tc/TL，由父层继承；原父/叶对象及另一调用/页保持，预算/error/panic回滚）。随后给单层leaf或双层parent局部Font预占FontEdit1，成功编辑后检查复制leaf保留原绑定并新增/使用FontEdit2；/tmp/rzpdf-form-subset-font-collision.log待终态。该后补版本不算入此前继承通过或1044全量，跨事件/跨调用自动子集仍未完成。

2026-10-09 多层Form继承矩阵补验：nested_show专项扩为四种文本事件×自然/补偿×单层显式状态/双层继承共16场景；继承样本leaf无Resources且无Tf/Tc/TL，由parent提供字体和文字状态。核对两层provenance、原parent/leaf对象不变，沿用预算/error/panic回滚、共享页及未编辑调用坐标隔离。/tmp/rzpdf-form-subset-inherited-matrix.log待终态，不计入此前1044完整门禁。局部名字冲突、同leaf跨事件及跨调用自动子集仍待推进。

2026-10-09 Form位置隔离专项终态：/tmp/rzpdf-form-subset-position-isolation.log退出0，1项通过、693过滤，测试内部八场景均完成后缀及下一事件y坐标、未编辑调用逐字符文本与x/y坐标保持断言。最新完整生产基线为下述1044门禁；新增断言仅此次专项验证。下一步多层Form继承/局部字体名称冲突与同leaf跨事件范围规划，P3-05仍未完成。

2026-10-09 Form位置隔离补验：在已通过的八场景测试中增加后缀及下一事件y坐标保持，并逐字符核对未编辑第二调用的文本与x/y坐标；/tmp/rzpdf-form-subset-position-isolation.log专项待终态。此为1044完整门禁之后的测试修改，不能声称新断言已被该门禁验证。

2026-10-09 最新完整门禁终态：会话32619退出0，/tmp/rzpdf-form-matrix-workspace.log共52组1044项通过，0失败/忽略/过滤，含694项PDF单元、38项render单元及三组零用例文档测试。Rust/Cargo集合SHA256保持c1b7bb5ad1ad6954e4fb5474ffa7a43a77584b7c05194ce7eec3652f56b9709c；本轮源码冻结结束。该结果覆盖Form独立字体对象注册及单事件八场景版本，不证明尚未实现的多层/同leaf跨事件/跨调用自动子集和真实Form样本，P3-05保持未完成。下一步补完整位置隔离断言并扩展多层继承。

2026-10-09 最新全量回归跟踪：会话32619对应cargo test --workspace --locked，日志/tmp/rzpdf-form-matrix-workspace.log；最近检查cargo及rustc子进程仍存活，尚在构建集成测试/示例，无测试终态，不重复启动。冻结期间Rust/Cargo文件集合SHA256为c1b7bb5ad1ad6954e4fb5474ffa7a43a77584b7c05194ce7eec3652f56b9709c，终态须重新核对；允许更新进度文档，但不改生产/测试源码。此前1041项不能替代本轮最新全量。

2026-10-09 Form八场景专项终态：/tmp/rzpdf-nested-subset-show-matrix-retry.log退出0，automatic_subset七项通过、687过滤；nested_show测试内部Tj/单引号/双引号/TJ各自然/补偿共八场景完成，包含预算拒绝释放、指定error/panic字节回滚重试、原Form及页Font字典不变、共享页隔离、TJ数字/注释和后缀x坐标检查。该结果不代表多层继承、Form跨事件/跨调用、真实Form字体样本或最新全量验收。下一完整回归日志/tmp/rzpdf-form-matrix-workspace.log；回归期间不修改生产和测试源码，P3-05仍未完成。

2026-10-09 Form八场景编译状态纠正：/tmp/rzpdf-nested-subset-show-matrix.log已退出101，E0596来自TJ检查中current_page缺少mut，未执行测试；下方“仍运行”记录失效。已补mutable绑定，重新运行日志/tmp/rzpdf-nested-subset-show-matrix-retry.log，结果待确认，不借此前七项通过证明八场景或最新全量。

2026-10-09 Form单事件八场景补验：公共合成样本扩为Tj/单引号/双引号/TJ跨项×自然/补偿；原TL/Td及双引号Tw/Tc保持，TJ跨两个字符串首→末起点范围保留后缀、-125/+75及注释。增加tiny ResourceLimit与预算释放、发布后指定panic payload完整字节回滚、每次失败重新取得root及nested provenance后成功重试；沿用指定error/共享页/原Form与页Font字典不变及后缀/下一事件坐标断言。/tmp/rzpdf-nested-subset-show-matrix.log最新七项仍运行，尚未验收该八场景。多层继承、同leaf跨事件/跨调用自动子集、真实Form样本与最新全量仍待推进，P3-05保持未完成。

2026-10-09 独立Form字体注册验收终态：/tmp/rzpdf-detached-nested-subset-tests.log退出0，最新automatic_subset七项通过（687过滤），包含目标页Font字典完整不变、复制leaf独立字体绑定、共享Form/另一页隔离与位置补偿和回滚重试。准备回调重构版/tmp/rzpdf-nested-prepared-regression.log亦退出0，嵌套补偿15/图像2/路径8/文字6共31项通过；该四组编译早于独立注册，不将其当作最新独立注册全量。下方“仍运行”历史状态由本记录取代。下一步补Form TJ/引号/多层继承及预算/panic矩阵，接同leaf跨事件和跨调用自动新子集，真实字体验证与最新workspace仍待推进，P3-05未完成。

2026-10-09 Form首轮公共终态及独立注册收敛：/tmp/rzpdf-nested-subset-verified-tests.log退出0，automatic_subset七项通过（687过滤），目标Form单事件新字自然/补偿、重复共享调用/另一页、原Form对象不变、指定error回滚/重试成立。随后embedded_font拆出内部EmbeddedCidFontObjects/EmbeddedSubsetFontObjects与Option页绑定注册，公开RegisteredCidFont/RegisteredSubsetCidFont页面名称契约不变，不用空名称冒充已注册句柄。Form片段使用None仅创建字体对象并绑定复制leaf FontEdit，去除额外页Font绑定；目标页仍仅新增必要复制Form XObject绑定。/tmp/rzpdf-detached-subset-check.log库check退出0；/tmp/rzpdf-detached-nested-subset-tests.log后补页Font字典完整不变断言的最新七项仍运行，不能借首轮证明独立注册版本已验收。最新workspace及完整Form跨事件/跨调用与真实矩阵仍待推进。

Form公共测试构建更正：/tmp/rzpdf-nested-subset-public-tests.log退出101，新增测试跨模块直接访问私有doc.ctx导致E0616，未运行测试；已改为公开ctx()只读入口。新七项日志/tmp/rzpdf-nested-subset-verified-tests.log重新编译验证，仍待终态；生产库check已通过，不将测试构建失败当作成功或更改断言。

2026-10-09 Form单事件自动子集接入：新增replace_nested_text_show_range_with_subset_and_budget，source锚定同一原Tj/引号/TJ（支持同TJ多字符串端点）；原root认证后预算prepare调用链，按每层原scope还原继承状态，注册新子集并在复制leaf scope绑定FontEdit，再生成局部Tf片段，预算化inside-out复制/根Do一次发布。新增局部资源副本租约持有到提交，不提前退租约。库check /tmp/rzpdf-nested-subset-check.log退出0；/tmp/rzpdf-nested-subset-public-tests.log最新automatic_subset七项运行，新增两共享调用/另一页、自然/补偿、指定error回滚/刷新重试、严格重开BBAAAAA/AAAAAA和原Form对象不变。先前准备回调集成/tmp/rzpdf-nested-prepared-regression.log已有嵌套补偿15项通过，其他三组仍待终态；两轮编译范围不可混算。此为单事件入口，不代表跨事件/跨调用Form选区完成；当前字体注册仍兼用目标页注册器，额外页字体绑定需后续独立注册接口收敛，不能声称只新增leaf绑定。P3-05与最新完整门禁未完成。

2026-10-09 Form认证后准备回调：commit_nested_edit_inner改为复用commit_nested_prepared_inner，新增内部commit_nested_prepared_with_budget。原页面快照认证成功后才执行prepare返回调用链与预算编辑程序，注册/局部scope修改和既有inside-out Form复制/根Do发布处于同一预算事务；现有入口仅包装原已准备参数，不重复实现发布。不刷新root快照、不绕过身份/修订认证。/tmp/rzpdf-nested-prepared-check.log库check退出0；/tmp/rzpdf-nested-prepared-regression.log嵌套文字/补偿/图像/路径四组公共集成仍运行，尚未验收本轮。下一步build_subset_text_fragment接局部Font name绑定及scope所有者，公共Form自动新子集入口仍待完成，P3-05不勾选。

2026-10-09 Form局部绑定专项终态：/tmp/rzpdf-form-local-font-binding-tests.log退出0，local_过滤10项通过（683过滤），包含新Font局部绑定和既有预算化XObject冲突/原子失败回归。名称FontEdit3避让、其他scope保持、原Font不变、实际租约/峰值少1失败和非字典输入拒绝成立；助手尚未用于公共Form自动子集编辑，不把前置专项当作Form交付完成。

2026-10-09 Form局部字体绑定前置：nested_edit预算化XObject绑定抽成私有同池资源类别助手，保留FmEdit名称/原错误契约；新增bind_local_font_with_budget仅改复制的有效Font scope，用FontEdit冲突避让，输入已验证font对象ID，返回真实资源副本/名称租约由调用者持有到发布。新增三类验证：已有FontEdit1/2避让且原scope/其他资源不变、精确峰值少1拒绝保持输入及租约、Font类别非字典拒绝恢复；/tmp/rzpdf-form-local-font-binding-tests.log运行中，未验收。尚未接公共自动子集Form编辑：注册必须放在原root认证之后，不能先改页资源再二次认证，也不能用刷新快照绕过；下一步抽出认证后的自底向上发布构造器与局部新font名称绑定回调。P3-05仍未完成，最新完整门禁待重跑。

真实跨事件字体独立检查终态：/tmp/rzpdf-events-latin-independent.log与/tmp/rzpdf-events-cjk-independent.log均退出0；六份抽取TTF通过精确字集/原GID、复合闭包、轮廓/指令/度量、head边界、全表/整字体校验和及许可字段检查，各两字体负例按指定语义拒绝。Latin新子集各2所选复合，NotoSC固定字形无所选复合；不能以此证明所有复合或CJK全字体支持。下一步Form局部字体绑定与原调用隔离接入、真实保留后缀边界矩阵，最新完整workspace仍待重跑。

2026-10-09 真实跨事件样例首轮：edit_subset新增--events，与--tj互斥，可叠加--cjk；将原文字拆到首Tj与尾TJ，跨事件替换至尾字符串结尾，保留-25/+15及注释，后续独立Tj验证自然/补偿推进。Arial A A A→A éÅ和NotoSC你你你→你好三页样例均生成退出0、严格重开精确文本、qpdf和MuPDF原前缀/后续坐标/字体字号及六XML负例通过，六页96DPI渲染已逐页目视，无缺字/重叠/裁切。输出output/pdf/rzpdf-{latin,cjk}-subset-events-edit.pdf；抽字体目录/tmp/rzpdf-events-latin-fonts.Rautqe及/tmp/rzpdf-events-cjk-fonts.KfXR4s，独立日志/tmp/rzpdf-events-{latin,cjk}-independent.log。固定样本不代替真实保留末端后缀/跨字号矩阵、Form或最新全量；P3-05保持未完成，fuzz继续暂缓。

2026-10-09 跨事件混合字体专项终态：/tmp/rzpdf-subset-cross-event-mixed-tests.log退出0，最新automatic_subset六项通过（686过滤），四场景同子集/后续单字节Courier×自然/补偿和tiny/stale/error/panic/重试均通过；严格重开保留后缀和下一事件Courier绑定与补偿坐标，旧共享页不变。仅该固定混合水平矩阵，不证明真实字体跨事件、竖排、跨Form或最新完整workspace，P3-05继续未完成。

2026-10-09 跨事件混合字体矩阵补充：automatic_subset_cross_event用例扩为同子集/后续Courier两种×自然/补偿四场景，首段双字节静态子集新B、尾段单字节Courier (A)/(AA) 部分范围；要求严格重开ABAA、后缀及下一事件仍Courier、各自字号/字距补偿、共享页和tiny/stale/error/panic/重试保持。后续Courier无FontFile2，测试若通过可证明删除段未被错误要求匹配首段完整源字体。最新六项日志/tmp/rzpdf-subset-cross-event-mixed-tests.log仍运行，尚不宣称通过；真实跨事件及Form仍待推进。

2026-10-09 跨事件预算/回滚补充终态：/tmp/rzpdf-subset-cross-event-rollback-tests.log退出0，最新automatic_subset六项通过（686过滤），包含tiny ResourceLimit、同快照精确stale拒绝、刷新后发布指定error及指定panic payload回滚、完整保存字节/租约恢复与成功重试。上条“仍待终态”已由本记录取代。此固定合成水平字体/两字号矩阵不代替混合字体、真实新增字符、嵌套Form或最新workspace门禁；继续按完整P3-05推进。

2026-10-09 跨事件首轮专项终态：/tmp/rzpdf-subset-cross-event-verified-tests.log退出0，automatic_subset六项通过（686过滤），新增跨事件自然/补偿、字号/字距变化、原索引一次发布、TJ数值注释、共享页、外层指定error回滚/重试、严格重开与后缀/下一事件位置均通过。后补tiny/stale/panic专项/tmp/rzpdf-subset-cross-event-rollback-tests.log仍待终态，不计入首轮六项范围。跨事件复杂混合字体与真实矩阵、Form仍未完成，最新1041工作区是本入口实现前基线。

2026-10-09 跨事件回滚覆盖补充：新增tiny预算请求要求ResourceLimit、失败后同原快照要求精确过期错误、刷新快照后发布并外层error回滚、指定panic payload回滚及再次刷新成功重试；每步要求完整保存字节和邻近池租约恢复。新日志/tmp/rzpdf-subset-cross-event-rollback-tests.log六项专项仍运行，原/tmp/rzpdf-subset-cross-event-verified-tests.log不包含后补预算/panic断言，两轮编译范围分别记录；当前尚未宣称跨事件验收通过。

跨事件测试构建更正：首轮/tmp/rzpdf-subset-cross-event-tests.log退出101，为新增坐标断言误用不存在的origin字段导致E0609，未执行测试；已改用ExtractedCharacter.position.x()/y()。新日志/tmp/rzpdf-subset-cross-event-verified-tests.log重编译六项，仍待终态，不能将上一条“运行”视作通过。生产库check通过不受该测试字段错误影响。

2026-10-09 跨事件自动子集接入首轮：片段抽取版五项专项/tmp/rzpdf-subset-fragment-tests.log退出0（5通过/686过滤）。新增replace_page_text_event_range_with_subset_and_budget，原快照认证后先规划后续旧字体删除片段，再构造首事件新字片段，复用finish_text_event_fragments按原索引一次组装/发布；首段计入合并字符额度，原码字节跨事件聚合，非文字状态和numeric-only TJ保持。库check /tmp/rzpdf-subset-event-check.log退出0；新增自然/补偿两模式公共回归，包含不同字号/字距、Tj→TJ部分后缀、数字注释、共享页、发布后指定error回滚与新快照重试、严格重开ABAA及后缀/下一事件坐标。最新六项日志/tmp/rzpdf-subset-cross-event-tests.log仍运行，未验收本轮跨事件入口；复杂混合字体、预算/panic全矩阵、真实字体及Form仍待推进，不借重构前1041门禁证明新代码。

2026-10-09 自动子集片段构造器抽取：subset_text_edit新增内部SubsetTextFragment（原index/预算字节/operation_count/注册字体所有者）和build_subset_text_fragment，接收原有效BudgetedTextMetrics，不提交内容或刷新快照。既有单字符串/引号/TJ入口仍在原快照认证后的同一外层事务内调用，再一次组装发布。片段临时字体度量和编码在构造结束释放，实际片段及注册所有者保留到提交。/tmp/rzpdf-subset-fragment-check.log库check退出0；/tmp/rzpdf-subset-fragment-tests.log五项专项仍运行，尚未验收本轮重构。此前1041全量属于重构前基线，跨事件组合/聚合预算/后续混合字体删除及Form仍未接入，P3-05保持未完成。

2026-10-09 最新TJ完整门禁终态：原cargo会话25252退出0，/tmp/rzpdf-subset-tj-workspace.log共52个成功测试组、1041项通过，0失败/忽略/过滤；PDF单元691项、render单元38项（64.31s），所有公共集成及font_data/pdf/render文档测试终态成功。Rust/Cargo集合指纹始终为3c0bca8f584ea1b9a0b626f8c74bbae6bd0d7f6ea67d091c7f3cd29860eab733，git diff --check通过，解除本轮源码冻结。此基线覆盖自动子集单字符串/引号/TJ单项及跨项和回滚快照测试修正；真实Arial/NotoSC跨TJ固定范围独立验收另见上方记录。下一步实施不提交的单事件片段构造器并接跨事件一次事务发布，再推进Form局部资源及真实边界矩阵；P3-05及完整项目仍未完成，fuzz继续暂缓，未创建新commit或推送。

2026-10-09 最新全量阶段结果：/tmp/rzpdf-subset-tj-workspace.log编译完成（5m40s），PDF单元691项通过、0失败/忽略/过滤，包含本轮TJ拆分/跨项规划和自动子集回滚修正。原cargo会话25252仍运行，进入adjusted_text等公共集成；不可将单元通过表述为全workspace终态。源码指纹保持3c0bca8f584ea1b9a0b626f8c74bbae6bd0d7f6ea67d091c7f3cd29860eab733，继续冻结，后续render和文档测试尚待确认。

2026-10-09 真实字体跨TJ样例：edit_subset增加可组合--tj/--cjk，保留-25/+15及注释，跨三个字符串起点部分保留→末项结束替换；Arial A A A→A éÅ、NotoSC你你你→你好，三页原共享资源/自然/补偿对照及下一Tj。两次生成退出0，严格重开精确文本、原数值注释断言、qpdf和MuPDF三页文字/字体/字号/原前缀与下一事件坐标检查及各六负例通过；六页96DPI渲染逐页目视无缺字/重叠/裁切，补偿空隙用于保持后续位置。抽字体目录/tmp/rzpdf-tj-latin-fonts.OjL6yW及/tmp/rzpdf-tj-cjk-fonts.ozRHKg，独立日志/tmp/rzpdf-tj-{latin,cjk}-independent.log。最新完整cargo test --workspace --locked已启动，日志/tmp/rzpdf-subset-tj-workspace.log，Rust/Cargo指纹3c0bca8f584ea1b9a0b626f8c74bbae6bd0d7f6ea67d091c7f3cd29860eab733；门禁期间不改源码。尚未全量终态，固定末项结束样本不代替真实保留后缀/全边界矩阵，跨事件/Form及P3-05仍未完成，fuzz不运行。

2026-10-09 跨TJ回滚复验终态：诊断日志/tmp/rzpdf-subset-tj-cross-diagnostic.log退出101，确认原tiny请求实际返回ObjectError("content snapshot is foreign, offline or stale")，不是预算路径失败。刷新快照及精确错误断言后的/tmp/rzpdf-subset-tj-cross-refreshed-tests.log退出0，最新automatic_subset五项通过（686过滤）；覆盖四类真实边界拒绝、tiny预算ResourceLimit、发布后指定error及panic回滚、成功重试、共享页隔离和自然/补偿位置。此轮仅修正测试假阳性风险，生产实现未变；不替代真实字体TJ矩阵、最新完整workspace或P3-05验收。

2026-10-09 跨TJ回滚专项复核：9788已终态，原公共五项通过；45518已终态，新增回滚专项4通过/1失败，失败于tiny预算预期ResourceLimit。检查DocumentState::StoreRollback确认失败回滚递增revision，连续复用快照会提前触发过期拒绝。补实际错误诊断（/tmp/rzpdf-subset-tj-cross-diagnostic.log），各非法请求前重新取快照并要求具体边界错误；tiny和发布后外层error前刷新快照，外层error要求指定发布后错误，防止.is_err()掩盖未执行编辑。最新五项复验日志/tmp/rzpdf-subset-tj-cross-refreshed-tests.log尚在运行，未宣称修复通过。生产代码未变，真实字体TJ矩阵、最新全量、跨事件/Form及P3-05仍待完成；fuzz继续暂缓。

2026-10-09 跨TJ事务隔离补充：在既有跨三字符串自然/补偿位置用例补第二共享字体页面；非法数字端点/半码/反向请求和tiny预算拒绝均须完整保存字节保持、租约恢复。发布后外层error及指定panic回滚，刷新原快照后成功重试；成功后共享页原内容字节与严格重开AAAAAAA保持。生产代码未再改变。45518日志/tmp/rzpdf-subset-tj-cross-rollback-tests.log验证新增断言；9788原跨项公共五项尚未终态，不将后补断言计入其编译范围。未宣称TJ跨项完成，跨事件/Form及真实字体验证/最新全量仍待推进。

2026-10-09 TJ跨项公共入口接入：86843聚合规划补充退出0，专项1项通过（689过滤），包含新增输出超限/原码字节超限/完全删除段ffff无效码拒绝。新增replace_page_text_array_range_with_subset_and_budget，与单项共用统一事务注册入口；按整数组规划字集，用既有budgeted_text_array_range_replacements仅删除旧码，既有逐项adjustments/数组序列化保留原数值词法和注释，再在起点前缀末拆分插入q/Tf新码/Q。补偿第一段旧→新、后续旧→空；不把新子集码交给旧字体解码。87893库check退出0；9788日志/tmp/rzpdf-subset-tj-cross-public-tests.log最新automatic_subset五项运行中，新增跨三字符串部分端点自然/补偿、严格重开ABAA、原/新字体分区、后缀/下一事件坐标与-125/+75注释保持及释放。公共跨项尚未验收，完整最新门禁、跨事件/Form和真实新增字符矩阵仍待推进，P3-05未完成。

2026-10-09 跨TJ规划首轮终态：63295退出0，registered_subset_descriptor专项1项通过（689过滤），同一Font注册/严格重开路径包含跨三字符串、同项插入、整段替换/删除及错误边界/缺字/输入字符聚合/短池释放规划断言。后补聚合输出插入超限、聚合原码字节超限和完全删除段含ffff无效码拒绝，86843日志/tmp/rzpdf-subset-tj-range-aggregate-tests.log运行中，未计入首轮。接公共跨项时必须让既有budgeted_text_array_range_replacements只删除旧码，不能传新子集码触发旧字体解码；在起点局部Tf单独显示新码，补偿分别计算第一段旧→新及后续段旧→空，并保留原TJ数字位置/值。公共跨项发布尚未完成。

2026-10-09 TJ跨字符串规划前置：新增plan_subset_text_array_range，借用两遍遍历整个原数组，按原字体校验所有CID/Unicode/源GID，包括完全删除的字符串；字符串端点和半码边界拒绝，数字槽不计字符，replacement仅在起点插入。聚合原码字节/输入字符/输出字符/UTF8字节额度，输出一次精确预留并持实际capacity租约，不为各项创建解码String或元数据Vec。已补跨三字符串ABA、同项插入、整数组替换/删除、数字端点/反向/越界/缺字/半码、聚合原字符上限及短池释放断言，registered_subset_descriptor定向日志/tmp/rzpdf-subset-tj-range-plan-tests.log运行中，尚未验收。仍未接公共跨项注册/内容发布与逐片段补偿，不缩减P3-05。

2026-10-09 TJ单项专项终态：96262退出0，automatic_subset三项通过（686过滤），覆盖Tj/两类引号/TJ×自然/补偿八场景、严格重开及原TJ数值注释/前后字体保持；9370退出0，最新四项通过（686过滤），后补TJ六类边界、数字端点/半码/越界无修改拒绝、发布后外层error回滚/新快照重试、共享页AAAA及租约释放通过。两个二进制的编译范围分别记录，不将四项后补测试算进先前三项结果。下一步跨字符串字集与逐片段补偿规划，再接跨事件/Form及真实新增字符矩阵；P3-05未完成，最新完整workspace仍为此前1038项冻结版本，不覆盖本轮TJ接口。

2026-10-09 TJ边界与回滚补充：48801拆分专项退出0，1项通过（688过滤），词法数值/注释/邻居及预算释放验证成立。96262八场景公共专项仍运行，不能提前宣称通过。另补automatic_subset_tj_item_boundaries_and_public_rollback：句首/中间/句尾插入、整串删除、前/后半替换六类，拒绝数字项端点/半字符码/越界保持完整保存字节；发布后外层error回滚并刷新快照重试，严格重开目标页与共享字体另一页AAAA，操作租约释放。最新9370日志/tmp/rzpdf-subset-tj-boundary-tests.log运行中，不将后补断言计入已编译96262结果。TJ跨项/跨事件/Form自动新字集仍未完成。

2026-10-09 TJ单字符串公共接入：新增replace_page_text_array_item_with_subset_and_budget，与Tj/两类引号共用replace_subset_string_inner的原快照认证、来源/GID/字符边界规划、外层事务注册与预算化新旧字体补偿。TJ原字符串前后由split接口生成旧字体数组，中间q/Tf显示新码后Q恢复，原数值间距及注释不重编码。14649库check退出0；96262最新automatic_subset专项运行中，位置矩阵新增TJ自然/补偿（合计八场景），要求严格重开ABBAA、原前后字体、后续坐标和-125/+75注释字节保留。48801较早拆分专项仍活跃，不将其未终态或旧1038项门禁当作公共接入验收。跨项、跨事件、嵌套自动子集路径仍未完成，P3-05未完成。

2026-10-09 TJ新字体片段前置实现：新增ContentProgram::text_array_split_string_fragments_with_budget，预算解析核对原TJ词法与对象；围绕单字符串解码字节范围生成两个旧字体TJ，原数值/注释/未修改字符串逐字节保留，前后缀仅重新序列化为hex。两份返回缓冲与生成token按实际capacity租约共存；字符码边界由上层字体规划验证，不在此词法接口猜测。定向48801日志/tmp/rzpdf-subset-tj-split-tests.log运行中，覆盖转义原字符串/数值注释/邻居保持、非法范围、短预算与释放；尚未验收。此为后续单项/跨项公共注册编辑前置，不以内部拆分替代完整TJ或P3-05；1038项全量只证明此前冻结版本。

2026-10-09 最新一致版本workspace完成：原98627交互句柄在等待中断后丢失，但Cargo PID18018继续运行至退出；原日志/tmp/rzpdf-subset-edit-workspace-final.log包含52个执行组及52个成功终态，总计1038项通过、0失败/忽略/过滤，PDF库688、render库38及全部公共集成/三组零用例文档测试完成。总进程退出码无法从丢失句柄取回，不虚报exit0；全部组终态与进程退出、无错误日志及Rust/Cargo集合指纹ea43e3b9e2bff811f14ac2abd26bf0503a60b56a671f5b81835a38f26359fe5e保持已核对。源码冻结结束，下一步接入统一新字体替换片段和TJ；P3-05及整体目标未完成，未提交，fuzz暂缓。

2026-10-09 最新全量分项：98627日志/tmp/rzpdf-subset-edit-workspace-final.log确认PDF库688项全部通过，0失败/忽略/过滤，包含当前自动子集编辑三项及已有span/事务回归；同一会话继续adjusted_text公共集成，完整workspace未终态。Rust/Cargo文件集合内容指纹ea43e3b9e2bff811f14ac2abd26bf0503a60b56a671f5b81835a38f26359fe5e保持，不据库单元结果宣布完整门禁或P3-05完成。

2026-10-09 独立检查器反例加固：78711/93630分别Latin/CJK增强检查退出0；六个XML反例现在必须在指定检查点以准确错误拒绝，不能把其他ValueError当通过。新增两类字体反例：修复校验和后的错误head边界须在全局轮廓边界检查拒绝；完整来源字体冒充子集须在精确Unicode字集检查拒绝。Python -O下两套全部通过。98627原会话50秒复查仍活跃，未重启、未计全量通过；Rust源码未改。

下一实施顺序（2026-10-09，P3-05范围不缩减）：

- [x] 当前98627全量门禁终态核对，期间不更改Rust生产/测试/示例源码；52组1038项均成功终态，指纹保持；总退出码因句柄丢失未取得，具体证据与限制见上。
- [ ] 统一新增字体替换片段规划：复用TextCodePosition、原快照认证、有效字体度量、OperationReplacement和预算持有/事务发布。现有text_array_items_fragment接口同字体假设不能直接用于混合原码/新子集码；规划必须先输出旧字体前缀，局部q/Tf显示新码并可补偿，再Q恢复旧字体后缀，不能错误重编码其他片段。
- [ ] TJ数组自动子集入口：逐项保留原数字的顺序和值，支持字符串内边界、跨字符串、空串/插入/删除；以旧码解码验证原GID并聚合新字集。替换字体仅作用于新码，补偿按新旧字体各自度量，不吞掉既有间距。严格重开、原数字检查、共享页隔离、位置对照、预算失败/error/panic回滚与原快照重试必须通过。
- [ ] 跨事件/嵌套接入既有调用树：按原有效状态分别计量、仅在起点插入新码；Form局部资源及其他调用隔离，保留未知操作和夹子调用范围契约。不得维护另一套独立提交/修订号机制。
- [ ] 真实中文/Latin矩阵及最新全量复验后再评估P3-05验收；shaping、字体回退和布局适配按支持矩阵明确范围，不由单字符串专项推出完整字体支持。

2026-10-09 真实编辑字体独立复核：44564退出0，MuPDF从两份三页编辑PDF各抽取三份TTF；FontTools逐份验证EDITAA/EDITAB/EDITAC唯一身份、精确Unicode字集、原GID、完整复合依赖、未选轮廓清空、保留轮廓/指令/水平及可选垂直度量、head全局边界、表及全字体校验和与许可字段。Arial新子集各4标量/8 GID/2选中复合字形，中文新子集各2标量/3 GID。已将复核入口纳入check-subset-edit.py --fonts，可与位置检查和六反例一起复跑。98627最新workspace仍运行，源代码在该门禁期间不扩展；此字体专项不代替全量门禁或P3-05剩余编辑范围。

2026-10-09 真实自动子集编辑收口：84087退出0，automatic_subset三项通过（685过滤），空替换/事务隔离/引号与补偿矩阵通过。最新短替换Arial与NotoSC示例76682/73870均退出0并严格重开；两份PDF的qpdf检查、MuPDF三页文字/字体/字号/前缀与后续坐标检查及Python -O六反例均通过。六张96 DPI PNG逐页目视无缺字、重叠或裁切；补偿保留后续位置，自然模式左移。Latin默认stext在补偿空隙插入一个视觉空格，检查器固定记录该输出，不通用过滤字符。此次未独立复核新增字体表，亦未完成最新workspace全量门禁；下一步先全量回归，再复用既有选区/事务/预算机制接入TJ、跨事件与Form，不分别复制编辑引擎。P3-05与整体项目保持未完成，fuzz暂缓。

2026-10-09 自动单字符串补偿/引号专项：85917自然模式公共预算/error/panic回滚、来源不匹配拒绝与共享页资源/内容流保持退出0；70330补偿版两项退出0，30010引号lib检查退出0；34345扩展两项退出0，位置测试覆盖Tj/单引号/双引号×自然/补偿六场景，A→BB严格重开ABBAA，原字体前后缀与后续显示保持，补偿时后缀/下一事件x误差<0.002、自然模式推进差异>1，y保持。双引号显式Tw/Tc/T*副作用不局部撤销。后补空替换删除中间字符/原字体/释放另轮运行中。以上合成水平静态字体专项不替代真实新增字符坐标/视觉、TJ或跨事件/嵌套入口、完整workspace，P3-05未完成。

2026-10-09 实验自动单Tj子集编辑接入：新增公开document::subset_text_edit及SubsetTextEditOptions，显式完整来源/描述指标/唯一标签/辅助表省略；replace_page_text_string_with_subset先原快照认证，再在同一外层事务注册新子集和q/Tf/Tj/Q原操作替换，保留前后缀与共享旧字体，非目标页不变。17676 lib检查退出0；69729预算Tf内容构建13项通过；73662首轮公共自动编辑专项退出0，BAA/AAA严格重开、旧编码器/旧字体保持、半码拒绝、陈旧快照拒绝及外层error回滚/刷新重试通过。后补_and_budget显式池与极小额度拒绝/邻接租约释放另轮进行中，未计入首轮。自然推进模式仅单顶层Tj，补偿/TJ/引号/跨事件/嵌套及真实新增字符视觉仍未完成，P3-05保持未完成。

2026-10-09 单字符串规划专项终态：99248退出0，subset前缀12项通过（672过滤），包含来源固定head字段/比较工作额度、原CID输入额度及完整注册/严格重开中的前后缀保留AA→BA/ABA/AB/空、半码/倒序/越界/缺字/聚合字符超额和释放。随后新增ContentBuilder::font_selection_with_budget，栈缓冲格式化有限f32字号，直接转义资源名，输出实际capacity租约持有；与既有写出逐字节比较及短额度/非法输入释放测试另轮运行中。自动新子集注册/内容发布、TJ/跨事件/嵌套路径仍待实现，P3-05未完成。

2026-10-09 Unicode规划前置：49140退出0，注册/严格重开1项通过（683过滤），含借用Unicode/GID遍历重复顺序与回调拒绝、两遍预算解码输出及短额度释放；不含后补head固定字段/代码输入额度与plan_subset_string。新增单原字符串前后缀+替换规划，检查源覆盖、分割码边界、输入及聚合输出字符/UTF8字节额度，全部缓冲实际容量计费；测试覆盖AA→BA/ABA/AB/空、半码/倒序/越界/缺C/字符超额及释放，subset前缀另轮运行中。仍为内部规划，未注册新资源或发布内容，跨事件/TJ/嵌套自动编辑未完成，不以此缩减P3-05。

2026-10-09 CID预算编码专项终态：88645退出0，cid_encoding4项全部通过（680过滤），包括新增中文/nonBMP固定字集编码、未覆盖字符释放、相邻租约及短额度失败/重试，原公开编码接口保持不变。随后validate_subset_codes委托借用visit_subset_scalars，提供Unicode/GID回调用于前后缀规划，并新增两遍预算化decode_subset_codes；已补重复顺序、回调拒绝、UTF8输出/短额度释放，另轮运行中未验收。这些仅为内部前置能力，自动新增字符选区/事务资源注册尚未接入，不代表P3-05完成。

2026-10-09 自动编辑前置验证：84043来源兼容/聚合比较上限两项退出0；字符映射用例27208因测试访问私有ctx字段编译失败（退出101），改现有ctx()访问后88630退出0，注册/严格重开用例通过，覆盖正确CID→Unicode→源GID、未知码、半字符码及错误源cmap拒绝。生产validate_subset_codes借用Type0映射，不逐字分配String，缺失/空/多标量映射明确拒绝，尚不支持简单字体或shaping。新增UnicodeCidEncoding预算化编码方法及中文/nonBMP/缺字/额度/释放重试测试，另轮进行中；原公开固定编码接口不变。自动事务注册/选区规划与嵌套发布仍未接入，P3-05未完成。

2026-10-09 来源兼容性初版专项：72099退出0，1项通过（681过滤），同源接受、保留hmtx/glyf及unitsPerEm改动/截断拒绝与重试通过；不含后来累计工作限制。84043启动subset_source两项，覆盖后补聚合比较字节额度与恢复重试，尚未终态。完整自动新增字符事务/字符码边界与ToUnicode规划、原字体恢复和跨事件/嵌套均仍待接入，不以内部来源校验替代P3-05。

2026-10-09 跨字体度量专项终态：28408退出0，font_binding12项通过（669过滤），含精确推进、字符额度重试与跨字体分开计量/状态不匹配拒绝。新增validate_subset_source初版比较双向保留表、unitsPerEm/原GID数量及非空保留字形原始字节（允许最多3字节零填充），借用表片段无新复制；后补累计比较字节上限。它仅证明当前静态原GID子集的来源兼容性，删除轮廓无法证明缺失部分原始身份，不是密码学认证。72099正在验证此前版本的同源/度量/字形/单位错误/截断用例，未覆盖后补工作上限；尚未接入公共自动编辑，P3-05未完成。

2026-10-09 自动子集编辑前置度量：边界修正版PDF库84993退出0，679项全通过，0失败/忽略/过滤；随后font_binding提取advance_1000并新增跨绑定replacement_adjustment_to，旧入口委托同绑定路径，新旧代码各按所属字体计算，明确相同有效文本状态/书写轴前置，不以Unicode空格替代单字节0x20的Tw规则。补Courier精确推进/空文本/字符额度失败重试/零字号与释放测试；4643仅验证提取版，运行中，不包含后补跨绑定方法及新增断言。自动新增字符公共入口、来源匹配、事务注册及跨事件/嵌套矩阵仍未实现，P3-05不勾选。

2026-10-09 边界真实样本修正版验收：46394退出0，subset前缀10项通过（669过滤），含公共FontBBox注册/保存严格重开；90747/53660两生成退出0。提取字体位于/tmp/rzpdf-bounds-extract.QYG20U/font-0006.ttf与font-0008.ttf，MuPDF重新提取XML并96DPI重渲染三页；逐页查看无缺字/重叠/裁切。qpdf两PDF检查退出0。72665/93027独立FontTools检查在-O下均退出0：Arial17标量/26GID/5真实复合、中文44/45/0，head联合与轮廓/度量/文本正确，七固定负例均准确拒绝，含修复校验和但错误head框。以上为有限真实样本和专项证据，不代表全面字体符合性、新版完整workspace或自动新增字符/通用选区编辑完成；P3-05仍未完成，fuzz暂缓。

2026-10-09 边界组装专项终态：10140退出0，subset前缀9项全部通过（669过滤），含源head框扩大到[-200,-300,400,500]后子集重开回到[0,0,30,60]、源不变及精确预算邻界回滚。随后新增公共FontBBox描述符断言，覆盖注册前后保存/严格重开，46394正在验证，未计入前述9项。更新后的独立检查器在-O下检查旧Arial子集，明确于subset global outline bounds拒绝（退出1），证明旧产物不满足新增字段要求；该拒绝不是新产物验证成功。新样本重生成及七负例验收仍待完成。

2026-10-09 子集整体边界修正进行中：来源绑定GlyphClosure在既有迭代依赖遍历中记录轮廓存在性，联合保留字形头边界，忽略零轮廓及仅引用空字形的复合字形，全空结果为零框；布尔缓存/扩大的遍历栈实际容量纳入OperationBudget，写入head36..44，注册路径继续从重建字体度量生成FontBBox。43352专项退出0：闭包/重建5项通过，覆盖负坐标、空复合、缓存依赖顺序及既有预算失败/回滚；不含随后新增组装断言。独立脚本补FontTools展开轮廓端点排空及head联合检查；组装层源框扩大后重开缩框/源不变断言另轮运行中。真实样本重生成、PDF FontBBox直接验证及新版完整验收尚未完成，P3-05仍未完成。

2026-10-09 字体子集冻结版完整门禁终态：会话49844的cargo test --workspace --locked退出0；日志/tmp/rzpdf-subset-workspace.W44TxU共52组结果、1027项通过，0失败/忽略/过滤，零用例文档测试不计入通过项。源码清单摘要仍为39e3ff309032bdeace5cd213969efb30baaf7cdbc0a7ea8dc7c936d97f03a324，确认全程同版。解除本轮源码冻结，下一步优先修正已确认的head/FontBBox边界缺口；本次绿色门禁不证明该未覆盖字段正确，不代表P3-05或整体TODO完成，fuzz继续暂缓。

2026-10-09 最新完整门禁阶段结果：49844仍运行，编译阶段已完成（7m04s）；冻结版PDF单元677项全部通过，0失败/忽略/过滤，随后进入adjusted_text公共集成。font_data单元为零用例，不计入测试通过数。该单元结果覆盖冻结源码及辅助表策略，但不替代所有集成/render/文档测试终态；head整体边界已确认缺口仍待修正。继续同会话/同日志/tmp/rzpdf-subset-workspace.W44TxU，不改源码、不重启，P3-05未完成。

2026-10-09 冻结版整体边界缺口确认：完整门禁49844仍运行（本轮见cargo PID38654及create_document编译子进程，不因0CPU等待重启）。只读FontTools对产物核对head全局边界：Noto旧head(-1002,-1048,2928,1808)，实际保留非空轮廓头联合(16,-229,974,880)；Arial旧head(-1361,-665,4096,2060)，保留轮廓头联合(-3,-421,1574,1780)。现subset.rs仅改loca格式/校验和，未重建head边界，独立检查器也未覆盖该字段。依据[OpenType head规范](https://learn.microsoft.com/en-us/typography/opentype/spec/head)，全局边界应按有轮廓字形计算并忽略无轮廓字形。下一源码冻结结束后优先从来源绑定闭包重建该联合及PDF FontBBox，补无轮廓/复合/负坐标与预算失败测试、独立检查并重生成真实样本；不把当前渲染通过当完整字体符合性，P3-05未完成。

2026-10-09 冻结期验收文档收敛：90577退出0，扩展复合检查后的CJK正常输入/六负例在-O下复验通过（44标量、45GID、0所选复合），停止轮询。docs/font-cmap-validation.md统一omit_layout_and_auxiliary及已通过/未完成边界；P3-05仅新增三个范围明确的固定验收子项勾选，总项/自动扩充/通用选区仍未勾选。完整门禁49844本轮确认仍运行，日志仍在编译警告阶段；生产/测试/示例/render文件内容清单指纹39e3ff309032bdeace5cd213969efb30baaf7cdbc0a7ea8dc7c936d97f03a324（rg --files排序逐文件sha256再摘要，含未跟踪新源码），继续冻结，不因长启动或静默重启；fuzz暂缓。

2026-10-09 真实复合闭环终态与完整门禁启动：93148生成退出0，output/pdf/rzpdf-latin-subset.pdf严格重开精确文本通过；qpdf结构、MuPDF独立文字及96DPI目视无重音缺失/裁切/重叠。检查器--latin明确要求>=5真实复合字符；86959退出0，17标量/26保留GID/5复合、原组件GID/轮廓/度量/指令/许可及六固定负例均通过，停止轮询。按PDF技能使用既有工具，Arial源未修改或入库。最新版检查器CJK回归90577运行中。新模块/涉及入口与示例完成局部rustfmt，差异检查通过；启动cargo test --workspace --locked，日志/tmp/rzpdf-subset-workspace.W44TxU，源码从此冻结至该一致版本完整门禁终态，不以旧专项替代最新完整结果。自动覆盖扩充/通用选区和其他阶段尚未完成，P3-05不勾选，fuzz暂缓。

2026-10-09 新增字符隔离专项终态：4145最新版单事件版退出0，九项子集专项全部通过、668过滤，含BA/AA与尾部字体恢复、外层失败/重试及共享资源隔离；停止轮询。68757较早整页版也退出0九项通过，停止轮询，不将其替代单事件证据。两版本不覆盖后补辅助表策略；93148实际复合示例仍在编译运行。P3-05未完成。

2026-10-09 真实复合样本接入待验收：依PDF技能完整读取流程，本机Arial.ttf只读FontTools核对é/Å/ö/ñ/ç均复合，fsType8可编辑嵌入；还包含LTSH/hdmx/VDMX/PCLT。按官方各表规范不盲目复制改写后的缓存/打印元数据，纳入显式允许布局+辅助省略并报告的策略（未知表仍拒绝）；公共布尔参数名明确omit_layout_and_auxiliary，原整字体入口不变。新增create_subset固定Latin复合样本示例，调用方显式指标/tag/辅助策略、严格重开核对文本；93148使用本机Arial生成output/pdf/rzpdf-latin-subset.pdf，编译运行中未验收，系统字体不复制入库。4145单事件与68757早期整页专项仍未终态，新表策略还须重验，真实独立轮廓/渲染及完整workspace仍待完成，fuzz暂缓。

2026-10-09 新增字符单事件隔离专项接入待验收：将前轮整页替换测试加强为既有replace_page_operation_sequences显式局部事件流程，两页AA共享原子集，仅第一页首个Tj替换为q/新字体Tf/编码B Tj/Q，严格重开期望BA/AA并核对尾部字体恢复原AAAAAA+Demo。外层失败与重试仍包含新字体注册和事件替换，原共享对象/另一页资源源码不变。新会话4145编译中未终态；旧68757整页版本仍运行，其结果不能冒充单事件版。该流程显式指定原操作/字体/字号，不是自动覆盖扩充或通用跨事件/嵌套选区API，后者仍待实现，P3-05不勾选。

2026-10-09 检查器负例终态与新增字符隔离专项：51393退出0，python3.11 -O正常输入及六个固定负例全部通过，拒绝阶段精确匹配，停止轮询。子集组装测试新增显式局部资源替换流程：两页共享A子集，另建B子集只替换第一页内容；核对原字体对象/第二页资源与内容不变、原编码器仍拒绝B、严格重开分别B/A。外层事务在新注册+内容替换后主动失败，检查保存字节恢复并随后重试成功，报告预算释放。最新68757编译中未验收；这是固定单事件整页内容替换流程，不代表任意选区/嵌套Form自动新增覆盖或公共字体扩充策略已完成，P3-05保持未完成，fuzz暂缓。

2026-10-09 独立子集检查器负例待验收：check-cjk-subset.py补精确MuPDF字体绑定/两行字号检查及--self-test，先验正常样本，再固定六负例（字体校验和损坏、整字体冒充子集、文字改动、缺页、误字体绑定、误字号）。只认可对应ValueError精确检查阶段，不把依赖失败等任意错误算通过；隔离临时目录生成控制数据，源不变，使用显式require而非优化时消失的assert。python3.11 -O会话51393运行中未终态，差异检查通过；这是确定性验证，不是fuzz，真实复合字体/新增字符编辑与完整workspace仍待完成。

2026-10-09 真实中文子集首轮闭环终态：22238生成退出0，output/pdf/rzpdf-cjk-subset.pdf两页；省略BASE/GDEF/GPOS/GSUB，构建peak546136字节。qpdf结构通过，MuPDF独立stext/96DPI两页渲染完成并逐页目视无缺字/重叠/裁切。新增只读docs/check-cjk-subset.py，FontTools独立验证44标量/45保留GID、全字体及各表校验和、原GID、所有所选轮廓/水平竖向度量/指令、未选空字形、版权许可/fsType及精确两页文本；17530退出0。原字体10595948字节→嵌入子集381196字节，PDF384596字节（不是完整项目性能验收）。26274最新八项专项全通过、668过滤，停止轮询。按PDF技能验证使用已有MuPDF/FontTools，未安装依赖；仍缺独立实际复合字体、编辑新增字符/共享隔离、检查器负例及最新完整workspace门禁，P3-05不勾选，fuzz暂缓。

2026-10-09 真实字体旧策略终态与新版验证启动：64943退出1，明确错误Unknown subset table policy: vhea，停止轮询；17774最新策略示例构建退出0，停止轮询，随后运行该已构建二进制生成同名中文子集PDF（新运行会话待终态，不是因静默重启旧进程）。新增组装竖向度量正常保留、缺单表及长度差1字节拒绝/释放测试，26274使用font::truetype::subset过滤覆盖组装/名称/策略，运行中未验收。真实字体独立校验/渲染和完整工作区仍待完成，P3-05不勾选。

2026-10-09 复合专项终态及真实字体表策略修正：47230退出0四项通过、671过滤，包含复合组件保留，停止轮询。FontTools只读检查实际静态Noto发现STAT/vhea/vmtx；官方STAT规范明确静态字体也可包含，故STAT改为保留（name记录ID及风格不变），不再误当变量输入；原GID/numGlyphs不变时保留vhea/vmtx，组装补成对存在/头长度/度量数量及数据长度检查，不代表新增竖排PDF入口。未知表仍拒绝，fvar/gvar等仍拒绝。官方依据[STAT](https://learn.microsoft.com/en-us/typography/opentype/spec/stat)、[vmtx](https://learn.microsoft.com/en-us/typography/opentype/spec/vmtx)。64943旧样本进程仍运行，预期旧策略拒绝但未终态不重启；最新示例构建另会话进行中，尚未验收后补策略，真实产物闭环与P3-05未完成。

2026-10-09 修正版公共专项终态及真实样本启动：55918退出0，三项组装/公共跨页严格重开/对象中途失败回滚全部通过（671过滤），覆盖权限夹具版本3与基础名称修正，停止轮询。47230复合版仍未终态。create_cjk新增--subset（旧默认不变），明确允许布局省略并打印报告/预算peak；64943使用既有静态Noto字体/tmp/rzpdf-outline-font.nFalKk/font-0008.ttf生成output/pdf/rzpdf-cjk-subset.pdf，运行中未验收。依PDF技能已完整读取流程，后续用现有MuPDF替代缺libtiff的Poppler，字体工具/结构/提取/PNG视觉核验尚未完成，P3-05不勾选。

2026-10-09 复合组装专项接入待验收：自建字体夹具增加A为复合GID1引用简单GID2，选A但不选B；新测试验证输出cmap不覆盖B但GID2轮廓保留、组件仍引用原2、依赖重验访问两字形及释放。maxp夹具同步复合点/轮廓/组件深度上限。最新47230编译中，尚未终态；55918修正权限/名称版仍运行。较早32013退出0两项通过，但不含后补禁止子集/非法tag断言及中途失败测试，不替代最新版本证据；停止轮询32013。差异检查通过，尚不代表实际中文字体独立渲染验收，P3-05未完成。

2026-10-09 公共专项失败定位与修正：24008退出101，组装及中途对象失败回滚两项通过，公共跨页测试失败于禁止子集断言；夹具OS/2版本0按既有规范忽略后定义高位，改为版本3后使用0x0100，没有放宽生产权限规则。另修复基础名称为空加tag后误接受，公共入口预检基础名称1–56字节/旧字符集、描述数值和PDF版本，补空名称/+和/非法断言。最新55918运行中未终态，32013仍未终态，不以旧构建覆盖修正；docs/font-cmap-validation.md记录实验入口和预算/支持边界。P3-05继续未完成。

2026-10-09 公共子集注册中途失败专项接入待验收：新增对象额度10/预占至ID8夹具，使FontFile2和描述对象插入后第三对象失败；核对ID1–8逐对象相等、页资源恢复、下一插入编号9以及临时子集预算归零。不用低额度保存限制替代对象状态检查。最新专项24008编译中（含前轮后补权限/tag断言），尚未终态；原32013已启动但仍未终态，继续观察同句柄不因静默重启。尚需取得专项结果及真实中文/复合字形独立闭环，P3-05不勾选，fuzz暂缓。

2026-10-09 公共子集运行专项接入待验收：组装模块增加公共ResourceManager测试，自建简单轮廓字体注册后编码AA并拒绝未选B，跨两页复用句柄/写入文本，严格保存重开核对两页各两个A及ABCDEF+Demo名称；tiny预算、禁止子集fsType及非法tag检查文档保存字节不变，句柄报告drop释放预算。会话32013编译中，但禁止子集/非法tag断言在启动后补入，须确认编译覆盖或随后重跑，不把之前组装测试冒充公共验收。差异检查通过；真实中文/复合字形、独立字体工具/渲染及注册中途对象失败仍待完成，P3-05不勾选，fuzz暂缓。

2026-10-09 子集组装专项终态：38183退出0，组装1项通过、671过滤，覆盖原GID/字符覆盖/空未选字形/校验和/源不变及精确预算邻居拒绝释放重试，停止轮询。该二进制在后补公共入口前构建，不代表公共入口运行验收；不把自建简单轮廓夹具替代中文复合字形和独立工具闭环。

2026-10-09 公共子集入口编译终态：56311退出0，cargo check -p pdf --locked通过，停止轮询；有80条警告，不声称无警告或运行测试通过。公共入口专项尚未添加，原组装38183仍未取得终态；继续验证创建/保存/回滚及字体闭环，整体目标保持进行中。

2026-10-09 显式公共子集入口接入待验收：ResourceManager::embed_truetype_subset_font接受借用源、固定字集、描述参数、调用方文档内唯一六大写字母tag、显式布局省略选项与输出操作预算；检查subset权限、构建子集后复用字体资源事务，统一name ID6/FontDescriptor/两个BaseFont名称。返回RegisteredSubsetCidFont包裹原字体句柄并保留可查询的省略表报告及其预算租约；旧embed_truetype_font签名与整字体名称验证不变。子集字节租约保持至注册完成，文档长期分配及旧解析器内部分配不在该临时输出池范围，不宣称全事务预算化。最新cargo check会话56311运行中；组装专项38183先前已启动但未终态，不能覆盖后补公共入口。尚需公共创建/保存重开/错误回滚及共享资源隔离专项，真实字体独立验收与P3-05均未完成，fuzz暂缓。

2026-10-09 组装初版编译检查终态：89803退出0，cargo check -p pdf --locked通过（有既存及未接入内部模块警告），停止轮询；该构建在maxp后补检查之前启动，不把它宣称为后补版专项通过。38183已取得锁并编译最新版单元测试，尚未终态；组装有效字体专项和真实字体独立验收仍待结果。

2026-10-09 子集组装专项接入待验收：新增自建SFNT夹具，含真实简单三角轮廓、两字符映射及必要名称/度量/权限表，验证子集重开后A原GID1保持、B不覆盖、未选GID2为空、numGlyphs保持、全字体校验和及源不变；补精确peak差1字节邻居拒绝/释放重试。该夹具不是独立工具验收或中文/复合字形闭环。组装入口进一步要求TrueType maxp版本1，避免沿用读入兼容的CFF版maxp。专项38183等待原89803构建锁；89803已进入pdf编译但尚未终态，不重启静默进程。P3-05与公共入口仍未完成。

2026-10-09 名称边界专项终态：84557退出0，最新版两项均通过、669过滤，包含语言引用和版本1/Mac/截断边界，停止轮询。该结果不覆盖后续新增subset组装层；89803编译检查仍在构建本地依赖，未终态。

2026-10-09 子集组装层接入待验收：新增subset::build_subset，将原GID闭包/glyf+长loca/cmap/name/head与显式表策略连接为SFNT输出；要求独立静态TrueType签名及必需表，检查subset=true/editable=true权限，并核对编码Unicode/GID/宽度与源字体一致。head副本设置indexToLocFormat=1，不修改输入；表清单、重建输出和省略表报告均持有操作预算，报告排序且随SubsetFont持有。现有Sfnt/TrueTypeProgram解析器内部普通分配及借用源字节明确不在该输出池统计范围，不能宣称全路径预算化。cargo check会话89803运行中，尚未终态；名称最新专项84557仍未终态。尚缺组装专项有效字体测试、公共资源入口、完整字段语义及真实字体/PDF独立验证，P3-05不勾选，fuzz暂缓。

2026-10-09 表策略/名称专项终态与补边界：89296退出0，表策略1项通过（668过滤）；96449退出0，初版名称1项通过（669过滤），两会话停止轮询，不代表完整回归。随后名称入口增加高位语言ID引用和语言标签偶数字节校验，新增name版本1/Mac编码原标签保留、截断及越界/失败释放测试；最新会话84557运行中，尚未终态，旧96449不覆盖这些后补改动。字体组装、公共嵌入与真实PDF独立验收仍待实现，P3-05保持未完成。

2026-10-09 子集名称重建接入待验收：新增subset_name::rewrite_postscript_name，读取name版本0/1并校验名称及语言标签字符串范围；保留原表内容（含版权、许可、语言标签），追加ASCII或UTF-16BE名称并只重定向ID6记录。名称遵守63字符及PostScript字符限制，不支持的ID6编码/缺少ID6明确拒绝；输出持有实际capacity预算并检查16位字符串偏移和流额度。依据[OpenType name规范](https://learn.microsoft.com/en-us/typography/opentype/spec/name)。新增保留许可/Unicode名称/非法名称/预算释放测试会话96449进行中，未验收；表策略89296已构建完成、测试进程启动但未终态。尚未组装真实子集字体或统一PDF资源名称；P3-05不勾选，fuzz暂缓。

2026-10-09 子集表策略接入待验收：新增内部subset_tables模块，原GID/direct-CID静态TrueType策略显式区分保留、重建和省略；glyf/loca/cmap/head/name要求重建，原GID度量和hint表保留，DSIG省略。布局表省略必须显式允许，未知表及变量/替代轮廓/颜色等未支持表拒绝，不能用允许布局省略绕过未知依赖。该分类不是表语义验证，尚未接入组装与公共入口；需报告省略表、补名称重建并独立真实字体验证。依据[DSIG内容签名规范](https://learn.microsoft.com/en-us/typography/opentype/spec/dsig)及[maxp资源字段规范](https://learn.microsoft.com/en-us/typography/opentype/spec/maxp)。新增策略测试会话89296进行中，尚未通过；最新完整门禁仍为字体子集改动前1007项，P3-05不勾选，fuzz暂缓。

2026-10-09 子集cmap专项终态：原9028退出0，CID模块三项通过，包含新格式12中文/非BMP/共享GID、未选字符零映射及预算失败释放，停止轮询。当前PDF单元总数668（3通过、665过滤），不代表完整验收。下一步组装真实静态TrueType子集并接入显式权限入口；需明确head/度量/名称与未知表策略，独立字体校验和PDF提取/渲染仍未完成。

2026-10-09 子集cmap接入待验收：UnicodeCidEncoding::subset_cmap生成Unicode平台0/4格式12，固定字集中每个标量一个单点组、保留原GID，按已有排序字集输出；共享GID不合并标量，未选字符无映射。输出实际capacity持有操作租约，检查长度/流限额；不生成Windows BMP兼容format4，不支持变体选择/shaping映射，公共支持范围须明确。官方结构依据[OpenType cmap格式12](https://learn.microsoft.com/en-us/typography/opentype/spec/cmap#format-12-segmented-coverage)。新增中文/非BMP/共享GID、未选字符和额度失败释放回归，原9028编译中，尚未验收；未接入完整子集组装或公共嵌入。

2026-10-09 原GID重建专项终态：原89126退出0且实际输出包含后补重建测试，四项通过；最新来源绑定版26737也退出0，四项通过，含闭包边界/缓存深度及重建复合引用、未选空范围和释放。停止轮询两终态会话。当前PDF单元总数667（4通过、663过滤），不是完整回归。仍需重建cmap、明确度量/未知表策略、公共子集权限入口及实际字体独立验收；原GID策略不表示未选字形的旧cmap可原样保留。

2026-10-09 原GID字形表重建待验收：GlyphClosure借用原GlyphLocations，重建入口不接受另一字体的位置表，避免同numGlyphs来源替换。保留原GID/numGlyphs，所选字形数据按四字节压紧、未选字形空范围、重建长loca并绑定两份输出预算；调用方仍须设置head.indexToLocFormat=1并明确cmap/度量/其他表策略。新增复合引用保持/未选空范围/缩小及释放回归。原89126较早来源参数版本执行中且新增测试启动后补入，不假定覆盖；最新26737来源借用绑定版本编译中，尚未验收，不是完整子集字体。

2026-10-09 SFNT容器专项终态：原80763退出0，较早一项结构测试通过；最新15013退出0，两项均通过，含排序/对齐/长度/整体校验及错误输入、精确预算失败/邻居保持/释放重试。停止轮询这两个会话。当前PDF单元总数666（本专项2通过、664过滤），仍不是完整验收；容器已验证固定结构，不是有效字体语义/子集字形/独立工具闭环，继续实施P3-05。

2026-10-09 SFNT容器写出实现待验收：sfnt_writer.rs接收显式借用表清单，预算化排序副本与输出共存，标签合法性/重复检查、四字节对齐、记录实际未填充长度及检查u32/输出限额；复制head后清零调整字段，生成表目录校验，再计算整个字体调整，不改输入。固定两表结构测试核对排序/对齐/长度/整体校验及解析；最新补非法标签、重复/缺失/截断head、输出限制和peak差1字节邻居保持/释放重试。原80763仅第一项版本执行中；最新15013两项版本编译中，均未终态。该容器不验证每张表语义或必需表完整性，两表夹具不是有效可绘制字体；子集glyf/loca/cmap/度量重建及独立真实字体工具验证仍待完成。

2026-10-09 SFNT基础专项终态：原77881退出0，目录枚举版3项通过；原60968退出0，最新目录/校验和版4项通过，包含虚拟零填充/回绕/调整算术。停止轮询两终态会话。当前PDF单元总数664（本专项4通过、660过滤）；不是664项完整验收。下一实现为预算化SFNT容器写出：显式输入表清单、标签排序/去重、四字节对齐、实际长度/offset检查、head清零后目录校验再整体调整；容器通过仍须重建glyf/loca与字符映射及独立字体验证，不能冒充字体子集完成。

2026-10-09 SFNT写出基础接入：新增内部借用table_records/tag访问，明确无序迭代必须由写出端排序，未知表也必须进入策略检查；新增未知标签及记录指针一致测试。新增无分配table_checksum，按大端32位回绕求和并虚拟零填充；head目录校验需先清零checkSumAdjustment，最终全字体校验调整另由写出端处理，依据[OpenType官方文件规范](https://learn.microsoft.com/en-us/typography/opentype/spec/otff#calculating-checksums)。补空输入/1–3字节尾部/回绕/整体调整算术测试。目录版原77881和最新校验和版60968分别待终态，不混用为同一版本；尚无实际SFNT子集产物或独立字体工具验收。

2026-10-09 修正版闭包专项终态：原71108退出0，3项全部通过，含256/255累计解析边界、精确共存峰值差1字节拒绝、邻居保持/释放重试及迭代器panic清理。原90080失败已由夹具修正后的本次结果收口，停止轮询终态会话。验证仅闭包模块，最新PDF单元总数662（3通过、659过滤）；SFNT写出/公共嵌入/新字符编辑及最新完整回归仍未完成。

2026-10-09 闭包边界失败与修正版：原90080退出101，两项通过、一项失败；32字节夹具先被Composite components/storage限制拒绝，未执行后续精确峰值及unwind断言。生产限制不变，夹具将复合字形范围填充到128字节，使双遍累计256字节接受/255字节精确累计解析拒绝，独立组件存储限制不再遮挡目标边界。修正版原71108编译中，尚未验收；旧32/31记录仅为失败夹具，不是通过证据。

2026-10-09 闭包补充边界待验收：新增第三项回归，复合单组件双遍解析32字节接受、31字节精确累计解析拒绝；测量实际scratch/result/组件共存peak，peak+7配7字节邻居成功、peak+6拒绝且保持邻居，释放后同池重试。根迭代器在一个根处理后panic，catch_unwind检查全部临时预算释放及原根重试。最新原90080编译中，不以较早70322两项证明新增断言通过。

2026-10-09 闭包专项终态：原70322退出0，glyph_subset两项通过，覆盖去重结果预算及循环/越界/工作额度/缓存长路径深度拒绝，停止轮询。当前PDF单元总数661（本专项2通过、659过滤），不能把过滤项当作通过；最新生产尚未完整工作区回归。后续需补聚合解析字节、精确 scratch/result共存额度及unwind验证，再接入SFNT子集写出；P3-05仍未完成。

2026-10-09 P3-05多根闭包已实现待验收：glyph_subset.rs复用GlyphLocations及预算化组件解析，迭代DFS去重并加入GID零；活动标记拒绝循环，完成子图缓存高度，命中时校验当前栈深度加缓存高度。状态结果、临时高度表/栈实际容量及组件集合均持有同池租约；根/唯一字形/组件边累计工作受max_objects限制，复合双遍输入费用累计受max_decoded_bytes限制。输入字体字节借用且由调用方负责预算；简单轮廓体未在此解码验证，不代表完整字形程序验证。新增两项覆盖去重、结果持有、邻居拒绝/释放重试、越界/循环/工作额度及先缓存短根后长根的深度拒绝；原70322编译中，尚未通过，未接入公共嵌入或SFNT写出。

P3-05预算化组件专项终态：原61184退出0，4项通过、0失败，覆盖最新预算化组件容量/邻居保持/释放重试及借用遍历三项；停止轮询该会话。仅该模块专项，不代表闭包、子集写出、编辑新字符或当前完整工作区验收。下一实现是多根去重字形闭包，须同时保留循环拒绝与最长依赖深度检查，不能因已见GID直接跳过而漏掉不同根路径的深度超限。

P3-05预算化组件收集已接入待验收：CompositeGlyph::try_new_with_budget先用无集合分配遍历校验/计数，再预留组件容量并校准实际capacity；返回BudgetedCompositeGlyph将Vec和租约绑定，借用访问、不暴露脱离租约的所有权转换。新增准确容量、邻居占用下operation live bytes拒绝、释放后同池重试、截断失败清理回归。最新原会话61184编译中，尚未通过；此前38759三项仅覆盖借用遍历版本。输入字体字节由调用方负责预算，双遍解析的聚合工作与闭包栈/集合仍需在后续闭包实现中约束，不能据此勾选完整依赖闭包。

P3-05接入专项结果：原38759退出0，复合组件3项通过，覆盖新增借用遍历与收集一致、回调错误传播、截断指令及既有变换/非末指令标志。原63960的CID单元2项通过，但命令未加--lib，仍在依次启动被过滤的公共集成程序，尚未整体退出；不因此重复启动。新增接入尚未完整工作区回归，闭包/子集写出/编辑新字符仍未完成。

P3-05复合组件接入进行中：CompositeGlyph新增内部visit_components回调遍历，共用已有flags/GID/参数/变换/指令边界校验，遍历本身不创建组件Vec；原try_new收集接口通过回调保留既有契约。回调可失败中止，但后续记录或指令仍可能校验失败，调用方必须丢弃已收集临时状态，不可边遍历边发布文档修改。新增结果一致/非末组件指令标志/回调错误/截断指令断言，原定向会话38759等待构建锁，尚未通过。无分配遍历不等于预算化闭包已完成，去重集合、栈、工作额度及子集写出仍待实现。

P3-05接入进行中：UnicodeCidEncoding新增内部借用glyph_mappings迭代器，提供(CID, Unicode标量, 原始GID, 宽度)，宽度及CIDToGIDMap序列化共用该来源；不分配映射副本、不合并共享GID的独立CID，CID零由后续子集规划显式加入。共享GID/非BMP断言已补入既有回归，定向测试原会话63960编译中，尚未验收。下述1007项完整门禁覆盖此修改之前的基线，不证明新增接口已完整回归；字体子集写出、依赖闭包及编辑新增字符仍未实现。

当前一致版本完整门禁已完成：原会话99029退出0，日志/tmp/rzpdf-indexed-consecutive-workspace.log，52组1007项通过，0失败、0忽略，包含657项PDF单元、38项渲染单元、公共集成及三组零用例文档测试。覆盖索引+连续完整路径复用、精确共存断言及后补额度阶梯，生产/测试源码diff指纹e5072c20de8453d64474c6dabd8de4b8182fd4aee2b1380d266320ac5a7292b6未变；本轮源码冻结结束。不以旧62635混合构建结果替代，整体TODO仍未完成，未提交。下一步审查差异并推进P3-05，不以当前回归通过冒充字体子集或完整项目完成。

当前实现：预算化调用树前缀索引保留首次出现节点ID；命中比较完整FormInvocation，连续完整路径相等时复用上一叶映射。实际表布局、扩容和输出共存费用计入预算；临时表返回前释放。完整选区专项14645已退出0，17项含精确共存额度/邻居保持/释放重试通过，日志/tmp/rzpdf-indexed-tree-consecutive-path-suite.log。公共及完整门禁尚未收口，不能扩大该专项范围。

当前验收证据：99029一致版本完整工作区退出0，1007项通过，包含当前15项nested_adjusted_text及2项same_invocation_span。当前源码真实TJ内部边界生成14322及独立验证69734均退出0，四页渲染/原间距/后缀坐标复验通过。63431较早索引版公共15项嵌套及2项同链、原33924的17项和29670精确额度专项均已通过；均仅各自当时范围，详见实施记录。

历史清理验证：单叶辅助函数移除不可达end_source分支后56361退出0（14项嵌套+2项同链）。原完整62635仍保留运行，但多个构建共享target，后续公共二进制可能已被替换，不能将它解释为统一当前版本门禁。完整差异收敛尚未完成，不提交。

索引优化前完整工作区终态：session11429退出0，`/tmp/rzpdf-unified-span-workspace.log`52组1005项通过，覆盖当时统一入口及精确预算断言；session48561退出0（51组1003项）只覆盖更早范围。均停止轮询，不据此证明当前索引版。

当前最新改动：同链端点夹子Form旧路径漏选已修正，公共span统一到展开规划；14项公共集成及656项PDF单元通过。同链/同事件专项session49141退出0，两项覆盖六种插入删除变体、精确存活内存额度拒绝及原端点重试、单叶契约对照和外层error/panic回滚。不同调用链、同链夹子Form及混合字体固定水平样本独立文字/坐标/渲染复验通过，范围见docs/cjk-creation-validation.md。完整工作区session11429退出0；旧workspace48561也已退出0，但不覆盖统一入口，不混用旧版门禁。

历史整工作区终态：session74387已退出0，`/tmp/rzpdf-current-owner-workspace.log`51组结果共1003项通过。仅覆盖当时的单片段表复用及后补断言；随后改动由上方session11429验证，整体目标未完成。

两轮工作区门禁均已终止：较早session54420退出0（1001项），最新session74387退出0（1003项）。不再轮询旧会话；后续修改需要按实际变更重新选择验证范围。

- [x] 将内部展开文本事件遍历改为预算化迭代帧栈。
- [x] 验证512层小栈成功、深度511拒绝、16/17空TJ事件配额及预算释放；证据：`/tmp/rzpdf-expanded-boundaries-validation.log`，命令退出0。
- [x] 完整工作区回归到终态；原session56430退出0。日志：`/tmp/rzpdf-iterative-span-workspace-validation.log`，PDF单元649项、render单元38项及全部集成/文档测试通过。后补事件配额/深度拒绝断言由session57302定向退出0独立验证；不宣称其已编入较早启动的workspace门禁。
- [ ] 检查并收敛提交差异，排除无关格式变更及来源不明的`test.png`，再保存本地检查点。
- [x] 接通跨不同Form调用的公共选区规划和单事务编辑；workspace session66920退出0，1001项通过；CID跨调用14项集成及不同Form对象四变体session73128定向通过。
- [x] 验收公共span的统一展开语义：同链夹子Form旧路径漏选已修正，定向/公共集成、错误/panic、同事件边界、精确额度拒绝重试和固定水平真实字体样本均通过；完整工作区session11429退出0（1005项）覆盖统一入口。保持显式单叶event_range的既有契约；本项只指统一入口及所列回归，不以有限样本证明全字体/全部选区或完整P3完成。
  - [x] 同链普通/补偿、错误拒绝及原端点重试、外层error/panic回滚和同事件六变体通过；session60487两项专项及14项既有集成、656项PDF单元通过。
  - [x] 固定水平Noto Sans SC同链夹子Form四页样本，qpdf、MuPDF精确文字/坐标、四负例、模式互拒及逐页PNG目视通过；详见docs/cjk-creation-validation.md。
  - [x] 同链2MiB存活内存额度下两模式重复超限拒绝、资源/对象/保存字节/stamp保持及原端点重试；字符限额4MiB，精确operation live bytes断言，session49141退出0。
- [ ] 扩大跨调用编辑的真实字体、图像/非文本邻接与独立阅读器视觉验收；基础公共入口通过不等于完整P3完成。
  - [x] 固定真实水平字体、共享两层Form的整事件及部分结束事件保留：qpdf、独立文字/坐标、负例与渲染通过；部分模式全13字符后缀位置保持。
  - [x] 路径/图像邻接四变体及严格重开验证；这是本库断言，不替代更多真实样本的独立视觉验收。
  - [x] 固定水平嵌入Noto Sans SC与标准14 Helvetica混合字体样本；独立字体名称/字号、文字/坐标及七个负例检查通过，不代表双嵌入或完整字体矩阵。
  - [x] 固定真实嵌入中文字体的TJ数组跨调用选区：结束于数组第一个字符串末尾，保留第二个字符串、-125/75间距及后续事件；普通/补偿两模式、严格保存重开、qpdf、MuPDF全13字符后缀坐标和四页视觉检查通过。字符串内部结束位置及更广TJ组合仍待扩展，不代表完整矩阵。
  - [x] 固定TJ字符串内部结束：第一个字符串包含中文和Hello，仅选中中文；保留同字符串Hello、第二字符串及-125/75间距。普通/补偿、严格重开、独立13字符坐标、结构和四页视觉通过；不代表任意字符串或全部TJ组合。
- [ ] 测量跨调用编辑的实际预算峰值，减少重复字体度量及逐事件完整程序重建；历史四调用小替换512 KiB拒绝在当前固定复杂标准字体样本中已不再出现，两模式四档额度通过；仍不证明最低需求、真实嵌入字体或整体性能验收。
  - [x] 每节点一次完整程序重建；事件临时字体/编码对象在序列化后实际释放，最新654项单元/14项集成及固定真实字体复验通过。
  - [x] 独立测量两调用四变体139736–141449字节、固定Courier四调用150571/153316字节；仅跟踪内部操作预算，不是RSS或最低额度。
  - [x] 合并乱序/重复索引和零额度拒绝保留donor与预算，释放重复片段后原计划重试通过。
  - [ ] 测量较复杂公共四调用样本及真实字体、大选区增长；完整工作区新版本回归、全流程计费边界与更多优化仍待收敛。
    - [x] 固定复杂四调用标准字体公共额度阶梯：512KiB/1MiB/2MiB/8MiB两模式均成功，严格重开、共享页、旧Form及补偿后缀坐标通过。只证明这些固定额度可用，不是精确峰值、最低预算、RSS或真实嵌入字体测量。
  - [x] 收敛选区自身调用树映射重复扫描并完成新版本门禁：from_paths前缀索引/连续路径复用，加上map_to_tree完整来源认证的既有leaf复用，避免该常用路径逐事件逐层arena搜索。宽度1/128/1024每叶八次测量、额度/身份负例、712项单元、26项公共回归、双向真实字体输出字节不变及最新完整工作区11693退出0（1063项）通过。不以路径长度总和代替节点额度；通用外来树仍保留搜索回退，不声称其复杂度已优化或整体性能验收完成。
    - [x] 优化前树构建基线：16/128/512重复及分叉来源的节点/叶映射、容量峰值、释放和宽树节点少1拒绝/恢复通过；仅树构建，不含外部来源或完整编辑，单次耗时不是稳定性能承诺。
  - [x] 验收规划程序发布复用：无选中子调用节点借用已验证计划，调用者计划表及租约持有至事务结束；有子节点仍按原操作索引合并文字片段和Do重绑。16项定向、14项公共集成、656项完整PDF单元及固定真实字体复验通过；session48561退出0，后续统一入口由session11429完整1005项门禁覆盖。不是移动所有权或裸克隆，不据此勾选整体性能任务。

fuzz按用户要求暂缓。以下为按时间记录的证据，旧记录中的“运行中/待验收”以较新的明确终态为准，不作为当前状态重复计算。

### 验证与实施记录

2026-10-08 当前索引混合字体/同链独立复验通过：原51039两份生成退出0，output/pdf/rzpdf-cjk-mixed-font-indexed.pdf及rzpdf-cjk-same-chain-indexed.pdf严格重开通过。两份qpdf、MuPDF精确文字/后缀坐标及-O负例通过；mixed七负例含三段字体/字号绑定，same-chain四负例。72DPI八页PNG分别与已验收旧模式四次cmp字节一致；mixed四页及same-chain新第一页面视正常，其余页与已查看基线一致。固定水平嵌入Noto+标准14及同链样本，不是双嵌入/真实竖排/全部字体矩阵。按PDF技能沿用MuPDF，源码指纹e5072c20de8453d64474c6dabd8de4b8182fd4aee2b1380d266320ac5a7292b6保持；99029继续集成，不提交，fuzz暂缓。

2026-10-08 当前冻结版本完整PDF单元阶段通过：原99029日志/tmp/rzpdf-indexed-consecutive-workspace.log显示657项全部通过，含索引/连续路径及精确共存预算，正在继续adjusted_text等公共集成；尚未完整workspace终态。按PDF技能启动原51039依次生成当前mixed-font/same-chain的indexed产物，已有编译当前样本缓存复用，尚待生成终态及独立文字/字体/坐标/qpdf/PNG复验。源码指纹仍e5072c20de8453d64474c6dabd8de4b8182fd4aee2b1380d266320ac5a7292b6，生产/测试冻结，fuzz暂缓，未提交。

2026-10-08 最新连续索引真实TJ复验终态：原14322生成退出0，output/pdf/rzpdf-cjk-tj-inside-indexed.pdf保存后严格重开通过；独立原69734退出0，qpdf、MuPDF精确文字/13字符后缀坐标、QDF原-125/75及-O四XML/三QDF负例通过。72DPI四页逐页目视无缺字/重叠/裁切，与优化前TJ内部样本PNG四次cmp字节一致。按PDF技能用引擎生成并沿用MuPDF替代缺依赖的Poppler，不改系统。仅固定水平真实字体TJ内部边界，不是所有字体/混合/竖排/完整门禁；原99029继续，冻结保持，fuzz暂缓，未提交。

2026-10-08 索引公共集成终态及当前门禁推进：原63431退出0，/tmp/rzpdf-indexed-tree-public-integration.log15项nested及2项same_invocation全部通过，覆盖连续快捷路径加入前的索引版；不混用成最新快捷版公共门禁。原99029编译7m22s后进入测试执行，真实TJ14322解除构建锁并继续原生成会话；源码指纹仍e5072c20de8453d64474c6dabd8de4b8182fd4aee2b1380d266320ac5a7292b6，冻结保持。未提交，fuzz暂缓。

2026-10-08 等待状态只读诊断：原63431同链子进程49259仍存在，CPU时间0、物理footprint12KiB；一次sample完成但无法获取二进制符号，仅主线程未知地址，报告/tmp/same_invocation_span-3c0687276370b2e3_2026-10-08_230026_YDOG.sample.txt。这些证据支持仍在启动阶段的推断，不证明具体系统阻塞原因或PDF测试逻辑死锁。当前完整99029有活跃rustc且子进程持续变化，真实字体14322等待构建锁，源码指纹未变；保留原句柄，不因静默重启或宣称通过，未改系统配置/生产/测试，fuzz暂缓，未提交。

2026-10-08 连续路径索引专项终态：原14645退出0，/tmp/rzpdf-indexed-tree-consecutive-path-suite.log17项全部通过，包含后补精确scratch/输出共存额度与邻居拒绝/重试断言。重复16/128/512来源elapsed_us25/87/117，tracked peak572/1468/4540；宽树117/579/1946微秒，3280/26016/103968字节。单次时间仅对照，不作稳定阈值；固定编辑全流程峰值仍与原版相同。源码指纹仍e5072c20de8453d64474c6dabd8de4b8182fd4aee2b1380d266320ac5a7292b6；原99029完整门禁继续，真实TJ原14322等待其构建锁，公共原63431同链待终态，未提交，fuzz暂缓。

2026-10-08 冻结版本验证继续：源码diff SHA-256仍为e5072c20de8453d64474c6dabd8de4b8182fd4aee2b1380d266320ac5a7292b6，原99029完整门禁编译中。原63431较早索引公共nested15项通过、同链未终态；原14645连续路径17项专项待终态。按PDF技能启动当前源码真实TJ内部边界复验原14322，日志/tmp/rzpdf-cjk-tj-inside-indexed.log，输出output/pdf/rzpdf-cjk-tj-inside-indexed.pdf尚未验收；使用引擎生成验证引擎，Poppler缺失沿用MuPDF，不改系统依赖。生产/测试冻结，fuzz暂缓，未提交。

2026-10-08 最新索引/连续路径一致版本门禁启动：原99029执行cargo test --workspace --locked，日志/tmp/rzpdf-indexed-consecutive-workspace.log待终态；自启动起冻结生产及测试源码，保留原14645快捷版定向、63431早期索引公共集成和62635历史完整会话，不因观察超时重启。新门禁不是重跑旧同一版本，而是覆盖新实现及后补测试的一致编译范围；fuzz暂缓，未提交。

2026-10-08 索引精确共存额度终态：原29670退出0，/tmp/rzpdf-indexed-tree-exact-budget.log专项1项通过。真实scratch/输出峰值加7字节邻居精确成功，少1字节operation live bytes拒绝后邻居保持，释放邻居原路径重试及live归零通过。只覆盖快捷路径加入前的索引版；最新14645仍待终态，未提交。

2026-10-08 连续路径快捷实现中：基于重复来源hash查找耗时增加的实测，仅对完整FormInvocation切片相等复用前一叶ID，路径/身份/偏移差异仍进入索引冲突检查；无新Vec或租约。新全选区专项日志/tmp/rzpdf-indexed-tree-consecutive-path-suite.log待终态，原29670/63431继续较早索引版精确额度及公共专项，不据其证明快捷版。原62635共享target后二进制可能混合构建时点，不声明统一版本完整通过。fuzz暂缓，未提交。

2026-10-08 索引版调用树专项终态：原33924退出0，/tmp/rzpdf-indexed-span-tree-suite.log17项全部通过（不含后补精确共存断言）。重复16/128/512来源elapsed_us51/197/946，tracked peak572/1468/4540；宽树282/528/2036微秒，3280/26016/103968字节。宽树512相较基线14991微秒更快，但scratch峰值增加71088字节；重复路径较慢，不宣称全场景优化。固定混合根/图像四变体、四调用及大选区全流程峰值与前版相同，释放/拒绝/重试及布局专项通过。新公共15项嵌套+2项同链原63431（/tmp/rzpdf-indexed-tree-public-integration.log）运行中，精确断言原29670仍待终态，旧完整62635继续仅优化前范围。未完整验收，fuzz暂缓，未提交。

2026-10-08 索引版精确共存预算断言补充：旧“第二张Vec失败”限额已不对应新增hash临时表的实际分配顺序，改用本平台独立探针峰值；加7字节邻居在精确额度成功，少1字节须精确operation live bytes拒绝并保留邻居，释放邻居后原路径重试成功。新原会话29670（/tmp/rzpdf-indexed-tree-exact-budget.log）待终态，原33924继续早一版全选区专项，原62635继续优化前完整门禁；不混用后补断言的覆盖范围，fuzz暂缓，未提交。

2026-10-08 调用树索引去重实现中：已移除逐来源深度前缀比较及已建节点线性查找，用预算化PrefixTable按父节点/原操作索引登记；同位点不同身份/字节偏移仍拒绝，唯一前缀限制含根节点。输出按首次出现ID填充，hash迭代不影响拓扑/叶顺序；表增长与输出共存实际收费，释放临时索引后返回。新span_tree全专项原33924运行中，未验收；原62635继续作为优化前基线，不混用。模块旧“公共入口未接通”注释同步修正，fuzz暂缓，未提交。

2026-10-08 TJ间距自动检查实现并通过：check-cjk-span.py新增--tj-qdf（须--partial），两种边界的QDF原始/重建五个间距数组各核对-125/75及顺序；额外尾事件补偿TJ不误计为原数组。末尾/内部样本正例及三个QDF变异负例通过，内部-O及混合字体七负例、同链四负例回归通过。初版固定所有TJ数量为五导致有效额外补偿数组拒绝，检查范围已纠正；生产/PDF未改，不是通用解析或输入同源认证，完整原62635继续，fuzz暂缓，未提交。

2026-10-08 调用树基线终态：原会话41714退出0，/tmp/rzpdf-tree-construction-baseline.log专项1项通过。重复路径16/128/512来源均3节点，tracked peak296/1192/4264字节，elapsed_us138/66/301；分叉路径18/130/514节点，峰值1136/8304/32880字节，时间50/1217/14991微秒。宽树节点额度少1精确span node count拒绝、live归零及恢复重试均通过。单次时间含环境波动，不据此作通用复杂度或RSS结论；后续索引优化可复用现有真实布局计费TableAllocator，须计入临时表与输出共存成本。完整原62635继续，657项单元总数仅表示新测试编入定向二进制，不表示全部657项重跑通过，fuzz暂缓，未提交。

2026-10-08 调用树测量基线实现：新增invocation_tree_repeated_and_wide_path_measurement_baseline，16/128/512来源分别重复同一路径或共享首层后分叉，核对唯一节点/叶映射、跟踪峰值及释放；宽树节点额度少1精确拒绝并恢复重试。日志/tmp/rzpdf-tree-construction-baseline.log定向待终态；时间仅测树构建，调用方来源和借用表排除，非整编辑/RSS或稳定性能阈值。只加测试，生产未改；原62635不覆盖后补测量测试，fuzz暂缓，未提交。

2026-10-08 四调用精确内存拒绝终态：原会话9336退出0，/tmp/rzpdf-four-call-exact-memory-quota.log专项1项通过。16MiB字符限额下8MiB替换在两模式均精确operation live bytes拒绝，stamp/资源/父子对象/完整保存不变，原root及端点小替换重试并严格重开成功。该证据替代此前仅ResourceLimit任意分支的内存解释；额度阶梯四档仍全部成功，两专项不同用途。完整原会话62635继续集成阶段，尚未终态；仅测试加固，生产未再改，fuzz暂缓，未提交。

2026-10-08 四调用超限证据加固中：审阅发现既有8MiB替换测试的默认1MiB字符限额可先拒绝，不能单独证明内存预算。该专项改为16MiB字符限额和精确operation live bytes错误断言，8MiB操作额度不变；新定向日志/tmp/rzpdf-four-call-exact-memory-quota.log待终态。仅改测试，生产未改；完整原会话62635继续，不以它证明后补断言已编入。fuzz暂缓，未提交。

2026-10-08 复杂四调用额度阶梯终态：原会话71264退出0，/tmp/rzpdf-complex-four-call-budget-ladder.log专项1项通过。普通/补偿分别在524288、1048576、2097152、8388608字节额度全部成功；严格重开、AXHCD结果、共享页原文、原父子Form及补偿后缀坐标检查通过。此次没有任何拒绝，不能声称新增测试的拒绝分支已执行，超限回滚证据仍来自既有专项。历史该样本512KiB拒绝不能再作为当前状态。完整原会话62635仍在PDF单元阶段，未终态；整体性能/真实字体任务未完成，fuzz暂缓，未提交。

2026-10-08 复杂四调用公共额度阶梯实现：新增complex_four_call_span_fixed_budget_ladder，固定Courier/Helvetica局部与继承状态、负Tz及两层四调用Tj/TJ样本，两模式各在512KiB/1MiB/2MiB/8MiB新文档中编辑。精确operation live bytes拒绝时检查stamp/资源/保存不变；成功时严格重开、共享页和补偿后缀坐标保持，原父子Form两分支均不变。定向原会话71264日志/tmp/rzpdf-complex-four-call-budget-ladder.log待终态，尚无各额度结论。完整原会话62635继续运行，不据它证明后补新测试编入；生产未再改，不声明真实嵌入字体峰值、最低预算或RSS，fuzz暂缓，未提交。

2026-10-08 单叶分支清理定向终态：原会话56361退出0，日志/tmp/rzpdf-single-leaf-cleanup.log，nested_adjusted_text14项及same_invocation_span2项全部通过。完整工作区原会话62635仍运行，继续原句柄；不以定向16项代替全工作区。预算API检查确认Limits::operation_budget每次新建独立池，公共span没有暴露该池，外部示例不能从另建池读取其真实峰值；复杂公共样本可做固定额度阶梯验收，精确峰值需内部定向测量，不为测量新增公开API或误称RSS。fuzz暂缓，未提交。

2026-10-08 清理后门禁推进：原会话56361的nested_adjusted_text已14项通过，同链专项仍在原会话运行，未重启。启动当前清理版本完整cargo test --workspace --locked，日志/tmp/rzpdf-single-leaf-cleanup-workspace.log，待终态；旧1005项仅作为清理前证据。TJ文档补齐生成、qpdf、MuPDF、-O、渲染及QDF数值复现命令，修正末尾/内部边界范围歧义；整体差异审阅及性能任务未完成，fuzz暂缓，未提交。

2026-10-08 片段合并预算审阅：OwnedTable按实际Vec容量收费；finish_text_event_fragments先检查聚合事件/字符上限和严格原索引顺序，预分配目标及借用表，完成完整程序解析后才take donor。单donor转交原表，多donor按预留容量append；空donor集合仍有已分配的空目标表，未发现expect空路径。该局部审阅不替代完整差异审阅或性能验收。定向原会话56361仍活跃，进程30875及其测试子进程31244确认存在，保持原句柄、不重启；测试结果待终态，fuzz暂缓，未提交。

2026-10-08 核心差异局部审阅：核对mod接入、公共span统一入口、原调用对象/字节位置认证、图像Do排除及单事务bottom-up发布。规划表由调用者持有，叶节点借用计划，重建父节点按原索引合并，复制Form的实际对象及预算租约保留到事务结束；未发现本段需要架构重写的问题。移除显式单叶辅助函数的不可达end_source分支，两个调用者原本均为None；保留统一span的独立端点认证。会话56361定向测试运行中，未声称当前新生产版本1005项已通过；仍有其它差异待审阅，fuzz暂缓。

2026-10-08 TJ内部结束样本终态：原会话22711退出0，output/pdf/rzpdf-cjk-tj-inside-span.pdf生成且严格重开通过。qpdf、MuPDF精确文字和13字符后缀坐标、Python -O四负例均通过；QDF五条原始/重建TJ均保留-125/75，结束调用保留同字符串Hello及第二字符串。四页72 DPI PNG逐页目视无缺字、重叠、裁切；普通后缀左移，补偿后缀原位置保持。只改示例/文档，生产未改，fuzz暂缓，未提交。

2026-10-08 TJ内部结束样本启动：compare_cjk_span新增--tj-inside，数组第一字符串为中文+Hello，端点仍为中文编码长度，第二字符串为空格PDF。原会话22711确认仍运行，日志/tmp/rzpdf-cjk-tj-inside-span.log，尚未终态；只改示例及待办，不宣称验收通过，fuzz暂缓，未提交。

2026-10-08 真实中文TJ样本终态：原会话47106退出0，output/pdf/rzpdf-cjk-tj-span.pdf生成成功并严格重开。qpdf检查通过，MuPDF独立精确文字和13字符后缀坐标检查（--partial）普通及-O四负例均通过；QDF内容流中原数组及普通/补偿重建均保留-125/75，补偿新增数值不取代原间距。四页72 DPI PNG逐页目视无缺字、重叠或裁切；补偿后缀保持原位置，普通后缀左移。这里只覆盖固定水平字体、数组第一字符串末尾，不覆盖字符串内部结束或完整TJ矩阵。生产未改，未提交，fuzz暂缓。

2026-10-08 真实中文TJ样本实现中：compare_cjk_span新增--tj，使用两个编码字符串及-125/75两种符号的数组间距；端点明确选择array_item=0并保留结束事件的英文字符串和后续PDF。生成会话47106已启动且原句柄确认仍运行，尚未获得生成终态、独立间距检查或视觉验收，不勾选TJ任务。生产引擎未改；fuzz暂缓，未提交。

2026-10-08 混合字体独立检查加固通过：check-cjk-span.py逐段验证基线文字对应NotoSansSC/24、Helvetica/18、NotoSansSC/24，新增子字体/子字号/父字体误绑定负例。最新mixed-font XML普通与-O均七负例拒绝且正例通过，默认/partial/same-chain三模式旧四负例回归通过。仅检查脚本改动，无生产引擎或PDF产物变化，1005项生产门禁仍适用；完整字体矩阵及全部TODO仍未完成，fuzz暂缓，未提交。

2026-10-08 固定混合字体独立验收终态：session27044退出0，mixed-font产物qpdf、MuPDF精确文字/后缀坐标、四负例及-O通过；XML中真实嵌入NotoSansSC与Helvetica分别出现，脚本新增字体区分断言后复验通过。新第一页面目视字体/字距正常，第二至四页PNG与same-chain已验收图cmp一致。详见docs/cjk-creation-validation.md；仅水平CJK+标准14混合，不声明完整字体矩阵完成。只改示例/检查脚本，生产1005项门禁仍适用，未知test.png保留，fuzz暂缓，未提交。

2026-10-08 混合字体真实样本启动：compare_cjk_span新增mixed-font模式，同链父端点采用真实嵌入Noto Sans SC，夹子Form采用Helvetica/18Tf/2Tc，与父24Tf不同。独立脚本精确要求两句中文夹Hello PDF的基线及编辑你好！ PDF和末尾坐标，已有默认/partial/same-chain XML回归四负例通过。生成session27044日志`/tmp/rzpdf-cjk-mixed-font-span.log`运行中，未验收；待独立字体区分、qpdf、文字/坐标与PNG。只是固定水平嵌入CJK+标准14字体，不宣称双嵌入字体或完整字体矩阵。生产代码未改，1005项工作区门禁仍覆盖生产范围，不覆盖后补示例；fuzz暂缓，未提交。

2026-10-08 统一入口当前完整workspace终态：原session11429退出0，cargo test --workspace --locked，`/tmp/rzpdf-unified-span-workspace.log`52组1005项全部通过，PDF单元656项、render38项、公共嵌套14项和最新同链专项2项、全部集成及文档测试终态。较早原session48561同时确认退出0，`/tmp/rzpdf-planned-leaf-reuse-workspace.log`51组1003项，仅旧范围。两原会话不再轮询；当前入口统一验收子项完成，不标记整个项目/P3/性能总任务完成。无生产变化，diff空白检查通过，未知test.png保留，fuzz暂缓，未提交/推送。

2026-10-08 展开来源与分组局部审阅：检查visit_expanded_text_span/events、validate_span_selection及group_events；端点匹配原完整调用路径+操作索引并验证字节位置，词法边界先检查，字体码边界由后续计划检查；组内原操作顺序严格递增，发布准备重新认证根快照，图片Do不递归为文字容器。未发现本段需即时修正的生产问题，不表示整个模块已审阅；仍有全页扫描和重复祖先定位开销。当前workspace11429原会话确认活跃编译，旧48561继续jpeg_embedding，不重启；内容编辑文档更新当前门禁范围，fuzz暂缓，未提交。

2026-10-08 统一入口当前完整工作区门禁启动：当前生产代码自入口统一后未再改变，656项PDF单元、14项公共集成、两项最新同链专项以及不同/相同调用链固定真实字体独立检查已通过。启动`cargo test --workspace --locked`，日志`/tmp/rzpdf-unified-span-workspace.log`，尚待终态；此为覆盖新生产版本的独立门禁，不是因旧会话观察超时重启。旧workspace48561仍活跃到image_transform，保留原会话且结果只归旧范围。生产差异局部审阅、diff空白检查通过，完整差异审阅仍未交付，未知test.png不动，fuzz暂缓，未提交。

2026-10-08 同链精确预算门禁终态：session49141退出0，`/tmp/rzpdf-same-invocation-span-memory-quota.log`两项全部通过。2MiB操作预算、4MiB字符限额，两种模式各重复两次2MiB替换均精确operation live bytes拒绝，stamp、资源、原父子对象和完整保存字节保持，原root/端点小替换成功；同事件六变体、单叶对照及外层error/panic回滚/新来源重试同时通过。生产代码未变，仍缺统一入口最新完整workspace门禁及更广字体/性能任务；fuzz暂缓，未提交。

2026-10-08 同链宽泛额度断言终态：原session35744退出0，`/tmp/rzpdf-same-invocation-span-quota.log`两项通过，但未区分字符额度和存活内存，不作为精确预算门禁。提高字符额度并精确要求operation live bytes的新版本已启动，日志`/tmp/rzpdf-same-invocation-span-memory-quota.log`，待终态；原会话已结束，不轮询/重复启动。生产代码未改，fuzz暂缓，未提交。

2026-10-08 同链预算断言防假阳性：默认max_text_chars为100万，2MiB替换可能先触发字符额度。此样本显式提高字符限额到4MiB，并要求ResourceLimit消息为operation live bytes，以验证实际存活预算拒绝。session35744已编译修改前断言，保留等待终态后再跑最新源；旧范围不能证明新断言。仅测试改动，未宣称验收。

2026-10-08 同链公共额度拒绝重试启动：same_invocation_span夹子Form用例改为2MiB操作预算，普通/补偿各重复两次2MiB替换请求，必须ResourceLimit且stamp、完整保存字节、页面资源和原父子对象保持；随后现有原root/两端的小替换继续成功断言。新增断言未验收，原656项PDF单元/14项公共集成不作为这组后补证据；最新定向session35744日志`/tmp/rzpdf-same-invocation-span-quota.log`。未变生产代码，原workspace48561确认活跃，fuzz暂缓，未提交。

2026-10-08 同链真实字体独立验收终态：生成session23177退出0，`output/pdf/rzpdf-cjk-same-chain-span.pdf`qpdf结构、MuPDF精确三句基线/你好！ PDF结果及后缀坐标、正常/-O四负例通过。四页72 DPI逐页目视无缺字/重叠/裁切，第二页图像与unified基线cmp一致；默认与same-chain模式互用均文字不符退出1。按PDF技能使用MuPDF渲染，不改系统依赖；只固定水平字体，不扩大为全部P3。生产代码未改，完整工作区48561继续旧范围，fuzz暂缓，未提交。

2026-10-08 同链真实中文视觉样本启动：compare_cjk_span新增与partial互斥的same-chain模式，父Form句首/句尾夹一个子Form中文事件（原三句），内置检查两端同链而中间不同链，要求编辑后仅你好！ PDF并检查保留后缀及共享基线。独立check-cjk-span.py新增same-chain模式，精确原三句/编辑文字和坐标检查，不沿用两句基线误认。现有默认及partial的unified XML用更新脚本正常/-O四负例回归通过。生成session23177日志`/tmp/rzpdf-cjk-same-chain-span.log`运行中，待独立qpdf/MuPDF/PNG验收；按PDF技能执行，生产代码未改，fuzz暂缓，未提交。

2026-10-08 统一入口真实字体复验终态：生成session95710/74173及独立检查session95377均退出0；两份unified产物qpdf结构、MuPDF精确文字/独立后缀坐标和四负例通过（部分模式python3 -O）。72 DPI八页PNG与reuse版逐页cmp一致，部分模式两编辑页目视无缺字/重叠/裁切；按PDF技能使用MuPDF，不改系统依赖。固定不同调用链样本覆盖最新生产入口，但同链真实字体视觉矩阵仍未交付。原workspace48561尚未终态且不覆盖统一入口，fuzz暂缓，未提交。

2026-10-08 公共span统一真实字体复验启动：默认session95710/部分结束事件session74173，以最新生产引擎生成`output/pdf/rzpdf-cjk-cross-span-unified.pdf`及`output/pdf/rzpdf-cjk-cross-span-partial-unified.pdf`；日志`/tmp/rzpdf-cjk-span-unified-default.log`、`/tmp/rzpdf-cjk-span-unified-partial.log`，尚待生成及独立qpdf/MuPDF/PNG验收。沿用PDF技能和MuPDF，不改系统依赖。此固定样本仍是不同调用链两端，仅证明入口统一后的既有真实字体路径无回归，不能代替同链夹子Form的真实字体视觉样本。原workspace48561继续原会话，fuzz暂缓，未提交。

2026-10-08 同链/同事件最新源专项终态：session60487退出0，`/tmp/rzpdf-same-invocation-span-boundaries.log`2项全部通过。新同事件六变体（普通/补偿×句首插入/中间零长度插入/整事件删除）以及原同链夹子Form、单叶对照、错误拒绝、外层error/panic回滚/重试均验证；严格重开、原Form与其他调用/共享页保持，补偿保留后缀坐标通过。生产代码未再改，最新656项PDF单元及14项公共集成证据仍适用，完整工作区及真实字体须按统一入口版本继续验收；fuzz暂缓，未提交。

2026-10-08 同链外层回滚终态及同事件边界启动：session25244退出0，`/tmp/rzpdf-same-invocation-span-rollback.log`同链两模式包含发布后error/panic回滚、保存字节/原对象保持、新来源重试及共享页严格重开全部通过。新增同事件句首/中间零长度插入及整事件删除，六变体要求原Form、其他调用/共享页不变，补偿保留后缀坐标；最新源定向日志`/tmp/rzpdf-same-invocation-span-boundaries.log`运行中，不把新增断言算入原已编译回滚门禁。生产代码未变，fuzz暂缓，未提交。

2026-10-08 公共span统一完整PDF单元终态：session27948退出0，`/tmp/rzpdf-unified-span-all-units.log`656项全部通过。生产代码未再变化；同链定向增加外层error/panic发生在实际编辑发布后的回滚，核对完整保存字节、原父子对象，获取新来源重试并严格重开检查编辑/共享两页；定向session25244日志`/tmp/rzpdf-same-invocation-span-rollback.log`运行中，后补断言未验收。原workspace48561仍为统一前版本，不借其门禁声明统一入口验收完成；fuzz暂缓，未提交。

2026-10-08 同链最新源定向终态：session94704退出0，`/tmp/rzpdf-same-invocation-span-final.log`1项两模式回归通过，含显式单叶event_range严格重开XKCAKBC对照、公共span XCAKBC、反向/非法偏移/不可编码拒绝、stamp/字节/对象保持与原端点重试。最新全部PDF单元session27948确认测试PID69642活跃，继续原会话；原workspace48561保留，不混算版本。fuzz暂缓，未提交。

2026-10-08 公共span统一初步集成终态：原session14895退出0，`/tmp/rzpdf-unified-span-integration.log`14项既有公共集成及1项同链夹子Form回归通过；两模式均删除中间K、保留原父子对象/共享页/其他调用，补偿C坐标及严格重开通过。后补拒绝重试和显式单叶对照尚需最新源复验，已启动独立日志`/tmp/rzpdf-same-invocation-span-final.log`；最新生产版全部PDF单元日志`/tmp/rzpdf-unified-span-all-units.log`同步启动。原workspace48561继续content_batch_edit，不覆盖统一入口，不重启或混用其证据；fuzz暂缓，未提交。

2026-10-08 统一span公共回归分项通过：原session14895日志`/tmp/rzpdf-unified-span-integration.log`中nested_adjusted_text 14项全部通过，同会话继续same_invocation_span，尚未完整终态。新增同一输入的显式单叶event_range普通/补偿对照，要求严格重开后XKCAKBC（保留子K），与公共span的XCAKBC相区别；此对照及此前后补拒绝断言未包含原已编译二进制，待其结束后验证最新源。没有新增生产改动、fuzz或提交。

2026-10-08 同链拒绝重试断言补充：same_invocation_span新增普通/补偿下反向选区、非法结束偏移、不可编码文字拒绝，要求stamp、完整保存字节、原父/子对象均保持，再以原root/端点成功重试。生产代码未再改变；原集成session14895确认cargo PID66527和nested_adjusted_text PID67070活跃，已编译范围不包含后补断言，待原会话结束后单独验证新增检查，不重复启动。较早workspace48561继续color_workflow，仅覆盖入口统一前生产版本。

2026-10-08 同链夹子Form缺陷确认及修正启动：session63920退出101，定向用例实际XKCAKBC而预期XCAKBC，确认中间子文字漏选。公共replace_nested_text_span两模式统一调用展开选区规划，显式单叶event_range不改；既有错误来源字节断言改为展开路径的ObjectError及精确消息，不取消拒绝检查。最新定向/14项公共集成日志`/tmp/rzpdf-unified-span-integration.log`运行中，未验收。原workspace48561基于统一前编译源，不能宣称覆盖本次生产修正；需另选最新门禁，fuzz暂缓。

2026-10-08 同链夹子Form回归启动：新增`pdf/tests/same_invocation_span.rs`，原文AKBCAKBC，两端为首次父调用的A和B，中间K来自子Form；连续展开选区替换应得XCAKBC。同时检查普通/补偿、原父子对象保持、第二调用/共享页隔离、补偿后C坐标及严格重开。现有实现预计保留K，但尚未观察终态，不作已确认缺陷声明；定向session63920日志`/tmp/rzpdf-same-invocation-span-before.log`。原workspace48561在新增文件前启动，不能作为该新用例验收；生产代码未改，fuzz暂缓。

2026-10-08 规划程序复用真实字体终态：session58000/86700均退出0，两种reuse产物的qpdf结构、MuPDF精确文字/独立后缀坐标及四负例通过，部分模式python3 -O通过。72 DPI八页PNG与owner已验收基线逐页cmp一致，部分模式编辑两页目视无缺字/重叠/裁切；详见docs/cjk-creation-validation.md。完整工作区原session48561继续等待终态，不重启、不借用旧1003项门禁；fuzz暂缓，未提交。

2026-10-08 最新规划程序复用独立验收启动：真实中文示例默认session58000、部分结束事件session86700，分别写入`output/pdf/rzpdf-cjk-cross-span-reuse.pdf`及`output/pdf/rzpdf-cjk-cross-span-partial-reuse.pdf`；完整工作区session48561，日志`/tmp/rzpdf-planned-leaf-reuse-workspace.log`。三条原会话保留，编译锁等待不算失败、不重新启动。内容编辑文档更新为真实借用生命周期及父节点原索引重建策略；fuzz暂缓，未提交。

2026-10-08 叶节点复用完整PDF单元终态：session12322退出0，`/tmp/rzpdf-planned-leaf-reuse-all-units.log`656项全部通过，0失败/忽略；当前生产优化已有完整PDF单元及14项公共集成证据，真实字体渲染和全工作区仍待复验。没有启动fuzz或提交/推送。

2026-10-08 叶节点复用公共集成终态：`/tmp/rzpdf-planned-leaf-reuse-integration.log`14项全部通过，原cargo及测试进程结束；覆盖CID边界/有符号度量、共享隔离、外层错误/panic回滚及额度拒绝重试。完整单元session12322尚待终态，不能沿用优化前1003项工作区门禁宣称当前版本全部验收。

2026-10-08 叶节点规划程序复用定向验证：`/tmp/rzpdf-planned-leaf-reuse-span-suite.log`终态16项通过，失败/panic回滚、预算释放及原端点重试包含在内。512事件内部预算峰值普通608485字节、补偿943836字节，较上一版764651/1322710下降约20%/29%；小样本峰值不变，不代表RSS或通用复杂度。公共集成进程PID57928/58698已确认活跃，不重复启动；完整PDF单元session12322启动，日志`/tmp/rzpdf-planned-leaf-reuse-all-units.log`。当前优化尚未全量验收，fuzz暂缓，未提交。

2026-10-08 最新workspace完整终态：原session74387退出0，cargo test --workspace --locked完整通过，日志`/tmp/rzpdf-current-owner-workspace.log`。51组共1003项通过，PDF单元656项、render38项、所有集成及文档测试到终态，涵盖片段重构、生命周期缩短、单表所有权复用及最新解析失败非额度假阳性断言；未运行fuzz。此前旧session54420亦1001项通过，但不与本门禁混算。当前收敛等待结束，尚需差异审阅与后续更广性能/字体矩阵、全部TODO实现；未提交/推送，未知test.png保留，不标记项目或目标完成。

2026-10-08 较早片段版本workspace终态：原session54420退出0，日志`/tmp/rzpdf-fragment-node-workspace.log`，51组结果合计1001项通过，完整测试及文档测试到终态。此门禁单元654项，不覆盖后补生命周期、单片段表复用及新增656项断言，不能声明最新生产版本全量验收完成。最新独立workspace原session74387仍活跃，当前image_transform，继续等待原进程；fuzz暂缓，未提交。

2026-10-08 最新完整工作区验收启动：当前生产版本656项单元、14项公共集成及真实字体两模式已有分项证据；为弥补较早workspace版本覆盖不足，独立启动cargo test --workspace --locked，session74387，日志`/tmp/rzpdf-current-owner-workspace.log`，尚未终态。旧session54420继续owned_document，没有终止或重启旧进程。无生产改动，diff空白检查通过；未知test.png保留，fuzz暂缓，未提交/推送，不声明全任务完成。

2026-10-08 解析失败测试预算假阳性排除：新增断言要求非法片段返回错误但不是ResourceLimit，防止额度不足导致测试未到达语法片段路径却误通过。session70871退出0，单/多donor定向两变体通过，保留所有者/预算及修复后重试检查仍通过，日志`/tmp/rzpdf-fragment-parse-stage-validation.log`。仅测试变更，最新全656项先前门禁不包含此后补断言；原完整workspace仍运行，fuzz暂缓。

2026-10-08 单片段表复用真实字体复验终态：session56828/98188生成退出0，最新生产引擎生成`output/pdf/rzpdf-cjk-cross-span-owner.pdf`及`output/pdf/rzpdf-cjk-cross-span-partial-owner.pdf`。两份qpdf结构、MuPDF精确文字/独立后缀坐标及四负例通过；72 DPI八页PNG与生命周期版本已验收图逐页cmp字节一致，部分结束事件编辑页目视复核无缺字/重叠/裁切。按PDF技能完成独立渲染，沿用MuPDF，不改系统依赖。取代顶部“单片段表复用后尚未再次复验”的状态，仍仅固定水平Noto Sans SC两层共享Form，不表示整个字体矩阵或性能验收；完整workspace原session54420继续nested_image_edit，fuzz暂缓。

2026-10-08 最新全部单元收敛终态：原session58455退出0，`cargo test -p pdf --lib --locked`656项全部通过、0失败/忽略，日志`/tmp/rzpdf-fragment-owner-final-all-units.log`。实际覆盖最新单片段表复用、大选区增长与512KiB拒绝重试、乱序/重复索引及新增解析失败所有者保持。未修改生产代码；公共集成14项证据仍对应当前生产版本，原workspace session54420较早版本继续至metadata，未终态。完整项目/P3/性能总任务不勾选完成，fuzz暂缓。

2026-10-08 解析失败所有者补验终态：session98268退出0，新增单元1项通过（单/多donor两变体），非法片段拒绝、donor及实际预算保持、修复后重试成功、结果释放live归零。日志`/tmp/rzpdf-fragment-parse-owner-validation.log`；生产逻辑未变，不宣称最新全656项已跑完。原workspace session54420继续至jpeg_embedding，未终态；fuzz暂缓。

2026-10-08 合并解析失败所有权测试：新增invalid_fragment_parse_preserves_single_and_multiple_donors，单/多donor分别以未闭合字符串片段触发解析拒绝，检查所有者和预算保持，修复为合法片段后成功消耗donor并释放预算归零。覆盖单表移交和多表预留路径，生产逻辑未变；session98268运行，日志`/tmp/rzpdf-fragment-parse-owner-validation.log`，未验收。此前655项不覆盖此新增单元，原完整workspace继续运行，fuzz暂缓。

2026-10-08 单片段表复用门禁终态：session14217退出0，最新PDF单元655项通过；session87737退出0，公共嵌套集成14项通过。覆盖大选区低预算拒绝/原端点重试、合并乱序/重复索引所有者保持及跨调用错误/panic回滚。未单独复测峰值或真实字体产物，不用此前测量声明该优化额外收益；原完整workspace session54420基于更早生产版本继续运行，当前image_replacement，不能替代此次全工作区验收。fuzz暂缓。

2026-10-08 单片段表所有权复用：finish_text_event_fragments只有一份donor时，解析成功后直接移交其实际元数据Vec及租约，不再为同一表分配第二份目标；多donor仍提前预留聚合表，所有fallible验证/解析先于消耗所有者。零donor仍保留空目标表，未改变公开签名/配额/事务。最新全部PDF单元session14217与公共集成session87737运行，日志`/tmp/rzpdf-single-fragment-owner-units.log`、`/tmp/rzpdf-single-fragment-owner-integration.log`，未验收，不复用旧655项或原workspace作为本次改动证据。fuzz暂缓。

2026-10-08 发布路径局部审阅：检查commit_prepared_span、build_prepared_bottom_up、SpanFormOutput/Retention及rebuild_parent_with_text_plan。当前事务包住底向上Form插入/局部资源重绑/根发布，源身份与预算准备在发布前验证，流与资源的实际所有者保留至事务结果消耗，父编辑借原操作索引片段而非改写程序索引。发现仍有规划完整程序到发布再次重建的开销，列为明确待评估项；不是已实现优化，也不声明整个3300行模块已审阅。原workspace session54420运行至image_hit，未终态；fuzz暂缓。

2026-10-08 差异与说明收敛：git diff --check退出0；核对mod.rs仅接入span_tree、nested_edit.rs仅公共跨调用分支/注释及内部复用可见性，未改无关模块。内容编辑文档同步最新655项单元、事件规模峰值和512KiB拒绝原端点重试，明确采样不是通用线性复杂度/最低额度。大块span_tree新增仍需完整审阅，不能仅凭小入口差异宣称全部差异无关项已排除；未知test.png保持未跟踪且未提交。完整workspace原session54420运行到image_extraction，未终态；fuzz暂缓。

2026-10-08 大选区拒绝重试及最新单元终态：session64093退出0，六组定向通过，512事件两模式在512KiB返回ResourceLimit、live归零、峰值不超过额度、完整保存字节不变，同一原快照/端点重试成功且共享页与严格重开保持。session54434退出0，最新PDF单元655项全部通过（含增长新项及乱序/重复索引后补断言），日志`/tmp/rzpdf-many-event-final-all-units.log`。原workspace session54420尚未终态，其较早单元654项不当作新增第655项覆盖。性能总任务、真实复杂字体及整体项目未完成，fuzz暂缓。

2026-10-08 大选区低预算拒绝扩展：512事件普通/补偿先以独立512KiB预算要求ResourceLimit，核对预算live归零、峰值不越额度和完整保存字节不变；随后沿原root/端点以独立预算重试，并继续原共享页/严格重开/未选位置断言。仅补测试，session64093运行，日志`/tmp/rzpdf-many-event-rejection-retry.log`；最新全部PDF单元亦启动，日志`/tmp/rzpdf-many-event-final-all-units.log`，均未验收，未借旧654项证明新增第655项。完整workspace原session54420继续至identity，fuzz暂缓。

2026-10-08 大选区事件增长终态：session33739退出0，六组普通/补偿定向全部通过；每调用8/64/256个Tj（两调用共16/128/512事件）的tracked peak普通139506/223452/764651字节，补偿143087/345810/1322710字节。成功live归零、XZ文字、共享页原文字、严格重开及未选根Z坐标保持。512事件补偿约1.32MB，明显高于小选区，不保证512KiB成功；16MiB为测量上限，不是最低额度/RSS，三个采样点不证明线性复杂度。新增1项单元需后续完整单元门禁；原workspace较早编译版本不覆盖此新增测试，fuzz暂缓。

2026-10-08 大选区事件增长测量启动：新增selected_span_many_events_measures_growth_and_releases_budget，固定Courier共享Form两调用，每调用8/64/256个Tj事件，普通/补偿六组。每次用独立16MiB预算测量完整内部选区编辑，检查live归零、增长峰值、文字XZ、共享原页及严格重开后未选根事件Z坐标保持。session33739运行，日志`/tmp/rzpdf-many-event-growth-validation.log`，未验收，不宣称线性复杂度/真实字体或最低额度。原workspace session54420继续运行，未重启；fuzz暂缓。

2026-10-08 重复索引补验终态：session78973退出0，共享祖先定向普通/补偿两模式通过；重复和乱序拒绝、真实所有者/预算保持、释放重复片段后回到原值及重试合并均通过。四调用峰值仍150571/153316字节；日志`/tmp/rzpdf-fragment-duplicate-validation.log`。当前收敛清单拆出已证实的重建、生命周期、峰值和失败重试子任务，不勾选整个性能任务。原workspace session54420继续至feature_versions，尚未终态；fuzz暂缓。

2026-10-08 重复片段索引回归补充：在真实共享祖先测试构造第二份同操作索引片段，要求finish拒绝重复索引、两份donor保持Some、实际存活预算不变；移回原donor并释放重复片段后预算回到原值，再沿原额度拒绝/重试链成功。仅补断言，未改变生产逻辑；session78973运行，日志`/tmp/rzpdf-fragment-duplicate-validation.log`，未验收。完整workspace原session54420仍活跃，不重新启动，不将其较早编译单元当作本断言证据；fuzz暂缓。

2026-10-08 片段乱序拒绝补验终态：session54485退出0，shared_ancestor_publication定向两模式通过。新增断言验证乱序finish拒绝且donor保持Some、原预算live不变，恢复顺序后零额度拒绝也保留所有者，再成功合并及释放回到基线；四调用完整操作峰值仍普通150571/补偿153316字节。仅补测试，不修改生产逻辑；内容编辑文档同步最新生命周期门禁、真实字体复验及两/四调用峰值，避免旧“待验收”误导。原完整workspace session54420运行至display_payload_budget，未终态，不借其覆盖后补断言，fuzz暂缓。

2026-10-08 生命周期优化真实字体复验终态：session18178/49100均退出0，以最新生产引擎重新生成整事件/部分结束事件模式，产物`output/pdf/rzpdf-cjk-cross-span-lifetime.pdf`及`output/pdf/rzpdf-cjk-cross-span-partial-lifetime.pdf`。两份qpdf结构、MuPDF精确文字及后缀坐标通过；脚本普通/-O均拒绝四负例。72 DPI八页PNG分别与优化前已验收图cmp逐字节一致，目视复核默认四页及部分模式编辑两页（部分基线与默认相同）无缺字/重叠/裁切或图片变化。PDF技能用于独立渲染验收，沿用MuPDF而不修改Poppler依赖。仅固定水平Noto Sans SC与两层共享Form，不能推广全部字体/竖排/TJ或整个P3；完整workspace session54420仍运行，当前进入content_program，fuzz暂缓。

2026-10-08 四调用峰值终态：原session24209退出0，定向测试两模式通过；全新严格重开Courier文档跨两层共享祖先四调用，内部replace_selected_text_span从展开选区、准备到事务发布的tracked peak普通150571字节、补偿153316字节，返回live=0。AXBC文字、共享页ABC×4、严格保存重开及补偿末尾B/C坐标保持通过；日志内callback panic为预期回滚用例。独立8MiB预算只作为测量上限，结果不包含调用方预先持有快照/字符提取、公共包装的前置端点认证或未跟踪分配，不是RSS、最低成功额度或全部字体性能验收。该测试不同于早期512KiB拒绝的较复杂四调用集成样本，不据此推断后者现在可在512KiB成功。原workspace继续运行，最新真实字体验收尚待复跑；fuzz暂缓。

2026-10-08 生命周期公共集成终态及四调用测量：原session3258退出0，nested_adjusted_text14项全部通过；最新生命周期版本已有15项选区模块及654项PDF单元通过。shared_ancestor_publication测试新增全新严格重开文档、独立8MiB预算下跨四调用整操作测量（原文ABC×4→AXBC），验证live归零、共享页保持、严格重开及补偿结束后缀位置。session24209定向运行，日志`/tmp/rzpdf-four-call-peak-validation.log`，尚无四调用峰值结论，不将两调用约41%改善推广；原workspace session54420继续运行，fuzz暂缓。

2026-10-08 生命周期全部单元终态：原session90213退出0，最新PDF单元654项全部通过，日志`/tmp/rzpdf-metrics-lifetime-all-units.log`。公共集成session3258与完整workspace session54420仍活跃，不能据单元结果宣称二者通过；下一步核对原进程终态、测量四调用并复验真实字体样本，性能待办不整体勾选。

2026-10-08 临时字体生命周期选区终态：原session22944退出0，选区模块15项通过（日志中的callback panic为预期catch_unwind回滚用例，最终0失败）。根/两调用含路径/图像邻接四变体tracked peak为139736/141056/140129/141449字节，相较片段重构前一版本236423/237743/236816/238136各减少96687字节，约40.6%–40.9%；只证明此固定样本的跟踪预算，不是RSS、四调用或所有真实字体。公共集成原session3258尚未终态；最新全部PDF单元已启动，日志`/tmp/rzpdf-metrics-lifetime-all-units.log`。原workspace继续运行，未混用版本证据；内容编辑文档合并了过时公共入口及阶段状态，fuzz暂缓。

2026-10-08 编辑临时字体生命周期收敛：确认Tj/TJ/引号片段返回独立BudgetedBytes，无字体或编码缓冲借用；PreparedEvent改为仅持有原索引、片段及操作数。完成边界验证、字符计数和补偿序列化后，事件临时BudgetedTextMetrics和编码BudgetedBytes随真实分配一起释放，不裸克隆、不仅退租约。新选区模块与公共集成日志`/tmp/rzpdf-metrics-lifetime-span-suite.log`、`/tmp/rzpdf-metrics-lifetime-public-integration.log`运行中（等待原workspace构建锁），未验收。原session54420的workspace以实际编译时点为准，不借其证明本次后补生命周期改动；fuzz暂缓。

2026-10-08 片段重构公共集成终态：原session41880退出0，nested_adjusted_text14项全部通过，包括跨四调用普通/补偿、合成CID横/竖排、共享页、额度拒绝后原端点重试与外层错误/panic回滚。完整workspace原session54420继续运行，未终态；本次不得声明整个工作区或真实字体视觉复验通过。下一步保留原进程核对完整门禁，并继续分析重复字体度量的实际持有成本；不提交未知test.png，fuzz暂缓。

2026-10-08 片段重构单元终态：session54847选区模块15项通过，涵盖新donor失败保留及重试路径；session10769全部PDF单元654项通过，均退出0。四变体tracked peak为236423/237743/236816/238136字节，比此前各少144字节；主要移除逐事件全程序构建，不将微小峰值改善解释为整体内存收敛。session34742为较早编译中间版本定向通过，不替代最终15项/654项证据。公共14项集成尚在原session41880，完整workspace已启动，日志`/tmp/rzpdf-fragment-node-workspace.log`，不得复用前次1001项作为本次全工作区结果。fuzz暂缓。

2026-10-08 多事件节点重建重构：新增TextEventFragments区分预算化片段与完整TextEventEditPlan；跨调用先生成原索引片段，再每节点一次完成完整程序重建，去掉逐事件完整程序与旧merge中间结果。原顶层/同叶入口通过包装沿用片段构建，保持公开签名；所有聚合配额/顺序验证、解析及分配仍在消耗donor之前完成，失败保留donor及租约。合并失败/预算释放测试改为直接覆盖新片段路径。最新选区模块`/tmp/rzpdf-fragment-node-span-suite.log`和14项公共集成`/tmp/rzpdf-fragment-node-public-integration.log`运行中，未验收，不借此前门禁声明本次重构正确或性能改善；fuzz暂缓。

2026-10-08 单计划优化定向终态：session24262退出0，根/两调用四变体及路径/图像邻接、共享页、失败重试、严格重开通过；session98221退出0，nested_adjusted_text全部14项通过，包括跨四调用CID、额度拒绝及外层错误/panic回滚。四变体tracked peak仍为236567/237887/236960/238280字节，与优化前相同：本次确实省去单donor的重复重解析，但没有降低该样本的全流程峰值，不宣称内存性能收敛。下一步需处理多donor逐事件完整程序重建及持有的重复度量；未重新运行完整workspace，fuzz暂缓。

2026-10-08 单计划节点重复重建优化：build_selected_span_plans在节点仅含一个非空donor时直接移交完整TextEventEditPlan及其预算所有权，跳过相同程序的merge重解析；多donor仍走原聚合路径，不放宽配额/事务约束。最新定向门禁日志`/tmp/rzpdf-single-donor-optimization.log`及公共集成`/tmp/rzpdf-single-donor-public-integration.log`运行中，未验收，不能借此前1001项证明本次生产改动。README同步修正基础版本的旧“待验收”描述，性能任务仍未完成，fuzz暂缓。

2026-10-08 部分结束事件真实字体验收：compare_cjk_span新增--partial，末次调用只删中文前缀，保留同一Tj中的Hello PDF及下一事件的空格PDF；session44663生成退出0，结果你好！Hello PDF PDF。qpdf结构、MuPDF精确文本及整个13字符后缀x/y独立坐标保持（0.002点容差）通过，普通控制左移至少10点。脚本普通/-O均通过且拒绝四负例，默认/部分模式互用均因text mismatch退出1。72 DPI第三/四页目视无缺字/重叠/裁切，前两页与已验收默认基线PNG cmp一致。生产引擎未变，未借此前workspace门禁声明新示例被覆盖；真实竖排/混合字体/TJ复杂选区仍待办，fuzz暂缓。

2026-10-08 独立跨调用验证器加固：`check-cjk-span.py`以显式ValueError检查代替assert，防止python -O禁用验收；argparse明确输入及--self-test，未知选项退出2。正常/优化模式均通过正例并拒绝四负例；复现生成、结构、stext、渲染命令与固定样本范围已写入`docs/cjk-creation-validation.md`。只加固检查脚本/文档，不改变PDF引擎生产逻辑，不将此结果推广为全部字体或P3完成。

2026-10-08 固定真实字体跨调用独立验收：四页72 DPI MuPDF PNG全部目视，无缺字/重叠/裁切；按PDF技能核对基线两行及原第二页图片、补偿/普通两页。新增`docs/check-cjk-span.py`读取MuPDF stext，四页精确文字、补偿PDF独立x/y原位（容差0.002）、普通控制向左至少10点且y保持通过；--self-test拒绝交换对照页/错误坐标/错误文字/缺页四负例，命令退出0。此前qpdf结构及本库严格重开通过。仅固定真实静态Noto Sans SC水平两层共享Form跨调用缩短，不代表真实竖排、所有字体、字形新增/排版或整个P3完成。Poppler缺依赖，使用MuPDF独立渲染，未修改系统依赖；fuzz暂缓。

2026-10-08 真实跨调用样本生成与首轮独立检查：session42311退出0，本库生成四页并检查补偿/普通末尾坐标、基线保持与严格重开；qpdf --check通过，MuPDF文字提取确认两基线句及两编辑页你好！/PDF。Poppler启动因缺libtiff.5.dylib退出134，改用现有MuPDF完成72 DPI四页PNG与stext XML，不修改系统依赖。已目视第3/4页，无缺字/裁切，补偿PDF保留右侧而普通左移；尚需第1/2页目视、独立精确坐标自动断言及负例，不能声明完整真实字体跨调用验收。产物`output/pdf/rzpdf-cjk-cross-span.pdf`，中间文件`tmp/pdfs/rzpdf-cjk-cross-span-*`，fuzz暂缓。

2026-10-08 真实字体跨调用对照示例：新增`pdf/examples/compare_cjk_span.rs`，复用真实嵌入字体构建共享两层Form两次调用、复制补偿/普通页，用双端来源span跨调用编辑。要求文本你好！ PDF、补偿末尾PDF位置保持而普通左移、基线不变及严格重开四页解释。session42311运行，日志`/tmp/rzpdf-real-cjk-cross-span.log`，输出`output/pdf/rzpdf-cjk-cross-span.pdf`，尚未验收；后续须qpdf、独立文本/坐标及渲染目视，不将本库坐标当作视觉验收。PDF技能用于独立渲染核验，仍由rzpdf生成样本，fuzz暂缓。

2026-10-08 邻接全单元终态：session94639退出0，最新PDF单元654项全部通过，日志`/tmp/rzpdf-cross-span-neighbors-final-units.log`，涵盖本轮路径/图像邻接及严格重开四变体；完整workspace1001项仍对应此前生产代码与CID版本，不混淆门禁范围。下一步真实字体跨调用视觉样本复用`create_cjk --images`输入、`compare_cjk_advance`的Form包装和`docs/check-cjk-advance.sh`独立坐标/文本核验，需新增跨调用样本与对应检查模式，不把现有同调用--nested/--events结果当作跨调用验收；独立工具pdftoppm/mutool/qpdf已确认可用，PDF视觉技能已读取。fuzz暂缓。

2026-10-08 图像邻接严格重开终态：session20191退出0，根/两调用四变体定向1项通过；两页图像数量、RGB像素、1×1尺寸、六分量矩阵在编辑后及严格重开保持，路径操作字节保持、文字/补偿后缀/共享页/失败重试同时通过。日志`/tmp/rzpdf-span-image-neighbor-roundtrip.log`，tracked peak四变体236567/237887/236960/238280字节，成功归零。已启动最新PDF全单元门禁`/tmp/rzpdf-cross-span-neighbors-final-units.log`，尚未终态；此前workspace1001项涵盖生产代码与CID集成，不覆盖本轮后补邻接断言，不把操作/像素提取当作独立阅读器视觉验收。fuzz暂缓。

2026-10-08 图像邻接编译修正及严格重开扩展：session77241退出101，新增测试使用不支持PartialEq的Matrix直接比较；改为比较a/b/c/d/e/f六分量，不改生产矩阵类型。补严格保存重开后两页图像数量、像素、1×1尺寸及矩阵保持断言。最新session20191运行，日志`/tmp/rzpdf-span-image-neighbor-roundtrip.log`，尚未验收；此前路径样本成功不证明图像扩展，fuzz暂缓。

2026-10-08 路径邻接终态与图像扩展：session37842退出0，路径邻接四变体定向1项通过，日志`/tmp/rzpdf-span-nontext-neighbor-fixed.log`。随后在两Form之间加入页面级RGB图像Do及独立矩阵，要求编辑页/共享页图像数量、像素数据、尺寸及矩阵保持，验证展开文字规划跳过图像；新增代码晚于该门禁，最新日志`/tmp/rzpdf-span-image-neighbor-validation.log`运行中，尚未验收，不宣称严格重开图像/独立视觉覆盖。

2026-10-08 邻接样本编译修正：session61927退出101，E0596来自新增测试中edited_page调用可变字符提取但变量未声明mut；已修正，仅测试代码变更。重新启动session37842，日志`/tmp/rzpdf-span-nontext-neighbor-fixed.log`，尚未验收；未借前一门禁或编译失败声明路径邻接通过。

2026-10-08 非文本邻接回归扩展：根/两调用四变体样本在叶AB与C事件之间插入独立填充矩形，在根中间文字前插入描边矩形；编辑后要求根和选中复制叶的原路径操作字节保持，仍检查XBC、共享页全文、普通/补偿后缀、失败重试及严格重开。最新定向session61927运行，日志`/tmp/rzpdf-span-nontext-neighbor-validation.log`，尚未验收；不借前一峰值或完整workspace结果证明改变后的样本，也不把操作字节保持当作独立视觉或图像邻接验收。fuzz暂缓。

2026-10-08 不同Form对象与峰值终态：session73128退出0，最新根/两调用定向1项通过，四变体普通共享230323、补偿共享231643、普通不同对象230716、补偿不同对象232036字节tracked peak，成功返回live=0；日志`/tmp/rzpdf-distinct-form-span-peak-validation.log`。这是独立预算下完整内部编辑的实际租约峰值，不含外部已持有快照或未跟踪分配，不是四调用峰值、RSS或最小成功额度。结合session66920完整workspace1001项及14项嵌套集成，基础公开跨调用入口任务可勾选；真实字体/独立视觉/更多邻接及性能优化继续待办，整体TODO未完成，fuzz暂缓，改动未提交且test.png保留。

2026-10-08 公共跨调用workspace终态：session66920退出0，`cargo test --workspace --locked`完整通过，日志`/tmp/rzpdf-public-cross-invocation-workspace.log`；PDF单元654项、渲染38项、全部PDF集成及文档测试通过，嵌套集成14项明确包含跨调用CID样本。不同Form对象/独立峰值变体不借此较早单元二进制验收，已启动最新定向`selected_span_prepares_root_text_between_shared_form_calls -- --nocapture`，日志`/tmp/rzpdf-distinct-form-span-peak-validation.log`，尚未终态。该门禁不代表真实字体视觉、性能优化及整个TODO完成，fuzz暂缓。

2026-10-08 workspace嵌套文字阶段终态：session66920日志明确显示nested_adjusted_text运行14项并全部通过，包含新增`cross_invocation_mixed_cid_span_checks_boundaries_and_signed_advance`，因此本轮该集成二进制实际编入了启动后新增CID样本；以实际测试名/计数为证，不再将CID样本列为未验收。覆盖合成Type0横/竖排、普通/补偿及B/空/B空格跨四调用、断码拒绝与重试、共享页/旧叶/严格重开及补偿后缀坐标。不同Form对象及独立峰值变体仍需最新单元定向验证；完整workspace现进入nested_image_edit，尚未终态，不能据阶段结果声明完整通过或真实字体视觉验收。

2026-10-08 完整workspace阶段结果：session66920编译完成，PDF单元654项通过、0失败/忽略，现进入adjusted_text及后续集成测试；日志`/tmp/rzpdf-public-cross-invocation-workspace.log`。进程仍存活，尚无完整workspace终态；晚于编译的CID/不同Form对象/独立峰值样本仍需最新定向验收，不借此阶段结果覆盖。

2026-10-08 不同Form对象覆盖扩展：内部根文字/两调用样本增加shared_form=true/false，与普通/补偿组成四种变体；false创建独立ObjectId的第二Form，仍要求ABCMABC→XBC、共享页全文保持、失败后原端点重试、严格重开及补偿后缀坐标。每种成功操作使用独立预算打印tracked peak，待最新定向验证，不能把同对象重复调用结果推广为不同对象矩阵已通过。session66920完整workspace经句柄和cargo/rustc进程核对仍在编译，不重启；新增变体晚于启动，后续单独验收。

2026-10-08 跨调用CID与峰值样本扩展：新增`cross_invocation_mixed_cid_span_checks_boundaries_and_signed_advance`，合成Type0混合单/双字节CMap、横/竖排W/W2、普通/补偿及B/空/B空格替换跨四调用，要求断码拒绝后stamp/保存字节保持、原端点重试、共享页/旧叶/严格重开及补偿后缀坐标。内部根/两调用样本用独立OperationBudget记录成功编辑的tracked peak并断言返回live=0，避免先前失败/选择探针污染峰值；不等同进程RSS或四调用最低额度。均晚于当前session66920 workspace启动，尚未验收，待其终态后运行最新定向样本；当前句柄仍存活。真实字体/独立视觉及内存优化仍待完成，fuzz暂缓。

2026-10-08 公共回滚矩阵终态：session56327退出0，nested_adjusted_text全部13项通过，日志`/tmp/rzpdf-public-cross-invocation-rollback-final.log`。覆盖四调用普通/补偿、非法结束偏移/不可编码/8 MiB预算拒绝后原端点重试、外层错误及panic恢复完整保存字节/旧Form后刷新重试、共享页/后缀坐标/严格重开。当前源码完整workspace门禁已启动，日志`/tmp/rzpdf-public-cross-invocation-workspace.log`，尚未终态；不把13项集成结果当作完整workspace或全部字体/视觉验收。fuzz暂缓。

2026-10-08 公共失败矩阵首轮诊断：session11373退出101，11项通过、跨调用额度样本1项失败；大替换拒绝后的512 KiB小替换重试仍报operation live bytes，不是文档回滚断言失败。四调用同时持有字体/节点计划需更高峰值，样本预算调整为内部已验证的8 MiB并将拒绝输入同步增至8 MiB，不改变生产预算限制，不宣称512 KiB可编辑四调用。新增外层事务在跨调用成功发布后主动错误/panic恢复保存字节/原Form、刷新快照重试样本。最新完整嵌套集成日志`/tmp/rzpdf-public-cross-invocation-rollback-final.log`运行中，尚未验收。

2026-10-08 公共跨调用首轮集成终态：session19291退出0，nested_adjusted_text全部11项通过，包含两层共享祖先四调用普通/补偿跨选区、共享页、后缀坐标及严格重开；日志`/tmp/rzpdf-public-cross-invocation-span-integration.log`。新增失败矩阵另由session11373运行，日志`/tmp/rzpdf-public-cross-invocation-failure-matrix.log`，尚未验收；首轮结果不证明后增断言。完整workspace及更广字体/晚发布失败仍待补，fuzz暂缓。

2026-10-08 公共跨调用失败矩阵扩展：共享祖先四调用样本增加非法结束偏移、不可编码Unicode失败后的stamp及完整保存字节不变，随后原端点成功编辑；新增普通/补偿跨调用512 KiB操作预算拒绝样本，要求原叶/祖先对象、Resources、stamp及保存字节不变并用原端点小替换重试和严格重开。均晚于session19291编译，需后续最新门禁；当前集成进程经句柄/进程核对仍存活，不重复启动，不将新增断言算作已验收。

2026-10-08 公共跨调用入口接入：session10090退出0，内部失败重试定向1项通过（反向、不可编码、零预算保存字节不变及原端点重试）。不同调用链的公共span转入预算化选区/调用树/逐节点计划及单事务发布，同调用分支保持原实现；旧拒绝不同调用断言改为拒绝反向跨调用。集成门禁session19291运行，日志`/tmp/rzpdf-public-cross-invocation-span-integration.log`，尚未验收。完整workspace、更多字体及晚失败矩阵仍待完成，fuzz暂缓。

2026-10-08 公共跨调用集成样本草稿：`nested_adjusted_text.rs`新增两层共享祖先四次调用的普通/补偿样本，预期跨调用编辑后AXHCD、共享页全文不变、严格重开一致及补偿后保留字符坐标不变。公共入口尚未接入，故此新测试暂不具备通过条件；待session10090内部失败重试门禁终态后接入接口，并将旧“不同调用必拒绝”断言改为反向跨调用拒绝，再运行完整集成门禁，不将草稿计为验收。

2026-10-08 普通/补偿混合样本终态：session69955退出0，定向1项通过，日志`/tmp/rzpdf-selected-span-mixed-matrix-latest.log`，覆盖XBC后缀坐标、共享页及严格重开。后增反向范围、不可编码Unicode、零预算失败及原端点重试断言由新一轮`/tmp/rzpdf-selected-span-failure-retry-latest.log`验证，尚未验收；公共接口继续保持原限制，待失败门禁后接入。

2026-10-08 内部发布门禁终态与失败重试：session96798退出0，`/tmp/rzpdf-selected-span-plan-publication-full-units.log`确认PDF单元654项通过；仅证明该次编译版本。最新普通/补偿混合样本由session69955运行，日志`/tmp/rzpdf-selected-span-mixed-matrix-latest.log`，尚未终态。其编译后又补不可编码Unicode及零预算失败断言：完整保存字节不变、预算释放、复用原端点成功重试；需后续最新门禁，不借用69955结果。公共跨调用入口仍未开放，fuzz暂缓。

2026-10-08 内部发布普通/补偿混合矩阵：根/共享Form样本扩展为两模式，原文字ABCMABC，选区覆盖前一次Form和根文字及末次Form的A，保留末次BC；要求选中页XBC、共享页ABCMABC、严格重开一致，并在补偿模式检查BC原坐标。事件组相应[1,1,1,0,2]，空TJ仍保留。新样本晚于session96798编译，需后续最新门禁；公共跨调用入口仍未开放，较晚失败及更多字体/图像/非文本邻接矩阵待补，fuzz暂缓。

逐事件计划合并门禁终态：原session19173退出0，普通/补偿两事件合并及零预算不消费、计数/片段顺序/释放断言通过。最新多节点计划与内部发布完整PDF单元已启动，日志 /tmp/rzpdf-selected-span-plan-publication-full-units.log，尚待结果；旧合并门禁不覆盖新端到端流程。

2026-10-08 多节点选区计划及内部发布接入：新增 build_selected_span_plans，认证选区/准备表，按节点分组逐事件生成原作用域度量计划，只首事件插入新文字，无字符串TJ不伪造计划，节点内合并并检查跨节点字符总量。新增 replace_selected_text_span 内部串联收集/树/映射/准备/分组/计划/单事务发布。混合根样本增加共享页、结果X/共享页ABMAB、持有预算恢复及严格重开验证。新代码晚于session19173编译，尚未门禁；需普通/补偿完整矩阵、较晚规划拒绝/提交错误回滚及公共入口验收，fuzz暂缓，改动未提交。

分组完整PDF单元终态：原session34439退出0，654项通过，包括根/嵌套事件分组。最新单节点计划合并定向已启动，日志 /tmp/rzpdf-span-node-event-plan-merge-validation.log，尚待结果；旧654项不覆盖合并器新增代码。

2026-10-08 单节点逐事件计划合并：新增 merge_text_event_plans，检查原事件严格顺序/数量及聚合字符配额，所有表分配和最终原程序重建成功后才消费donor计划，转移原字体/编码/片段所有者并保留真实容量计费；失败不消费donor。共享祖先普通/补偿样本增加两事件[1,4]合并、计数(3,1)、零预算拒绝原计划保持及合并drop恢复额度。新代码晚于session34439编译，不能借用旧分组门禁证明；还需将分组生产事件接入多节点计划并单事务发布，公共编辑仍未开放，fuzz暂缓。

混合根选区门禁终态：原session1375退出0，真实页面根事件/两次共享Form准备及同位置新修订拒绝通过。最新分组完整PDF单元已启动，日志 /tmp/rzpdf-selected-span-grouping-full-units.log，尚待结果；旧混合根结果不覆盖分组新增代码。

2026-10-08 选区事件节点分组：新增 SpanEventGroups，预算化持有节点偏移及事件索引表，构建时另持有实际容量游标表，保留原展开顺序在各节点的投影；节点越界/映射长度异常及同节点原操作重复或倒序拒绝。混合根样本新增根[2]、首Form[0,1]、末Form[3]的分组检查及持有/drop额度恢复。代码晚于session1375编译，需后续门禁；尚未组合每事件文本计划与多叶发布，公开跨Form接口仍未开放，fuzz暂缓。

生产选区准备PDF单元终态：原session75287退出0，653项通过，包含新准备入口与旧准备器回归。最新混合根事件/旧修订样本定向已启动，日志 /tmp/rzpdf-selected-span-mixed-root-validation.log，尚待結果；旧653项不覆盖新样本。

2026-10-08 根/嵌套混合真实选区样本：新增 selected_span_prepares_root_text_between_shared_form_calls，页面包含同一Form两次调用及中间页面级(M)文字；选区事件节点预期[1,1,0,2]，空TJ保留，根文字范围与两个Form准备均需成功。随后以相同原操作/字符串仅增加尾注释生成新修订，旧选区配新根应拒绝且预算不变，drop全部所有者后额度归零。该测试晚于session75287编译，待原进程终态后最新门禁；仍需分组及多叶计划组合，不开放公共跨调用编辑，fuzz暂缓。

来源认证门禁终态：原session8550退出0，正确选区认证及错误页面/其他页面快照拒绝与临时预算恢复通过。随后启动最新完整PDF单元，日志 /tmp/rzpdf-selected-span-preparation-full-units.log，包含新生产准备入口和旧准备器回归，尚待结果；不能借用旧定向结果宣称新入口通过。

2026-10-08 完整选区生产准备入口：抽出 prepare_span_nodes 复用唯一节点预算化准备；旧来源入口保留原路径/叶位置校验。新增 prepare_selected_span，先核对选区stamp/页面ID与当前根，再认证实际页面，直接准备映射树、核对全部原事件；根节点事件按页面程序验证，不再借助伪造NestedDrawSource。共享祖先认证样本改用新入口，移除测试每事件来源复制。新增代码晚于session8550编译，旧结果不证明新入口；还需真实根事件混合矩阵、事件分组及多叶编辑计划，公共编辑仍未开放，fuzz暂缓。

完整选区树PDF单元终态：原session87166退出0，653项通过、0失败，包括完整选区建树/根事件/中间调用/节点配额。随后启动最新选区来源认证定向，日志 /tmp/rzpdf-span-selection-source-validation.log，尚待结果；旧653项不覆盖后写来源字段/认证入口。

2026-10-08 选区来源认证：SpanSelection保留原DocumentStamp及页面ID，新增validate_span_selection核对映射/准备表树身份、选区与根身份、当前页面来源，逐事件检查原操作字节位置及完整字符串范围/裁剪端点；无字符串TJ与真实空字符串严格区分。共享样本以每事件测试来源准备完整选区树，新增正确验证及错误页面/其他页面快照拒绝、临时预算恢复断言；生产选择准备仍需去掉此测试用来源复制并支持根事件。代码晚于session87166编译，旧结果不证明新增认证。多叶计划/公开编辑未完成，fuzz暂缓，改动未提交。

2026-10-08 完整选区树构建：树映射原session10409退出0。新增 from_selection，从全部选区事件路径构建树及事件叶表，复用 from_paths 前缀去重/调用冲突/节点配额，旧来源入口保留既有验证；无需额外复制借用路径表，页面级空路径映射根。共享祖先测试改用完整选区建树，新增根事件/中间调用/精确6节点边界与5节点拒绝释放测试。当前完整PDF单元门禁已启动，日志 /tmp/rzpdf-span-complete-selection-tree-units.log，尚待结果；旧映射结果不覆盖新建树。原程序认证、事件分组、多叶计划组合及公共编辑仍待完成，fuzz暂缓，改动未提交。

选区所有权门禁终态：原session23767退出0，描述表/路径/裁剪结果一致性及持有/drop预算释放通过。最新树映射定向已启动，日志 /tmp/rzpdf-span-selection-tree-mapping-validation.log，尚待结果；旧门禁不覆盖树映射新增代码。

2026-10-08 选区树映射：新增 SpanMappedSelection，借用原选区/树，预算化持有事件对应节点表；逐完整FormInvocation（含原索引/字节位置/ID）沿父子匹配，空路径映射页面根，缺少任一选中调用拒绝，不用相同Form对象ID冒充同调用。共享样本追加8事件两叶映射、所有者身份、映射额度持有/drop恢复及缺节点失败释放断言。此新增晚于session23767编译，需后续独立验收；仍需从完整选区构建覆盖所有中间调用的树、原程序认证、事件分组与多叶计划组合，不开放公共编辑，fuzz暂缓。

裁剪门禁终态：原session61460退出0，跨两叶部分范围及同事件双端裁剪通过。随后启动最新选区所有权定向，日志 /tmp/rzpdf-span-selection-ownership-validation.log，尚待结果；旧裁剪门禁不覆盖新描述表。

2026-10-08 预算化选区描述持有：新增 SpanSelection/SpanSelectedEvent 与 collect_expanded_text_span，计数后保留每个原事件的调用路径、操作索引/字节位置、裁剪代码范围及首事件标记；事件表及每条路径的真实容量同池持有，不复制全部内容程序。空路径预留给嵌套端点之间的页面级事件。当前实现通过重复受限遍历收集，不宣称线性/单遍或完成树映射。新增描述一致性、持有时额度非零、drop后归零断言；这是session61460编译后新增代码，需另行门禁，不能借用旧裁剪/652项结果。多叶计划组合、顶层/嵌套混合树映射及公开编辑仍未完成，fuzz暂缓。

字符码范围完整PDF单元终态：原session3198退出0，652项通过、0失败，包含两项独立TJ槽位/引号/非法参数范围样本。随后启动最新裁剪范围定向测试，日志 /tmp/rzpdf-span-clipped-ranges-validation.log，结果待确认；652项旧门禁不覆盖后写裁剪入口。

2026-10-08 原端点裁剪接入：新增内部 visit_expanded_text_code_ranges，将已经认证/排序的展开选区逐事件映射成原字符码字节范围，首尾按原端点裁剪，None只表示没有字符串的空/纯数字TJ，Some空范围仍是实际字符串的合法边界，并标记首事件供后续插入文本。共享祖先样本新增跨两叶首偏移1/末偏移0、4个无字符串事件保留、同事件双端裁剪及预算释放断言。此新增在session3198编译后写入，需原进程终态后验证最新源码，不借用旧门禁证明；字体码边界及多叶计划组合仍待完成，公开接口未开放，fuzz暂缓。

字符码范围接入门禁终态：原session93087退出0，共享祖先选区样本内的字符串范围/空及纯数字TJ无范围断言通过。已启动当前源码完整PDF单元命令 cargo test -p pdf --lib --locked，日志 /tmp/rzpdf-span-code-range-full-units.log，包含随后新增两项独立范围样本，结果待确认。

2026-10-08 字符码范围独立样本：追加 full_event_ranges_keep_tj_numeric_slots_and_quote_operands 与 full_event_ranges_reject_invalid_shows_without_fabricating_endpoints，验证带数字槽位TJ首/末字符串位置、空字符串Tj、空/纯数字TJ、两种引号及错误元素/参数拒绝。这些新增测试尚未执行；原session93087已完成编译且测试进程仍存活，不重启，不以此前992项工作区门禁证明新增范围辅助。多叶计划组合及公开接口仍待完成，fuzz暂缓。

定位门禁终态：原session25610退出0，跨两次叶调用的8事件定位、首尾标记、反向范围/错误字节位置的零回调与预算释放通过（共享祖先普通/补偿样本）。此结果不覆盖后写的 full_event_range 辅助；最新定向已启动，日志 /tmp/rzpdf-span-code-range-validation.log，尚待结果。

2026-10-08 选区到字符码范围辅助：新增 full_event_range，独立验证 Tj/TJ/引号参数，返回首末字符串槽位及原字节边界；空/纯数字 TJ 返回无可编辑字符串，保留其选区顺序，不伪造字符串端点。选区样本新增4个字符串事件范围及4个无字符串事件断言。该改动在session25610编译后写入，旧进程结果不能证明新增辅助覆盖；待原定位门禁终态后运行最新定向验收。字体边界、部分端点裁剪、多叶计划组合及公开API仍待实现，fuzz暂缓。

2026-10-08 跨调用选区定位：新增内部 visit_expanded_text_span，两次预算化遍历，先认证根并找到双端调用路径/原操作/字节位置，检查字符串槽位及字节偏移和正向顺序，再按展开顺序回调完整闭区间事件并标记首尾。无需为全部事件复制路径；错误端点/反向范围在选区回调前拒绝，仍由字体编辑规划验证完整字符码边界。共享祖先样本新增跨两叶8事件（含空/纯数字TJ）、原路径及首尾标记、反向/错误字节位置零回调和预算释放断言。当前定向session25610运行，日志 /tmp/rzpdf-expanded-span-selection-validation.log，尚未验收；旧992项workspace结果不证明此新增代码。尚未组合多叶计划并开放公共跨调用编辑，fuzz暂缓，改动未提交。

2026-10-08 完整回归终态：cargo test --workspace --locked 原session56430退出0，取代下方所有该进程“运行中”描述。主代码迭代遍历、原调用者状态和共享祖先发布进入完整回归；最新边界追加由定向日志独立证明，公开跨不同Form选区仍未开放，整体TODO未完成。改动未提交，fuzz暂缓。

2026-10-08 完整回归阶段结果：原session56430日志显示PDF单元649项通过、0失败，并进入 adjusted_text 集成测试。该649项编译范围含初版512层小栈样本，不含后加事件配额/深度拒绝的新断言；最新两项边界由原session57302定向退出0独立证明。整个workspace尚未终态，不勾选完整验收。

2026-10-08 最新边界验收终态：原 session57302 退出0，/tmp/rzpdf-expanded-boundaries-validation.log 显示5项通过（其中2项为本次遍历边界，3项既有字形展开测试）。明确覆盖512层Form在256 KiB线程栈成功、深度上限511拒绝、16个空TJ通过/第17个拒绝，以及上述成功/失败后的预算释放和页面保持。取代下方该定向测试“待锁/未验收”状态；不代替完整workspace验收。完整回归原session56430已完成编译并进入测试，仍未终态；整体目标未完成，fuzz暂缓，改动未提交。

2026-10-08 深度边界：512层小栈样本扩展为 max_depth=512 成功、511在最深层事件前拒绝，两条路径均检查预算归零。启动当前源码定向命令 cargo test -p pdf --lib --locked expanded_，日志 /tmp/rzpdf-expanded-boundaries-validation.log；它会正常等待既有workspace编译锁，不终止或重启旧回归。新配额与深度边界尚未验收，workspace原session56430仍运行。

2026-10-08 文本事件配额边界：新增 max_objects=16 时16个空 TJ 通过、17个空 TJ 在第17个回调前拒绝的测试，检查已访问计数、失败后原页面内容及预算释放。该新增测试是在 session56430 编译启动后写入，不借用该进程结果证明覆盖；待完整工作区进程终态后运行当前源码的新增定向验收。当前 session56430 经句柄及进程检查仍存活，日志 /tmp/rzpdf-iterative-span-workspace-validation.log 处于编译阶段，不重启。fuzz 暂缓，完整公开跨调用选区尚未完成。

2026-10-08 迭代遍历验证进展：主工作树原 session44722 退出0，PDF单元648项通过，覆盖迭代遍历及共享祖先样本的16事件/回调失败/panic/零预算断言。随后新增独立512层Form、256 KiB线程栈回归，尚未运行验证；已启动包含此最新源码的 cargo test --workspace --locked，日志 /tmp/rzpdf-iterative-span-workspace-validation.log。此前旧工作区日志已到文档测试尾部，但不作为最新代码验收。仍需新增深层样本结果、更多配额边界与完整公开跨调用选区，改动未提交。

2026-10-08 原调用顺序事件遍历（当前状态）：分类样本已补齐 Form BBox，旧副本单元日志显示 648 项通过；这不覆盖下述最新修改。内部 visit_expanded_text_events 已由递归改成预算化显式帧栈，帧持有 PreparedForm 及其资源所有者；保留来源认证、文本操作数检查、循环/深度/节点/事件/操作/解码工作总量限制。当前样本要求四次两层调用访问16个事件，包含空 TJ 与纯数字 TJ，并检查原位置/路径；新增回调失败、panic 和零预算后的预算释放断言。最新主工作树 cargo test -p pdf --lib --locked 在 session44722 编译验证中，尚未确认通过，之后仍需完整 workspace 验收、深层调用及更多配额边界测试。尚未组合完整跨调用公开选区；fuzz 暂缓，改动未提交，整体目标未完成。

2026-10-08 展开前Do分类：新增Page::form_call_at_operation_with_budget，按原scope预算化解析XObject字典/绑定/流及Subtype，Form返回原操作索引/字节位置/条目ID，Image不进入文本递归，非Do不递归，缺失绑定/非流/不支持Subtype拒绝；完整Form结构仍由单调用准备器验证。新增分类位置保留、图像排除、缺失绑定错误及临时额度释放回基线单元；当前仍是展开辅助入口，不是完整文本事件遍历，不用字符提取冒充空/纯数字TJ覆盖。完整PDF单元新门禁 `/tmp/rzpdf-span-call-classification-full.log` 运行中，新增部分未验收。主旧工作区回归继续，范围展开仍待实施，fuzz暂缓，改动未提交。

原调用者状态门禁终态：原session31185退出0，`/tmp/rzpdf-span-inherited-metrics-full.log` 647项通过、0失败/忽略，取代下文运行中状态；页面到两层叶的字体/间距/负水平缩放继承、普通/补偿公共祖先发布、较晚拒绝回滚和新快照重试均通过。仍需完整选区事件展开和其他字体/状态/错误矩阵，整体任务未完成。

2026-10-08 多层原调用者文字状态：唯一节点准备原session56060退出0，`/tmp/rzpdf-span-unique-preparation-full.log` 647项通过、0失败。增加span_text_metrics_for_operation，校验树/节点及当前根，沿各原父程序Do前缀传递预算化文字状态，最终在叶原事件处取得度量，不裸克隆字体；路径祖先定位有重复扫描，不宣称线性CPU。两层公共祖先测试改为叶不设置Tf，从页面继承Courier/12Tf/2Tc/3Tw/-75Tz，两个叶普通/补偿计划均调用新入口，继续要求后缀/其他调用坐标保持、共享隔离及较晚拒绝回滚/重试。完整单元新门禁 `/tmp/rzpdf-span-inherited-metrics-full.log` 运行中，新增状态继承未验收，不扩大为全部字体/竖排/所有调用者状态矩阵。主旧完整工作区回归继续，fuzz暂缓，改动未提交。

2026-10-08 唯一节点准备接入：补齐资源的完整单元原session91111退出0，`/tmp/rzpdf-span-integrated-full-units-fixtures.log` 647项通过/0失败。继续增加document内部prepare_span_forms，认证页面根、核对来源槽/原路径/文本几何标志，逐唯一节点准备Form，检查祖先ID循环、累计根及全部节点解码字节/操作数，最后校验全部文本叶索引/字节位置；共享前缀不再通过重复完整链准备。两层共享祖先测试的初次准备/新快照重试改走此入口，增加错误页面根和错误叶字节位置拒绝、预算释放；旧链去重保留作对照。新增完整单元门禁 `/tmp/rzpdf-span-unique-preparation-full.log` 运行中，新增路径未验收，不能借用647旧版结果。范围展开/继承/完整多分支矩阵待实现，主旧工作区回归继续，fuzz暂缓，改动未提交。

2026-10-08 完整单元资源校正/单调用准备：原session63723退出101，647项中597通过/50失败，失败日志为隔离副本未复制仓库tests/path-test.pdf导致文件不存在，不能验收；补齐仓库tests资源后重跑完整PDF单元，日志 `/tmp/rzpdf-span-integrated-full-units-fixtures.log` 运行中。新增Page::prepare_form_call_with_budget只准备一个原调用，检查Do/操作数/字节位置、有效XObject条目ID、Form结构及预算化解码/字典/scope，保留页面fallback；调用方负责根来源认证、祖先循环和总工作限额，尚未接入逐唯一节点遍历。旧链接口不改；此新增代码同步纳入本轮编译，但不能以编译证明调用方缺口完成。范围展开仍缺空/纯数字TJ事件完整枚举，fuzz暂缓，改动未提交，整体目标未完成。

加强回滚门禁终态：原session72725退出0，`/tmp/rzpdf-span-rollback-resources.log` 11项通过、0失败，失败后两页资源/原Form完整对象保持及严格重开通过。随后将主模块注册及当前源码同步到独立副本，启动完整PDF单元 `cargo test -p pdf --lib --locked --offline`，日志 `/tmp/rzpdf-span-integrated-full-units.log`，运行中；它不再过滤既有单元，但仍不是整个工作区/集成测试验收。

2026-10-08 内部模块集成：主document/mod.rs接入span_tree，原型转为内部编译模块，公开跨调用span仍拒绝、不改变公开承诺；docs/content-editing.md说明单事务发布、总计费、树身份和预算持有，并明确范围展开/继承/临时重复分配/完整矩阵缺口。加强回滚资源门禁原session72725仍运行（`/tmp/rzpdf-span-rollback-resources.log`）；接入后的完整单元门禁待原进程结束启动，尚未验收。正在运行的旧完整工作区门禁只证明其原已编译范围，不能当作本次集成完整证明，后续必须覆盖当前源码。fuzz暂缓，改动未提交，整体目标未完成。

晚祖先拒绝门禁终态：原session92584退出0，`/tmp/rzpdf-span-late-ancestor-rejection.log` 11项通过、0失败，输出总量拒绝及普通/补偿两模式较晚祖先拒绝/预算清理/新快照重试通过。随后已同步回滚资源/完整对象/严格重开加强检查并启动 `/tmp/rzpdf-span-rollback-resources.log` 独立门禁，运行中，取代下文未同步状态；新增检查尚未验收。

2026-10-08 回滚证据加强：主草稿补较晚祖先拒绝后两页资源字典、原叶/祖先完整对象相等，以及失败状态严格保存重开两页仍为ABCABCABCABC；不只比较根内容字节。共享前缀准备去重同时比较原程序/字典/有效scope，拒绝同内容不同资源的准备结果。原session92584上一版拒绝门禁仍确认存活（`/tmp/rzpdf-span-late-ancestor-rejection.log`），未覆盖该隔离副本源码；新增检查尚未同步/验收，需原门禁终态后另跑。主完整回归继续，fuzz暂缓，改动未提交，整体目标未完成。

2026-10-08 多叶计费/晚祖先拒绝：原session75891退出0，`/tmp/rzpdf-span-aggregate-chars.log` 11项通过，输入总量4精确边界/限额3拒绝通过。继续补输出总量5超过4且各叶计划单独合规的拒绝/恢复原计划；在真实两层测试准备表的公共祖先注入预算化F标记，使两个子叶已插入后祖先PdfStream构造精确返回UnsupportedFeature，检查编辑页/共享页恢复、预算回到仍持有叶计划的基线，再用新根快照与保留计划重试。普通/补偿两模式运行，属确定性拒绝/回滚测试，不是fuzz。新隔离门禁 `/tmp/rzpdf-span-late-ancestor-rejection.log` 运行中，新增部分未验收；公共API仍未开放，改动未提交，整体目标未完成。

公共祖先门禁终态：原session61821退出0，`/tmp/rzpdf-span-shared-ancestor-final.log` 11项通过、0失败（636项过滤），普通/补偿两模式真实共享祖先发布、严格重开及后缀/未选调用坐标验证通过。新增多叶字符计数随后已同步并启动 `/tmp/rzpdf-span-aggregate-chars.log` 独立门禁，运行中，取代下文尚未同步状态；新增总量检查尚未验收。

2026-10-08 多叶总字符计费：TextEventEditPlan保留既有逐事件校验所得输入/输出字符计数，统一事务在任何写入前累计所有节点计划并分别校验 max_text_chars/溢出，避免多个单叶合规却整体超限。公共祖先测试增加两叶总输入4/输出1、精确限额4通过、限额3拒绝且内容和预算保持；只是发布前总量校验，尚不等于范围展开阶段提前计费。上一版公共祖先门禁原session61821仍确认运行（`/tmp/rzpdf-span-shared-ancestor-final.log`），尚未终态，因此新增计数接口/测试尚未同步到隔离副本，不借用旧结果；需先收取结果再运行新增门禁。fuzz暂缓，改动未提交，整体目标未完成。

2026-10-08 真实跨叶文本发布确认：原session16221退出0，`/tmp/rzpdf-span-transaction-text.log` 10项通过、0失败，两个真实叶普通计划一次发布、编辑页X/共享页ABAB及预算回基线验证通过。继续新增两层共享祖先测试，选择同外层的两个叶调用，普通/补偿两模式替换与清空，要求同一公共祖先合并、未选外层调用坐标不变、旧叶/外层流保持、共享页保持、严格重开；补偿模式检查两后缀C位置。首轮测试误用不存在的PdfStream::data方法，改raw_data后隔离日志 `/tmp/rzpdf-span-shared-ancestor-final.log` 运行中，新增测试尚未验收。仍缺完整来源范围展开、全目标字符计费/作用域继承、多分支晚错误/panic矩阵，公共API未开放，fuzz暂缓，改动未提交。

2026-10-08 实际发布回滚校正：原session39852退出101（9项通过/1项失败），回滚内容恢复但修订号使旧根快照失效；改为取得新根快照后重试，不绕过来源校验。原session49882退出0，`/tmp/rzpdf-span-transaction-publish-final.log` 10项通过，真实双调用发布/共享页保持/外层错误回滚及新快照重试/严格保存重开通过，取代上一轮“原快照重试”的错误假设。继续为两个真实叶生成普通文本计划，第一AB改X、第二AB清空，要求严格重开编辑页X、共享页仍ABAB、全部计划释放回预算基线；新日志 `/tmp/rzpdf-span-transaction-text.log` 运行中，尚未验收。该样本页/叶同字体作用域，不证明跨作用域继承或补偿、多层公共祖先矩阵；公共span仍未开放，fuzz暂缓，改动未提交。

2026-10-08 多分支实际发布原型：未接入模块增加内部 `commit_prepared_span`，验证计划表数量，在一次预算化事务/页面根快照提交内从叶到根构建，插入新Form并按新子ID准备局部绑定，页面根注册各新顶层Form后一次合并；全部流保留所有者/绑定租约随结果表持有，编辑字节按全部节点累计限额。仅把现有预算化局部绑定与Do构造辅助函数改为pub(super)供同层调用，主完整回归仍针对85ff816已编译源范围。隔离原session41652退出0，10项既有测试通过，证明新内部接口可编译，不证明发布行为已验收。随后新增真实双调用发布、共享页保持、外层错误回滚/原快照重试及严格保存重开测试，日志 `/tmp/rzpdf-span-transaction-publish.log` 运行中。公共跨调用span仍未开放，范围展开/全部字符计费/多层公共祖先及晚失败矩阵待验证，fuzz暂缓，改动未提交，整体目标未完成。

流所有者分离门禁终态：原session74287退出0，`/tmp/rzpdf-span-form-retention.log` 10项通过、0失败（636项过滤），取代下文该门禁待确认状态。下一步接对象插入/子Form资源重绑和根合并的同一事务，不能把保留所有者接口视为发布功能完成。

2026-10-08 流插入前所有者分离：长度检查原session6764退出0，`/tmp/rzpdf-span-form-length.log` 10项通过、0失败。物化结果拆分为PdfStream与 `SpanFormRetention`，`into_parts` 不克隆、不释放预算，保留所有者继续覆盖已分离流及原程序/字典/资源，必须持有到发布结束；新测试验证分离时额度不变、丢弃流不提前解除发布保留额度、释放所有者回到基线。隔离原session74287、日志 `/tmp/rzpdf-span-form-retention.log` 运行中，未验收；仍未将流插入文档、未实现真实多分支事务。主完整回归继续，nested_adjusted_text 10项通过，fuzz暂缓，改动未提交。

Form物化上一版终态：原session62988退出0，10项通过、0失败（636项过滤），`/tmp/rzpdf-span-form-output.log`。随后已同步新增长度检查/测试并启动独立门禁 `/tmp/rzpdf-span-form-length.log`，运行中，取代下文“尚未同步”状态；新增长度约束尚未验收。

2026-10-08 Form物化长度前置检查：主草稿在Resources/内容缓冲分配前同时检查 max_raw_stream_bytes 和 max_decoded_bytes；真实Form测试新增两种上限少一字节拒绝和所有预算释放回树基线。原session62988物化上一版隔离测试进程仍确认存活（`/tmp/rzpdf-span-form-output.log`），尚未终态，因此新增长度检查/测试尚未同步到临时副本，也未验收；需先收取原结果再运行新门禁，不覆盖活跃门禁源码。完整工作区原进程仍运行，整体目标未完成，fuzz暂缓，改动未提交。

2026-10-08 Form流物化准备：树身份门禁原session46078退出0，`/tmp/rzpdf-span-prepared-identity.log` 10项通过、0失败。继续增加 `SpanFormOutput`，用合并程序和有效Resources构造预算化PdfStream，持有真实中间字典/资源、原Form及程序所有者，字段析构顺序确保资源先于其租约释放；不插入对象、不发布页面。真实准备表测试增加叶节点流物化、Resources存在、根看到两个准备结果、全部释放回树预算基线。隔离门禁日志 `/tmp/rzpdf-span-form-output.log` 运行中，新增部分未验收。实际子Form新ID资源重绑、根合并与单事务提交仍待实现，主完整回归继续，fuzz暂缓，改动未提交。

2026-10-08 准备表消费门禁：原session12814退出0，`/tmp/rzpdf-span-real-forms-build-final.log` 10项通过、0失败（636项过滤），取代下文待确认状态；真实双调用/重复来源去重、晚链失败释放、子节点先于根、结果持有预算验证通过。继续将 `SpanPreparedForms` 生命周期绑定原树，并在消费前检查引用身份，防止同节点数的其他树套用该表；新增零回调/准备表释放测试，原session46078、日志 `/tmp/rzpdf-span-prepared-identity.log` 已编译、进程仍在运行，尚未验收。主完整回归仍运行，jpeg_embedding 4项通过；原型实际文档写入/事务发布仍待实现，fuzz暂缓，改动未提交。

2026-10-08 准备表消费推进：原session33263退出101，9项通过/1项失败，真实测试Form缺BBox，被页面校验正常拒绝；样本补齐BBox。树增加 `build_prepared_bottom_up`，消费节点Form所有者并提供已完成子结果，根必须无Form、非根必须有Form；结果表保留实际Form/租约到调用方释放。新测试确认两个调用节点先于根准备、保留所有者期间计费、释放回树基线。首轮构建闭包泛型推断失败，指定结果类型后重跑原session12814，日志 `/tmp/rzpdf-span-real-forms-build-final.log`，运行中未验收。仍不注册资源、不插入对象、不发布页面；主完整回归继续，image_transform 4项已通过。fuzz暂缓，改动未提交，整体目标未完成。

2026-10-08 真实调用链表准备：原型增加 `SpanPreparedForms` 与 `prepare_forms`，调用页面预算化准备器校验每个来源，按树节点移动真实字典/有效作用域/内容程序所有者；重复来源共享节点、不同调用不合并，拒绝深度或共享程序不一致，较晚失败释放整表。当前仍先准备完整链再丢弃重复前缀，存在临时重复分配，未满足完整的唯一前缀准备优化；页面根不放入Form表，也不写入文档。新增真实同Form双调用及重复字形来源、晚错误清理/重试测试，隔离原 session33263 日志 `/tmp/rzpdf-span-real-forms.log` 已编译、运行中，未验收。主完整回归原进程仍运行；实际树作用域重绑和单事务发布尚未接入，改动未提交，fuzz暂缓。

真实叶计划重试门禁终态：原 session36266 退出0，`/tmp/rzpdf-span-real-plan-retry.log` 9项通过、0失败（636项过滤），取代下文运行中状态；第二绑定失败后计划预算保留、同计划重试成功均通过。完整工作区回归已通过 image_remove 2项，仍未终态。

2026-10-08 真实叶计划组合准备：`TextEventEditPlan::original_replacements` 只读借用已有原始片段，字体/字符代码/片段预算仍由计划持有；树原型的 `rebuild_parent_with_text_plan` 使用预算化描述表接入父程序批量合并。真实 Courier 文档测试覆盖普通双引号和补偿展开后的兄弟 Do 索引、未发布内容保持及计划/合并结果分别释放。首轮测试误将双引号补偿的 Tw/Tc/T*/TJ 四操作计为三，修正后原 session64665 退出0，9项通过、0失败，日志 `/tmp/rzpdf-span-real-plan-final.log`。继续补第二绑定错误的精确载荷、计划预算仍持有及同计划重试，新增门禁原 session36266，日志 `/tmp/rzpdf-span-real-plan-retry.log`，运行中，不借用上一版结果。主工作区完整回归仍针对提交85ff816已编译源码，本次只读计划接口变更另行验证；原型仍未接入实际资源/发布链。改动未提交，fuzz 暂缓，整体目标未完成。

2026-10-08 多分支局部编辑组合：原型增加 `rebuild_parent_with_replacements`，把父程序自己的替换片段与全部子 Do 绑定按同一原始索引排序、一次重建；重叠/逆序/越界的局部位置在绑定前拒绝，检查替换数量额度，并按前置片段操作数增减定位最终 Do。新测试覆盖文本展开成两操作及删除成零操作、兄弟绑定不丢失、未知邻接保持、与子调用冲突时零回调和预算释放；隔离门禁日志 `/tmp/rzpdf-span-local-merge.log`，8 项通过、0 失败（636 项过滤）。这解决父节点同时有局部编辑和子调用时的索引组合基础，不证明真实 scope、叶计划或文档发布已接入；主生产代码冻结，完整工作区回归继续，已通过 image_extraction 9 项。本次改动未提交，fuzz 暂缓，整体目标未完成。

2026-10-08 提交后推进：未接入的 `span_tree` 增加全部原调用前置校验测试（较晚字节位置错误时绑定回调次数必须为零）及第二绑定 panic 的精确载荷、预算释放、原内容不变测试。首轮测试错误地对 `Option<u64>` 加整数，修正后隔离测试命令退出 0，7 项通过；日志 `/tmp/rzpdf-span-parent-preflight-final.log`。主生产模块未接入该原型；完整工作区回归原进程仍运行，已通过 image_display_translation 3 项，整体结果未确认。下一步仍需组合真实叶编辑计划、作用域及公共祖先资源，并通过单事务发布；跨调用编辑未完成，fuzz 暂缓，本次改动尚未提交。

工作区包含 `pdf`、`render`、`font-data` 三个 crate。现有代码支持文件/字节/Reader 读取、基础对象与 xref 表/流解析、对象流加载、常见内容解释、DisplayList、Skia 渲染、对象增删改、页面增删、内容追加/替换和全量保存。

新建文档、PageBuilder、资源注册、标准字体、RGB/RGBA/PNG/JPEG 嵌入、Info/Metadata 和全量保存已有部分实现。中文嵌入/CID/ToUnicode已有两页示例和独立验证记录（见docs/cjk-creation-validation.md），但完整P1样本矩阵尚未验收。内容编辑已有来源快照和Tj/TJ单项事务替换，不代表完整布局编辑；加密、签名、表单和批注仍未形成完整能力。JPX/JBIG2 当前返回不支持错误；Type3、Pattern、部分颜色/函数和安全处理器存在未实现路径。

初次源码审查找到 60 个 `#[test]` 标记，仓库内仅看到一个 PDF 样本。依赖已获取，本地工作区测试基线已建立；最新结果见实施记录。真实样本集与远端 CI 验证仍需推进。

架构决策与迁移顺序见 [架构方案](docs/architecture.md)。完成实现和验收后再勾选任务。

### 当前能力摘要

| 能力 | 当前状态 | 下一验收缺口 |
|---|---|---|
| 解析与读取 | 对象、xref 表/流、对象流及基础页面解释已有实现 | 真实样本、模糊测试、未支持路径及聚合预算 |
| 从零构建 | new、页面配置、基础资源及中文CID/ToUnicode两页示例已有实现/验证记录 | 完整字体覆盖、样本矩阵及P1整体验收 |
| 写出与保存 | 全量保存、原始/解码流状态、有效版本及定向最低版本检查已有实现 | 平台支持矩阵、峰值内存、完整功能版本审计 |
| 页面编辑 | 增删/重排、导入、合并拆分、事务与共享隔离已有局部回归 | 全部继承/循环共享/失败边界及真实样本矩阵验收 |
| 内容编辑 | ContentProgram、顶层绘制来源与集合身份、文字单项/局部替换插删、图片绑定/单次Do替换与删除、图片几何命中及用户/显示空间变换、路径参数编辑与绘制取消已有实现 | 布局补偿、真实字体矩阵、Form/内联来源、文字路径命中及显示编辑、裁剪/透明可见性、完整视觉与预算验收 |
| 高级能力 | 部分绘制能力已有实现，不代表完整兼容 | 批注、表单、加密、增量保存、签名分别立项验收 |

以上为源码及既有验证记录的范围摘要，不以测试数量推算完成百分比。最新实际测试范围见实施记录，未勾选任务仍需各自验收。

### 架构实施工作包

以下任务细化 A1–A4，不重复代表阶段已完成；与对应 P 项一起验收。

- [x] AR-01（A1 / P1-01）抽取 DocumentState，统一持有 ObjectStore、Trailer、版本及文档身份；确定隐藏 Reader 泛型的兼容方案。验收：无输入源创建与文件打开共用状态服务，旧入口有迁移说明。已采用非泛型 Document facade＋兼容 PdfDocument<R>；底层类型进一步消除泛型不计作已完成。
- [x] AR-02（A2 / P0-11）引入事务提交修订号及派生缓存失效契约。验收：编辑后重复读取/解释结果更新，嵌套失败与 unwind 不留下错误缓存；低层修改也纳入规则。现有源缓存隔离与派生快照生命周期已审计；不表示新增统一缓存层或所有解码器兼容性完成。
- [ ] AR-03（P0-10）增加操作级 Budget，覆盖临时解码、转换、对象克隆、缓存及保存缓冲。验收：多个单项未超限但合计超限时拒绝；失败释放额度，原生后端未计费范围有明确说明。
  - [x] 独立活跃字节额度、共享状态和不可复制的所有权租约；失败预留不改计数。OperationBudget/MemoryLease原语及Limits独立配置已实现，部分文字范围结果/元数据已接入；字体、源码重建、事务、缓存及保存尚未贯通，不代表完整操作预算完成。
  - [ ] 文字编辑入口贯通字体/编码、ContentProgram及Form复制，返回缓冲携带额度，不在函数返回时提前释放。
    - [ ] 字体资源副本逐对象生命周期：当前font_binding只按前缀操作聚合租约，下一步以内部非Clone拥有对象联合PdfObject与可选MemoryLease，字段顺序保证先销毁对象后归还额度。font_scope_resource返回资源对象所有者，类别字典仅在查询/复制时存活；ext_state_font仅转移字体所有者，大小副本与Font数组用后释放。混合模式解析核心需支持借用拥有结果，预算/普通路径共用类型与错误验证，BM数组仍校验全部元素；不可为提前释放额度而留下无租约对象。验收：长BM数组元素峰值不累计、Tf/gs真实资源共存少1拒绝及释放邻居重试、返回派生字体租约保持、类型错误/异池/unwind归零；之后复跑字体专项、公共编辑与统一工作区。此项不包含首次源缓存或原生内存总量约束。
    - [ ] 字体结果所有权：增加不可裸Clone的预算字体所有者，字体派生数据及其租约一起返回；顶层编码/范围编辑借用该所有者，持有至提交返回，q/Q共享同一所有者而非另发无租约副本。
    - [ ] SimpleFont构造：BaseFont、Encoding、AFM及ToUnicode派生数据分配前预留；Descriptor的FontFile解码使用同池，预算字节所有权移交字体，不先普通decode再补记。
    - [ ] Type0构造：Encoding/ToUnicode CMap、DescendantFonts、CID宽度/映射与字体程序副本贯通同池；提前确定HashMap表的可计费存储策略，不能仅按元素数猜测桶容量。
    - [ ] 字体验收：两个字体/资源各自未超限但共存超限拒绝，构造/编码/提交失败和unwind释放；字体结果存活时额度不归零，释放后恢复；FreeType等原生内存单独说明未计费范围。
      - [x] 字体构造双结果共存：Courier/Helvetica分别在共享门槛内成功，保留Courier再构造Helvetica时合计峰值超限拒绝；仅保留首字体额度，释放后第二字体重试成功。此项不代替资源/顶层编辑共存与全链路unwind验收。
      - [x] 页面资源字体结果共存与unwind：两页Courier/Helvetica经资源解析在同池共存，合计超限不影响首结果，释放后重试恢复；两个结果同时unwind归零，页面内容不变。此项不代替顶层编辑提交失败及全链路验收。
      - [x] 字体/编码所有者跨内容提交与外层回滚：Tj/TJ同池保留编码字体所有者直到提交返回，外层错误/unwind恢复源字节及流对象，邻居额度不变，旧快照失效后新快照同池重试成功。覆盖共享内部提交路径，不代表所有公开入口及全部失败点完成。
    - [ ] 嵌套Form文字编辑同池贯通：准备链/循环检测槽位、XObject/Form拥有副本及预检、原始Form解码/程序解析、页面资源与继承字体、prepared副本、局部绑定及页根Form注册增长、编码、叶/父/根程序重建、复制流载荷/字典与事务快照载荷已接入同池。预算页定位保留单目标但校验整树，已用于页根Form注册和内容提交来源页/工作页；内部get_page_with_budget已用于四类顶层Unicode替换、嵌套Unicode准备及预算当前程序读取。Normal/Compressed源加载、ObjStm预算解码/表/载荷、作用域加载栈数组共存已接入，成功缓存后转为持久内存；未知对象token直接预算读取后报错，不普通重读。预算store快照改为计费数组并复用既有桶回滚；编辑中新对象引发工作store桶增长、诊断/共享控制结构及其他容器/原生分配仍有未计费边界，普通公共get_page/非预算入口不在此接入范围，不能视为完整贯通。
      - [x] 嵌套文字叶节点重建：整字符串/单TJ项/字符串范围/TJ跨项范围重建同池返回BudgetedContentProgram，commit_nested_edit借用所有者保留租约至事务结束；精确峰值少1拒绝且文档/资源不变，释放后短文本重试恢复。此项不代表父Form复制与完整提交预算。
      - [x] 嵌套继承字体所有者：页面资源同池解析，BudgetedFont逐流移动并合并新前缀构造额度，q/Q恢复继承字体不深Clone；错误/异池拒绝/unwind释放，结果持有期间额度保留。原始Form链及prepared资源副本仍未计费。
      - [x] 原始Form解码/程序解析：预算解码字节直接移交ContentProgram预算解析，PreparedProgram保留源码/边界表/操作表/递归操作数租约，Unicode准备链实际接入；异池拒绝、精确峰值门槛、解析失败及unwind清理已验证。此项不覆盖链槽位、Form字典/资源复制及首次源缓存。
      - [x] 准备链/循环检测槽位：分配前按PreparedForm/ObjectId槽位预留，按实际capacity校准；BudgetedFormChain与双端迭代器保持槽位租约直到Vec实际释放，部分/耗尽反向迭代与unwind不提前归还额度。Form字典/资源堆数据仍未计费。
      - [x] prepared字典/有效资源副本：使用分配前预算克隆，PreparedForm持有副本租约，取出Form并释放链迭代器后额度不归零，提交时汇入外层租约保留至事务返回。页面资源准备同池，并去除重复工作资源克隆；Form预检临时分配与复制时资源增长仍未计费。
      - [x] Form预检与拥有副本：普通/预算共享字段验证核心，预算对象/引用数组/数值/Resources/Group分配前预留，BBox/Matrix数值采用栈数组；返回仅保留资源/Group租约。准备链XObject/Form对象解析已同池，首次源缓存仍未计费；间接字段、null语义、错误类型、峰值拒绝及unwind清理已验收。
      - [x] 局部XObject绑定/Form.Resources重建：预算字典替换先克隆保留值与新值、再原子交换，覆盖键/表扩容旧新共存；FmEdit名称由栈缓冲生成后预算复制，返回租约保留至事务返回。验证碰撞命名、峰值拒绝原字典不变、释放恢复/unwind及深度/键长/条目限制；页根注册、父/根程序和流输出仍待接入。
  - [ ] 解码/图片/事务克隆按容量与共存峰值计费，明确持久缓存转移及保留额度。
    - [x] 工作对象表增长：DocumentStore采用hashbrown与TableAllocator，在实际Layout分配前预留，覆盖控制区/对齐及旧新表共存，不以capacity乘条目尺寸估算。try_insert_object与reserve_entry使插入、更新缺失ID、删除缺失ID共用预算表分配；事务作用域同池嵌套，异池拒绝，普通内层事务继承外层预算和可失败数组快照。回滚快照恢复/销毁后退出表作用域，仅最外层将保留表存储转为持久内存；原有持久分配不追溯计费。state.rs的table_growth_budget_rejects_insert_update_delete_and_restores_retry、table_budget_inherits_plain_savepoints_rejects_foreign_pool_and_unwinds及table_allocator.rs专项覆盖扩容峰值少1拒绝、内容/对象号保持、足额重试、嵌套回滚/unwind与邻居额度。entries改为只读StoreEntries视图，隐藏具体表类型，但原显式HashMap类型使用者需适配；store_entries公开查询/遍历专项已通过。全工作区67632退出0、1065项通过，源码指纹见本文顶部。控制结构/预算Rc状态与对象载荷另计，不代表完整AR-03。
      - 当前实现：提交a270aca已引入hashbrown 0.16.1与allocator-api2 0.2.21，TableAllocator在分配前按真实Layout预留，两个内联槽持有扩容旧新租约；同池嵌套、普通内层继承、异池拒绝、回滚及最外层转持久已接入。插入/更新缺失ID/删除缺失ID峰值拒绝与重试专项、工作区938项回归均通过。entries已迁移为StoreEntries只读视图，仓库内下游编译通过，外部下游兼容性未承诺；补充公开接口查询/借用/遍历专项中。上条为原验收要求，不再代表尚未实现；完整AR-03仍未完成。
  - [x] 固定Compressed/ObjStm保存组合：合法xref流样本中四压缩对象冷加载，原始/Flate对象流与Preserve/FlateUnencoded四组合的小额度拒绝不发布缓存、释放归零；拒绝后编辑压缩对象、充足额度保存不会被源展开覆盖，与独立普通保存字节一致且严格重开保留页/修改值。仅覆盖固定样本，不表示兼容性/持久总量/后端内存完整验收。
  - [x] 固定普通xref源流保存冷加载：未引用64 KiB流在严格打开时未加载，小额度保存拒绝不发布该缓存、额度归零且修订号不变；充足额度重试输出一致，缓存转为持久内存后普通/预算保存与页面读取不变。只覆盖此固定样本，不代表Compressed/ObjStm完整保存或持久缓存总量验收。
  - [x] 固定内存文档保存结果组合预算：Preserve/Flate结果持有期间计费，保留首结果导致第二次合计超限拒绝且原输出/修订号不变，释放后同池重试恢复；充足额度下双结果共存并逐份归还容量。仅覆盖已计费分配，不代表冷源加载、编码器私有内存或完整AR-03验收。
  - [ ] 保存/渲染接入，验证返回转移、扩容共存、失败/unwind释放及原生后端未计费边界。内部预算保存已接入对象序列化、最终输出、源物化副本、对象ID表、xref槽位、待写对象副本、trailer字段/构建表、对象图预检、功能最低版本预检工作表/对象副本、有效版本计算副本及压缩输出/重建字典/显式暂存工作区；固定语法数字格式化已改用栈缓冲。编码器后端私有状态、诊断/路径缓冲和持久容器增长等仍有未计费边界，尚不开放完整预算保存公共接口。顺序与验收细节见docs/architecture.md的AR-03小节。
- [ ] AR-04（A3 / P1-06、P1-08）明确原始编码流与修改后解码内容的状态转换，统一 Filter/DecodeParms/Length 和有效版本策略。验收：未修改流保留编码，修改流字典与字节一致，Header/Catalog/功能最低版本无冲突。
- [ ] AR-05（A3 / P2-04）实现文档身份校验、对象图复制及资源复制后修改。验收：跨文档同号对象不误复用、循环与共享关系保留、目标页修改不影响其他页、导入失败整体回滚。
- [ ] AR-06（A4 / P3-01）设计 ContentProgram、流边界和来源映射，再实现内容编辑。验收：未知操作可保留、目标内容可定位、未修改片段不被无意重写；DisplayList 仅用于绘制/提取。

先推进 AR-01～AR-04 的契约与基础收口；AR-05 在页面导入前完成，AR-06 在文字替换前完成。各任务可先提交小范围兼容迁移，不要求一次替换整个核心。

### 近期交付优先级

2026-10-04 用户调整：暂缓 fuzz 的实现、运行和 CI 接入，保留已有代码与验证记录，未完成项不勾选。近期主线改为：保存版本/流状态收口（AR-04）、文档状态与事务契约（AR-01/02）、中文字体构建闭环（P1-05）、页面重排与导入（P2）。常规单元/集成测试继续执行；fuzz 和长时异常输入专项在用户要求恢复后再推进，不阻塞其余功能交付。

以下是下一批工作，不以已有低层 API 或部分实现代替阶段验收。

| 优先级 | 工作包 | 交付与验收 | 架构前置 |
|---|---|---|---|
| 1 | 保存与流状态收口（AR-04 / P1-08） | 统一有效版本、原始/解码流状态和压缩策略；保存失败不破坏旧文件 | Serializer、SaveOptions；不把输出长度当峰值内存 |
| 2 | 文档状态与事务（AR-01/02） | 状态与输入源解耦；修订号和缓存规则；错误/嵌套回滚验证 | ObjectStore、Trailer、文档身份 |
| 3 | 中文字体及创建验收（P1-05～10） | 嵌入字体、CID 编码、ToUnicode；中英文/图片多页文件独立显示和提取正确 | ResourceManager、字体字符码与 Unicode 分离 |
| 4 | 常用页面编辑（P2） | 重排、复制、跨文档导入、合并拆分；共享对象、继承属性与失败回滚回归 | ObjectGraphCopier、文档身份与事务 |
| 5 | 内容编辑与高级能力（P3–P5） | 先内容来源定位，再文字/图片修改；批注、表单、加密、增量与签名单独验收 | ContentProgram；保留原始字节与变更记录 |

不在此文档安排无依据的完成百分比或承诺工期；每个工作包完成范围与样本确认后再估时。

P0 的普通回归、错误处理、预算和真实样本登记贯穿上述工作包；fuzz 专项暂缓，未完成的安全/兼容验收仍明确保留。

### 当前执行队列（不含 fuzz）

上述表格保留阶段优先级；结合已有实现，接下来的具体交付按以下队列推进：

- [x] P3-02 顶层图片来源定位：将图片绘制事件直接关联到 ContentProgram 的操作索引与字节位置；重复 Do、注释、多内容流（含跨流操作符）、未知操作索引及严格保存重读已验收，不从 DisplayList 绘制顺序反推来源。只完成此队列项，不代表下方完整 P3-02 完成。
- [ ] P3-02 剩余范围：完善 Form/内联图片解释与分层来源路径，定义嵌套资源作用域、矩阵/裁剪、循环与递归预算；补齐文字/路径命中及稳定定位验收。非Group Form已有子解释器、分层来源/只读叶定位、资源scope及本库路径/文字/图片BBox像素回归；Group/内联图片、独立阅读器与完整命中仍待验收。
- [ ] P3-04：顶层Form单次删除/变换/替换，以及沿嵌套调用链复制后删除路径/图片/文字、替换文字、修改路径数值和变换图片已有专项回归；覆盖重复调用/共享页隔离、资源scope、失败整体回滚及嵌套路径/图片/Helvetica文字像素；图片绑定替换、跨文档导入后源释放已有专项。顶层及同一嵌套叶调用的普通/补偿跨事件范围已实现，合成混合字符码横竖排、来源/预算拒绝和回滚已定向验证；固定真实水平Noto两Tj样本已独立结构/文本/坐标和目视验证。带来源两端接口目前明确拒绝跨不同调用，不等于该功能完成。下一步为跨调用多分支合并、扩大真实字体/TJ/竖排样本、命中定位及完整独立阅读器矩阵。显示坐标与当前用户坐标明确区分，不将相对变换称为完整可视化编辑。
- [ ] AR-04 / P1-08：扩大真实文件保存读回和独立阅读器验证，整理尚未覆盖的功能最低版本；保持未修改编码流，不自动宣称完整规范符合性。
- [ ] P1 / P2：补齐中英文、图片、多页构建与导入/重排样本矩阵；分别记录本库回归与独立工具验证。
- [ ] AR-03：实现操作级聚合预算；单项上限和输出长度不能替代峰值内存约束。

执行约束：不新增或运行 fuzz，不接入 fuzz CI，不删除已有 fuzz 文件；常规单元、集成、像素回归和合法样本验证继续。每项仅在其验收条件达成后勾选，不因暂缓 fuzz 将完整项目标记完成。

### 跨不同调用范围的下一批实施前置

- [ ] 在原root展开顺序中解析带来源的两端，明确包含哪些中间文字事件；处理空字符串/纯数值TJ，不从有字形的DisplayList倒推全部事件。验证调用链、递归/循环及操作总量上限。
- [ ] 将每个目标叶的事件范围映射到原有效资源scope与继承状态，共用TextEventEditPlan；新文字只在起点字体编码，累计所有目标的字符/内存额度，不能逐叶绕过总量。
- [ ] 按原调用路径建立预算化多分支编辑树，合并公共祖先，保留其他调用、资源与非文字操作；最终统一复制叶/祖先及局部Do重绑。不得把逐叶独立提交后刷新快照当作已验证的多分支合并。
- [ ] 所有计划构建/资源分配成功后一次事务发布；覆盖不同深度、共享同一Form、多级公共祖先、原操作索引变化、较晚分支失败及外层错误/panic回滚和额度释放重试。
- [ ] 完成上述实现后再开放跨不同调用span，验证严格重开、独立文本/坐标/真实字体渲染；现有span不同调用拒绝契约在实现验收前保持。此清单不替代其他P3/P1/P2/AR未完成项。

### 实施记录

- 2026-10-08 公共父程序批量重建草稿：路径树增加rebuild_parent，先验证全部原Do位置/操作数/字节位置，再按原调用顺序准备绑定片段，一次批量重建并验证结果仍为单Do，片段与最终程序共同持有预算；不插入对象、不发布资源字典。首轮测试闭包缺usize类型导致编译失败，修正后隔离原session55928退出0，6项通过、0失败/忽略（636项过滤），日志/tmp/rzpdf-span-parent-isolated-final.log；逆输入兄弟顺序、双绑定合并/未知邻接保持、第二绑定错误释放、非Do拒绝均通过，取代父内容合并待确认状态。主模块仍未接入，真实scope复制/叶计划组合/公共祖先资源合并及原子发布待实现，完整工作区原session46179继续运行，P3/AR-03未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 多分支准备arena补充：增加子节点优先遍历、预算化原操作顺序子节点列表和泛型build_bottom_up结果表，保留全部子结果到调用方发布，回调只准备不提交文档。首版隔离原session24621退出0，4项通过；增强所有者版原session58193退出0，5项通过、0失败/忽略（636项过滤），日志/tmp/rzpdf-span-tree-build-owner-isolated.log。较晚分支错误/panic精确载荷释放、重试以及结果析构期间元数据仍计费均通过，取代增强版待确认状态。主模块仍未接入，实际叶内容/公共祖先合并与事务发布未完成；完整工作区原session46179继续运行，fuzz暂缓，P3/AR-03未完成，后续改动未提交。

- 2026-10-08 路径树顺序隔离终态：原session70505退出0，临时副本span_tree 3项通过、0失败/忽略（636项过滤），日志/tmp/rzpdf-span-tree-order-isolated.log，取代新增顺序算法待确认状态。公共祖先/不同深度/兄弟/重复叶的原调用顺序、非法来源槽/文本与Do位置冲突，以及既有拓扑和实际表预算释放通过。主工作区模块仍未接入，不能称跨调用编辑实现或主构建验收；下一步需将原scope叶计划、公共祖先资源/Do重绑合并并一次事务发布。完整工作区原session46179仍运行，已显示批量编辑5项通过但整体未终态，P3/AR-03未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 路径树首轮隔离结果：原session4085退出0，临时副本的span_tree两项测试通过、0失败/忽略（636项过滤），日志/tmp/rzpdf-span-tree-isolated.log。实际唯一前缀/重复叶、矛盾路径、节点和叶表计费、首/第二表额度失败释放均验证；仅为未接入的基础表证据，不证明多分支编辑或主构建集成。继续新增只读arena访问、公共祖先定位与原调用位置顺序比较，覆盖不同祖先深度/兄弟/重复叶/非法source槽及文本位置与Do冲突；原session70505日志/tmp/rzpdf-span-tree-order-isolated.log运行中，未借用旧两项结果证明新增算法。完整工作区原session46179继续运行，生产源码不变，P3/AR-03未完成，fuzz暂缓。

- 2026-10-08 多分支树隔离验证启动：路径树草稿增加唯一节点计数超限提前拒绝和第二张表额度不足释放测试；只在临时源码副本/tmp/rzpdf-span-tree-validation.zh9AQL接入mod span_tree，使用其独立target执行cargo test -p pdf document::span_tree::tests --lib --locked --offline，原session4085日志/tmp/rzpdf-span-tree-isolated.log运行中。主工作区模块仍未接入，完整回归原session46179已显示636项单元通过并继续其他组，不以局部通过计整体退出0；两门禁证据范围不同，不以副本基础表测试证明跨调用编辑。P3/AR-03/多分支实际合并及全目标未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 跨调用多分支树草稿：新增未接入模块pdf/src/document/span_tree.rs，按原调用位置合并共享前缀、为每个输入来源保留叶节点映射，区分同Form不同调用，拒绝同父调用位置的矛盾ID/字节位置；节点/叶映射表按实际容量持有预算租约，预计算实际唯一前缀而不是路径长度之和。包含共享祖先/重复叶、矛盾路径及额度失败释放测试草稿，已rustfmt语法检查，但尚未编译执行或接入计划/提交，不将其算作功能验收、不开放跨调用span。完整工作区原session46179继续验证既有已接入源码，当前新模块未被mod引用，不改变该门禁生产范围；待门禁结束后接入并验证，再实现叶计划/公共祖先编辑合并和整体事务。P3/AR-03及全目标未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 真实跨事件正负对照脚本：新增docs/check-cjk-event-negatives.sh，普通合法输入先通过，再用独立qpdf交换顶层/嵌套四页输出的补偿与普通页；交换样本语法合法，比较脚本必须退出1且已到MuPDF四页坐标阶段，防止缺依赖/结构错误误算拒绝。bash -n及实际原session59505均退出0，日志/tmp/rzpdf-cjk-event-negatives.log；脚本只删除自身新建临时负例和日志，原输入/生产代码不改。按PDF技能补独立合法样本验证，非fuzz；完整工作区原session46179仍运行，不以此脚本结果替代工作区门禁或完整P3/AR-03验收，后续改动未提交。

- 2026-10-08 当前工作区完整常规回归启动：cargo test --workspace --locked，原session46179日志/tmp/rzpdf-current-workspace-validation.log运行中，尚未验收。工作区成员为pdf/render/font-data，不调用cargo fuzz，不新增fuzz/CI；本门禁用于检查当前跨事件/嵌套/来源端点改动对保存、页面编辑、提取及渲染等既有功能的影响，不复用c81fd6d旧工作区结果。生产源码当前冻结待门禁，后续如改动需重新标定证据范围；阶段P3/AR-03以及TODO各未完成项不因定向或工作区回归通过而自动勾选，后续改动未提交，test.png保留。

- 2026-10-08 嵌套来源限定端点终态：原session41041退出0，PDF单元636项、nested_adjusted_text 10项、text_event_range 8项全部通过、0失败/忽略，合计654项，日志/tmp/rzpdf-nested-span-final.log，取代该命令待确认状态。新span接口两端不同调用链/错误结束字节位置拒绝、stamp及内容保持、正确端点普通/补偿成功、其他调用/共享页/严格重开通过，README与接口文档更新。source仍是文档局部路径而非独立持久身份，需当前root配对；旧裸索引语义不变。此为定向门禁，不是完整工作区、全部字体/失败点或跨不同Form实际编辑验收，P3/AR-03未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 嵌套来源限定端点实现中：新增借用NestedDrawSource的NestedTextCodePosition与replace_nested_text_span/其preserving_advance入口。两端先受来源额度约束，调用链不同时明确Unsupported，文本端点拒绝路径geometry；同调用时沿原链验证root/start并核对end操作原字节位置，再复用既有范围计划/一次事务提交，不复制调用链或改旧API语义。编译检查原session85542退出0；新增同数值操作索引但不同调用拒绝、错误结束字节位置拒绝/stamp原内容保持、正确两端普通/补偿成功及其他调用/共享页/严格重开回归，最终原session41041日志/tmp/rzpdf-nested-span-final.log运行中，尚未验收。跨不同Form选择仍未实现，P3/AR-03/完整字体与失败矩阵/独立阅读器继续待完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 嵌套范围来源终态：原session80528退出0，nested_adjusted_text 9项通过、0失败/忽略、4.67秒，日志/tmp/rzpdf-nested-event-provenance.log，取代来源专项待确认状态。普通/补偿选中第四调用、前三调用与共享页保持、该调用补偿位置和严格重开通过；错误页面/离线/外文档root拒绝且stamp及源保持。文档已澄清裸端点按所选叶程序解释、未携带独立调用身份，不声称自动识别另一Form的裸结束索引或已实现跨调用范围。生产代码未改，636项单元证据仍对应当前生产版，P3/AR-03未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 嵌套范围来源契约补验中：新增选中两层四调用的最后一次调用，验证相同叶操作索引由不同invocations限定，仅第四调用变化、前24字符与共享页保持，补偿保留末端位置并严格重开。错误页面/离线/外文档root即使源码或对象号相似仍拒绝，拒绝后stamp及原内容保持再用正确root成功。普通/补偿均覆盖，专项原session80528日志/tmp/rzpdf-nested-event-provenance.log运行中，生产代码未改；端点本身是所选叶程序的相对索引，不能据此声称可识别带另一个来源身份的结束端点或已实现跨不同Form。P3/AR-03未完成，fuzz暂缓。

- 2026-10-08 真实中文顶层跨事件及默认CLI终态：原session39731/67694分别退出0，顶层--events及旧默认示例输出通过内置自检/严格重开，分别经现有脚本独立qpdf/MuPDF结构/坐标/文本检查通过。顶层跨事件四页96DPI PNG逐页cmp与已目视的嵌套跨事件输出完全一致，原session17875退出0；日志/tmp/rzpdf-cjk-event-independent.log与/tmp/rzpdf-cjk-event-default-independent.log。README和中文验证文档已记录命令、范围及真实嵌套PDF哈希，取代对应待确认记录。生产代码未改；这固定水平Noto/相邻两Tj样本不等于全部TJ/字体切换、真实竖排、跨不同Form或完整P3/AR-03验收，fuzz暂缓，后续改动未提交。

- 2026-10-08 真实中文嵌套跨事件限定验收：原session27878退出0，--nested --events四页输出完成并通过内置坐标/共享基线/displaylist/严格重开自检；qpdf/MuPDF独立结构、字体嵌入/图像/Form复制关系、尾部P/D/F坐标及文本检查退出0，日志/tmp/rzpdf-cjk-nested-event-independent.log；96DPI四页PNG逐页目视正常。Poppler缺libtiff.5.dylib，按PDF技能改用现有MuPDF渲染，未改系统依赖。真实静态Noto水平相邻两Tj/同一两层Form调用内范围证据、命令和PDF哈希见docs/cjk-creation-validation.md；顶层--events原session39731与旧默认CLI原session67694另运行，不借用本结果证明它们。真实竖排/跨不同Form/完整字体及失败矩阵/P3/AR-03未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 真实中文跨事件对照实现中：compare_cjk_advance新增--events（可与--nested组合），以原已嵌入字体把中文/英文原句拆成相邻两Tj，再用范围普通/补偿入口生成四页对照，保留原默认及单事件嵌套模式。新增模式自检仍检查尾部PDF位置、共享基线、displaylist及严格重开，原session27878日志/tmp/rzpdf-cjk-nested-event-advance.log运行中；输出output/pdf/rzpdf-cjk-nested-event-advance.pdf待生成与qpdf/MuPDF独立验证/逐页渲染目视，尚未验收。按PDF技能使用真实项目输出而非替代生成器；跨不同Form/真实竖排/完整字体及P3/AR-03仍未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 嵌套多事件额度终态：原session13635退出0，nested_adjusted_text 8项通过、0失败/忽略、4.57秒，日志/tmp/rzpdf-nested-event-quota.log，取代额度专项待确认状态。普通/补偿的重复大替换拒绝后stamp/整条旧Form链/资源/共享两页源及流ID保持，原root/source小替换重试与严格重开通过。生产代码未改，636项单元证据仍对应当前生产版，接口文档已补限定验收。下一步扩大真实字体跨事件独立阅读器样本，不把合成CMap、共享隔离和错误回滚当作真实字形视觉；跨不同Form/完整失败矩阵/P3/AR-03仍未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 嵌套多事件预算补验中：新增512KiB操作额度下普通/补偿两种范围入口重复大替换ResourceLimit拒绝，核对stamp、旧叶/父Form、资源字典、共享两页源码和Contents流ID保持，再使用原root/source小替换成功并严格重开验证共享页和文本。专项原session13635日志/tmp/rzpdf-nested-event-quota.log运行中，生产代码未改；此为公共多事件拒绝重试，不以既有单事件额度测试代替，也不宣称全部失败点/精确峰值。跨不同Form/真实竖排/独立阅读器及P3/AR-03未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 嵌套范围复合字体终态：原session71434退出0，nested_adjusted_text 7项通过、0失败/忽略、5.23秒，日志/tmp/rzpdf-nested-event-mixed-cid.log，取代复合补验待确认状态。继承混合一/两字节Type0跨Tj/TJ/Tj的W/W2×普通/补偿×B/空/B加双字节空格十二组合通过，起止截断码拒绝/stamp资源保持、三段补偿独立数值/Tw规则/保留后缀与下一行、普通TJ项数和原间距、其他三调用/共享页/旧叶及严格重开通过。生产代码未改，636项单元门禁仍对应当前生产版，内容编辑文档已更新限定证据；真实竖排字形/跨不同Form/完整字体与失败矩阵/独立阅读器及P3/AR-03仍未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 嵌套范围集成终态及复合补验：原session78782退出0，nested_adjusted_text首版5项与text_event_range 8项全部通过，日志/tmp/rzpdf-nested-text-event-integration.log，取代该命令待确认状态；不将此旧5项冒充后续6项失败矩阵。继续新增混合一/两字节Type0同叶跨事件：横向W/竖向W2、普通/补偿、B/空/B加双字节空格十二组合，起止截断码拒绝/stamp资源不变、补偿三段独立值/保留后缀和下一行、普通TJ项数/原间距、其他三调用及共享页、旧叶对象和严格重开。专项原session71434日志/tmp/rzpdf-nested-event-mixed-cid.log运行中，生产代码未改；合成竖排不代表真实竖排字形或独立阅读器验收，P3/AR-03未完成，fuzz暂缓。

- 2026-10-08 嵌套叶范围失败补验终态：原session40020退出0，nested_adjusted_text 6项通过、0失败/忽略、4.75秒，日志/tmp/rzpdf-nested-text-event-rollback.log，取代增强专项待确认状态。来源锚点/不可编码拒绝、普通及补偿外层错误/精确panic回滚、资源/流ID/旧Form恢复、旧来源拒绝和刷新重试均通过；README与接口文档已说明同一叶调用端点、继承状态及非跨不同Form的边界。636项单元门禁仍对应未改动的生产代码；原session78782集成顶层组仍待终态。新增嵌套范围的复合字体/完整失败矩阵/独立阅读器仍待扩大，P3/AR-03未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 嵌套叶跨事件首轮回归：PDF单元原session70664退出0，636项通过，日志/tmp/rzpdf-nested-text-event-units.log；专项日志/tmp/rzpdf-nested-text-event-integration.log已显示nested_adjusted_text 5项通过（两层四调用普通/补偿范围、新旧Form/共享页/保留位置/严格重开），原session78782尚在运行顶层text_event_range组，不以局部通过计整个命令退出0。新增来源锚点拒绝、不可编码文字及普通/补偿外层错误/精确panic回滚、流ID/资源/旧Form恢复/旧来源拒绝/刷新重试回归，原session40020日志/tmp/rzpdf-nested-text-event-rollback.log仍待终态。生产代码未改，尚未以5项证明新增失败矩阵；跨不同Form/真实复杂字体/独立阅读器及P3/AR-03未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 嵌套叶Form跨事件实现中：拆出拥有完整结果及字体/字符码/片段租约的TextEventEditPlan，字体状态由原scope解析回调提供，与顶层共用端点/聚合额度/源重建而非复制整套算法。新增replace_nested_text_event_range及preserving_advance入口，source锚定起始事件，端点限定同一选中叶调用；逐事件重建原调用链继承状态，最后复用逐层Form复制/Do重绑事务。编译检查退出0，新增两层四调用、普通/补偿三种替换、继承状态/保留位置/其他调用与共享页/旧Form对象不变/严格重开回归；专项原session78782日志/tmp/rzpdf-nested-text-event-integration.log及单元日志/tmp/rzpdf-nested-text-event-units.log运行中，尚未验收。跨不同Form选择、全失败/复杂字体矩阵与独立阅读器仍待完成，完整P3/AR-03未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 普通/补偿跨事件最终门禁终态：原session76521退出0，PDF单元636项、adjusted_text 14项、content_program 29项、text_event_range 8项全部通过、0失败/忽略，合计687项，日志/tmp/rzpdf-text-event-range-ordinary-fragments-final.log。普通模式保留Tj/两类引号操作及TJ项数，不生成零调整；零字号/后续文字独立位移、引号Tw/Tc数值拼写、共享隔离与严格重开回归通过。补偿模式既有混合字符码横竖排/预算拒绝/错误及panic回滚同时通过，README与接口文档已更新。取代该最终命令待确认状态，不复用首版零调整片段门禁证明本版；并非完整工作区或全部字体/失败点验收。下一步为可复用的显式资源scope/继承状态批量计划及嵌套范围，另需独立阅读器与真实字体扩大样本。P3/AR-03仍未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 首版普通模式旧门禁终态：原session23422退出0，adjusted_text 14项及text_event_range 7项通过，日志/tmp/rzpdf-text-event-range-ordinary.log；原session55831退出0，PDF单元636项通过，日志/tmp/rzpdf-text-event-range-ordinary-units.log。两者对应零调整片段的首版，不覆盖随后普通Tj/引号及TJ项数保持改造。最终原session76521已完成编译，单元进程确认存活，日志/tmp/rzpdf-text-event-range-ordinary-fragments-final.log继续观察；不重启、不以旧结果代替新门禁。整体P3/AR-03未完成，fuzz暂缓。

- 2026-10-08 普通跨事件片段语义调整：普通模式不再生成补偿格式中的零调整/空后缀；新增预算原始片段构建，Tj/两类引号保持原操作，双引号Tw/Tc数值拼写保留，TJ直接更新选中字串且项数不变，不承担补偿的额外数组项额度。补偿模式仍按原路径拆分并调整。新增普通引号/TJ项数/操作数/严格重开回归；最终专项与单元/解析日志/tmp/rzpdf-text-event-range-ordinary-fragments-final.log运行中，首版普通模式两组测试仍观察终态，但不覆盖本调整。跨Form/完整字体/独立阅读器及P3/AR-03仍未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 跨事件普通Unicode替换实现中：新增replace_page_text_event_range，复用字符码端点/起点字体编码/额度/批量源重建和事务提交，普通模式不计算新旧度量（因此零字号不因补偿而拒绝），不保持后续文字原推进。保留原TJ数字、引号状态及结束后缀，当前输出仍拆为前缀/零调整/后缀TJ，提交后刷新项索引。新增零/非零字号、后缀与后续文字独立位移、共享隔离及严格重开回归；专项和单元正在验证，日志/tmp/rzpdf-text-event-range-ordinary.log与/tmp/rzpdf-text-event-range-ordinary-units.log，未验收。P3/AR-03/跨Form范围/独立阅读器仍未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 跨事件预算补验终态：原session17541退出0，text_event_range 6项通过、0失败/忽略、0.33秒，日志/tmp/rzpdf-text-event-range-memory.log，取代预算测试待确认记录。512KiB操作额度重复大替换拒绝后，stamp/资源字典/流ID/共享两页源码保持，原快照可直接小替换并严格重开；既有复合字体与回滚矩阵同时通过。生产代码未改；不代替同池精确峰值/全部失败点、跨Form或独立阅读器验收，P3/AR-03仍未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 跨事件预算拒绝重试补验中：新增512KiB操作额度下的重复大替换拒绝，检查ResourceLimit、revision/stamp不变、资源字典/原流ID/共享两页源码不变，再用同一快照小替换成功并严格重开验证原页隔离。此为公共入口预算拒绝与重试，不宣称测得同池峰值差1或全部失败点；专项日志/tmp/rzpdf-text-event-range-memory.log仍待终态，生产代码未改，fuzz暂缓，P3/AR-03未完成。

- 2026-10-08 跨事件复合字体终态：原session32708退出0，text_event_range 5项通过、0失败/忽略、0.33秒，日志/tmp/rzpdf-text-event-range-mixed-cid-final.log。新增混合一/两字节Type0跨Tj/TJ/Tj范围，横向W及竖向W2各覆盖B/空/B加双字节空格；独立核对三段调整、双字节空格排除Tw、起止截断码精确错误及stamp不变、TJ原数值、末端后缀/后续事件/下一行位置、共享原页与严格重开。生产代码未改，内容编辑文档已补本限定证据；不把合成竖排当真实竖排字形或独立阅读器验收。跨Form范围、全部字体/失败矩阵及完整P3/AR-03仍未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 跨事件既有集成终态：原session51452退出0，adjusted_text 14项、content_batch_edit 5项、text_event_range 4项全部通过，日志/tmp/rzpdf-text-event-range-integration-v2.log，取代此前该命令待确认状态。继续增加混合一/两字节Type0跨事件范围回归，分别核对W/W2及Tc、双字节空格不加Tw、起止截断码拒绝、TJ原数值、结束后缀/后续文字/下一行位置、共享页及严格重开。首轮测试误用了PdfNumber::as_f32，已修正为value并校准实际错误文本，复验运行中；不以首轮失败或既有4项证明新矩阵，生产代码未改，P3/AR-03及独立阅读器验收仍未完成，fuzz暂缓。

- 2026-10-08 跨事件最终生产回归终态：原session57677退出0，PDF单元636项、content_program 29项、text_event_range 4项全部通过，日志/tmp/rzpdf-text-event-range-final.log，取代前条该命令待确认状态。生产代码冻结；原session51452中既有补偿14项及批量5项已通过，最后跨事件组仍待该命令终态。此为定向669项证据，不称当前完整工作区验收；P3/AR-03仍未完成，后续改动未提交。

- 2026-10-08 跨事件范围首轮功能证据：原session49285退出0，text_event_range 4项通过、0失败/忽略，日志/tmp/rzpdf-text-event-range-expanded.log。覆盖字体切换/TJ/两类引号/保留后缀位置与共享页严格重开、非法端点、跨事件聚合字符额度、外层错误/精确panic载荷回滚及原流ID恢复/旧快照拒绝/刷新重试。README与内容编辑文档已说明字符码字节端点、起点编码、逐事件度量、中间状态保留及非排版边界。最终生产版单元日志已显示636项通过，但整个解析/跨事件命令原session57677仍待终态；既有补偿14项通过，批量/跨事件集成原session51452仍待终态，不宣称整个命令完成。混合字节Type0跨事件/真实竖排/跨Form/独立阅读器及完整P3/AR-03仍未完成，fuzz暂缓，后续改动未提交。

- 2026-10-08 本地提交2ea8ff8已保存TJ/嵌套推进补偿、批量操作与CJK验证；提交范围定向694项测试退出0，日志/tmp/rzpdf-commit-validation.log。跨文字事件Unicode范围草稿未纳入该提交，现继续实现replace_page_text_event_range_preserving_advance：原快照字符码端点、起始事件字体编码、逐事件有效度量及后缀前补偿、保留中间状态操作，最后统一批量事务提交。新增字体切换/TJ/两类引号/空及纯数值TJ、共享隔离/严格重开/非法端点回归，当前编译检查退出0，专项与最终单元/解析回归仍在运行，尚未验收。跨Form范围、完整字体/失败矩阵及独立阅读器验证仍待完成；P3/AR-03保持未完成，fuzz暂缓，不自动提交后续改动。

- 2026-10-08 批量片段最终终态：原session72276退出0，content_batch_edit 5项、既有content_program 29项全部通过、0失败/忽略，日志/tmp/rzpdf-batch-content-integration-final.log；原session50444退出0，PDF单元636项通过、0失败/忽略、4.07秒，日志/tmp/rzpdf-batch-content-pdf-unit-final.log，取代前条待确认状态。最终双侧分隔/文档多事件事务均覆盖，不复用首版仅单侧分隔的原session91507（三项及29项退出0）或原session38509（636项退出0）作为最终证据。此为跨事件范围的批量源重建/原子提交基础，下一步接入字符码端点、分段有效字体、Unicode编码及逐事件后缀前补偿；不将原始片段/手动TJ测试等同自动跨事件编辑，完整P3/AR-03未完成，fuzz未运行、新增改动未提交，test.png保留。

- 2026-10-08 跨事件批量重建前置：新增OperationReplacement与with_replaced_operation_sequences_with_budget，按原操作索引严格有序验证片段，片段/描述表/完整结果在同池共存，一次遍历原流边界重建并统一最终解析，逐一核对未选操作及操作数。片段两侧换行防旧字符串分隔变词法粘连/尾注释吞邻接；跨流操作、空流及其他字节保留。新增replace_page_operation_sequences通过原快照身份/流ID与共享事务一次提交多事件，不逐事件刷新/提交。新content_batch_edit覆盖跨3流/空流/未知操作、插入多指令/删除、邻接词法、索引/计数/额度拒绝、峰值差1/邻居/双结果/unwind、公共两事件共享隔离/外层错误/指定panic/重试及严格重开。首轮check原session81721退出0；随后增加双侧分隔和文档入口，最终集成原session72276日志/tmp/rzpdf-batch-content-integration-final.log、单元原session50444日志/tmp/rzpdf-batch-content-pdf-unit-final.log待终态，Rust源码冻结。旧单侧版本原session91507/38509仍需观察但不替代最终结果；跨事件Unicode端点/分段字体/自动补偿仍未完成，不以底层批量提交替代完整P3，fuzz未运行，新增改动未提交。

- 2026-10-08 嵌套复合字体及真实中文独立验证：原session28938退出0，nested_adjusted_text 4项通过、0失败/忽略、4.95秒；新增混合一/两字节CMap水平W/竖排W2，五类目标/长短空替换、双字节空格排除Tw、独立调整/保留后缀/其他调用/共享页/严格重开及跨项三个截断码边界拒绝。生产代码未改，636项单元基线仍对应当前生产版。compare_cjk_advance新增--nested两层真实静态Noto Sans SC四页补偿对照，最终原session82446退出0（首轮Option字典转换示例编译错误已修正）；qpdf11.2.0/MuPDF1.27.0结构、独立P/D/F坐标、归一化文本、四页96DPI目视与同分辨率顶层PNG逐页cmp全部通过。脚本按唯一Form引用统计保守保留的旧资源别名，错误模式和交换对照页负例拒绝，详情/哈希/命令见docs/cjk-creation-validation.md。原默认CLI回归原session25317退出0，生成的新顶层对照也经新脚本通过复验；全工作区原session7661仍运行且为提交c81fd6d，不覆盖新代码。真实竖排字形/TJ范围独立视觉/跨事件/完整P3/AR-03未完成，fuzz未运行，新增改动未提交，test.png保留。

- 2026-10-08 嵌套补偿最终集成终态：原session61779退出0，adjusted_text 14项、nested_adjusted_text 3项、既有nested_text_edit 6项全部通过、0失败/忽略，日志/tmp/rzpdf-nested-compensation-integration-final.log；本版PDF单元原session6117退出0、636项通过、1.99秒，取代此前待确认状态。固定Courier嵌套矩阵勾选；实际五类文字目标、两层四调用/单次复制、独立调整/邻接位置、共享页/旧Form/严格重开、不可编码/外层错误/指定panic/聚合额度拒绝与重试通过。完整P3/AR-03仍未完成；下一步补嵌套复合字体/竖排和独立视觉验证、跨文字事件范围。提交c81fd6d的全工作区原session7661仍未终态，不覆盖新代码；未运行fuzz、新增改动未提交、test.png保留。

- 2026-10-08 嵌套补偿版单元终态：原session6117退出0，PDF单元636项通过、0失败/忽略、1.99秒，日志/tmp/rzpdf-nested-compensation-pdf-unit.log。首轮集成原session94430编译退出101，仅测试夹具页面变量缺mut；修正后原session61779日志/tmp/rzpdf-nested-compensation-integration-final.log已通过adjusted_text 14项，新增nested_adjusted_text及既有nested_text_edit仍待终态，Rust源码继续冻结。未将旧版14项或首次编译错误当作新增嵌套专项通过；fuzz未运行，新增改动未提交。

- 2026-10-08 嵌套推进补偿提交接入：新增replace_nested_text_preserving_advance、replace_nested_text_array_item_preserving_advance、replace_nested_text_array_range_preserving_advance，复用调用链有效字体/字号/间距状态与同池叶重建、逐层复制/局部Do重绑和事务提交。顶层/嵌套范围提取共用BudgetedTextAdjustments，按被删码段度量并在保留后缀前调整，元数据实际容量同池所有权。新nested_adjusted_text两层四调用矩阵覆盖Tj/引号/TJ单项/跨项的长短删除、独立Courier调整、后缀/数组邻接/下一事件/下一行、共享页/旧Form、严格重开、不可编码/外层错误/指定panic/刷新重试和公共聚合额度拒绝。cargo check原session97973退出0；最终集成原session94430日志/tmp/rzpdf-nested-compensation-integration.log、PDF单元原session6117日志/tmp/rzpdf-nested-compensation-pdf-unit.log待终态，Rust源码冻结。完整P3/跨事件/复杂字体/独立视觉及AR-03仍未完成，未运行fuzz，新增改动未提交，test.png保留。

- 2026-10-08 复合范围/嵌套状态最终集成终态：原session1678退出0，adjusted_text 14项、nested_text_edit 6项、text_source 9项全部通过、0失败/忽略，日志/tmp/rzpdf-inherited-text-state-integration-final.log；该版PDF单元原session91993退出0、636项通过，取代此前待确认状态。混合一/两字节横竖排范围及双字节空格独立数值通过，嵌套调用链完整字体/字号/间距状态传递已有回归。下一步接入嵌套补偿公开入口及逐层复制提交，仍不表示真实字形/独立阅读器、跨事件或完整P3/AR-03完成。全工作区原session7661仍在运行且对应提交c81fd6d，不覆盖新代码；未运行fuzz，新增改动未提交，test.png保留。

- 2026-10-08 嵌套状态版单元终态：原session91993退出0，PDF单元636项通过、0失败/忽略、2.01秒，日志/tmp/rzpdf-inherited-text-state-pdf-unit.log，含新增无字体父流/三层状态/qQ/所有权错误与unwind专项。此前仅复合字体测试版原session17633退出0，adjusted_text 14项通过、1.73秒，但不覆盖之后状态代码；最终集成原session1678日志/tmp/rzpdf-inherited-text-state-integration-final.log仍待终态。完整目标保持进行中，嵌套公开补偿/复杂字体独立视觉仍未完成，fuzz未运行，新增改动未提交。

- 2026-10-08 复合字体范围与嵌套状态前置：新增混合一/两字节Type0横/竖排范围专项，显式W/W2独立调整、双字节空格排除Tw、保留前缀/后缀/下一事件/下一行、严格重开/共享隔离及三种截断字符码边界精确拒绝。首轮原session17633日志/tmp/rzpdf-mixed-cid-range-compensation.log仍待终态；随后新增BudgetedTextState，font可空而标量状态继续传递，继承字体所有者移动不Clone；嵌套Unicode绑定沿调用链和叶前缀复用完整状态，新增无字体父流、q/Q恢复字体/字号/间距、失败与所有者unwind专项。最终单元原session91993日志/tmp/rzpdf-inherited-text-state-pdf-unit.log、集成原session1678日志/tmp/rzpdf-inherited-text-state-integration-final.log待终态。首次集成命令因不存在nested_text_range目标退出101，已按实际nested_text_edit/text_source目标修正，不当作实现失败或通过。当前Rust源码冻结；嵌套补偿公开提交入口仍未接入，真实竖排字形/独立视觉及完整P3/AR-03未完成，fuzz未运行，新增改动未提交。

- 2026-10-08 TJ跨项范围补偿终态：原session78035退出0，adjusted_text 13项、content_program 29项全部通过、0失败/忽略，日志/tmp/rzpdf-tj-range-compensation.log；原session52401退出0，PDF单元635项通过、0失败/忽略、1.94秒，日志/tmp/rzpdf-tj-range-pdf-unit.log，取代下条待确认状态。固定Courier范围矩阵勾选；完整连续编辑/P3/AR-03仍未完成。下一步补复合字体多字节边界/竖排范围、嵌套状态继承与独立渲染；提交c81fd6d的全工作区原session7661仍在运行，不是新版本全部工作区证据。未运行fuzz、新增改动未提交，test.png保留。

- 2026-10-08 TJ跨项范围补偿接入：新增replace_page_text_array_range_preserving_advance，复用完整字符码边界/总量验证与同池范围重建；每受影响字符串拆为前缀加插入文字、按被删部分计算的调整、保留后缀，补偿位于后缀前而非整串后，保留原数组数值/注释。低层批量入口单次扫描/完整重解析，预检每项新增两个数组元素，调整元数据与旧新缓冲共存计费。新增末项部分后缀、跨三项/单项/零宽插入与独立Courier数值/保留字符坐标、预算所有者/错误与panic回滚专项；原session78035日志/tmp/rzpdf-tj-range-compensation.log、PDF单元原session52401日志/tmp/rzpdf-tj-range-pdf-unit.log均待终态，Rust源码冻结。跨文字事件/嵌套、真实竖排/独立阅读器及完整P3仍未完成，fuzz未运行，新增改动未提交。

- 2026-10-08 TJ单项补偿终态：原session3616退出0，adjusted_text 10项及既有content_program 29项均通过、0失败/忽略，日志/tmp/rzpdf-tj-item-compensation.log；原session33192退出0，PDF单元635项通过、0失败/忽略、1.97秒，日志/tmp/rzpdf-tj-item-pdf-unit.log，取代下条待确认状态。固定Courier矩阵项勾选，完整P3/AR-03保持未完成。提交c81fd6d的完整工作区原session7661仍在运行，不作为当前TJ版本完整工作区证据；新增改动未提交，fuzz未运行，test.png保留。

- 2026-10-08 本地提交及TJ单项推进补偿接入：此前完成的文字补偿/中文编辑示例与文档已提交c81fd6d，未推送，test.png未纳入。该提交完整工作区测试原session7661、日志/tmp/rzpdf-precommit-workspace.log仍待终态，不覆盖随后改动。新增replace_page_text_array_item_preserving_advance与低层预算入口，在选中字符串后立即插入调整并补数字分隔，保留既有数值/注释/其他字符串，预检查增加一项的额度。新增数组内部邻接/下一事件/下一行、共享隔离/严格重开、峰值差1/邻居/结果所有权/unwind、公共错误与外层错误/panic回滚专项；原session3616日志/tmp/rzpdf-tj-item-compensation.log，PDF单元原session33192日志/tmp/rzpdf-tj-item-pdf-unit.log均待终态。当前Rust源码冻结；跨项范围/嵌套继承、真实竖排和完整P3/AR-03仍未完成，未运行fuzz，新增改动未提交。

- 2026-10-08 公共补偿外层unwind增强终态：原session71814退出0，adjusted_text七项通过、0失败/忽略、1.36秒，日志/tmp/rzpdf-compensation-outer-unwind-verified.log，含指定panic载荷确认；两共享页源码/Contents流ID恢复、旧快照拒绝、刷新重试及严格重开文本/后续字符/下一行位置均通过，取代此前待确认状态。此轮仅测试改动，生产版635项PDF单元证据仍有效；不表示所有失败点、TJ/嵌套或完整P3已完成，未运行fuzz、未提交。

- 2026-10-08 公共补偿unwind首轮终态与增强复验：原session45816退出0，adjusted_text七项通过、0失败/忽略、1.37秒；随后新增panic载荷精确断言，要求确为成功编辑后注入的“injected compensation commit unwind”，避免其他断言panic被误认成功。增强版原session71814、日志/tmp/rzpdf-compensation-outer-unwind-verified.log，终态待确认，Rust源码冻结，不沿用初版结果。生产代码未改，635项单元基线仍有效；未运行fuzz、未提交。

- 2026-10-08 自动补偿公共编辑外层unwind补验：新增adjusted_text回归，对Tj/两类引号先成功补偿并确认源码变化，再注入外层panic；核对两共享页源码/Contents流ID恢复、旧快照拒绝、刷新后重试、保存严格重开后的文本与后续C/下一行D位置。原session45816、日志/tmp/rzpdf-compensation-outer-unwind.log，终态待确认，Rust源码冻结。此轮只新增测试，生产版仍对应635项PDF单元通过；不把既有返回所有者unwind等同此次公共编辑链验收，未运行fuzz、未提交。

- 2026-10-08 竖排增强终态及真实字体补偿对照：原session25333退出0，adjusted_text六项通过、0失败/忽略、1.36秒，含独立-1000/800/700与Vertical断言，取代下方待确认状态。新增compare_cjk_advance四页静态Noto Sans SC基线/原第二页/补偿/无补偿样本，最终原session30593退出0（首次示例索引Result编译错误已修正），本库保存/严格重开保持后续P/D/F位置；qpdf 11.2.0、MuPDF 1.27.0独立stext坐标/归一化文本和四页目视通过。check-cjk-advance.sh拒绝旧三页样本，交换对照页的四页负例在坐标检查拒绝；第二页PNG与原图cmp一致。命令/哈希/范围见docs/cjk-creation-validation.md；完整P3仍未完成，未运行fuzz、未提交。

- 2026-10-08 竖排自动补偿合成回归：新增Identity-V/CIDFontType2、显式W2和ToUnicode样本，覆盖长替换/删除/双字节空格、Tc/Tw与负Tz/仿射文本矩阵、后续字符/下一行及严格重开。原session23609退出0，adjusted_text六项通过、0失败/忽略、1.40秒；随后增强独立数值断言（预期TJ为-1000/800/700）与实际WritingMode::Vertical检查，避免仅同一实现前后比对。增强版原session25333、日志/tmp/rzpdf-vertical-text-compensation-verified.log，终态待确认，Rust源码冻结。此轮只新增测试，生产代码仍对应635项PDF单元通过；合成度量不证明真实竖排字形/阅读器视觉，未运行fuzz、未提交。

- 2026-10-08 预算重建专项终态：原session78609退出0，adjusted_text五项通过、0失败/忽略、0.47秒，日志/tmp/rzpdf-adjusted-program-ownership-final.log。新增Tj/两类引号的峰值差1拒绝、邻居保留、双结果合计拒绝、首结果不变、释放后重试和返回所有者unwind归零均通过，取代下条待确认状态。此轮只新增测试，生产版本仍对应全部PDF单元635项通过；没有运行全工作区，不把结果所有者unwind等同整个公共编辑链unwind或完整AR-03。完整P3仍未完成，未运行fuzz、未提交。

- 2026-10-08 自动补偿版PDF单元终态及预算重建补验：原session43804退出0，全部PDF单元635项通过、0失败/忽略、1.80秒，日志/tmp/rzpdf-automatic-text-advance-pdf-unit.log，取代此前待确认状态。新增adjusted_text预算重建专项，覆盖Tj/两类引号峰值差1拒绝保留邻居、双结果共存拒绝/首结果不变、释放后重试、返回所有者unwind归零；首次测试编译原session95693退出101（预算程序须用AsRef访问，不是Deref），已修正测试，不改生产接口。当前原session78609、日志/tmp/rzpdf-adjusted-program-ownership-final.log终态待确认，Rust源码冻结。仅生产源码635项及此前四项专项已通过，不计新增预算测试为通过；未运行fuzz、未提交。

- 2026-10-08 自动推进补偿专项终态：原session66501退出0，adjusted_text四项通过、0失败/忽略、0.52秒，日志/tmp/rzpdf-automatic-text-advance.log。Courier Tj/两类引号、长短/空替换、Tc/Tw、正负Tz、文本与页面仿射、后续C及下一行D位置、共享页/编码失败/外层回滚/旧快照拒绝及重试均通过。当前版本全部PDF单元原session43804、Cargo PID93587、日志/tmp/rzpdf-automatic-text-advance-pdf-unit.log，终态待确认，Rust源码冻结。TJ数组/嵌套、竖排/真实字体、完整unwind/独立渲染仍待补，完整P3未完成，未运行fuzz、未提交。

- 2026-10-08 文本度量版专项终态及自动补偿接入：原session47982退出0，字体状态9项通过；原session90098退出0，全部PDF单元635项通过、0失败/忽略、1.85秒，取代此前待确认状态（日志/tmp/rzpdf-text-metrics-binding-verified.log与/tmp/rzpdf-text-metrics-pdf-unit.log）。随后新增replace_page_text_preserving_advance，按实际字符码宽度/字号/Tc/Tw计算TJ调整；Tj转TJ、单引号保留T*、双引号保留Tw/Tc/T*，字体/编码所有者与重建程序贯通同池。新旧字形遍历不分配字符数组，Tw仅作用于单字节空格，Tz不重复计入；零字号和溢出拒绝。新专项原session66501、日志/tmp/rzpdf-automatic-text-advance.log，终态待确认，Rust源码冻结。635项旧状态版不覆盖新补偿代码；TJ跨项/嵌套Form/独立渲染及完整P3仍待完成，未运行fuzz、未提交。

- 2026-10-08 文本度量跟踪验收进程更新：原session71876编译退出101，测试误把add_page的返回值当作ObjectId；已改为读取实际页面引用，没有放宽接口可见性。当前专项原session47982、Cargo PID90508、日志/tmp/rzpdf-text-metrics-binding-verified.log，以及全部PDF单元原session90098、日志/tmp/rzpdf-text-metrics-pdf-unit.log，终态待确认。Rust源码冻结，不沿用上一构建版633项结果；未运行fuzz、未提交。

- 2026-10-08 TJ构建版PDF单元终态与度量跟踪接入：原session92362退出0，633项通过、0失败/忽略、2.03秒，日志/tmp/rzpdf-adjusted-text-pdf-unit.log，取代下方待确认状态；不覆盖后续字体状态改动。当前BudgetedTextMetrics与已有字体前缀扫描共用，返回同池字体所有者和字号/Tc/Tw/Tz；q/Q保存/恢复标量与字体共享引用，ExtGState Font跟踪字号，双引号在目标绘制前设置Tw/Tc，栈按新增槽位大小计费。新增两项状态/ExtGState/所有权回归；首轮原session84047编译退出101（测试夹具构造器/私有方法），已修正；最终专项原session71876、日志/tmp/rzpdf-text-metrics-binding-final.log，终态待确认。自动推进差、嵌套继承及编辑入口仍待实现，未运行fuzz、未提交。

- 2026-10-08 TJ调整专项终态：原session65546退出0，adjusted_text两项通过、0失败/忽略、0.18秒，日志/tmp/rzpdf-adjusted-text.log，取代下条专项待确认状态。明确正负千分字号单位、字符/词间距及缩放/旋转矩阵下后续位置与严格保存读回；PDF单元原session92362仍运行，Rust源码继续冻结。仅证明显式调整原语，不表示自动推进补偿已实现。

- 2026-10-08 P3推进补偿构建前置：新增ContentBuilder显式TJ调整入口及BudgetedBytes版本；按真实输出容量持有租约，数字用固定栈缓冲格式化，拒绝非有限值和token/输出限额。新增adjusted_text两项专项，验证数值解析/预算所有权/失败原builder不变，以及Courier字符/词间距、Tz、旋转文本矩阵下后续C位置与严格保存读回。专项原session65546、Cargo PID86843，日志/tmp/rzpdf-adjusted-text.log；PDF单元原session92362、Cargo PID87078，日志/tmp/rzpdf-adjusted-text-pdf-unit.log；终态均待确认，Rust源码冻结。仅构建原语，不是自动字体度量或文档编辑补偿，后续状态/所有权方案见docs/content-editing.md。完整P3仍未完成，未运行fuzz、未提交。

- 2026-10-08 P3真实CID文字范围替换：新增edit_cjk_text固定示例，复制第一中文/RGB页，只在副本将世界替换为你好；按字体编码字节计算范围，未映射字符拒绝保持stamp/源码、成功编辑后的外层注入错误回滚、刷新快照重试及原页隔离均通过。最终源码原session45065退出0；qpdf 11.2.0、MuPDF 1.27.0六行文本/共享字体图片与目视通过，前两页PNG与原图cmp一致。check-cjk-page-edit新增--text-edit模式，两种模式互相拒绝错误产物。只覆盖此静态真实字体/单Tj，完整P3及预算仍未完成；未运行fuzz、未提交，详见docs/cjk-creation-validation.md。

- 2026-10-08 P2合并/拆分真实样本：extract_pages/merge_documents示例新增复制后释放源再保存，原session25380/5964均退出0。中文/RGB/RGBA样本逆序拆分两页、两源合并五页已通过本库严格读回、qpdf 11.2.0、MuPDF 1.27.0精确页序/文本和图像结构验证；七页PNG逐一cmp与已目视验收的原页一致，另检查拆分第一页/合并末页。新增固定check-cjk-transfer.sh及负例（错误顺序/页数退出1）；详细命令/哈希见docs/cjk-creation-validation.md。完整P2仍未完成，生产源码未变、未再次跑全工作区、未运行fuzz、未提交。

- 2026-10-08 P2真实字体/图片页面编辑组合：新增edit_page_order示例，逆序跨文档导入、复制、重排为原始页序＋最后页副本；释放源后解释/保存，严格重开三页通过。固定中文/RGB/RGBA样本由qpdf 11.2.0、MuPDF 1.27.0精确六行/页序和三页目视检查验收；三份PNG逐一cmp与原页/副本一致。新增只读check-cjk-page-edit.sh检查页面树顺序、共享嵌入字体/ToUnicode和图片/SMask，错误两页输入退出1。README与验证记录已更新；只完成此矩阵子项，完整P2仍未完成，未运行fuzz、未提交。

- 2026-10-08 P1组合创建样本：create_cjk新增可选--images，与--overlay组合兼容；当前静态Noto Sans SC字体的两页中英文/RGB/RGBA文件已由本库严格读回、qpdf 11.2.0结构检查、MuPDF 1.27.0精确文本和两页目视检查验收。默认命令仍通过原脚本，无图片样本在--images检查下拒绝，重复/未知选项不写输出。覆盖组合另已读回、提取及目视检查。详细命令/哈希/范围见docs/cjk-creation-validation.md，完整P1仍未完成，未运行fuzz、未提交。

- 2026-10-08 StoreEntries公开接口专项终态：原session57226退出0，1项通过、0失败/忽略。覆盖查询/删除标记、临时视图借用与迭代及后续修改，取代下条待确认状态；不宣称标准HashMap全部接口或外部下游兼容。生产引擎源码未变，未再次运行整个工作区。

- 2026-10-08 StoreEntries公开接口补验：新增pdf/tests/store_entries.rs，覆盖查询/容量/删除标记、临时视图返回的借用引用与迭代器、遍历返回原条目引用及后续编辑。`cargo test -p pdf --test store_entries --locked --offline --quiet` 原session57226、Cargo PID80283运行中，终态待确认，不计作通过。生产源码未改变；同步对象表条目的实现状态，未运行fuzz、未提交新改动，test.png保持不动。

- 2026-10-08 对象表预算工作区验收终态：原session1967退出0，`cargo test --workspace --locked --offline --quiet` 汇总46组、938项通过、0失败、0忽略，日志 `/tmp/rzpdf-table-budget-full-workspace.log`。验收期间Rust源码保持冻结，此结果取代下方本轮待确认状态；工作对象表已接入真实Layout计费与事务作用域，完整AR-03及整个TODO仍未完成。按用户要求准备本地提交，不推送、不纳入test.png、未运行fuzz。

- 2026-10-08 对象表预算当前单元终态：原session7874退出0，全部PDF单元633项通过、0失败/忽略、1.76秒，日志 `/tmp/rzpdf-table-budget-pdf-unit.log`，包含实际编辑/字体/保存预算及对象表/事务新增回归。当前开始工作区`cargo test --workspace --locked --offline --quiet`，日志 `/tmp/rzpdf-table-budget-full-workspace.log`，终态待确认，Rust源码冻结至终态。完整AR-03及整个TODO仍未完成，未运行fuzz、未提交，不套用旧提交933项结果。

- 2026-10-08 AR-03工作对象表接入专项：DocumentStore迁移hashbrown真实Layout分配器，entries改只读StoreEntries视图（迁移说明见docs/architecture）；预算事务激活同池表作用域，普通内层继承预算并用数组快照，异池在快照前拒绝，回滚/快照销毁后最外层转持久。对象表专项4项、原事务5项、扩容三入口事务6项和最新嵌套/unwind事务7项均退出0，最新原session39734日志 `/tmp/rzpdf-transaction-table-unwind.log`。插入/更新缺失ID/删除缺失ID峰值少1拒绝、拒绝原表不变、回滚对象号恢复、足额重试、内层回滚保留外层额度与外层panic邻居额度恢复已覆盖。当前启动全部PDF单元，日志 `/tmp/rzpdf-table-budget-pdf-unit.log`，终态待确认；尚未全工作区复验，完整AR-03未完成，不沿用旧提交933项结果，未运行fuzz、未提交。

- 2026-10-08 AR-03表分配器原型：新增内部TableAllocator，按Global收到的真实Layout在分配前预留；两个内联登记槽覆盖单表扩容旧新共存，不另分配登记HashMap，释放实际存储后归还租约。hashbrown真实表专项原session9309退出0，1项通过；锁文件恢复四个无关平台依赖，仅保留allocator-api2 0.2.21及pdf直接使用既有hashbrown 0.16.1的变更。锁定/离线预算组原session47948退出0，日志 `/tmp/rzpdf-table-budget-locked.log`。当前仅原型，未接入DocumentStore、事务作用域和持久化转移，内联控制结构/Rc及载荷不由此计费；完整AR-03保持未完成，未运行fuzz、未提交。

- 2026-10-08 AR-03预算输入所有权补充验收：新增底层读取回归，额度预留失败不调用Reader，部分读取错误/panic释放临时缓冲额度且保留邻居结果，成功重试返回所有者持有容量直至释放。专项原session29695退出0，1项通过、628项筛选，日志 `/tmp/rzpdf-budget-input-failure.log`。现有StreamReader普通错误不回退源位置，而SyntaxParser.read_stream_block普通错误尝试恢复解析位置；未将panic清理等同位置回滚。未改变生产逻辑、未运行fuzz、未提交；933项全量对应前一提交，不覆盖本新增测试，完整AR-03仍未完成。

- 2026-10-08 当前提交全量验收终态：提交53a9011的Rust源码在验收期间保持不变；原session39247退出0，`cargo test --workspace --locked --quiet`日志 `/tmp/rzpdf-coons-retained-domain-full-workspace.log` 汇总46组、933项通过、0失败、0忽略，包含628项PDF单元、8项着色流程与38项render（53.40秒）。此结果取代下方本轮待确认状态；提交已按用户要求完成，未推送、未纳入test.png、未运行fuzz。仅证明当前回归基线，完整Type6、操作级聚合预算及整个TODO仍未完成。

- 2026-10-08 区间映射版render终态/新全量启动：原session3626退出0，render38项通过、0失败/忽略，54.68秒，日志 `/tmp/rzpdf-coons-retained-domain-render.log`，覆盖当前三根分割线修订及精确边界整页像素回归。此前模型87项、流程8项也退出0；完整Type6及整个TODO仍未完成。差异检查通过后启动`cargo test --workspace --locked --quiet`，原session39247、Cargo PID742，日志 `/tmp/rzpdf-coons-retained-domain-full-workspace.log`，最终退出码待确认。Rust源码冻结至终态，不套用927项旧版结果，不重启等待超时的进程；未运行fuzz、未提交。

- 2026-10-08 区间映射版流程终态及架构同步：原session73555退出0，shading_workflow8项通过，日志 `/tmp/rzpdf-coons-retained-domain-workflow.log`，当前三根分割线修订的导入/两保存严格重开无回归。docs/architecture补候选域全根保留、首轮较宽域单轴覆盖、区间仿射递归与证书/额度不变量，明确固定样例不等于完整折叠支持。render全量原session3626仍运行，Cargo98106/测试98785存活，日志 `/tmp/rzpdf-coons-retained-domain-render.log`，已有38项测试进度但无终态；未重启/修改Rust源码，当前全工作区仍未复验，未运行fuzz、未提交。

- 2026-10-08 三根分割线修订模型终态：原session87069退出0，shading87项通过、541项筛选，日志 `/tmp/rzpdf-coons-retained-domain-mapping.log`；首轮较宽候选域供单轴遍历、区间仿射映射保留误差后，三根v=0.6875完整覆盖和既有折叠/排序回归通过。取代前两版1/3项失败状态，但不表示任意折叠、连续根及整个Type6交付完成。当前流程与render全量已重测，日志 `/tmp/rzpdf-coons-retained-domain-workflow.log`、`/tmp/rzpdf-coons-retained-domain-render.log`，终态待确认；全工作区尚未复验。未运行fuzz、未提交。

- 2026-10-08 保留候选域/单轴遍历修订：未唯一认证时将收缩域保留给细分，按较宽参数轴分成覆盖所有存活根的两个子域，递减深度并共享工作额度。初版session92509退出101，84项通过、3失败，日志 `/tmp/rzpdf-coons-retained-domain-model.log`，过窄域影响既有折叠认证；未宣称修复。进一步保留首轮较宽存活域供遍历，并携带区间仿射原点/尺度，而非把浮点端点外包围当作精确映射，避免深层参数映射丢失舍入误差；修订session87069运行中，日志 `/tmp/rzpdf-coons-retained-domain-mapping.log`。三根分割线与既有折叠回归需全部通过才能继续验收；未运行fuzz、未提交。

- 2026-10-08 render全量及多根分割线新缺口：原session24620退出0，render38项通过，日志 `/tmp/rzpdf-coons-contraction-render.log`，55.14秒。随后扩展三根折叠回归至v=0.6875并断言全域收缩不误判唯一；原session63291退出101，86通过、1失败，日志 `/tmp/rzpdf-coons-contraction-fold-model.log`，完整遍历返回Unresolved。现有收缩在没有唯一证书时丢弃已收缩候选域，继续对原域细分，仍会把多根置于划分边界。下一步需将未解决但保留全根的收缩域交还遍历，并明确参数映射区间、递归深度/工作预算及全域去重，不能仅更换split常数或删除此回归。当前工作区有已知失败模型测试；已通过的38项render不代表完整Type6。未运行fuzz、未提交。

- 2026-10-08 分割线生产采样原子性复验：补ColoredPatch.sample的完整额度前缀失败输出保持与足额恢复，验证无细分时分割线参数包围及颜色精度；格式整理shading_patch和shading_workflow（未格式化整个render文件）。原session7305退出0，shading87项通过，日志 `/tmp/rzpdf-coons-contraction-atomic-model.log`；原session59180退出0，流程8项通过，日志 `/tmp/rzpdf-coons-contraction-workflow.log`。render全量原session24620运行中，Cargo PID91855、测试PID92661存活，日志 `/tmp/rzpdf-coons-contraction-render.log`；尚无终态，不宣称全render/工作区通过。未运行fuzz、未提交。

- 2026-10-08 候选域收缩模型专项终态：原session38303退出0，shading87项通过、541项筛选，日志 `/tmp/rzpdf-coons-contraction-margin-model.log`，含新增分割线根无细分唯一认证、全部额度前缀耗尽、域外无根及退化连续根不误判。取代下方该专项运行状态；当前流程/全量及更广曲面兼容性仍需复验，完整Type6保持未完成。未运行fuzz、未提交。

- 2026-10-08 分割线像素修复终态：带1/16包围余量的候选域收缩修订，原session54795退出0，精确边界整页像素专项1项通过、37项筛选，日志 `/tmp/rzpdf-coons-contraction-margin-pixels.log`，54.55秒。完整50×50着色域预检及实际渲染、上下边灰度64/16、两种保存严格重开整页像素一致性通过；取代此前该样例已知失败状态，不表示任意折叠/一般边界完整交付。新增分割线无细分认证、全部额度前缀、域外无根及退化连续根拒绝模型专项原session38303运行中，日志 `/tmp/rzpdf-coons-contraction-margin-model.log`。性能仍需改进，当前修订全量及流程复验未完成；未运行fuzz、未提交。

- 2026-10-08 全根候选域收缩初版：完整搜索在子域细分前尝试最多8轮Krawczyk交集，交集保留全部候选根，仅受限域严格证书成功才返回；计入共享工作额度。原session69900退出0，shading86项通过，日志 `/tmp/rzpdf-coons-candidate-contraction-model.log`；像素session34845退出101，仍失败pixel(0,16)/shading(4,1056)，日志 `/tmp/rzpdf-coons-candidate-contraction-pixels.log`。修订为交集前扩大Krawczyk包围各侧当前域宽的1/16，保留严格存在认证所需余量；扩大只保留根而不充当存在容差。修订像素原session54795待终态，日志 `/tmp/rzpdf-coons-contraction-margin-pixels.log`；当前仍不宣称修复完成，未运行fuzz、未提交。

- 2026-10-08 精确像素失败定位：新增整页50×50着色坐标预检诊断，原session64723退出101，日志 `/tmp/rzpdf-coons-boundary-location.log`；首失败pixel(0,16)映射到shading(4,1056)，原参数v=1056/1536=0.6875，属于内部子域分割线，不是外边界。此前“优先修边界存在/唯一性”的诊断不足以解释本例，应先实现全根候选域的Krawczyk交集收缩：每轮交集保留原域全部根，受限域认证成功后才可返回，空交集证明无根；需计费、错误原子性和分割线回归。保留完整画面测试和失败诊断，不更换split常数绕过；未运行fuzz、未提交。

- 2026-10-08 新精确边界像素暴露覆盖缺口：新增非均匀Coons的精确dyadic矩阵，使上下边像素中心对应u=1/4，预期直接/平方灰度64/16并严格保存后整页像素一致。原session79940退出101，日志 `/tmp/rzpdf-coons-boundary-precision-pixels.log`，渲染display时遇到其他采样点的Unresolved，尚未到颜色断言；保留失败回归，不缩窄BBox绕过。下一步需将已有边界存在证明与全域至多一个根的收缩证明结合：严格Krawczyk内部包含是充分存在条件，已有原域边根见证时不应重复要求延拓内部存在，但必须仍证明原域唯一性并保留根包围和额度。当前工作区含已知失败测试，不宣称像素/完整Type6通过；未运行fuzz、未提交。

- 2026-10-08 精确整数Decode及非端点边界：恒等`[0,2^bits-1]`/反向映射在1..32位原始整数上直接返回精确区间，避免先除后乘引入可消除的舍入包围；非恒等相邻范围仍保留不确定区间。新增全位宽/端点/内部整数及反向回归，原session61452退出0，shading相关86项通过，日志 `/tmp/rzpdf-coons-exact-decode-shading.log`。流程矩阵加入1536内部整数边界及65535端点两尺度，原session27367退出0，8项通过，日志 `/tmp/rzpdf-coons-exact-decode-workflow.log`，真实函数/镜像/导入/两保存严格重开通过。此前内部整数边界无法认证的样例已覆盖，不表示一般非恒等Decode、任意方向或弯曲边界完成；当前新增解码修订仍需像素及全量复验。未运行fuzz、未提交。

- 2026-10-08 非均匀直边流程修订与像素终态：原session52735退出0，shading_workflow 8项通过，日志 `/tmp/rzpdf-coons-nonuniform-workflow-recheck.log`；新增样例覆盖直接/PDF平方函数×四镜像×两边，原文档、导入及两保存策略严格重开颜色64/16正确。原session1317退出0，既有Skia像素专项1项通过、36项筛选，日志 `/tmp/rzpdf-coons-precision-pixels.log`，12组合和整页保存读回像素无回归，不是新增精确边界像素验收。取代本轮等待状态；当前精度源码仍未全量复验，非端点解码不确定边界、一般边界与折叠仍未完成。未运行fuzz、未提交。

- 2026-10-08 精度修订整体与新流程：原session72808退出0，shading相关85项通过，日志 `/tmp/rzpdf-coons-edge-precision-shading.log`。新增16位非均匀边界的直接/PDF平方函数×四镜像、导入和两保存策略严格重开测试；首次session92350退出101，7项通过、1项失败，非Decode端点的常边坐标产生非零解码区间而无法认证。样例改用0/65535端点保持精确常坐标，并从编码控制点的精确Bernstein权重计算u=1/4目标；修订重测日志 `/tmp/rzpdf-coons-nonuniform-workflow-recheck.log`，终态待确认。不放宽区间证书，非端点不确定边界仍属未完成范围；未运行fuzz、未提交。

- 2026-10-08 非均匀直边镜像/额度失败契约：新增水平、垂直、双镜像及两边共8组合；测得实际工作量后逐一耗尽所有额度前缀，均返回WorkLimit且分量/包围输出保持原值，恰好足额重试成功；直接/平方灰度64/16及精确参数包围通过。原session72968退出0，nonuniform专项2项通过、624项筛选，日志 `/tmp/rzpdf-coons-edge-precision-budget.log`；此终态取代该专项镜像/耗尽待验证状态，不代表实际PDF函数、导入/保存或像素验收。当前精度修订仍需shading整体及流程/像素复验，不能沿用前版927项；未运行fuzz、未提交。

- 2026-10-08 非均匀直边精度缺陷修复：新增上下边`x(u)=3u+(3/8)u(1-u)`回归，精确u=1/4；原session38502退出101，直接灰度实际62而非64，日志 `/tmp/rzpdf-coons-edge-precision.log`。直边延拓唯一根改用既有`tighten_unique_cell`八轮、共享工作额度细化；原session36551退出0，专项1项通过，直接/平方灰度64/16且参数包围通过，日志 `/tmp/rzpdf-coons-edge-precision-recheck.log`。本修订不在前版927项全量范围内；镜像、细化耗尽不改输出/重试及完整复验仍待完成，任务不勾选。未运行fuzz、未提交。

- 2026-10-08 当前直边修订全量终态：原session19703退出0，`cargo test --workspace --locked --quiet` 共46组、927项通过、0失败、0忽略，含624项PDF单元和37项render测试，日志 `/tmp/rzpdf-coons-edge-full-workspace.log`。验收期间Rust源码冻结，取代下方本轮等待状态；结果覆盖镜像/不确定性修订，不涵盖之后的精度修改。完整Type6及整体TODO仍未完成；未运行fuzz、未自动提交。

- 2026-10-08 当前直边修订验收：像素专项原session36993退出0，日志 `/tmp/rzpdf-coons-edge-pixels.log`，既有Type4/5/6的12组合及两种保存策略整页像素一致性复验通过；不代表精确边界像素或独立阅读器验收。随后启动全工作区 `cargo test --workspace --locked --quiet`，原session19703、Cargo PID43287，日志 `/tmp/rzpdf-coons-edge-full-workspace.log`，终态待确认；Rust源码冻结至该轮终态，不沿用角点版925项结果。未运行fuzz、未自动提交。

- 2026-10-08 直边方向与不确定性专项：提交fb4ff54后新增水平/垂直镜像四边参数方向验证，以及边界控制区间仅包含目标而非精确常坐标时拒绝认证；修正根排序文档注释归属。`cargo test -p pdf --lib straight_edge_certificate --locked --quiet` 原session90190退出0，2项通过、622项筛选，日志 `/tmp/rzpdf-coons-edge-direction.log`。仅覆盖轴向直边证书，任意方向/弯曲边界及折叠仍未完成；本轮修改未提交、未运行fuzz，当前版本全量与像素复验仍待完成。

- 2026-10-08 直边流程断言确认：原session35593退出0，shading_workflow 7项通过，日志 `/tmp/rzpdf-coons-edge-roundtrip.log`；Type6矩阵四边非角点新增直接/平方函数颜色断言，覆盖原文档、导入及两个文档Preserve/FlateUnencoded严格重开。证据限定固定轴向矩形，任意方向/弯曲边界与折叠仍未完成；当前直边修订像素和全量复验仍待验证，未自动提交、未运行fuzz。

- 2026-10-08 直边证书生产接入确认：角点分支之后尝试常坐标直边IVT存在及全域唯一根认证，失败继续完整搜索，不将失败当域外。矩形四边中点现在验证正确颜色和包围区间。shading专项原session34705退出0，82项通过、541项筛选，日志 `/tmp/rzpdf-coons-production-edges.log`；当前直边修订尚需流程/像素/全量复验，不能沿用前版925项结果。任意方向直边、弯曲边界、连续根及折叠仍未完成，未自动提交、未运行fuzz。

- 2026-10-08 直边非角点证书基础：边界一个坐标的四控制区间均精确等于目标，另一坐标端点严格夹住目标时，连续性证明原域边上存在根；再用包含整个原域的延拓唯一性证书排除竞争根，固定边参数并与原域相交保留根区间。专项session74483退出0，1项通过、622项筛选，日志 `/tmp/rzpdf-coons-straight-edge.log`，覆盖四边非角点、邻近域外及零工作额度。尚未生产接通，不代表弯曲边界或折叠完整交付；未自动提交、未运行fuzz。

- 2026-10-08 角点修订版全量终态：原session43908最终退出0，`cargo test --workspace --locked --quiet` 共46组结果、925项通过、0失败、0忽略，含622项PDF单元和37项render测试，日志 `/tmp/rzpdf-coons-corner-full-workspace.log`；验收期间Rust源码冻结，结果取代下方该轮等待状态，不与ba4ac29旧版920项混用。唯一角点证书与四角导入/保存断言修订无全量回归；一般边界、连续根、任意折叠、Type7/Pattern及整个TODO仍未完成。未自动提交或推送、未运行fuzz。

- 2026-10-08 角点修订全工作区复验启动：差异检查通过后启动`cargo test --workspace --locked --quiet`，session43908、Cargo PID97927，日志 `/tmp/rzpdf-coons-corner-full-workspace.log`；ps确认原Cargo及rustc子进程存活。本轮验收为ba4ac29之后的角点证书/边界区间及流程断言修订，冻结Rust源码直到取得终态，不套用旧session5708的920项结果。完整TODO仍未完成，未自动提交、未运行fuzz。

- 2026-10-08 格式修订专项确认：原session3545退出0，shading相关81项通过、541项筛选，日志 `/tmp/rzpdf-coons-formatted-corners.log`；仅格式整理后复验，生产角点能力和未完成边界范围不变。新增修订仍未工作区全量复验，未自动提交、未运行fuzz。

- 2026-10-08 角点修订工作区编译确认：格式整理后`cargo check --workspace --locked --quiet` 原session28171退出0，日志 `/tmp/rzpdf-coons-corner-workspace-check.log`；git diff --check通过。这是编译门禁而非全量测试；完整边界需将一般边界参数候选的存在证明与覆盖整个原域的根枚举/唯一性相结合，Newton近似不能单独作为根证书。未自动提交、未运行fuzz。

- 2026-10-08 角点修订像素复验：原session60537退出0，既有Skia网格专项1项通过、36项筛选，日志 `/tmp/rzpdf-coons-corners-pixels.log`；单测试含Type4/5/6共12组合及两保存策略整页像素不变。证明角点修订未破坏该固定矩形像素矩阵，不是新增边界像素或独立阅读器验收；当前新增修改尚未全工作区复验，未自动提交、未运行fuzz。

- 2026-10-08 角点导入/保存断言确认：修订shading_workflow原session80412退出0，7项通过，日志 `/tmp/rzpdf-coons-corner-roundtrip.log`；Type6矩阵新增四角颜色断言，直接灰度/平方函数均覆盖原文档、导入及两个文档的Preserve/FlateUnencoded严格重开。取代前条“尚未加入独立角点保存断言”状态；固定矩形角点证据不证明一般边界、折叠或独立阅读器完整验收，未自动提交、未运行fuzz。

- 2026-10-08 角点修订流程复验：`cargo test -p pdf --test shading_workflow --locked --quiet` 原session85788退出0，7项通过，日志 `/tmp/rzpdf-coons-corners-workflow.log`；既有Type4/5/6导入、保存策略严格重开和注册失败恢复流程无回归。该矩阵尚未加入独立角点保存断言，不能据此认定角点完整流程验收；像素及当前修订全量仍待验证，未自动提交、未运行fuzz。

- 2026-10-08 唯一角点生产接入：非恒定补丁先尝试精确角点见证及全原域延拓唯一性，成功才使用精确角参数插值，认证不成功继续既有完整搜索，不当域外。矩形四角采样回归改为成功、一般四边中点仍明确Unresolved且输出不变。shading专项session12356退出0，81项通过、541项筛选，日志 `/tmp/rzpdf-coons-production-corners.log`；取代下方“尚未接通”的该角点分支状态。仅通过证书的角点可用，不是一般边界/折叠完整交付；仍需流程、像素及全量复验，未自动提交、未运行fuzz。

- 2026-10-08 唯一角点组合证书：精确参数见证与覆盖整个原域的延拓唯一根证书组合，成功时返回精确角参数；不认证时返回候选未解决而非域外。首次session47917因测试assert_eq要求Interval实现PartialEq而编译失败，改为匹配错误后专项session66347退出0，1项通过、621项筛选，日志 `/tmp/rzpdf-coons-unique-corner-recheck.log`；四角、连续根退化拒绝、非角点与零工作额度验证通过。尚未接通生产采样，完整边界/折叠/排序仍待实现，未自动提交、未运行fuzz。

- 2026-10-08 精确角点存在见证及边界区间修订：首次专项session71504退出101，暴露张量转换在边界产生多余抵消区间；修订为四边直接保留原边界控制点区间，不放宽断言。修订Coons模块/适配器专项session8793退出0，58项通过、563项筛选，日志 `/tmp/rzpdf-coons-boundary-identity.log`，含角点精确见证、邻近/不确定点拒绝、延拓和既有搜索回归。见证仅证明存在，仍需唯一性、域归属和全覆盖排序；未接通生产边界路径，新增修改未全工作区复验、未自动提交、未运行fuzz。

- 2026-10-08 非直边延拓验证：专项session94399退出0，2项通过、618项筛选，日志 `/tmp/rzpdf-coons-curved-continuation.log`；新增非直边曲面81个二进制精确参数点的区间包围检查，含四边和角点，不用epsilon放宽断言。该采样回归验证重参数化实现，不证明任意曲面边界根存在或全域覆盖；生产边界路径仍未接通，完整Type6保持未完成。

- 2026-10-08 延拓基础回归确认：`cargo test -p pdf --lib color::shading_patch --locked --quiet` 原session69894退出0，56项通过、563项筛选，日志 `/tmp/rzpdf-coons-continuation-regression.log`；包含新增边界失败契约及延拓认证检查，git diff --check通过。此为Coons模块/资源适配器专项，不代替新增修改后的工作区全量验收；完整Type6仍未完成。

- 2026-10-08 边界认证基础：抽取有限矩形上的区间Bernstein重参数化，允许多项式延拓；原restrict入口仍拒绝原单位域外参数，生产采样覆盖判定不变。专项session88613退出0，1项通过、618项筛选，日志 `/tmp/rzpdf-coons-continuation-tests.log`，验证矩形角根在延拓域内可严格认证，同时原域外根也可认证，故延拓证书不能替代原域成员证明。需继续边界存在证明、重复根去重与全覆盖排序；完整Type6仍未完成，当前新增修改尚未全量复验。未运行fuzz、未自动提交。

- 2026-10-08 Type6边界缺口回归：新增矩形四边中点及四角的采样行为检查，当前严格内部根认证明确返回Unresolved，保留输出值/区间不变，并验证随后内部点仍可成功采样。专项session4171退出0，1项通过、617项筛选，日志 `/tmp/rzpdf-coons-boundary-contract.log`；这仅固定当前失败契约，不是边界支持验收。下一步仍需证明边界根存在及完整覆盖/排序，不能靠放宽严格包含或误报域外绕过。未运行fuzz、未自动提交。

- 2026-10-08 Type6部分接入全量门禁确认：原session5708最终退出0，`cargo test --workspace --locked --quiet` 日志 `/tmp/rzpdf-coons-workspace-tests.log` 共46组结果、920项通过、0失败、0忽略，含617项PDF单元、37项render测试及集成/文档目标。验收源码对应提交ba4ac29；本记录取代下方该轮“仍待终态”状态。完整Type6边界根、连续根、任意折叠与独立阅读器验收仍未完成，相关TODO不勾选；未运行fuzz。本轮仅补文档，未自动提交或推送。

- 2026-10-08 全量过程结果：同一工作区门禁session5708日志已确认617项PDF单元全部通过（0失败/忽略），日志 `/tmp/rzpdf-coons-workspace-tests.log`；原Cargo仍运行，集成/render/doc目标及最终退出码未返回，不将此阶段结果当全工作区通过。本轮未改Rust源码、未重启、未提交、未运行fuzz，整体目标保持未完成。

- 2026-10-08 测试文档同步与全量观察：docs/testing补函数域76项、注册恢复7项与Skia12组合单项终态证据，明确固定矩形不证明完整Type6。全工作区session5708同句柄轮询仍运行，ps确认Cargo PID40699及其rustc集成目标编译子进程存活，尚无终态；本轮未改Rust源码、未重启测试，git diff --check通过。保持当前门禁观察，不把编译警告/存活当测试通过；未提交、未运行fuzz。

- 2026-10-08 Type6像素专项确认：session76506本轮最终轮询退出0，1项Skia测试通过（内含Type4/5/6直接/非线性×正常/镜像共12组合及两保存策略整页像素相同），日志 `/tmp/rzpdf-coons-pixel-tests.log`；取代该专项等待状态。该固定矩形/BBox证据不证明边界根、任意曲面或独立阅读器一致；全工作区session5708仍待终态。

- 2026-10-08 当前工作区全量门禁启动：冻结本轮Rust源码，启动 `cargo test --workspace --locked --quiet` session5708，Cargo PID40699本轮ps确认存活，日志 `/tmp/rzpdf-coons-workspace-tests.log`，尚未计通过。像素专项session76506同句柄轮询仍运行，日志已进入running 1 test（该测试内含12组合），不是终态；保持原进程观察，不重启。该新门禁与此前退出码缺失的旧三角网格门禁不同，须分别记账。git diff --check通过，未提交、未运行fuzz，完整目标未完成。

- 2026-10-08 函数域修订专项确认：session59546本轮最后轮询退出0，76项shading相关单元通过（541 filtered），日志 `/tmp/rzpdf-coons-function-domain-tests.log`，取代该专项等待状态；像素专项session76506仍待终态，不宣称全量通过。

- 2026-10-08 验证等待：本轮未修改Rust源码；函数域专项session59546与像素专项session76506经原句柄多次轮询仍运行，ps同时确认Cargo及各自PDF/render测试子进程存活。日志仍为编译警告尾部，尚无测试终态，不据此报通过或重启；git diff --check通过。完整Type6与整体目标保持未完成，未提交、未运行fuzz。

- 2026-10-08 Type6像素矩阵扩展：现有Skia三角网格像素测试加入Type6直接灰度/平方函数×正常/镜像四组合，保留Type4/5八组合，共12组合，验证明确像素中心值、BBox外空白，以及Preserve/FlateUnencoded严格重开后整页像素一致。启动render专项session76506，日志 `/tmp/rzpdf-coons-pixel-tests.log`，尚未计通过；函数域修订模型session59546仍待确认。注册恢复专项session81879退出0、7项集成通过，日志 `/tmp/rzpdf-coons-workflow-recovery-tests.log`；工作区编译session71418退出0，日志 `/tmp/rzpdf-coons-workspace-check.log`，不等于工作区全量测试。git diff --check通过；边界/连续根、独立阅读器、完整渲染验收未完成，未提交、未运行fuzz。

- 2026-10-08 Type6函数域拒绝补充：模型回归新增Function Domain [0.2,0.8]不能包含Decode参数[0,1]时构造返回ColorError，避免采样时悄然裁剪替代原参数语义。该新断言尚未执行；启动工作区编译 `cargo check --workspace --locked --quiet` session71418，日志 `/tmp/rzpdf-coons-workspace-check.log`，本轮同句柄轮询仍运行，注册修复专项session81879亦仍运行，不重启、不计通过。git diff --check通过，未提交、未运行fuzz，完整计划保持未完成。

- 2026-10-08 Type6流程专项确认：session71037退出0，6项shading_workflow集成通过，日志 `/tmp/rzpdf-coons-workflow-tests.log`，包含Type4/5保留组合与Type6直接/平方函数资源注册、显示来源、导入及两保存策略严格重开。新增注册失败恢复未包含在该启动版本中，已启动修订专项session81879，日志 `/tmp/rzpdf-coons-workflow-recovery-tests.log`，本轮同句柄轮询仍运行、尚未计通过。docs/testing同步定向证据/未完成边界，未宣称全量或完整Type6交付；未提交、未运行fuzz。

- 2026-10-08 Type6注册失败恢复回归：新增截断flag0记录拒绝注册、revision及保存字节不变，修复为完整记录后同一对象可注册并生成无诊断显示事件，恒定端点补丁点采样为白色。此新测试尚未执行；既有资源/导入/两保存策略专项session71037本轮同句柄轮询仍运行，保持观察，不将新增源码算进旧专项范围。预算修订模型session47741退出0、76项通过（541 filtered），日志 `/tmp/rzpdf-coons-sample-budget-tests.log`。git diff --check通过，完整Type6边界/连续根和独立像素验收仍未完成，未提交、未运行fuzz。

- 2026-10-08 Type6公共流程回归扩展：现有Type4/5资源工作流加入Type6直接灰度/平方函数矩阵，验证注册、显示来源、三个内部点、域外、跨文档导入，以及原始/导入文档的Preserve与FlateUnencoded严格保存重开；保留Type4/5所有既有组合。启动 `cargo test -p pdf --test shading_workflow --locked --quiet` session71037，日志 `/tmp/rzpdf-coons-workflow-tests.log`，尚未计通过；预算模型专项session47741本轮同句柄轮询仍运行，不重启。git diff --check通过；此流程样本不替代边界/折叠/页面像素或独立阅读器验收，P4仍未完成，未提交、未运行fuzz。

- 2026-10-08 生产接入模型专项确认：session59003退出0，76项shading相关单元通过（541 filtered），日志 `/tmp/rzpdf-coons-production-tests.log`；不覆盖随后新增采样预算断言。预算修订编译检查日志无error，命令链session95702退出0、diff检查通过；启动修订版同范围测试session47741，日志 `/tmp/rzpdf-coons-sample-budget-tests.log`，尚未计通过，不宣称全量或完整Type6验收。

- 2026-10-08 Type6采样临时预算：Shading记录构造时max_decoded_bytes，采样从已有保留载荷扣剩余额度，分配前检查分量/区间缓冲请求总量，每次reserve后检查实际容量；补无剩余内存/零工作额度拒绝及恢复后可采样断言。编译检查session95702运行中，日志 `/tmp/rzpdf-coons-sample-budget-check.log`；原模型专项session59003同句柄轮询仍运行，仅覆盖启动时版本，不将后加预算断言视为已通过。仍不包括函数求值输出/栈/allocator元数据或整页OperationBudget，不宣称完整AR-03；未提交、未运行fuzz。

- 2026-10-08 Type6生产模型接入：去掉patch模块cfg(test)，Shading新增CoonsMesh持有入口、Function可选及参数Domain包含校验、实际容量记账与逐点工作上限，先认证/插值参数再求函数及ColorSpace；未解决几何/顺序返回UnsupportedFeature，额度耗尽ResourceLimit，不误判域外。writer不支持诊断样本改Type7。`cargo check -p pdf --locked --quiet` session19650退出0，日志 `/tmp/rzpdf-coons-production-check.log`；新增矩形直接灰度/平方函数、域外及非流拒绝模型测试，`cargo test -p pdf --lib color::shading --locked --quiet` session59003仍运行，日志 `/tmp/rzpdf-coons-production-tests.log`，未计通过。之前资源保留专项session35216退出0、54项通过（562 filtered）。README注明部分支持；边界/连续根、逐点缓冲聚合预算、完整绘制/导入保存/独立阅读器验收仍待实现，P4-03不勾选。未提交、未运行fuzz。

- 2026-10-08 细分错误传播专项确认：session31628退出0，53项独立patch测试通过，日志 `/tmp/rzpdf-coons-fallible-split.log`，取代该专项运行状态；资源保留Cargo专项仍待终态，不宣称当前工作区全量通过。

- 2026-10-08 Type6搜索错误传播：首次偏移细分不再expect曲面限制成功，使用固定四块缓冲并向外传播InvalidInput，保持原分区/遍历顺序和工作额度语义。独立rustc专项session31628运行中，日志 `/tmp/rzpdf-coons-fallible-split.log`；资源保留专项session35216经本轮同句柄轮询仍运行，日志 `/tmp/rzpdf-coons-retained-adapter.log`，两者均未计通过，不因启动延迟重启。git diff --check通过，生产Type6/完整边界与连续根验收仍未完成，未提交、未运行fuzz。

- 2026-10-08 Type6资源保留预算接入：资源适配在完整流解析后使用PatchMesh实际容量统计，与已有载荷checked聚合并检查max_decoded_bytes；补容量有值、克隆数据及预算拒绝后独立重试回归。启动 `cargo test -p pdf --lib color::shading_patch --locked --quiet`，session35216本轮轮询仍运行，日志 `/tmp/rzpdf-coons-retained-adapter.log`，尚未计通过；git diff --check通过。后续继续观察同一句柄，不因启动延迟重跑。仍是cfg(test)范围资源适配，非生产Shading接入或全局预分配预算完成；未提交、未运行fuzz。

- 2026-10-08 Type6生产接入前保留载荷统计：PatchMesh按patch容器与各角点分量/区间Vec实际capacity进行checked统计，内嵌几何不重复计费；补Debug/Clone供Shading持有。新增空余容量、clear保留容量及释放后归零回归。独立rustc专项session66388退出0，53项patch测试通过，日志 `/tmp/rzpdf-coons-retained-tests.log`，git diff --check通过。此专项不包含资源适配/Cargo全量；模块仍cfg(test)，生产接入、完整OperationBudget和Type6交付未完成。本轮未提交、未运行fuzz。

- 2026-10-08 提交检查点：资源适配修正版专项 session1356 退出0，53项测试通过、562项被筛选，日志 `/tmp/rzpdf-coons-resource-adapter-recheck.log`；git diff --check通过。本次提交包含Type6几何、认证搜索、流解析/采样基础及测试范围内资源适配，仍以cfg(test)注册，未接入生产Shading/渲染，不宣称Type6完整支持或当前工作区全量通过。不包含来源未确认的 `test.png`，不推送远端、不运行 fuzz。

- 2026-10-08 资源适配编译修正：session3909退出101，PdfArray不支持下标访问；已改为get并传播维度错误，重启修正版专项，日志 `/tmp/rzpdf-coons-resource-adapter-recheck.log`，尚未计通过。此前运行状态由此取代，不把编译失败当测试通过。

- 2026-10-08 Type6资源字典适配：新增测试范围内的资源适配模块，校验ShadingType/整数位宽/Decode维度与有限数值，经原数值lexeme解析f64 Decode，防止先转f32损失大坐标；解析分量Decode容量、扣已有Shading载荷及解码输入Vec余量后调用持有型流解析，区分结构错误与资源限额。新增连续记录、非整数位宽/错维度及已有载荷超预算回归。启动当前 `cargo test -p pdf --lib color::shading_patch --locked --quiet`，session3909运行中，日志 `/tmp/rzpdf-coons-resource-adapter.log`，尚未计通过。仍为cfg(test)注册，未开放Type6；旧流解码器先分配后聚合检查、Decode文本到f64舍入等边界未视为完整AR-03/数学实数契约。git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 网格采样专项确认：session86907退出0、52项独立patch测试通过，日志 `/tmp/rzpdf-coons-mesh-sampling.log`，取代此前专项运行状态；Type6对外资源/显示与整体验收未完成。

- 2026-10-08 Type6网格覆盖采样：新增网格逆序采样，后块命中覆盖前块、证明域外才能回退，后块未解决/超额度直接返回错误而非跳过；单patch包括恒定分支也扣采样工作额度，统一检查深度/目标有限值。新增后块覆盖、后块域外回退、整体域外/超额度保留输出、后块退化未解决不能选前块颜色回归；专项session86907运行中，日志 `/tmp/rzpdf-coons-mesh-sampling.log`，尚未计通过。仍是已编码分量/参数采样，不包含函数/ColorSpace、完整边界/连续根或Type6资源/绘制接入，完整OperationBudget未贯通。git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 单patch采样专项确认：session18837退出0、51项独立patch测试通过，日志 `/tmp/rzpdf-coons-patch-sampling.log`，取代此前运行状态；完整Type6网格/资源/绘制未完成。

- 2026-10-08 Type6单patch采样串联：新增调用者缓冲的已编码分量/函数参数采样，恒定退化处理或全部根认证→证明覆盖顺序→角点值/区间插值；固定64根栈存储，达到上限显式WorkLimit，不用截断根集选色，未解决几何/顺序继续报错。仅精确单点Decode区间可恢复恒定身份，非零解码误差不凭浮点相等恢复。新增仿射参数0.25、区间包含、域外/工作限额保留输出及精确端点Decode退化采样回归；专项session18837运行中，日志 `/tmp/rzpdf-coons-patch-sampling.log`，尚未计通过。堆无隐藏分配不代表栈/全局OperationBudget已计费；网格覆盖顺序、边界/连续根、函数/ColorSpace与Type6资源/显示仍待接入。git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 角点区间专项确认：session30557退出0、50项独立patch测试通过，日志 `/tmp/rzpdf-coons-corner-intervals.log`，取代此前专项运行状态；当前Cargo全量/Type6功能验收仍未完成。

- 2026-10-08 Type6认证参数→角点区间：常规Cargo session25805退出0，注册时47项patch单元通过（562 filtered），日志 `/tmp/rzpdf-coons-cargo-tests.log`；不包含随后新增单元，不宣称当前全量。新增认证u/v区间与分量误差的四角参数空间插值，输入预检后写调用者区间缓冲，不转RGB/不提前求函数；新增0.625再平方0.390625顺序、参数矩形9点包含及错误维度不改输出回归。独立专项session30557运行中，日志 `/tmp/rzpdf-coons-corner-intervals.log`，尚未计通过。非线性函数/ColorSpace区间处理、根边界/连续根和Type6资源/显示仍待实现，git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 完整流矩阵专项确认：session86064退出0、49项独立patch测试通过，日志 `/tmp/rzpdf-coons-stream-matrix.log`，取代此前专项运行状态；常规Cargo注册专项仍待终态，Type6对外功能未完成。

- 2026-10-08 Type6完整流矩阵：新增8位记录的共享标志1/2/3全部几何/分量继承、flag0重新开始独立块，以及首块成功后第二块Decode失败整流拒绝、修复编码后重试成功回归。独立专项session86064运行中，日志 `/tmp/rzpdf-coons-stream-matrix.log`，尚未计通过。常规Cargo session25805本轮确认仍活跃，不重启；该先前启动目标可能只覆盖注册时47项，后加两项由此次独立专项明确验证，不混为当前完整门禁。git diff --check通过，未提交、未运行 fuzz；Type6资源/绘制接入与完整预算仍待完成。

- 2026-10-08 Type6常规单元注册：color模块以cfg(test)注册shading_patch基础模块，后续可由cargo测试统一执行，不再只依赖独立rustc。启动 `cargo test -p pdf --lib color::shading_patch --locked --quiet`，session25805运行中，日志 `/tmp/rzpdf-coons-cargo-tests.log`，尚未计通过。仅测试注册，正常构建仍不暴露Type6资源/显示支持，完整功能接入仍待完成。原全量进程已确认终止但退出码缺失，不与此次新增模块范围混为全量门禁。git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 持有型流专项确认：session87199退出0、47项独立patch测试通过，日志 `/tmp/rzpdf-coons-owned-stream.log`，取代此前专项运行状态；完整Type6资源/显示/预算验收未完成。

- 2026-10-08 Type6持有型流容器：新增预扫描后的完整流Decode容器，持有全部patch几何、四角分量和误差区间，共享两角数据直接复制上一块，单scratch复用；分配前以记录数/实际结构sizeof/分量规模预检输入长度与存储总量，try_reserve_exact后计实际Vec容量。新增完整两记录/共享分量区间、边界额度与计数拒绝回归；专项session87199运行中，日志 `/tmp/rzpdf-coons-owned-stream.log`，尚未计通过。仅计借用输入长度及可见Vec容量，不包括allocator元数据/栈/编码配置或全局OperationBudget，不宣称AR-03完成。Type6资源/求值/显示仍未接入。

- 2026-10-08 原工作区全量进程终态观察：ps确认Cargo PID90238及其子进程已不存在，`/tmp/rzpdf-workspace-mesh-current.log`末尾为0项doc目标结果，失败关键词检查无结果。原session7123观察句柄已丢失，未取得进程退出码；不据日志末尾单独宣称门禁退出0。该运行不覆盖未注册的patch新模块。未重启同一任务、未提交、未运行 fuzz。

- 2026-10-08 解码记录专项确认：session64595退出0、46项独立patch测试通过，日志 `/tmp/rzpdf-coons-decoded-record.log`，取代此前运行状态；完整流容器/预算/Type6资源与绘制仍待实现。

- 2026-10-08 Type6解码记录装配：新增借用编码配置与单记录原始位读取→几何Decode/共享继承→角点Decode串联，无隐藏堆分配，全部成功才提交流游标/返回patch；工作scratch明确可在失败时改变，解码输出和游标保持原状。新增连续新块/共享块、缺前块拒绝、角点f32失败保留输出/游标回归；专项session64595运行中，日志 `/tmp/rzpdf-coons-decoded-record.log`，尚未计通过。仍须持有共享角点/区间的完整流容器、预算/资源字典与显示接入；Type6尚未支持。全量Cargo PID90238本轮确认活跃，日志已出现render 37项通过（55.22s），仍不宣称整个workspace终态；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 流预扫描专项确认：session31420退出0、45项独立patch测试通过，日志 `/tmp/rzpdf-coons-stream-scan.log`，取代此前专项运行状态；尾位独立验证及完整Type6流/引擎接入未完成。

- 2026-10-08 Type6流结构预扫描：新增无分配连续记录预扫描，校验编码/首条共享关系、完整记录位数、计数上限和checked位置；输出记录数/有效位数/最终不足一字节尾位，完整额外字节不得当任意填充。新增连续两记录、计数拒绝、截断/空流/无前patch/非法位宽与完整尾字节拒绝回归；专项session31420运行中，日志 `/tmp/rzpdf-coons-stream-scan.log`，尚未计通过。最终partial-byte尾位契约仍须独立文件兼容验证；预扫描不替代Decode/预算/装配或Type6引擎接入。全量Cargo PID90238本轮确认活跃，日志新增9项通过目标；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 角点Decode专项确认：session25587退出0、44项独立patch测试通过，日志 `/tmp/rzpdf-coons-corner-decode.log`，取代此前专项运行状态；Type6完整流/预算/显示未完成。

- 2026-10-08 Type6角点Decode缓冲：新增2/4个新编码角点的分量Decode至调用者f32/区间缓冲，整批预检尺寸/位宽/原始范围/f32有限性后才写入；不分配、不转RGB，区间同时包围精确解码实数与存储f32舍入值。新增多分量/反向Decode、舍入区间包含、末项非法输入不部分写出、角点数量及f32溢出拒绝回归；专项session25587运行中，日志 `/tmp/rzpdf-coons-corner-decode.log`，尚未计通过。共享分量区间所有权/预算、完整流装配与Type6引擎接入仍待实现。全量Cargo PID90238本轮确认活跃，日志新增5项通过目标；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 原始身份专项确认：session44291退出0、42项通过；session99309退出0、43项通过，日志分别 `/tmp/rzpdf-coons-encoded-identity.log`、`/tmp/rzpdf-coons-identity-collision.log`，取代此前运行状态；预算/完整流与Type6显示仍待完成。

- 2026-10-08 Type6原始控制身份：保存原始12点整数、坐标位宽和Decode，共享记录保留旧四点原始值并复制新八点；不允许共享记录改变同一流的编码配置。带Decode误差的同v证明改用原始整数身份与非恒定Decode，浮点相等不再替代身份；直接浮点尾部延续不保留原始身份。新增真实原始值不同却浮点相等的拒绝证明、位宽/Decode变化拒绝和共享原始值顺序回归。初始专项session44291及新增碰撞专项session99309继续观察，日志 `/tmp/rzpdf-coons-encoded-identity.log`、`/tmp/rzpdf-coons-identity-collision.log`，尚未计新专项通过。结构增长仍须未来预算按实际sizeof计费，Type6未接入。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 共享新点Decode专项确认：session63384退出0、42项独立patch测试通过，日志 `/tmp/rzpdf-coons-decoded-continuation.log`，取代此前专项运行状态；完整Type6流/引擎接入未完成。

- 2026-10-08 Type6共享新点Decode：新增共享记录8个原始新控制点的Decode延续构造，继承旧四点坐标/区间并保存新八点解码区间，先验证标志和所有解码输入，不修改旧patch。新增标志1/2/3全部继承误差一致、新点内值误差保留、非法标志/超位宽失败与旧patch不变回归；专项session63384运行中，日志 `/tmp/rzpdf-coons-decoded-continuation.log`，尚未计通过。原始控制身份/同编码契约、角点Decode/预算/尾填充和流接入仍待贯通，不宣称Type6整体支持。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 解码几何专项确认：session40090退出0、41项独立patch测试通过，日志 `/tmp/rzpdf-coons-decoded-geometry.log`，取代此前运行状态；共享新点Decode、原始身份与流/引擎集成仍待完成。

- 2026-10-08 Type6解码几何误差贯通：CoonsPatch新增可选12点坐标区间及整数Decode构造，转换控制网继承原Decode误差，共享边延续保留旧四点区间；直接f64构造维持精确输入语义。对带Decode误差几何禁用仅凭浮点相等的恒定/同v结构快捷证明，避免舍入碰撞误报身份；仍需原始控制身份贯通。新增Decode区间包含、转换与共享区间原样保留、超位宽拒绝回归；专项session40090运行中，日志 `/tmp/rzpdf-coons-decoded-geometry.log`，尚未计通过。结构存储增加，未来网格预算须按实际sizeof/容量计入，尚未接入流/引擎。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Decode标量专项确认：session3366退出0、40项独立patch测试通过，日志 `/tmp/rzpdf-coons-decode-scalar.log`，取代此前运行状态；完整Type6解析/绘制仍未完成。

- 2026-10-08 Type6 Decode映射：新增原始整数→加权端点f64值与精确实数区间的映射，支持1..32位、反向Decode、精确端点、超位宽原始值/非法范围拒绝；内点用端点加权避免先计算max-min溢出，区间纳入整数比例除法与每步舍入。新增32位相邻高端坐标、反向范围、极端有限端点及非法输入回归；专项session3366运行中，日志 `/tmp/rzpdf-coons-decode-scalar.log`，尚未计通过。仍须把解码区间贯通到patch控制网，并完成分量缓冲/共享记录/尾填充与预算接入；Type6未接入引擎。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 原始记录专项确认：session97178退出0、39项独立patch测试通过，日志 `/tmp/rzpdf-coons-raw-record.log`，取代此前运行状态；完整Decode/装配/Type6引擎集成仍未完成。

- 2026-10-08 Type6整条原始记录读取：位读取session60839退出0、38项通过。新增非消费标志预读和控制点→全部角点分量的整数记录读取，先检查完整记录位数/输出尺寸，失败不消费游标、不写调用者缓冲；固定30位新patch+20位共享patch串联及截断/尺寸失败回归。初编译预读副本的可变借用表达式错误已改为局部cursor后重跑；专项session97178运行中，日志 `/tmp/rzpdf-coons-raw-record.log`，尚未计通过。仍待Decode映射、共享记录装配、尾填充及预算接入，不宣称Type6解析完成。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6连续位读取：核对PDFium原生Type6读取路径及MeshStream，patch坐标/颜色读取无逐patch ByteAlign，三角顶点路径则显式对齐。新增连续MSB位读取器，1..32位、低两位标志、检查算术、失败不消费输入；新增30位新patch紧接20位共享patch、32位极值、保留位/尾位与失败游标回归。专项session60839运行中，日志 `/tmp/rzpdf-coons-bit-reader.log`，尚未计通过。依据 https://pdfium.googlesource.com/pdfium/+/refs/heads/main/core/fpdfapi/render/cpdf_rendershading.cpp 及 https://pdfium.googlesource.com/pdfium/+/refs/heads/main/core/fpdfapi/page/cpdf_meshstream.cpp 。仍待记录解码/尾填充契约、预算和Type6集成，不把读取器当完整解析。全量Cargo PID90238及shading_workflow子进程本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 流布局专项确认：session9262退出0、37项独立patch测试通过，日志 `/tmp/rzpdf-coons-record-layout.log`，取代前述专项运行状态；位流解析和Type6集成未完成。

- 2026-10-08 Type6流记录布局：共享分量session92184退出0、36项通过。核对PDF Reference Table4.32及Type6说明，新增新patch12点/4角、共享patch8点/2角的位数布局检查，坐标/分量/标志位宽与维度校验、全程checked算术；不在此假设字节补齐方式。新增灰度/RGB、子字节记录、非法位宽/标志/空维度与usize溢出回归；专项session9262运行中，日志 `/tmp/rzpdf-coons-record-layout.log`，尚未计通过。布局不等于位流解码，流顺序、补齐、解码误差/预算与引擎接入仍待完成。规范来源 https://chromium.googlesource.com/external/skia/+/7983cf450ea4da39f8a9bd2a27d0412fb6a2fad0/experimental/PdfViewer/PdfReference-okular-1.txt 。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6共享角点分量：新增与共享边标志1/2/3相同角点索引的颜色/函数参数继承接口，返回旧两角点与新两角点的借用切片，不分配、不提前转换RGB；检查非空一致维度与有限分量。新增全部标志顺序、指针身份（不复制）、非法标志/空/NaN拒绝回归；专项session92184运行中，日志 `/tmp/rzpdf-coons-shared-components.log`，尚未计通过。流解码接入时仍需预算复制/所有权保存，借用接口不替代完整生命周期或流解析。全量Cargo PID90238本轮确认活跃，日志新增6项通过目标；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 恒定退化专项确认：session16239退出0，35项独立patch测试通过，日志 `/tmp/rzpdf-coons-constant-coverage.log`，取代此前专项运行状态；一般连续根与Type6接入仍未完成。

- 2026-10-08 Type6恒定退化曲面：新增全部12控制点精确重合的结构判断，曲面命中恒定点时连续根按最大v再最大u选择[1,1]，不命中则无覆盖；非恒定曲面不进入此分支，非法目标拒绝。新增相邻浮点点不命中、改变一个控制点后取消身份及[1,1]对应PDF角点2分量回归；专项session16239运行中，日志 `/tmp/rzpdf-coons-constant-coverage.log`，尚未计通过。这只处理点状退化连续根，不代替线状/一般连续根、单位域边缘或任意内部边界认证；尚未接入引擎。全量Cargo PID90238本轮确认活跃，日志新增14项通过目标；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 同v结构专项确认：session20576退出0、34项独立patch测试通过，日志 `/tmp/rzpdf-coons-common-v.log`；固定折叠的全部根认证与规范最大u选择串联通过，其他未完成边界不变，取代前述专项运行状态。

- 2026-10-08 Type6同v结构证明：从原12控制点的精确身份判断任一坐标是否仅为v的严格单调三次曲线；同一目标点的认证根因此具有相同v，再以分离u区间选最大u。未满足身份时仍走原区间v/u证明，不使用epsilon相等。新增固定折叠全根认证→最大u选择串联、破坏边界坐标身份后不能证明同v回归；专项session20576运行中，日志 `/tmp/rzpdf-coons-common-v.log`，尚未计通过。仅对结构可证明的同v情形成立，不代替任意同v、边界或连续根处理，Type6未接入引擎。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 中点边界专项确认：session79149退出0，33项独立patch测试通过，日志 `/tmp/rzpdf-coons-offset-split.log`；固定中点折叠三根可认证，其他边界/连续根仍未完成，取代前述专项运行状态。

- 2026-10-08 Type6收紧确认与中点边界回归：session37355退出0、32项通过，修正后的收紧/统一工作额度/折叠根宽度断言均通过。认证遍历首层改在3/8切分，随后继续精确二分，避免常见u/v=1/2根始终位于子边界；新增原三参数折叠样本认证回归。专项session79149运行中，日志 `/tmp/rzpdf-coons-offset-split.log`，尚未计通过。此仅处理特定中点边界，不取代任意内部边界、单位域边缘、奇异/连续根与同v证明，Type6仍未接入。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6收紧贯通与失败修正：session70391退出101（30通过/1失败），首次贯通session56860退出101（30通过/2失败），暴露重复严格存在性认证阻碍极窄根区间继续收紧；未降低收紧断言。提取Krawczyk包围与认证分类，已有唯一根证明下用包围映射交集保留根，不要求每轮重新严格内部包含；遍历接入8轮收紧，搜索/认证统一工作额度，并新增耗尽前不发出根回归及折叠根区间宽度<1e-6断言。修正专项session37355运行中，日志 `/tmp/rzpdf-coons-tightening-image.log`，尚未计通过。仍待专项终态与边界/同v/连续根处理，不宣称Type6接入；全量Cargo PID90238本轮确认活跃，git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6认证根收紧：任意矩形限制session23854退出0、30项通过。新增唯一根初始认证→区间限制→重新认证→交集的收紧循环，每轮扣工作额度、最多32轮；不能继续收紧保留原认证区间，额度耗尽显式报错，不返回部分精化。新增弯曲限制块的区间不扩张/收紧及零额度回归；专项session70391运行中，日志 `/tmp/rzpdf-coons-root-tightening.log`，尚未计通过。仍须收紧与遍历/根排序贯通及同v、边界和连续根处理，Type6未接入引擎。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6任意参数矩形限制：参数区间求值session1368退出0、29项通过。新增以三次blossom构造任意单位域矩形的双三次限制控制网，继承区间误差并逐步向外舍入，完整域限制保持输入控制区间原样。新增弯曲25组局部→全局参数对照、完整域身份和倒置矩形拒绝回归；专项session23854运行中，日志 `/tmp/rzpdf-coons-restriction.log`，尚未计通过。用于后续认证根收紧/非固定中点边界处理，尚未完成该链路或Type6引擎集成。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6参数矩形区间求值：区间de Casteljau扩展为不确定u/v矩形求值，单点接口委托相同实现，保留严格端点身份与每步向外舍入。新增弯曲参数矩形9组点覆盖、倒置区间拒绝和仿射认证根区间映射包含目标回归；专项session1368运行中，日志 `/tmp/rzpdf-coons-parameter-box.log`，尚未计通过。为根收紧/区间结果验证提供基础，不以目标被区间包含代替根存在证明。全量Cargo PID90238本轮确认活跃，模块未注册，git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 认证顺序专项确认：session76145退出0、28项独立测试通过，日志 `/tmp/rzpdf-coons-certified-precedence.log`，取代此前运行状态。证明不足的根区间顺序继续返回未解决，不宣称Type6整体完成。

- 2026-10-08 Type6认证根区间顺序：新增带比较额度的认证根选择，只有v区间严格分离、或证明v为相等单点且u顺序可证明时选赢家；区间重叠不按中点/epsilon强行排序，整体返回Unresolved。新增可证明v顺序、精确同v的u顺序、重叠未解决、零额度与空输入回归；专项session76145运行中，日志 `/tmp/rzpdf-coons-certified-precedence.log`，尚未计通过。后续仍需根区间收紧、同v结构证明及边界/连续根支持，Type6尚未接入；全量Cargo PID90238本轮确认活跃，git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 自适应认证专项确认：session39206退出0、26项通过；session10069退出0、27项通过，日志分别 `/tmp/rzpdf-coons-root-certificate.log`、`/tmp/rzpdf-coons-adaptive-certification.log`。固定非边界折叠样本三个根区间全部认证，边界/连续根仍未完成，Type6未接入引擎；前述专项运行状态由此取代。

- 2026-10-08 Type6自适应认证遍历：新增区间范围排除→局部Krawczyk认证→未解决细分的完整兄弟区域遍历，认证根区间向外转换到全局参数；逐节点预算、深度限20、回调错误传播，达到限额/深度仍未解决时整体报错，暂定根不得当成功完整结果。新增非边界折叠三根区间分离、退化未解决及零额度回归；专项session10069运行中，日志 `/tmp/rzpdf-coons-adaptive-certification.log`，尚未计通过。局部认证session39206继续观察；边界/奇异连续根及覆盖顺序仍待处理，Type6尚未接入引擎。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 区间求值专项确认：session4914退出0、25项通过，日志 `/tmp/rzpdf-coons-interval-evaluation.log`；局部根认证session39206仍待终态，不计入通过数。

- 2026-10-08 Type6局部区间根认证：依据Rump对Krawczyk存在性/唯一性验证的说明（https://www.tuhh.de/ti3/rump/Research_Rump/topics.shtml），新增本地单位方块的Krawczyk包围，区间残差/导数和矩阵运算向外舍入；严格内部包含且收缩上界<1才报告唯一根，包围与方块不交可排除，否则未解决。新增仿射内部根包含、外部无根、边界根及退化曲面未解决回归；专项session39206运行中，日志 `/tmp/rzpdf-coons-root-certificate.log`，尚未计通过。区间求值session4914继续观察，不重启。仍须自适应认证遍历、边界/连续根与覆盖次序处理，不宣称Type6支持。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6区间求值：新增本地单位参数的区间de Casteljau曲面求值，控制点不确定性、1-t和每步乘加全部向外舍入；端点直接保留控制区间，避免不必要0×无限扩张。新增弯曲曲面25组前向值包含、不确定角点端点原样保留、非法参数拒绝回归；专项session4914运行中，日志 `/tmp/rzpdf-coons-interval-evaluation.log`，尚未计通过。为根存在性/唯一性检查提供区间残差，尚未实现完整检查或引擎集成。全量Cargo PID90238本轮确认活跃，日志新增6项通过目标；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 区间候选专项确认：session95198退出0，24项独立patch测试通过，日志 `/tmp/rzpdf-coons-interval-search.log`，取代此前运行状态；不证明叶候选根认证或Type6完整接入。

- 2026-10-08 Type6区间候选搜索贯通：新增区间控制网候选遍历，转换→细分→范围排除全部保留误差，不需手工padding；回调携带子区间网/全局参数矩形，逐节点扣额度、限制深度、传播回调未解决错误。新增不使用padding的折叠三个参数区域保留、域外单节点排除和回调错误传播回归；专项session95198运行中，日志 `/tmp/rzpdf-coons-interval-search.log`，尚未计通过。叶候选仍不是认证根，须继续唯一性/存在性和连续根处理，不绕过该契约直接显示。全量Cargo PID90238本轮确认活跃，日志新增8项通过目标；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 区间链专项确认：session21075退出0、22项通过，session45033退出0、23项通过，日志分别 `/tmp/rzpdf-coons-interval-subdivision.log`、`/tmp/rzpdf-coons-interval-hulls.log`；此前运行状态由此取代，仍不声明完整根认证或Type6引擎支持。

- 2026-10-08 Type6区间范围/导数：新增带不确定控制点的凸包范围和区间导数包围，贯通转换/细分产生的误差；明确细分块导数对应本地单位参数，不能直接当全局u/v导数。新增半块范围及1.5导数精确值、宽输入不确定性保留、倒置区间拒绝回归；专项session45033运行中，日志 `/tmp/rzpdf-coons-interval-hulls.log`，尚未计通过。细分session21075继续观察，不重启；全量Cargo PID90238本轮确认活跃。根认证与实际流/显示仍待实现，git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6区间细分：转换误差session50436退出0、21项通过。新增区间控制网de Casteljau四分细分，逐步向外舍入、继承输入区间，不丢弃转换误差；先拒绝NaN/倒置区间，保守无限范围允许继续传递。新增两级细分精确矩形控制值包含、共享边区间一致与非法区间拒绝回归；专项session21075运行中，日志 `/tmp/rzpdf-coons-interval-subdivision.log`，尚未计通过。仍须区间范围/导数/根认证贯通，流解码误差与引擎显示尚未接入。全量Cargo PID90238本轮确认活跃；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6转换误差区间：新增向外舍入的加/减/乘区间原语，以及从已存f64点转换到双三次控制网的逐步误差包围，包含1/3、2/3有理系数；无效无限运算保守回退全实数范围。新增矩形精确整数控制网、弯曲浮点控制值包含及0×无限回退回归；专项session50436运行中，日志 `/tmp/rzpdf-coons-conversion-intervals.log`，尚未计通过。该转换不覆盖原始流解码误差，尚未接到细分/导数/根认证链路；不宣称完整数值认证。全量Cargo PID90238本轮确认活跃，日志新增12项通过目标；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 未解决候选专项确认：session15328退出0，20项独立patch测试通过，日志 `/tmp/rzpdf-coons-unresolved.log`；前一记录运行状态由此更新，Type6集成与完整根认证仍未完成。

- 2026-10-08 Type6未解决候选契约：SearchError新增Unresolved，回归退化候选细化失败必须向外返回未解决而非成功空结果；新增细分后折叠外侧正向、内侧反向雅可比区间排除零回归。专项session15328运行中，日志 `/tmp/rzpdf-coons-unresolved.log`，尚未计通过。全量Cargo PID90238及子进程metadata-e937ad7b8b25d73e本轮确认活跃，日志继续新增通过目标，不重启；模块未接入，未提交、未运行 fuzz。完整认证仍须处理误差传播及奇异/连续根，不把这些局部检查当Type6整体验收。

- 2026-10-08 Type6雅可比范围：导数专项session28142退出0、17项通过。新增du_x*dv_y-du_y*dv_x区间包围，乘法与减法向外舍入，导数无限界或无效无限运算退回全实数范围，不误报排除奇异点。新增矩形正定、折叠变号/采样覆盖、退化零值及极端溢出回归；雅可比session32859退出0、18项通过，日志 `/tmp/rzpdf-coons-jacobian.log`。行列式区间不含零只说明局部非奇异，不证明全局单射或候选根唯一；区间求逆认证与此前构造误差仍待实现。全量Cargo PID90238本轮确认活跃，模块未接入引擎；git diff --check通过，未提交、未运行 fuzz。

- 2026-10-08 Type6导数凸包：候选错误传播session18523退出0、16项通过。新增双三次控制网的du/dv、x/y导数范围，差分和乘3分别向外舍入，固定数组，不把控制网构造误差包含在此范围声明中。新增矩形精确导数、弯曲/折叠25组解析导数覆盖、导数变号与NaN拒绝回归；专项session28142运行中，日志 `/tmp/rzpdf-coons-derivative-hulls.log`，尚未计通过。后续仍需区间求逆/唯一性认证、误差传播与奇异连续根处理，不把导数凸包视为完整认证。全量Cargo PID90238本轮确认活跃；git diff --check通过。未提交、未运行 fuzz。

- 2026-10-08 Type6候选回调错误传播：候选遍历回调改为返回Result，细化额度错误可通过?终止遍历，不再需要测试回调unwrap；新增回调首个候选失败后不访问后续候选、遍历自身预算仍有剩余回归。专项session18523运行中，日志 `/tmp/rzpdf-coons-fallible-search.log`，尚未计通过；git diff --check通过。已产生的回调输出仍为暂定结果，只有完整成功返回才可继续处理；未解决/奇异候选和数值认证仍待实现。全量Cargo PID90238本轮确认活跃，模块仍未注册，未提交、未运行 fuzz。

- 2026-10-08 参数优先级专项确认：session5606退出0，15项独立patch测试通过，日志 `/tmp/rzpdf-coons-precedence.log`；前一记录的运行状态已由此终态取代，Type6对外支持仍未完成。

- 2026-10-08 Type6参数根优先级：重新核对Adobe提供的ISO32000-2曲面规则（https://developer.adobe.com/document-services/docs/assets/5b15559b96303194340b99820d3a70fa/PDF_ISO_32000-2.pdf），新增已提供根的最大v、相同v最大u选择；不使用epsilon合并近邻v，先验证所有根有限且在单位域，非法根整体拒绝。新增顺序无关、重复/空根、相邻浮点v及非法根回归。独立专项session5606运行中，日志 `/tmp/rzpdf-coons-precedence.log`，尚未计通过；git diff --check通过。选择器不证明输入根有效/完整，仍不得绕过未解决候选直接用于显示。全量Cargo PID90238本轮确认活跃；未提交、未运行 fuzz。

- 2026-10-08 Type6折叠搜索/细化串联回归：局部细化session18437退出0、13项通过。新增三个独立参数根的精确参数/残差验证及候选遍历→局部细化串联，固定折叠样本三个u根全部找回；串联专项退出0、14项通过，日志 `/tmp/rzpdf-coons-search-refinement.log`。另验证退化常量曲面可返回一个残差合格参数但不证明无限根完整性。此为特定样本证据，不是任意曲面全部根认证；覆盖顺序、误差传播、奇异根/连续根处理和Type6流/显示接入仍未完成。全量Cargo PID90238本轮确认活跃；git diff --check通过。未提交、未运行 fuzz。

- 2026-10-08 Type6局部根细化：候选专项日志显示12项通过。新增候选矩形约束的Newton细化与阻尼，每次迭代/试步扣工作额度，最多32轮与每轮16试步；仅接受坐标残差达标的参数点，奇异/未收敛返回未解决，绝不等同无覆盖。初测受限区域100额度不足，正确返回WorkLimit；测试改为1024充分额度后重跑session18437，日志 `/tmp/rzpdf-coons-refinement-recheck.log`，尚未计通过，保留零额度拒绝回归。仍未证明根唯一/全部覆盖，折叠根验证及规范顺序选择、误差传播和引擎集成待实现。全量Cargo PID90238本轮确认活跃；未提交、未运行 fuzz。

- 2026-10-08 Type6候选参数搜索：范围专项session38279退出0，11项通过。新增基于控制网范围/四分细分的全部候选参数矩形遍历，节点逐项扣除调用者额度，深度限制20，不分配搜索堆缓冲；显式区分非法输入和工作额度耗尽，回调结果在成功返回前不得作为完整结果。新增折叠三个u根全部保留、域外单节点排除、零额度/非法容差回归；专项session28470启动，日志 `/tmp/rzpdf-coons-candidates.log`，尚未计通过。候选不是已验证根，误差padding由调用方提供，完整数值误差/根验证/最大v再最大u选择和流集成仍待实现。全量Cargo PID90238本轮仍活跃；未提交、未运行 fuzz。

- 2026-10-08 Type6控制网范围：新增16点控制凸包AABB并向外舍入一个可表示值，允许极端有限控制点产生无限外界，拒绝非有限输入。测试弯曲内部超出角点AABB、四个细分块100组采样覆盖、零/极大值边界；独立专项session38279运行中，日志 `/tmp/rzpdf-coons-bounds.log`，尚未计通过。该范围只针对当前控制网，转换/多级细分误差仍须独立计入，不作为已完成求逆的证明。全量Cargo PID90238再次确认活跃；git diff --check通过。未接入引擎、未提交、未运行 fuzz。

- 2026-10-08 精确细分专项确认：session92524退出0，10项独立几何测试通过，日志 `/tmp/rzpdf-coons-subdivision.log`；仅验证未注册的patch基础模块，不声明Type6显示支持或全量完成。

- 2026-10-08 Type6精确细分基础：新增双三次控制网的de Casteljau中点四分细分，固定数组、不分配堆缓冲，子块按低v/低u、低v/高u、高v/低u、高v/高u排序。新增64组子参数到父参数对照、共享边逐控制点一致、大数中点避免加法溢出及NaN拒绝回归。独立rustc专项session92524仍运行，日志 `/tmp/rzpdf-coons-subdivision.log`，尚未计通过；git diff --check通过。全量Cargo PID90238仍活跃，日志新增4项通过目标但未终态。不代替求逆/多根选择或Type6集成；未提交、未运行 fuzz。

- 2026-10-08 Type6张量表示基础：Coons几何新增精确双三次Bernstein控制网（按[v][u]索引），供后续凸包范围排除和细分求逆使用，不以四角范围代替折叠内部范围。弯曲与折叠组合曲面25组参数与原Coons前向结果对照，独立rustc专项退出0，9项通过，日志 `/tmp/rzpdf-coons-tensor.log`。测试初次构造类型错误已修正后重跑；模块仍未接入引擎，尚未实现细分、逆映射、多根选择或Type6流解码。全量Cargo PID90238本轮确认仍活跃；不声称全量终态。此次改动未提交、未运行 fuzz。

- 2026-10-08 提交检查点：按用户要求整理 Type4/5 网格 Shading、测试与文档，以及尚未注册到引擎的 Type6 几何基础。四角插值独立测试日志 `/tmp/rzpdf-coons-colors.log` 显示8项通过；网格注册集成6项、八组合像素专项1项通过。全量测试 Cargo PID90238 仍在运行，不宣称全量通过；Type6尚未接入，整体计划保持未完成。不包含来源未确认的 `test.png`，不推送远端、不运行 fuzz。

- 2026-10-08 Type6折叠前向几何确认：session28090退出0，7项独立几何通过，日志 `/tmp/rzpdf-coons-folds.log`，包含三参数映到同一点及弯曲内部精确值；不证明逆映射/多根覆盖选择已实现，新四角颜色插值另待专项。

- 2026-10-08 Type6四角分量插值：独立patch模块新增不分配缓冲区的参数空间四角插值，验证单位域、维度和有限值后才写出，供后续颜色转换/函数求值使用。新增RGB精确值[0.25,0.625,0.25]、单参数0.625→平方0.390625顺序，以及域外/错误维度/NaN保留原缓冲回归；session64780运行中，日志 `/tmp/rzpdf-coons-colors.log`，尚未计通过。折叠session28090复核仍活跃，未取终态；全量Cargo PID90238仍活跃，继续日志观察，不重启。patch模块仍未接入workspace/流解码/折叠求逆/显示，不宣称Type6完整支持；未提交、未运行 fuzz。

- 2026-10-08 Type6导数确认与折叠回归：session10504退出0，5项独立几何测试通过，日志 `/tmp/rzpdf-coons-derivatives.log`。新增同一位置对应三个u参数的折叠曲面，检查导数方向反转；新增弯曲内部精确值[1.5,2.625]，明确不以角点双线性几何替代Coons曲面。独立session28090运行中，日志 `/tmp/rzpdf-coons-folds.log`，尚未计通过；只是前向几何验证，未实现逆映射多根选择/绘制。全量原session7123句柄观察返回Unknown process id，但ps确认Cargo PID90238仍活跃，日志继续新增通过目标；继续以PID/日志观察，不重启、不宣称终态。新模块仍未注册，现有接入源码不变；未提交、未运行 fuzz。

- 2026-10-08 Type6共享边与导数：独立几何 session99367退出0、3项通过；随后共享边独立测试退出0、4项通过，日志 `/tmp/rzpdf-coons-shared-edge.log`，确认标志1/2/3继承控制点及角点索引、各曲线方向连续和非法标志/新点拒绝。已核对PDF Reference Table4.32原文。新增Coons解析u/v导数及矩形/弯曲曲面有限差分对照，独立session10504运行中，日志 `/tmp/rzpdf-coons-derivatives.log`，尚未计通过；导数不是逆映射/折叠多根求解。模块仍未注册到工作区，既有接入源码指纹不变，不宣称Type6对外支持；未提交、未运行 fuzz。

- 2026-10-08 Type6纯几何核心：新增尚未注册到color模块的 shading_patch.rs，按PDF12控制点顺序构造Coons参数映射（两组边界线性混合减去角点双线性混合），de Casteljau求四条曲线，提供PDF角点顺序的参数颜色权重与有限/单位域检查。新增矩形、四条弯曲边界方向与非法参数3项独立单元，rustc --test session 99367 运行中，日志 `/tmp/rzpdf-coons-geometry.log`，尚未计通过。该新文件未纳入正在运行的工作区编译，已有接入源码指纹不变，不声称全量覆盖新模块；Type6流解码、折叠/重复映射覆盖、显示与完整预算仍未实现。规范公式依据：https://chromium.googlesource.com/external/skia/+/7983cf450ea4da39f8a9bd2a27d0412fb6a2fad0/experimental/PdfViewer/PdfReference-okular-1.txt 。未提交、未运行 fuzz，P4/整体目标保持未完成。

- 2026-10-08 Flate 本库视觉对照确认：session 90428 退出0，十四页Flate严格读入/Skia渲染完成，第9–14页最新PNG按 PDF 技能目视检查；与同目录Preserve对应六页RGBA逐像素相同（200×120）。保存策略内一致性不消除MuPDF/Skia非线性色阶差异，不泛化为复杂网格验收。README校正Type1/4/5接入、DeviceN的受限Type4求值，以及Type4构造已编译程序的旧描述；保持未完成范围，源码未改，全量7123仍运行。未提交、未运行 fuzz。

- 2026-10-08 Flate 网格独立显示：session 35498 退出0，十四页 Flate 样本生成成功。MuPDF渲染第9–14页无诊断，按 PDF 技能检查六个最新PNG，与Preserve对应页面六页RGB逐像素完全相同（均200×120）。证据 `tmp/pdfs/shading-lattice.TcXO6V/flate.pdf` 与 `flate-*.png`，固定样本不代替复杂类型矩阵。Skia Flate session 90428 已启动，日志 `/tmp/rzpdf-shading-lattice-flate-skia.log`，尚未终态；工作区7123已开始执行测试，跟踪源码指纹仍一致。未提交、未运行 fuzz。

- 2026-10-08 格点行宽专项与Flate独立样本：session 73623 退出0，13个网格单元通过（549 filtered），包含解码前最低格点规模额度拒绝，日志 `/tmp/rzpdf-shading-lattice-row-budget.log`。使用现有示例启动十四页 Flate 样本生成 session 35498，输出 `tmp/pdfs/shading-lattice.TcXO6V/flate.pdf`、日志 `/tmp/rzpdf-shading-lattice-flate.log`；按 PDF 技能准备生成后独立渲染与Preserve网格页面对照，尚未生成/渲染终态，不计通过。当前源码汇总仍 `97fa1fe7f2048249d7b5db89f1fb55360984da04042f06679c4d74396a9e725f`，全量 session 7123 继续活跃，源码未改；未提交、未运行 fuzz。

- 2026-10-08 网格验收文档归并：session 65853 退出0，6项Shading集成通过，残缺格点注册不改变修订号/保存字节及补齐后恢复通过。docs/testing.md 新增 Type4/5 检查点，归并模型/流程/八组合像素、独立样本差异和未完成边界；架构补 Type5 接入说明，未把 P4 勾选。全量 session 7123 仍运行，暂保持Rust/清单/锁文件不变；当前跟踪源码 SHA-256 汇总 `97fa1fe7f2048249d7b5db89f1fb55360984da04042f06679c4d74396a9e725f`（git ls-files→逐文件shasum→汇总，排除文档和test.png），门禁未终态。行宽专项73623继续跟踪；未提交、未运行 fuzz。

- 2026-10-08 Type5 独立对照与旧组合终态：session 93718 退出0，十四页 Skia 渲染成功，第12–14页最新PNG按 PDF 技能逐页目视；MuPDF/Skia 均200×120，最大通道差2/4/4、平均最大差0.6965/0.959833/0.852875、差>3像素0/400/388，内部三个选点与外部白色的Skia值均符合解析公式，不宣称非线性全页完全一致或其差异原因已证实。证据 `tmp/pdfs/shading-lattice.TcXO6V/`，仅固定2×2灰度格点矩阵。较早 PDF组合 session 29860 退出0：557单元、25颜色、17版本、14资源、5Shading流程通过；源码期间有变化，且早于当前Type5，不据此宣称当前完整绿色。新工作区 `cargo test --workspace --locked --quiet` 已启动，日志 `/tmp/rzpdf-workspace-mesh-current.log`，尚未确认通过；注册恢复65853和行宽73623仍跟踪。未提交、未运行 fuzz，P4与整体TODO未完成。

- 2026-10-08 Type5 像素/容量确认与行宽前置额度：session 2500 退出0，网格像素专项1项通过（36 filtered，八组合+两策略严格重开全页一致），日志 `/tmp/rzpdf-shading-lattice-pixels.log`；session 24851 退出0，12项网格单元通过（549 filtered），日志 `/tmp/rzpdf-shading-lattice-capacity.log`。新增流解码前最低格点规模检查：两行2*w顶点+2*(w-1)三角形 checked 总量超过max_objects立即 ResourceLimit，新增缺其余字段仍优先额度拒绝专项；session 73623 运行中，日志 `/tmp/rzpdf-shading-lattice-row-budget.log`，尚未计通过。独立Skia93718/注册恢复65853仍活跃，未完成对照。完整性能/操作级预算与P4仍未完成，未提交、未运行 fuzz。

- 2026-10-08 Type5 独立显示初证与注册恢复：样本生成 session 68287 退出0；MuPDF info确认 PDF1.7、14页，第12–14页200×120 Lattice，draw成功无诊断，三个最新MuPDF PNG按 PDF 技能逐页目视，梯度/镜像正常。Skia同一文件 session 93718 运行中，日志 `/tmp/rzpdf-shading-lattice-independent-skia.log`，对照未完成；临时证据 `tmp/pdfs/shading-lattice.TcXO6V/`。新增不完整格点行注册失败保持修订号/保存字节、补齐后注册恢复回归，新日志 `/tmp/rzpdf-shading-lattice-registration.log`，尚未记通过。像素2500/容量24851继续跟踪，完整Type5矩阵/独立对照/预算与P4未完成；未提交、未运行 fuzz。

- 2026-10-08 Type5 流程确认：session 89611 退出0，5个 Shading 集成通过（0失败），包含Type4/5×直接/非线性注册、导入与两策略严格保存重读，日志 `/tmp/rzpdf-shading-lattice-workflow.log`；该门禁早于随后Type5容量前置修正，不替代新额度专项和独立显示验收。

- 2026-10-08 Type5 解码输入容量前置检查：复查发现格点路径虽在拓扑分配前扣除完整输入容量，但顶点分配阶段仍只计有效长度；新增 owned-lattice 入口先扣除 capacity-len，再顶点解码及输入/拓扑共存检查。新增128容量解码输入、完整网格精确额度、少1字节 ResourceLimit 和重试回归；session 24851 运行中，日志 `/tmp/rzpdf-shading-lattice-capacity.log`，尚未计通过。已读 PDF 技能，样本生成68287、流程89611、像素2500均复核仍活跃，暂未生成可检查PDF，不重启会话。局部额度仍不能替代完整操作级预算，Type5独立验收/整个P4仍未完成；未提交、未运行 fuzz。

- 2026-10-08 Type5 模型专项与独立样本准备：session 34876 退出0，19个 Shading/网格单元通过（541 filtered），日志 `/tmp/rzpdf-shading-lattice-model-tests.log`，覆盖Type5流/模型与此前网格数值。create_shading新增第12–14页格点直接灰度、非线性/镜像样本，session 68287 生成中，日志 `/tmp/rzpdf-shading-lattice-fixture.log`，临时目录 `tmp/pdfs/shading-lattice.TcXO6V/`。已重新读 PDF 技能，生成后需独立渲染/目视及本库对照，目前不能计样本验收；Type5流程89611、像素2500、较早PDF组合29860均继续跟踪。未提交、未运行 fuzz，完整P4及整体TODO仍未完成。

- 2026-10-08 Type5 流程与像素矩阵：Type5无标志顶点 session 58745 退出0，11项网格单元通过，日志 `/tmp/rzpdf-shading-lattice-vertices.log`；格点拓扑 session 94214 亦退出0，两者均早于后续Type5流/模型接入，不混报全部支持。流程扩展 Type4/5×直接/非线性四组合，session 89611 运行中，日志 `/tmp/rzpdf-shading-lattice-workflow.log`；像素扩展 Type4/5×直接/非线性×原始/镜像八组合，保留BBox和两策略严格重开全页一致断言，新日志 `/tmp/rzpdf-shading-lattice-pixels.log`，尚未确认通过。模型 session 34876 与较早 PDF组合29860继续跟踪；独立Type5样本/性能/完整预算仍未完成，未提交、未运行 fuzz。

- 2026-10-08 Type5 模型编译确认：session 90787 退出 0，cargo check -p pdf --locked --quiet 成功；专项与实际显示证据仍待验证，不据此标记 Type5 完成。

- 2026-10-08 Type5 流/模型接入与渲染门禁：from_stream 允许 Type4/5，Type5 校验精确整数 VerticesPerRow≥2、无 BitsPerFlag，解码后扣除活跃输入容量再构造完整格点行拓扑；Shading 新增 LatticeMesh，复用可选 Function、Decode 域与 Indexed 限制、保留载荷检查和逐点采样。新增完整2×2直接/非线性数值、外部点、实数/非法/不完整行宽回归。编译 session 90787，日志 `/tmp/rzpdf-shading-lattice-model-check.log`；专项 session 34876，日志 `/tmp/rzpdf-shading-lattice-model-tests.log`，均尚未确认通过。较早 render 全量 session 97909 已退出 0，37项通过、0失败/忽略（52.81秒），包含Type4镜像/保存像素；门禁早于Type5且测试期间网格基础曾变化，不宣称当前全部代码全量通过。Type5流程/像素/独立阅读器、性能和完整P4仍未完成。未提交、未运行 fuzz。

- 2026-10-08 Type5 无标志记录与网格持有：提取共同顶点位读取，新增无标志位 lattice 解码，保持 Type4 入口拒绝零位宽标志；新增逐记录补齐/截断/精确额度回归，session 58745 运行中，日志 `/tmp/rzpdf-shading-lattice-vertices.log`，尚未确认通过。随后新增 try_new_lattice，复用顶点有限值/维度、边界和共享索引持有，拓扑专项补网格采样断言；该后续修改尚未计入已启动门禁。Type5 PDF 流构造/Shading/显示仍未接入，不能计对外支持；原会话 94214/29860/97909 继续跟踪，不重启。未提交、未运行 fuzz。

- 2026-10-08 Type5 格点拓扑基础：依据 Adobe PDF Reference 的逐行顶点顺序与两组三角形连接规则，新增 assemble_lattice，要求至少两条完整且每行至少2顶点；三角形数量 checked 运算，复用提取的 Type4 顶点/分量/三角形合计存储检查。新增2×3精确连接、非法行宽/不完整行及对象额度边界回归；session 94214 运行中，日志 `/tmp/rzpdf-shading-lattice-topology.log`，尚未计通过。规范镜像来源：https://chromium.googlesource.com/external/skia/+/7983cf450ea4da39f8a9bd2a27d0412fb6a2fad0/experimental/PdfViewer/PdfReference-okular-1.txt 。Type5 无标志位解码、流构造/显示及像素尚未接入，仍不支持，不勾选 P4。原 PDF组合 session 29860/render全量 session 97909 继续跟踪；提取公共检查已改变前者源快照范围，不能混报当前全部代码全量通过。未提交、未运行 fuzz。

- 2026-10-08 网格输入容量专项确认：session 62698 退出 0，9 个 shading_mesh 单元通过（548 filtered），日志 `/tmp/rzpdf-shading-mesh-capacity.log`；输入空余容量及既有载荷共存边界通过。此门禁启动早于整体边界缓存改动，不替代当前 PDF 组合门禁。

- 2026-10-08 Type4 网格保存后全页像素：在直接/非线性×原始/镜像四组合中增加 Preserve/Flate 保存、严格重开、全页 RGBA 完全一致断言，覆盖既有明确选点与 BBox 外白色。启动 render 全量 session 97909，日志 `/tmp/rzpdf-shading-mesh-render-full.log`，尚未确认通过。PDF组合 session 29860 与容量 session 62698 已复核仍活跃，继续等待同一会话，不重启；新 render 测试修改不属于 PDF组合的验证目标。项目整体 TODO、Type5–7/Pattern/复杂网格性能/完整预算仍未完成；未提交、未运行 fuzz。

- 2026-10-08 RGB流程与网格外快速排除：session 34625 退出 0，5 个 Shading 流程测试通过，包含 RGB 三分量顶点/内部值/域外及导入、两保存策略严格重读，日志 `/tmp/rzpdf-shading-mesh-rgb-workflow.log`。TriangleMesh 构造缓存整体有限边界，Shading 在分配分量缓冲/扫描三角形前排除边界外点；边界包含只作保守筛选，不替代三角形判断，新增边界端点/外部/NaN 回归，不改变内部插值。启动当前 PDF lib+shading_workflow+color_workflow+feature_versions+resources 组合门禁 session 29860，日志 `/tmp/rzpdf-shading-mesh-current-gate.log`，尚未确认通过；容量专项 session 62698 仍跟踪，不重启活跃会话。内部逐像素分配/线性扫描、抗锯齿和完整预算仍待完善。未提交、未运行 fuzz。

- 2026-10-08 网格既有载荷额度与输入容量：session 24965 退出 0，15 个 Shading/网格单元通过，日志 `/tmp/rzpdf-shading-mesh-retained-budget.log`。流入口新增 owned-input 解码边界，将解码 Vec 的 capacity-len 空余容量也计入共存额度，不仅按有效字节长度计算；新增128容量输入精确边界、少1字节拒绝，并区分借用切片的已知长度计费回归。session 62698 运行中，日志 `/tmp/rzpdf-shading-mesh-capacity.log`，尚未确认通过；RGB流程 session 34625 仍待终态。解码器构造期间临时峰值、对象解析及全操作共享租约仍未贯通，不标记 AR-03 完成。未提交、未运行 fuzz。

- 2026-10-08 Type4 RGB 直接分量流程补充：新增三分量记录 RGB 三顶点、内部 [128,64,64] 混合色、三角形外不绘制，以及资源注册/导入/两保存策略严格重读回归；session 34625 运行中，日志 `/tmp/rzpdf-shading-mesh-rgb-workflow.log`，尚未记通过。同步 README 的 Type4 calculator 过时描述和架构的 Type1/4 接入检查点，仍明确 Type5–7/Pattern/性能/操作级预算未完成。新额度 session 24965 继续跟踪；未提交、未运行 fuzz。

- 2026-10-08 非法网格注册恢复确认：session 33657 退出 0，4 个 Shading 流程测试通过，日志 `/tmp/rzpdf-shading-mesh-registration.log`；新增错误网格注册保持修订号/保存字节及修复后重试已通过。该门禁早于后续保留载荷额度修改，不替代新预算专项。

- 2026-10-08 Type4 既有载荷共存额度：修正 Shading 网格构造的分配前边界，from_stream_with_retained 从网格局部额度预先扣除已保留 Function/Background，随后 Decode/输入/顶点/拓扑按剩余额度验证，避免仅最终合计检查。Decode 容量乘法也改 checked_mul。新增既有100字节载荷+完整网格精确边界、少1字节 ResourceLimit 及去掉既有载荷后重试回归；session 24965 运行中，日志 `/tmp/rzpdf-shading-mesh-retained-budget.log`，尚未计通过。流解码器/对象解析临时分配与原生开销仍未完整聚合，不标记 AR-03 完成；注册失败专项 session 33657 仍活跃。未提交、未运行 fuzz。

- 2026-10-08 Type4 独立对照与注册失败：session 46013 退出 0，十一页 Skia 渲染成功，第9–11页最新PNG均按 PDF 技能目视检查。三页 MuPDF/Skia 均200×120，最大通道差分别2/4/4，平均最大差0.6965/0.959833/0.852875，差>3像素0/400/388；三个内部选点及外部白色的 Skia 与解析公式一致，不把非线性差异归结为已证实的单一原因或宣称完全一致。临时证据 `tmp/pdfs/shading-mesh.LNagm8/`，仅固定灰度双三角形/函数/镜像组合。新增截断记录、非法起始共享边和不完整三角形注册失败的修订号/保存字节不变及修复后重试回归；session 33657 运行中，日志 `/tmp/rzpdf-shading-mesh-registration.log`，尚未计通过。网格性能/预算、颜色/几何全矩阵与完整P4仍未完成，未提交、未运行 fuzz。

- 2026-10-08 Type4 独立显示初证：session 78832 镜像像素专项和 session 84370 样本生成均退出 0。MuPDF info 确认 preserve.pdf 为 PDF1.7、11页，第9–11页200×120 Triangle mesh；draw 成功无诊断，三个最新 MuPDF PNG 均目视检查，直接/非线性/镜像梯度方向正常。PDF 技能指导渲染后目视检查，不仅看解析结果；系统 Poppler 依赖问题仍存在，使用已有 MuPDF。Skia 同一PDF渲染 session 46013 运行中，日志 `/tmp/rzpdf-shading-mesh-independent-skia.log`，尚无对照证据；临时目录 `tmp/pdfs/shading-mesh.LNagm8/` 保留待对照，不交付为最终验收。未提交、未运行 fuzz，完整网格功能矩阵/P4仍未完成。

- 2026-10-08 Type4 独立样本准备：组件/颜色限制 session 10804 退出 0，14 项 Shading/网格单元通过（541 filtered），覆盖新函数域及组件/Indexed 限制。create_shading 示例新增第9–11页共享边网格直接灰度、非线性及镜像，session 84370 生成中，日志 `/tmp/rzpdf-shading-mesh-fixture.log`，临时目录 `tmp/pdfs/shading-mesh.LNagm8/`。按 PDF 技能检查 Poppler，再次确认缺 libtiff.5.dylib 无法启动，计划使用现有 MuPDF，不修改系统库。尚未生成/渲染/目视确认，不能计独立验收；镜像像素 session 78832 仍待终态。未提交、未运行 fuzz。

- 2026-10-08 Type4 像素专项与镜像：session 51921 退出 0，1 个网格像素测试通过（36 filtered），确认共享边两三角形直接/非线性颜色的像素中心数值与 BBox 外白色，日志 `/tmp/rzpdf-shading-mesh-pixels.log`。随后扩展水平镜像 CTM，按逆变换参数核对四个组合；旧专项结束后启动 session 78832，日志 `/tmp/rzpdf-shading-mesh-mirrored-pixels.log`，新版尚未通过。较早工作区 session 75157 已退出 0，日志 `/tmp/rzpdf-workspace-shading.log`；执行期间源码有变化且起始指纹早于当前 Type1/网格/示例，不把此门禁称为当前工作区完整通过。组件模型 session 10804 继续活跃。未提交、未运行 fuzz；独立网格样本及完整 P4 尚未完成。

- 2026-10-08 Shading 保存/导入专项确认：session 45931 退出 0，日志 `/tmp/rzpdf-shading-mesh-workflow.log` 确认 3 个集成测试通过，包含新增 Type4 两三角形直接/非线性组合、注册/来源/导入及 Preserve/Flate 严格重读。像素 session 51921 尚未终态，不据此宣称渲染/独立工具验收。

- 2026-10-08 Type4 模型数值与颜色组合：session 84956 已退出 0，13 项 Shading/网格单元通过，日志 `/tmp/rzpdf-shading-mesh-model-tests.log`；该门禁启动早于随后函数域修改，不据此宣称新域检查全量通过。新增 RGB 三个组件函数精确颜色 [64,16,191]、无 Function 的 Indexed 顶点黑白值，以及 Indexed+Function、Pattern、错误函数数组拒绝专项；session 10804 运行中，日志 `/tmp/rzpdf-shading-mesh-component-tests.log`。保存/导入 session 45931、像素 session 51921 继续确认活跃，尚无终态；网格性能/预算、独立工具、完整 P4 仍未完成。未提交、未运行 fuzz。

- 2026-10-08 Type4 流程/像素回归接入：新增共享边双三角形、直接灰度与非线性 Function 两组合的资源注册、来源事件、跨文档导入、两种流保存策略严格重读回归（session 45931，日志 `/tmp/rzpdf-shading-mesh-workflow.log`）；新增 Skia 明确像素中心值及 BBox 外空白回归（日志 `/tmp/rzpdf-shading-mesh-pixels.log`），门禁尚未确认通过。函数域校验 cargo check session 77367 已退出 0；新模型回归 session 84956 仍待终态，独立阅读器和完整网格验证未完成，不勾选 P4-03。未提交、未运行 fuzz。

- 2026-10-07 Type4 点采样接入：流字段 session 13234 退出 0，7 项通过。Shading FreeformMesh 点采样按三角形插值编码分量/参数，直接颜色经 ColorSpace 转换，有函数先插值参数再 evaluate_color，域外不绘制；增加直接灰度64与非线性结果16、域外和非流拒绝模型回归，writer 既有不支持类型 fixture 改为仍不支持的 Type6。点采样 cargo check session 54163 退出 0；模型回归 session 84956 运行中，日志 `/tmp/rzpdf-shading-mesh-model-tests.log`。随后新增全类型 Function input_domain getter 与 Decode 参数区间包含校验/过窄域拒绝回归；新编译日志 `/tmp/rzpdf-shading-mesh-domain-check.log`，尚未确认新门禁。Skia 复用既有逐点路径，但尚无网格实际像素/保存/独立阅读器证据，不计 P4-03 完成。逐像素缓冲分配与全三角形扫描仍需性能/工作量预算完善；未提交、未运行 fuzz。

- 2026-10-07 Type4 Shading 模型初步接入：Shading 构造允许 stream 字典，新增 FreeformMesh 几何标记与持有网格；Function 可选，直接颜色采用 ColorSpace 分量数，有函数采用单参数，Indexed 仅无函数网格允许，保留网格与函数载荷合计检查。cargo check session 94893 退出 0，日志 `/tmp/rzpdf-shading-mesh-model-check.log`。网格点采样暂明确返回 UnsupportedFeature，不静默略过；Function Decode 域覆盖校验、构造专项、旧不支持 fixture 迁移及显示/像素仍待接入，本次不计完整 Type4 支持。字段回归 session 13234 仍待终态；未提交、未运行 fuzz。

- 2026-10-07 Type4 流字段回归：新增有效流采样、间接引用位宽、错误 ShadingType、实数/负数/缺失位宽、非法位宽、Decode 长度/非数值、截断及编码分量数不匹配测试。首轮 session 35517 退出 101，测试初始化未解包 PdfDocument::new 的 Result；已修正初始化并使用现有 get_object 解析接口，重跑 session 13234，日志 `/tmp/rzpdf-shading-mesh-stream-tests-fixed.log`，尚未确认通过。生产私有流入口此前 cargo check 成功；新测试不计通过，Type4 注册/显示/像素仍未接入。未提交、未运行 fuzz。

- 2026-10-07 Type4 私有流入口编译确认：cargo check -p pdf --locked --quiet，session 96950 退出 0。仅证明编译，不替代尚未增加的字段回归、Shading 注册/显示和像素验收。

- 2026-10-07 网格专项及 Type4 流入口：session 47928/98786 均退出 0，分别确认 5 项所有权阶段和 6 项解码阶段网格单元通过。新增私有 from_stream，解析精确整数 ShadingType/位宽、动态有限 Decode，并经现有 decode_data 接入网格解码；编码分量数仍由后续 ColorSpace/Function 构造方提供。Decode 存储也从局部额度扣除；流解码器/对象解析临时分配不在此局部检查完整覆盖范围。cargo check session 96950 已启动，日志 `/tmp/rzpdf-shading-mesh-stream-check.log`；新流入口尚无字段单元回归，不计 Type4 对外支持。未提交、未运行 fuzz。

- 2026-10-07 网格插值专项确认：session 15186 退出 0，日志 `/tmp/rzpdf-shading-mesh-interpolation.log` 确认 4 项通过、0 失败；覆盖此前顶点解码、拓扑、权重和分量插值，不包含后续新增的网格所有权及解码入口测试，后两项门禁仍待终态。

- 2026-10-07 Type4 解码/网格持有入口：新增 MeshEncoding 和 TriangleMesh::decode，串联字节解码、共享顶点与拓扑；分配拓扑时从额度扣除仍活跃的输入数据，并提供按容量计算的 retained_payload_bytes 供后续显示事件记账。新增完整字节记录→采样、输入与拓扑共存额度精确边界/少1字节拒绝及失败后重试测试；session 98786 运行中，日志 `/tmp/rzpdf-shading-mesh-decoded.log`，尚未确认通过。该局部检查不等同 AR-03 操作级聚合预算；Type4 Shading 构造与渲染尚未接入，未提交、未运行 fuzz。

- 2026-10-07 Type4 网格所有权/采样准备：新增私有 TriangleMesh，统一持有共享顶点与经验证的三角形索引，检查顶点分量维度和有限值，提供不分配缓冲区的单三角形采样及安全索引检查。新增共享边一致性、第二三角形内部精确值、非法维度/坐标回归；session 47928 运行中，日志 `/tmp/rzpdf-shading-mesh-owned.log`，尚未计通过。重心权重 session 41800 已退出 0、3 项通过；此证据早于新所有权测试。Type4 PDF 流构造及 sh/渲染接入仍未完成；未提交、未运行 fuzz。

- 2026-10-07 Type4 颜色分量插值准备：增加接受调用方缓冲区的 triangle 插值，不分配逐像素临时 Vec；先插值编码颜色分量或函数参数，不先转换顶点 RGB/求函数值。新增双分量精确值、非线性函数顺序、域外不改写及维度错误回归。session 15186 正在运行，日志 `/tmp/rzpdf-shading-mesh-interpolation.log`；上一重心权重 session 41800 也确认仍活跃，均尚未记为通过。Type4 仍未接入构造和渲染，本轮未提交、未运行 fuzz。

- 2026-10-07 Type4 网格插值准备：私有 shading_mesh 增加按顶点顺序的 f64 重心权重，支持正反绕序、边界，并拒绝域外、非有限或零面积三角形；权重用于颜色分量/函数参数插值之前，不对顶点 RGB 做插值。新增边界、退化、极大/极小坐标专项；session 41800 正在运行，日志 `/tmp/rzpdf-shading-mesh-weights.log`，尚无通过结论。Type4 构造/插值接入/渲染仍未完成，不勾选网格支持；未运行 fuzz，本轮未自动提交。

- 2026-10-07 用户要求本地提交检查点：提交 Shading 渲染、Type1 二维函数着色、资源校验、版本检查、流程/示例与私有 Type4 网格基础。网格顶点专项日志确认 1 项通过，新增共享边三角形组装专项日志 `/tmp/rzpdf-shading-mesh-topology.log` 确认 2 项通过；Type4 尚未接入 Shading 构造、插值或渲染，仍不支持。此前 render 全量 36 项通过；工作区全量仍运行且期间源码有变化，不代表本次提交全量通过。不提交来源未确认的 `test.png`，不推送远端、不运行 fuzz；项目总体 TODO 仍未完成。

- 2026-10-07 Type1 render 全量确认：session 31572 退出 0，36 渲染测试通过、0 失败/忽略（47.07 秒），日志 `/tmp/rzpdf-shading-type1-render-full.log`。该门禁早于新增网格模块，不代表 Type4 网格或全部当前工作区通过；Type1 固定独立页面证据及明确未完成边界保留。未运行 fuzz 或自动提交。

- 2026-10-07 Type4 网格顶点解码阶段：新增私有 shading_mesh 顶点解析，校验规定 flag/坐标/分量位宽、Decode 数量/有限值、记录字节对齐、截断和输入/顶点/分量共存上限；f64 Decode 支持32位坐标、反转分量及仅低2位标志。新增补齐位/逐记录重置/32位/截断/额度单元，session 93337 运行中，日志 `/tmp/rzpdf-shading-mesh-vertices.log`。尚未接入 Shading 构造、三角形共享边、插值或显示，Type4 仍不支持，不能把解析基础计为完整网格实现。Type1 render 原全量 session 31572 继续跟踪；未运行 fuzz 或自动提交。

- 2026-10-07 Type1 独立显示证据：session 53352/81708 均退出 0，八页样本生成与 Skia 严格读取渲染成功；MuPDF info 确认第八页 Function Shading、200×120，draw 无语法诊断，两张最新 PNG 均目视检查。全页 MuPDF/Skia 最大通道差2、平均最大差0.6、差>3为0，三个内部选点及外部空白的 Skia RGB 与解析公式一致。证据在 `tmp/pdfs/shading-type1.l7PEIo/`；只覆盖此固定 Type4 RGB 页面，不表示全部 Type1 函数/矩阵/抗锯齿验收。已启动含 Type1 的 render 全量，日志 `/tmp/rzpdf-shading-type1-render-full.log`，尚无终态；未运行 fuzz 或自动提交。

- 2026-10-07 Type1 流程/像素确认：session 91422 退出 0，2 个 shading_workflow 集成通过；session 60844 退出 0，新增 Type1 双函数组合像素/两保存策略严格重开专项 1 通过（35 filtered）。session 55524 退出 0，日志实际结果为 545 单元、25 颜色流程、2 Shading 流程通过；组件/Sampled 新单元此前另由 92575 覆盖，不混报当前全部546单元全量。生成示例已增加第八页 Type1 RGB/显式矩阵，session 53352 运行中，临时输出 `tmp/pdfs/shading-type1.l7PEIo/preserve.pdf`，尚未生成/独立渲染终态。P4 仍未完成，未运行 fuzz 或自动提交。

- 2026-10-07 Type1 实际像素回归补充：Skia 新增 Type4/Type0 Sampled 双输入 RGB 两组合，检查 Matrix 后明确像素、BBox 外透明且直接 sh 不以 Background 填充、Preserve/Flate 严格重开全页一致。新增代码仅局部格式；session 60844 运行中，日志 `/tmp/rzpdf-shading-type1-pixels.log`，尚未记为通过。Type1 模型和流程原会话继续跟踪；未运行 fuzz 或自动提交，P4-03 保持未完成。

- 2026-10-07 Type1 数值专项确认：session 92575 退出 0，5 个 Shading 单元通过（541 filtered），覆盖二维域/矩阵、双输入组件数组和 Sampled 插值。session 55524 的 545 单元已通过但颜色/流程目标仍运行；不把两个运行范围混记为当前全部单元/流程全量。未运行 fuzz 或自动提交。

- 2026-10-07 Type1 流程回归补充：shading_workflow 新增双输入 Type4/Type0 Sampled RGB 两种函数，检查资源注册、单个带来源 Shading 显示事件、矩阵求值/域外不绘制、跨文档页面导入、源与导入文档的 Preserve/Flate 严格读回。新 session 91422 运行中，日志 `/tmp/rzpdf-shading-type1-workflow.log`，尚未计通过。新测试不在先前 session 55524 编译的旧流程目标内；Type1 独立像素矩阵与完整 P4 仍待验收，未运行 fuzz 或自动提交。

- 2026-10-07 Type1 函数表示覆盖补充：新增 RGB 三个双输入 Type4 组件函数，检查 (.25,.75)→(64,191,128)；新增二维 Type0 Sampled [2,2] 网格端点、中心及非中心双线性插值。session 92575 专项运行中，日志 `/tmp/rzpdf-shading-type1-functions.log`，不记为通过。此前通用求值重构 session 12621 已退出 0（3 个原 Shading 单元通过）；Type1 首轮模型/流程 session 55524 尚未终态。未运行 fuzz 或自动提交。

- 2026-10-07 Type1 模型/点求值接入：新增 FunctionBased 几何（二维 Domain 默认 [0 1 0 1]、Matrix 默认单位矩阵）、f64 逆变换和域外透明；双输入单函数/组件数组校验以及函数 Domain 包含 Shading Domain 检查，奇异矩阵不绘制。单位置 color_at 对 Type1 明确拒绝，geometry 提供完整域/矩阵，旧 domain getter 为 x 区间。补默认/矩阵/BBox/奇异/维度/域错误单元；历史 unsupported 操作 fixture 改为仍未支持的 Type4 网格。render 编译检查 session 47796 退出 0；PDF 模型/流程 session 55524 运行中，日志 `/tmp/rzpdf-shading-type1-model.log`，尚无测试通过结论。后端像素、组件函数数组、Sampled、保存/导入和独立阅读器矩阵仍待验收；P4-03 不勾选，未运行 fuzz 或自动提交。

- 2026-10-07 Type1 实现准备：核对二维 Domain 与 Matrix、2→n/组件 2→1 函数要求；先抽取 Shading 内部通用坐标输入颜色求值，组件函数不再直接索引输出而检查恰好一个值。Type1 构造/二维域映射/矩阵/函数域包含校验尚未实现，不能声称支持；轴向/径向模型专项日志 `/tmp/rzpdf-shading-evaluator-refactor.log` 运行中。未运行 fuzz 或自动提交。

- 2026-10-07 当前 Shading 修复门禁确认：session 87146 退出 0，544 单元、25 颜色流程、17 保存版本、14 资源测试全部通过；session 5710 退出 0，Form 局部覆盖/来源/状态与跨文档页面导入、两策略严格读回组合 1 测试通过。日志 `/tmp/rzpdf-shading-current-final.log`、`/tmp/rzpdf-shading-form-import.log`。当前完整工作区仍未验收；session 75157 尚运行且使用修复前源码，不作为当前全量结论。下一实现缺口为边缘覆盖、Type1/4–7、Pattern、完整预算与跨后端矩阵，P4-03 不勾选，未运行 fuzz 或自动提交。

- 2026-10-07 Form Shading 来源/作用域回归通过：session 84732 退出 0，新增集成 1 测试通过，两保存策略严格重开保留同名局部覆盖、调用矩阵/裁剪、各调用来源及页面状态恢复。随后扩展跨文档页面导入与保存读回，验证资源图中的 Shading 与来源层级保留；新测试仍运行，日志 `/tmp/rzpdf-shading-form-import.log`，不把旧结果当作扩展通过。当前版本门禁已通过 544 单元和25颜色流程，剩余目标尚未终态；未运行 fuzz 或自动提交。

- 2026-10-07 Form Shading 集成回归补齐：新增独立 `pdf/tests/shading_workflow.rs`，页面与 Form 同名 Shading 验证局部覆盖、两次调用不同来源、Form 裁剪/矩阵及回到页面后状态恢复，两保存策略严格读回。session 84732 运行中，日志 `/tmp/rzpdf-shading-form-workflow.log`，尚未记为通过；当前四目标门禁 session 87146 亦运行中，不包含这个新增测试文件。原工作区门禁仍为修复前基线，未运行 fuzz 或自动提交。

- 2026-10-07 Shading 别名/预检修复验证完成：session 9160 退出 0，24 个颜色流程、14 个资源测试全部通过；session 19190 退出 0，3 个 Shading 专项通过（22 filtered），包括低层非法修改后解释拒绝不改修订号及合法恢复。此前 session 99023 的 544 单元已通过但流程失败，不抹去该失败记录；本次尚未重新验证全部 25 个颜色测试或新工作区，不能宣称完整当前全量通过。README/架构文档同步直接颜色定义与事务前预检边界；未运行 fuzz 或自动提交。

- 2026-10-07 Shading 事务前预检进展：session 9160 的 24 个颜色流程已全部通过，资源目标仍运行，未记完整终态。新增合法注册后低层改为非法别名、解释拒绝不改修订号、修回 DeviceGray 后解释恢复的回归；session 19190 专项运行中，日志 `/tmp/rzpdf-shading-alias-interpretation.log`，尚无结论。该新测试不属于 9160 已执行颜色目标的范围；未运行 fuzz 或自动提交。

- 2026-10-07 Shading 修复回归发现并调整事务边界：session 99023 单元 544 通过，但颜色流程新增回归失败（23 通过/1 失败，修订号 10 对 9），并未全部通过。原因是非法定义在事务内校验，回滚按既有契约推进修订号；改为 Shading 在事务前预检并避免事务内重复构造。新 session 9160 验证颜色/资源目标，日志 `/tmp/rzpdf-shading-preflight-revision.log`，仍运行，不记为通过。源码再次变化，旧工作区门禁仍只代表修复前基线；未运行 fuzz 或自动提交。

- 2026-10-07 Shading 显式颜色定义修复：删除模型辅助别名查找以及资源注册/内容解释中的 ColorSpace 资源解析；构造对未知名称别名明确 ColorError。合法流程测试改为 DeviceGray，新增别名注册失败修订号/保存字节不变回归。新 session 99023（544 单元＋颜色/资源目标）运行中，日志 `/tmp/rzpdf-shading-explicit-colorspace.log`，尚未记为通过。自此生产/测试源已变化；原工作区 session 75157 仅代表启动时修复前版本，不能证明本修复或新示例通过。未运行 fuzz 或自动提交。

- 2026-10-07 Shading 别名问题定位：Adobe PDF Reference 1.6 色彩空间定义要求内容流外直接指定颜色空间，不通过 ColorSpace 资源子字典。当前 register_existing 只在临时校验副本解析别名，源字典未改，可能保存非规范 LocalGray 名称；此为待修实现缺口，不能把本库读回成功当作合法写出。Form 样本改用明确 DeviceGray，局部 Shading/S 与双次调用仍保留；新样本尚待重新生成验证。原工作区 session 75157 针对修复前生产源继续运行，不把该结果作为别名修复证明。

- 2026-10-07 Form 样本发现兼容性问题：MuPDF 第七页报告 `unknown colorspace: LocalGray`，虽 draw 退出但有语法诊断，不能视为独立显示成功。需核对 Shading.ColorSpace 是否允许资源别名及本库注册/解释的规范边界；Skia session 23073 尚运行，先保留该失败证据，不忽略诊断或直接判为支持。

- 2026-10-07 Form Shading 样本准备：第七页增加仅在 Form.Resources 中定义的 LocalGray 颜色别名及 Shading/S，同一 Form 两次平移调用。修正示例 Option/Result 字典转换后 session 62095 退出 0，生成 `tmp/pdfs/shading-form.DNyq6U/preserve.pdf`；第七页独立/Skia 渲染对比尚进行中，不计为 Form 验收。原工作区 session 75157 继续运行，未修改其既有生产/测试源，未运行 fuzz 或自动提交。

- 2026-10-07 Shading 变换样本扩展：生成示例新增 CTM 90° 旋转叠加页面 Rotate=90、带剪切与非均匀缩放的径向两页，共六页；session 27468 退出 0，临时 PDF 位于 `tmp/pdfs/shading-transforms.nN0A8N/preserve.pdf`。旧四页证据未覆盖；新 Skia/MuPDF 对比尚待完成，不记为变换验收。原工作区 session 75157 继续执行集成测试，未运行 fuzz 或自动提交。

- 2026-10-07 Shading 差异分层：Ghostscript 9.23 四页渲染退出 0且逐页目视检查；三处同心径向像素的解析 N=2 结果与 Skia 完全一致，参考后端互有差异。另统计 MuPDF/Skia 白色覆盖分类不一致 120/141/121/0 像素，轴向集中在边界列；径向内部仍存在色阶差异。分析及具体坐标写入 docs/testing.md；未据单一后端修改求值或放宽容差，边缘覆盖/参考近似审计仍待实施。工作区原 session 75157 继续运行，未运行 fuzz 或自动提交。

- 2026-10-07 Shading 跨后端首轮比较：新增严格读取、逐页 72 DPI 输出且拒绝覆盖文件的 `render/examples/render_document.rs`；修复页数/索引 API 编译错误后 session 49063 退出 0，四页 Skia PNG 已逐页目视检查。MuPDF/Skia 同为 200×120；每页最大通道差分别 255/255/255/4，差>3 的像素分别 449/2952/744/23，平均最大通道差约 2.412/2.454/1.847/0.336。形状一致但尚未通过差异验收；径向内部 (140,60) 的 RGB 为 MuPDF(161,0,93)/Skia(167,0,88)，边界存在高差，需分离抗锯齿/采样误差并追踪原因，不能仅放宽容差。PNG 位于 `tmp/pdfs/shading-validation.XVQgC4/`；当前工作区回归仍运行，新增两示例不在其启动时指纹内。未运行 fuzz 或自动提交。

- 2026-10-07 Flate Shading 样本：session 60506 退出 0，MuPDF info/draw 同样成功；两保存策略的四张 PNG 对应 SHA-256 完全相同，故前述逐页目视检查也覆盖相同像素内容。尚未进行 Skia 与 MuPDF 的跨后端对比，不扩大为完整 Shading/P4 验收。

- 2026-10-07 Shading 样本独立读取初步证据：新示例 Preserve 生成 session 57563 退出 0；临时样本 `tmp/pdfs/shading-validation.XVQgC4/preserve.pdf` 经 MuPDF info 确认 PDF 1.7、4 页、200×120 及两轴向/两径向，draw 退出 0，四页 PNG 已逐页目视检查。系统 Poppler 虽有命令但启动缺 libtiff.5.dylib；未修改系统库，改用现有 MuPDF。不把此次识别/显示视为跨后端像素一致性验收，Flate 样本、本库同页图像比较及完整矩阵仍待完成。工作区 session 75157 中 544 单元和 23 颜色流程已通过，但整体仍运行；未运行 fuzz 或自动提交。

- 2026-10-07 独立 Shading 样本准备：新增 `pdf/examples/create_shading.rs`，四页覆盖轴向/同心径向/移动缩小径向、N=2、Extend、BBox、当前裁剪及直接 sh 的 Background，支持 Preserve 与 --flate 保存参数。仅已局部格式与差异检查；尚未编译、生成 PDF 或经 Poppler 验证，不计为验收。该新示例不属于已启动 session 75157 的源码指纹范围，后续单独运行验证；既有生产/测试源指纹未变化，全工作区测试仍运行。未运行 fuzz 或自动提交。

- 2026-10-07 新工作区门禁已启动：`cargo test --workspace --locked --quiet`，session 75157，日志 `/tmp/rzpdf-workspace-shading.log`；当前仍运行，尚无全量通过结论。启动时跟踪的 Rust/清单/锁文件 SHA-256 汇总为 `59b7e69141cee28061d8f58de73d46e5554e2bfec874d2f26ab2afdd598f899a`，测试期间未修改这些文件。workspace 不包含 fuzz；本门禁不代替独立阅读器或未完成 TODO 的验收。

- 2026-10-07 Shading 渲染全量及版本检查：session 73417 退出 0，render 35 测试通过、0 失败/忽略（48.48 秒），日志 `/tmp/rzpdf-shading-render-full.log`。ShadingType 精确整数 1–7 最低版本检查接入已有普通/预算预检解析器，低于 1.3 写出前拒绝；session 13812 退出 0，feature_versions 17 测试通过，日志 `/tmp/rzpdf-shading-versions.log`，覆盖直接/间接字段、输出/修订号不变、显式升级严格读回及不舍入未知数字。依据 Adobe PDF 32000-1 的 Shading 版本定义；这是定向功能检查，不是完整规范符合性。新完整工作区、独立阅读器与其他 P4 功能仍待推进，未运行 fuzz 或自动提交。

- 2026-10-07 Shading 状态/预算专项：新增裁剪与 BBox 交集、0.5 填充透明度明确像素、栅格共存预算 9599 字节拒绝/9600 字节接受、拒绝不留下部分绘制、NaN 透明度不改变已有画面及去裁剪后重试回归。session 1630 退出 0：2 个 Shading 专项通过，日志 `/tmp/rzpdf-shading-state.log`。新增代码已局部格式整理，render 全量回归正在运行，尚不记为通过；完整工作区、独立阅读器、Type1/4–7 和 Pattern 仍待推进，P4 不勾选，未运行 fuzz 或自动提交。

- 2026-10-07 Shading 后端初步接入：Skia 新增 Type2/3 逐像素反变换、调用原 PDF 函数/颜色转换的绘制路径，不以端点 RGB 线性插值替代非线性函数；无覆盖区域透明，裁剪/填充透明度/混合模式使用图形状态，奇异矩阵不绘制。按表面、Rust 栅格、Skia 像素副本共存检查 max_render_bytes，原生记账和函数求值临时分配仍未纳入完整聚合预算。cargo check -p render 退出 0；session 67521 退出 0，轴向 N=2 与径向像素及 Preserve/Flate 严格保存读回专项 1 测试通过，日志 `/tmp/rzpdf-shading-render-pixels.log`。裁剪/透明度/预算失败专项、完整 render/工作区与独立阅读器尚待验证；P4 不勾选，未运行 fuzz 或自动提交。

- 2026-10-07 本地提交检查点：Shading Type2/3 已接入命名颜色资源解析、注册复用、sh 显示事件、状态/来源记录及回放；新增轴向投影和径向求根的点采样。最终 session 89144 退出 0：544 单元、23 颜色流程、14 资源测试全部通过，日志 `/tmp/rzpdf-shading-geometry-final.log`。Type4 扩展后的 render 全量此前 33 测试通过；当前尚未重新跑完整工作区。PageObject::Shading 从占位变为携带 ShadingObject，Device 新回调默认返回不支持；Skia 实际渐变绘制仍未实现。函数保留容量及每次绘制载荷已有保守计数，但不等于完整操作级聚合预算。未运行 fuzz，未推送远程，P4 仍未完成。

- 2026-10-07 Shading 参数模型阶段：新增 color::shading 的 Type2 轴向/Type3 径向参数，校验有限坐标、非负半径、Domain/Extend/BBox/Background/AntiAlias，拒绝 Pattern/Indexed 基空间。单函数必须 1→颜色分量，组件函数数组各 1→1，函数树节点额度跨组件共用；color_at 按 Domain 插值后调用已有函数/颜色转换。参数错误与总节点失败后重试/修订号不变已覆盖。session 67163 退出 0：541 单元、22 流程通过，0 失败/忽略，日志 `/tmp/rzpdf-shading-model-final.log`。当前仅模型/数值，sh 资源解释、带状态显示事件、显示载荷计数及实际后端尚未接入，不能声称渐变可显示；Type1/4–7、Pattern 和独立阅读器另待推进。未运行 fuzz/自动提交，P4 未完成。

- 2026-10-07 Type4 像素扩展后的 render 全量完成：session 75585 终态退出 0，33 通过、0 失败/忽略（52.73 秒），日志 `/tmp/rzpdf-type4-render-final.log`。当前 PDF 最终回归为 539 单元、22 颜色流程，Skia 覆盖 Type4 Separation/六分量 DeviceN 默认/反转及两保存策略严格读回。独立阅读器、新全工作区、Pattern/Shading 与共享预算尚未完成；Type4 已有选定范围实际求值证据，不等于完整 P4，未运行 fuzz/自动提交。

- 2026-10-07 Type4 Skia 像素专项：既有校准/专色图像测试增加 Type4 驱动的 Separation 与六分量 DeviceN，默认/反转和两保存策略严格重开逐像素一致；现有测试共 16 个参数组合但仍一个测试函数。session 58096 退出 0：1 通过、32 filtered，日志 `/tmp/rzpdf-type4-pixels.log`。此前既有 render session 16781 也已退出 0：33 通过、0 失败/忽略（53.10 秒），日志 `/tmp/rzpdf-type4-render.log`；早于像素组合扩展，不能替代包含新组合的全量。独立阅读器、全工作区与共享预算未完成；未运行 fuzz/自动提交。

- 2026-10-07 Type4 字面量/栈边界：文字实数限制为有限 f32 范围，与现有 PDF 数字实现保持一致；中间运算仍使用 f64。补 100 项操作数栈接受、99 项容量拒绝、过大字面量拒绝。session 52277 终态退出 0：539 单元、22 流程通过，0 失败/忽略，日志 `/tmp/rzpdf-type4-boundaries-final.log`。结果更新前述 Type4 边界待完成状态；Skia Type4 专项、独立阅读器、全工作区和完整操作级聚合预算仍未完成，P4 不勾选；未运行 fuzz/自动提交。

- 2026-10-07 Type4 求值与颜色流程：接入 40 个运算符及 true/false，支持显式执行帧、整数/实数/布尔/过程类型，整数溢出提升实数、零除/未定义计算拒绝、copy/roll 工作量计费、每次执行独立栈及深度/步骤/存储限额。公共 eval 校验有限输入、Domain 裁剪、精确输出数量/数字类型与 Range 裁剪后转 f32；f64 中间值允许超过 f32 范围。间接参数严格重开从旧 UnsupportedFeature 预期升级为 add 实际求值。新增运算符/错误/额度与公共裁剪单元、Separation/DeviceN 条件函数路径/图像绿红及两保存策略严格读回。session 71459 退出 0：538 单元、22 流程通过，0 失败/忽略，日志 `/tmp/rzpdf-type4-color-final.log`。字面量/100 项栈边界、Skia Type4 专项、新全工作区及聚合预算仍待完成；未运行 fuzz/自动提交，P4 未完成。

- 2026-10-07 Type4 程序模型阶段：新增 calculator 私有模块，按 PDF 数字/布尔/注释/花括号语法编译为扁平指令和过程区间，以显式帧限制嵌套，限定规定运算符并拒绝一般 PostScript 语法。构造保留程序及额度配置；session 17097 退出 0：533 单元、21 颜色流程通过，日志 `/tmp/rzpdf-type4-compiler.log`。后续已编写有界显式执行帧、数值/逻辑/栈/条件运算代码，编译检查仍进行中，尚未接入公共 eval 或完成求值测试；Type4/P4 不勾选，未运行 fuzz/自动提交。

- 2026-10-07 扩展专色像素后的 render 全量完成：session 67031 终态退出 0，33 通过、0 失败/忽略（53.78 秒），日志 `/tmp/rzpdf-spot-image-render-final.log`。包含普通专色/None 扩展，PDF 最终为 530 单元、21 流程通过。没有新增函数数目，不把 12 个参数组合重复计为 12 个测试；独立阅读器及新全工作区门禁仍待完成，完整 P4 未完成；未运行 fuzz/自动提交。

- 2026-10-07 普通专色/None 图像像素专项通过：扩展已有校准图像 Skia 测试为 Lab、ICC、Separation Spot、六分量混合 None DeviceN、六分量全 None DeviceN、Separation None，共 12 个默认/反转组合。检查明确灰度、None 全白、原 Image 对象保留及两保存策略严格读回全页一致。session 9040 终态退出 0：1 通过、32 filtered，日志 `/tmp/rzpdf-spot-image-pixels.log`。仅局部格式整理，测试函数总数仍 33；更新此前专色像素待补齐记录。独立阅读器、原生色版/叠印与完整 P4 未完成；未运行 fuzz/自动提交。

- 2026-10-07 非设备图像错误/预算验收：新增 Decode 类型/数量/非数值拒绝、null 默认、空/超长采样拒绝、端点 15 字节拒绝/16 字节接受、保留端点容量计数与 RGBA 限额用例。session 22664 终态退出 0：530 单元、21 流程通过，0 失败/忽略，日志 `/tmp/rzpdf-spot-image-budget-final.log`。此前专色改动的 render session 41069 也已退出 0：33 通过（52.16 秒），日志 `/tmp/rzpdf-spot-image-render.log`；无新专色像素专项。本条更新下方待验收状态，完整 OperationBudget、专色像素与新全工作区仍待完成；未运行 fuzz/自动提交。

- 2026-10-07 普通 Separation/DeviceN 图像：接入高精度 packed 采样路径，Decode 端点改为可失败预留的动态数组，按分量数和 decoded-byte 限额检查，保留数组容量计入 display payload；设备/Indexed/SMask 固定端点辅助入口显式拒绝超过四分量。新增六分量 DeviceN（一位、两行填充、混合 None 名称、第六分量反转、两保存策略严格读回）与 16 位 Separation 反转用例。session 81024 终态退出 0：529 单元、21 颜色流程通过，0 失败/忽略，日志 `/tmp/rzpdf-spot-image-regressions.log`。仍仅 tint-transform/Alternate 显示近似，专色色版/叠印、额外错误/额度验收、Skia 新专项及新全工作区尚未完成；未运行 fuzz/自动提交，P4 保持未完成。

- 2026-10-07 普通 Lab/ICC 像素专项后的 render 全量完成：session 99155 终态退出 0，33 测试通过、0 失败/忽略（48.66 秒），日志 `/tmp/rzpdf-calibrated-image-render-final.log`。本条取代下方该 session 待终态说明，包含新像素专项；当前 PDF 最终回归为 529 单元、20 颜色流程通过。新全工作区、独立阅读器、CalGray/CalRGB 专项及完整 P4 仍未完成；未运行 fuzz/自动提交。

- 2026-10-07 普通 Lab/ICC 图像像素专项：新增 Skia `calibrated_image_pixels_and_reverse_decode_survive_save`，四组合覆盖 Lab 黑白、ICC 64/191 灰度的 16 位默认/反转，明确内部像素值及两保存策略严格读回全页逐字节一致。专项 session 42740 退出 0：1 通过、32 filtered，日志 `/tmp/rzpdf-calibrated-image-pixels.log`。仅局部格式整理新增测试，未整文件重排；包含新增测试的 render 全量 session 99155 已启动，日志 `/tmp/rzpdf-calibrated-image-render-final.log`，待终态。不是独立阅读器对照，CalGray/CalRGB 专项和完整 ICC 尚未验收；未运行 fuzz/自动提交。

- 2026-10-07 校准颜色图像的 Skia 既有回归完成：session 28900 终态退出 0，32 测试通过、0 失败/忽略（47.28 秒），日志 `/tmp/rzpdf-calibrated-image-render.log`。本条更新下方对应实现记录的“Skia 回归待完成”；仅验证既有用例没有回退，未新增普通校准图像的 Skia 像素专项，不能冒充新空间视觉验收。架构文档补充 packed 保留与 data()/RGBA 表示边界；仍需新增像素专项、独立阅读器及全工作区验证。

- 2026-10-07 普通 CalGray/CalRGB/Lab/ICCBased 图像：保留 packed 载荷及位深/Decode，RGBA 转换逐行读取 1/2/4/8/16 位并以 f64 插值后进入颜色转换。Lab 默认 Decode 使用 L/a/b 范围，ICC 使用 Range，仍走 Alternate 近似。新增 Lab 默认范围、16 位 CalGray 反转、CalRGB 行填充单元；真实 Lab/ICC 图像资源增加 16 位默认/反转及两保存策略严格读回。最终 session 6230 退出 0：529 单元、20 流程通过，0 失败/忽略，日志 `/tmp/rzpdf-calibrated-image-final.log`。Skia 回归、独立阅读器、完整 ICC、Separation/DeviceN 普通图像和新全工作区仍待完成；未运行 fuzz/自动提交，P4 未完成。

- 2026-10-07 Indexed 图像转换缓存：RGBA 转换使用单次调用局部的 256 槽固定栈数组，仅对出现过的采样索引求值；重复索引不再逐像素分配分量数组、求值 tint-transform，未用表项不提前转换，缓存不跨图像/调用持久化。既有低位深/Decode 单元追加跨行重复索引用例。`cargo test -p pdf --lib --test color_workflow --locked --quiet` 终态退出 0：528 单元、20 流程通过，0 失败/忽略，日志 `/tmp/rzpdf-indexed-palette-cache.log`；差异检查通过。未建立性能基准或全链路预算验收，不能勾选 P5-06/AR-03；此前 812 全工作区结果早于本缓存修改，未运行 fuzz，未自动提交。

- 2026-10-07 颜色/图像阶段全工作区门禁完成：下方 session 3702 已终态退出 0，`cargo test --workspace --locked --quiet` 日志 `/tmp/rzpdf-workspace-image-colors.log` 共 45 条结果，812 测试通过、0 失败/忽略，包含 pdf 单元 528、39 个集成目标及 render 32；文档测试正常结束。源差异指纹在提交前保持一致，随后本地提交为 `853ae38`，提交动作未修改被测代码。启动记录的“尚待终态”由本条结果取代；历史专项记录的未运行全量仅描述各自当时状态。未运行 fuzz，远端 CI 未验证，P0-01/P4/整体目标仍未完成；未推送，未知 `test.png` 保持未跟踪。

- 2026-10-07 颜色/图像阶段全工作区门禁启动：此前工作区 792 测试记录早于后续 DeviceN/None/stencil/Indexed 改动，不能替代当前版本全量结果。启动 `cargo test --workspace --locked --quiet`，进程 session 3702，日志 `/tmp/rzpdf-workspace-image-colors.log`，尚待终态；期间冻结生产代码，不重复启动。workspace 仅 pdf/render/font-data，fuzz 项目不在成员内，不运行 cargo-fuzz。当前 HEAD `5fd7f56`，pdf/src/render/src/pdf/tests 的工作区差异 SHA-256 `732f29e15d894cba31f088155e5c9f42604e2542e3c5b212a4c66466652596dd`；Darwin x86_64，rustc/cargo 1.96.0，39 个 pdf 集成目标。差异检查通过；现有未使用项和 cargo/config 弃用警告保留。本次启动不是通过声明，远端 CI 仍未验证，P0-01/P4/整体目标保持未完成，未自动提交。

- 2026-10-07 Indexed DeviceN/None 绘制：移除 Indexed RGB 转换对 DeviceN 基空间的旧一概拒绝，完整调色板分量经现有 tint-transform/替代空间显示近似；混合 None 分量仍传入函数，不误判为整体不绘制。Indexed::is_nonpainting 转发基空间，ColorSpace 纳入 Indexed，复用统一 PaintingDevice 抑制对应路径/文字/普通图像而不丢采集对象。新增双输入 Sampled 调色板的路径颜色与两位图像 RGBA（白/青/品红/蓝）、Cyan/Magenta 与 Cyan/None 两组合、两保存严格读回；新增 Separation None/全 None DeviceN 基空间三对象保留、路径/图像抑制和后续红色路径。已有 Skia None 像素测试追加两种 Indexed 包装，仍为同一测试函数，覆盖文字保留与组合绘制可见侧。最终 pdf 单元 528、color_workflow 20 全部通过（退出 0，0 失败/忽略），render 全量 32 通过（退出 0，0 失败/忽略，48.47 秒）；日志 `/tmp/rzpdf-indexed-devicen-none.log`、`/tmp/rzpdf-indexed-none-render.log`。README/架构/测试同步，差异检查通过；新 Indexed 图像独立像素/阅读器、原生专色/叠印、Type4 及完整 P4/聚合预算仍未完成；未运行全工作区/fuzz、未自动提交。

- 2026-10-07 Indexed Lab/ICC 基空间范围修正：旧实现统一将调色板字节除以 255，错误忽略 Lab L=0..100、a/b Range 与 ICCBased 声明 Range。新增 crate 内 Lab 固定端点查询，Indexed 转换读取基空间范围、检查长度、可失败预留分量缓冲，并以 f64 中间值映射后交给既有转换；ICCBased 仍走 Alternate 近似，非零 Lab BlackPoint 仍拒绝。新增 Lab 黑白/非对称 a/b 范围与 ICC 灰度/RGB 非默认范围字节测试；真实图像资源注册/页面解释及 Preserve/FlateUnencoded 严格保存读回保持 Lab 黑白、ICC 64/191 灰度。Lab 专项退出 0：1 通过/526 filtered（早于 ICC 新增）；最终 `cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0：528 单元、18 流程全部通过，0 失败/忽略，日志 `/tmp/rzpdf-indexed-base-ranges-final.log`。render 全量回归终态退出 0：32 通过、0 失败/忽略（47.23 秒），日志 `/tmp/rzpdf-indexed-base-render.log`。README/架构/测试说明同步，差异检查通过；独立阅读器/新 Skia 像素、完整 ICC/OutputIntent、操作级聚合预算与 P4 尚未完成，未运行 fuzz/全工作区、未自动提交。

- 2026-10-07 Indexed 图像索引解码：复用逐行解包/精确长度/可失败分配检查，新增索引模式（默认 Decode [0, 2^bits-1]，非灰度归一化），显式 Decode 以索引单位映射后取整/裁剪，RGBA 经 Indexed 调色板转换。DeviceRGB 单元覆盖 1/2/4/8 位及 legacy 16 位、低位深两行填充、反转、越界端点与数据长度错误；页面 Image 资源流程覆盖两行反转及两种保存策略严格读回相同 RGBA。首轮专项 1 通过/525 filtered，早于 16 位追加；首轮回归 526 单元、17 颜色流程通过；16 位追加后的最终 `cargo test -p pdf --lib --test color_workflow --locked --quiet` 终态退出 0：526 单元、17 流程全通过，0 失败/忽略，日志 `/tmp/rzpdf-indexed-image-final.log`。差异检查通过，README/架构/测试文档同步。Lab/ICC 等调色板基空间精度、其他非设备图像颜色空间、Skia 专项/独立阅读器像素、PDF 2.0 合规与聚合预算仍未完成；未重跑 render/全工作区、未运行 fuzz、未自动提交，P4 仍未勾选。

- 2026-10-07 Stencil ImageMask 基础闭环：支持一位 XObject、默认 BitsPerComponent=1、逐行填充位与 Decode [0 1]/[1 0]，禁止 ColorSpace/SMask 及其他位深；当前非描边颜色与透明覆盖分离，rgba_buffer_with_stencil_color 校验有限 RGB，普通 rgba_buffer 对 mask 明确报错。Skia 按当前填充色生成 RGBA，None 适配层抑制真实 mask 而保留对象。单元覆盖反转、红/绿复用、非法参数/数据长度与额度不足；集成覆盖 None 后红色绘制、原两个对象保留及两种保存严格读回；Skia 像素覆盖蓝背景上的红/绿复用、反转、None 与保存读回逐字节一致。专项单元退出 0：1 通过/524 filtered；专项 render 退出 0：1 通过/31 filtered。首轮完整单元旧 ImageMask UnsupportedFeature 预期失败，更新为带 ColorSpace 的非法 stencil 应报 ObjectError 后，最终 pdf 单元 525、color_workflow 16 全部通过（退出 0，0 失败/忽略），日志 `/tmp/rzpdf-stencil-regressions-final.log`；render 全量终态退出 0：32 通过、0 失败/忽略（52.69 秒），日志 `/tmp/rzpdf-stencil-render-all.log`。README/架构/测试文档同步。Pattern 着色、内联图像、独立阅读器像素对照、非设备颜色空间图像和完整 P4 未完成；未重跑全工作区、未运行 fuzz、未自动提交。

- 2026-10-07 None 直接执行与真实像素验证：新增 None 填描组合直接 Interpreter::run 与 DisplayList::replay 的回调一致性，原 4 个对象保留、只发出 stroke/fill/fill 三次绘制。新增 Skia 像素回归覆盖 Separation /None 与全 None DeviceN：路径/文字全白、Hidden 仍提取、组合绘制保留红描边/绿填充且抑制侧中心保持白色，Preserve/FlateUnencoded 严格保存读回像素完全一致。专项 pdf 1 测试通过（523 filtered），render 1 通过（30 filtered），随后 `cargo test -p render --lib --locked --quiet` 终态退出 0、31 测试全部通过，0 失败/忽略；差异检查通过，渲染新增测试仅局部格式整理，无生产代码变化。日志 `/tmp/rzpdf-none-direct-dispatch.log`、`/tmp/rzpdf-none-render-pixels.log`、`/tmp/rzpdf-none-render-all.log`。是本库 Skia 后端像素验证，不是独立阅读器对照；stencil 解析/绘制、普通非设备颜色空间图像、文字裁剪后端及完整 P4-03/P4-04 仍未完成；未重跑完整 pdf 单元/全工作区或 fuzz。

- 2026-10-07 None 统一绘制分派：新增 PaintingDevice，直接 Interpreter::run 与 DisplayList::replay 共用；Separation /None 与全 None DeviceN 通过 ColorSpace::is_nonpainting 抑制路径对应填充/描边，填描组合保留可见侧，文字八种模式保留裁剪位并转换有效绘制，普通图像使用自身空间。run_to_display_list 未过滤采集以保留原始对象、文字/来源，不修改内容。首轮集成暴露既有 stencil image masks UnsupportedFeature；仍保留该限制，改为验证普通图像不随 None 填充色消失，未将 mask 接入判据冒充真实 mask 文件支持。新增原对象 13 个/文字 A×8 保留、组合路径、文字八模式、普通图像及两保存策略读回；旧 None 错误测试升级为跳过后继续绘制/空颜色成功回放。最终 `cargo test -p pdf --lib --test color_workflow --test resources --locked --quiet` 退出 0：523 单元、15 颜色流程、14 资源测试通过，0 失败/忽略；差异检查通过，日志 `/tmp/rzpdf-none-paint-dispatch.log`。渲染像素、直接解释 None 专项一致性、真实 stencil 解码/绘制和文字裁剪后端仍待验证/实现；P4-03/P4-04 未完成，此前全工作区门禁早于本次生产修改，未运行 fuzz。

- 2026-10-07 Separation 特殊色名显示策略：/All 在有限单 tint 检查/裁剪后忽略替代空间和函数，返回 1-tint 灰度显示近似；/None 的直接 rgb 返回 UnsupportedFeature，不再错误绘制替代颜色。新增以尚未支持执行的 Type4 为合法维度 tint-transform，验证 All 确实不求值，填充/描边/裁剪/严格保存读回结果一致；None 失败在任何颜色输出前传播。最终 `cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0：523 单元、14 颜色流程测试通过，0 失败/忽略；差异检查通过，日志 `/tmp/rzpdf-separation-special-colorants.log`。已检查 DisplayList/RenderDevice：设备级 None 应跨路径、文字（保留裁剪）、图像及填描组合统一处理；本轮仅 RGB 控制，尚非完整 None 支持，原生分色/叠印也未实现。P4-03 保持未完成，未重跑全工作区或 fuzz。

- 2026-10-07 Separation 构造一致性：低层构造器要求 /Separation 家族标记，tint-transform 维度必须为单输入/替代颜色分量数，避免注册成功后才在绘制时暴露错误。新增错误家族低层回归，RGB/灰度输出错误和双输入 Type4 函数注册连续失败后保存字节不变，合法注册仍使用 CS1，灰度页面严格保存读回一致。最终 `cargo test -p pdf --lib --test color_workflow --test resources --locked --quiet` 退出 0：523 单元、13 颜色流程、14 资源测试通过，0 失败/忽略；差异检查通过，日志 `/tmp/rzpdf-separation-dimensions.log`。Type4 fixture 仅验证参数维度拒绝，不实现执行器；Separation All/None 的设备绘制语义、原生专色及 OperationBudget 仍待完善，P4-03 未完成。此前 792 项门禁早于此修改，未重跑全工作区或 fuzz。

- 2026-10-07 Type2 宽有限系数求值：依据 Adobe Reference 1.3 §3.9.2，使用 f64 中间幂/系数差/插值，先 Range 裁剪再转回 f32；常量分量直接输出，避免巨大幂×0 产生 NaN。NaN 或无有效裁剪且无法表示为有限 f32 的输出仍 FunctionError。新增 ±300000000000000000000000000000000000000 系数的端点/中点、2^128 与 2^2000 的 Range 裁剪、巨大幂常量输出、无 Range 真正超范围错误及恢复。原 Type3 错误传播 fixture 曾把有限系数差的 f32 中间溢出当作错误，现改为输出真正超过 f32 范围，未删除错误传播断言。最终 `cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0：522 单元、12 颜色流程测试通过，0 失败/忽略；差异检查通过，日志 `/tmp/rzpdf-type2-wide-evaluation.log`。不宣称任意精度、所有函数/独立视觉/OperationBudget 已完成；此前全工作区门禁早于此修改，无 fuzz。

- 2026-10-07 Type2 参数数组分配前检查：Domain 首先要求两项，Domain/C0/C1/Range 逐数组检查 max_objects 与 len×sizeof(f32) 的 max_decoded_bytes，再 try_reserve_exact；求值输出改为可失败预分配，防止按未知参数数量逐项无限增长。新增 8 字节精确通过/7 字节拒绝、三项 C0/C1/Range 超节点/字节限额、反复失败恢复和错误 Domain 结构回归。最终 `cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0：521 单元、12 颜色流程测试通过，0 失败/忽略；差异检查通过，日志 `/tmp/rzpdf-type2-parameter-limits.log`。普通 resolver 的对象/数组副本仍先于数值数组额度检查；默认值小数组、源缓存及多参数/输出共存未贯通 OperationBudget，不据此完成 P0-10/AR-03/P4-03。此前 792 项门禁早于此修改，未重跑全工作区或 fuzz。

- 2026-10-07 DeviceN 家族标记/额度精确边界：低层 DeviceN::try_new 现在先解析首项并要求 /DeviceN，拒绝错误家族、数值/null/字符串；间接家族标记修改后重新构造及时拒绝。新增名称槽位预算恰好足够/少一字节、累计名称字节恰好足够/多一字节、组件数量超限及重复失败后恢复回归。最终 `cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0：520 单元、12 颜色流程测试通过，0 失败/忽略；差异检查通过，日志 `/tmp/rzpdf-devicen-constructor-limits.log`。上一 792 项全工作区门禁早于本次生产修改，不冒充最新门禁；这些是局部数量/字节检查，不是聚合 OperationBudget，P0-10/P4-03 未完成，fuzz 暂缓。

- 2026-10-07 颜色/函数后全工作区门禁终态：同一次 `cargo test --workspace --locked --quiet` 已完成、退出 0，未在观察超时后重启。完整日志逐条求和：PDF 单元 518、39 个 PDF 集成目标共 244、render 单元 30，总计 792 通过，0 失败/忽略；45 条结果含四个零测试目标。包含当前 DeviceN、Lab/Type3、颜色注册/导入、函数/颜色最低版本及编辑/保存/渲染回归。日志 `/tmp/rzpdf-workspace-colors-functions.log`；rustc/cargo 1.96.0，现有 macOS 本地环境。过程更新中的手工累计数被最终完整日志汇总取代。差异检查通过，未改运行中的代码、未新增提交、未运行独立 fuzz 工程或远端 CI。仅完成本地工作区门禁，不证明真实文件完整兼容性、独立视觉矩阵、跨平台或全 TODO 验收；P0-01/P4-03 等保持未完成。

- 2026-10-07 DeviceN 间接/循环恢复回归：覆盖名称数组、分量名称、替代空间、tint 函数、属性字典及 Subtype 的间接字段；名称自引用与颜色数组经替代空间形成循环连续两次失败后，修复对象可重新解析，旧不可变颜色模型仍可用，保存严格读回转换结果一致。首次测试误把一层 resolver 拒绝的“引用指向引用”期待为深度错误，已分开验证 alias 的 ColorError 与实际颜色数组循环的 ResourceLimit，不改变现有普通 resolver 语义。最终 `cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0：518 单元、12 颜色流程测试通过，0 失败/忽略；差异检查通过，日志 `/tmp/rzpdf-devicen-indirect.log`。本轮只增回归，仍未证明精确额度、OperationBudget、完整 NChannel 属性或独立视觉验收；P4-03 未完成，无 fuzz。

- 2026-10-07 DeviceN 替代空间显示路径：替换空占位结构，接入 ColorSpace 解析/rgb/default/components 与资源校验。校验四/五项数组、非空名称、重复名称（None 例外）/All、属性 Subtype、NChannel 禁止 None、非特殊替代空间和 tint 函数维度；名称数量/字节限额及可失败分配，有限 tint 按 0..1 裁剪。全 None 明确 UnsupportedFeature，避免错误输出颜色；NChannel 使用规范允许的 tint-transform 近似，不声称原生专色/叠印/自定义混色。新增单分量查询/异常 tint 单元及双分量 Sampled→RGB 页面默认值/裁剪/两保存策略严格读回、错误注册回滚及全 None 错误传播集成。初次编译发现原占位结构单元与新 API 不匹配、测试误用资源对象为颜色模型，已修正；最终 `cargo test -p pdf --lib --test color_workflow --test feature_versions --locked --quiet` 退出 0：517 单元、12 颜色、15 版本测试通过，0 失败/忽略；差异检查通过，日志 `/tmp/rzpdf-devicen-workflow.log`。OperationBudget、更多间接/循环/精确额度回归、独立视觉对照及设备级 None 抑制仍待完成；P4-03 未完成，fuzz 暂缓。

- 2026-10-07 嵌套颜色基空间版本预检：有界迭代跟随 Indexed/I、Pattern、Separation、DeviceN 的基空间引用；Indexed 使用 Separation 基空间需 PDF 1.3（Adobe Reference 1.3 §4.5.5）。解析副本在普通/预算入口共用 resolver，预算租约覆盖当前与下一副本共存；max_depth/max_objects 限制遍历，循环返回 ResourceLimit。新增 Indexed→ICCBased/Separation/DeviceN 的低版本拒绝及升级严格读回，Indexed 引用自循环拒绝时 writer 输出/revision 不变，修复后再次保存成功。`cargo test -p pdf --lib --test feature_versions --test color_workflow --locked --quiet` 退出 0：516 单元、15 版本、10 颜色流程测试通过，0 失败/忽略；差异检查通过。日志 `/tmp/rzpdf-nested-colorspace-version.log`。仍非完整合法性验证：ICCBased Alternate、DefaultRGB 等替代空间、DeviceN 属性及内容操作符尚未全面审计；高版本可跳过定向检查，不据此宣称循环颜色合法。P1-08/P4-03 未完成，未运行 fuzz。

- 2026-10-07 ColorSpace 字段最低版本检查：普通/预算保存共享预检解析直接/间接 ColorSpace 值及资源表项，Pattern/Separation 要求 PDF 1.2，ICCBased/DeviceN 要求 PDF 1.3。版本用例覆盖四种家族、资源表与单值字段、引用和显式升级，低版本拒绝时 writer 输出及 revision 不变，升级后严格读回。`cargo test -p pdf --lib --test feature_versions --test color_workflow --locked --quiet` 终态退出 0：516 单元、14 版本、10 颜色流程测试通过，0 失败/忽略；差异检查通过。日志 `/tmp/rzpdf-colorspace-version.log`。用例仅验证保留字段的版本识别，不证明参数合法性、ICC/DeviceN/Pattern 渲染；嵌套基空间、DefaultRGB 等替代空间、内容操作符、无 ColorSpace 字段的独立数组仍未完整审计，P1-08/P4-03 保持未完成；fuzz 暂缓。

- 2026-10-07 函数版本预检边界回归：补充间接 Catalog /Version 1.4 覆盖 1.2 Header 时默认保存及严格读回后再次保存，确认不重写 Header、不修改 revision；未知/负数/超 u64/实数 FunctionType 仅保留，不错误舍入成已知类型或冒充函数合法性校验。含 f32 舍入为 2 的长小数与整数值实数 token 2.0、直接/间接字段。`cargo test -p pdf --test feature_versions --locked --quiet` 退出 0，13 测试通过，0 失败/忽略；差异检查通过。日志 `/tmp/rzpdf-function-version-boundaries.log`。本轮仅增加回归，无生产代码变化，无最新全工作区门禁、无 fuzz；整体任务继续。

- 2026-10-07 保存函数最低版本预检：普通/预算保存共用检查识别直接或间接 FunctionType，Type0 要求 PDF 1.2，Type2/3/4 要求 PDF 1.3；低版本必须显式升级。回归覆盖字典/流、拒绝时 writer 输出及文档 revision 不变、升级后严格读回。`cargo test -p pdf --lib --test feature_versions --test color_workflow --locked --quiet` 终态退出 0：516 单元、11 版本、10 颜色流程测试通过，0 失败/忽略；差异检查通过。仅为定向版本检查，不验证函数完整合法性或 Type4 执行，也不证明全部颜色资源版本、聚合内存预算和完整 PDF 符合性；P1-08/P4-03 保持未完成，fuzz 继续暂缓。

- 2026-10-07 直接颜色注册对象额度边界：新增max_objects=16固定文档填满对象额度的回归，已有DeviceRGB值重复注册仍复用CS1且不改修订；新DeviceGray两次注册均ResourceLimit拒绝、页资源表及原颜色保持，失败后仍可复用CS1。color_workflow10项全部通过、0失败/忽略，退出码0，git diff --check通过。本轮仅回归/文档，不改生产代码；未重跑单元/全工作区/独立视觉验证，未提交。对象数量门槛不是操作级活跃内存预算，fuzz暂缓。

- 2026-10-07 P1-03颜色空间直接构建入口：新增register_color_space(page_index, PdfObject)，校验页面/颜色空间、解析间接根，在当前继承页作用域中复用相等颜色值与确定性最小名称，否则事务化创建间接对象并注册；内部裸引用仍由调用方保证属于当前文档，跨文档使用导入/句柄入口。新增重复值/间接根复用不改修订、非法值/页面无状态修改、外层失败恢复字节与名称、短值重试及严格重开回归。pdf单元516项、color_workflow9项、resources14项全部通过、0失败/忽略，退出码0，git diff --check通过。README/测试文档同步；未重跑全工作区/独立视觉验证，未提交，fuzz暂缓；P1-03与聚合预算仍未整体完成。

- 2026-10-07 颜色空间导入失败边界：新增非法Lab白点在复制前拒绝、max_objects=4下间接参数图复制中途超限的重复失败回归；每次目标保存字节恢复、源文档不变，随后较小DeviceRGB资源导入成功且下一对象号恢复为3，严格重开保留该对象。color_workflow8项全部通过、0失败/忽略，退出码0，git diff --check通过。本轮仅集成回归/文档，未改生产代码、未重跑单元/全工作区/独立视觉验证，未提交；图节点限制不等于操作级活跃内存预算，fuzz暂缓。

- 2026-10-07 P1-03/P2-04颜色空间资源注册与导入：ResourceKind增加ColorSpace，register_existing/register_handle经ColorSpace解析校验后写入页ColorSpace类别，使用CS前缀避让名称、同对象复用，复用现有事务和共享资源表复制后修改。import_resource_from支持名称/数组颜色空间根及间接参数图，保留文档身份/版本/加密前置规则；原Font/Image/Form类型约束保持。新增名称冲突/重复绑定不改修订、共享表隔离、外文档句柄拒绝、Lab间接参数导入/源修改隔离、同文档复用、非法注册与外层失败回滚和严格重开回归3项。pdf单元516项、color_workflow7项、resources14项全部通过、0失败/忽略，退出码0，git diff --check通过。README/架构/测试文档同步；下游穷举ResourceKind需补ColorSpace分支，解析注册不代表全部颜色空间可渲染。未重跑全工作区/独立视觉验证，未提交，fuzz暂缓；P1-03/P2-04及聚合预算仍未整体完成。

- 2026-10-07 P0-08 Lab数组标识校验：直接/上下文Lab构造入口在解释参数前要求首项为Lab名称，上下文入口解析间接名称，避免将DeviceRGB/CalRGB等错误数组按Lab处理；参数字典入口保持兼容。新增名称/类型拒绝、正确直接/间接名称、低层名称修改、引用成环与恢复回归；pdf单元516项与color_workflow集成4项全部通过、0失败/忽略，命令退出码0，git diff --check通过。未重跑全工作区或独立视觉验证，前轮775项完整通过不是本次变更的全量验证；未提交，fuzz暂缓。

- 2026-10-07 共享颜色函数修订可见性：color_workflow新增源文档冷打开后两页共享间接Stitching函数的低层修改回归；先加载两页蓝色结果，修改函数后旧stamp失效、新页面读取同时变红，已有DisplayList仍是拥有蓝色数据的旧快照，保存严格重开两页保持红色。color_workflow目标4项全部通过、0失败/忽略，git diff --check通过。本轮仅新增集成回归及文档，不改生产代码；没有重跑全工作区，前轮775项通过记录不包含本次新增用例；未独立视觉验证、未提交，fuzz暂缓。

- 2026-10-07 本地完整工作区再验收：cargo test --workspace --locked --quiet同一次进程最终退出码0；pdf单元515项、39个pdf集成目标合计230项、render测试30项，合计775项全部通过，0失败/忽略，零测试目标亦正常结束。平台Darwin x86_64，rustc 1.96.0 (ac68faa20 2026-05-25)、cargo 1.96.0 (30a34c682 2026-05-25)，git diff --check通过。覆盖最新函数/颜色及保存等当前工作区回归；未运行独立fuzz工作区、未触发远端CI或独立阅读器视觉检查，未提交。编译仍有未使用项及本机cargo配置弃用警告，普通CI的Ubuntu原生依赖/远端结果未验收，P0-01及总体仍未完成。

- 2026-10-07 P4-03颜色公共工作流回归：新增color_workflow集成目标，PageBuilder页资源中的Lab/Separation-Stitching经内容解释和DisplayList设备回放校验灰/蓝/白填充描边、q/Q颜色恢复；Preserve/FlateUnencoded保存严格重开颜色保持，复制页编辑不影响原页，不支持的非零BlackPoint在设备回放时停止后续绘制。首次编译误给按值SaveOptions传引用，修正后3项全部通过、0失败/忽略，退出码0，git diff --check通过。本轮生产代码未改，未重跑单元/全工作区或独立阅读器视觉验证，未提交；低层RGB转换与自定义设备测试不代表完整ICC/渲染验收，P4-03仍未完成，fuzz暂缓。

- 2026-10-07 P0-08/P0-10 Lab参数路径收口：上下文构造复用校准颜色固定数组读取器，在3/4项长度检查后再解析成员，不再逐成员重建PdfArray及回写参数字典；直接/上下文入口共享白点/黑点/Range验证。新增畸形长度数组包含循环引用时优先ColorError，以及两个入口默认值/合法参数/错误验证一致回归2项；最终pdf单元515项全部通过、0失败/忽略，git diff --check通过。普通resolve仍可能分配拥有副本，本项不是完整OperationBudget；未运行集成目标/全工作区/独立视觉验证，未提交，fuzz暂缓。

- 2026-10-07 P4-03 Lab颜色转换：零BlackPoint的Lab已从明确不支持改为L/a/b域裁剪、CIE Lab到XYZ、Bradford白点适配到D65、sRGB传递及通道裁剪，全部矩阵/中间值使用f64栈数组；非零BlackPoint补偿仍明确UnsupportedFeature，不可适配白点返回ColorError。新增5项灰阶/D50-D65及等能白点、RGB主色、输入/Range裁剪、异常与间接颜色空间保存严格重开回归；最终pdf单元513项全部通过、0失败/忽略，git diff --check通过。README/架构/测试文档记录固定显示策略与未完成边界；未运行独立阅读器/视觉验证、集成目标或全工作区，未提交。ICC意图/BlackPoint补偿/感知色域映射及CalRGB/CalGray策略统一仍待实施，P4-03保持未完成，fuzz暂缓。

- 2026-10-07 P0-08/P0-10 Type4参数构造补充：Domain/Range及数组成员均解析间接引用，非空成对/有限有序、数组条目和请求字节上限在函数流解码前检查，分配使用try_reserve_exact；明确不支持求值的路径不再无用分配100槽位。新增间接参数保存严格重开、非法参数优先拒绝、精确字节/条目门槛、解码失败重试及循环引用回归4项；首轮误将未知滤镜错误断言为UnsupportedFeature，已按既有FilterError契约修正。最终pdf单元508项全部通过，0失败/忽略，git diff --check通过；README/测试文档同步。Type4代码仍未解析/执行，参数与解码仍是单项Limits而非操作聚合预算，P0/P4主项不勾选；未运行集成目标/全工作区/独立阅读器，未提交，fuzz暂缓。

- 2026-10-07 P4-03 Stitching函数实现与P0构造校验补充：Type3从明确不支持改为单输入分段求值，Domain裁剪、右侧区间边界、反向Encode、可选Range裁剪、嵌套子函数及末界等于Domain上界的单点区间已实现；插值采用f64中间计算。内部Function维度查询使构造时拒绝多输入子函数、输出维度不匹配和非法Range，保留深度/总节点预算。新增6项函数回归与1项Separation RGB转换保存严格重开回归；首轮宽域样本误用PDF不支持的指数数字语法，已改为完整十进制，最终pdf单元504项全部通过，0失败/忽略，git diff --check通过。README/测试文档同步；未重跑独立渲染、集成目标或全工作区，未提交。Type4求值、Shading/Pattern及函数聚合内存仍未完成，P0/P4主任务保持未完成，fuzz暂缓。

- 2026-10-07 本地提交前验证：pdf单元497项，以及page_transfer_edit、page_rotation、page_revision、create_document、stream_edit、stream_structure、feature_versions、structural_rewrite、owned_document九集成目标合计42项全部通过，0失败/忽略，命令退出码0；git diff --check通过。提交本轮保存预算、原子保存清理、页面组合回归及计划/架构/测试文档，不包含来源未确认的test.png，不推送远端。未运行全工作区测试及fuzz，AR-03与总体目标仍未完成。

- 2026-10-07 页面转移组合回归：新增“重排 → 跨文档导入 → 复制页编辑 → 严格重开”测试，验证CropBox/Rotate/UserUnit对应的页面尺寸、字符位置及书写轴保留；无效重排与旧内容快照拒绝不改状态，编辑副本不影响源文档和其他页。仅覆盖固定Helvetica样本，不代表P2或完整继承属性矩阵验收，fuzz仍暂缓。

- 2026-10-07 公共保存交付路径与编辑输出独立验证：create_document、stream_edit、stream_structure、feature_versions、structural_rewrite、owned_document六集成目标共31项通过、0失败/忽略。现有edit_content示例生成独立两页原/编辑对照，qpdf 11.2.0结构/流编码与页数通过，MuPDF 1.27.0提取Original/Edited content；按PDF技能逐页核对渲染，复制原页不变，编辑页文字替换、图片旋转/单次删除及红路径绘制删除符合预期，无明显视觉缺陷。记录与产物见docs/testing.md。Poppler缺少libtiff.5.dylib，使用既有MuPDF回退、未改系统依赖。本轮未重跑单元/全工作区测试、未提交；固定Helvetica/顶层编辑样本不等于完整P1/P3矩阵，AR-03/AR-04与总体仍未完成，fuzz暂缓。

- 2026-10-07 P1-08原子保存unwind释放顺序：File局部绑定调整到临时路径守卫之后，使正常/Err显式关闭与Rust unwind自动关闭均先于路径清理，避免panic时先尝试移除仍打开文件。新增新建/已有目标×写入/同步/实际替换前panic六组合，验证到达阶段、目标原字节或不存在状态保持及临时文件清理。pdf单元497项全部通过、0失败/忽略，git diff --check通过；本轮未重跑集成/全工作区测试、未提交。只验收本地panic=unwind，未验收成功rename后撤销、abort/断电、真实磁盘故障或Windows，P1-08及整体仍未完成，fuzz暂缓。

- 2026-10-07 P1-08原子保存同步/替换故障回归：内部动作拆分为write/sync/replace，默认仍调用原write_all/sync_all/rename，保留写句柄先关闭再清理/替换顺序。新增新建/已有目标与sync/replace模拟失败四组合，验证同步失败不调用替换、替换前临时字节已完整、旧文件不变/新目标未创建以及临时文件清理。pdf单元496项全部通过、0失败/忽略，git diff --check通过。本轮未重跑集成/全工作区测试、未提交；模拟错误不代表真实磁盘故障、断电或Windows/并发验收，P1-08与AR-03仍未完成，fuzz暂缓。

- 2026-10-07 预算字节转移入口统一：对象流现有异池拒绝/清理路径已检查，保存侧重复into_owned_parts移除并复用into_reserved_parts；内部转移保持载荷与MemoryLease一起移交，不作为公共裸Vec导出。新增扩容后地址/capacity/共享池身份保持、重新包装无复制/不中断额度、释放与转移后unwind归零回归。pdf单元495项全部通过、0失败/忽略，git diff --check通过。本轮未重跑集成/全工作区测试、未提交；原语与固定调用回归不表示整个AR-03完成，fuzz继续暂缓。

- 2026-10-07 预算字典构建器同池契约修复：insert在查找/覆盖/扩容前检查键/值租约属于构建池，拒绝异池（含0字节租约），共享OperationBudget克隆仍可使用；入参按载荷先于租约的条目顺序拥有，异池、条目上限与扩容失败统一先释放数据再释放额度，覆盖亦先销毁旧值。新增异池键/值的插入及覆盖拒绝、原绑定与双方额度/邻居保持、零租约拒绝、共享克隆插入及finish清理回归。最终pdf单元494项及content_edit_transaction、nested_text_edit、nested_path_edit、nested_image_edit集成19项全部通过、0失败/忽略，git diff --check通过。本轮未运行全工作区测试或fuzz、未提交，AR-03仍未完成。

- 2026-10-07 Compressed/ObjStm完整内部保存组合回归：复用结构重写样本构造方式，合法xref流中Catalog/Pages/Page/64 KiB字符串四对象压缩存于ObjStm，覆盖原始/Flate输入与两种保存策略。冷保存小额度拒绝后根/目标/源流缓存未发布、额度归零且修订号不变；更新目标并改用充足额度池后，完整保存字节与独立普通实例一致，源缓存展开不覆盖编辑视图，严格重开保留页面及修改值，释放结果后仅持久缓存保留。pdf单元493项及structural_rewrite集成3项全部通过、0失败/忽略，git diff --check通过。本轮未重跑其他集成或全工作区测试、未提交；兼容性/持久缓存总量/后端私有内存未完整验收，AR-03未完成，fuzz暂缓。

- 2026-10-07 普通xref冷源保存预算验收：本库生成的合法保存字节严格打开，未被引用64 KiB源流的缓存状态通过仅测试观测方法确认。小额度保存拒绝后该大对象未发布、额度归零/修订号不变；Preserve/Flate充足额度重试与独立普通保存一致，输出仅保留capacity额度，源缓存转为持久内存后仍可保留并再次保存/读取页面一致。pdf单元492项及content_edit_transaction、nested_text_edit、nested_path_edit、nested_image_edit集成合计19项全部通过、0失败/忽略，git diff --check通过。本轮未运行全工作区测试、未提交；Compressed/ObjStm完整保存及持久总量/后端私有内存未验收，AR-03未完成，fuzz暂缓。

- 2026-10-07 压缩暂存工作区计费：核对锁定flate2 1.1.9/miniz_oxide 0.8.9源码，预算压缩改用低层Compress与显式32 KiB固定容量Vec，预留在分配前并校准实际capacity，避免高层Writer隐藏暂存Vec；total_in/out检查推进与完成，输出仍同池几何扩容。新增空输入、全字节和跨暂存区的固定计数数据普通/预算字节一致、解压读回、工作区门槛/峰值少1拒绝及输出长度拒绝释放。最终pdf单元491项全部通过、0失败/忽略，git diff --check通过。后端CompressorOxide/Huffman/LocalBuf私有状态仍未全面计费，不能以外层size_of代替；AR-03未完成。本轮未重跑集成/全工作区测试、未提交，fuzz暂缓。

- 2026-10-07 完整内部保存结果组合预算验收：固定内存文档在Preserve/Flate策略下，结果capacity与本次实测保存峰值合计少1拒绝第二次保存、保留首结果额度和字节；释放首结果后同池重试恢复，充足额度下双结果共存/逐份释放，文档修订号与普通保存字节不变。该门槛不推算其他文档/冷源/HashMap布局或编码器未计费工作区。pdf单元490项及content_edit_transaction、nested_text_edit、nested_path_edit、nested_image_edit集成合计19项全部通过、0失败/忽略，git diff --check通过；本轮未运行全工作区测试或fuzz、未提交。AR-03仍未完成，继续补计费边界及其他阶段验收。

- 2026-10-07 保存格式化临时缓冲收口：Header、对象头、xref段头/定宽条目、startxref及对象引用使用64字节StackFormat和format_args，不再新增format!临时String；引用长度预检共用栈格式化，栈token完成后单次追加输出。新增u32/u64最大数值、容量不足输出不变、xref极值字节一致/结果额度及非法偏移/generation/状态写前拒绝回归。pdf单元489项全部通过、0失败/忽略，git diff --check通过。本轮未重跑集成/全工作区测试、未提交；编码器私有内存、诊断/路径缓冲与持久增长仍未全面计费，AR-03未完成，fuzz暂缓。

- 2026-10-07 保存压缩输出预算接入：Filter/DecodeParms引用链同池解析，未编码流使用预算压缩输出适配器，几何容量增长计入旧新共存；FlateDecode名称、替换字典及压缩载荷租约保留至写出，编码流/外部流仍按原规则保留。新增间接null设置、普通/预算压缩对象一致、大字典返回额度保留、峰值少1拒绝/unwind清理及外部流保留回归，原保存选项字节一致回归继续通过。pdf单元488项全部通过、0失败/忽略，git diff --check通过。本轮未重跑集成/全工作区测试、未提交；编码器Rust/原生私有工作区、格式/诊断小缓冲和持久增长未全面计费，AR-03未完成，fuzz暂缓。

- 2026-10-07 保存有效版本预算接入：新增内部effective_version_with_budget，当前trailer Root借用入口、Catalog拥有副本与Version字段拥有解析同池，租约覆盖合并计算；普通/预算共用Header/Catalog取较大版本、null及类型/未知错误规则。新增间接Version、Catalog大字段副本峰值少1拒绝、原对象/修订号不变、null/低版本/未知名称/数值错误一致以及预算/普通完整保存字节一致回归。修正共享合并方法的拥有值借用后，pdf单元487项全部通过、0失败/忽略，git diff --check通过。本轮未重跑集成/全工作区测试、未提交；压缩、格式缓冲和持久增长仍未全面计费，AR-03未完成，fuzz继续暂缓。

- 2026-10-07 保存功能版本预检预算接入：现有定向最低版本规则共用普通/预算实现，引用遍历表按实际槽位容量预留、增长计入旧新共存；字段、滤镜数组及成员、Group/Resources/ExtGState递归拥有副本由FeatureObject持有同池租约，多级资源检查保持副本共存。新增资源三层复制峰值、少1拒绝及短输入重试、透明功能拒绝、拥有副本unwind和修订号不变回归。修正滤镜成员拥有类型借用后，最终代码pdf单元486项全部通过、0失败/忽略，git diff --check通过。有效版本计算、压缩、格式缓冲和持久增长仍有缺口，AR-03未完成；本轮未重跑集成/全工作区测试、未提交，fuzz暂缓。

- 2026-10-07 保存对象图预检预算接入：普通/预算共享节点、深度与直接流校验，预算工作表分配前预留并校准实际容量，几何扩容新旧共存；原trailer、压缩前后对象、最终trailer和内部对象序列化均已使用。新增精确扩容共存门槛、少1拒绝保持旧表、unwind、非法直接流及零额度清理回归；补齐测试类型导入后pdf单元485项全部通过、0失败/忽略，git diff --check通过。版本/功能预检、压缩及格式化小缓冲仍有缺口，AR-03未完成。本轮未重跑集成/全工作区测试、未提交，fuzz继续暂缓。

- 2026-10-07 保存trailer预算接入：保留字段键/递归载荷、BudgetedDictBuilder临时与最终表及生成Size数值同池，xref专属字段与旧Size不克隆；Size十进制使用栈缓冲。新增普通/预算字节一致、过滤与替换、结果额度保留、峰值少1拒绝、零额度拒绝、原字典不变及unwind清理回归。首次新增断言误将Size名称中的z识别为旧载荷，修正为连续载荷标记后，pdf单元484项全部通过、0失败/忽略，git diff --check通过。本轮未重跑集成/全工作区测试、未提交；图预检、压缩、格式缓冲和持久增长仍未全面计费，AR-03未完成，fuzz暂缓。

- 2026-10-07 本地提交验证：pdf单元483项，以及content_edit_transaction、nested_text_edit、nested_path_edit、nested_image_edit集成合计19项全部通过，0失败、0忽略；git diff --check通过。提交包含预算接入代码与计划/架构文档，不包含来源未确认的test.png，不推送远端；未运行全工作区测试及fuzz，AR-03仍未完成。

- 2026-10-07 完整保存流程的内部预算接入：新增内部write_document_with_budget，与普通保存共享版本检查、源对象校验和写出流程；最终输出容量、源对象物化副本、对象ID表、xref槽位及待写对象拥有副本接入同池。预检、trailer构造、压缩、格式化临时缓冲和持久容器增长仍有未计费边界，尚不开放完整预算保存公共接口，AR-03保持未完成；新增保存字节一致、重开、预算拒绝和释放测试，fuzz继续暂缓。

- 2026-10-07 保存对象输出预算基础：普通/预算write_object_limited共用规则，新增内部serialize_object_with_budget返回BudgetedBytes并保留实际capacity额度；输出几何扩容计入旧新缓冲共存，标量token、转义键缓冲、排序引用表与流Length栈十进制/预算词法字符串/替换字典同池，数字直接写借用lexeme。有限字典写出统一按键排序，普通保存键顺序同步规范化，避免HashMap与预算字典流输出差异。新增普通/预算字节一致（全字节字符串、转义Unicode名称、精确大整数、引用及流）、结果持有/峰值少1/长度拒绝/unwind、显式容量增长共存及拒绝原缓冲保持测试。所选pdf单元481项及内容事务/嵌套文字/路径/图片集成19项全部通过、0忽略，git diff --check通过。此为保存预算基础，内部入口尚未接入完整save；图预检、引用/诊断格式缓冲及保存对象准备/xref/trailer/压缩/最终输出仍有缺口，AR-03未完成、全工作区未验收；fuzz暂缓，本轮未提交。

- 2026-10-07 预算快照容器计费：预算DocumentStore快照改为StoreSnapshot数组，分配前预留(ObjectId, StoreEntry)槽位并校准实际Vec容量，递归载荷同池，纯Deleted元数据亦计费；不再新增快照HashMap桶。回滚清空工作store、将数组条目移回既有桶并恢复next_obj_num，依赖键不移除/桶不缩的写入契约，混合嵌套回滚亦保留足够容量；成功提交丢弃数组，普通事务及公共entries接口不变。Trailer准备失败路径的租约声明顺序同步修正。新增元数据峰值少1拒绝、数组unwind/对象数限制、增长后回滚容量保持与编号恢复、普通/预算混合嵌套；原嵌套快照精确额度测试按实际表槽＋载荷更新。cargo check -p pdf --locked --quiet退出0；pdf单元479项及内容事务/嵌套文字/路径/图片集成19项全部通过、0忽略，git diff --check通过。新对象引发工作store桶增长、诊断/共享控制结构及原生后端仍有未计费边界，AR-03未完成、全工作区未验收；fuzz暂缓，本轮未提交。

- 2026-10-07 加载栈和未知token预算收口：BudgetedLoadingGuard每层预留新ID数组/实际容量后交换路径，上一数组保留在守卫中，嵌套时旧新同时计费；退出、错误和unwind先释放当前数组/恢复上一数组，再释放租约，原普通路径已有容量原样恢复。Normal/Compressed预算加载均接入，Free诊断不再普通加载；未知对象token原本即拒绝，预算解析直接用首次计费位置生成同普通路径的错误，不再普通重读。新增栈/额度共存、峰值少1拒绝原栈保持、循环/深度拒绝和panic恢复，严格/兼容未知token错误/位置一致及token超限释放测试。cargo check -p pdf --locked --quiet退出0；pdf单元477项及内容事务/嵌套文字/路径/图片集成19项全部通过、0忽略，git diff --check通过。事务HashMap桶、诊断/共享控制结构及原生后端仍未计费，AR-03未完成、全工作区未验收；fuzz暂缓，本轮未提交。

- 2026-10-07 对象流展开预算：普通/预算ObjectStreams共用索引、偏移、对象边界/尾部及严格禁用项校验；BudgetedObjectStreams拥有解码输入租约，预算索引/排序去重/结果表容量，递归对象载荷租约随BudgetedStreamObjects返回。PDFContext预算源路径接入ObjStm同池解码与待发布表，全部选中条目通过最新xref所属流/索引检查后才发布；Compressed请求读取源流而非编辑视图，解码依赖保持source-resolution隔离。新增普通/预算复合对象一致、输入/结果持有额度、峰值少1拒绝/unwind/异池清理、重复/越界/多余数据拒绝、压缩索引错误全部缓存不发布及重试；间接解码参数编辑/删除隔离同时覆盖普通和预算路径。cargo check -p pdf --locked --quiet退出0；pdf单元475项及内容事务/嵌套文字/路径/图片集成19项全部通过、0忽略，git diff --check通过。合法源缓存发布后返回副本拒绝可以保留缓存，但不改变编辑视图；加载栈、非标准token回退、HashMap控制桶及原生分配仍未贯通，AR-03未完成、全工作区未验收，fuzz暂缓，本轮未提交。

- 2026-10-07 普通源对象冷缓存预算：SyntaxParser/PdfParser新增间接对象预算入口，复用预算对象/数组/字典/流与token读取，校验对象ID并保留载荷租约；PDFContext未缓存Normal源对象预算解析后检查既有raw-stream缓存上限，在源额度仍持有时预算克隆返回副本，预算拒绝不发布缓存/不增加缓存流字节，成功源缓存转为持久内存、只保留返回副本租约。新增字符串、复合字典、大流与普通结果一致，冷缓存峰值少1拒绝且缓存/加载栈清理、重试、暖缓存持有额度与大流较低峰值、unwind和ID/结束标记错误释放回归；暖缓存仍需支付字典构建临时表，不能用最终副本容量替代峰值。cargo check -p pdf --locked --quiet退出0；选定pdf单元473项及内容事务/嵌套文字/路径/图片集成19项全部通过、0忽略，git diff --check通过。Compressed/ObjStm展开、加载栈容量、非标准解析回退及控制结构仍未预算化，AR-03未完成、全工作区未验收，fuzz暂缓，本轮未提交。

- 2026-10-07 操作内部预算取页：新增get_page_with_budget复用预算定位，在返回前释放临时目标页字典/祖先及其额度，PdfPage仅持有同普通入口的内联间接引用。四类顶层Unicode替换、嵌套Unicode准备及预算内容提交当前程序读取改用此入口，消除这些路径的普通整树临时副本。新增两页取目标、来源页/修订号/流对象与普通程序一致、邻居额度保持、零额度拒绝/越界不改修订和内容、unwind清理回归。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --test nested_path_edit --test nested_image_edit --locked --quiet退出0，472单元＋19集成通过、0忽略；git diff --check通过。首次源对象加载/缓存、事务HashMap桶及其他容器/原生后端仍有未计费边界，AR-03未完成；普通公共get_page及非预算路径兼容不变，未验收全工作区，fuzz暂缓，本轮未提交。

- 2026-10-07 预算页树定位：新增page_at_with_budget/page_location_with_budget，同池解析Catalog及Pages/Page拥有副本；预算祖先栈和排序已访问ID表按实际容量计费，扩容旧新共存，只保留目标页及祖先所有者，但仍校验全树Type/Kids/Count、重复节点与深度/对象数限制。页根Form注册移除普通全页收集及二次目标字典克隆，预算内容提交的来源页/工作页定位亦接入。新增三个目标页与普通路径一致、结果持有额度、精确峰值少1拒绝及unwind清理、越界/后续非法兄弟/错误Count拒绝回归；既有内容提交门槛探测改用实际预算定位。cargo check -p pdf --locked --quiet退出0；cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --test nested_path_edit --test nested_image_edit --locked --quiet退出0，471单元＋19集成通过、0忽略；git diff --check通过。get_page等其他普通入口、首次缓存及事务HashMap桶仍未计费，未验收完整AR-03或全工作区；fuzz暂缓，本轮未提交。

- 2026-10-07 页根Form注册预算：新增内部register_form_with_budget，预算拥有解析/预检Form、继承资源与XObject，栈Fm名称查重后预算复制；绑定表、资源及页面Resources字段使用预算替换构建器，最终页面/名称租约返回并汇入嵌套提交retained_copies，注册自身快照载荷亦同池。保留普通入口命名/同ID复用规则；新增Fm1碰撞/Fm2匹配、重复注册仅保留名称额度、邻居占满拒绝及释放后同池成功、外层panic回滚和非法Form清理测试。cargo check -p pdf --locked --quiet退出0；pdf单元470项及选定内容事务/嵌套文字/路径/图片集成19项全部通过、0忽略，git diff --check通过。页面定位仍普通收集整棵页树，首次源缓存、HashMap桶等未计费，AR-03仍未完成；fuzz暂缓，本轮尚未提交。

- 2026-10-07 本次提交验证：pdf单元469项及content_edit_transaction、nested_text_edit、nested_path_edit、nested_image_edit合计19项集成全部通过，0忽略；git diff --check通过。未运行全工作区测试或fuzz，不代表完整AR-03验收。

- 2026-10-07 提交准备：嵌套Unicode提交接入父/根程序重建、转义Do输出、复制流载荷/字典及外层和内部内容事务快照载荷预算。无序对象存储的快照遍历可能改变临时峰值，组合测试以确定性重建下界验证拒绝、以充足额度验证公共提交；不再宣称跨独立文档的完整事务精确门槛，固定快照峰值也不保证缩短文字即可成功。页根资源注册内部普通分配、HashMap桶及首次源缓存等仍未覆盖，AR-03未完成；fuzz继续暂缓。提交排除未确认用途的test.png，不推送远端。

- 2026-10-07 局部XObject绑定与Form.Resources重建预算：新增预算字典替换构建器，预算克隆保留值/替换值及键，覆盖构建/最终表共存并检查深度、键长、条目限制，失败不改原字典；局部FmEdit名称由栈缓冲生成、查重后预算复制，分类字典解析、绑定表与资源父字典重建同池。Unicode提交使用预算变体，Form.Resources亦预算重建，新增租约与prepared副本额度一起保留至外层事务返回。新增碰撞命名/普通结果一致、共同峰值少1原子拒绝、释放恢复/unwind、字典限制失败清理；已有精确峰值测试扩展到实际资源提交，提交内拒绝允许事务修订号推进但内容/资源回滚，新快照短文本重试成功。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --test nested_path_edit --test nested_image_edit --locked --quiet退出0，468单元＋19编辑集成通过，git diff --check通过。页根资源注册、form_do/父根程序重建、复制流及事务快照仍待贯通，AR-03未完成，fuzz暂缓，本轮未另行提交。

- 2026-10-07 Form预检与XObject/Form拥有解析预算：PdfForm普通/预算共用验证核心，ResolvedFormObject将对象/租约一起持有，全部预检字段预算拥有解析；BBox/Matrix采用栈数值数组，保持有限值、坐标规范化、字段类型/长度规则，Resources/Group直接移交解析结果，仅其额度保留到返回。准备链分类字典及Form对象解析同池，预检结果额度覆盖prepared副本构造。新增间接字段及逐数值引用、BBox规范化/矩阵/资源/Group结果一致、nullable语义、六字段错误类型一致、峰值少1拒绝/恢复和unwind释放测试。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --test nested_path_edit --test nested_image_edit --locked --quiet退出0，466单元＋19编辑集成通过；cargo test -p render --lib --locked --quiet退出0，30渲染单元通过；git diff --check通过。首次源缓存、复制时资源增长、父/根重建、复制流与事务快照仍待贯通，AR-03未完成，fuzz暂缓，本轮未另行提交。

- 2026-10-07 prepared Form字典/有效资源副本预算：页面资源准备同池，后继调用借用上一prepared资源，移除重复工作克隆；最终字典与资源使用clone_dict_with_budget分配前预留，副本租约置于PreparedForm末字段。提交前汇入外层retained_copies保留到事务返回，已移交持久文档的数据不作为常驻内存继续计费。新增取出Form/释放迭代器后副本额度仍保留、字典/资源内容一致、丢弃/unwind与邻居额度保持测试，真实Form准备结果检查非零副本租约，并纳入实测峰值门槛。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --test nested_path_edit --test nested_image_edit --locked --quiet退出0，465单元＋3事务＋6嵌套文字＋8嵌套路径＋2嵌套图片集成通过，git diff --check通过。Form预检临时对象/资源副本、复制时资源增长、父/根重建、流输出及事务快照仍待贯通，AR-03未完成，fuzz暂缓，本轮未另行提交。

- 2026-10-07 Form准备链与循环检测槽位预算：链Vec与seen数组在分配前checked_mul元素大小预留，按实际capacity校准；BudgetedFormChain返回Vec与槽位所有者，BudgetedFormIter在双端/反向消费中保持租约至数组实际销毁，commit_nested_edit兼容普通/预算迭代器。新增部分反向消费、耗尽但数组仍保留、丢弃/unwind及邻居额度保持测试，嵌套编辑实测峰值同时计入新槽位。cargo test -p pdf --lib --locked --quiet退出0，最新464项单元通过；选定content_edit_transaction/nested_text_edit/nested_path_edit/nested_image_edit回归退出0，3＋6＋8＋2共19项集成通过；git diff --check通过。仅勾选槽位子项，不代表Form预检/字典/资源堆数据、父/根重建、复制事务及完整AR-03验收；fuzz继续暂缓。本轮改动尚未另行提交。

- 2026-10-07 原始Form解码/程序解析预算验收与阶段快照：Unicode准备链实际使用预算解码及from_decoded_bytes_with_budget，源码不复制、解析结果携带边界/操作/递归操作数租约，由PreparedProgram保留至提交。新增普通/预算源码与操作边界一致、峰值少1拒绝、短程序恢复、解析失败/异池拒绝/unwind清理；嵌套编辑精确峰值探测同步计入准备链额度。最近完成的cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --test nested_path_edit --test nested_image_edit --locked --quiet退出0，463单元＋3事务＋6嵌套文字＋8嵌套路径＋2嵌套图片集成通过；此为所选回归，不代表当前全工作区验收。准备链槽位、Form预检/字典/资源副本、父/根重建及复制事务仍待接入，AR-03未完成；fuzz暂缓且保留既有文件。阶段快照排除target/output/tmp与未确认用途的test.png，不删除本地文件。

- 2026-10-06 嵌套继承字体预算接入：新增font_before_operation_in_scope_with_budget消费不可Clone的BudgetedFont，跨流移动前合并原租约，新前缀解析/字体构造租约汇入结果，异池拒绝时保留所有者字段释放顺序；nested_unicode页面资源、调用链与叶字体改用同池路径，字体持有覆盖编码/重建/提交。新增Courier继承与局部Helvetica q/Q恢复、持有额度、非法前缀/异池拒绝清理、unwind及同池恢复专项。组合精确门槛样例改用较长保留注释＋短文本，使重建峰值高于字体/编码峰值；只扩大探测池以测量，不改变生产默认限额，实际峰值少1仍拒绝。嵌套集成显式池随新增字体计费调整到131072，保留大文本拒绝/短文本恢复。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，462单元＋3事务＋6嵌套文字集成通过，git diff --check通过。原始Form链/prepared资源、父/根重建、复制流和事务快照仍未同池，AR-03未完成，fuzz暂缓。

- 2026-10-06 嵌套Unicode文字叶节点重建预算：整字符串/TJ单项、字符串范围及TJ跨项范围使用与编码相同的预算重建入口，返回BudgetedContentProgram；commit_nested_edit泛型借用叶所有者，保留租约至事务结束，普通路径/图片/路径编辑继续兼容。新增真实Form链精确编码＋重建峰值测试，编码独立可完成但共同峰值少1拒绝，额度随结果释放，失败不改变修订号、源内容/流对象及资源，短文本重试成功。原嵌套集成小门槛按新增程序计费调整为短文本成功/大文本拒绝，并由新专项保留精确峰值门槛验收。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，461单元＋3事务＋6嵌套文字集成通过；cargo test -p pdf --test nested_path_edit --test nested_image_edit --locked --quiet退出0，8嵌套路径＋2嵌套图片集成通过；git diff --check通过。只勾选叶节点重建子项，继承字体/资源、原Form链、父/根重建、复制流及事务快照预算仍待接入，AR-03未完成，fuzz暂缓。

- 2026-10-06 字体预算内容提交/外层回滚组合验收：新增Tj/TJ×错误/unwind四路径，共享字体/编码、重建、提交及外层快照预算池，确认编码字体所有者在内容提交返回时仍占额度；内层写入后外层回滚恢复源字节/流对象，临时额度归还且邻居保留，旧快照拒绝、新快照同池重试成功。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，460单元＋3事务＋6嵌套文字集成通过。新增嵌套Form同池贯通待办，按源码明确普通字体构造/程序重建/提交仍有缺口；不扩大为所有公开入口或完整AR-03验收，fuzz暂缓。

- 2026-10-06 页面资源字体共存与unwind验收：新增两页Courier/Helvetica资源解析结果共存测试，分别单独成功但保留首结果时第二结果按共同峰值少1门槛拒绝，首字体仍可查询且额度不变，释放后重试成功；两结果同时unwind释放归零，页面源内容保持不变。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，459单元＋3事务＋6嵌套文字集成通过；cargo test -p render --lib --locked --quiet退出0，30渲染单元通过。仅勾选页面资源结果共存/unwind子项，顶层编辑、原生/共享控制块、首次缓存及保存预算仍待收口，AR-03未完成，fuzz暂缓。

- 2026-10-06 两字体构造共存预算验收：新增Courier/Helvetica分别独立峰值与同池共同峰值探测，确认共同峰值严格大于每个单项峰值；共同峰值少1门槛下两个单项分别成功，但保留Courier构造Helvetica拒绝且首字体查询/额度不变，释放首字体后Helvetica重试成功并释放归零。cargo test -p pdf --lib font:: --locked --quiet退出0，字体相关112项通过（346项过滤），git diff --check通过。仅勾选构造双结果共存子项，不扩大为资源/顶层编辑/全量回归及完整字体预算验收；AR-03其他缺口仍未完成，fuzz暂缓。

- 2026-10-06 FreeType输入副本预算：SimpleFont Type1/TrueType初始化使用OperationFace，预算输入经BudgetedBytes预留后复制，Face先于租约释放，普通路径保留原行为；新增有效Helvetica容量存活、邻居拒绝/恢复、无效字体释放及unwind归零专项。首轮3个64KiB探测池因新增实际输入副本超限，按原池＋实际Helvetica字节数调整测试探测上界（生产预算不变），仍用完整实测峰值邻居1字节拒绝，并新增旧64KiB拒绝/释放断言。最终cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，457单元＋3事务＋6嵌套文字集成通过，git diff --check通过。FreeType原生/共享控制块、缓存/保存及整体渲染预算仍有缺口，AR-03未完成，fuzz暂缓。

- 2026-10-06 字体预算存储渲染回归：Encoding/AFM/CMap/CID宽度表及Subtype分派调整后，cargo test -p render --lib --locked --quiet退出0，30项通过（19.09秒），git diff --check通过。与上一轮456单元＋9编辑集成结果分别记录，不扩大为独立阅读器或全量工作区验收。只读核对SimpleFont的TrueType/Type1初始化仍有两处fontfile.to_vec交给FreeType产生未计费Rust输入副本，下一步需分配前预留并持有至Face释放；原生内存仍独立列为未计费范围，AR-03未完成，fuzz暂缓。

- 2026-10-06 CIDFont最终宽度表容量预算：W/W2复用MappingTable普通HashMap/预算排序Vec，预算新CID插入前预留capacity、扩容旧/新表共存、重复CID覆盖不扩槽；结果表租约返回并并入CIDFont/Type0/BudgetedFont。更新逐项专项确认存活表容量/租约一致、释放后归零；新增字体级W＋W2跨存储比较、CID覆盖/纵向度量、最终名称＋两表容量精确额度、峰值邻居拒绝恢复及W2非法后释放已构造W表专项。最终cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，456单元＋3事务＋6嵌套文字集成通过，git diff --check通过。共享控制块、FontProgram/原生初始化与首次缓存/保存预算仍未贯通，AR-03未完成，fuzz暂缓。

- 2026-10-06 CIDFont W/W2逐项解析预算：共享load_metrics_inner同池解析起始CID、数组/端点及逐项数值，CID构造实际接入，普通包装入口保留同样解析规则；临时数组租约覆盖验证、数值额度用后释放。新增间接三元组数组/高度、重复CID2范围覆盖，核对普通/预算三项度量一致、解析临时峰值邻居拒绝/恢复与目标null释放。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，455单元＋3事务＋6嵌套文字集成通过，git diff --check通过。最终宽度HashMap仍未计费，返回时临时额度归零明确不代表结果预算；表迁移/结果租约仍待推进，AR-03未完成，fuzz暂缓。

- 2026-10-06 CIDFont默认宽度引用与预算：DW外层数值、DW2外层数组及两数值项普通/预算路径都解析引用，预算路径临时数组/数值同池计费，最终默认度量仍内联。新增间接DW600、DW2[900 间接-1100]，验证普通/预算横宽600与纵向(-1100,300,900)一致、最终仅名称额度、实际峰值邻居拒绝/恢复及目标null错误释放。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，454单元＋3事务＋6嵌套文字集成通过，git diff --check通过。W/W2逐项解析与最终表、共享控制块/原生及首次缓存预算仍未贯通，AR-03未完成，fuzz暂缓。

- 2026-10-06 CIDFont名称/映射/Descriptor预算接入：共享普通/预算构造核心，Subtype/BaseFont解析引用，最终名称copy_string预留；CIDToGIDMap预算解码后验证偶数字节/条目数并移交租约；Descriptor同池解析/字体程序预算构造，W/W2外层解析副本同池计费。Type0实际接入CID预算构造，结果租约并入字体。新增间接名称与Descriptor、CID1→GID42、程序ABC、最终名称＋映射＋程序容量精确存活、峰值邻居拒绝/恢复及奇数字节失败释放专项。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，453单元＋3事务＋6嵌套文字集成通过，git diff --check通过。W/W2逐项解析/最终表、DW/DW2引用及program/原生/共享控制块仍有缺口，AR-03未完成，fuzz暂缓。

- 2026-10-06 间接字体Subtype预算分派：普通Font入口先resolve Subtype再验证，预算入口同池resolve_owned后保持临时租约覆盖构造；Type1/TrueType/Type0统一分派到预算构造，不再间接字段普通构造后返回零结果额度。新增间接Type1普通/预算构造、结果额度/实测峰值邻居拒绝恢复、引用目标null错误释放；Type0命名/流CMap矩阵增加间接Subtype普通/预算分派检查。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，452单元＋3事务＋6嵌套文字集成通过，git diff --check通过。CIDFont内部宽度/映射/Descriptor等、共享控制块、原生及首次缓存预算仍未贯通，AR-03未完成，fuzz暂缓。

- 2026-10-06 Type0 BaseFont/CMap部分预算：Font直接Type0分派接入共享构造核心，BaseFont及Encoding/ToUnicode/DescendantFonts解析副本同池；BaseFont复制、流CMap和命名预定义CMap源/表使用分配前预算，聚合结果租约移交字体。新增Identity-H与流Encoding两构造路径，验证代码/宽度/Unicode、字体结果存活、实测峰值邻居拒绝/恢复和非法ToUnicode释放。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，451单元＋3事务＋6嵌套文字集成通过，git diff --check通过。CIDFont内部名称/宽度/映射/Descriptor/程序仍普通构造，间接Subtype预算分派、共享控制块及首次源缓存/原生仍有缺口；AR-03未完成，fuzz暂缓。

- 2026-10-06 CMap继承链预算：链Vec按capacity预留后分配，祖先名称copy_string预留再复制，聚合临时租约保持到子解析器/链释放；各层链副本和源共存累计计峰值，链额度不移交最终映射。嵌套源专项门槛精确加入3个String槽位＋2个BudgetOuter名称＋1个BudgetInner名称，继续验证峰值邻居拒绝/释放恢复，并补充Cycle循环错误归零。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，450单元＋3事务＋6嵌套文字集成通过，git diff --check通过。共享控制块、Type0、FreeType输入副本/原生及首次源加载缓存仍未贯通，AR-03未完成，fuzz暂缓。

- 2026-10-06 CMap继承/目的数组源预算：内置继承源与目的数组子源用BudgetedBytes分配前预留/复制，外部resolver返回Vec后按capacity接收计费，内部回调分配仍属调用方责任；源租约保持到子解析器/视图释放，不移入最终映射额度。新增1024＋2048嵌套源共存峰值3072、邻居1字节拒绝/恢复、回调错误清理专项；代码空间继承峰值断言改为至少8槽位并继续按完整路径实测峰值验证邻居拒绝，因为新增源开销可能成为主峰值。最终cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，450单元＋3事务＋6嵌套文字集成通过，git diff --check通过。继承链名称/槽位、共享控制块、Type0与原生副本仍待贯通，AR-03未完成，fuzz暂缓。

- 2026-10-06 CMap Unicode/CID映射容量预算：MappingTable兼容普通HashMap和预算排序Vec；预算插入/继承合并按capacity预留、几何扩容旧/新表共存，表租约与Unicode/代码空间租约聚合返回并移交字体，预算路径无HashMap桶。继承矩阵增加10槽位（本地8＋保守基底2）额度断言；新增倒序/u32最大键/重复覆盖、无桶、2→4峰值6槽位、最终capacity一致、邻居拒绝及恢复专项。最终cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，449单元＋3事务＋6嵌套文字集成通过，git diff --check通过。继承源/链、目的数组子源、共享控制块、Type0及原生数据仍未贯通，AR-03未完成，fuzz暂缓。

- 2026-10-06 CMap代码空间表预算：本地新增与继承合并前按capacity预留几何扩容表，旧/新容量共存；表租约与Unicode租约合并返回并移交字体，消费的基底表额度暂保守保留。新增本地三范围2→4槽位峰值6槽位/最终capacity一致，先/后继承三范围峰值8槽位/最终保守6槽位、邻居拒绝/释放恢复、继承后截断清理专项。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，448单元＋3事务＋6嵌套文字集成通过，git diff --check通过。Unicode/CID映射HashMap桶、继承源/链、目的数组子源及共享控制块仍未计费，AR-03未完成，fuzz暂缓。

- 2026-10-06 CMap Unicode继承/范围预算矩阵：新增程序先继承、后继承和命名基底三入口，组合bfchar局部覆盖、bfrange数组多字符/代理对、标量递增；逐码核对C/B/AB/😀/D/E并按七条实际解码容量确认含被覆盖A的保守总租约。每入口完整峰值邻居1字节拒绝、失败释放、同池恢复及继承后截断目的数组清理均通过。cargo test -p pdf --lib font:: --locked --quiet退出0，字体相关101项通过（346项过滤），git diff --check通过。不扩大为全量回归或完整CMap预算验收；映射表/代码空间容量、继承源/链和目的数组子源仍待计费，AR-03未完成，fuzz暂缓。

- 2026-10-06 CMap Unicode结果预算贯通：encoding_rs解码器提供容量上界，String分配前预留/capacity校准、decode_to_string不扩容；bfchar、bfrange数组/标量及继承结果字符串租约聚合返回，并实际移交SimpleFont/BudgetedFont，重复覆盖/未选中继承字符串额度暂保守保留。新增全部65536单码元与既有解码对照、BMP/代理对/非法/奇数字节/UTF-16及UTF-8 BOM、capacity/租约一致、邻居拒绝恢复专项；更新源/十六进制路径测试确认映射字符串仍存活时额度不归零、结果释放后归零。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，446单元＋3事务＋6嵌套文字集成通过，git diff --check通过。映射HashMap桶/代码空间及继承源/链、目的数组子源仍未计费；预算继承/范围专项矩阵仍需补验，AR-03未完成，fuzz暂缓。

- 2026-10-06 CMap十六进制转换预算：解析器增加同池句柄，流入口实际设置并传递至继承/目的数组子解析器；字符码、代码空间及Unicode转换结果改由HexBytes持有字节/临时租约，分配前预留、按capacity校准、用后释放。新增两个2字节缓冲共存4字节峰值、额外缓冲/邻居拒绝、释放恢复、非法字符不改额度及实际bfchar映射一致专项。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，445单元＋3事务＋6嵌套文字集成通过，git diff --check通过。Unicode字符串、映射表/代码空间、继承源/链及目的数组子源仍未预算化，AR-03未完成，fuzz暂缓。

- 2026-10-06 CMap词法源视图：解析源使用共享Rc<Vec<u8>>，token仅保留源句柄和起止区间，消除普通词/名称/十六进制串/数组/括号串/字典逐项Vec复制，保持源预算覆盖解析期。新增注释/所有token类型边界、源句柄及字节指针共享、解析器释放后视图有效、空名称/截断数组与字典兼容专项。最终cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，444单元＋3事务＋6嵌套文字集成通过，git diff --check通过。共享控制块、转换结果、目的数组子源、继承链和映射存储仍未预算化，AR-03仍未完成，fuzz继续暂缓。

- 2026-10-06 CMap重复分配清理：十六进制转换借用输入验证/计数、可失败exact预留直接生成字节，去除过滤字符Vec；UTF-16使用into_owned移交已有解码字符串，解析上一词移交而非clone。新增空串、全部PDF空白、大小写/奇数位补零、非法字符/括号、BMP/代理对/非法代理替换专项。cargo test -p pdf --lib font:: --locked --quiet退出0，字体相关97项通过（346项过滤），git diff --check通过。此项是预算接入的分配准备，字节/词法/映射存储尚未计费，不宣称完整CMap或全量工作区验收，AR-03未完成，fuzz暂缓。

- 2026-10-06 ToUnicode/CMap源缓冲预算：SimpleFont实际加载路径同池解析ToUnicode流，CMap共享流加载核心同池解析WMode/UseCMap、直接预算解码，源租约覆盖解析且先释放字节后释放额度。新增间接WMode、Unicode映射一致、完整源峰值邻居拒绝/恢复、模式冲突与截断失败归零专项。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，442单元＋3事务＋6嵌套文字集成通过，git diff --check通过。CMap映射HashMap桶/Unicode字符串/代码空间、词法及UTF-16临时数据、继承源仍未计费，返回映射存活时源额度归零明确仅表示源预算；后续需预算映射存储和结果租约贯通，AR-03未完成，fuzz暂缓。

- 2026-10-06 Descriptor字体程序预算接入：Descriptor共享普通/预算构造核心，标量字段和FontFile流同池解析，嵌入程序直接预算解码并移交字节/租约；SimpleFont预算构造解析外层Descriptor字典并将程序租约并入字体所有者。新增ASCIIHex字体程序ABC结果容量/租约一致、存活与释放、完整峰值邻居拒绝/恢复、非法流归零专项。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，441单元＋3事务＋6嵌套文字集成通过，git diff --check通过。FreeType输入to_vec副本和原生分配、ToUnicode/Type0以及首次源加载/缓存仍未计费；AR-03继续未完成，fuzz暂缓。

- 2026-10-06 Encoding上下文与字体预算贯通：SimpleFont预算入口同池解析Encoding对象，字典编码用预算工作表重建Differences/BaseEncoding，原/解析后数组共存计费、逐项引用同池解析；解析临时租约在构造完成后释放，最终编码名称/表租约并入字体所有者。新增间接编码字典＋数组＋字形名＋BaseEncoding的实际加载专项，验证Unicode/字形查询、结果存活额度、完整峰值邻居拒绝且无结果、释放后恢复与非法字形归零。最终cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，440单元＋3事务＋6嵌套文字集成通过，git diff --check通过。首次源加载/缓存、Descriptor/ToUnicode/Type0及原生数据仍未覆盖，AR-03不勾选，fuzz继续暂缓。

- 2026-10-06 Encoding直接构造预算：新增直接字典部分预算入口，字形名称分配前预留，排序表按capacity预留、扩容旧/新表共存计费，聚合租约随编码结果返回；重复码旧名称额度暂保守保留。新增三名称表2→4槽位峰值6槽位＋3名称、覆盖Unicode、最终容量/租约一致、邻居1字节拒绝与释放后恢复、非法项失败归零专项。cargo test -p pdf --lib font::encoding:: --locked --quiet退出0，3项通过（436项过滤）。上下文解析及SimpleFont加载尚未接入此预算入口，不扩大为顶层编辑预算或全量回归验收；AR-03仍未完成，fuzz继续暂缓。

- 2026-10-06 Encoding Differences存储准备：HashMap改为按字符码排序的Vec，二分查找、重复码最后覆盖，新增倒序插入全部256码、排序/查询、克隆隔离及解析重复码Unicode一致测试。表插入采用可失败预留，但表容量、字形名称和上下文解析副本仍未接入OperationBudget；架构文档记录后续分配前计费、旧/新容量共存及结果租约移交要求。cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，438单元＋3事务＋6嵌套文字集成通过，git diff --check通过。AR-03继续未完成，fuzz实现/运行/CI继续暂缓，既有fuzz文件保留。

- 2026-10-06 AFM排序预算表标准矩阵验收：标准14字体逐字形对照普通HashMap与预算排序表的代码/宽度/BBox/名称查询，并对照全部256字符码查询；逐字体确认最终名称＋表capacity总额与存活额度/租约相等、释放归零。cargo test -p pdf --lib font::afm::tests --locked --quiet退出0，AFM专项8项通过（429项过滤）；cargo test -p render --lib --locked --quiet退出0，渲染30项通过（18.88秒），git diff --check通过。确认这些标准度量数据无两种存储查询差异，不扩大为独立阅读器或完整字体预算验收；其他Encoding/CMap/Descriptor及原生数据仍未贯通，AR-03未完成，fuzz暂缓。

- 2026-10-06 AFM预算度量表存储迁移：预算入口改用排序Vec键/度量表，普通入口保留HashMap；名称和字符码查询支持两种存储，预算槽位按实际capacity预留，扩容旧/新表共存，表租约随AFM/字体结果返回。新增2→4槽位峰值6槽位＋3名称、邻居拒绝/恢复、重复A覆盖、无HashMap桶、最终表容量/保守名称额度及释放归零专项。范围测试发现固定字体表构造峰值使原短输入门槛无法用更短文本恢复，增大输入以继续验证编码共存主导的精确少1拒绝/成功/短重试；预算门槛仍由完整路径测量。最终437单元＋3事务集成＋6嵌套文字集成全部通过，git diff --check通过。不同名称重复字符码预算存储按名称顺序选首项（原HashMap顺序不稳定）；标准字体预算/普通度量全表对照和最新渲染仍待补验，其他字体/CMap/原生预算未完成，AR-03未勾选，fuzz暂缓。

- 2026-10-06 AFM度量名称预算：度量解析先借用字形名验证数值，再按容量预留并生成表键；名称租约聚合至AFM结果，通过已有SimpleFont构造路径移交预算字体。新增两个名称2字节精确存活、邻居拒绝/恢复、结果释放归零及第二记录NaN失败释放专项。最终436单元＋3事务集成＋6嵌套文字集成全部通过，git diff --check通过。重复名称覆盖额度暂保守保留，HashMap桶/度量槽位尚未计费，完整AFM存储预算与其他AR-03缺口仍待推进，fuzz暂缓。

- 2026-10-06 AFM头名称分配预算：新增部分预算解析，FontName/FamilyName按容量预留，重复字段旧/新名字共存计峰值，返回最终名称聚合租约；SimpleFont预算宽度加载实际接入并将租约移交BudgetedFont。新增头替换峰值6字节/最终5字节、邻居拒绝/恢复及缺值错误释放专项。回归修正此前只按BaseFont9字节计最终结果的断言，Helvetica现在保留BaseFont＋AFM头名称27字节，峰值允许等于最终值，未放宽预算池。最终435单元＋3事务集成＋6嵌套文字集成全部通过，git diff --check通过。AFM度量名/HashMap桶、其他字体派生数据与原生内存仍未计费，AR-03未完成，fuzz暂缓。

- 2026-10-06 AFM去重后的渲染/编辑组合验收：cargo test -p render --lib --locked --quiet退出0，30项通过（23.73秒）；cargo test -p pdf --test nested_text_edit --test content_edit_transaction --locked --quiet退出0，6嵌套文字＋3事务集成通过。覆盖既有字体度量参与的绘制/文字编辑与回滚回归，git diff --check通过，不代表独立阅读器或全量工作区验收。AFM预算构造及其他AR-03缺口仍未完成，fuzz暂缓。

- 2026-10-06 AFM度量名称所有权去重：解析结果将名称String直接移交HashMap键，CharMetrics移除重复名称副本，保留名字查询/字符码查询及重复名称最后覆盖规则；新增重复N取末项、重复记录覆盖与克隆查询一致专项。cargo test -p pdf --lib font:: --locked --quiet退出0，字体相关88项通过（346项过滤），git diff --check通过。该优化消除重复分配，但AFM名称/桶及FontName/FamilyName尚未预算化，AR-03未完成，fuzz暂缓。

- 2026-10-06 SimpleFont宽度加载副本预算：预算构造Widths数组/逐项数值使用同池resolve，数组租约覆盖全表验证，各数值临时额度用后释放；最终宽度保持内联256项存储，验证完成才赋值。新增间接数组＋间接500/直接600、完整峰值邻居拒绝/原表不变/同池恢复、修改数值为null后失败归零专项。最终433单元＋3事务集成＋6嵌套文字集成全部通过，git diff --check通过。加载器后续AFM解析及Descriptor/Encoding/ToUnicode/原生初始化仍未计费，AR-03未完成，fuzz暂缓。

- 2026-10-06 SimpleFont基础字段解析预算：预算构造Subtype/BaseFont/FirstChar/LastChar使用resolve_owned_with_budget，字段副本验证后释放；BaseFont解析副本与最终名称共存计入峰值，结果租约仍仅持有最终名称。专项按实际完整字段峰值验证邻居拒绝/恢复、9字节最终存活及释放，补间接BaseFont一致和非法FirstChar=256失败归零。实现后432单元＋3事务集成＋6嵌套文字集成通过；追加断言后的名称字段专项1项通过（431项过滤），git diff --check通过。其他字体加载器/派生表及原生初始化仍未全面计费，AR-03未完成，fuzz暂缓。

- 2026-10-06 SimpleFont最终名称构造预算：SimpleFont新增部分预算构造入口，BaseFont最终String按容量分配前预留并随字体返回租约；预算字体绑定Tf/ExtGState Font实际使用，构造租约并入BudgetedFont保留至提交返回，普通构造保持原行为。实现后431单元＋3事务集成＋6嵌套文字集成通过；随后新增Helvetica名称9字节精确容量、邻居拒绝/恢复及字体＋租约释放归零专项1项通过（431项过滤），git diff --check通过。当前仅SimpleFont最终名称计费，内部resolve、Descriptor/Encoding/AFM/ToUnicode、Type0及原生后端仍未覆盖，AR-03未完成，fuzz暂缓。

- 2026-10-06 预算字体结果所有权接入：内部BudgetedFont不实现Clone，持有Font及Resources/字体选择已计费副本的聚合租约；整串/TJ单项编码返回字符码＋字体所有者，两个范围入口在外层保留字体，四入口持有至提交返回。新增/扩展字体结果存活额度非零、输出字符码与字体额度共存、释放归零验证。范围峰值测试先发现旧估算遗漏持续字体额度，改按完整共享提交实际峰值验证少1字节拒绝/精确允许，保持内容/流ID不变与刷新快照重试，允许事务内拒绝沿用回滚修订契约。最终PDF单元431项、事务集成3项、nested_text_edit集成6项全部通过，git diff --check通过。当前租约保守保留已销毁资源副本额度，不代表Font派生数据已全面计费；构造/原生数据与逐字体q/Q所有权仍待推进，AR-03未完成，fuzz暂缓。

- 2026-10-06 最新字体/内容编辑组合回归：固定字形表、共享字体保存点及BM预算查询改动后，cargo test -p pdf --lib --test content_edit_transaction --test nested_text_edit --locked --quiet退出0，431单元＋3事务集成＋6嵌套文字集成全部通过，git diff --check通过。包含现有顶层Unicode预算拒绝/恢复、字体选择和嵌套编辑回滚等覆盖；本次仅所列范围，不替代全部PDF集成或独立阅读器验收。AR-03字体构造派生负载/原生内存、缓存/保存等仍未完成，fuzz暂缓。

- 2026-10-06 SimpleFont字形派生映射固定表：将仅支持u8字符码的code_to_gid从HashMap改为内联256项Option<u32>，消除该映射的桶分配/扩容，保持未映射与GID=0的区别和全部边界码；新增完整256项、u32::MAX字形值及克隆隔离专项。cargo test -p pdf --lib font:: --locked --quiet退出0，字体85项通过（346项过滤）；cargo test -p render --lib --locked --quiet退出0，渲染30项通过（18.20秒），git diff --check通过，不属于独立阅读器验收。Font本体/Rc包装、其他CMap/Encoding/AFM/Descriptor和原生后端预算仍待推进，AR-03未完成，fuzz暂缓。

- 2026-10-06 混合模式预算边界验收：新增偏好数组Unknown/Multiply/Screen选择首个支持模式、Normal之后null仍必须拒绝、未知单名返回None/未知数组回退Normal专项。使用实际递归预算副本回调验证完整峰值邻居1字节拒绝/恢复、成功与类型错误均释放额度。cargo test -p pdf --lib page:: --locked --quiet退出0，页面相关43项通过（387项过滤），git diff --check通过。属于定向验收，不代表全量430项重跑；AR-03未完成，fuzz暂缓。

- 2026-10-06 字体绑定混合模式查询预算：混合模式解析抽出共享回调核心，预算字体绑定BM名称/偏好数组/元素副本使用同池resolve，普通绘制路径保持既有解析方式；仍验证全部数组元素并保留未知偏好回退规则。ExtGState文字替换测试增加间接BM数组（未知名称＋间接Normal），保存严格重读/文字提取验收继续通过。最终cargo test -p pdf --lib page:: --locked --quiet退出0，页面相关42项通过（387项过滤），git diff --check通过，不宣称全库/渲染重新验收。字体派生数据与完整渲染预算仍待实施，AR-03未完成，fuzz暂缓。

- 2026-10-06 字体保存点共享优化：字体绑定q/Q栈改存Option<Rc<Font>>，保存/恢复共享不可变绑定而非深Clone字体表；返回前释放栈别名并移交选中Font，栈容量计费调整为共享句柄大小。新增Helvetica/Courier多次切换＋内层Q恢复、目标处外层q仍打开时旧/预算选择一致且额度归零专项。实现后PDF单元428项、事务集成3项、nested_text_edit集成6项通过；随后新增测试的font_binding专项5项通过（424项过滤），git diff --check通过，不宣称全量429项重跑。Font派生数据、Rc包装及内部查询仍未全面计费，AR-03未完成，fuzz暂缓。

- 2026-10-06 最新PDF全量普通回归：cargo test -p pdf --locked --quiet原进程正常退出0，428单元＋226集成共654项通过，0失败/0忽略，40组结果（含0项文档测试）；示例目标编译通过，不代表示例全部运行或独立阅读器验收。覆盖近期流参数、内容/对象复制、工作页字典、预算事务快照、字体资源/继承及q/Q表接入后的全部现有PDF回归，git diff --check通过。此前render30项记录属于其单独测试范围，本次不合并为工作区全量。AR-03仍有字体派生负载、HashMap桶、页树枚举、缓存/保存/渲染等缺口，整体TODO未完成；fuzz未运行。

- 2026-10-06 资源预算查询失败边界：新增无Resources返回None、非字典节点返回PageParentIsNotDict、两层深度不能到达第三层资源时与旧查询同类错误专项；各分支保留邻居7字节额度，失败后同池仍可读取近层null属性，最终释放归零。cargo test -p pdf --lib document::page_tree::budget_tests --locked --quiet退出0，2项通过（426项过滤），git diff --check通过。本轮为定向验收，不宣称全量428项已重跑；AR-03未完成，fuzz暂缓。

- 2026-10-06 字体资源继承查询预算：新增内部预算Parent链属性查询，初始节点、节点字典/父引用、最终属性递归复制分配前计费，循环检测ObjectId表按max_depth预留并复核capacity；每层临时副本释放，最终Resources携带独立租约并由顶层字体绑定持有至选择结束。新增父资源与旧查询相等、返回仅持有结果额度、完整峰值邻居拒绝/恢复及父循环失败归零专项。最终PDF单元427项、事务集成3项、nested_text_edit集成6项全部通过，git diff --check通过。首次源加载/缓存、完整页树枚举、Font派生堆负载/内部查询仍待计费，AR-03未完成，fuzz继续暂缓。

- 2026-10-06 字体绑定q/Q栈表预算：预算字体选择对Option<Font>槽位按容量预留，几何增长时旧/新表共存计费，搬移后释放旧表租约；弹栈保留仍分配capacity额度，错误/返回释放。新增3层q与Q恢复测试，2→4槽位扩容以6槽位峰值验证邻居1字节拒绝/同池恢复/归零，以及多余Q失败释放。最终PDF单元426项、事务集成3项、nested_text_edit集成6项全部通过，git diff --check通过。Font.clone派生堆负载、继承资源scope与字体内部解析仍未全面计费，AR-03未完成，fuzz暂缓。

- 2026-10-06 顶层字体选择资源副本预算：四个Unicode替换入口使用带预算字体绑定，Font/ExtGState类别字典、所选对象及ExtGState Font数组/大小/字体解析使用resolve_owned_with_budget，副本租约聚合保留到选择结束；旧/嵌套只读字体选择保留行为。新增Helvetica旧新绑定一致、实际峰值邻居1字节拒绝/释放/恢复及错误目标失败释放专项。回归发现旧顶层8KiB池已不足以容纳新增字体副本，顶层整串/TJ与嵌套测试中的独立顶层分支改为128KiB池和同长度超大输入，仍验证拒绝后短替换恢复，嵌套精确额度未改。最终PDF单元425项、事务集成3项、nested_text_edit集成6项全部通过，git diff --check通过。继承scope取得、字体派生数据/内部解析、q/Q副本与混合模式查询尚未计费，AR-03未完成，fuzz暂缓。

- 2026-10-06 预算保存点交互与渲染回归：最新内容提交接入预算快照后，nested_text_edit集成6项通过（0.82秒），render单元30项通过（18.38秒），包含既有文字/图片/Form像素等回归，不代表独立阅读器验收。新增两个显式预算保存点实际进入后分别回滚：同池3→6→3→0字节，内层恢复456、外层恢复123，深度归零且无成功提交修订；DocumentState专项4项通过（420项过滤），git diff --check通过。该专项不宣称全量424项已重跑；AR-03及整体目标仍未完成，fuzz继续暂缓。

- 2026-10-06 内容提交接入预算保存点：共享池提交改用edit_transaction_with_budget，当前程序/编辑结果/工作页/流载荷与Trailer及修改对象快照负载共用额度，普通提交保留原入口。新增工作页复制峰值允许而快照额度不足的定向测试，验证拒绝前编辑准备已完成、未进入事务、修订/内容/流ID不变、其他持有者额度保留和同池重试成功；16流回滚测试额外计入快照峰值，仍验证事务内失败、全部ID不变与刷新后重试。最终PDF单元423项、ContentProgram集成29项、事务集成3项全部通过，git diff --check通过。HashMap桶、页树旧副本、字体资源、普通外围事务与源缓存仍未计费，保存/渲染尚未贯通，AR-03未完成，fuzz继续暂缓。

- 2026-10-06 显式事务快照负载预算：新增edit_transaction_with_budget入口，进入事务前递归预算复制Trailer与修改对象负载，保留Deleted和对象分配游标；快照租约覆盖提交丢弃、错误回滚和panic恢复，嵌套显式预算保存点共用池分别占额，预检失败不进入闭包/不改深度修订。新增精确3字节对象＋邻居拒绝/恢复、嵌套耗尽、错误/panic恢复和公开入口拒绝/外层回滚/成功重试专项。PDF单元422项通过，最终公开事务集成3项通过，git diff --check通过。HashMap桶容量、闭包普通API和普通嵌套事务不自动计费；现有内容提交还未使用新事务入口，源缓存/保存/渲染仍待接入，AR-03未完成，fuzz暂缓。

- 2026-10-06 工作页字典提交预算：身份检查取得的页位置用后释放，共享池提交对准备完成后的最新工作页字典递归预算克隆，保留租约至提交/回滚返回；Contents使用已有键原位替换，不分配新键或扩大表，缺键明确失败。新增HashMap/排序表两种存储的替换一致、容量不变、缺键无修改专项。工作页预检额度不足验证未进入事务、修订/内容/流ID不变及释放后重试；原回滚用例因新增计费提前拒绝，改按工作页峰值预留并扩为16流（含空流），保持事务内失败、所有ID不变和刷新快照后重试成功验收。PDF单元421项、ContentProgram集成29项、事务集成2项全部通过，git diff --check通过。page_location旧页树遍历/副本、事务快照、字体资源及保存/渲染仍待计费，AR-03未完成，fuzz继续暂缓。

- 2026-10-06 解码态提交流字典重建预算：共享池提交不再先克隆整个字典后无预算插入Length，而是按预算直接重建，跳过旧Length/Filter/DecodeParms/DL，保留自定义属性；Length在栈上格式化，键/数字原文及临时/最终字典表分配前预留，租约持有至事务返回，载荷沿用独立预留。新增与普通构造相等、Decoded状态、精确峰值邻居拒绝/恢复、释放归零和外部文件流拒绝专项。PDF单元420项、ContentProgram集成29项、事务集成2项通过；随后增加零深度拒绝检查后，重建专项1项通过，git diff --check通过。页字典改写、事务快照、字体资源、首次加载/缓存及保存/渲染仍未全面接入，AR-03未完成，fuzz继续暂缓。

- 2026-10-06 流解码参数预算接入：decode_data_with_budget 对 Filter/DecodeParms 的引用解析、数组表和参数字典重建按容量预留，临时副本与解码输出共用操作池；已知参数解析引用，未知参数及 JBIG2Globals 保留既有惰性策略。租约保留至解码返回，返回字节仅持有自身容量额度。新增旧新参数结果相等、未知引用保留、峰值邻居拒绝/恢复、实际参数流解码及非法编码失败释放回归；修正旧多过滤器测试遗漏参数副本峰值的上限假设。PDF 单元419项与 ContentProgram 集成29项通过，随后补充实际解码/失败释放断言的参数专项6项通过。原始源首次加载/缓存、原生 codec、事务快照、字体资源及保存/渲染仍未全面计费，AR-03未完成；fuzz未实现、未运行、未接入CI。

- 2026-10-06 页面字典取得预算：新增内部pagedict_with_budget，content_program_with_budget实际在预留后复制当前页字典，与Contents副本短暂共存计费，取得Contents后显式释放页字典及租约，不并入返回程序持续额度；旧页面入口保持兼容。数字操作数旧测试的解析-only上限导致一次回归失败，更新完整路径峰值门槛，保留解析峰值下界、邻居1字节拒绝/恢复和最终操作数表/源码额度精确验证。当前代码PDF单元418项通过，修订测试后ContentProgram集成29项及content_edit_transaction集成2项通过，git diff --check通过。页面构造/属性继承、首次源加载及事务位置查询/页字典快照仍待计费，AR-03未完成，fuzz未运行。

- 2026-10-06 页面源流收集副本预算：content_program_with_budget取得Contents和逐个流实际改用resolve_owned_with_budget，数组/流副本与解码输出共同计费，局部所有者收集结束后释放，返回程序最终额度仍仅包含结果。初次回归发现空流/多流旧测试按最终表或操作增长计算上限而遗漏副本峰值；更新为完整路径观察峰值少1字节拒绝/精确允许，保留返回源码/元数据/操作表额度、邻居恢复及释放归零验证。cargo test -p pdf --lib --test content_program --test content_edit_transaction --locked --quiet退出0，418单元＋29页面集成＋2事务集成通过，git diff --check通过。页字典取得、未缓存源首次加载/缓存、解码参数克隆与事务仍待接入，AR-03未完成，fuzz未运行。

- 2026-10-06 提交取得源对象副本预算：PDFContext新增内部resolve_owned_with_budget，直接借用输入/修改存储对象执行递归预算克隆，不先生成普通Clone；沿用删除标记与原始源解析作用域的语义。共享池提交取得Contents及原流实际使用该入口，租约保守保留至事务结束。新增直接/引用解析相等、邻居额度拒绝/恢复、源作用域忽略修改存储并恢复深度、删除引用拒绝且直接对象仍可复制专项。PDF单元418项、ContentProgram集成29项、content_edit_transaction集成2项通过，git diff --check通过。未缓存源对象首次加载/缓存、页面收集源流、页字典/事务快照与字典改写仍未全部计费，AR-03未完成，fuzz未运行。

- 2026-10-06 递归对象/提交流字典预算克隆：内部新增按新容量预留的可失败克隆，覆盖字符串、名称二进制＋显示串、数字词法原文、数组、字典构建及流载荷，引用仅复制ID不解析，保留流数据状态。共享池提交实际将流字典副本改用此入口并持续持有聚合租约至事务结束；移除字段时保守保留克隆额度，不提前减计。新增嵌套结构/二进制/数字/引用语义相等、精确存活/峰值邻居拒绝后恢复、Decoded流状态和数量/深度/token限制失败释放。PDF单元417项、ContentProgram集成29项、content_edit_transaction集成2项通过，git diff --check通过。resolve_owned取得原流的副本、页/事务克隆及后续Length/Filter字典改写仍未计费，AR-03未完成，fuzz未运行。

- 2026-10-06 当前渲染/嵌套回归收口：近期递归对象/字典/流、顶层Unicode结果及提交载荷/Contents预算改动后，cargo test -p render --lib --locked --quiet退出0，30项通过，执行21.62秒；覆盖现有文字/图片/Form、矩阵/BBox/裁剪、页面坐标、透明/混合等像素回归，不是新增独立阅读器验收。nested_text_edit先发现旧测试用编码-only极小额度运行已聚合顶层TJ入口；改为独立顶层8192池验证超大编码拒绝不修改/短替换恢复，同时保留嵌套7/8字节及后缀表精确边界。重跑cargo test -p pdf --test nested_text_edit --locked --quiet退出0，6项通过（0.79秒），未缩小嵌套验收范围。架构文档补当前预算覆盖状态表以区分历史记录，git diff --check通过；AR-03仍未完成，fuzz未运行。

- 2026-10-06 Contents输出表提交预算：数组输出改为按已知数量精确分配Vec，预算入口分配前预留槽位、按capacity复核并保留至事务结束，避免默认追加导致未计费扩容。新增2流提交在载荷预留后被其他持有者占满额度导致表预留失败，验证回滚后内容/两个流ID不变、操作额度释放而其他持有者保留；遵循现有回滚修订推进/旧快照失效契约，重新取快照释放额度后提交成功、仅变化流获得新ID且保留流数量。PDF单元416项、ContentProgram集成29项、content_edit_transaction集成2项通过，git diff --check通过。源数组/页及流字典克隆、事务快照和嵌套入口仍待计费，AR-03未完成，fuzz未运行。

- 2026-10-06 提交流载荷复制预算：共享池提交在事务开始前预留所有变化流的目标字节，Vec分配后按实际capacity补计，保留至提交/回滚完成；未变化流不占新增载荷额度。四个顶层Unicode预算入口实际使用此路径。新增编辑准备后其他持有者耗尽额度导致复制预检拒绝，验证未进入文档修改、修订/内容/流ID不变、当前程序额度释放而其他持有者保留；释放后同池预算结果可正常提交并释放本次额度。PDF单元415项、ContentProgram集成29项、content_edit_transaction集成2项通过，git diff --check通过。成功后字节属于文档常驻对象，操作租约不构成文档常驻内存上限；Contents输出表、源/字典克隆、事务快照及嵌套编辑仍待贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 TJ预算结果及顶层Unicode贯通：新增with_replaced_text_array_items_codes_with_budget，借用源码对数组项核对/开括号token计费，逐项hex生成与替换缓冲扩容旧新容量共存计费，返回完整预算程序。TJ单项、单字符串范围及跨TJ范围文档入口改为实际返回该预算结果；连同整串共4个顶层Unicode入口持有当前/编码/范围/结果同池至提交返回。新增跨3流字符串、多项二进制替换、25.00原文/未改空串/流边界保留、旧新结果一致、精确峰值邻居拒绝/恢复、错误数字项/重复索引失败释放集成。最终版本PDF单元414项、ContentProgram集成29项、content_edit_transaction集成2项通过，git diff --check通过。事务快照、源/资源对象解析、写入流复制及嵌套编辑尚未贯通，借入原快照不重复计费，AR-03未完成，fuzz未运行。

- 2026-10-06 Tj/引号预算结果与整串提交接入：新增with_replaced_text_codes_with_budget，对生成hex token、原操作数核对、替换片段与重建结果同池计费；原操作源码定位改为借用而非复制。Tj、单引号及双引号共用旧/预算核心，保留注释/邻居和双引号数值原文。提交核心接受AsRef程序所有者并持有至事务返回，replace_page_text实际返回预算结果而非无租约克隆；新增预算结果实际提交/释放专项。连续提交测试刷新当前快照，保持既有修订失效契约。新增三个操作类型的旧新结果一致、二进制字符码、峰值邻居拒绝/恢复与原程序不变集成。PDF单元414项、ContentProgram集成28项、content_edit_transaction集成2项通过，git diff --check通过。TJ单项/范围结果、源对象/事务快照/提交流复制仍待贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 Tj构建预算基础：ContentBuilder新增encoded_text_with_budget，token/流长度预检后预留准确容量，返回持有租约的BudgetedBytes；旧构建器与预算入口共用长度/十六进制写入核心。新增空串、ASCII/二进制与旧输出完全一致、邻居1字节拒绝/恢复、精确持续容量和token/流上限失败释放单元；新增预算文字片段＋预算操作替换结果同池共存、释放结果仍保留输入额度的实际组合集成。PDF单元414项、ContentProgram集成27项通过，git diff --check通过。TJ数组/引号构建与文档Unicode提交返回预算结果的迁移仍待实施，不代表整条提交或AR-03完成，fuzz未运行。

- 2026-10-06 操作替换结果预算入口：ContentProgram新增with_replaced_operation_with_budget，旧接口共用替换核心但保持无预算返回；带预算入口对替换片段源码/范围/操作表和递归负载、重建源码/流范围/重新解析结果计费，片段持续持有至邻居操作/操作数相等校验完毕，最终返回不可Clone预算包装并移交结果额度。借入原程序/replacement不重复计费。新增旧新结果/流范围/离线来源一致、结果精确额度、片段共存峰值、邻居1字节拒绝后恢复、错误操作数目/未闭合失败释放和原程序不变回归。PDF单元413项、ContentProgram集成26项通过，git diff --check通过。文字构建器与提交闭包尚未返回预算结果，其他编辑入口、事务/源对象等仍待贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 范围文字提交共享预算：单字符串范围与跨TJ范围Unicode入口改用共享池提交校验，当前程序与编码/范围结果额度同时保留；连同此前整串/TJ单项共4个入口接入。新增OperationBudget::peak_bytes共享成功预留峰值，失败不增加、释放不归零；新增克隆池/邻居共存/失败扩容/释放专项。单字符串范围预算门槛按实际当前程序峰值与当前存活＋编码峰值取最大值，验证少1字节拒绝不修改、精确允许及失败后较小编辑恢复。PDF单元413项、ContentProgram集成25项、content_edit_transaction集成2项通过，git diff --check通过。峰值只反映已跟踪分配，编辑结果/其他入口/源对象/事务仍待贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 整串/TJ单项文字提交快照预算：新增可共享操作池的提交核心，这两个Unicode入口的当前页面程序校验实际调用content_program_with_budget，与编码输出同池并持续持有至提交返回；其他入口保持旧解析以待后续逐项迁移，不宣称统一提交全部计费。新增额度不足在编辑闭包之前拒绝、邻居额度保留、编辑失败释放校验程序且修订不变、后续同池成功恢复验证；整串/TJ单项的旧编码-only预算测试改为包含当前程序的操作范围，超大编码拒绝后短替换成功。PDF单元412项、ContentProgram集成25项、content_edit_transaction集成2项通过，git diff --check通过。编辑结果解析/事务克隆/源对象及其他编辑入口尚未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 间接Length预算接入：间接对象解析新增共享旧/预算核心，预算路径头部objnum/gennum/obj、对象负载与endobj同池计费；流Length引用解析持有返回租约至数值检查结束，不保留临时引用对象额度，继续在成功/错误后恢复原流位置。新增有效数字、错误对象ID、非数字值、错误尾标记、未闭合值与额度不足的位置/邻居额度恢复；真实xref流样本预算结果与旧接口相等，最终仅持有字典键/表＋原始流数据。cargo test -p pdf --lib --locked --quiet退出0，411项通过，git diff --check通过。压缩Length引用仍沿用既有不直接解析策略，xref/源结构、原生分配、事务/保存/渲染及旧入口分配仍未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 丢弃行无分配收口：to_next_line不再调用read_line构建随后丢弃的Vec，改为逐字节计数跳过，保留max_token_bytes、EOF及LF/CR/CRLF语义；stream起始行兼容修复因此不再产生该临时堆负载。新增与保留行读取的位置/边界等价验证、3字节上限允许/拒绝及stream ignored CRLF合法兼容恢复样本。cargo test -p pdf --lib --locked --quiet退出0，410项通过；ContentProgram集成25项通过，git diff --check通过。间接Length解析及端到端其他分配仍待接入，AR-03未完成，fuzz未运行。

- 2026-10-06 嵌入流原始字节预算：共享旧/预算流读取核心，预算块读取分配前预留、核对capacity并保留租约，尾标记token同池计费；声明长度不匹配时第一次载荷/租约在兼容回退前释放，成功合并原始数据与字典额度。新增直接Length、错误Length兼容回退、缺失Length恢复的对象/位置兼容和精确持有额度，严格长度不匹配失败释放、块预算不足位置不前进及随后短读恢复。cargo test -p pdf --lib --locked --quiet退出0，409项通过，git diff --check通过。间接Length解析、stream起始行的兼容修复及其他未跟踪分配仍待接入，不代表端到端流预算完成；AR-03未完成，fuzz未运行。

- 2026-10-06 字典扩容/边界专项：构建器第三个键触发2旧＋4新槽位表共存，精确6×BudgetEntry＋3键字节预算在邻居1字节存在时拒绝且原键/表/额度不变，释放邻居后同构建器成功并最终释放归零；语法新增不同键/重复键均按出现次数受max_objects限制，超限后同解析器重置位置并放宽限制可完成；嵌套字典深度2拒绝/恢复到3成功、零键额度允许空字典，失败均恢复深度并保留邻居额度。cargo test -p pdf --lib --locked --quiet退出0，408项通过；git diff --check通过。下一步接入嵌入流原始字节和尾标记/回退临时token计费；间接Length解析、原生分配及端到端其他组件仍待贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 字典预算当前单元回归：递归字典/构建器接入及专项新增后，cargo test -p pdf --lib --locked --quiet退出0，406项通过、无失败/忽略/过滤；另ContentProgram集成25项通过。未把此前620项全量结果替代当前未重跑的其他集成/渲染验证，AR-03仍未完成，fuzz未运行。

- 2026-10-06 递归字典预算接入：BudgetedDictBuilder对临时槽位表/最终连续表按实际capacity预留并计费共存，逐键/值持有租约，名称解码二进制释放后直接移交键String；递归值共用内容预算池。兼容模式重复键保留原键并释放旧值/传入重复键额度，strict模式拒绝重复键；数量/深度状态沿用并恢复。新增空字典、转义键、嵌套字典/数组结果与位置兼容、替换后精确额度、最终表峰值邻居拒绝/恢复、非法/未闭合/严格重复键失败释放，及实际页面替换字典负载持有。SyntaxParser单元35项、ContentProgram集成25项通过，git diff --check通过。多键扩容峰值和更多边界仍需专项；嵌入流字典参与计费但原始流字节仍走旧读取路径，AR-03未完成，fuzz未运行。

- 2026-10-06 字典可计费容器基础：PdfDict新增内部有序连续表路径，普通HashMap构造不变；统一查询/覆盖/删除/Clone、跨存储按内容相等比较，以及无需堆分配的ExactSize/Fused迭代器。新增不同插入顺序、重复覆盖、删除不存在键及双向相等/克隆/迭代长度兼容专项，字典单元2项通过；包含最新数组改动的PDF单元404项通过。此容器尚未接入预算语法构建，键/值租约、扩容预留和重复键负载释放仍待实现，AR-03未完成；既有620项全量结果属于此前版本，fuzz未运行。

- 2026-10-06 此前全量PDF回归收口：cargo test -p pdf --locked --quiet原会话完成，退出0；40个结果组共620项通过（401单元＋219集成，含0项doc-tests组）。该运行验证此前数字/引用与名称预算版本，不覆盖随后递归数组和字典容器改动；其专项验证分别记录，不把历史全量结果当作最新全量证明。fuzz未运行。

- 2026-10-06 数组数量限制/架构边界复核：新增max_objects=1解析双元素数组明确ResourceLimit、已有邻居额度不变且解析深度恢复，重置位置并放宽为2后同解析器可完成且返回容量准确释放；SyntaxParser单元33项通过。架构文档同步当前递归数组覆盖及字典容器预算接口/重复键所有权的待实现验收，明确不以HashMap键数量估算宣称实际容量已计费。此前全量回归继续按原会话观察，未重启；fuzz未运行。

- 2026-10-06 递归数组内容操作数预算：新增内容对象预算入口，数组元素表按容量扩容前预留、旧新表共存计费，元素所有权移动不复制；递归串/名称/数字负载及子数组表额度并入返回租约，布尔/null临时token也计费。沿用嵌套深度与元素数量限制，成功/失败均恢复解析深度。新增嵌套结果/位置兼容、精确存活容量、6倍对象大小增长峰值邻居拒绝后同解析器恢复、未闭合/非法名称/深度错误释放，以及实际页面程序合计额度回归。SyntaxParser单元33项、ContentProgram集成24项通过，git diff --check通过。字典/流仍明确走旧对象路径，HashMap表/键/重复键替换和其他未跟踪分配尚未计费，AR-03未完成，fuzz未运行。此前启动的401单元版本全量回归仍在执行集成组，不将该运行当作本条新增数组改动的全量验证。

- 2026-10-06 顶层数字/引用预算：数字词法原文通过带预算token的Vec→String所有权转移进入PdfNumber，不额外复制；引用解析保留旧接口的三token前瞻与回退位置，前瞻期间所有token共同计费，引用结果不持有临时token额度。页面数字负载合并到程序租约，并更新精确峰值/返回额度测试；新增长小数/符号/整数原文、引用范围、位置兼容及前瞻峰值邻居拒绝/恢复验证。SyntaxParser单元31项、ContentProgram集成23项通过，git diff --check通过；全量PDF回归已通过401项单元测试，集成组仍在运行，尚不宣称全量通过。嵌套数组/字典、布尔/null旧token及其他未跟踪分配仍待接入，AR-03保持未完成，fuzz未运行。

- 2026-10-06 顶层名称操作数预算：名称解码先验证并计算输出，再对二进制Vec和有损UTF-8 String分别预留实际容量；避免临时字符串，解析token与结果在解码期间共同计费，返回后结果额度并入页面程序。新增有效/非法转义、二进制与不完整UTF-8兼容、邻居额度拒绝/恢复，以及页面/A#ff操作数持续计费/释放回归。名称单元4项、ContentProgram集成22项通过，git diff --check通过。数组/字典内嵌对象、数字及其他未跟踪分配仍待接入，AR-03未完成；不新增或运行fuzz，既有fuzz文件保持不动。

- 2026-10-06 字符串负载预算增长/错误专项：新增字面abcde和对应hex的4旧＋8新容量共存精确允许、邻居1字节导致拒绝、结果capacity8持有/释放归零；新增未闭合字面、非法hex及未闭合hex在已分配输出后失败，验证邻居3字节额度保留。SyntaxParser单元29项通过。只覆盖直接字符串负载，内嵌对象/数字名称及完整解析预算仍待接入，AR-03未完成，fuzz未运行。

- 2026-10-06 字面续行无输出预算收口：遇反斜杠先读取转义，只有非LF/CR续行才预留输出容量，避免空续行串分配和恰满缓冲后因续行扩容。扩展1字节预算下LF/CR/CRLF空续行输出0额度，以及4字节字面加LF/CRLF续行容量仍4回归；SyntaxParser单元28项通过，含既有转义/八进制/换行验证。内嵌对象预算与完整解析峰值仍未完成，AR-03未完成，fuzz未运行。

- 2026-10-06 字符串按实际输出预留收口：hex仅在成对输出或奇数尾半字节补零时预留，不为空白/高半字节/结束符分配；字面串最外层右括号先返回，避免空串分配及长度恰满容量时仅因结束符扩容。新增1字节预算下空字面/空hex/空白hex、奇数F→F0及4字节字面容量保持4回归；SyntaxParser单元28项通过。字面续行等其他无输出路径及内嵌对象预算仍待收口，AR-03未完成，fuzz未运行。

- 2026-10-06 顶层字符串负载预算：抽取字面/hex字符串共享解析核心，预算入口对缓冲增长分配前预留并保留旧容量，成功返回PdfString/租约一起转移；页面内容解析对直接字符串操作数实际调用此入口并合并额度，旧接口保持转义/换行等语义。新增(abc)/<616263>的输出、字符串capacity及页面合计持续计费/释放回归；ContentProgram集成21项、SyntaxParser单元27项通过。数组/字典内嵌字符串、名称/数字负载和其他对象仍未递归计费，AR-03未完成，fuzz未运行。

- 2026-10-06 渲染层当前回归：JPEG/CCITT解码、fax0.2.7及内容解析/页面预算改动后执行cargo test -p render --lib --locked --quiet，退出0，30项通过、无失败/忽略/过滤，测试执行24.09秒。现有范围覆盖Form/图片变换与隔离、BBox/裁剪、透明/混合、页面旋转/UserUnit及编辑像素等；不代表新增独立阅读器、真实复杂字体或完整图片/预算矩阵验收，AR-03仍未完成，fuzz未运行。

- 2026-10-06 token预算错误验收：新增非法UTF-8名称转换和max_token_bytes=3读取4字节token拒绝，验证仅释放当前token额度、邻居内容/额度不变，后续较小token可正常读取并释放归零；预算word单元2项通过。此次不自动回滚SyntaxParser读取位置，也不代表对象解析内部旧token/负载已计费，AR-03未完成，fuzz未运行。

- 2026-10-06 内容名称token预算：SyntaxParser新增内部带预算word读取，PdfWord按容量扩容前预留并保留旧容量至复制结束；内容解析分类token释放后解析操作数，操作名称Vec→String无复制转移同时保留租约并合并到页面程序。更新页面精确峰值/返回额度回归，ContentProgram集成20项通过；新增5字节名称4＋8扩容峰值、邻居拒绝/恢复、String容量租约转移单元通过。对象解析内部旧word路径及数字lexeme/字符串等负载仍未迁移，AR-03未完成，fuzz未运行。

- 2026-10-06 操作数预算失败验收：新增数组槽位额度少1时在未闭合字符串对象解析前ResourceLimit且源位置仍0；新增两操作已累计操作数额度后遇到未闭合字符串，验证所有本次额度释放、邻居内容/额度及文档修订不变，后续较小合法程序可成功并释放归零。内容解析单元6项、ContentProgram集成20项通过。仅覆盖已接入表/源码/元数据，未计费的对象内部负载与token仍待迁移，AR-03未完成，fuzz未运行。

- 2026-10-06 页面操作数数组预算：ContentParser新增内部预算读取入口，对Vec<PdfObject>扩容先预留新容量并保留旧容量，移动元素而非克隆；成功操作将数组租约absorb到解析累计所有者，最后随页面程序返回，失败路径释放。新增两个数字操作数验证源码/元数据/操作表/操作数表合计峰值、邻居拒绝、返回持续计费和Drop归零；ContentProgram集成19项通过。名称token、数字lexeme及字符串/数组/字典等对象内部负载仍未计费，不称完整解析预算，AR-03未完成，fuzz未运行。

- 2026-10-06 同池租约连续转移：新增内部MemoryLease::absorb，核对Rc池身份后转移所有权字节，不改变活跃计数、不释放再预留；跨池失败保持双方租约/计数不变。页面返回前将源码、范围、ID及操作表租约合并到一个所有者，预算缓冲释放后仍完整计费。预算相关单元10项、ContentProgram集成18项通过。此原语用于后续解析负载转移，不代表负载已计费；AR-03未完成，fuzz未运行。

- 2026-10-06 内容token复用位置验收：补回BI拒绝前恢复读取起点，保持旧peek语义；新增带注释尾部反复EOF、注释后BI反复拒绝均位置不变，以及尾部只有数字操作数时报ContentParseError并保留实际数字结束位置。内容解析单元5项通过。未实现内联图片解析，不将重复拒绝视为可继续解释；完整负载预算仍未完成，fuzz未运行。

- 2026-10-06 内容操作符去重复扫描：ContentParser首次get_next_word直接用于操作符名称/偏移构造，避免peek后重新扫描并分配同一操作符token；操作数分类token先释放再回退交给对象解析，EOF/读取错误仍恢复起点。内容解析单元4项、ContentProgram集成18项通过。此改动仅消除重复扫描和临时token共存，不是分配前负载预算；操作名称/操作数内部堆内存、解析临时对象仍未计费，AR-03未完成，fuzz未运行。

- 2026-10-06 页面解析操作表预算：预算解析核心对Vec<SourceOperation>几何增长分配前预留新容量字节，复制期间保留旧容量租约，移动已有操作而非克隆负载；容量核对后随ProgramLeases保留至返回包装Drop。更新两操作回归验证源码/元数据＋1旧槽/2新槽共存峰值、邻居拒绝及结果2槽持续计费；ContentProgram集成18项通过。操作名称/操作数内部堆内存、解析临时对象及文档解析副本仍未计费，旧Clone/编辑不继承租约，AR-03未完成，fuzz未运行。

- 2026-10-06 ContentProgram复用范围所有权：抽取私有parse_with_stream_ranges，页面预算/旧读取、多流构造及操作替换直接将已有范围数组移入解析结果，不再先分配单项默认范围再替换丢弃；单源码公开入口仍保留0..len默认范围语义。ContentProgram集成18项、嵌套文字6项通过。此改动消除此前未计费的默认范围临时分配，不代表解析操作数/对象副本或旧编辑已接入完整预算，AR-03未完成，fuzz未运行。

- 2026-10-06 页面流元数据预算：先解析Contents并检查流数，按实际数量一次可失败预留范围/来源ID数组；预算入口分配前预留两者字节，分配后核对capacity并与源码租约一起保留到返回包装对象Drop。更新源码峰值回归加入元数据，新增两空流源码0字节仍按元数据合计拒绝、精确允许及释放归零；ContentProgram集成18项通过。解析操作数、临时默认范围、Contents/对象解析副本仍未计费，旧Clone/编辑不继承租约，AR-03未完成，fuzz未运行。

- 2026-10-06 页面源码预算贯通入口：新增content_program_with_budget与不可Clone的BudgetedContentProgram包装，旧入口共用收集逻辑保持兼容；预算入口实际调用流预算解码，内部同时转移Vec/租约，首流直接接管，后续新源码在旧源码和当次解码缓冲仍存活时预留/分配，解析期间及返回后持续持有源码容量额度。新增2＋1＋4峰值、邻居合计拒绝/文档不变、返回额度与解析失败释放回归；ContentProgram集成17项通过。解析操作数/元数据/文档对象仍未计费，借用program后使用旧Clone/编辑不继承租约，旧页面默认接口尚未迁移，不称AR-03完成，fuzz未运行。

- 2026-10-06 Group4提前EOFB补白语义回归：核对fax0.2.7发现声明高度下提前结束会补白到Rows，而非报告不足；保留当前兼容行为，扩展17列4行样本声明5行验证前4行不变、末行255白像素，以及新增第5行容量计费/结果释放。过滤器单元38项通过。该验证不代表PDF全部CCITT语义符合性，尤其不把补白称为严格拒绝或真实数据行；参数/真实样本矩阵与完整聚合预算仍未完成，fuzz未运行。

- 2026-10-06 Group3合法EOL/RTC与末行收口：发现锁定fax0.2.6对固定合法两行白色码字解码失败，采用已本地核对EOL处理的fax0.2.7补丁版本；仅保留fax及其移除fax_derive锁文件变化，恢复Cargo顺带更新的无关平台依赖。Group3改用逐行接口，先交付非空终止末行再返回，不丢弃RTC前最后行。新增固定8列2行白色码字、8＋16扩容峰值、邻居失败释放、声明行数不符归零；过滤器单元38项通过，含Group4原回归。仍不证明完整CCITT参数/真实样本矩阵，内部转换行缓冲不计费，AR-03未完成，fuzz未运行。

- 2026-10-06 CCITT输出预算入口：新增ccittfax_decode_with_budget并接入CCF/CCITTFaxDecode分派；新旧接口共用参数/限额预检，行追加可失败预留，预算扩容保持旧容量至新容量复制结束；回调分配错误停止输入并保留原错误，返回缓冲持有租约。扩展17列4行Group4测试，验证51＋68增长峰值、邻居拒绝/释放恢复、两名称输出及额度、输出限额失败归零；过滤器单元37项通过。fax库内部转换行缓冲不计费，Group3预算专项及真实样本仍待补，不称完整操作预算，AR-03未完成，fuzz未运行。

- 2026-10-06 JPEG已知fatal unwind转错误：按本地mozjpeg错误管理器行为，在DCT共享核心局部catch_unwind，仅将String且以libjpeg fatal error前缀开始的已知载荷转换为FilterError；其他panic继续resume_unwind，不修改全局hook。新增空输入、非法签名、畸形标记的旧/预算入口回归，验证返回错误且邻居额度不变、释放后归零；过滤器单元37项通过。本条收口下文已发现边界，仅适用可展开panic策略，不保护panic=abort/原生崩溃或证明所有异常JPEG；原生内存仍不计费，AR-03未完成，fuzz未运行。

- 2026-10-06 JPEG输出单缓冲及预算入口：DCT改为一次精确可失败分配并用read_scanlines_into直接写入输出，去掉RGB/CMYK像素Vec与逐像素复制；新增dct_decode_with_budget和DCT/DCTDecode预算分派，尺寸/输出限额检查后提前预留容量，返回租约。RGB/Gray/CMYK均匀像素容差、邻居合计拒绝、结果容量生命周期及输出限额专项通过，过滤器单元36项通过。原生JPEG状态/分配器瞬时额外容量不计费；本轮观察到非法JPEG依赖错误管理器resume_unwind而非返回Err，异常输入边界仍待收口，不称完整预算或健壮性验收，fuzz未运行。

- 2026-10-06 Crypt identity预算入口：抽取无分配identity参数检查，旧入口改为精确可失败复制；新增crypt_decode_with_budget并接入统一分派，检查输出限额后在同池预留容量并返回租约。新增邻居合计拒绝/恢复、输出限额、无参数/显式Identity、非Identity继续要求安全处理器及错误释放回归；过滤器单元35项通过。本次不实现加密算法或安全处理器，不改变旧参数默认规则；图片原生解码器及页面/缓存贯通仍待迁移，AR-03未完成，fuzz未运行。

- 2026-10-06 PDF crate完整回归：在当前过滤器预算入口、LZW固定字典/码宽专项及页面源码合并改动后执行cargo test -p pdf --locked --quiet，退出0；386项单元＋212项集成＝598项通过，文档测试0项，无失败/忽略/过滤。仍有既有未使用代码/导入等编译警告；本次不包含render crate、独立阅读器或真实文件矩阵，不代表AR-03或项目整体完成，fuzz未运行。

- 2026-10-06 LZW码宽与满表专项：新增独立按明确literal序号编码的4200字符流，分别覆盖EarlyChange=0/1的9→10→11→12位切换、4096项满表后继续读取、末项4095引用、12位Clear后回到9位及结束码；固定期望字节同时验证旧/预算接口及返回额度释放。过滤器单元34项通过。仅证明本专项构造流，不代表真实文件矩阵或完整预算验收；页面/缓存贯通仍未完成，AR-03未完成，fuzz未运行。

- 2026-10-06 LZW输出预算与分派：新增lzw_decode_with_budget，新旧入口共用固定字典解码核心，预算扩容预留新容量并保留旧容量至复制结束；预测器原地处理后返回带容量租约缓冲。统一预算分派加入LZW/LZWDecode。新增最终9＋10字节扩容峰值、邻居合计拒绝/恢复、两种名称返回生命周期及输出超限释放回归；过滤器单元33项通过。16KiB固定栈存储、分配器瞬时额外容量未计费，码宽/满表专项与页面/缓存贯通仍未完成，AR-03未完成，fuzz未运行。

- 2026-10-06 LZW字典收口：以固定4096项前缀/后缀数组和展开栈代替Vec<Option<Vec<u8>>>及逐码entry/previous克隆；Clear重置有效条目边界，不分配新表，输出追加先限额检查再可失败预留。新增前向码重复展开、Clear后重建/旧条目失效和输出上限回归；过滤器单元32项通过。固定数组约16KiB栈空间未计费，输出扩容仍未接入共享租约，码宽切换/满表与真实样本专项待补，不称完整LZW预算验收，AR-03未完成，fuzz未运行。

- 2026-10-06 页面源码合并首流转移及扩容收口：直接接管首个解码Vec，后续按不超过max_decoded_bytes请求的几何容量可失败扩容，避免大量微小流逐次精确扩容造成反复复制。新增实际页面首空流、逐字节流/交错空流与跨流字符串/未知操作验证；ContentProgram集成16项通过。实际分配capacity可能大于请求且旧/新容量共存仍未共享计费，不宣称完整预算或性能基准验收，AR-03未完成，fuzz未运行。

- 2026-10-06 页面源码收集去全量中间流集合：content_program改为逐流解码并追加到合并源码，记录原始流范围后释放当次解码缓冲，不再保留所有Vec流再执行完整拼接复制；继续不插分隔符，保留跨流token、空流边界、来源ID与文档修订快照。ContentProgram集成15项、嵌套文字编辑6项通过。源码扩容仍可能产生新旧容量共存，解码仍为兼容接口，解析对象/源码/元数据尚无持续共享租约，不称AR-03完成，fuzz未运行。

- 2026-10-06 PdfStream预算解码接口：新增decode_data_with_budget，保留解码深度约束，新旧入口共用外部流拒绝、状态分支与Filter/DecodeParms引用解析；预算入口调用预算链，已解码/无过滤流按容量复制并返回租约，不提前用旧接口解码未迁移的JBIG2Globals。新增间接Filter两层链、邻居合计拒绝、源载荷/状态不变、结果持续计费和已解码流回归。流单元5项、PDF全部单元383项通过。存储载荷、字典/引用解析副本与原生状态未计费，既有页面/解析调用和持久缓存尚未迁移，AR-03未完成，fuzz未运行。

- 2026-10-06 统一预算过滤器分派与链：新增apply_filter_with_budget/apply_filters_with_budget，接入ASCIIHex/ASCII85/RunLength/Flate及别名；新旧入口共用泛型链结构预检，前层BudgetedBytes在下一层解码期间保持计费，空链按容量复制并保留租约。新增两层ASCIIHex共存7＋3精确允许、邻居合计拒绝及失败释放、返回租约和空链回归；过滤器单元31项通过。未迁移解码器在新预算入口明确报错，不静默绕过预算；旧兼容入口保留，借用输入需由所有者计费，持久流/缓存尚未迁移，AR-03未完成，fuzz未运行。

- 2026-10-06 Flate共享预算入口：新增flate_decode_with_budget，按固定块读取，输出扩容先在同池预留新容量，旧容量在复制期间继续计费，精确可失败分配并核对实际capacity；原地预测器处理后返回BudgetedBytes持续持有租约。新增8192→8193增长峰值、邻居合计拒绝/释放恢复、输出超限和zlib校验失败归零回归；过滤器单元30项通过。zlib内部状态、固定栈块和分配器瞬时额外容量不计费，旧分派/持久流尚未迁移，AR-03未完成，fuzz未运行。

- 2026-10-06 PNG预测器原地还原：完整行/过滤值预检后在输入Vec中直接还原并压缩掉行过滤字节，最后truncate，消除完整输出分配与输入/输出双缓冲。新增指针和容量保持回归；过滤器单元29项通过。保留原输入容量，不以缩短长度代替容量计费；过滤器链共享租约及真实PDF像素验收仍待接入，AR-03未完成，fuzz未运行。本条更新下文此前双缓冲状态。

- 2026-10-06 PNG预测器专项验收：新增两项独立固定字节期望回归，覆盖0～4过滤方式、首行上邻居为零、跨行依赖、RGB多字节像素、16位分量按字节回绕、1位打包行长度，以及非法过滤值和截断行拒绝。过滤器单元28项通过；尚未进行真实PDF像素对比，也未完成输入/输出共存的共享预算接入，AR-03仍未完成，fuzz继续延期且未运行。

- 2026-10-06 PNG预测器去逐行缓冲：预检行步长和完整长度后按实际输出长度一次可失败预留，在最终输出中直接还原0～4过滤行，左/上/左上引用已解码结果，不再分配previous_row或current_row副本；移除旧私有逐行分配辅助函数。过滤器单元26项通过，git diff --check通过。输入解压载荷与最终预测输出仍共存且未共享计费，真实样本/完整预算仍需验收，AR-03未完成，fuzz未运行。

- 2026-10-06 Flate可失败输出扩容：read_to_end自动增长改为8KiB固定栈块读取，每块追加前检查受限输出长度并try_reserve，分配失败返回ResourceLimit；保留limit＋1探测与既有zlib错误/预测器处理。新增零输出、8191/8192/8193边界精确允许及少1拒绝回归。固定栈块、解码器内部、Vec扩容共存和预测器尚未共享计费，不宣称完整峰值保护，AR-03未完成，fuzz未运行。

- 2026-10-06 ASCII85预算解码入口：无分配组访问器采用固定5位栈组，移除分组Vec及尾组克隆，先校验/累计受限长度，再一次精确可失败预留输出；新增ascii_85_decode_with_budget分配前同池预留并返回带租约缓冲。新增z＋部分尾组、终止后忽略、邻居合计拒绝/释放恢复、z插组/单尾字符/32位溢出/畸形结束序列归零回归。固定栈组及分配器瞬时额外容量不计费，过滤器分派/持久流仍未迁移，AR-03未完成，fuzz未运行。

- 2026-10-06 RunLength输出预算入口：统一无分配段访问器，先验证字面/重复段并累计受限长度，再一次精确可失败预留输出，旧接口不再逐段隐式扩容；新增run_length_decode_with_budget分配前预留、返回BudgetedBytes。新增混合AB＋CCC、终止符后忽略、邻居合计超限、释放恢复、输出超限/截断段归零回归。过滤器分派/持久流仍未迁移，不宣称完整峰值保护，AR-03未完成，fuzz未运行。

- 2026-10-06 ASCIIHex输出预算入口：去掉逐对String及奇数末尾format分配，统一无分配半字节访问器；预检计算受限输出长度，再一次可失败精确预留。新增ascii_hex_decode_with_budget分配前预留同池额度，返回BudgetedBytes，旧接口复用同一解码核心。新增邻居合计拒绝/释放恢复/空白与奇数补零/终止符后忽略/输出超限及非法字符失败归零回归；过滤器分派尚未迁移，分配器额外容量仍分配后核对，不称完整峰值保护，AR-03未完成，fuzz未运行。

- 2026-10-06 过滤器链全结构前置验证：使用无分配借用stage访问器统一名称/参数解析，先扫描全链检查名称与参数类型，再开始解码；避免第一层输出分配后才发现后续非法项。新增第一层在零额度下原会超限、后续非法名称项/参数项仍优先FilterError回归。共享峰值租约及持久对象仍未接入，AR-03未完成，fuzz未运行。

- 2026-10-06 多过滤器链去副本专项：新增两层ASCIIHex合法链（第一层616263>、第二层abc），验证第二层使用前层结果、7字节中间输出精确允许而限额6拒绝，即使最终仅3字节仍检查中间层；两项Null参数正常、DecodeParms长度不匹配拒绝。此前去副本实现未接入共享租约，仍不宣称合计峰值保护，AR-03未完成，fuzz未运行。

- 2026-10-06 过滤器数组去输入副本：第一层解码直接借用原始载荷，后续层只持有上一层输出，不再预先input.to_vec；空过滤器数组按输出限额先检查再可失败预留复制。新增空数组精确3字节/超限4/零限额空串回归；此前filter单元19项通过，新回归另见当前测试结果。尚未将过滤器结果/持久流对象接入共享租约，AR-03未完成，fuzz未运行。

- 2026-10-06 带额度读取失败验收：新增可定位读源读2字节后注入I/O错误，验证临时4字节缓冲额度释放、源位置保持实际推进2、邻居3字节内容/额度不变，显式seek后较小读取成功且所有Drop归零；扩展usize::MAX分配失败验证池归零及位置不推进。仍不自动回滚I/O位置，不代表旧解析/解码调用已迁移，AR-03未完成，fuzz未运行。

- 2026-10-06 带额度读取接口：StreamReader新增read_bytes_with_budget，先检查剩余输入、分配前预留并核对实际capacity，返回BudgetedBytes保持额度所有权；预算/输入预检失败不推进，实际I/O失败可部分推进但释放缓冲额度。新增同池多次读取合计拒绝、位置保持、输入越界、释放恢复和成功内容回归。旧read_bytes仍保留，解析/解码调用方尚未迁移，不将新接口存在视为管线已接入，AR-03未完成，fuzz未运行。

- 2026-10-06 ContentProgram解析去整份副本：ContentParser泛化为Read＋Seek读取器并保留Cursor<Vec<u8>>默认类型，ContentProgram直接借用保留源码的游标解析，操作符/操作数仍独立所有权，解析器释放后转移源码，不再分配parse_bytes整份复制。初次借用改动因读取器固定类型编译失败，泛化后content_program15、nested_text_edit6及content_parser单元4共25项通过，git diff --check通过。源码/操作对象/事务与持久对象完整预算仍未接入，AR-03未完成，fuzz未运行。

- 2026-10-06 ContentProgram改写去逐流中间副本：with_replaced_operation_sequence按已检查总长度一次预留新源码，直接拼接各原流前后片段并记录范围，不再先分配每流Vec再拼接第二份新源码；保留空流/跨流操作边界及原有邻接操作/操作数验证。content_program15项通过，覆盖跨流与源码保留行为。解析副本、结果对象/事务及持久流预算仍需接入，不宣称完整峰值保证，AR-03未完成，fuzz未运行。

- 2026-10-06 跨TJ全数组前置结构校验：统一范围验证在对象数量上限内检查全部条目，仅允许字符串/数字，非法项即使不在选中范围也在字体加载/编码前拒绝；新增布尔/null/名称/嵌套数组/字典五类中间项及范围内外回归，数字调整/空字符串合法样本仍通过。新增单元1项通过（367筛选）。本次不扩展布局或完整预算能力，AR-03未完成，fuzz未运行。

- 2026-10-06 跨TJ端点校验统一：顶层/嵌套Unicode范围准备及底层结果构造共用validate_text_array_range，统一数组对象上限、端点顺序、字符串项和字节偏移检查；顶层也在字体加载/编码前拒绝非法端点，字符边界仍由字体访问器检查。既有额度单元补数字端点/usize::MAX偏移/逆序范围配不可编码中文均ObjectError且修订保持，1项通过；nested_text_edit6项通过（0.83秒）。完整预算/布局补偿仍未完成，fuzz未运行。

- 2026-10-06 嵌套Unicode准备前置验证：来源/Form链校验后、字体scope加载和编码前统一验证整串/单字符串范围的目标项及范围长度，跨TJ范围验证目标、端点顺序、字符串项与偏移；字符码边界仍由字体解析确认。既有嵌套回归补充数字项/usize::MAX搭配不可编码中文仍优先ObjectError、修订保持且后续合法替换成功；nested_text_edit6项通过。未改变布局/补偿语义，完整预算仍未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 TJ单项准备阶段前置验证：顶层Unicode单项替换在字体加载/编码前检查目标TJ单操作数及字符串项，数字项/越界项立即返回ObjectError；来源校验仍由提交入口先执行。既有额度单元扩展不可编码中文搭配数字/usize::MAX目标仍优先结构拒绝、修订保持及后续合法替换通过，1项通过；text_source9项通过。提交流副本转入持久对象尚缺文档级额度转移，不将提交返回即释放当作完整预算，AR-03未完成，fuzz未运行。

- 2026-10-06 Simple固定候选满容量验收：新增自行构造256条ToUnicode映射，公开/预算编码逐字验证00～FF全部字符码，包含NUL及最高槽位FF；max_objects255拒绝且池归零，恢复256后ÿ编码FF成功，返回仅输出capacity计费并释放归零。新增单元1项通过（366筛选），git diff --check通过。仅候选容量边界证据，不代表真实字体显示或完整操作预算，AR-03未完成，fuzz未运行。

- 2026-10-06 Simple候选容器去堆分配：简单字体至多256字符码，候选表改为固定256槽Option数组，映射借用/内联且仅遍历有效前缀，不再分配/扩容候选Vec；max_objects仍按有效候选数检查，局部工作量上限仍计有效结构，公开编码接口不变。反向编码2项通过。固定栈表未计入共享额度，源字体、解释/事务/缓存/保存亦未贯通，不宣称完整进程峰值，AR-03未完成，fuzz未运行。

- 2026-10-06 Simple候选映射去字符串复制：内部EncodingMapping区分借用ToUnicode字符串和内联char标量，不再在枚举0～255候选时逐项生成String；匹配长度、最长可编码后缀及低码优先规则保持，公开unicode_mapping接口不变。局部工作量上限改计候选元组结构，不重复计借用文本；反向编码2项通过。候选Vec容量尚未共享计费、源字体仍未计费，AR-03未完成，fuzz未运行。

- 2026-10-06 字体编码输入工作量收口：Simple/Type0的公开及预算编码共用输入预检，字节超限立即拒绝，Unicode标量计数最多取max_text_chars＋1（饱和加法），避免先扫描完整超限文本；字符码及编码选择语义不变。新增多字节标量、字节/字符独立超限、零限额空串、usize::MAX不溢出回归1项通过；既有反向编码5项通过。仍不代表完整CPU预算或Simple候选表/全管线内存接入，AR-03未完成，fuzz未运行。

- 2026-10-06 Simple后缀表共享预算接入：预算编码转至可选操作池内部实现，对choices槽位分配前预留、分配后实际capacity核对，与输出额度共存，返回/失败释放表额度；旧公开无预算编码语义不变。顶层整串/范围与嵌套单字符串额度按实际编码峰值更新，跨TJ仍保留结果数组共存限额。预算边界单元2、字体映射单元1、nested_text_edit6共9项通过，git diff --check通过。Simple候选映射Vec/字符串及字体加载/解释/事务/缓存/保存未计费，AR-03未完成，fuzz未运行。

- 2026-10-06 AR-03架构现状校正：对照Type0编码及文字编辑实际接入，修正候选表/后缀表未计费、跨TJ仅计编码、其他Unicode入口未共享池等过时描述，明确每次操作独立池及已接入与未计费范围。仅文档更新，git diff --check通过，未新增代码验收或重跑测试，AR-03保持未完成，fuzz未运行。

- 2026-10-06 Type0候选表扩容共存边界验收：混合映射单元新增短输入a，使4槽旧表＋8槽新表＋4输出请求的峰值大于最终8槽候选表＋2槽后缀表＋4输出；最终容量额度拒绝并归零，精确共存峰值额度成功，编码返回仅输出capacity存活且Drop归零。1项通过（364筛选），并将仅用于生命周期的候选租约变量标为明确保留，消除新增未读警告。Simple临时表及全管线预算仍未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 Type0候选表共享预算接入：预算路径为借用Unicode字符串的候选元组Vec按实际capacity计费，倍增且受max_objects约束，扩容先另行预留新表、旧表仍计费，分配/容量核对/复制完成后才替换旧表与租约；旧无预算公开接口保留原行为。混合字符码单元按输出＋候选表＋后缀表精确额度验收，少1拒绝、失败归零及返回只保留输出通过（1项，364筛选）。Simple临时表、字体加载和解释/事务/缓存/保存仍未接入，AR-03未完成，fuzz未运行。

- 2026-10-06 Type0后缀选择表共享预算接入：既有编码实现抽为内部可选操作池路径，公开旧编码接口保持原行为；预算编码在输出额度存活时，对choices按槽位数×Option元组大小分配前预留，分配后核对实际capacity，租约声明先于Vec以保证析构顺序，返回/失败时释放临时表额度。混合1～4字节编码单元更新按输出＋选择表共存容量计算精确额度，少1拒绝且池归零、返回仅输出容量存活、邻居耗尽时合计拒绝均通过（1项，364筛选）。候选映射/字符串、Simple后缀表及解释/事务/缓存/保存仍未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 TJ所有者异常展开验收：既有跨TJ单元新增同池存活BudgetedBytes邻居、带租约结果数组及借用表同时持有时注入panic，catch_unwind后验证结果/表临时额度均释放、邻居内容和capacity额度保持、邻居Drop后归零；1项通过（364筛选）。本轮仅扩大所有权异常路径证据，未实现字体临时表计费，AR-03仍未完成，fuzz未运行。

- 2026-10-06 Type0编码输出前置预留：新增内部预算编码入口，输入限额校验后按既有输出请求容量Unicode标量数×4检查溢出并预留，编码返回Vec转移至BudgetedBytes，Font分派改用此入口，所有已接入Unicode编辑路径受益。既有1～4字节/多标量映射单元扩展验证20容量成功、19拒绝、持有结果时另一编码合计拒绝、Drop恢复及不可编码失败恢复，1项通过（364筛选）；nested_text_edit6项通过（0.80秒）。更新架构状态；字体映射/后缀表、解释/事务/缓存/保存仍未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 顶层整串编码前置接入：页面内部encode_text_for_operation显式接收共享预算并返回BudgetedBytes，顶层整串及TJ单项替换移除编码后独立预留，统一使用Font预算分派；Simple输出请求容量编码前预留，Type0仍编码后计费，字体选择/字符码语义不变。整串额度边界单元1项通过（364筛选），text_source9项通过；更新架构过时描述。字体临时表、Type0前置预留及解释/事务/缓存/保存仍未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 顶层范围编码前置接入：Font新增内部预算编码分派，Simple分支调用编码前输出容量预留，Type0暂保持编码后容量计费；顶层单字符串范围、跨TJ范围与嵌套Unicode编辑共用该入口，顶层范围编码Vec/独立租约改为BudgetedBytes只读访问。nested_text_edit6项通过（0.84秒），顶层代码/结果共存额度边界单元1项通过（364筛选）；不改变字符码语义。顶层整串/单项入口、Type0前置预留及字体临时表/解释/事务/缓存/保存仍需贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 嵌套简单字体编码输出预留：SimpleFont新增内部encode_text_with_budget，在既有输入上限校验后按Unicode标量数预留既有编码器的输出请求容量，编码返回Vec转移为BudgetedBytes；嵌套整串/单字符串范围/跨TJ均使用该入口。既有反向编码单元扩展验证多标量映射输出、返回容量计费、另一编码合计拒绝、Drop恢复及不可编码失败释放，1项通过（364筛选）；nested_text_edit6项通过。映射/后缀表未计费，Type0和顶层编码尚未前置预留，分配器额外capacity仍为分配后核对，AR-03未完成，fuzz未运行。

- 2026-10-06 顶层整串Unicode预算接入：replace_page_text及replace_page_text_array_item将编码返回Vec实际capacity计入操作池，转移到BudgetedBytes并保留到提交结束。新增Tj/TJ两分支额度2拒绝XYZ、保持修订/源字节/流对象ID、失败后同快照XY精确额度成功单元1项通过（364筛选），text_source9项通过；架构文档更新。只约束编码返回载荷，编码临时表及ContentProgram/事务/缓存/保存仍未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 跨TJ结果所有权收口：新增私有BudgetedTextReplacements，将结果Vec/各字符串容量与总租约绑定，仅只读切片访问；接受外部租约的底层函数收为模块私有，顶层与嵌套均通过返回带额度的构造路径使用，所有者保留到实际提交返回。既有单元扩展验证返回后全部capacity计费、Drop归零和输出超限失败归零，1项通过（363筛选）；nested_text_edit6项通过。架构文档更新；字体、源码重建、Form链、事务/缓存/保存尚未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 嵌套文字缓冲所有权收口：Unicode编码Vec转移到BudgetedBytes，单字符串范围结果另行预留后转移，旧代码/新结果共存计费，赋值后才释放旧所有者，结果及租约保留到嵌套提交返回；整串/范围/跨TJ入口共享只读访问，无额外载荷复制。nested_text_edit6项全部通过（0.80秒），涵盖精确额度/少1拒绝/失败重试、共享调用隔离、字符边界、严格保存读回与事务回滚；架构文档更新。编码临时表、跨TJ结果数组所有权、解释/事务/缓存及保存预算仍未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 顶层范围结果所有权收口：BudgetedBytes新增内部已预留Vec转移构造，核对实际capacity并绑定租约，无载荷复制；replace_page_text_range在外层保留实际结果所有者至提交结束，替代独立结果租约。新增指针不变、容量持续计费、析构释放及容量超限失败恢复单元1项通过，既有顶层文字范围合计额度边界单元1项通过（各363筛选）；本轮未重跑全工作区，此前604项记录不包含此变更。仍不覆盖字体编码临时表、ContentProgram、事务及保存峰值，AR-03未完成，fuzz未运行。

- 2026-10-06 预算接入后完整工作区回归：cargo test --workspace --locked --quiet最终退出0，PDF单元363、PDF集成211、render30共604项通过，0失败/0忽略；其余目标及文档测试无测试项。覆盖近期字体访问器、独立预算配置、顶层/嵌套文字范围额度及TJ结果/借用表所有权改动，render30项17.41秒通过；仍有既有未使用项及Cargo配置弃用警告。同步修正AR-03原语子项的过时描述，明确部分接入与端到端预算未完成；不代表真实复杂字体、完整独立阅读器矩阵或未勾选能力已完成，fuzz未运行。

- 2026-10-06 构建示例独立验收：现有create_document生成两页文件，qpdf 11.2.0结构/流编码检查通过且页数2；MuPDF 1.27.0正确提取两页英文标题与页码，按PDF技能逐页查看渲染图，文字及红蓝图片无明显裁切/重叠。命令、SHA-256和产物位置见docs/testing.md；本机Poppler缺旧libtiff、Swift/PDFKit JIT失败未算通过，未修改系统依赖。仅验证该标准字体/RGB构建样本，复杂字体、编辑/导入保存及完整独立验证矩阵仍待验收，AR-04/P1不整体勾选，fuzz未运行。

- 2026-10-06 TJ借用表所有权收口：内部BudgetedTextItems将私有表Vec与MemoryLease绑定，构造时统一预留/核对槽位容量，顶层及嵌套共用该构造，仅暴露只读切片，表返回后额度不提前释放。新增精确额度、构造返回保留、析构释放和少1失败计数恢复单元1项通过（362筛选），nested_text_edit6项全部通过，git diff --check通过。保留既有共存额度语义；字体/源码/对象/缓存/保存峰值预算仍未完成，fuzz未运行。

- 2026-10-06 跨TJ借用表计费：顶层/嵌套构建(数组索引,&[u8])借用表前，同池预留len×size_of槽位，try_reserve_exact后按实际capacity核对；租约保留至写出/嵌套提交返回。预算回归计入代码、两字符串结果、两个结果元数据槽及两个借用槽，在顶层和嵌套均验证合计少1拒绝、精确值成功及失败后较小操作恢复；nested_text_edit6项全部通过，git diff --check通过。只覆盖这些结果载荷/结构，编码临时表、字体、ContentProgram重建/解释、Form链/事务对象、缓存和保存仍未贯通，AR-03未完成，fuzz未运行。

- 2026-10-06 跨TJ数组元数据预算：规划选中字符串数量，预留字符串计划长度＋结果数组(usize,Vec<u8>)槽位字节，一次try_reserve_exact整个结果数组，核对实际capacity后才生成各字符串，避免逐项增长数组的旧新容量遗漏。嵌套Tj/TJ预算回归按平台size_of计入两槽元数据，合计额度少1拒绝、精确值成功且共享调用/失败重试保持；nested_text_edit6项全部通过，git diff --check通过。借用表、字体/源码/链/事务对象及缓存/保存仍未计费，AR-03未完成，fuzz未运行。

- 2026-10-06 跨TJ结果缓冲计费：公共跨项辅助函数在输出规划阶段累计仅选中字符串的计划结果长度，在分配前以调用方租约预留；逐项分配后按实际capacity增加超请求部分。顶层/嵌套入口让该租约与编码容量共用同池并保留到提交结束；未选借用字符串不重复计费。嵌套预算回归扩为Tj/TJ两变体，代码3＋跨两项结果4+1，上限7拒绝且状态/资源不变、较小操作重试成功，上限8成功，第二调用保持；单元text_array_range1项及nested_text_edit6项通过，git diff --check通过。数组元数据/借用表、字体/源码/链/对象克隆等未计费，分配器超容量仍在分配后核对，AR-03未完成，fuzz未运行。

- 2026-10-06 嵌套Unicode范围输出预算：replace_nested_unicode共用同次操作池计费编码结果capacity；单字符串范围另预留/核对替换capacity，旧码缓冲销毁后释放旧租约，新租约留到嵌套提交结束。新增两次重复Form调用AB插入XYZ，旧编码3＋结果5合计8，上限7无修改拒绝且revision/Resources/根流字节与ID保持，之后较小替换成功；上限8成功且另一次调用保持AB。nested_text_edit现6项全部通过，git diff --check通过。跨TJ项仅编码结果计费，其他结果与链/字体/源码/对象克隆、缓存/保存尚未统一接入，编码预检仍晚于编码分配，AR-03未完成，fuzz未运行。

- 2026-10-06 操作预算初接顶层范围：replace_page_text_range一次创建Limits操作池，Unicode编码完成后计费codes实际capacity，替换分配前预留计划长度、分配后核对capacity，替换租约留到content commit结束。新增旧AB插入XYZ时编码3＋结果5合计8，上限7拒绝且revision/根流字节/ID不变、随后较小操作成功，上限8成功；专项1项及text_source9项通过，git diff --check通过。仅此顶层单字符串路径的两块结果缓冲计费，编码前临时表/字体加载、ContentProgram源和重建、事务克隆/缓存/保存及嵌套/跨项入口尚未接入；编码额度在分配后检查，不宣称严格瞬时峰值，AR-03保持未完成，fuzz未运行。

- 2026-10-06 独立操作预算配置：Limits新增max_operation_bytes默认512MiB，与单项解码/缓存限制分开；operation_budget工厂创建独立池，同次操作须共享同一个句柄。新增配置5字节、解码100字节且两个3字节缓冲合计拒绝/释放恢复、零额度回归，operation_limit过滤2项通过（新增1项＋既有1项，359筛选）；cargo check --workspace --all-targets --locked --quiet退出0，git diff --check通过。全字段Limits字面量需补成员，默认展开维持默认配置；旧解析/编辑/保存入口尚未自动接入，不宣称配置即有端到端峰值保护，AR-03未完成，fuzz未运行。

- 2026-10-06 预算字节缓冲扩容：BudgetedBytes新增try_extend_from_slice，容量内追加不分配；需要增长时为新缓冲另建租约，在旧容量仍计费时预留新容量，再复制并整体替换。失败不改变原内容/容量/计数，空追加不计新额度。新增旧3＋新6超8拒绝、精确9成功、相邻所有者保留及失败恢复回归；budget::tests共8项通过（含执行预算1项，352筛选），git diff --check通过。仍存在分配器超请求容量分配后核对边界，实际编辑/缓存/保存尚未接入，AR-03未完成，fuzz未运行。

- 2026-10-06 预算绑定字节所有权：新增BudgetedBytes，将不可复制租约与私有不可变Vec绑定，不提供丢弃租约的裸Vec转换。复制先预留请求字节，再try_reserve_exact并核对实际capacity；try_clone在同池另行计费原/副本共存，返回或移动后额度保持，缓冲销毁后释放。新增返回/移动及合计超限复制/克隆失败恢复测试2项；budget::tests共7项通过（含既有执行预算1项，352筛选），git diff --check通过。分配器可能超请求容量，核对发生在分配后保留前，不能据此承诺严格分配瞬时进程峰值；尚未接入字体/编辑/缓存/保存或可增长缓冲，AR-03仍未完成，fuzz未运行。

- 2026-10-06 AR-03预算原语：新增公开budget::OperationBudget共享Rc活跃字节池及不可Clone的MemoryLease，reserve/resize检查合计额度和溢出，失败计数不变，移动保留额度，析构按自身额度释放。四个新增测试覆盖单项合规合计超限/精确上限、调整与移动、unwind保留已有所有者、零与usize溢出；budget::tests过滤共5项通过（含既有执行预算1项，352筛选），git diff --check通过。只勾选原语子项；尚未接入Limits、字体/编辑、容量增长、缓存或保存，也不计预算管理结构本身及原生内存，AR-03父项保持未完成，fuzz未运行。

- 2026-10-06 AR-03接入审计：核对ExecutionBudget与字体编码/范围/事务缓冲边界，确认现有计数为工作量或局部逻辑上限，不是共享活跃内存池。架构文档明确独立配置、不可复制租约、返回缓冲额度转移、克隆/容量/缓存/保存接入及失败/unwind验收顺序，TODO拆分全部未勾选；避免局部函数返回提前释放仍存活结果额度。此次为实施契约细化，无新增预算实现或测试验收，fuzz未运行。

- 2026-10-06 SimpleFont解码契约收口：简单字体新增内部受限访问器，既有字符向量收集保留一次精确可失败预留并复用逐码宽度校验，Font枚举仅转发到Simple/Type0访问器，不再独立维护简单字体解码逻辑。SimpleFont单元4、nested_text_edit5及text_source9共18项通过，git diff --check通过；公开chars/范围API不变。未实现统一操作内存预算，fuzz未运行。

- 2026-10-06 Type0访问器竖排契约回归：非Identity源码41映射CID7，直接访问器验证W2指定(-1200,350,900)及未映射码DW2回退(-1000,50,880)，与收集接口逐项code/length/width/origin完全一致；字符上限1只回调一次，回调失败不访问后续码。Type0全部6项通过（347筛选），git diff --check通过。是度量与遍历契约，不替代真实竖排字形/布局像素或统一峰值预算，fuzz未运行。

- 2026-10-06 Type0编码读回去向量化：复合字体Unicode编码的最终Encoding/ToUnicode读回改用CMap受限访问器，逐码验证文本前缀并检查完整结束，移除完整读回CharCode向量；工作额度公式相应移除已不存在的字符数组项，保留输出与候选/动态规划表计费。Type0单元6项通过（347筛选；同命令集成被过滤不计），另运行nested_text_edit5项全部通过，git diff --check通过；docs/content-editing.md同步真实计费范围。不是统一操作峰值预算或CPU预算，fuzz未运行。

- 2026-10-06 字体访问器接入范围编辑：Font内部受限访问器直接验证SimpleFont宽度并逐码回调；Type0经CMap访问器逐码应用水平宽度或竖排宽度/原点，既有chars_limited改为同一路径收集。text_range_replacement及跨TJ输入/输出计数均不再为边界/数量校验创建完整CharCode向量，保留有效字体、剩余字符额度和字符码完整推进校验。PDF单元353、nested_text_edit5、text_source9共367项通过，git diff --check通过；未覆盖编码候选表/字体加载/结果缓冲及事务克隆等统一峰值预算，AR-03保持未勾选，fuzz未运行。

- 2026-10-06 CMap受限访问器：将解码推进/字符上限/错误处理集中到内部visit_chars_limited，逐个传递CharCode并返回数量，不要求收集结果Vec；现有chars_limited以访问器可失败预留并收集，公共接口保持。校验正向有效推进、剩余输入范围，测试额度耗尽不继续回调、回调错误立即返回、空输入零额度与精确数量。CMap18项通过（335筛选），git diff --check通过。访问器自身不保留完整向量，但字体/范围入口尚需迁移，未将完整峰值预算标为完成，fuzz未运行。

- 2026-10-06 CMap逐字符临时分配清除：next_char直接借用输入的1～4字节候选切片，移除每字符临时Vec，保持最短有效codespace匹配；剩余输入长度约束偏移运算。新增1/2/3/4字节码、每种截断、末尾/越界/usize::MAX偏移、空输入和非法码回归。CMap专项17项通过（335筛选），git diff --check通过。只消除该局部堆分配，不宣称整字符向量/字体验证或操作峰值预算完成，fuzz未运行。

- 2026-10-06 TJ字符工作预算前移：原整TJ字符数在输出规划遍历中预检，逐项chars_limited使用剩余输入字符额度，全部合规后才生成替换字符串；最终结果逐项解码同样使用剩余输出字符额度。补充旧串4字符、输入上限3时即使全删到空也拒绝的单元回归，text_array_range1项通过（350筛选），git diff --check通过。执行树ExecutionBudget仍是工作量而非可释放内存计数，此修改不覆盖编码输入、容量/字体/对象/缓存/保存缓冲峰值，AR-03仍未完成；上条591全工作区记录早于本次改动，不混用验收范围，fuzz未运行。

- 2026-10-06 最新全工作区回归：`cargo test --workspace --locked --quiet`最终退出0，PDF单元351、PDF集成210、render30共591项通过，0失败/0忽略；其余目标/文档测试无测试项，示例编译通过，git diff --check通过。包含跨TJ批量/Unicode范围/限额/回滚/共享页及多色图片方向与替换、Helvetica跨项像素等近期变更。存在既有未使用项与Cargo配置弃用警告；不代表真实复杂字体、独立阅读器、推进补偿、完整预算等未勾选验收完成，fuzz未运行。

- 2026-10-06 TJ批量序列化限额：两字符串同时扩展为20码字节，测试最终完整程序max_decoded_bytes与目标操作片段max_raw_stream_bytes各自精确上限成功、少1字节分别拒绝ResourceLimit，原快照不变。首次测试误把原始额度当完整程序额度导致断言失败，按现有目标片段语义修正并明确区分，不据此声称统一预算。content_program最终15项全部通过，git diff --check通过，fuzz未运行。

- 2026-10-06 跨TJ实现后扩大回归：`cargo test -p pdf --lib --test content_program --test content_edit_transaction --test text_source --test text_remove --test nested_text_edit --locked --quiet`退出0，PDF单元351、混合事务2、内容程序14、嵌套文字5、文字删除5、文字来源9共386项通过，0失败/0忽略，git diff --check通过。验证公共TJ批量写出/范围预检/嵌套选择类型与既有顶层入口；不是最新全工作区验收。执行队列明确同TJ跨字符串已具专项证据，下一步仍为真实复杂字体、跨事件/推进补偿、命中及独立阅读器；fuzz未运行。

- 2026-10-06 顶层跨TJ事务验收：非法端点检查revision/Resources不变，外层事务内替换暂时提取AXH后注入失败，根流字节/ID、Resources和原ABCDEFGH恢复；旧快照拒绝，刷新后é€跨项替换成功。保留顶层结果副本到最终严格保存重开，验证Aé€H及原共享页，不只验证后续嵌套结果。测试中副本插入索引先出现PageNotExist，修正为有效尾部插入并验证后续页序移动；nested_text_edit最终5项全部通过，git diff --check通过。fuzz未运行，完整计划未完成。

- 2026-10-06 嵌套文字编辑选择类型收口：内部NestedTextEdit明确区分WholeString、StringRange及ArrayRange，替代item/range/array_range三个独立Option，避免单字符串范围与跨项范围同时存在或被分支顺序静默覆盖。公共接口不变，仍共用调用链有效字体、编码和事务提交；嵌套文字5项全部通过，git diff --check通过。属于内部契约整理，不据此认定完整架构或编辑计划完成，fuzz未运行。

- 2026-10-06 跨TJ项输出预检：公共范围辅助函数先借用遍历全部数组项，以checked运算预测替换后的总字符码字节量并拒绝超限，再分配各字符串替换缓冲，避免晚于逐项分配才暴露总逻辑输出超限。原有最终累加检查保留；不是字体加载/编码输入/字符数组/对象克隆等统一峰值预算，AR-03未完成。text_array_range单元1项及nested_text_edit5项通过，git diff --check通过，fuzz未运行。

- 2026-10-06 跨TJ项Helvetica像素：两层Form/BBox与图片混合样本新增[-25(MM)25(MM)]TJ变体，跨首/末字符串替换WWWW及é€，再单字符串范围改éA；参考页显式使用[-25 新码 25 <>]TJ，数字保留且末项清空。原/严格重开整页RGBA一致，第二调用、共享页及先前删除页保持，提取文字和é€字符码正确。render全部30项通过（该测试现7变体），git diff --check通过；只验证本库Helvetica像素和既定不补偿策略，真实复杂字体/跨事件推进补偿及独立阅读器仍待验收，fuzz未运行。

- 2026-10-06 跨TJ项复合字符码：合成Type0/CMap样本在两个双字节字符串间包含空字符串、数字和注释；起始/结束半字符偏移及半字符零长度插入均拒绝且revision不变。替换输入4字符与旧串2字符分别合规、保留末项后合计5字符时拒绝ResourceLimit；合法跨项替换为单字节中后原/严格重开目标中文文文及码长[1,2,2,2]、共享文文文文/[2,2,2,2]和原Form字节保持。nested_text_edit5项全部通过，git diff --check通过。只证明合成字符码/提取与限额，不证明真实中文嵌入字体渲染；跨项像素和连续推进补偿仍待实现，fuzz未运行。

- 2026-10-06 跨TJ项事务回滚专项：嵌套范围对数字端点、越界偏移和倒序端点拒绝，revision及Resources不变；外层事务内跨项替换暂时提取AXHABCDEFGH后注入失败，Resources、根流字节/对象ID及完整原文字恢复。回滚前快照拒绝，刷新根与来源后é€跨项替换成功并严格重开/共享页保持。nested_text_edit全部5项通过，git diff --check通过；不证明跨项复合字体或像素验收，fuzz未运行。

- 2026-10-06 跨TJ字符串Unicode范围实现：新增replace_page_text_array_range及replace_nested_text_array_range，端点为(数组项索引, 解码字符码字节偏移)，共用字符边界、原/结果整TJ字符数和结果总码字节限额校验。插入起始项、清空中间字符串、保留末项后缀以及数字/注释/未选项，不补偿字形推进。顶层非法端点拒绝、跨三字符串é€替换，嵌套重复调用/共享页与严格重开读回已验证；content_program14和nested_text_edit5共19项通过。新增公共辅助函数的空项插入、跨项替换、全删、精确额度及总字符/字节各自超限单元测试1项通过（350筛选；同命令集成项被筛选，不计入本次单元结果）。跨项复合字体、像素、外层回滚专项和跨事件编辑仍待补齐，fuzz未运行，P3-04不勾选。

- 2026-10-06 跨TJ项编辑基础：ContentProgram新增with_replaced_text_array_items_codes，一次借用源码扫描同时替换多个有序字符串项；单项入口复用批量逻辑，移除整段解析字节复制，保留间距数字、注释和未选字符串原始token。拒绝倒序、重复、数值项及越界索引，输出checked累加并可失败预留。content_program14、nested_text_edit4、text_remove5共23项通过，git diff --check通过。仅字符码批量写出基础；跨项Unicode范围映射、整体字符额度和推进补偿仍需实现，不据此勾选P3-04，fuzz未运行。

- 2026-10-06 变换后多色图片绑定替换：在四象限两层Form样本中分别先旋转、镜像、缩小平移，再刷新来源只替换第一调用为青/品红/黑/白图片；保留既有变换与BBox，硬编码颜色映射的独立矩形参考整页RGBA一致，第二调用/共享页及严格重开保持。`cargo test -p render --lib --locked --quiet`30项全部通过，0失败/0忽略；git diff --check通过。未覆盖任意仿射插值、透明纹理或独立阅读器，完整P3-04仍未完成，fuzz未运行。

- 2026-10-06 嵌套多色图片方向：新增2×2红/绿/蓝/黄四象限样本，两层Form重复调用只变换第一张图片；旋转90度、水平镜像、缩小平移分别与硬编码象限颜色的直接矩形参考整页RGBA一致，共享页与严格保存重开保持。专项1项通过，`cargo test -p render --lib --locked --quiet`全部30项通过，0失败/0忽略。此为本库后端、整数轴对齐样本，不替代任意仿射插值或独立阅读器验收；fuzz未运行，完整P3-04保持未勾选。

- 文字范围输出额度补充：顶层与嵌套入口共用拼接与字符码边界校验，新增精确字节/字符额度、各自超限及空结果零额度单元测试。`cargo test -p pdf --lib text_range_output_limits --locked --quiet`：1 通过、349 被筛选，未运行 fuzz；此记录不代表最新全工作区回归。

- 2026-10-06 公共范围重构顶层回归：cargo test -p pdf --lib --test content_program --test content_edit_transaction --test text_source --test text_remove --locked --quiet退出0；PDF单元349、内容程序13、混合事务2、文字来源9、文字删除5共378项通过，0失败/0忽略，git diff --check通过。覆盖公共数值/范围辅助函数及既有顶层入口，不把本次子集当作最新全工作区验收；跨字符串/布局补偿和完整计划仍未完成，fuzz未运行。

- 2026-10-06 局部范围文字像素：两层重复Helvetica混合样本在é€后刷新来源仅以范围1..2把€改A，Tj/两引号/TJ单项及非零leading/字符间距六变体均走范围接口；直接参考<E941>保留TJ数字或显式间距，原/严格重开整页RGBA一致、UnicodeéAMMMM、第二调用/共享页/先前删除页保持。render29项全部通过，git diff --check通过。单字符串WinAnsi局部范围，不跨TJ项、不证明复合字形或推进补偿，fuzz未运行。

- 2026-10-06 公共文字范围校验整理：提取text_range_replacement给顶层与嵌套Unicode范围入口共用，统一旧码边界/无零长度进展、范围越界、拼接字节额度/可失败预留及最终整体字符数量检查，移除两套重复实现；有效字体选择与Tj/TJ项定位仍由各入口负责。nested_text_edit4及text_source9共13项通过，git diff --check通过。公共API不变，仍不跨字符串项或补偿布局，峰值内存不据此宣称完成，fuzz未运行。

- 2026-10-06 嵌套范围整体字符限额：复合字体样本配置max_text_chars4，明确确认新输入中文中文可编码6字节/4字符，旧串2字符亦合规；只替换旧串一个字符后拼接5字符，最终chars_limited拒绝ResourceLimit且revision/Resources/根流字节及ID不变，随后合法范围成功并严格重开。nested_text_edit4项通过，git diff --check通过。为字符串整体字符逻辑上限，不替代文档/操作峰值内存或所有额度边界验收，fuzz未运行。

- 2026-10-06 复合字体嵌套范围边界：新增合成Type0/CMap一字节41→中、两字节8001→文的Form重复调用/共享页样本，范围1..2、2..3及内部空范围2..2均拒绝且revision不变；完整1..3替换中成功，原/严格重开目标中中中文/码长[1,1,1,2]、共享中文中文/[1,2,1,2]及原Form字节保持。nested_text_edit现4项通过，git diff --check通过。合成非嵌入字体验证编码/提取，不证明中文字形可见、真实字体/CMap或独立阅读器验收，fuzz未运行。

- 2026-10-06 TJ/引号局部范围编辑：两层清空恢复样本对Z字符串末端插入é，再仅首码Z替换Y，分别走TJ项0、单引号及双引号范围入口；原/严格重开ZéDABC、YéDABC、共享ABCABC保持，叶show操作名不变，TJ数字25/-10及双引号2/1操作数保持。nested_text_edit3项通过，git diff --check通过。仍为单字符串内WinAnsi范围，不跨TJ项、不补偿连续字形推进或证明范围像素完备，fuzz未运行。

- 2026-10-06 嵌套范围插入/删除边界：两层继承Helvetica样本刷新来源后在A€末端2..2插入Z，再以空Unicode替换0..3清空该字符串；原/严格重开目标依次A€ZAB、AB，第二调用与共享ABAB及原叶字节保持。nested_text_edit3项通过，git diff --check通过。覆盖单字符串边界插入/完整删除，不宣称跨TJ项/复合码或布局补偿验收，fuzz未运行。

- 2026-10-06 两层继承字体范围编辑：同名scope/q恢复样本在é€全串替换后刷新两层来源，局部首码é替换A；原/严格重开目标A€AB、共享ABAB，全字符有效Helvetica且原叶字节保持。nested_text_edit3项通过，git diff --check通过。覆盖两层无Tf叶继承的单字节范围，不推断复合码边界/TJ跨项或像素完备，fuzz未运行。

- 2026-10-06 嵌套文字范围回滚：空literal/hex再插入WinAnsi样本补外层主动Err，事务内局部é€→éA生效，回滚恢复é€、Resources及根流字节/ID，旧root拒绝；刷新后范围提交并沿既有共享页/严格重开验收。nested_text_edit3项通过，git diff --check通过。单层/主动错误，不替代复合码边界、多层TJ范围或预算失败验收，fuzz未运行。

- 2026-10-06 嵌套单字符串范围编辑：新增replace_nested_text_range，接入调用链有效字体/Unicode编码与复制事务，按PDF字符码字节范围校验完整边界，输出拼接额度及最终整体字符数量；支持Tj/引号或单TJ项，不跨项/不补偿推进。空literal/hex插入样本先生成é€再仅替换第二码为A，越界预检revision不变，原/严格重开éA/[E9,41]及共享空页/原流保持，nested_text_edit3项通过，git diff --check通过。当前仅WinAnsi范围专项，复合字体边界、多项范围/回滚像素仍待补，fuzz未运行。

- 2026-10-06 编辑计划现状校准：内容编辑文档同步两层图片替换/导入/实色像素、路径矩形像素及未知前缀拒绝、两层Tj/TJ/引号/非ASCII文字像素的已验收范围，移除过时未实现描述；分开嵌套内容编辑与多步事务章节。当前队列保留复杂字体/跨字符串范围/连续推进、纹理方向/命中及独立阅读器等缺口，阶段复选框仍不勾选。仅文档调整，git diff --check通过，本轮未重跑测试，fuzz未运行。

- 2026-10-06 公共路径数值拒绝回归：确认旧get_as_f32已有有限值检查，新增旧NaN/±Infinity明确PageOperatorError、新NaN/±Infinity明确ObjectError，以及旧非数字/少多操作数/非几何操作拒绝专项；与输出额度专项共2项通过，git diff --check通过。旧非有限值为内存构造Operator样本，不称为合法PDF数值词法；没有修改解析容错策略，fuzz未运行。

- 2026-10-06 公共路径参数输出边界：新增m/re/c含负数样本，序列化字节精确匹配，max_decoded_bytes恰好含操作名时通过、少一及零额度均ResourceLimit；覆盖两字节re操作名不能漏计。PDF专项1项通过，git diff --check通过。仅内部输出逻辑额度，不替代内容拼接/调用链/操作整体峰值预算，fuzz未运行。

- 2026-10-06 路径参数共用实现：提取内部path_parameter_replacement供顶层/嵌套入口共用m/l/c/v/y/re数量、旧数值读取、新有限值及数值词法、输出额度/可失败预留检查，删除两处重复序列化；顶层补齐最终操作名追加的额度/分配检查，与嵌套保持一致。nested_path_edit8及path_edit4共12项通过，git diff --check通过；公共API保持，嵌套专属来源/前缀验证仍在调用方，非聚合峰值预算完成，fuzz未运行。

- 2026-10-06 嵌套图片操作类型整理：私有NestedImageEdit枚举明确Remove/Transform/Replace，替代matrix/replacement两个Option，内部不再允许同时变换与替换的含混组合；共用来源/旧类型校验与事务提交，公共三入口及序列化契约保持。nested_image_edit2项通过，git diff --check通过；仅内部架构整理，不增加自动布局/完整解码或其他公共能力，fuzz未运行。

- 2026-10-06 嵌套路径未知前缀拒绝：replace_nested_path_numbers在重建几何成员前扫描叶前缀，未知操作返回UnsupportedFeature，已知固定操作数数量不符返回ContentParseError；不假设未知操作无路径状态效果。新增Vendor与带错误操作数q样本，使用真实解析位置构造来源，拒绝时revision/Resources/根流字节及ID保持。nested_path_edit现8项通过，git diff --check通过。仅名称/固定数量检查，非完整图形状态语义验证，fuzz未运行。

- 2026-10-06 引号非零文字状态像素：两层重复文字样本加入leading3的单引号、leading3及wordspace1/charspace2的双引号，初始文本矩阵Y8经引号推进到Y5；Unicode替换参考用显式Y5与Tw/Tc描述相同状态，删除/WWWW/é€原及严格重开整页RGBA继续一致，另一调用/共享页保持。render专项1项通过，git diff --check通过。字符串无空格，因此wordspace数值设置不等于空格推进验收；仍不证明连续事件删字推进补偿或全部文本布局，fuzz未运行。

- 2026-10-06 TJ嵌套文字像素：两层重复文字混合样本新增[-25 (MMMM) 25] TJ，删除清空字符串后第二调用仍可见；WWWW/é€通过replace_nested_text_array_item索引1替换，直接参考保留前后数字调整。原/严格重开整页RGBA、Unicode/字符码、共享原页及此前删除页保持对应结果。render29项全部通过，git diff --check通过。仅单字符串TJ，不证明多项跨字符串范围/连续文本推进/复杂字体或独立阅读器完备，fuzz未运行。

- 2026-10-06 引号嵌套文字像素：两层Form/同页重复调用/共享页混合像素样本逐种使用Tj、单引号、零字距双引号；初始MMMM真实可见，首调用删除及WWWW/é€Unicode替换均原/严格重开整页RGBA与直接裁剪参考一致，另一调用及共享页保持。render专项1项通过，git diff --check通过。引号样本为零leading/字距，不替代非零状态副作用或连续文本推进像素验收，TJ及复杂字体仍待继续，fuzz未运行。

- 2026-10-06 重复文字调用像素隔离：两层文字样本外Form重复调用同一叶Form，第二次平移20单位；直接参考含两处独立裁剪文字。首调用删除后目标仍保留第二MMMM的4字符/黑字形，首调用WWWW或é€替换后读回WWWWMMMM或é€MMMM；原/严格重开整页RGBA与参考精确一致，第二调用、共享8字符原页及此前删除页保持。render专项1项通过，git diff --check通过。非任意字符范围/TJ/引号/复杂字体或独立阅读器全覆盖，fuzz未运行。

- 2026-10-06 两层嵌套文字像素：Helvetica/BBox混合样本新增外层Form与独立Resources调用原文字Form，删除、WWWW和é€替换均明确核对来源深度2；原/严格保存重开继续与直接内容裁剪参考整页RGBA精确一致，真实黑字形、图片/页面路径及共享MMMM副本保持。render29项全部通过，git diff --check通过。两层各一次文字调用，不代表同页重复文字、多字体/CID、TJ/引号或独立阅读器全部验收，fuzz未运行。

- 2026-10-06 非ASCII嵌套文字像素：Helvetica混合样本在WWWW之后刷新来源再替换é€，直接参考明确写<E980>PDF字符码，目标与参考整页RGBA原/严格重开精确一致，黑字形存在、共享MMMM页及删除页像素保持；提取Unicodeé€、源码[E9,80]核对。render专项1项通过，git diff --check通过。单层WinAnsi/既有BBox内可见字形，不证明中文/CID、任意字体或全部字符无裁剪可见，fuzz未运行。

- 2026-10-06 嵌套Unicode文字替换像素：同Helvetica/BBox/图片混合样本将共享原页单次嵌套MMMM替换为WWWW，与直接内容相同文字+显式裁剪参考整页RGBA原/严格重开精确一致；确认黑字形存在且像素区别于MMMM，另一共享副本MMMM及此前删除页图形保持，文字提取WWWW。render专项1项通过，git diff --check通过。Unicode入口但字样为ASCII/单层，不推断非ASCII字形、两层重复文字或TJ/引号像素完备，fuzz未运行。

- 2026-10-06 嵌套文字删除像素：扩展Helvetica实际字形/Form BBox/图片及绿色页面路径样本，先确认原页确有黑色MMMM字形像素，再删除嵌套文字事件，与仅红图片+绿路径的直接参考整页RGBA精确一致、无黑字形；共享复制页原像素及四字符保持，目标零字符，原/严格保存重开一致。render专项1项通过，git diff --check通过。仅单层Tj Helvetica删除，不替代两层重复文字、Unicode替换/TJ/引号或独立阅读器像素验收，fuzz未运行。

- 2026-10-06 局部XObject绑定职责：bind_copied_form内部更名bind_local_xobject，统一复制祖先Form与替换图片的调用，明确只修改准备中的局部scope、类型由调用方校验、包含流在事务中提交；保留既有FmEdit命名以不改变序列化行为及冲突测试。嵌套图片2/路径7/文字3共12项集成通过，git diff --check通过；无公共API变化，完整计划仍未完成，fuzz未运行。

- 2026-10-06 跨文档图片导入后嵌套替换：两层仿射/图片专项在拒绝foreign句柄后，通过import_resource_from(Image)获得本地句柄并释放源文档；导入前旧root明确拒绝，刷新root后沿既有主动Err回滚、成功替换及严格保存重开验收，首图蓝色/所有变换矩阵/其余三调用与共享页保持。nested_image_edit2项通过，git diff --check通过。合法实色RGB导入样本，不替代复杂过滤器/软蒙版依赖或独立阅读器，fuzz未运行。

- 2026-10-06 嵌套图片替换像素：两层Form/BBox/重复叶调用专项新增首图红换青，颜色区别于邻接蓝路径，与直接矩形参考整页RGBA原/严格保存重开精确一致；中心像素覆盖新青区、蓝色覆盖/裁剪外白、第二次红调用及绿色页面路径，共享页原像素保持。render29项全通过，git diff --check通过。仅实色RGB替换，不是多色纹理、复杂过滤器/透明图或独立阅读器验收，fuzz未运行。

- 2026-10-06 嵌套替换句柄归属：两层图片替换专项构造另一个文档中对象ID精确相同且为合法蓝色Image的句柄，确保不是靠对象号码或类型差异拒绝；并补本地非流句柄拒绝。四类仿射样本均验证失败revision/Resources/根流字节及ID保持，再沿原成功/回滚/严格重开验收；nested_image_edit2项通过，git diff --check通过。foreign导入成功与替换像素/预算失败仍待补，fuzz未运行。

- 2026-10-06 嵌套图片绑定替换：新增replace_nested_image_draw，复用旧图片Do/叶scope校验并验证同文档句柄及新Image流，将新名字仅加入准备中的叶局部Resources，沿调用链复制及事务提交；不修改旧共享对象，不自动调整尺寸。两层四类仿射样本后再把首图红换蓝，原/严格重开仅首图RGBA改变、所有六参数矩阵/其他三图/共享页保持；误传Form句柄revision不变，外层主动Err恢复根字节/流ID/Resources，刷新后成功。nested_image_edit2项通过，git diff --check通过。独立替换像素、foreign句柄专项与预算失败待补，新图预检只验证流/Subtype、不宣称完整图片解码验证，fuzz未运行。

- 2026-10-06 嵌套编辑完整工作区回归：cargo test --workspace --locked --quiet最终退出0，PDF单元347、集成205、render29，共581项通过，0失败/0忽略；git diff --check通过。覆盖近期嵌套路径数值/来源成员、图片删除与仿射变换、Unicode/空Tj、调用链复制与回滚，以及公共内容提交内部更名；编辑文档同步两层图片矩阵/实色像素已验收范围。此为本库测试，不替代独立阅读器、复杂内容、可信命中或聚合峰值预算，P3-02/P3-04及完整项目仍未完成，fuzz未运行。

- 2026-10-06 公共内容提交职责整理：内部commit_page_text_edit更名commit_page_content_edit，更新顶层文字/图片/路径/Form及嵌套调用，明确共用快照验证、变化流COW及准备资源须处于同一外层事务的契约；无公共API变化。架构文档修正旧缓存段“仅成功提交推进修订号”，区分可见状态revision与commit_revision，回滚失效不能只用提交计数。PDF单元347、混合事务2及嵌套12共361项通过，git diff --check通过；未据局部回归宣称全工作区/全部架构调整完成，fuzz未运行。

- 2026-10-06 嵌套变换预检及联合回归：两层图片样本补变换入口对非Do、Form Do、缺对象ID、错页及offline叶快照的拒绝，与删除共同验证revision/Resources/根流ID未变。nested_image_edit2、nested_path_edit7、nested_text_edit3共12项集成全部通过，git diff --check通过；仅为这些嵌套专项联合回归，不宣称完整工作区/可视化编辑验收，fuzz未运行。

- 2026-10-06 两层图片旋转/镜像像素：扩展既有两层Form图片像素样本，加入90度旋转结合非均匀缩放和平移、水平镜像结合缩放和平移，逐种与手算直接矩形参考整页RGBA一致；中心像素区分旋转后旧白区重新着红，原/严格重开邻接蓝路径、第二调用、绿色页面路径及共享页保持。render29项全通过，git diff --check通过。纯红单像素图验证变换轮廓，不证明多色纹理方向/插值，亦非独立阅读器或任意仿射全覆盖；fuzz未运行。

- 2026-10-06 两层图片仿射矩阵组合：变换专项扩展平移、90度旋转、非均匀缩放及镜像四种，手算既有(4,5)缩放CTM、图片顶部归一化及页面Y翻转后的线性分量/位移；原/严格重开首图精确对应，其余三图/共享页矩阵及两层原流保持，原回滚/对象来源检查也执行。nested_image_edit2项通过，git diff --check通过。像素仍仅有缩放平移样本，未宣称全部仿射/任意父Matrix或独立阅读器验收，fuzz未运行。

- 2026-10-06 两层图片变换像素：两层Form/BBox/重复叶调用像素专项新增首图相对缩小一半并平移四分之一单位，直接矩形参考独立描述实际红图可见区，整页RGBA原/严格保存重开精确一致。中心像素验证原区白/缩放后红区、未变蓝色邻接路径、BBox之外白、第二调用及绿色页面路径；共享页原像素保持。render29项全通过，git diff --check通过。参考同用本库Skia，不是独立阅读器，不覆盖旋转/镜像/复杂图片与混合透明全部组合，fuzz未运行。

- 2026-10-06 两层嵌套图片变换：相对变换专项加入独立Resources/Child绑定的外层Form，来源链深度2；目标第一外调用的两图片来源均指向复制后的外/叶对象，但仅首图矩阵变化，第二外调用/共享页仍指向原外/叶对象，原两层流字节保持。既有NaN预检、外层主动Err回滚、旧snapshot拒绝与严格保存重开全部在两层样本通过，nested_image_edit2项通过，git diff --check通过；变换后渲染像素、全部仿射及额度失败仍待补，fuzz未运行。

- 2026-10-06 嵌套图片变换回滚：变换专项补外层事务主动Err，事务内首图显示坐标(+4,-10)生效且邻接图不变；回滚恢复根字节/流ID、Resources及全部图片六参数矩阵，旧快照拒绝，刷新后成功变换并沿既有严格重开/共享页验收。nested_image_edit2项通过，git diff --check通过。仅为单层主动错误回滚，不替代两层变换、限额失败、全部仿射或渲染像素验证，fuzz未运行。

- 2026-10-06 嵌套图片相对变换：新增transform_nested_image_draw，与删除共用叶scope图片Do校验，使用既有q/cm/Do/Q生成与深度检查，再逐层复制及事务提交。单层Form非均匀缩放/重复图片与重复Form调用样本验证相对平移(1,2)对应显示坐标(+4,-10)，其余三调用/共享页矩阵及RGBA保持，原Form字节与严格保存重开结果正确，NaN拒绝revision不变。nested_image_edit2项通过，git diff --check通过；修正测试显示Y向下的预期，不宣称嵌套全部仿射/像素或显示坐标反算完成，fuzz未运行。

- 2026-10-06 嵌套路径状态边界：新增先re/n丢弃、再m/q/颜色/l/Q/l/h/S样本，几何来源精确[2,5,7,8]；混入已丢弃re、遗漏q/Q前路径或遗漏h的列表均拒绝且revision不变，h本身不接受数值替换。合法修改q之前m成功，原/严格重开核对其余操作名及全部数值、重复调用和共享页保持，nested_path_edit现7项通过，git diff --check通过；未据此宣称未知操作/全部解释状态或可视化命中完备，fuzz未运行。

- 2026-10-06 嵌套路径几何成员校验：replace_nested_path_numbers在复制前从叶操作重新计算最后一次paint/n之后、目标paint之前的m/l/c/v/y/re/h索引，完整匹配来源列表；无额外索引数组分配，q/Q不作为路径清空边界。两层矩形专项新增把前一个已绘制路径几何混入第二事件的有序合法索引列表，编辑拒绝且revision不变；nested_path_edit6项通过，git diff --check通过。来源仍为文档内定位数据而非授权凭据，未知操作及完整解释状态/命中校验待继续，fuzz未运行。

- 2026-10-06 嵌套矩形参数修改像素：扩展两层Form路径像素专项，首个re平移并缩窄，挂起W裁剪随几何变化使邻接蓝路径右侧变白；与独立直接内容参考整页RGBA精确一致，中心像素覆盖旧区白/新区红、后续裁剪蓝/白、重复调用及绿色页面路径，原/严格保存重开共享页保持。render29项在初版修改样本通过，最终加强缩窄样本专项1项通过，git diff --check通过。修正删除分支旧裁剪范围内蓝像素的测试预期；非独立阅读器或六类几何的全像素验收，fuzz未运行。

- 2026-10-06 嵌套六类几何参数回归：新增单层Form同一路径含m/l/c/v/y/re及S，逐类修改首个调用的一个操作，原/严格保存重开核对全部操作名和邻接数值，第二调用/共享页原字节、注释及原Form字节保持；负宽矩形覆盖合法有符号数值。nested_path_edit现6项通过，git diff --check通过。六类型样本为单层，另有两层矩形专项；不替代修改后像素、所有仿射/裁剪组合及可信命中验收，fuzz未运行。

- 2026-10-06 嵌套路径参数修改：新增replace_nested_path_numbers，调用链验证后替换来源几何列表内m/l/c/v/y/re参数，数量/有限值校验，复用逐层复制、资源物化及事务提交。两层重复调用矩形含负宽样本覆盖NaN/错数/邻接操作拒绝且revision不变、外层错误回滚根流ID/字节/Resources、旧snapshot拒绝及刷新后成功；原/严格重开仅首个矩形参数变化、其余三路径/共享页/注释及原叶字节保持。nested_path_edit现5项通过，git diff --check通过；所有几何类型嵌套专项、像素及可信命中仍未完成，来源列表不作为授权凭据，fuzz未运行。

- 2026-10-06 嵌套图片删除像素：新增两层Form/重复叶调用、红色RGB图片超出叶BBox及邻接蓝路径样本，删除首个图片后与独立构造的直接矩形参考整页RGBA精确一致；七处中心像素覆盖清空白区、BBox裁剪、后续红/蓝调用及绿色页面邻接路径。目标与共享页原/严格保存重开像素保持对应结果。render单元29项全部通过，git diff --check通过；参考与被测均使用本库Skia，不是独立阅读器验收，也不覆盖复杂图片/透明混合/全部变换，fuzz未运行。

- 2026-10-06 空嵌套Tj端到端插入：新增合法间接Form空literal/hex Tj样本，空字符报告情况下用实际解析Do/Tj索引、偏移及对象ID构造来源，继承页面Helvetica插入é€。原/严格重开字符及码[E9,80]正确，第二次调用及共享页保持空、原Form字节保持。nested_text_edit现3项通过，git diff --check通过，内容文档说明手工解析来源边界；不是自动空事件定位/可视化命中或像素验收，fuzz未运行。

- 2026-10-06 空文本字体边界：显式scope字体专项补充空literal/hex Tj，无本地字体资源时仍能使用继承Helvetica编码é€为[E9,80]，无继承字体则返回None。专项1项通过，git diff --check通过；这仅验证字体选择/编码，不替代空事件来源定位及真实嵌套插入端到端验收。P3-04近期队列同步已实现的嵌套删除/文字替换与剩余像素、几何编辑、命中及独立阅读器工作；fuzz未新增或运行。

- 2026-10-06 嵌套非ASCII编码补验：继承Helvetica两层样本最终替换为é€，原/严格重开读回é€AB、共享ABAB；提取charcode精确0xE9/0x80且码长1，复制叶Tj原始解码字符串字节精确[E9,80]，排除把UTF-8直接写入字体码。nested_text_edit2、text_source9、text_remove5、混合事务2共18项集成通过，git diff --check通过。为WinAnsi，不推断中文/任意复合字体或真实字体/像素验收，fuzz未运行。

- 2026-10-06 嵌套Unicode继承字体回滚：两层同名scope/q恢复样本在XY编码及复制注册提交后外层Err，事务内XYAB可见，回滚恢复ABAB、根字节/流ID及Resources，旧root拒绝，刷新后Unicode替换成功并严格重开共享页/Helvetica验证。nested_text_edit2项通过，git diff --check通过。不据外层主动Err替代所有预算/异常失败或复杂字体/像素验收，fuzz未运行。

- 2026-10-06 嵌套Unicode替换接通：新增replace_nested_text及replace_nested_text_array_item，沿验证链每层父Do前继承实际Font，再结合叶scope/Tf/gs编码并共用复制事务。新两层样本页面Helvetica、外/叶同名Courier，外q临时选Courier后Q恢复继承Helvetica、叶无Tf；不可编码中文预检拒绝且revision不变，正常XY替换原/严格重开目标XYAB/共享ABAB且全为Helvetica，原leaf字节保持。此前TJ/引号清空恢复改用Unicode入口，nested_text_edit2项通过，git diff --check通过；文档补接口。尚未完成复杂CMap/中文字体、空事件来源及像素/独立阅读器、字符范围或峰值预算，fuzz未运行。

- 2026-10-06 调用者字体状态准备：新增font_before_operation_in_scope接受可选继承Font，前缀Tf/gs加载实际字体，q/Q保存恢复Font而非不同流的索引；支持Do前字体状态查询，Do位于BT内拒绝，原Unicode文字入口仍要求文字操作。重构后PDF单元347项通过，专项验证无Tf叶继承Helvetica、局部Courier在Q后恢复Helvetica、父Do前选中Helvetica，专项1项通过，git diff --check通过。前缀字体现在即时解析，先前被覆盖的无效Tf也拒绝；Font克隆/内部容量未统一峰值计费。尚未接通多层继承选择与Unicode提交，fuzz未运行。

- 2026-10-06 嵌套Unicode字体scope准备层：font_binding抽出font_for_text_operation_in_scope，Tf及gs/ExtGState Font查找改用调用方传入完整scope，现有顶层入口仍传页面资源；不混合缺失名称。重构后PDF单元346项通过，新增同名资源Helvetica页面/Courier显式scope选择、空scope不fallback、无字体前缀拒绝专项1项通过，git diff --check通过。当前不推断调用者继承字体，无公开嵌套Unicode入口；下一步须跟踪父Do处状态并进入叶流，不以显式Tf准备层宣称完整支持，fuzz未运行。

- 2026-10-06 嵌套编辑输出字节精确成功边界：同一实际root7/leaf505样本扩展512、513、514限额矩阵，前两者提交末端超限整体回滚，514精确成功；独立解析复制叶确认paint改n，实际新root＋复制叶字节正好514、页面流ID为副本，原leaf均不变。nested_path_edit4项通过，git diff --check通过。仅编辑逻辑字节额度，不推断低额度渲染/保存/峰值内存能力，fuzz未运行。

- 2026-10-06 最终root聚合字节真实失败：新增nested_final_root_byte_limit_rolls_back_created_form_and_resource，512额度下原root7＋注释填充leaf505精确达限，来源取实际解析位置（不依赖低额度渲染）。叶复制/资源注册后最终root加入聚合因新增词法分隔字节报Nested edited decoded bytes；revision推进但Resources、root字节/流ID和原leaf恢复，刷新root只读叶验证仍通过。初次样本填充长度手算多1，断言捕获后修正为491注释字符，非放宽额度。nested_path_edit4项通过，git diff --check通过。未声称额度512支持整页渲染/保存或操作级峰值预算，fuzz未运行。

- 2026-10-06 嵌套编辑输出字节聚合修正：检查现有删除/字符码替换不增加操作数，但原提交只累加复制Form，遗漏最终替换页面root的逻辑字节；改为最终root加入同一max_decoded_bytes总量，超限返回错误并整体回滚副本/资源。新增计费函数精确总量、root多一字节拒绝、整数溢出及零额度边界单元1项通过；嵌套图片1/路径3/文字1共5项集成通过，git diff --check通过。计费发生在root重解析后，仍非预分配或操作级峰值保证，真实root超限回滚专项待补，fuzz未运行。

- 2026-10-06 嵌套复制名称冲突：两层路径隔离样本改为原XObject绑定FmEdit1，复制父流精确验收选中Do改FmEdit2、另一Do仍FmEdit1，原流两次FmEdit1及共享页保持；原/严格重开与回滚均通过。初测字节断言遗漏既有替换工具插入的词法分隔换行，核对实际输出后将预期写为Do换行＋原空格，未改实现或忽略其他字节。nested_path_edit3项通过，git diff --check通过；不是全部资源冲突/峰值验收，fuzz未运行。

- 2026-10-06 嵌套字符码替换回滚：nested_text_edit三分支在两层副本/资源注册成功后注入外层Err，暂存内容DABC可见，回滚恢复CABC、根流ID/字节与Resources；旧root拒绝，刷新后替换成功并继续TJ/引号恢复、严格保存重开及共享页验收。1项集成测试通过，git diff --check通过。为外层错误注入，不替代所有真实额度失败/异常、布局及像素验收，fuzz未运行。

- 2026-10-06 嵌套原始文字码替换：新增replace_nested_text_codes及replace_nested_text_array_item_codes，叶程序词法替换后复用链复制事务。nested_text_edit三分支先将独立Tj邻接C改D，再把已清空TJ项/引号字符串改Z，原及严格重开目标ZDABC/共享ABCABC、原leaf字节保持；TJ数值项替换拒绝且revision不变。1项集成测试通过，git diff --check通过，文档明确输入原始字符码、不自动字体校验/Unicode编码/布局补偿。Unicode/字符边界接口、替换回滚专项及像素仍待实施，fuzz未运行。

- 2026-10-06 两层嵌套图片复制隔离：nested_image_edit增加显式外Form资源scope并重复外调用，实际来源链均2层；删除首图片后首剩余图片的两个祖先ID均为副本，另一外调用及共享页的来源ID仍为原outer/leaf，原两Form载荷保持。既有类型/身份预检拒绝、资源/流回滚、图RGBA与邻接路径及严格重开保持。nested_image_edit1、nested_path_edit3、nested_text_edit1共5项通过，git diff --check通过；像素/复杂格式scope与峰值仍待验收，fuzz未运行。

- 2026-10-06 嵌套复制真实对象额度失败：新增nested_copy_object_limit_failure_restores_store_capacity_and_page_bindings，max_objects=100新建连续ID文档填充至99，仅留1槽；嵌套删除创建叶副本后父复制精确报document object count。回滚后revision推进但Resources、根字节/流ID恢复，严格保存重开仍1绘制；再次插入获ID100，证明副本未泄漏容量/ID，下一次插入明确超限。nested_path_edit3项通过，git diff --check通过。不是仅外层主动Err注入，也不据对象额度替代操作级内存峰值保证；其他预算/复杂scope与完整P3-04仍待验收，fuzz未运行。

- 2026-10-06 嵌套图片删除类型/身份拒绝：nested_image_edit加入合法空Form Do作为叶邻接操作，伪造准确位置的Form Do来源由Image subtype预检报ObjectError，非Do同样拒绝；无Form ID、错页root及offline叶program均拒绝，revision/Resources/根流ID保持。之后正常单次图片移除/回滚及严格重开仍通过，空Form邻接保留。1项集成测试通过，git diff --check通过；多层图片/像素及额度注入仍待验收，fuzz未运行。

- 2026-10-06 嵌套图片删除入口：新增remove_nested_image_draw，校验叶Do及有效资源scope中Image subtype，移除一次操作后复用调用链复制/事务提交，不解码图片或删除共享资源。nested_image_edit公共集成含一Form两图片Do、两次外调用与邻接路径，目标4图变3图/路径保持，共享页4图；资源注册后外层错误回滚恢复资源/原流ID/4图。原及严格重开图RGBA红样本、原Form字节/共享页内容保持，1项通过，git diff --check通过。文档补入口；多层/拒绝矩阵/像素与复杂scope格式仍待补验，fuzz未运行。

- 2026-10-06 嵌套文字状态补验：nested_text_edit在原/严格重开复制叶程序断言TL仍为14，双引号两个数值仍2/1；实际解释后邻接独立C的TextObject状态保留word_space=2/char_space=1，TJ/单引号分支仍0/0。连同此前字符串/操作/共享与原流验收，1项集成测试通过，git diff --check通过。独立Tm不检验连续文本推进；引号换行位置/像素及混合字体仍待验收，fuzz未运行。

- 2026-10-06 两层嵌套TJ/引号清空：新增nested_text_edit，分别TJ及两类引号在两层Form/两次外调用下清空首次事件，原/严格重开验收目标CABC、共享ABCABC，两个被选祖先均复制且原leaf字节保持。复制叶操作名称不变，TJ25/-10数值、空字符串及注释保持，引号字符串清空；邻接C用独立Tm，不推断连续文本推进保持。连同nested_path_edit2项共3项通过，git diff --check通过。像素/混合字体、详细引号状态及整体峰值仍待验收，fuzz未运行。

- 2026-10-06 嵌套文字清空入口：抽取commit_nested_edit共用逐层复制/资源重绑/事务提交，新增remove_nested_text_draw，叶编辑复用with_cleared_text_draw保留操作/TJ数值/引号状态。扩展缺省Resources测试，已删首次路径的Form再清空首次Tj，外层错误回滚后仍4字符，正常提交原/严格重开目标AB、共享ABAB与原Form字节保持。nested_path_edit2项通过，git diff --check通过，接口文档记录单层Tj验收及未完成TJ/引号/多层/像素边界。未补偿布局或安全擦除，fuzz未运行。

- 2026-10-06 嵌套路径删除像素闭环：新增nested_path_removal_pixels_preserve_pending_clip_and_other_invocations，两层Form中重复平移调用叶流，目标红路径含W挂起裁剪，后续蓝路径越界。仅首次红paint改n，独立直接内容参照逐RGBA验证目标红墨消失、蓝仅裁剪内显示、第二次红蓝和邻接绿色保持；共享页原整图、严格保存重开目标/共享整图一致。render库28项全部通过，git diff --check通过。仅合成矩形/非零平移与W，其他曲线/奇偶裁剪、混合内容及独立阅读器仍待验收，fuzz未运行。

- 2026-10-06 嵌套编辑缺省资源scope：新增nested_edit_materializes_page_fallback_without_losing_font_bindings，省略Resources的Form用页面Helvetica且含路径/文字，重复调用只删首次路径。原及严格保存重开验收复制Form已有显式Resources，ABAB及Helvetica保持、目标3事件、共享页4事件，原Form仍省略Resources且原字节不变。nested_path_edit共2项通过，git diff --check通过。仅旧式页面fallback单层样本，不宣称PDF2.0允许省略Resources；匿名、多层不同scope、额度及像素仍待补验，fuzz未运行。

- 2026-10-06 嵌套路径删除预检拒绝：实际编辑接口新增内层调用字节缺失、伪造外Form ID、错页root及offline叶program拒绝；另构造字节位置准确但指向re而不是绘制操作的来源，明确由叶编辑类型检查报ObjectError。所有失败后revision、Resources、根字节和原流ID保持，之后正常回滚/复制隔离/严格重开回归仍通过。nested_path_edit1项通过，git diff --check通过；尚未替代额度注入、匿名/缺省scope、混合资源及像素验收，fuzz未运行。

- 2026-10-06 嵌套路径删除初步入口：新增remove_nested_path_draw，验证当前root/调用链/叶绘制操作，改为n后事务自内向外复制Form及有效Resources，新增局部名称仅重绑选定Do，最后注册复制外Form并提交页面变化流；原Matrix/BBox/资源及几何保留，复制解码字节聚合限额、资源项/名称限额与可失败分配。nested_path_edit两层×两次调用专项：目标4事件变3，未选内调用仍绑定原leaf、未选外调用绑定原outer，共享页4事件及原两流字节保持；外层错误回滚恢复Resources/流ID/4事件，旧root拒绝，严格重开验证。连同form_replacement3项共4项通过，git diff --check通过；文档补专用入口。尚未完成匿名/缺省scope、复杂资源/峰值、像素/独立阅读器及文字图片嵌套编辑，P3-04保持未勾选，fuzz未运行。

- 2026-10-06 嵌套复制编辑准备层：只读定位内部提取prepare_form_chain，沿同一验证流程保留每层原字典、有效资源scope（含缺省页面fallback）及离线ContentProgram，供后续自内向外复制修改，公开nested_content_program仍只返回叶程序。可失败预留链容器，累计解码/操作预算保持；不保留原编码流载荷，但字典clone/capacity及整体峰值预算尚未统一。重构后PDF单元345项通过，实际九层原/严格重读专项新增逐层Form头、前8层XObjectscope/末层空scope及1/2操作数断言，专项1项通过；git diff --check通过。此层只读，无嵌套修改入口，下一步事务创建叶副本并逐级重绑单次Do，fuzz未运行。

- 2026-10-06 编辑文档/队列同步：对照现有公开签名更新docs/content-editing.md，列出Form删除/相对变换/替换及导入顺序、快照失效、q/Q深度、资源事务边界和分层来源只读校验；纠正“Form递归来源尚未实现”与字符码范围的过时描述。当前队列反映已验收顶层调用编辑，将嵌套调用链复制后修改/共享隔离/失败回滚明确列为下一步，完整P3-02/P3-04仍未勾选。git diff --check通过；本轮仅文档，无新增测试或fuzz运行。

- 2026-10-06 导入Form私有字体/图片依赖：新增imported_form_keeps_private_font_and_image_scope_after_source_release，源Form显式Resources引用Helvetica及红1×1 RGB，完整图导入后源释放；目标页原有Courier和绿RGB不能替代Form私有scope。重复Form调用及复制页在原/严格保存重开验证ABAB、每字符BaseFont=Helvetica、两红RGBA样本、嵌套来源ID与叶Do定位准确且顶层来源为空。form_replacement共3项通过，git diff --check通过。仅标准字体/简单RGB，不替代嵌入中文字体/所有图像格式、独立阅读器/像素或嵌套叶隔离编辑验收，fuzz未运行。

- 2026-10-06 跨文档嵌套Form导入替换闭环：新增imported_nested_form_replaces_one_call_after_source_is_released，源两层间接Form的自身Matrix及显式Child资源图先import_resource_from，再释放整个源文档；目标第二次Do替换为导入handle，原及严格保存重开解释确认两层实际来源链、顶层目标ID、累计9/11平移及叶流精确字节。第一原Form调用保持，复制共享页内容/两事件保持。form_replacement共2项通过，git diff --check通过，导入接口注释补Form。仅合成路径资源图，不替代字体/图片混合依赖、全部共享/循环关系、像素/独立阅读器或嵌套叶内容复制修改验收；fuzz未运行。

- 2026-10-06 Form变换q/Q深度精确边界：新增实际文档测试，max_depth=8下原7层q的Form调用可增加孤立包装并解释/严格重开；原8层q虽然合法绘制，编辑前拒绝额外q，ResourceLimit且revision、源内容字节/流ID、Resources保持。两分支复制共享页原字节及重开解释均保持。form_remove现3项全部通过，git diff --check通过。仅本次新增包装的图形状态头寸，不替代全调用树峰值/全部工作预算或嵌套共享编辑验收，fuzz未运行。

- 2026-10-06 Form替换像素专项：新增replaced_form_pixels_change_only_selected_call_and_survive_reopen，两个不同平移的共享红8×8 Form调用，仅第二个换为蓝4×4；以独立直接红/蓝/绿路径页面逐RGBA验收，实际红/蓝/绿及被缩小区域白像素确认，首调用和邻接绘制保持。共享复制页原整图及严格保存重开目标/共享整图一致。render库27项全部通过，git diff --check通过；仅合成实色路径＋无旋转页面，不替代混合内容/不同Form Matrix、独立阅读器或嵌套叶修改验收，fuzz未运行。

- 2026-10-06 Form替换拒绝边界：扩展form_replacement，实际接口拒绝同字节错页snapshot、只读offline叶program、q/Q/越界操作位置、非流替换handle和缺BBox的Form handle；全部预检失败后revision、页面Resources、内容字节及两个流ID保持。连同跨文档handle/过期snapshot、资源注册后回滚及正常严格保存重开专项，1项集成测试通过，git diff --check通过。缺BBox对象仅用于未绑定资源的拒绝测试，不宣称完整文件规范验证；共享嵌套修改与像素验收仍待推进，fuzz未运行。

- 2026-10-06 Form单次调用替换：新增replace_page_form_draw，当前页面snapshot/旧目标Form/同文档替换handle预检，外层事务包含页面局部Form资源注册及变化流复制提交。新form_replacement公共集成覆盖跨两流Do操作替换、重复调用仅第二次变更、复制共享页不变、同号外文档handle拒绝且revision不变、旧snapshot拒绝；注册完成后错误回滚恢复页面Resources与原流ID。原及严格保存重开验证实际绘制来源ID为old/new及共享页old/old、两Form原载荷未修改，1项通过，git diff --check通过。README补入口；未替代替换Form尺寸/Matrix自动匹配、像素/独立阅读器或嵌套叶内容复制修改验收，fuzz未运行。

- 2026-10-06 Form调用编辑像素闭环：新增edited_form_affine_pixels_match_direct_content_and_preserve_shared_page，60×60合成页面上覆盖旋转/非均匀缩放/反射及Form自身Matrix，超BBox红路径＋蓝路径与邻接绿路径；编辑后与显式cm/BBox W n/路径参照逐RGBA比较，实际三色存在且不同原图。再删除目标Do，与仅邻接绿路径参照整图一致、无红蓝；两阶段共享复制页原整图及严格保存重开整图保持。render库26项全部通过，git diff --check通过。为本库Skia路径样本，不替代独立阅读器、其他内容类型/任意复杂状态、嵌套叶流隔离修改或完整P3-04验收；fuzz未运行。

- 2026-10-06 Form仿射组合回归：新增form_affine_edits_compose_form_matrix_and_parent_ctm_without_changing_second_call，Form自身平移Matrix、父CTM非均匀缩放＋平移下，分别编辑第一次调用的90度旋转/非均匀缩放/反射，用手算六系数验证显示矩阵（含左上坐标转换），第二调用精确原矩阵保持。原文档及严格保存重开都验收，两页共享源字节、原Form载荷及ID不变。form_remove现2项全部通过，git diff --check通过，README更新已验证范围。不是像素/所有仿射/独立阅读器或嵌套叶流复制修改验收，完整P3-04未完成；fuzz未运行。

- 2026-10-06 Form调用相对变换入口：新增transform_page_form_draw，验证当前页面snapshot与Form头，复用q/cm/Do/Q孤立调用提交；ContentProgram抽出with_transformed_xobject_operation，保留旧图片接口作为转发，不改共享Form Matrix/流。Form集成补验NaN与图片Do拒绝且revision不变、平移3/4后的显示矩阵、邻接图片及路径矩阵不变、共享页字节及严格保存重开保持。form_remove1项及既有image_transform4项共5项通过，git diff --check通过；README记录当前用户空间及非嵌套编辑边界。未以平移专项推断所有仿射/像素/独立阅读器或完整P3-04验收，fuzz未运行。

- 2026-10-06 顶层Form单次调用删除：新增remove_page_form_draw，验证当前页面snapshot、命名Do及PdfForm头，复用事务/变化流复制提交，只移除一次调用而保留共享Form。form_remove真实间接Form集成覆盖重复调用/复制共享页/邻接图片路径、拒绝offline叶snapshot/图片Do/非Do/过期snapshot、外层错误回滚恢复流ID及4事件，成功提交后严格保存重读剩余3事件与原Form字节/ID保持。专项1项通过，既有混合事务2项亦通过；README补入口与边界，git diff --check通过。未修改嵌套叶内容、不宣称共享Form复制后修改或独立阅读器/像素验收，P3-04仍待推进，fuzz未运行。

- 2026-10-06 嵌套来源额度与全工作区回归：只读nested_content_program入口在资源查找/叶流解析前检查调用深度、几何来源索引数量及调用链＋索引逻辑字节，避免先遍历超额来源；新增精确字节/操作/深度边界、少一字节/操作拒绝、空链拒绝及独立复制一致性单元测试。cargo test --workspace --locked --quiet正常退出0，PDF单元345、集成187、render25，共557项通过，0失败/0忽略；git diff --check通过。覆盖近期Form执行/裁剪/来源及混合编辑事务回归，不替代独立阅读器、嵌套编辑隔离或操作级峰值内存预算；未据此勾选完整阶段，fuzz未运行。

- 2026-10-06 混合编辑事务成功/失败闭环：content_edit_transaction新增成功提交专项，同一外层事务依次替换文字范围、删除图片绘制、停止路径绘制；共享复制页原内容字节与流ID保持、目标页复制后修改，旧snapshot拒绝。提交前后及严格保存重开验收目标AXYC/仅文字、共享页ABC/图片/路径保持，逻辑删除未删除图片资源。连同既有错误/异常注入回滚专项，共2项集成测试通过。仅覆盖顶层Helvetica/RGB合成样本，不替代嵌套Form复制后修改及独立阅读器验收，P3-04保持未勾选；fuzz未运行。

- 2026-10-06 嵌套路径几何来源：NestedDrawSource新增叶流路径构建操作索引，解释器为Form路径保留m/l/c/v/y/re/h来源，复制及显示列表保留量计入索引字节。只读叶程序验证拒绝非递增、绘制位置及非路径操作索引；实际九层原/严格重读及重复Form用例验证准确索引，并覆盖伪造索引拒绝。PDF库344项单元测试通过，git diff --check通过。该校验仅验证结构定位，不证明完整路径语义、不授权共享Form编辑；编辑事务/共享隔离与操作级峰值预算仍待实施，fuzz未实现或运行。

- 2026-10-06 无ID内存Form定位分支：新增合成内存对象测试，间接外Form的显式Resources中直接嵌入内Form流，实际解释生成2层调用链（外Some(id)/内None），只读定位经父scope得到准确叶字节；伪造内Form ID拒绝。PDF库344项单元测试通过，git diff --check通过。仅验证库内直接流对象/None-ID分支健壮性，不把该内存结构当符合PDF的直接stream文件样本，未保存/宣称独立阅读器支持；标准PDF流应使用间接对象。共享隔离编辑与存储额度专项仍待实施，fuzz未运行。

- 2026-10-06 嵌套来源只读验证拒绝矩阵：扩展实际九层原/严格重读测试，新增叶字节位置缺失、中间第5层调用索引越界、用offline叶程序充当页面root及不同文档身份root的拒绝验证；均由嵌套定位接口实际执行，不仅matches_program布尔比较。PDF库343项单元测试通过，git diff --check通过。保留此前FormID/调用字节/叶索引与旧版本拒绝用例；合法源链验证不等于嵌套修改授权，共享隔离编辑及匿名Form/额度专项仍待实施，fuzz未运行。

- 2026-10-06 嵌套ContentProgram只读验证：新增 `PdfPage::nested_content_program(root, source)`，要求root匹配当前文档revision/page，沿每层完整资源scope验证Do操作及字节位置、实际Form ID，检查深度/循环，累计解码工作和操作数，末端核对叶索引/字节位置。返回无stamp的offline叶program，不能视为共享Form编辑权限或将叶索引应用顶层。实际九层原/严格重读测试验收叶流原字节与f索引，并拒绝伪造FormID/调用位置/越界叶索引；嵌套图片验证叶Do，Form仅修改后旧root在新只读接口拒绝，即使两个历史snapshot仍能互相matches。PDF库343项单元测试通过，git diff --check通过。资源clone/解析分配在逻辑聚合计费前，仍非容量/操作级峰值保证；尚无嵌套编辑事务/共享隔离、匿名Form与存储额度完整专项，fuzz未运行。

- 2026-10-06 Form来源snapshot失效补验：扩展真实嵌套报告测试，文字报告只匹配自身文档/页面revision的顶层ContentProgram，不匹配保存重读的新文档身份；图片报告初始匹配，仅修改Form流（页面Contents未改）后旧报告不匹配新program，旧报告＋旧program仍合法配对为历史snapshot。复制同内容页后当前报告匹配原页面但拒绝另一页面，验证page身份独立于字节相同。PDF库343项单元测试通过，git diff --check通过；接口注释明确matches_program只是snapshot匹配，不证明当前可编辑或嵌套叶索引属于顶层。嵌套定位验证/编辑事务入口仍待做，fuzz未运行。

- 2026-10-06 提取结果Form分层来源：ExtractedCharacter增加独立nested_source，调用链使用可失败复制、按每字符累计逻辑字节限额；ExtractedTextRun提供借用首字符来源getter，不额外复制链。图片paint-order报告原样保留ImageObject新来源，采样字节额度合计调用链逻辑字节并检查溢出。扩展真实重复Form的原文档/严格重读：8个Tj/TJ提取字符验证调用0/1和叶2/3；两张提取图片验证调用0/1与顶层source为空。PDF库343项单元测试通过，git diff --check通过。尚未做提取链存储额度专项、嵌套ContentProgram校验/编辑与文档revision失效验证，不宣称可直接使用叶索引编辑页面顶层，fuzz未运行。

- 2026-10-06 文字/图片Form分层来源挂接：TextObject/ImageObject新增nested_source，子解释器的Tj/TJ与Image Do填写相同调用链/叶操作，顶层source仍保留原行为；Trace将调用链字节与Unicode/图片采样载荷统一累计保留计费。扩展真实Form测试，重复调用的Tj/TJ区分顶层调用0/1及叶操作2/3；嵌套图片原文档/严格保存重读验证相同FormID、调用0/1及叶Do索引1、字节位置，顶层source均None。PDF库343项单元测试通过，git diff --check通过。尚未将新来源挂接ExtractedCharacter/Image输出或嵌套ContentProgram编辑校验，文字保存来源专项与全工作区回归待补；fuzz未运行。

- 2026-10-06 路径Form分层来源：新增公开 FormInvocation/NestedDrawSource，外到内保存调用Form的文档局部ID、调用方Do索引/字节位置及叶流绘制索引/字节位置；子解释器维护调用路径，PathObject新增nested_source而不复用顶层source。调用链复制可失败/长度字节限制，Trace保留payload累计计费。扩展实际九层原/严格重读测试验证9调用链、FormID与调用位置、叶f索引1；重复同Form验证相同ID但顶层调用索引0/1不同，叶f索引2相同。PDF库343项单元测试通过，git diff --check通过。此来源仍按当前文档/页面revision有效，不是持久编辑handle；尚无路径几何操作列表、文字/图片分层来源、嵌套ContentProgram定位/编辑验证和额外存储额度专项，完整P3-02保持未完成，fuzz未运行。

- 2026-10-06 文字/图片裁剪实际修复：审查Form混合渲染发现Skia仅路径set_clip，文字/图片忽略图形状态clip。set_clip改接通用ClipPath，文字和图片在save后应用clip再设置绘制矩阵，clip错误恢复canvas。新增Form图片40×40越出10×10 BBox与大字号MMMM越界样本，全RGBA对照直接矩形＋显式文字clip；逐像素验证两BBox之外白色、红/黑实际ink，以及调用后绿色父路径证明clip恢复，严格保存重读一致。专项通过，render全部25项通过，git diff --check通过。此次覆盖单层默认矩阵的文字/图片，不代表全部嵌套资源重名/变换像素或文字clip渲染模式支持；分层来源与独立阅读器仍待验收，fuzz未运行。

- 2026-10-06 真实九层Form调用验收：新增九个实际Form流，通过各自显式XObject/Child间接资源连接，逐层[1,2]平移，末端绘制路径；原文档和严格保存重读均只有1个末端事件、9个继承BBox裁剪operand及独立[9,18]累计用户坐标预期，嵌套事件不提供顶层source。相同文件max_depth8精确返回Form invocation depth，max_depth9完整执行，非预设内部深度。PDF库343项单元测试通过，git diff --check通过。尚不覆盖九层像素/文字图片/分层来源或原生深度硬上限全样本，完整P3-02不勾选，fuzz未运行。

- 2026-10-06 Form 全树路径/文字工作额度：ExecutionBudget增加共享path_segments/text_chars与剩余字符查询，解释器构建路径和Tj/TJ解码实际挂接；字符解码前以共享剩余额度约束，避免先完整解码再拒绝。基础测试覆盖兄弟累加/失败不计费；真实Form重复调用验证共享路径额度，普通Tj＋数组TJ共8字符正常绘制、总6字符限额拒绝。路径专项用独立Trace保留额度，以精确隔离“构建工作超限”与既有“display保留段数超限”；初测先被保留clip载荷额度拒绝，区分两种预算后验收通过。PDF库342项单元测试通过，git diff --check通过。逻辑工作累计不等于操作级峰值；BBox/继承clip由独立clip/保留额度计费，真实多层深度与来源路径仍待验收，fuzz未运行。

- 2026-10-06 Form 执行预算专项补验：扩展现有执行测试，重复同一Form正常返回两次绘制（不误判循环）；先预占共享解码工作额度，再实际执行两次Form，精确断言第二次报 content invocation decoded work，避免仅验证文档解析限额。预设内部form_depth=128后调用实际Do分支，精确验收原生递归硬上限错误（不把预设深度称为真实129层样本）。PDF库342项单元测试通过，git diff --check通过。配置深度的真实多层、路径/文本全树聚合预算与操作级峰值仍待补，fuzz未运行。

- 2026-10-06 Form BBox/继承裁剪像素验收：新增render回归，含剪切/平移Form、越出BBox的大红路径、内部蓝路径、父绿色路径与继承clip；将Form调用与独立直接内容（q/Matrix cm/BBox W n/相同内容/Q）全RGBA逐像素比较。在非零CropBox、UserUnit1.5及0/90/180/270旋转下原文档与严格保存重读均一致，三色实际像素存在防止空通过。render专项1项及全部24项通过，git diff --check通过。仅验证非Group路径Form裁剪/状态及此矩阵，尚无嵌套文本/图片/资源重名像素、分层来源或独立外部阅读器证据，不勾选完整P3-02；fuzz未运行。

- 2026-10-06 Form 内容初步执行：分离 run_content 与页面start/end，Do/Form创建独立子解释器，应用 invocation_state、完整自身资源（缺省兼容仅页面）、同ParseOptions、共享操作/累计解码工作预算与分支深度；间接Form活动路径检测循环，原生递归另硬限128。父路径/颜色/状态无需手工恢复，子q/BT必须自闭合；Group不支持，子兼容诊断暂拒绝而非丢弃。嵌套文字/路径/图片不填写顶层来源索引，分层来源尚待实现。新增真实文档及严格保存重读回归验证子绘制/矩阵/BBox clip、父未完成路径/颜色/裁剪不变、重复调用共享总操作限制、自循环、子未配对q拒绝；旧Form subtype测试改为合法空Form成功，PS仍不支持。PDF库342项单元测试通过。解码工作在decode后累计，文本/路径局部限额、容器克隆/全流程峰值仍未统一；未跑render像素/完整工作区，不将初步执行当完整P3-02完成，fuzz未运行。

- 2026-10-06 调用树共享工作预算前置：新增 ExecutionBudget，同调用树共享操作数/累计解码工作，深度按分支记录，失败计费不提交；页面解释器操作计数改用共享budget，本地操作索引仍独立保留来源语义。普通测试验证父/子/兄弟共享计数、兄弟深度不累加、重复解码计费及溢出/限额失败原子性；PDF库341项单元测试通过。Form尚未创建子解释器，child/decoded计费仅有基础测试、尚未挂接Do/解码；不是操作级峰值内存预算，不替代循环检测/所有工作额度，fuzz未运行。

- 2026-10-05 解释器资源作用域接入：新增 `Interpreter::with_resource_scope` 和统一 named_resource，字体Tf、ColorSpace、ExtGState与XObject/Do查找均使用显式完整scope或默认页面资源；显式scope缺类别/条目不回退，类别经上下文解析、必须字典，null条目拒绝。新增测试覆盖四类别、缺项/null，真实注册页面Helvetica后验证默认能找到而显式空scope拒绝（避免空通过）；PDF库340项单元测试通过，git diff --check通过。尚未从Form/Do创建子解释器，Form仍不支持执行；子来源路径、全调用树操作预算/深度与循环、图形状态进入恢复和渲染验收继续待做，fuzz未运行。

- 2026-10-05 Form 独立调用状态：新增 `PdfForm::invocation_state`，预计算checked CTM与BBox裁剪元素，再克隆父状态、追加继承裁剪的交集operand并设置子CTM；父状态成功/失败均不修改，Group显式拒绝而非忽略透明语义。新增测试验证2倍父CTM下独立平移[16,28]、文字rise继承与修改隔离、嵌套clip累积、限额失败不改变原clip和Group拒绝；PDF库339项单元测试通过，git diff --check通过。仍未从Do执行内容或恢复解释器解析状态/路径/来源；父状态clone为既有不可失败克隆，容量/操作级峰值未完整计费，渲染交集未在本项验收，fuzz未运行。

- 2026-10-05 Form BBox 裁剪元素前置：新增 `bbox_clip_element`，将实际显示四边形构建为闭合 Winding ClipElement，矩阵identity避免重复变换；五段与逻辑存储前置限额、可失败分配路径。新增 `PdfPath::try_add_subpath` 与 `ClipPath::try_add_element`，累计裁剪元素/子路径/段数量及逻辑字节检查后追加，失败保持旧裁剪不变。扩展Form几何测试验证闭合元素、拷贝裁剪后追加保留继承operand且不修改原集合，以及数量/段/字节失败原子性；PDF库338项单元测试通过。只是裁剪元素构建及追加接口，不执行几何相交，解释器尚未使用该接口；图形状态克隆峰值/容量未计费，Form内容/透明Group与来源仍未实现，fuzz未运行。

- 2026-10-05 Form 调用几何前置：新增 `invocation_matrix` 与 `bbox_display_corners`，Form Matrix→调用CTM→page matrix使用checked组合，保留BBox变换四边形而非扩大为轴向包围框，不修改调用方矩阵；固定四点计算不分配容器，非有限/坐标溢出拒绝。独立标量预期测试剪切＋旋转＋页面翻转的四点、退化矩阵及NaN/溢出；PDF库338项单元测试通过，git diff --check通过。当前仅提供裁剪几何，尚未建立ClipElement/与继承clip相交、进入隔离图形状态或执行内容；完整Form仍未实现，下一步将四边形接入解释器状态并补渲染验收，fuzz未运行。

- 2026-10-05 Form 跨文档资源导入验收：扩展间接 Form 测试，外文档 handle 直接注册拒绝；通过 `import_resource_from(..., ResourceKind::Form)` 重映射并注册到目标。释放源文档后，目标及其严格保存重读均验证 Form 原内容 `q Q`、间接 Matrix/坐标平移和自身资源 Marker，不依赖源上下文。PDF库337项单元测试通过（扩展既有测试），git diff --check通过。本项仅验证合法资源图导入/归属/持久化，不代表 Form 执行、BBox裁剪、状态隔离或循环内容验收，P3-02仍未勾选，fuzz未运行。

- 2026-10-05 Form 间接资源/注册/严格保存前置：资源管理器新增 `ResourceKind::Form`，事务内校验 PdfForm 后注册 XObject/Fm，导入入口增加源 Form 校验分支（跨文档验收仍待做）。新增合法文档回归：BBox/Matrix/矩阵坐标/Resources/类别/条目均通过间接引用，原文档及严格保存重读验证平移、页面命名资源可达、自身 Shared 条目隔离和 ParentOnly 不回退；错误字典作为 Form 注册拒绝。PDF库337项单元测试通过，git diff --check通过。初编译发现 ResourceKind 无 XObject 枚举及新增 Form 的导入匹配缺失，按仓库现有接口补齐。仍未执行 Form 内容、BBox clip、状态恢复、循环/递归与聚合预算，不勾选完整P3-02，不运行fuzz。

- 2026-10-05 Form 资源作用域前置：新增 `PdfForm::resource_scope/named_resource`，显式 Resources 使用整个自身字典，缺类别/缺名称不混合父资源；Resources 缺省时仅使用调用方显式提供的 legacy page scope，不自动搜索外层 Form。类别和条目经上下文解析，类别非字典/条目null明确拒绝。扩展既有Form测试覆盖同名资源隔离、缺省回退、显式空阻断、缺名/类别/null/错误类别类型，PDF库336项单元测试通过。规则参照[PDF规范勘误7.8.3](https://pdf-issues.pdfa.org/32000-2-2020/clause07.html)：现代Form应显式资源，旧文件缺省兼容取页面资源；当前helper未做版本门控，尚未接入嵌套解释，不宣称完整PDF2符合性。后续接入解释器资源查找并补间接资源/保存重读，BBox裁剪/状态恢复/循环预算/分层来源仍待实现，fuzz未运行。

- 2026-10-05 Form 字典前置：新增 `PdfForm::try_new`，检查 stream/Subtype、支持 FormType1、必须 BBox、默认 identity 或六个有限坐标 Matrix，并经上下文解析数组/坐标引用；保留 Resources 缺省/null 与显式空字典差异，Group 仅保留字典不执行。Do/Form 分支先校验字典再明确返回解释不支持，未返回虚假空绘制。普通测试覆盖默认/平移、资源作用域差异、Group、缺 BBox、错误类型/数组长度/FormType；原 subtype 引用测试补合法 Form BBox，继续验收 UnsupportedFeature。PDF 库336项单元测试通过。初编译两次误用本仓库数字接口，已改用 PdfObject::as_f32 与 PdfNumber::new；不改变数字类型。后续仍需嵌套资源查找、BBox clip/状态隔离、循环与聚合预算、分层来源；本轮未跑完整工作区/fuzz，P3-02不勾选。

- 2026-10-05 复合字体链路后工作区回归：`cargo test --workspace --locked --quiet` 完整结束，退出0；PDF库335项单元、186项集成及render23项，共544项测试通过，doc-tests未发现测试项。核对普通CI仅运行工作区测试，既有fuzz工作流保留手动配置且job显式false禁用，未执行fuzz或远端CI。完整回归证明当前测试矩阵未发现回归，不证明全部PDF规范/格式支持；Form/内联图片解释、操作级整体预算、真实字体可见性等仍未完成，执行队列不勾选。

- 2026-10-05 真实复合字形独立对照：inspect_glyph_outline 改用统一 SFNT 入口；只读 check-glyph-outline.py 支持简单/复合并新增 --require-composite 门槛，输出实际复合样本数，拒绝空轮廓/空输入。复用本机非可变 Arial.ttf（SHA256 525979822591a3447cfc49d943d6f7683508e25543407871c0ed8fed05fd2bd9），é/Å/ö/ñ/ç 五个实际复合字形 GID、FontTools BoundsPen 边界和 PointInsidePen 非零绕数网格全部一致：1445 点，0 Boundary 排除。示例编译通过；仅 A 加 require-composite 退出1，空列表退出2；git diff --check通过。未下载/复制/分发系统字体或安装依赖，无 PDF 文件变更；此有限合法字体采样不证明全部复合 flags、phantom/hinting/metrics/变体或 PDF 可见性，完整任务不勾选，fuzz 暂缓。

- 2026-10-05 PDF 复合轮廓接入：新增 `Font::glyph_outline`，共用真实 code/CID→GID 与字体程序检查，保留原 simple-only API；TextObject 显示轮廓/边界/填充几何查询改用统一入口。扩展现有 SFNT 单测构建 Type0/Identity-H、显式 CIDToGIDMap 流和 FontFile2，字符 65→复合 GID0、66→空 GID2，无 Unicode cmap；原文档及严格保存重读都验证 em 轮廓、显示位置独立标量预期、顶点 Boundary 与空字形，simple-only 对复合仍拒绝。PDF 库全部 335 项单元测试通过（扩展既有测试，数量不增长）；尚未真实复合字体对照、render/集成全量，无 phantom/hinting/metrics/变体或可见性保证，fuzz 继续暂缓。

- 2026-10-05 SFNT 复合 em 入口：新增 `TrueTypeProgram::checked_glyph_outline`，统一简单/复合/空字形；复用同源 SFNT 表、GID 和 unitsPerEm 检查，在默认实例真实点展开及二次命令生成后归一化 em。原 `checked_simple_glyph_outline` 保留 simple-only 行为，共用内部归一化逻辑。新增三字形四表 SFNT 测试，独立验证 0.5 缩放加 [3,5] 位移后 /1000 坐标，简单两入口一致、空/越界、深度限额及旧入口复合拒绝。PDF 库 335 项单元测试通过；尚未接入 Font/显示查询或真实字体对照，无 phantom/hinting/metrics/变体与整体峰值保证，fuzz 暂缓。

- 2026-10-05 复合轮廓命令：`ExpandedGlyph::outline_commands` 将展开点转为闭合 Move/Line/Quadratic/Close，保留 f64 分数与连续离曲线点的隐含中点，不通过 i32 简单点中转；支持 usize 端点。公开点数据再次检查端点严格递增/全部归属/有限坐标，点＋端点＋命令逻辑字节累计约束，可失败分配。回归覆盖分数隐含中点、空轮廓、非法端点/NaN/额度，已有三层复合样本进一步验证 8 条命令及独立实际边界 [4,7,19,17]。PDF 库全部 334 项单元测试通过。下一步接入 SFNT/em 与 PDF 字体查询，真实复合字形独立对照仍待做；原 simple API 范围不变，fuzz 继续暂缓。

- 2026-10-05 本轮复合点展开后验证：`cargo test -p pdf --lib --locked --quiet` 全部 333 项单元测试通过，`git diff --check` 通过；未运行集成/render/全工作区或 fuzz，不据此勾选完整字体/渲染任务。

- 2026-10-05 复合字形点展开：新增 `expanded_glyf` / `GlyphLocations::expanded_glyph`，显式栈按组件顺序展开嵌套，保留 f64 分数坐标、on-curve 标志及 usize 轮廓端点；应用完整矩阵、scaled/unscaled offset，未指定采用 unscaled，支持先变换再对齐已有真实控制点。先做依赖图校验，展开过程累计输入/组件/帧/叶转换/合并逻辑字节并限制最终点数，可失败分配；不是容量或整体峰值预算。普通单测覆盖嵌套/顺序/重复组件、负向剪切/分数、offset 两模式、点对齐、空字形/非法索引/循环及额度，TrueType 40 项通过。初测发现缩放 offset 的测试期望算错，按独立矩阵计算修正后重跑通过。仅提供未 hinting 默认实例真实点，phantom 点引用明确报错；尚未转复合二次命令、接入 PDF/em 轮廓或真实字体对照，metrics/网格取整/变体仍待实现。fuzz 未实施或运行。

- 2026-10-05 复合字形依赖检查：新增 `GlyphLocations::validate_composite_dependencies`，迭代遍历检测活动路径循环，允许合法重复引用；重复展开分别计数，限制深度、访问次数与累计解析/逻辑存储预算，返回访问数及最大深度。回归覆盖重复引用 DAG、自环/互环、深度/次数/累计预算、越界 GID、截断叶头和空字形。TrueType 专项 38 项通过；未展开复合轮廓、校验简单叶点数据或实现整体峰值计费。fuzz 实现、运行和 CI 继续暂缓，既有文件保留；后续先推进复合轮廓变换/点对齐，再做真实字体与保存重读验收。

- 2026-10-05 P1/P3复合字形组件记录：新增composite_glyf模块与loca.composite_glyph，区分signed字节/word XY和unsigned点对齐索引，解析F2DOT14等比/分轴/2×2矩阵及原flags/GID，保留任意组件声明的尾部指令（借用、不执行）。首组件XY、保留位、互斥缩放/offset标志、GID范围、输入/组件数量及逻辑字节限额检查，可失败分配。回归覆盖两组件/提前指令标志、负byte/word、点索引300、完整signed矩阵、所有截断、冲突标志/越界/数量及loca挂接；TrueType37项通过，git diff --check通过。规范链接与记录解析≠递归展开/点对齐/metrics/hinting边界已写文档；原simple入口仍拒绝复合，容量/整体峰值未完整计费，全工作区及fuzz未运行。

- 2026-10-05 P3-02真实字形网格独立验收：inspect_glyph_outline新增--grid固定17×17实际边界内单元中心查询；新增只读check-glyph-outline.py，对照FontTools的GID/BoundsPen/PointInsidePen非零绕数，拒绝空字符列表，显式报告数值Boundary排除数。复用既有同哈希静态Noto字体，O/A/B/中/一五个简单字形GID与边界一致，1445分类点全部一致、0边界排除；示例编译成功，空列表退出2，git diff --check通过。未下载字体/安装依赖/改动PDF；确定性合法样本网格不是fuzz，尚不证明全部字形、精确曲线边界、复合或PDF可见性，完整任务继续未勾选。

- 2026-10-05 P3-02真实字体轮廓独立对照：新增只读inspect_glyph_outline示例，单独字体Unicode查询（不是PDF编码替代）；按PDF技能从已有中文PDF以mutool提取FontFile2到独立临时目录，字体SHA256与既有静态Noto记录一致。O=GID48，28轮廓命令、em边界[0.058,-0.013,0.684,0.746]，中心孔洞Outside、左笔画[0.0893,0.3665]Inside、框外Outside。复用已有Python3.11/FontTools4.39.4 BoundsPen/PointInsidePen独立核对GID、边界、孔洞及左笔画一致；默认Python缺包的失败记录明确，不安装依赖、不改PDF或发布字体。示例实际运行成功，git diff --check通过；当前只是一个真实简单字形对照，复合/变体/完整PDF命中和可见性尚待验收，fuzz未运行。

- 2026-10-05 P3-02简单字形填充几何命中：新增outline_hit模块及TextObject.character_outline_location_display，返回Inside/Outside/Boundary；先验证全部闭合命令与查询有限值，非零绕数处理直线/二次曲线，按y极值拆单调段固定64次二分求交，半开端点避免切线重复，不先折线化。归一化避免大数溢出，边界使用64×f64::EPSILON，明确非精确算术；命令预算与无额外容器。回归覆盖曲线内部/边界/控制点框内曲线外、孔洞、同向重叠、切线、水平回折、镜像/极端尺度、非法输入/额度和实际PDF显示轮廓边界查询。TrueType35项通过，git diff --check通过；真实字体/描边/裁剪透明可见性、复合与完整P3-02仍未验收，全工作区及fuzz未运行。

- 2026-10-05 P3-02二次轮廓实际边界：新增glyf.outline_bounds及TextObject.character_outline_display_bounds，检查闭合命令序列/有限值/命令预算，计算二次曲线内部极值而非控制点框；显示曲线先变换再取边界，空命令None，无额外容器。回归覆盖曲线最高10而非控制点20、横向极值、大数、非法/未闭合序列、限额及PDF推进字符独立边界/空字形严格保存重读。TrueType33项通过，git diff --check通过；仍非填充命中/描边可见边界，完整P3-02未验收，全工作区及fuzz未运行。

- 2026-10-05 文字显示轮廓接入后核心回归：cargo test -p pdf --lib --locked --quiet全部325项通过，0失败/忽略，git diff --check通过；本轮未重新运行集成/render全部测试，完整字体/文字命中阶段仍未完成。

- 2026-10-05 P3-02文字显示轮廓接入：新增TextObject.character_outline_display(index,limits)，复用真实字体/CharCode映射及简单em轮廓，以当前字符推进原点和Tf/Tz＋文字/页面矩阵转换全部Move/Line/Quadratic坐标，f64计算并拒绝非有限/f32显示范围溢出，空字形保持空。扩展PDF字体回归，两个同GID非空字形的不同推进位置、剪切Tm、Ts、50%Tz与独立标量公式一致，空字形与非法索引验收，严格保存重读相符；专项通过，git diff --check通过。按需从DisplayList Text查询，不对所有字符无条件复制轮廓；仍无命中、裁剪/透明/描边可见性、复合/hinting/变体或整体峰值预算，fuzz未运行。

- 2026-10-05 PDF字体轮廓接入后的关联回归：cargo test -p pdf --lib --locked font:: --quiet字体单元全部70项通过，0失败/忽略，git diff --check通过；未用局部字体组结果替代全工作区或阶段验收。

- 2026-10-05 P3-02 PDF字体字符轮廓接入：Font新增simple_glyph_outline(&CharCode,limits)，按既有PDF code/CID→GID选择字形、检查u16范围并读取可用字体程序的em轮廓，不通过Unicode重新猜测映射。扩展SFNT回归构造真实PDF Type0/Identity-H＋CIDToGIDMap＋FontFile2，字符65→GID0、66→空GID1，显示列表字体查询及严格保存重读结果一致，专项通过；初编译误用ObjectId方法名已按实际obj_num修正。仍未接入提取字符/轮廓命中，真实字体/复合/hinting/完整峰值预算待验收；全工作区及fuzz未运行。

- 2026-10-05 P1/P3 SFNT简单轮廓与em归一化：新增TrueTypeProgram.checked_simple_glyph_outline(data,gid,limits)，复用同源maxp/head/loca/glyf范围/点/轮廓检查，读取head.unitsPerEm并检查16～16384，将命令坐标原地归一化为f64 em；空字形无命令，越界GID及复合/CFF拒绝。四表自编SFNT回归16/1000/2048/16384的实际坐标、空/越界及非法单位/复合拒绝；字体组32项通过，git diff --check通过。尚未从PDF编码选取GID、接入字符/渲染命中、复合/hinting/变体或统一聚合峰值；本轮未全量回归或fuzz。

- 2026-10-05 P1/P3 TrueType简单字形二次轮廓：新增GlyphOutlineCommand及SimpleGlyph.outline_commands，将真实点序列转Move/Line/Quadratic/Close，不折线近似；首点离曲线按末曲线点或首尾中点起笔，连续离曲线插入隐含中点，末离曲线用二次曲线闭合，多轮廓独立结束。f64保留半整数中点；公开数据修改后的端点/数量一致性重新检查，命令上界/字节预算及可失败分配。回归覆盖全曲线点、单控制点、连续/全离曲线点、末控制点、多轮廓、非法范围、空轮廓及限额；字体组31项通过，git diff --check通过。尚未接入字体映射/单位归一化/渲染命中，复合/hinting/可变实例仍待实施；全工作区及fuzz未运行。

- 2026-10-05 P1/P3 TrueType简单字形点解析：新增glyf模块SimpleGlyph/GlyphPoint和loca.simple_glyph接口，解码严格递增轮廓端点、借用指令、重复标志、短/有符号16位坐标增量、unchanged轴与on-curve标记；零轮廓允许仅头，复合字形明确拒绝。输入/轮廓/点数量与端点＋临时flags＋points字节额度前置检查，可失败分配/累加溢出拒绝。新增短重复、负长增量、曲线点、全部截断、重复越界、点/内存限额、指令借用、端点倒序及loca挂接回归；TrueType30项通过，git diff --check通过。尚未实现二次曲线路径、复合展开/hinting/边界交叉验证或文字命中，聚合操作预算仍未完成；全工作区和fuzz未运行。

- 2026-10-05 P1/P3 TrueType字形头前置：新增GlyphHeader公共头读取及GlyphLocations.glyph_header，在既有loca边界上零拷贝读取有符号轮廓数与四个设计单位坐标；空loca范围None/零轮廓Some区分，负轮廓判断复合、不把推荐-1强制成唯一允许值。新增简单/零/复合、负坐标、所有头截断、倒置/相等边界、越界GID及短字形拒绝回归。font::truetype全部28项通过，git diff --check通过；官方glyf规范及声明边界≠真实轮廓/变体实例/精确命中已写入font-cmap-validation.md。完整字形解析和文字命中仍待实施，未运行全工作区或fuzz。

- 2026-10-05 P3-02字符局部坐标仿射验收：扩展text_geometry，以独立标量方程顺序施加Tf/Tz、Ts、Tm、cm及页面变换，覆盖旋转/剪切/镜像Tm、剪切CTM、-75%水平缩放、3单位文字升高、1.5UserUnit和四种页面旋转，三个局部点正/逆转换及严格保存读回相符；NaN/Infinity输入点拒绝。集成组2项通过，git diff --check通过。该验证支持后续命中坐标基础，不提供真实轮廓、垂直字形指标、裁剪/透明可见性或布局编辑；本轮未跑全工作区或fuzz。

- 2026-10-05 P3-02文字局部坐标前置：ExtractedCharacter新增em_to_display_matrix及display_to_em_point，以每个推进字符的baseline为原点，包含Tf/Tz和文字/页面变换及UserUnit，不使用run矩阵的原始平移冒充所有字符位置。新增text_geometry覆盖AB不同字符原点、四种旋转、10pt/50%Tz/1.5UserUnit、局部(1,1)与独立页面变换参照、逆转换及严格保存重读，零水平缩放/NaN字号拒绝；专项1项通过，README说明不等于字形轮廓/边界或可见性命中。完整文字/路径命中和P3-02继续未勾选，fuzz未运行。

- 2026-10-05 P3-02顶层图片来源定位验收审计：逐项检查Interpreter.run在读取操作时记录索引、Do绘制直接绑定token偏移，以及ImageExtractionReport的文档/页面/revision匹配；image_extraction覆盖跨流Do、注释、重复绘制、未知操作索引和保存重读，image_replacement覆盖按来源单次替换/共享隔离/旧快照拒绝，image_remove覆盖跨流删除与邻接路径保留。cargo test -p pdf --locked --test image_extraction --test image_replacement --test image_remove --quiet共18项通过（9/7/2），git diff --check通过。仅勾选当前队列的顶层图片项，完整P3-02及Form/内联、文字路径命中仍未完成；未运行全工作区或fuzz。

- 2026-10-05 AR-01/P2 UserUnit几何缓存原子性修复：检查图片来源剩余范围时发现update_dimensions在UserUnit检查前已提交bbox/宽高/旋转矩阵，失败会污染旧缓存。改为局部构建所有几何，完成单位值、显示尺寸及checked_mul检查后统一提交；扩展既有geometry回归覆盖零/负/错误类型/尺寸溢出，以及显示尺寸有限但平移系数溢出的页面，失败保留原框、宽高及全部六个矩阵系数，恢复合法输入后正常刷新。PDF geometry筛选12项及render UserUnit2项均通过，git diff --check通过；本轮未全量回归，Form/内联图片解释与来源仍未实现，fuzz未运行。

- 2026-10-05 UserUnit外部样本检查自动化：新增只读docs/check-user-unit-example.sh，以qpdf/mutool/jq按实际页树顺序检查1.6、六页单位/旋转/MediaBox/CropBox、逐页RGB图片元数据及全部提取文本，不依赖固定对象编号。原样本通过、交换前两页拒绝（1）、缺参数拒绝（2），bash语法及git diff --check通过；复现命令与字段检查≠视觉验收边界已写入merge-extract-validation.md。没有修改原样本或新增fuzz任务，完整兼容验收仍待推进。

- 2026-10-05 P1/P2/P4 UserUnit独立阅读器样本：新增create_user_unit示例，生成output/pdf/rzpdf-user-unit-validation.pdf（PDF1.6，非零CropBox，Helvetica文字、RGB红绿图和蓝色路径）；1.5比例覆盖四种旋转，另含0.5/2.5未旋转页。示例实际运行成功；qpdf11.2.0 --check无语法/流编码错误；MuPDF1.27.0以72dpi渲染6页，PNG尺寸逐页断言为240×120、120×240、240×120、120×240、80×40、400×200，并逐页查看确认文字完整、图像方向和蓝块位置符合旋转/缩放。mutool文本提取六条标签完整。Poppler仍因缺libtiff.5.dylib退出134，未修改系统依赖；此项只证明该合法样本的qpdf/MuPDF检查，不证明跨阅读器完整兼容。fuzz未运行。

- 2026-10-05 P2 UserUnit叶页/默认值传输边界：新增user_unit_is_leaf_only_even_after_copy_and_import，严格解析源Pages根含UserUnit=9，四叶页分别缺失/直接null/间接null/间接数值2；验证本库查询、同文档复制、跨文档批量导入及严格保存重读均为1/1/1/2，不将非继承字段物化到叶页。清除真实叶值恢复1而非父节点9，复制页与已导入目标保持2。page_transfer_edit全部4项通过，git diff --check通过；新增测试不改变既有实现，不代表独立阅读器或完整规范验收，fuzz未运行。

- 2026-10-05 页面传输变更后的关联回归：resource_import单元8项与page_transfer_edit集成3项均通过，0失败/忽略，git diff --check通过。本轮未重新跑全工作区；独立阅读器及阶段完整验收仍保留待办。

- 2026-10-05 P2 UserUnit跨文档编辑闭环：新增page_transfer_edit集成回归，以严格解析的1.6源页覆盖0.5/1.5/2.5单位、不同旋转；低版本1.5目标批量导入/整文档合并拒绝且stamp、页数与有效版本不变，目标严格保存读回仍正常。1.6目标批量导入、合并、重排抽取和严格保存重读保留单位、旋转及尺寸；目标单页清除不影响同值其他页和源页，源stamp保持。page_transfer_edit全部3项通过，git diff --check通过；沿用既有源有效版本门槛，无新增自动升级/功能降级。独立阅读器、完整内容与版本兼容矩阵尚待验收，fuzz未运行。

- 2026-10-05 UserUnit小数混合渲染后的组回归：cargo test -p render --lib --locked --quiet全部23项通过，0失败/忽略；git diff --check通过。本轮未重跑全工作区或独立阅读器；完整兼容验收及阶段任务继续保留未勾选。

- 2026-10-05 P2/P4 UserUnit小数比例混合渲染：新增fractional_user_unit_matches_dpi_scaling_for_text_and_images，非零CropBox与四种旋转覆盖0.5/1.5/2.5及72×72、72×144 DPI，Helvetica文字、2像素RGB图和蓝色实色路径混合。将单位缩放与未修改共享页的DPI缩放作独立管线参照，全部RGBA及实际输出尺寸相等，严格保存读回相等；检查红/绿/蓝与纯黑文字像素确实存在。初始12pt文本在缩小后未形成纯黑像素，改24pt后专项通过；保留缩小比例及完整像素比较。README同步限定范围；不替代任意比例/字体/图片或独立阅读器验收，fuzz未运行。

- 2026-10-05 P1/P2 UserUnit构建入口：PageBuilder新增user_unit(f32)，默认省略字段；add_page_with_builder在事务/对象分配前检查有限正值、有效PDF≥1.6，并复用PdfPage.geometry验证缩放尺寸及矩阵，创建时写叶页字段，不重写源坐标。新增1.5倍非零Media/Crop＋90度严格保存读回、非法值/溢出/低版本拒绝、revision和页数不变、失败不消费对象编号回归。最初整份输出字节比较因字典遍历顺序不固定而失败，改用严格重读语义状态、revision及下一对象编号验证；最终PDF全部320项单元通过，git diff --check通过。README同步；本轮未重跑全工作区/独立阅读器，像素完整比例和内容矩阵仍待验收，fuzz未运行。

- 2026-10-05 UserUnit像素变更后的渲染组回归：cargo test -p render --lib --locked --quiet全部22项通过，0失败/忽略，git diff --check通过。本轮未重新运行整个工作区，上一轮522项基线另见下方记录；独立阅读器及完整比例/内容类型矩阵仍待验收。

- 2026-10-05 P2/P4 UserUnit像素回归：新增user_unit_scales_rotated_pixels_and_survives_save，非零CropBox下覆盖0/90/180/270度，72dpi输出尺寸翻倍，红/绿/蓝/白实际像素存在；以未缩放图的独立2×像素复制为参照逐像素比较全部RGBA，验证叶页共享隔离、严格保存读回和清除恢复。专项1项通过，git diff --check通过。该参照限定整数对齐实色路径，不证明小数比例、字体/图片、独立阅读器或完整UserUnit验收；fuzz未运行。

- 2026-10-05 页面几何/UserUnit变更全量回归：cargo test --workspace --locked --quiet终止码0，PDF单元319、集成182、render21，共522项通过，0失败/忽略；git diff --check通过。覆盖本轮几何逆矩阵、四角矩形包围盒、UserUnit读取/版本门槛/编辑入口及已有常规回归；已有编译警告仍保留。全量通过不替代真实样本、独立阅读器或聚合预算验收，fuzz未运行。

- 2026-10-05 P2 UserUnit页面编辑入口：新增set_page_user_unit(index, Option<f32>)，Some设置叶页比例并要求有效PDF≥1.6，None删除字段恢复默认1；提交前验证几何，不解码Contents，不改写内容/批注/页面框源坐标。page_geometry共8项通过，新增非法值/显示尺寸溢出/不存在页面拒绝且revision不变、共享页隔离、外层事务回滚、严格保存重读与低版本拒绝回归。README同步接口；专用构建器、像素及独立阅读器验收仍待推进，阶段保持未勾选，fuzz继续暂缓。

- 2026-10-05 P2/P4/AR-04 UserUnit读取与保存门槛：按Adobe PDF1.6页字典定义，读取叶页直接/间接有限正数（缺失/null默认1），几何显示宽高及全部坐标矩阵乘单位比例，暴露PageGeometry.user_unit，不继承父节点。Writer已知特性扫描上限扩至1.6，Page非null UserUnit在低版本拒绝；原ObjStm/XRef与16位图像门槛补明确version<1.5条件，不误拒1.5。page_geometry7＋feature_versions10＋page_rotation2共19项通过，覆盖2.5倍、逆坐标、1.5拒绝/显式1.6严格保存读回及零/负/非有限/尺寸溢出拒绝，git diff --check通过。规范：https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.6.pdf 。尚无专用构建setter，像素/跨阅读器与完整版本/几何矩阵待验收，全工作区及fuzz本轮未运行。

- 2026-10-05 P2/P3 仿射矩形边界修复：Matrix::transform_rect原仅转换两个对角点，旋转/剪切会遗漏极值或返回倒置边界，现按四角取轴对齐包围框；新增checked_transform_rect拒绝非有限输入/变换溢出。矩阵3项通过，专项覆盖恒等、90度、45度系数、剪切、双轴反射、反向输入角点，以及NaN/Infinity/溢出拒绝；git diff --check通过。当前仓库没有transform_rect调用点，属于公开几何工具修正与未来定位基础，不能据此宣称已完成路径/文字命中、可见性或可视化编辑，全工作区及fuzz本轮未运行。

- 2026-10-05 P2/P3 显示到用户坐标查询：新增Matrix::checked_inverse（f64中间、f32有限结果校验）与PageGeometry::display_to_user_point，支持将显示定位转回未旋转用户空间，不裁剪或推断命中。矩阵2项与page_rotation2项共4项通过：仿射/反射/旋转往返、1e30/1e-30缩放（行列式在f32会溢出/下溢）、奇异/非有限/逆溢出拒绝，四页角度非零裁剪四角往返及NaN点拒绝。git diff --check通过，README同步范围；f32结果仍有舍入，不代表条件数控制、完整命中/可视化编辑或聚合预算验收，全工作区及fuzz本轮未运行。

- 2026-10-05 页面几何与流转全工作区基线：cargo test --workspace --locked --quiet正常退出0，PDF单元315、集成182、render21，共518项通过，0失败/0忽略。覆盖独立geometry查询、间接/继承页框、旋转整数词法统一、跨文档拆分合并后内容编辑及整幅像素保持；原创建、提取、内容编辑、保存与渲染测试均通过。git diff --check通过。仅当前工作区证据，不代替真实样本/独立阅读器、完整所有权/布局/聚合预算及其余未勾选任务验收，fuzz未运行，完整目标仍在进行。

- 2026-10-05 P2 旋转读写整数契约统一：页面读取与rotate_page共用normalized_page_rotation，按原整数lexeme解析i64、验证90倍数并先归一化，不再相对编辑时额外限制为i32或经f32舍入。新增正负2147483700和接近i64边界的正负9223372036854775800，显式预期分别180/0，旋转90后严格读回正确；i64越界、非90倍数、90.0读取与相对编辑均拒绝且revision保持。page_geometry6项通过，git diff --check通过；有效继承角度错误现沿用读取端PageResourceError，不改变绝对setter的输入校验。不代表任意精度整数/完整几何与P2验收，全工作区及fuzz本轮未运行。

- 2026-10-05 P2 独立页面几何查询：新增公开PageGeometry及PdfPage::geometry，复用页面框/旋转验证并返回有效交集、归一化角度、显示尺寸和user_to_display矩阵，不读取Contents/字体。新增四角度缺失图片Do夹具，DisplayList确实失败但几何查询成功；验证非零裁剪框四角公式、查询stamp保持、已持有页面旋转后刷新、旧几何值仍为快照及严格保存重开。page_rotation2项通过，git diff --check通过，README同步使用与快照边界。非完整页面属性API、真实样本/独立阅读器或聚合预算验收，全工作区及fuzz本轮未运行。

- 2026-10-05 P2 继承间接页框导入闭环：page_transfer_edit新增实际严格读回的两页源，叶页删除MediaBox/CropBox/Rotate，父Pages持有间接数组及四个间接数值和90度旋转；导入顺序[1,0]后两页显示60×80。目标第一页裁剪重置采用已解析MediaBox变为100×200，已持有页面刷新、第二页仍60×80，保存严格重开尺寸保持；源两页及stamp未改。page_transfer_edit现2项通过，git diff --check通过。夹具使用new文档已知Pages对象1，不暴露私有ctx；本轮非全工作区/像素/独立阅读器或完整继承矩阵验收，fuzz未运行。

- 2026-10-05 P2/P4 页面框间接坐标修复：Rectangle增加内部上下文转换，页面MediaBox/CropBox读取和set_page_crop_box解析四个坐标引用后沿用原有限数值/规范化校验；普通无上下文TryFrom不自动猜测对象引用。新增间接数组＋四个间接数值专项，验证尺寸、裁剪重置、严格保存重开及已持有页面在数值修改后刷新；坐标更新为null时读取/编辑拒绝且revision不变。page_geometry5项通过，git diff --check通过。对照PDFium数组矩形读取入口：https://pdfium.googlesource.com/pdfium/+/refs/heads/main/core/fpdfapi/parser/cpdf_array.cpp 。当前仅页框上下文解析，不代表所有几何数组/多级引用/继承矩阵或完整P2/P4验收，全工作区及fuzz本轮未运行。

- 2026-10-05 P2-08 页面流转像素矩阵：新增render公共流程专项，四种Rotate＋非零CropBox、红绿双像素方向纹理及蓝路径，源先断言红/绿/蓝/白实际存在；跨文档合并到已有空白页之后，严格读回，再按[4,2,1,3]重排拆分并二次严格保存重开。逐页整幅RGBA与对应源图完全一致，源页面重绘与stamp保持。merge_and_reordered_extract_preserve_rotated_page_pixels专项1项通过，git diff --check通过；当前是本库Skia自编RGB/路径样本，非独立阅读器、真实样本或所有字体/资源/继承与所有权场景验收，全工作区及fuzz本轮未运行。

- 2026-10-05 P2/P3 拆分合并后内容编辑闭环：新增page_transfer_edit公共集成，创建四角度共享字体/图片且非零CropBox的四页源，按[3,1]顺序extract_pages，再merge_document进另一文档并严格保存重开。逐字符验证Unicode、位置、writing_axis及页尺寸；图片RGBA和四角变换保持；在目标第一页清空文字后二次严格保存重开，图片仍在、第二页文字不变，原源/拆分源Page3文字及stamp均保持。专项1项通过，git diff --check通过；当前为自编Helvetica/RGB样本，未作独立阅读器/像素或完整继承/所有权矩阵验收，全工作区及fuzz本轮未运行。

- 2026-10-05 文字清空全工作区基线：cargo test --workspace --locked --quiet正常退出0，PDF单元313、集成179、render20，共512项通过，0失败/0忽略。新增整事件清空、批量TJ及跨流/空数组边界、共享页隔离/外层回滚和四类文字删除像素回归均纳入，原创建/提取/页面编辑/保存专项无回归。README补公开入口及字形推进/旧对象残留边界；未据此完成P3布局、真实字体、独立阅读器或聚合预算验收，未勾选阶段任务，fuzz未运行。

- 2026-10-05 P3-03 文字清空跨流与空数组边界：text_remove新增跨三流的TJ（转义字符串及操作符均跨界）＋第四独立未知操作流专项，验证原流基数、TJ数值/注释、邻接操作保持，非目标第四流字节与对象ID未改，共享页全部旧字节/ID保存前与严格重开均保持。另验证空TJ、仅数值TJ清空后数组值不变且严格读回。text_remove现5项全部通过，git diff --check通过；不以跨流语法/共享隔离证据代替字体/布局/完整P3或安全擦除验收，全工作区及fuzz本轮未运行。

- 2026-10-05 P3-03 文字清空像素闭环：新增render专项，Tj/TJ/两类引号四组合，原AB及独立Tm定位的C均断言确有墨迹，删除后目标区域全白、区域外逐像素保持，蓝矩形颜色正确，共享页原整图不变，严格保存重开整页RGBA与保存前一致。cargo test -p render --lib --locked removing_text_draw_clears_ink_without_changing_independent_neighbors --quiet退出0，1项通过，git diff --check通过；文档明确独立Tm邻接证据不代表连续文本布局补偿，仅Helvetica自编样本，真实字体/独立阅读器、完整P3及预算仍待验收。全工作区及fuzz本轮未运行。

- 2026-10-05 P3-03 TJ清空批量化：将整事件清空逻辑下沉ContentProgram，TJ借用源字节单次逐项扫描、一次生成替换缓冲、一次完整程序重解析，不再每字符串clone/重解析整个程序。预校验数组项/数量及源原始和解码上限，可失败预分配源长度，字符串替换为不增长的空十六进制token；数值/注释/其他操作保持。新增1500项数组（900字符串含转义/空串、600数值、300注释）及未知邻接操作严格保存重开专项；text_remove3、content_program13共16项通过，git diff --check通过。文档更新算法与限额边界，仍不补偿字形推进/不作安全擦除，完整性能与操作聚合预算未验收，全工作区及fuzz本轮未运行。

- 2026-10-05 P3-03 整个文字事件清空：新增remove_page_text_draw，覆盖顶层Tj/TJ/两类引号，保留操作符、引号状态与TJ数值/注释，先验证完整TJ项类型再准备替换，使用来源校验与事务提交。text_remove两项通过，四种操作分别验证邻接C保留、引号换行、共享页原源字节、严格重开、旧快照拒绝；另测非法项/非文字/越界无修改，以及无字体依赖的TJ清空和外层失败回滚。git diff --check通过，文档说明取消字形推进、不补偿布局、不是安全擦除，逐项TJ重解析性能与完整预算仍待验收；本轮未重跑全工作区，fuzz未运行。

- 2026-10-05 旋转与路径派发修复全工作区基线：修正overlay_workflow旧镜像坐标预期后，cargo test --workspace --locked --quiet正常退出0，PDF单元313、集成174、render19，共506项通过，0失败/0忽略。覆盖相对旋转、裁剪后的图像与文字方向、来源修订、共享页隔离、严格保存重开、纯填充/描边派发及整幅像素旋转；上次退出101记录保留，不以途中通过替代终点。该结果仅证明当前工作区测试，真实样本/独立阅读器矩阵、完整预算及其余未勾选任务仍待实施，fuzz未运行。

- 2026-10-05 旋转修复跨功能回归：cargo test --workspace --locked --quiet实际退出101，PDF单元313和此前集成通过，但overlay_workflow的文字原点/方向专项仍沿用旧镜像矩阵。按非零CropBox左上原点/Y向下的顺时针变换，修正90/270文字原点、writing_axis及第二字符推进方向断言；重跑overlay_workflow13项全部通过，git diff --check通过。不修改字体/文字解释以迎合旧预期；本次全工作区未到终点，不能作为完整绿色基线，后续需重跑全工作区，fuzz未运行。

- 2026-10-05 P4 DisplayList路径派发契约回归：新增公共内容解释到重放的专项，覆盖S/s/f/F/f*/B/B*/b/b*九种绘制操作，记录实际Device回调，断言直接Interpreter与DisplayList重放的纯描边/纯填充/填充加描边序列一致；三种回调各自失败时立即停止，不绘制下一对象、不调用end_page。page::display_list::tests三项通过，git diff --check通过。补强上轮纯填充误描边修复的非像素证据，不将九种派发视作完整路径/裁剪/渲染兼容性验收；本轮未重跑全工作区，fuzz未运行。

- 2026-10-05 P2/P4 旋转像素回归发现并修复两处绘制缺陷：页面90/270矩阵曾直接沿用Y向上旋转，遗漏显示Y翻转，产生镜像；现统一左上原点/Y向下。既有geometry与page_rotation的错误预期同步纠正，上一条坐标专项不再作为旧矩阵正确性证据。DisplayList重放还把false+Winding/EvenOdd派发为填充加描边，现所有纯填充派发fill_path，消除意外黑边。新增非零CropBox、非对称红绿蓝图形四角度整幅像素逐点旋转对照，包含裁剪边缘、原稿隔离、严格重开全缓冲一致；初测确实暴露黑边后才修复。PDF单元312、page_rotation1、image_display_translation3及render全部19，共335项通过，git diff --check通过。旋转/Y翻转参考PDFium当前UpdateDimensions与GetDisplayMatrix组合：https://pdfium.googlesource.com/pdfium/+/refs/heads/main/core/fpdfapi/page/cpdf_page.cpp 。README标记旧坐标兼容影响；本轮未运行完整工作区、独立阅读器或fuzz，不代表完整P2/P4验收。

- 2026-10-05 P2-03 相对旋转显示闭环：新增page_rotation公共集成专项，四种初始角度×四种正负跨周增量共16组合，验证已持有PdfPage刷新尺寸与来源stamp、旧stamp失效、保存不改当前stamp。裁剪原点非零，逐一按独立坐标公式核对图像四角；原稿共享页保持原角度，两页内容字节与共享Contents对象ID保持，保存前与严格重开均通过。cargo test -p pdf --locked --test page_rotation --quiet退出0，1项通过，git diff --check通过。本轮是几何与保存证据，非像素/独立阅读器或完整P2验收，未重跑全工作区，fuzz未运行。

- 2026-10-05 P2-03 相对页旋转：新增rotate_page(index, degrees)，基于有效Rotate（包括父节点及间接值）累加，仅写叶页，先归一化再相加避免i32溢出；无效继承角度/非90倍数/不存在页拒绝且revision不变。page_geometry四项专项通过，新增继承负角度、邻页与父节点保持、大角度、外层事务回滚及严格保存重开证据；初始夹具被叶页显式Rotate遮蔽，已移除以实际覆盖继承。README同步相对/绝对语义，不变换内容或注释；P2完整样本与视觉验收仍未完成，本轮未重跑全工作区，fuzz未运行。

- 2026-10-05 非fuzz主线全工作区回归：cargo test --workspace --locked --quiet退出0，PDF单元310、集成173、render18，共501项通过，0失败/0忽略。覆盖文字局部插删、单次图片删除、路径绘制取消及混合事务回滚，新增DisplayList混合载荷/裁剪副本、绘制前检查与q/Q裁剪栈限额均无回归。架构文档合并预算说明，区分源解释、活跃栈和事件留存的独立计数及前置检查边界；这些仍非统一峰值内存池，AR-03和完整P3保持未完成。能力摘要补齐图片删除与路径取消，不将逻辑删除视作安全擦除；fuzz未实现、未运行、未接入CI，既有文件保留。

- 2026-10-05 AR-03 q/Q裁剪栈复制边界：Interpreter增加活跃栈裁剪段计数，q在状态克隆/压栈前校验累计max_path_segments，Q按弹出状态释放，计数加减检查溢出/不一致。display_payload_budget现4项通过，新增五段裁剪两层q限额9拒绝/10成功，限额5反复qQ成功且revision保持；image_transform4项无回归，共8项通过，git diff --check通过。仅栈裁剪段，源/留存/栈仍分别限额，未统一AR-03跨组件峰值池及全部状态预算，fuzz及全工作区本轮未运行。

- 2026-10-05 AR-03 绘制预检查：Device新增默认放行只读preflight_draw，TraceDevice复用对象数/留存路径段验证；Interpreter在文字/路径状态克隆和图片解码前询问，最终回调仍复核。display_payload_budget新增额度9时重复裁剪先拒绝（不触发未知图像过滤器）、额度10时到达解码并按既有FilterError拒绝专项，证明检查顺序；修正测试最初误用UnsupportedFeature类别为实际FilterError，不改变解码错误契约。仅对象/段前置，载荷/状态栈/字体及完整AR-03仍未完成，fuzz未运行。

- 2026-10-05 AR-03 留存裁剪复制累计：TraceDevice内部对象收集计PathObject几何与文字/图片/路径GraphicState中的裁剪路径副本，重复留存累计受max_path_segments限制，仅在成功提交事件后推进计数。display_payload_budget现2项通过，新增源re五段裁剪一次留存5成功、两次留存限额9拒绝/10成功且revision保持；path_edit4项无回归，共6项通过，git diff --check通过。检查晚于对象/状态构造，不覆盖先行克隆/容量/栈/原生后端，AR-03不勾选，全工作区及fuzz本轮未运行。

- 2026-10-05 AR-03 DisplayList混合载荷累计：TraceDevice内部收集将字符Unicode实际字节和图片/软蒙版样本共用留存载荷计数，文字和图片提交前检查，不推进失败事件计数。新增display_payload_budget1项，使用小型CID/CMap避开AFM前置限额，96图片字节+31/32Unicode成功、+33超限，文字/图先后六组合均正确且revision保持；image_extraction9项无回归，共10项通过，git diff --check通过。检查在事件已生成后，未计所有容器/容量/缓存/临时峰值，非完整AR-03 Budget，全工作区及fuzz本轮未运行。

- 2026-10-05 AR-03/P3 DisplayList收集边界修复：回归复现图片提取累计超限但直接DisplayList仍成功，内部TraceDevice改为受文档Limits的对象数/重复图片规范化样本+软蒙版累计检查，Vec可失败增长，并关闭未使用的调试XML副本。image_extraction9项通过，覆盖255/256边界及重复绘制；新增私有收集max_objects=1第二对象拒绝/集合保持/无XML及公开TraceDevice仍输出XML单元。检查在图像解码后，未覆盖所有容器/临时峰值/replay或原生内存，AR-03不勾选，fuzz未运行。

- 2026-10-05 P3 独立样本检查自动化：新增只读check-content-edit-example.sh，qpdf JSON检查两页/MediaBox/独立Contents/共享图片字体及图片尺寸，MuPDF精确文本和XML trace验证文字/图像/路径事件数、显示旋转矩阵及邻接蓝路径一致性。当前样本退出0；qpdf另生成缺第二页的合法PDF，结构检查通过但样本脚本退出1，负对照有效；bash -n/git diff --check通过。按PDF技能记录只读检查与原视觉证据范围，未修改交付PDF；不作为通用符合性/像素或完整P3验收，fuzz及全工作区本轮未运行。

- 2026-10-05 P3 独立工具内容编辑交付样本：新增edit_content示例，两页原稿/编辑稿在事务内局部标题替换、图片显示旋转、图片删除及路径填充取消，每步刷新快照，自身严格读回；产物output/pdf/rzpdf-edit-content-example.pdf。cargo run退出0，qpdf检查退出0，MuPDF提取标题及两页100dpi渲染退出0，按PDF技能完整目检纹理方向/删除/邻接蓝图/原稿保持无视觉缺陷。Poppler因libtiff.5缺失退出134，采用MuPDF而未修改系统。验证记录见docs/content-edit-example-validation.md；仅自编样本，完整P3/字体矩阵等仍未验收，fuzz及全工作区本轮未运行。

- 2026-10-05 P3 混合内容编辑外层回滚：新增content_edit_transaction1项覆盖外层事务内文字局部BC→XY、单次Do删除、路径填充改n的嵌套提交，每步刷新快照；事务内保存严格重开确认暂存三项生效，再分别注入Err/panic。回滚恢复原源字节及Contents对象引用，revision单调推进/旧快照失效，原页与共享页保存前/严格重开均保留图片资源、一图三绘制事件。专项通过，git diff --check通过；文档明确事务内输出字节不会撤回，完整失败矩阵/P3/聚合预算仍待实施，全工作区及fuzz本轮未运行。

- 2026-10-05 P3 删除路径绘制：新增remove_page_path_draw，九种填充/描边操作改为n以保持几何和路径结束、挂起裁剪提交，复用快照/事务COW。path_edit4项通过，新增九操作×严格重开/裁剪记录/邻接路径/共享页/非绘制拒绝；render18项通过，新增删除红填充保持W裁剪，后续蓝矩形仅剪内显示、外围白、保存前后整页RGBA一致。git diff --check通过；只矩形像素及本库后端，曲线/复杂闭合裁剪、独立阅读器和完整P3仍待验收，fuzz及全工作区本轮未运行。

- 2026-10-05 P3 单次图片绘制删除：ContentProgram零操作替换修正数量及邻接映射（不使用usize减一偏移），提供without_operation；高层remove_page_image_draw验证Image资源并复用快照/事务COW，真正移除Do，不插入n。image_remove2项覆盖跨流Do/正在构建路径/邻接图/资源与共享页保持/严格重开，以及离线首末唯一操作/空流基数/来源清除；content_program13和image_transform4项无回归，共19项通过，git diff --check通过。保留旧对象/图像数据，非安全擦除；Form/内联/完整编辑与聚合预算未验收，fuzz及全工作区本轮未运行。

- 2026-10-05 P3 局部编辑最终字符串限额修复：新增max_text_chars=3下旧ABC与插入DEF各自合格但拼接超限回归，修复前实际失败（编辑错误允许），修复后对最终拼接字符码再次chars_limited检查，在流写入前拒绝且revision/源内容保持；等长ADC替换可在相同限额严格保存重开。仅目标字符串计数，非全页合计/AR-03峰值预算；完整P3仍未完成，fuzz未运行。

- 2026-10-05 内容来源与局部编辑全工作区回归：cargo test --workspace --locked --quiet退出0，PDF单元309、集成164、render17，共490项通过，0失败/0忽略。涵盖统一来源身份、路径/文字/图片定位、解码字符范围及混合码宽局部插删、引号行为、图片几何命中与显示仿射/像素保存重开。README、能力摘要与架构文档同步当前实现及来源/坐标/字体边界，git diff --check通过。Form/内联、文字路径命中/显示编辑、可见性、布局、真实样本/独立阅读器及聚合预算等仍待验收，不按测试数量宣称完成，fuzz未运行。

- 2026-10-05 P3 局部文字编辑混合码宽与插删：text_source新增一/两字节混合codespace+CID+ToUnicode间接流夹具，确认0..1/1..3/3..4，切入中间或中间零长插入拒绝且revision保持，合法B→A严格保存重开字符码与Unicode正确；另覆盖TJ空字符串项插入、局部删除、逐项清空，数组基数及正负间距保持，每步严格重开提取。专项现8项通过，git diff --check通过。仅自编映射，非真实字体矩阵/布局/P3整体验收，全工作区及fuzz本轮未运行。

- 2026-10-05 P3 局部文字编辑取消整页绘制依赖：抽取font_for_text_operation供Unicode编码及字符码边界共用，直接用目标字体解析旧字符串/校验边界、同字体编码新文本；取消局部编辑的整页DisplayList/图片解码与字符报告分配。text_source6、overlay_workflow13项通过，新增页面渲染因后续缺失图片失败但前面局部编辑/严格保存重开正确、未支持绘制原字节保持回归。未知前缀仍保守拒绝，不保证资源符合性或修复页面；完整字体/布局/P3验收未完成，fuzz未运行。

- 2026-10-05 局部文字编辑多字节边界回归：overlay_workflow竖排两字节CID字符码范围中间1..2编辑明确拒绝且revision不变，保存读回仍保留原字符。overlay_workflow13及content_program13项通过，git diff --check通过；本轮相关专项共31项，非全工作区或完整字体矩阵验收，fuzz未运行。

- 2026-10-05 P3 字符串局部Unicode编辑：新增replace_page_text_range，要求Tj/引号或TJ字符串项及解码字符码范围，通过同修订字符报告检查完整字符码边界、有效字体编码Unicode，保留前后字符码并复用词法字符串替换/事务COW。text_source5项通过，新增四类操作局部BC→XY/前后字符/TJ间距/原操作/共享页/严格重开及越界拒绝revision保持验证。完整页面解释/解码限制、转义拼写重写和无布局补偿在文档明确；多字节边界专项继续验证，完整P3/全工作区/fuzz本轮未验收。

- 2026-10-05 P3 字符码内部来源范围：CharItem和ExtractedCharacter新增可选decoded_code_range，按字体/CMap实际字符码长度累计并检查边界，TJ每字符串项重置。text_source现4项通过，新增八进制/括号转义及hex源码与解码范围区分、数组项重置/严格重开；overlay_workflow13项通过，竖排两字节CID范围0..2/2..4保存前后正确。git diff --check通过；不是Unicode或原始转义源码范围，字符串局部编辑入口/字形命中/Form与完整P3仍未完成，全工作区及fuzz未运行。

- 2026-10-05 P3 显示空间旋转像素闭环：render新增40×40红/绿两像素横纹图，公共显示仿射(40-y,x)转为竖纹，检查红绿方向/旧区域白/邻接蓝图保持；编辑后与严格保存重开整页RGBA缓冲精确一致。render现17项全部通过，git diff --check通过。仅本库Skia/自编样本，独立阅读器及任意仿射采样、裁剪/透明/完整P3验收仍待实施，全工作区及fuzz本轮未运行。

- 2026-10-05 P3 图片显示空间仿射变换：新增transform_page_image_draw_display，按C*D*inverse(C)反算隔离用户空间矩阵，移除图片归一化完整反射/位移，f64中间值/f32最终参数；平移入口统一复用。image_display_translation现3项通过：新增四页面旋转×显示旋转/缩放/剪切镜像12组合，四角按显示空间请求逐点验证，严格保存重开/邻接保持；既有16平移组合和失败拒绝无回归。git diff --check通过；未自动选择支点/绝对定位、文字路径显示编辑、像素可见性及完整P3验收，fuzz与全工作区本轮未运行。

- 2026-10-05 P3 图片显示坐标平移：新增translate_page_image_draw_display，完整解释并核对集合来源，从CTM/page线性变换反算用户空间位移（移除图片归一化y反射），沿用来源快照及q/cm/Do/Q事务COW。image_display_translation2项覆盖四页面旋转×四CTM组合/CropBox/严格保存读回/邻接与共享页保持，另覆盖奇异/非有限/溢出/非法目标且revision/内容不变，全部通过，git diff --check通过。仅平移、f32精度且需页面解释/图片解码，不支持未知编码无解码移动或显示空间旋转缩放，完整P3仍未完成，全工作区及fuzz本轮未运行。

- 2026-10-05 P3-02 图片几何命中：ImageObject新增显示坐标单位方形仿射判定，DisplayList提供无分配、后绘制优先的图片候选迭代；f64中间计算避免有限f32矩阵的行列式上溢/下溢，边界包含、奇异无二维面积、非有限报错。image_hit2项覆盖重叠顺序/严格保存读回/来源选择变换及旋转剪切镜像/极大极小尺度/边界/非法数值，均通过，git diff --check通过。仅几何候选，不含裁剪透明遮挡、文字路径命中或显示坐标编辑反算，P3-02未勾选；全工作区及fuzz本轮未运行。

- 2026-10-05 P3 来源集合身份统一：抽出私有PageSourceProvenance，图片/字符/run报告及DisplayList共用同文档/同页/同修订规则。文字报告提供stamp与配对查询，DisplayList手动构造无stamp且不匹配；run分组搬运保留报告身份。新增两类文字报告及DisplayList的跨页/离线/重开/旧修订/当前性和手动列表拒绝专项，text_source3、image_extraction9、path_edit3项通过，git diff --check通过。身份不认证可变载荷、消费集合会丢弃身份，Form来源/稳定句柄/聚合预算与完整P3验收仍待实施；本轮全工作区及fuzz未运行。

- 2026-10-05 P3 图片来源配对契约：ImageExtractionReport保留文档修订及页面身份，提供source_stamp()/matches_program()，拒绝跨页、跨文档、重开、离线与不同修订ContentProgram配对。新增同页匹配、另一页、离线同字节、严格重开、修改后新快照及两个旧快照仍互相匹配但currentness失败专项；image_extraction9项通过，git diff --check通过。调用方需同时验证当前性与配对，不自动把裸索引升级为安全句柄；文字/路径报告身份和完整P3验收仍待实施，本轮全工作区及fuzz未运行。

- 2026-10-05 P3 引号文字操作编辑：with_replaced_text_codes及Unicode有效字体绑定支持Tj/单引号/双引号，重新解析源操作定位字符串词法片段，只替换字符码并保留原操作符及双引号数值/注释；沿用快照事务/COW提交。新增Helvetica等宽A→B的两类引号替换/换行与邻接位置/间距/严格保存读回验证，text_source2及content_program13项通过，git diff --check通过。字符宽度变化的布局补偿、Form与完整编辑验收未完成，全工作区及fuzz本轮未运行。

- 2026-10-05 P3-02 顶层文字绘制来源：TextObject新增TextDrawSource，字符/run提取保留操作索引及字节偏移；TJ字符保留完整数组项索引，包括数值/空字符串对索引的影响。新增跨流TJ、Tj/两类引号来源对应、run一致性及通过字符来源替换单个TJ字符串严格保存读回专项，text_source 1项通过。字形/字符串内部范围、引号编辑、Form及稳定句柄未实现，文档记录结构体构造兼容变化，fuzz未运行。

- 2026-10-05 P3-02 顶层路径绘制来源：PathObject新增可选PathDrawSource及source()，记录显式几何操作索引和绘制操作索引/字节偏移；来源跨流与q/Q保持，n和绘制后清空，s/b隐式关闭不伪造几何操作。新增交错颜色/状态、多流、丢弃路径、严格保存读回及来源选择几何编辑回归，path_edit 3项通过。来源向量采用可失败分配及操作数边界，非聚合预算；CTM历史/裁剪来源/Form内联/稳定句柄与完整P3验收仍未完成，fuzz未运行。

- 2026-10-05 P3 图片来源到编辑闭环：新增通过提取来源索引选择跨流拆分的第二次Do进行替换，验证首张图片和共享页面不变、注释/两流边界保留、旧快照拒绝且revision不变、严格保存读回重新提取来源与新ContentProgram一致。image_replacement现7项通过；cargo test --workspace --locked --quiet退出0，PDF单元309、集成149、render16，共474项通过，0失败/0忽略。来源不升级为稳定句柄，Form/内联及完整P3验收仍待实施，fuzz未运行。

- 2026-10-05 P3-02 顶层图片绘制来源：ImageObject新增可选ImageDrawSource及source()，解释器直接记录从零操作索引和Do操作符字节偏移，手动构造无来源。新增重复Do/注释/跨流拆分Do的严格保存读回与ContentProgram一致性，以及兼容模式未知操作计数回归；image_extraction 8项通过。文档说明修订生命周期及结构体字面量迁移，不把索引作为稳定编辑句柄。Form/内联图片、其他绘制来源和完整P3-02验收仍未完成，fuzz未运行。

- 2026-10-05 几何编辑全工作区回归：cargo test --workspace --locked --quiet退出0，PDF单元309、集成146、render16，共471项通过，0失败/0忽略；git diff --check通过。覆盖路径参数编辑、图片隔离四操作替换/qQ深度保护、仿射组合和两类保存前后像素回归，既有文字/共享编码流/保存测试无回归。当前能力摘要及架构文档同步公共快照+COW提交、最新资源页字典、全替换块/邻接操作比较契约。P3-04及整体目标仍未完成，命中/显示坐标反算/Form内联定位/聚合预算与独立验收待继续；fuzz未运行。

- 2026-10-05 P3 路径编辑像素回归：render新增公共矩形参数编辑后30×10页面三中心像素验证，首个红矩形平移后旧区域白/新区域红，邻接蓝矩形保持；保存前及严格保存重开RGBA精确一致。render16项及path_edit2项通过，git diff --check通过；README补路径参数/图片相对变换入口，内容文档记录本库Skia而非独立阅读器/全部路径轮廓验收。P3-04与完整目标仍未完成，本轮全工作区/fuzz未跑。

- 2026-10-05 P3 图片变换CTM与像素验证：image_transform新增既有非均匀CTM下旋转/缩放/镜像三组合，按图片左上归一化与页面显示转换手算线性/平移分量，后续绘制矩阵保持，保存前后验证；专项现4项通过。render新增30×10红/蓝图公共编辑像素回归，10倍既有缩放中相对平移1单位使红图移10页面单位，原区域白/新区域红/后续蓝图位置保持，保存前与严格重开中心RGBA精确一致；render15项通过，git diff --check通过。文档明确提取坐标约定及非独立/非所有仿射裁剪验收，P3-04整体验收仍未完成，本轮全工作区/fuzz未跑。

- 2026-10-05 P3 图片变换状态深度保护：离线图片变换检查整个Contents q/Q零操作数/配对/配置深度，目标处预留新增q的一层额度；超限或结构异常在文档写入前拒绝。新增max_depth8下源7层变为8层成功并相同限额严格保存读回可解释，源8层拒绝且revision/原字节不变；Q欠配、q未闭合及q/Q带操作数拒绝。image_transform3及PDF单元309项通过，git diff --check通过；文档明确q/Q定向验证，其他语义/原生状态克隆聚合预算仍待审计，P3整体未完成，全工作区/fuzz本轮未跑。

- 2026-10-05 P3-04 单次图片相对变换：新增transform_page_image_draw及ContentProgram::with_transformed_image_operation，为目标Do生成q/cm/Do/Q隔离块；局部拼接私有化支持明确操作数量，比较整个替换块与所有邻接操作/操作数，仍保留流边界/来源清除及COW提交。新增NaN/非Do拒绝revision不变、首张图片平移(10,20)/后续绘制矩阵恢复、数据不变、共享另一页保持及严格保存读回专项。image_transform1、content_program13、image_replacement6及PDF单元309项通过，git diff --check通过。文档明确相对当前用户空间、不反算显示CTM；全局图形状态/深度、复杂裁剪/视觉与P3-04整体验收仍未完成，本轮全工作区/fuzz未跑。

- 2026-10-05 P3-04 路径几何参数编辑：新增replace_page_path_numbers，支持m/l/c/v/y/re原操作参数替换，校验旧参数与新参数数量/有限数值，通过来源快照及公共事务/COW提交保留邻接操作/注释和共享另一页。矩形负宽、NaN/Infinity/参数错数/非路径/错页拒绝revision不变，六类操作严格保存读回数值/原操作名/描边操作及DisplayList无诊断。path_edit2及PDF单元309项通过，git diff --check通过，文档明确当前用户空间/f32、非显示坐标反算/命中/整路径联动或独立像素验收。P3-04仍未完成，本轮全工作区/fuzz未跑。

- 2026-10-05 图片编辑全工作区回归：cargo test --workspace --locked --quiet退出0，PDF单元309、集成140、render14，共463项通过，0失败/0忽略；git diff --check通过。覆盖页面图片绑定替换、单次Do替换及公共内容提交保留新资源的变更，图片专项6项均在全回归内；README补两入口/作用范围及快照回滚规则。P3-04仍含图片移动、路径修改、完整来源定位/独立视觉/聚合预算等缺口，未勾选；fuzz未运行，完整项目目标仍待持续实施。

- 2026-10-05 P3 图片Do已有名称复用/转义：新增目标图已绑定A /#资源的公共单次绘制替换回归，确认写成/A#20#2F#23 Do、复用资源名称集合且操作数目不变，严格保存读回仅第一张蓝图、邻接绘制及共享原页仍红。image_replacement现6项全部通过，git diff --check通过，文档说明已有名称复用与转义；本轮为代码路径证据而非新增转义实现，不代替完整单次Do定位/视觉/峰值预算验收，全工作区及fuzz未重跑。

- 2026-10-05 P3 单次图片替换限额回滚：设置max_decoded_bytes等于原Contents长度，新名注册后操作替换因必需分隔换行超限1字节，确认ResourceLimit、资源名称/原内容字节/原流ID恢复，保存严格读回仍为红图；revision按事务契约推进使旧快照失效，重复旧快照拒绝不再推进revision。image_replacement现5项通过，git diff --check通过，文档区分状态恢复与修订号回退、逻辑内容额度与聚合峰值预算。未新增聚合Budget或单次Do视觉验收，P3与整体目标未完成，本轮全工作区/fuzz未重跑。

- 2026-10-05 P3 单次图片Do替换：新增replace_page_image_draw，当前来源快照/目标单名Do/旧新Image及同文档句柄检查后，外层事务内注册页面资源并局部替换操作。公共内容提交在准备后重新读取页字典，保留新注册资源；未变流复用、变化流COW沿用文字编辑提交。新增两次同名绘制仅第一张改蓝、第二张及共享另一页保持红、原注释、错页/非Do/过期拒绝revision不变、严格保存读回与外层回滚资源名称和原字节恢复。image_replacement4、content_program13及PDF单元309项通过，git diff --check通过。文档明确顶层Do、变换保持、非自动尺寸/非Form内联定位/非安全擦除；P3完整定位/视觉验收/聚合预算仍待实现，本轮全工作区及fuzz未跑。

- 2026-10-05 P3 图片替换类型拒绝专项：image_replacement新增null/普通字典/无Subtype流/Form流新句柄拒绝，逐项确认revision、目标页完整对象及共享原图片不变；已有名称实际指向Form时即使新图有效也拒绝，目标页与revision保持。专项现3项通过，git diff --check通过，内容文档同步非图片替换边界。本轮仅补类型失败证据，不新增Form替换/单次Do编辑或全图编码校验；P3整体验收仍待推进，全工作区及fuzz未重跑。

- 2026-10-05 P3 图片替换继承资源验收：image_replacement新增两页从共同父节点继承Resources，覆盖直接及间接Resources/XObject映射；替换后父节点/另一页完整字典不变、目标页Contents引用不变、Font类别保留，严格保存前后蓝/红两图解码正确。以copy_page返回ID和Parent/Kids关系定位，不依赖默认对象号。专项现2项通过，git diff --check通过，内容文档补继承隔离边界；本轮未新增图片编码/单次Do定位/视觉验收，P3总体验收仍未完成，全工作区及fuzz未重跑。

- 2026-10-05 P3 图片资源绑定替换：新增ResourceManager::replace_image_handle，校验同文档句柄、旧/新Image流及名称/页号，复制有效继承Resources/XObject后事务写回目标页，不修改共享原图或内容字节。新增两页共享红图、目标页改蓝而另一页红图保持、同名两次绘制、严格保存读回、外部句柄/缺名/非法页拒绝且revision不变、外层事务回滚集成；image_replacement1及PDF单元309项通过，git diff --check通过。文档明确全页同名绑定替换而非单次Do/Form/内联图编辑，仅类型校验、不重新验证所有编码，非安全擦除/整体峰值预算；P3图片整体验收仍未完成，本轮未跑全工作区或fuzz。

- 2026-10-05 AR-04 Trailer流结构保存回归：新增Trailer直接流/数组内流/嵌套字典内流三类拒绝专项，验证Writer既有字节、revision及Trailer本身均不变；合法Trailer自定义字段的间接流引用严格保存读回保留引用ID与载荷。PDF单元309项通过，git diff --check通过。本轮补充上轮流间接对象约束在Trailer路径的证据，未增加Trailer编辑公共API、未验收全部对象图或文件系统错误矩阵；整体目标仍未完成，全工作区/fuzz未重跑。

- 2026-10-05 AR-04 流间接对象保存约束：保存图校验拒绝数组/字典/流字典内部的直接流，独立间接流+引用仍正常严格保存；新增stream_structure2项覆盖三类无效嵌入、Writer目标与revision不变和合法引用读回。全回归识别overlay_workflow两项旧夹具的Encoding/ToUnicode直接流，已改为独立对象引用且保留原提取/竖排断言。修正后cargo test --workspace --locked --quiet退出0，PDF单元308、集成134、render14，共456项通过，0失败/0忽略，git diff --check通过。架构文档记录规范与不自动提升对象的边界；完整图符合性、AR-04及项目仍未完成，fuzz未运行。

- 2026-10-05 AR-04 外部流保存版本收口：FFilter与Filter共用已知编码最低版本检查，支持名称/数组/间接引用；按PDF流字典规定，非null F/FFilter/FDecodeParms字段本身要求有效1.2。新增外部Flate/JBIG2/JPX版本拒绝及显式升级、Writer目标/revision保持、不存在外部文件仍只保留内联字节与字段专项；独立字段门槛专项不作为字典整体符合性验收。feature_versions10、stream_edit9及PDF单元308项通过，git diff --check通过，文档更新已覆盖FFilter且不读取外部载荷。未知编码/完整特性版本、外部流解码与AR-04整体验收仍未完成；本轮全工作区及fuzz未运行。

- 2026-10-05 AR-04 压缩对象删除保存闭环：structural_rewrite新增原始/Flate源×预加载/未预加载四组合，删除对象4后触发页面懒展开仍拒绝解析；移除Catalog示例Private入向引用后全量保存严格重开，主动解析保留ObjStm也不复活free对象，二次保存仍保持删除，保存不改revision。专项现3项全部通过，git diff --check通过；文档明确旧结构流原字节仍保留，逻辑删除不等于安全擦除/涂黑，不自动清理所有入向引用。未新增回收或擦除功能，AR-04整体、真实样本矩阵仍待实施；本轮未跑全工作区或fuzz。

- 2026-10-05 AR-02/AR-04 压缩对象事务失败闭环：structural_rewrite新增未显式读取原压缩对象即更新为99、页面Contents嵌套修改/懒展开原ObjStm、事务中全量保存严格重开确认99，再分别注入Err和panic；原始/Flate源四组合均恢复原值11及空内容，回滚后保存严格读回仍正确，revision按既有契约单调推进。structural_rewrite现2项通过，git diff --check通过；本轮只新增事务集成证据，不新增缓存实现、不代替真实样本/资源限额矩阵验收，未重跑全工作区或fuzz，整体目标仍未完成。

- 2026-10-05 结构解析与版本约束全工作区回归：cargo test --workspace --locked --quiet退出0，PDF单元308、集成128、render14，共450项通过，0失败/0忽略；git diff --check通过。涵盖外部流读取边界、Metadata/16位图像/结构流版本检查及实际活动ObjStm/XRef修改重写回归。顶部日期/能力摘要按现有实现和验证记录更新，移除中文闭环和内容程序尚完全缺失的过时描述，保留完整样本矩阵/布局/版本审计等缺口。fuzz未运行，AR-04与完整项目目标仍未完成。

- 2026-10-05 AR-04 活动对象流重写闭环：新增structural_rewrite公共集成，四个活动对象均位于ObjStm且由活动XRef流索引，覆盖原始/Flate源流×Preserve/FlateUnencoded保存四组合。实际发现并修复两处缺陷：限定对象边界内字典尾部空白导致stream探测EOF误报；PDFContext初始化压缩对象缓存把流内索引误作generation，现固定压缩对象generation=0。修改压缩数值对象/页面Contents后全量保存，按startxref精确定位新传统表，严格读回页数/值99/两操作/无诊断，第二次保存仍保持99。专项1项及修复后PDF单元308项通过；仅自编单页夹具，非真实样本/跨阅读器/完整结构链验收，AR-04仍未勾选，fuzz及本轮全工作区未运行。

- 2026-10-05 AR-04 结构流版本保存边界：全量writer生成传统xref表但保留已插入结构流，补齐Type ObjStm/XRef（直接/间接）的有效1.5门槛，低版本拒绝且Writer目标/修订号不变。空ObjStm带N/First、空XRef带W/Size，显式升级严格读回保留字节及页数；普通字典同名Type不误判。feature_versions8及PDF单元308项通过，git diff --check通过，规范与边界同步内容文档。本轮不证明活动压缩对象/xref链验收、结构流清理或xref流写出；AR-04未完成，全工作区及fuzz未运行。

- 2026-10-05 AR-04 16位图像版本保存边界：按PDF 1.5图像字典，对Subtype Image流的整数BitsPerComponent=16（直接/间接）要求有效1.5；普通自定义流同名字段不误判。新增1.4保存拒绝、Writer目标/修订号不变、显式1.5严格读回和真实DeviceGray样本原字节[0x80,0]保留/解码RGBA[128,128,128,255]专项。feature_versions7及PDF单元308项通过，git diff --check通过；文档记录规范与非完整图像符合性审计边界。其他功能最低版本、整体保存验收及AR-04仍待完成；本轮未运行全工作区或fuzz。

- 2026-10-05 AR-04 Metadata流版本保存边界：按ISO 32000-1 §14.3.2补齐Type为Metadata流的最低1.4检查，支持间接Type；高层setter原有门槛之外，覆盖低层插入/导入及已序列化未使用对象。新增1.3拒绝/显式1.4升级/原字节保留/Writer目标与修订号不变、普通同名字典不误判专项；feature_versions6、metadata12、PDF单元308项通过，git diff --check通过。文档记录规范来源及非XML/XMP语义验收边界；组件Metadata关联、其他版本特性与AR-04总体验收仍待完成，本轮未跑全工作区或fuzz。

- 2026-10-05 AR-04 外部流读取边界：PdfStream解码遇到F/FFilter/FDecodeParms明确UnsupportedFeature，不把内联占位字节当真实外部内容、不读取文件或网络；与现有替换/压缩保守策略一致，包括null字段。新增三个字段/null的失败且原字典字节保持单元回归，以及合法qQ占位页面在保存前后ContentProgram/DisplayList均拒绝、读取不改修订号/原字节的集成回归。PDF单元308、stream_edit集成9项通过，git diff --check通过；架构文档同步读取边界并纠正Preserve与版本检查的引用解析说明。未运行本轮全工作区或fuzz；外部文件流读取、完整版本审计及AR-04总体验收仍未完成。

- 2026-10-05 保留流版本最终回归：cargo test --workspace --locked --quiet退出0，PDF单元307、集成123、render14，共444项通过，0失败/0忽略；git diff --check通过。版本约束及实现边界已同步README和内容编辑文档；fuzz未实施或运行，AR-04及整体项目仍待后续验收。

- 2026-10-05 AR-04 保留流版本约束：最低版本检查移至输出对象准备之后，覆盖保留编码流的Filter名称/数组/间接引用；Flate/Fl最低1.2、JBIG2最低1.4、JPX/Crypt最低1.5，低版本拒绝且不修改文档，显式升级后保持原编码字节。PDF单元307与feature_versions集成5项通过；真实Flate负载保存读回后字节不变且解压正确，其他过滤器占位负载只证明版本检查和原字节保留，不证明解码支持。普通字典Filter不误判；未知编码、外部FFilter及其他版本特性仍待审计，AR-04不勾选，fuzz继续暂缓。

- 2026-10-05 透明版本最终回归：补充ExtGState映射/未使用项为null不误拒绝后，cargo test --workspace --locked --quiet退出0，PDF单元307、集成121、render14，共442项通过，0失败/0忽略；git diff --check通过。本轮为定向透明最低版本检查，不代替全部功能版本审计或组/图形软蒙版实现，fuzz专项未运行。

- 2026-10-05 AR-04/P4 透明最低版本保存检查：writer新增feature_versions定向语义扫描，在输出对象写入前使用Header/Catalog/显式输出有效版本检查1.4门槛；识别声明Type的透明ExtGState、Page/Pages/Form资源中的无Type或间接ExtGState、Image SMask和透明Group。不猜测普通同名自定义字段，遍历/待处理项/资源映射额度与可失败分配；没有自动升级或改变源文档。新增低版本拒绝/显式升级/写入目标不变/修订号不变、直接和间接资源、低层SMask插入及RGB/自定义字段不误判、Form组/资源和Typed状态专项，feature_versions3项通过；Catalog覆盖满足1.4但保留源头1.3单元通过。高层RGBA构建器原已有1.4拒绝，回归单独检查保存边界而不绕开构建器错误。文档链接Adobe透明规范并说明序列化孤立对象也被检查、其他功能版本/整体峰值/组绘制及独立阅读器未验收；AR-04/P4-04不勾选，fuzz暂缓。

- 2026-10-05 混合模式最终工作区回归：按ISO规范修正未识别名称数组回退Normal后，重新运行cargo test --workspace --locked --quiet退出0，PDF单元306、集成118、render14，共438项通过，0失败/0忽略；git diff --check通过。P4-04仍包含未完成的颜色管理/透明组/图形软蒙版、功能最低版本和独立阅读器验收，fuzz专项未运行。

- 2026-10-05 P4 混合模式接入：GeneralState新增默认Normal的BlendMode，BM解析16种标准名称/Compatible别名与名称偏好数组，首个可用模式确定选择、验证全部元素和额度；单个未知名称在其他状态应用前诊断/严格拒绝，字体绑定复用解析。核对ISO 32000-1 §11.6.3后，名称数组全部未知或为空时回退Normal，不保留早期过严拒绝策略，接口文档链接规范。Skia路径/文字/图片传递混合模式并受q/Q恢复。新增偏好数组/Normal回退/别名/非可分离名称/严格保存与恢复、非法BM及未知单名称不部分应用集成；render新增9种常见模式×alpha1/0.5的RGB公式与qQ恢复、Multiply文字/SMask图片像素保存前后验证，专项通过。未支持诊断样本改用图形SMask None，不再把Multiply当未实现。其余非可分离/任意混合颜色空间/色彩管理、组/图形软蒙版、功能最低版本/独立阅读器仍待验收，P4-04未勾选，fuzz暂缓。

- 2026-10-05 常量alpha完整回归：cargo test --workspace --locked --quiet 退出0，PDF单元306、集成117、render12，共435项通过，0失败/0忽略；git diff --check通过。本轮未运行fuzz；P4-04混合模式/图形软蒙版/透明组及功能版本、独立阅读器验收仍未完成。

- 2026-10-05 P4 常量alpha绘制：ExtGState新增ca/CA有限[0,1]验证，GeneralState默认填充/描边alpha均改为1；Skia路径、文字与图片应用相应alpha，图片与已有图像SMask相乘。路径B及文字Tr2改为先填充后描边的两次绘制，使用独立颜色/alpha；文字paint每次重置避免前序线型残留，描边使用线宽/端点/连接/斜接。新增alpha状态/qQ/严格读回、三类非法alpha集成；render新增半透明红填充/灰描边/qQ恢复、RGBA软遮罩×ca、Tr0/1/2全透明但仍提取ABC的保存前后像素专项，专项通过。原未支持诊断样本改用BM Multiply（不再把已支持ca当未支持）；混合模式/图形软蒙版/透明组、功能最低版本与外部阅读器验收仍待实现，P4-04未勾选，fuzz暂缓。

- 2026-10-05 ExtGState基础完整工作区回归：cargo test --workspace --locked --quiet 退出0，PDF单元306、集成116、render11，共433项通过，0失败/0忽略；git diff --check 通过。运行范围为常规单元/集成/绘制测试，未运行或修改fuzz专项；P4-04透明度/组/软蒙版/混合及其他未勾选项仍待实施。

- 2026-10-05 P4 ExtGState基础解释：gs接入资源感知解释器，支持Font、LW、LC、LJ、ML、D、FL和可选Type；应用Font/字号到文字状态并复用线型验证，解析间接参数/虚线元素。未知或尚未实现的透明度/混合/软蒙版条目在应用前检查，严格拒绝、兼容诊断并忽略整个状态；字体绑定共享支持键检查，避免与解释状态不一致。原AllState无资源上下文的process_ext_gs不再假装成功，非空状态明确拒绝；虚线数组改为受限可失败分配。PDF单元306通过；新增extgstate集成3项（常规参数/qQ恢复/严格保存读回、未支持状态不部分应用、非法值/缺失资源拒绝）；既有字体绑定专项扩展严格提取BC与字号12。render新增ExtGState字体/虚线与显式操作符保存前后逐像素相等专项通过。P4-04仍未完成，透明度/混合/软蒙版/组仍待继续，fuzz暂缓。

- 2026-10-05 P3 Unicode提交字体绑定：新增replace_page_text（Tj）/replace_page_text_array_item（TJ单项），复用来源快照验证/事务写入，提交前追踪目标之前Tf、q/Q及ExtGState Font并按有效既有字体编码；BT/ET跟踪文字对象但不清除字体，状态栈深度/逻辑字节受限。缺失字体、目标在文字对象外、前置未知操作、Q无q、嵌套BT等明确拒绝且不修改内容/修订号。新增自定义ToUnicode低码字体与标准字体切换/qQ恢复、严格读回ABBB及五类失败不变专项；原中文与TJ保存回归改用Unicode提交。PDF单元306及content_program13项通过，差异检查通过。ExtGState编码绑定单元通过，但既有DisplayList仍拒绝gs，不冒充绘制/提取闭环；Form内定位、字体覆盖与布局补偿待继续，P3-03不勾选，fuzz暂缓。

- 2026-10-05 P3 复合字体Unicode编码：新增 Type0::encode_text_with_limits 并接入页面encode_text_for_font，借用既有ToUnicode映射构建候选，验证1～4字节整数可表示性、ToUnicode codespace、Encoding真实解码长度和非零CID；可完成后缀的最长匹配，相同映射按短码/小源码确定选择，最终Encoding解码验证Unicode完整读回。输入/映射项/候选表/偏移表/输出与读回字符数组的逻辑额度受限，加载/容器实际峰值和CPU未统一计费。新增混合1～4字节/多标量/非BMP、同值候选确定性、最长匹配死路回退、CID0/长度冲突/缺失ToUnicode/额度拒绝；公共合成Type0资源将中替换为中文，严格保存读回Unicode、码长[1,2]与编码一致。PDF单元305及content_program11项通过，差异检查通过。合成样本不证明真实嵌入字形可见；有效Tf自动绑定、字体覆盖/新映射与布局补偿仍待完成，P3-03未勾选，fuzz暂缓。

- 2026-10-05 P3 简单字体Unicode编码前置：新增 SimpleFont::encode_text_with_limits 与 PdfPage::encode_text_for_font，显式字体资源反向使用已有ToUnicode，只有ToUnicode缺失才回退Encoding/Differences；支持多标量/非BMP映射，后向动态规划选择可完成后缀的最长映射，同映射用较小字符码，缺失编码明确拒绝。逻辑工作额度覆盖映射字节/项、偏移表与输出，字体加载/原生及流程峰值未聚合计费。新增最长匹配死路回退、重复映射确定性、非BMP、空串、缺失映射、输入/项数/工作字节限额、WinAnsi回退及ToUnicode优先专项；现有公共Tj/TJ保存回归改用页面字体Unicode编码，缺失资源和中文不支持拒绝。PDF单元302及content_program10项通过，差异检查通过；更新内容编辑文档与README。复合字体反向编码、有效Tf自动绑定和布局补偿仍未实现，P3-03保持未完成，fuzz继续暂缓。

- 2026-10-05 常规工作区回归：TJ单项替换与共享编码流验证完成后，cargo test --workspace --locked --quiet 退出0，PDF单元300、集成110、render10，共420项通过，0失败/0忽略；git diff --check 通过。本轮未运行或修改fuzz专项；完整内容编辑、Unicode/布局、聚合预算及其他未勾选任务仍待继续。

- 2026-10-05 P3 TJ单项替换：新增 with_replaced_text_array_item_codes / replace_page_text_array_item_codes；item_index包含数值项但只允许替换字符串，校验全部TJ项为字符串或数值，原始语法定位目标字符串再局部拼接，保留其他文字词法/数字间距/注释。两类文字提交共用来源验证及事务写入。新增转义字符串/十六进制/间距词法保留、非法项和额度拒绝、公共Helvetica严格保存读回ACD/无诊断及过期拒绝；content_program10项通过。补充共享ASCIIHex编码流专项，修改页复制新流，其他页及未修改流保留原字节/编码、修改流保留Private属性且移除Filter；单元专项通过。新增 docs/content-editing.md 并链接README。未实现整段TJ、Unicode自动编码或字宽补偿，P3-03仍不勾选，fuzz 暂缓。

- 2026-10-05 P3 文档级已编码文字提交：新增 replace_page_text_codes，校验文档身份/修订号、页面对象身份、当前字节/流边界/流ID；事务内仅对变化流复制写入，保留未变化引用、空流和流数量，保留修改流的非编码字典属性。ContentProgram 页面快照记录来源页面，离线替换不携带页面身份。公共Helvetica保存严格读回BC、离线/跨文档/错页/过期/非Tj拒绝、跨流替换及外层事务回滚专项通过，content_program8项通过。写入前拒绝不推进修订号；事务回滚遵循既有单调修订契约，恢复内容但使旧快照失效。Unicode字体选择、TJ、字宽补偿与完整布局验证仍待完成，P3-03不勾选；fuzz 实现、运行和CI继续暂缓。

- 2026-10-05 P3 离线替换邻接不变量：替换重解析后逐项比较目标与独立替换操作、所有非目标操作名称/操作数数量/值，不再只凭总操作数一致认为邻接内容保留；新增两侧未知操作的数字/名称/数组操作数不变及输出额度少1拒绝、失败原程序不变回归。content_program7项及差异检查通过。此检查不代表图形状态/资源/字体和布局完整有效，文档提交仍待接入，fuzz 暂缓。

- 2026-10-05 P3-03 已编码Tj替换基础：新增ContentProgram::with_replaced_text_codes，仅允许一个字符串Tj，调用方提供当前字体字符码，复用受限十六进制内容构建和离线替换；ContentBuilder::as_bytes借用生成字节避免额外build复制。新增公共Helvetica编码BC替换A、前后注释保留、非Tj拒绝、既有低层Contents提交及严格保存读回Unicode BC/无诊断回归，content_program6项与差异检查通过。未做Unicode字体选择、字宽补偿/布局、TJ策略或来源校验提交，不冒充完整高层文字编辑，P3-03未完成，fuzz 暂缓。

- 2026-10-05 P3 离线操作替换：ContentProgram新增with_replaced_operation，替换选中lexical_range，保留周围原字节/流数量/空段，跨流替换内容放入起始流；替换必须独立解析为一个操作，末尾追加换行防止新注释吞邻接内容，重解析并检查总字节/操作数，返回离线程序清除来源身份、不修改原程序或文档。新增跨流CustomOp替换/头尾注释/空段/邻接Q保留及空、多操作、索引越界拒绝专项，content_program5项与差异检查通过。尚无文档提交、完整状态/资源/字体验证或高层文字替换，P3未完成，fuzz 暂缓。

- 2026-10-05 P3 编辑词法范围前置：SourceOperation新增lexical_range，与含前导注释/空白的source_range区分；跳过PDF六类空白和行注释，只定位操作数/操作文本，不改变原字节及操作内注释。扩展原始字节专项验证前导注释后q的精确词法范围及所有操作符offset位于两种范围，content_program4项及差异检查通过。此为保留未修改注释的编辑前置，尚无替换/删除提交入口，P3未完成，fuzz 暂缓。

- 2026-10-05 P3-02 操作流内片段定位：ContentProgram新增operation_source_fragments，惰性无分配返回操作范围与各原始解码流的非空交集、流内偏移和拼接偏移，跳过空流；越界操作索引明确错误。扩展跨流hello/CustomOp专项，验证两片段分别来自流0/2、局部/全局字节一致且拼回完整操作源范围，content_program4项及差异检查通过。来源为解码字节而非压缩文件位置，绘制对象映射/稳定编辑定位仍待完成，P3-02未勾选，fuzz 暂缓。

- 2026-10-05 P3-01 命名资源用途：新增ContentResourceKind/ContentResourceUse及SourceOperation::named_resource_use，以借用名/操作数索引标记Tf/Do/gs/CS/cs/sh和BDC/DP命名属性，不复制负载；内置颜色空间及直接属性字典不冒充资源引用，未知操作不猜语义，非法已知名操作数明确错误。新增六类别、名称转义、三种不推断及非法Do专项，content_program4项及差异检查通过。Pattern scn需状态追踪、资源绑定/编辑重写仍待实现，P3-01未完成，fuzz 暂缓。

- 2026-10-05 P3-01 内容来源身份：页面ContentProgram记录各流原始间接ObjectId（直接流为None）及DocumentStamp（文档身份/revision），原始字节构造不伪造来源；新增stream_object_id/source_stamp查询。扩展公共回归验证流ID存在、源revision正确、编辑后快照revision增加、重开文件文档身份不同，content_program3项及差异检查通过。ID不是独立稳定编辑句柄，事务来源有效性/资源引用/编辑序列化仍待接入，P3-01未完成，fuzz 暂缓。

- 2026-10-05 P3-01 页面来源程序入口：新增PdfPage::content_program，读取最新Contents（单流/数组/null/缺省）、解码后保留数组顺序和空段，复用多流精确拼接解析；流数/累计解码字节检查及流容器可失败分配。新增公共读取空页、未知操作保留、替换后新快照/旧快照不变及严格保存读回源字节/边界一致回归，content_program3项及差异检查通过。尚缺来源对象ID/资源引用/稳定编辑定位；解码缓存与多副本峰值未统一计费，P3-01未完成，fuzz 暂缓。

- 2026-10-05 P3-01 多流边界基础：ContentProgram新增from_decoded_streams精确拼接（不插入分隔符），保留每段全局范围及stream_bytes访问，支持空段与字符串/操作跨段；累计源字节和流数量先验额度、可失败源与范围分配。新增跨段hello字符串/未知操作、空流范围/逐段字节原样恢复/越界索引/总字节少1拒绝回归，content_program2项通过，差异检查通过。尚未接入页Contents来源ID/资源引用及编辑序列化，累计源额度不等于多副本/容器峰值，P3-01保持未完成，fuzz 暂缓。

- 2026-10-05 P3-01 来源程序基础：新增ContentProgram::from_decoded_bytes，保存单段解码源原字节与SourceOperation（解析操作/操作数、含前导空白注释的源范围及操作符自身offset）；未知操作符不丢弃，原词法拼写/尾注释保留。源字节/操作数检查与可失败复制/容器分配；新增名称转义、字符串转义、未知CustomOp、范围对应、原字节一致、操作超限及尾部无操作符拒绝专项通过，差异检查通过。尚未接入页Contents多流边界/资源来源/编辑序列化，解析副本与对象容器峰值未统一计费，不把单段基础视为AR-06/P3-01完成，fuzz 暂缓。

- 2026-10-05 P2-01 文本段诊断报告：新增TextRunExtractionReport/extract_text_runs_paint_order_report，单次解释后移动字符入段并保留诊断，不重复解释/复制Unicode；旧段Vec入口保持兼容。扩展未知操作符+有效A专项验证字符/段报告警告一致、Unicode与对象索引保留，overlay_workflow13项及差异检查通过，文档更新。语义段落/来源位置及完整阅读顺序仍待完成，fuzz 暂缓。

- 2026-10-05 P2-01 CMap字典书写模式：from_stream在解码前解析可间接WMode整数0/1，程序没有模式时采用字典值，字典与程序有效模式冲突明确FontError，不静默选择。新增0/1采用、冲突以及负数/2/小数拒绝回归，pdf299项单元及差异检查通过。依据ISO32000-1 CMap字典一致性规则：https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf 。完整字典属性及流值UseCMap链仍待推进，P2-01未完成，fuzz 暂缓。

- 2026-10-05 提取/字体改动整体回归：cargo test --workspace --locked --quiet 正常退出，408项通过（pdf单元298、集成100、render10），0失败/忽略；差异检查通过。README补文本/图片诊断报告与单栏空间排序入口、引用接口文档，并纠正旧“中文字体与ToUnicode未实现”的泛化说法，保留复杂字体/子集/语义阅读顺序等限制。测试不替代全部真实样本/独立渲染或阶段验收，目标仍继续，fuzz未运行。

- 2026-10-05 P2-01 空间排序边界：单栏策略补拒绝负/零/非有限字号，避免负Tf镜像被误认为正向；新增三基线相邻差1但总差2的固定锚点回归、同位置原索引稳定及容差0，索引字节精确额度成功/少1拒绝。overlay_workflow13项与差异检查通过，文本提取文档补边界。仍为显式空间策略而非多栏/bidi/结构树语义阅读顺序，P2-01未完成，fuzz 暂缓。

- 2026-10-05 P2-01 显式空间顺序：TextExtractionReport新增single_column_reading_order_indices，按显示y基线锚点容差分行/行内x排序，返回索引而不修改绘制事件，同位按原索引稳定；检查方向/位置/容差及字符数/索引字节额度，拒绝不适用方向。新增绘制CBA→空间ABC、源事件不变、非法容差/额度/反向拒绝回归；新增docs/text-extraction.md区分绘制事件、单栏空间策略和语义阅读顺序。多栏/bidi/结构树与完整阅读顺序仍待实现，P2-01不勾选，fuzz 暂缓。

- 2026-10-05 P2-01 水平缩放元数据：ExtractedCharacter新增horizontal_scale单位因子，与矩阵分开；writing_axis水平轴乘Tz，竖排y轴不变，溢出返回None，仍不冒充实际宽度/间距推进。扩展q/Q与严格读回专项验证1→0.5→1，并新增Tz-50/0/100与向左/静止/向右推进一致回归；overlay_workflow11项及差异检查通过。阅读顺序、完整来源定位与字体覆盖仍待实现，fuzz 暂缓。

- 2026-10-04 P2-01 文本提取诊断报告：新增 TextExtractionReport 和 extract_characters_paint_order_report，单次解释返回字符与ContentDiagnostic，移动DisplayList对象/诊断；旧字符Vec入口注明丢弃兼容警告。extract_text示例改用报告并拒绝有警告页面，仍保留缺Unicode拒绝。新增未知操作符+有效A兼容提取保留警告、严格读回拒绝专项，overlay_workflow10项通过，示例locked编译和差异检查通过。阅读顺序与完整字体/内容覆盖仍待完成，fuzz 暂缓。

- 2026-10-04 P2-02 提取诊断报告：新增 ImageExtractionReport 与 extract_images_paint_order_report，同次解释移动图片/ContentDiagnostic，不额外克隆解码负载；旧Vec接口明确是丢弃诊断的简便入口。新增未知操作符兼容模式报告kind/name/offset、严格模式拒绝回归，图片提取6项通过；文档更新诊断使用建议。无诊断不等于普遍完整覆盖，峰值预算/嵌套与内联仍待推进，fuzz 暂缓。

- 2026-10-04 P2-02 累计样本预算验收：新增每次绘制96RGB+32SMask字节的公共专项，2次绘制256额度精确成功、255额度拒绝、256额度3次拒绝，单张解码均合法，成功/失败均保持文档revision不变。图片提取5项通过；新增docs/image-extraction.md说明四入口、源引用、样本/RGBA差别、显示矩阵、额度和兼容诊断边界。峰值统一预算/嵌套与内联仍待推进，fuzz 暂缓。

- 2026-10-04 P2-02 图片绘制提取：新增 extract_images_paint_order 返回实际解释的 ImageObject（重复绘制分别保留、未使用资源不输出、矩阵为裁剪旋转后的显示空间）；DisplayList::into_objects移动负载不额外克隆，按对象数及RGB/灰度样本+SMask累计字节检查结果额度。新增同图两次不同矩阵/未使用另一图/透明度及严格保存读回专项，图片提取4项通过，差异检查通过。解释器/DisplayList构建期间峰值不等于结果额度，内联/Form/诊断完整性仍待推进，P2-02未整体验收，fuzz 暂缓。

- 2026-10-04 P2-02 图片资源发现：新增 PdfPage::image_resource_names，解析有效页资源与间接XObject，筛选Subtype Image，输出稳定字典序名称（包含未绘制资源），支持扫描条目/单名称/累计名称字节限额和可失败分配；错误类型/缺Subtype不静默丢弃。新增空资源、未知编码仍可发现、12个未使用图片字典序及严格保存读回回归，图片提取3项通过。Form内部/内联图片和资源扫描原有克隆峰值仍待改进，P2-02未整体验收，fuzz 暂缓。

- 2026-10-04 P2-02 未知编码保留：资源名称额度校验前移到资源解析前，避免超长调用参数先触发资源克隆；新增未知Filter /RzpdfUnknown及私有DecodeParms测试，原始五字节可提取、解码明确失败、严格保存读回后Filter/DecodeParms/字节均保持。图片提取2项专项通过；原始获取不等于完整图像解码，嵌套/内联与峰值预算仍待补齐，fuzz 暂缓。

- 2026-10-04 P2-02 图片提取公开入口：新增 PdfPage::extract_image_source 按页资源名返回原始流/字典及宽高/BPC，保留Filter/DecodeParms和SMask引用，不要求编码可解；extract_decoded_image 返回既有规范化样本模型，rgba_buffer应用软遮罩。移动已解析流取消额外全文副本，原始字节/名称额度检查与名称可失败分配；新增公共RGBA注册→未绘制资源提取→严格保存读回、RGB样本/alpha128及缺名拒绝专项通过。页资源解析/字典引用仍属源文档，不冒充可移植文件；内联/嵌套Form、全部编码和聚合峰值仍待完成，P2-02未勾选，fuzz 暂缓。

- 2026-10-04 P4 负字号与塌缩变换：Skia 使用abs(Tf)字号，Tf负号放入字形仿射线性分量，竖排原点使用对应局部绝对字号；字号/Tz非有限在提交前拒绝。像素专项验证负36Tf与正36Tf加双轴负矩阵逐像素一致且确实可见，奇异矩阵像素保持全白；render10项通过，差异检查通过。此为现有仿射模型专项，裁剪/描边/字体替代等仍待推进，fuzz 暂缓。

- 2026-10-04 P4 竖排字形原点：Skia 在竖排时按W2/DW2的v将水平字形原点定位到书写原点减v，字号/1000换算，Tz仅缩放横向偏移，局部y转换为Skia向下；不改变CharItem基线与提取位置。新增偏移(-1.25,8.8)、来源不变和非有限拒绝回归，render10项通过。依据ISO32000-1图40：https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf 。当前为偏移数值专项，不冒充真实CJK竖排像素/独立阅读器验收，fuzz 暂缓。

- 2026-10-04 P4 文本仿射字形变换：Skia 字号不再乘矩阵a，改用声明Tf字号、完整文本/页面/DPI仿射矩阵绘制字形，显式转换PDF向上/Skia向下的局部y轴；通过f64逆线性变换恢复局部字符位置，溢出在绘制前拒绝，面积塌缩矩阵不绘制，canvas保存/恢复隔离。实际 Helvetica 像素专项新增a=0的90度旋转仍有可见字形及x方向2倍非均匀缩放，既有Tz50/抗重复叠绘测试继续通过，render9项全部通过。负Tf、文字描边/裁剪及复杂CJK样本仍需验收，fuzz 暂缓。

- 2026-10-04 P4 Tz 字形外形：TextObject 新增 horizontal_scale 单位因子，Tj/TJ解释时快照当前值，Skia字体 set_scale_x 应用该值；推进与字形外形均采用Tz百分比。扩展内置 Helvetica 像素专项，以 M 的可见像素边界验证50%宽度约为100%的一半（抗锯齿容差2像素）；render9项通过。一般文本矩阵旋转/剪切/非均匀缩放尚未完整应用，不能据此宣称完整文本渲染；fuzz 暂缓。

- 2026-10-04 P2-01 水平缩放百分比缺陷：q/Q 文本状态专项首次失败，发现 Tz 直接把50用作乘数；解析现在按百分比除100，缺省1.0保持不变。新公共回归验证 Tc5/Tz50/Ts10 对推进和抬升的作用、Q恢复参数而保留已推进文本矩阵、后续TJ/Tj及严格保存读回一致；overlay_workflow9项通过。同步更新提取矩阵文档包含文字抬升；字形轮廓的水平拉伸仍未接入完整渲染变换，P2/P4不宣称完成，fuzz 暂缓。

- 2026-10-04 P2-01 词间距正确性：Tj/TJ 的 Tw 现在只对长度1的原始字符码32应用，不以 ToUnicode 是否为空格判断，同时取消无必要的旧 Unicode fallback 分配。新增 Type0 单/双字节编码专项：code32 映射 A、code65 映射空格，分别验证17/5与5/5的后继推进，两种显示操作路径共四场景通过；overlay_workflow8项及差异检查通过。依据 ISO32000-1 §9.3.3：https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf 。完整文本状态/阅读顺序仍待推进，fuzz 暂缓。

- 2026-10-04 P2-01 文本抬升位置：Tj/TJ 的文本段矩阵及每字符位置现在把 Ts 的 y 平移放在文本矩阵、CTM、页面矩阵之前，不修改用于推进的文本矩阵；修复解析 Ts 后位置仍无变化。新增公共构建、提取及严格保存读回测试，覆盖 Ts0→10→0、Tj与TJ一致、文本段/字符原点一致；overlay_workflow7项通过，差异检查通过。水平缩放、复杂变换及完整文本状态仍待验收，fuzz 暂缓。

- 2026-10-04 P1 文本构建渲染模式：ContentBuilder 新增类型化 set_text_rendering_mode，覆盖 Tr0–7，避免手写操作符；新增全部八种枚举输出及 try_build 验证。不可见文字公共像素回归改用新接口，pdf298项与render9项单元通过，差异检查通过。构建可表达裁剪模式不代表 Skia 已支持文字裁剪，Tr4–7 仍明确拒绝渲染，fuzz 暂缓。

- 2026-10-04 P4 文本模式正确性：Skia 的 Tr3 不可见文本现在不提交绘制，仍保留 DisplayList 与字符提取；尚未实现的 Tr4–7 文字裁剪模式明确 UnsupportedFeature，不再静默当普通填充。新增公共构建/页面提取/渲染专项：Hidden 提取完整、全部像素保持白色，四种裁剪模式逐项拒绝；render9项单元通过，差异检查通过。真正的文本裁剪路径累积与 ET 应用仍待实现，此门控不视为 P4 完成，fuzz 暂缓。

- 2026-10-04 P4 文本批量绘制像素回归：使用项目内置 Helvetica 实际字形，通过公共新建/字体注册/内容替换/页面 display 入口分别渲染 I 与 I I；逐像素比较首字符区域，确认追加字符不改变此前字形，并要求夹具确实产生抗锯齿中间色像素，避免空白假通过。render8项单元全部通过；不依赖系统字体或外部 PDF，未宣称复杂变换/CJK/竖排和透明度全部完成，fuzz 暂缓。

- 2026-10-04 字体修改整体回归完成：前条在运行的 cargo test --workspace --locked --quiet 已正常退出，共391项通过（pdf单元297、集成87、render7），0失败/忽略；文本批量绘制改动后另跑 render7项通过，差异检查通过。普通回归不替代真实字形像素验证，P2/P4未整体验收，fuzz未运行。

- 2026-10-04 P4 文本批量绘制修正：Skia 文本渲染把 draw_glyphs_at 从逐字符循环移到完整文本段收集之后，避免每个字符重复绘制此前累积字形及平方级提交量。render7项单元和差异检查通过；未新增像素专项，不能据此宣称文本变换/透明度或完整渲染验收。工作区整体回归已启动，最终结果待记录，fuzz 暂缓。

- 2026-10-04 P2-01 竖排缺省度量：修复 W2 存在但未覆盖某 CID 时不回退的缺陷；未覆盖条目使用 DW2，DW2 缺省按 ISO32000-1 表117 的 [880 -1000]，水平原点为该 CID 宽度的一半。新增显式 W2、缺项、缺省 DW2、自定义 DW2 四路径回归；pdf297项单元通过。规范来源：https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf 。完整竖排渲染仍待验收，fuzz 暂缓。

- 2026-10-04 P2-01 TrueType CID 字形选择：CIDFontType2 加载可间接解析的 CIDToGIDMap Identity/流，解码流检查双字节条目和条目额度，超出映射范围选择 GID0；Type0 字形查询现在执行原始字符码→CID→GID，不再直接把字符码当 GID。新增实际流 GID300、缺省 Identity、越界/奇数字节拒绝及非 Identity 编码 CID7 字形查询回归，pdf296项单元通过。CIDFontType0 的 CFF charset 未实现，字形查询返回 None，不冒充支持；真实字体渲染与完整验收仍待推进，fuzz 暂缓。

- 2026-10-04 P2-01 非 Identity CID 度量：新增编码 CMap 的字符码→CID 查询，Type0 水平宽度、总宽度及竖排 W2 度量改按 CID 查找，保留原始字符码供 ToUnicode/提取使用；未映射码使用 CID0。新增 source65→CID7 的水平700/竖排-1200、未映射默认宽度与 Unicode A 回归，pdf295项单元通过。CIDToGIDMap/实际字形选择仍待完善，不将本轮视为完整非 Identity 字体支持，fuzz 暂缓。

- 2026-10-04 P2-01 预定义根 CMap 分配边界：new_from_predefined_with_limits 在复制内置根源前检查 max_decoded_bytes，并改用 try_reserve_exact，避免先 to_vec 后才拒绝；新增 Identity-H 根源字节精确边界和少一字节拒绝回归。pdf 294 项单元及差异检查通过；源额度不等于展开映射/合并容器峰值预算，P2-01 仍未完成，fuzz 暂缓。

- 2026-10-04 P2-01 命名字典 UseCMap 接入：新增 Cmap::from_stream，统一 Type0 Encoding/ToUnicode 与 SimpleFont ToUnicode，解析流字典 UseCMap 名称（可间接解析）后继承内置基映射，本地程序优先并共享条目/源字节/深度；流值基映射暂明确 UnsupportedFeature。公共竖排夹具改用空 Encoding 流＋字典 UseCMap Identity-V，字符/模式/位置及严格保存读回6项专项通过；pdf294项单元及差异检查通过。流基映射链、完整字典属性/阅读顺序仍待推进，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 WMode 严格校验：CMap WMode 仅接受0/1，非法/负数/小数/名称及缺失值明确 FontError，不静默回退水平模式；新增本地 WMode0 定义在 Identity-V 继承前/后均覆盖基模式回归。pdf 294 项单元测试和差异检查通过；完整字体模式/字典 UseCMap/阅读顺序仍待完成，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 resolver 字节所有权与拒绝：自定义基映射返回的 Vec 额度检查后直接移入子解析，取消额外全文副本；内置静态源仍可失败复制且先验额度。新增 resolver 超剩余源额度拒绝及原始错误准确传播回归，pdf 293 项单元及差异检查通过。resolver 自身分配和合并峰值仍未聚合计费，PDF 字典 UseCMap/阅读顺序待推进，P2-01 未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 自定义 CMap resolver：新增 Cmap::try_new_with_resolver，调用方按名称返回基程序，None 回退内置映射；解析器把 resolver 传递给全部子解析，继承条目/源字节/深度及路径循环规则继续共享。新增实际 CycleA→CycleB→CycleA 循环拒绝、本地 Unicode B 覆盖自定义 Base A 及继承代码空间解码回归，pdf 292 项单元测试及差异检查通过。resolver 内部取得 Vec 的分配仍由调用方负责，尚未接入 PDF 字典 UseCMap 或文档级映射登记，P2-01 未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 CMap 循环诊断：解析器维护路径局部继承名称链，重复祖先基名返回明确 FontError cycle，链/名称复制可失败分配；同层重复继承同一基映射仍合法且分别计费。新增注入已在链的 Identity-H 循环拒绝及同层双继承正常回归，pdf 291 项单元测试和差异检查通过。当前 resolver 仍仅内置映射，路径名称/容器峰值未纳入聚合预算，不冒充自定义映射与完整 P2-01 验收，fuzz 暂缓。

- 2026-10-04 P2-01 CMap 继承累计源预算：CmapParser 从当前 max_decoded_bytes 计费根程序及全部内置继承源字节，子解析消耗回传父预算，复制基程序前预检剩余额度；继承深度/映射条目预算继续共用。扩展回归验证根+Identity-H 总源字节精确边界成功、少一字节拒绝，pdf 290 项单元测试及差异检查通过。源字节累计不等于展开映射/容器/合并期间的总峰值预算，循环诊断及完整样本仍待完成，P2-01 未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 Identity-V 统一继承路径：移除前轮专用 H→V 分支，Identity-V 实际解析内置 usecmap/WMode，与其他继承共用预算和深度；扩展回归验证 V 的零深度拒绝、本地 CID 定义放在继承前/后均优先。pdf 290 项单元与公共覆盖/提取6项专项通过，差异检查通过。自定义 resolver/循环诊断和字节峰值累计仍待完成，P2-01 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 预定义 usecmap 继承：解析器递归加载命名的内置基 CMap，继承与本地共用 remaining_entries，按当前 Limits.max_depth 限制链深度；合并 Unicode/CID 默认项、本地项优先及代码空间，加入 WMode0/1 解析。未知基名称继续明确 UnsupportedFeature，取代前轮全部拒绝的临时门控。新增 Identity-H 继承、本地前置覆盖、竖排 WMode、两字节读取、零深度与跨继承累计额度回归；pdf 290 项单元测试通过，差异检查通过。尚未提供用户定义基映射 resolver/循环诊断、字节峰值聚合或全部 CJK 预定义样本验收，P2-01 未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 usecmap 静默不完整结果修正：通用 CmapParser 目前未继承基映射，本轮将打印后继续改为明确 UnsupportedFeature，新增嵌入 /Identity-H usecmap 拒绝回归；标准预定义 Identity-V 仍使用已实现专用映射路径。pdf 289 项单元测试及差异检查通过。此为临时正确性门控而非完整继承交付，后续必须实现本地覆盖优先、代码空间/Unicode/CID 合并、循环/深度与跨继承累计预算；含 usecmap 的其他预定义或嵌入字体当前可能明确拒绝，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 Identity-V 边界/往返：新增预定义 V 映射两字节 code1/code65535、字符数超限、半截码拒绝及 max_objects=1 映射预算拒绝回归；公共竖排提取增加保存严格读回后字符码/Unicode/模式/位置逐项一致。pdf 288 项单元及 overlay_workflow 6 项专项通过，差异检查通过。通用 usecmap、真实字形竖排显示与阅读顺序仍待完成，P2-01 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 Identity-V 缺陷修复：新增真正 Identity-V/DW2(-1000) 字体的公共结构提取专项，首次暴露 bundled Identity-V usecmap 未继承 Identity-H 而无法解码。标准预定义 Identity-V 现在复用受限 Identity-H 映射并设置 WMode1，不冒充通用 usecmap 实现。专项验证 Vertical 模式、y 书写轴、两字符显示空间向下推进12及 Unicode A/B，overlay_workflow 6 项通过，差异检查通过。夹具无字形文件、不证明真实竖排渲染，通用 CMap 继承及阅读顺序仍待完成，P2-01 未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 显示空间书写轴：ExtractedCharacter 新增 writing_axis()，Horizontal 返回矩阵 x 轴、Vertical 返回 y 轴，无书写模式为 None；向量不归一化，零轴保留，不冒充实际推进方向（负宽/间距可反向）。公共四旋转回归核对四个水平书写轴，overlay_workflow 5 项专项及差异检查通过。真实竖排与阅读顺序仍待推进，P2-01 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 多字符/补充平面映射：扩展公共 ToUnicode 回归，C 单字符码映射 UTF-16BE 代理对 emoji 加 fi，提取保持一个事件和完整 "😀fi" 字符串、原字符码 C，连同真实替换字符/缺失映射在保存严格读回后保持。overlay_workflow 5 项专项及差异检查通过；不把多 Unicode 字符误当多个 PDF 定位事件，完整复杂编码/阅读顺序仍待推进，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 缺失映射公共闭环：新增标准字体携带 ToUnicode 夹具（A 显式映射 FFFD，B 缺失），通过文字覆盖解释/提取，验证 Some(U+FFFD) 与 None 区分、未知映射字符仍保留位置推进，保存严格读回后结果一致。overlay_workflow 5 项专项与差异检查通过；不代表复杂 ToUnicode 或阅读顺序完整验收，P2-01 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 简单编码缺失映射：Encoding 新增 unicode_mapping，未知 Differences 字形名/缺失基编码返回 None，零表项作为未定义（保留 PDFDoc code0 特例）；SimpleFont 无 ToUnicode 时改用可选编码映射，不把未知码猜作 U+FFFD/NUL，旧显示 unicode 接口不变。扩展回归核对 WinAnsi A 与未定义 code0；pdf 287 项单元测试及差异检查通过。复杂编码/阅读顺序仍待完成，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 文字运行分组：新增 ExtractedTextRun/extract_text_runs_paint_order，按解释文字对象索引分组并移动字符事件，可失败分配，空文字不产生运行；不把组当行/段落或原始操作跨度。公共工作流验证两组长度5/10、索引递增及展开字符与原提取完全一致，专项4项和差异检查通过；README 补 API/边界。阅读顺序/复杂字体与来源映射仍待完成，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 Unicode/提取接入后工作区回归：cargo test --workspace --locked --quiet 正常退出，pdf 单元 287、集成 85、render 7，共 379 项通过，0 忽略；差异检查通过。README 同步当前字号/BaseFont/书写模式/显示空间坐标、已有 Cmap/可选映射及 extract_text 行为，撤下过时“无字体元数据/未接 ToUnicode”表述。普通回归不代替阅读顺序/复杂字体/峰值预算完整验收，P2-01/P2-06 保持未勾选；未运行 fuzz。

- 2026-10-04 P2-01 可选映射边界：SimpleFont::unicode_mapping 在 ToUnicode 查询前拒绝超出 255 的简单字体字符码，与旧 unicode 契约一致；扩展既有回归验证空映射返回 None 与非法码拒绝。移除 Type0 无 ToUnicode 时标准输出调试信息，避免污染只读提取正文。pdf 287 项单元测试及差异检查通过；复杂映射/阅读顺序仍待推进，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 真实中文提取：新增 extract_text 只读示例，以 Document 严格打开，按绘制对象分隔打印字符 Unicode，缺失映射拒绝而非猜文字，页间分页符保留。实际对既有中文覆盖 PDF 运行成功，与 MuPDF 分页提取逐字一致（仅规范化空行及首尾空白）；验收文档补命令。本轮使用 PDF 技能核对，只读未改 PDF，视觉沿用先前检查；未重跑工作区，阅读顺序/复杂映射/完整元数据仍待推进，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 缺失映射语义修正：源码确认已有 Cmap 支持 bfchar/bfrange 且 Simple/Type0 已接 ToUnicode，并非空 to_unicode.rs 表示未接入；新增 unicode_mapping 可选接口，解释字符事件使用实际映射，Type0 无/缺失映射为 None，显式 U+FFFD 保留 Some，旧 unicode 显示替换行为保留。新增两者区分单元回归；pdf 287 项单元测试通过，差异检查通过。Simple 无 ToUnicode 仍沿用既有编码回退，完整未知映射/真实中文提取与阅读顺序待验证，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 字符额度专项：新增 max_text_chars=1 文档下空页返回空事件、一个字符精确边界成功、两个字符返回 ResourceLimit 的公共回归，提取成功/失败均不推进 revision。overlay_workflow 4 项专项通过，差异检查通过；额度拒绝覆盖解释器路径，不冒充元数据累计字节或整体峰值预算完整验收，P2-01 未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 字体名称输出：ExtractedCharacter 新增可选 font_name，复制解释字体 BaseFont 名称而非字体文件，明确不是资源键或稳定身份；复制字体名与 Unicode 共用累计保留字节限额，名字单项限制 token，字符串分配可失败。公共提取专项验证全部文字为 Helvetica，3 项专项及差异检查通过。字体来源身份、完整 Unicode/阅读顺序及聚合峰值预算仍未完成，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 编辑/回滚快照回归：新增公共提取专项，同一 PdfPage 先提取 A、替换后 B、事务临时 C、失败恢复后 B，原返回快照始终 A，确认提取重新解释最新内容且无临时结果误复用。overlay_workflow 3 项专项通过，差异检查通过；不代表字体身份/完整 Unicode/阅读顺序已完成，P2-01 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-01 字体书写模式：ExtractedCharacter 增加可选 writing_mode，从解释字体取得 Horizontal/Vertical，不由页面矩阵推断；WritingMode 增加 Clone/Copy/Eq 便于快照返回与比较。公共四旋转专项确认页面旋转不改变标准字体 Horizontal 模式，2 项专项及差异检查通过。真实竖排字体/完整字体身份与 Unicode/阅读顺序仍待实现，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 显示空间四旋转专项：公共 overlay_workflow 增加 CropBox(10,20,200,300)、0/90/180/270 旋转上的 AB 字符提取，保存严格读回后核对首字符原点依次(38,252)/(28,152)/(152,28)/(252,38)、矩阵平移一致及后续字符对应轴向推进。专项 2 项通过，差异检查通过；没有新增外部视觉验证，不把页面旋转视为字体竖排支持，P2-01 仍未完成，fuzz 暂缓。

- 2026-10-04 P2-01 坐标契约补验：审查解释器 text_matrix×CTM×page_matrix 合成与页面旋转/裁剪显示变换，明确 position 为显示空间基线原点（左上原点、y 向下），matrix 为文字运行初始矩阵而非各字符推进后的矩阵。公共工作流新增原点 x=48、首字符与运行矩阵平移一致、水平基线方向及后续字符推进断言；专项 1 项通过，差异检查通过。旋转/竖排/完整方向和字符框仍需专项验证，P2-01 不勾选，fuzz 暂缓。

- 2026-10-04 P2-01 字号元数据：ExtractedCharacter 增加 font_size，取解释文字对象的 Tf 声明字号而非矩阵推算显示高度；公共工作流专项逐字符验证 DRAFT 为 24、页码为 12。专项 1 项及差异检查通过，字体身份/方向/完整 Unicode 与坐标契约仍待完善，P2-01 不勾选；fuzz 暂缓。

- 2026-10-04 P2-01 绘制顺序字符基础：新增 ExtractedCharacter 与 PdfPage::extract_characters_paint_order，返回字符码/码长、可选 Unicode、解释器位置/文本矩阵及对象索引，缺失映射保留 None，不猜测阅读顺序。字符数和复制 Unicode 字节限额及可失败分配检查；公共 overlay_workflow 回归逐页提取并核对 DRAFT/逆序页码字符码与单字节码长，专项 1 项通过。尚缺完整 ToUnicode/字体/方向元数据、文本段与阅读顺序及坐标定义收口，P2-01 不勾选；fuzz 暂缓。

- 2026-10-04 P2-06 公共工作流语义加强：overlay_workflow 集成测试新增逐页 DisplayList 字符码核对，确保页 0 为 DRAFT+Page 8 / 2、页 1 为 DRAFT+Page 7 / 2，覆盖逆序选择与非 1 起始编号；核对两页共两个 Helvetica 对象，页内水印与页码复用字体。专项测试 1 项通过，差异检查通过；未重跑工作区或视觉样本，P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 公共 facade 工作流验收：新增 pdf/tests/overlay_workflow.rs，从非泛型 Document 外部 API 添加两页水印与页码，外层事务强制失败后逐对象核对原有六对象恢复；随后成功水印/逆序起始 7 编号，严格保存读回并解释两页。cargo test -p pdf --test overlay_workflow --locked --quiet 1 项通过，差异检查通过；本轮未重跑全部工作区或新增视觉检查，P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 CID 编码分配前额度：水印/页码 CID 路径统一经 encode_overlay_cid，按当前文档额度预检字符数、双字节编码乘法溢出及原始/解码字节上限，再调用字体保留的编码器；避免只依赖编码器自身额度并在生成后才拒绝。pdf 286 项现有单元测试通过，差异检查通过；不冒充聚合内存预算或新限额专项完全验收，P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 旋转/裁剪结构组合：新增 0/90/180/270 页面旋转与非默认 CropBox 上的路径覆盖，验证覆盖前后 Rotate/MediaBox/CropBox/Parent/Resources 不变，保存严格读回及全部 DisplayList 成功。首次全量回归暴露 CID 回滚测试按保存字节比较受字典排序影响，改为比较既有全部 13 个对象状态而非序列化顺序；最终 pdf 286 项单元测试通过，差异检查通过。此次是结构与解释验收，尚未外部渲染旋转样本，不表示自动布局或 P2-06 完整验收；fuzz 暂缓。

- 2026-10-04 P2-06 真实中文覆盖验收：使用既有登记许可的 Noto 静态实例运行 create_cjk --overlay 成功，本库严格读回/两页 DisplayList、qpdf 结构与 JSON 单一 FontFile2 检查通过；MuPDF 提取原文、草稿和对应中文页码正确。按 PDF 技能查看两张 72 DPI 渲染，无遮挡/裁切/缺字。产物 output/pdf/rzpdf-cjk-overlay-example.pdf，docs/overlay-validation.md 补命令、哈希、字体来源与边界。旋转/裁剪组合及复杂内容/聚合预算仍待完成，P2-06 保持未勾选；fuzz 暂缓。

- 2026-10-04 P2-06 中文组合示例：create_cjk 新增可选 --overlay，默认五参数行为保持；开启时预先加入草稿/页码/数字字符集，用同一 CID 字体在外层事务内添加两页中文水印和页码，再沿用保存严格读回/全部 DisplayList。cargo check -p pdf --example create_cjk --locked --quiet 与差异检查通过；本轮未实际运行或视觉检查，真实中文组合验收仍待完成，P2-06 未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 CID 水印快捷入口：新增 watermark_pages_cid，使用已有 RegisteredCidFont 编码同一水印、校验归属、跨页注册和覆盖整体事务化。扩展 SFNT 元数据夹具，验证固定字符集缺字时 revision 不变、两页成功覆盖后仍只有一个 FontFile2 并严格读回；pdf 285 项单元测试通过，差异检查通过。README 记录字符集/归属责任；元数据夹具不证明真实中文字形显示，中文视觉样本仍待完成，P2-06 未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 CID 页码专项结构回归：扩展既有 SFNT 元数据夹具，为数字 1/2 映射同一 glyph 后嵌入固定字符集，验证 number_pages_cid 第二页非法内容失败恢复保存字节、外来字体句柄拒绝且 revision 不变、逆序页选择成功严格读回及只有一个 FontFile2。pdf 285 项单元测试通过，差异检查通过。该夹具不是可渲染字体，本轮未证明中文显示/真实字形或视觉行为，真实中文样本仍待验收；P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 CID 页码组合入口：新增 number_pages_cid，预校验页选择及字体句柄归属，在同一事务内生成标签、使用已有固定 UnicodeCidEncoding 编码、跨页注册同一 RegisteredCidFont 并覆盖；不重复嵌入字体文件，固定字符集缺字时返回错误。pdf 285 项现有单元测试及差异检查通过，但尚无该新入口的专门中文/回滚样本验收，不能把既有测试当作新组合行为完成证明。README 补固定字符集与验收边界，P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 选择校验统一：抽取 validate_overlay_selection，水印/页码/逐页文字覆盖共同使用数量额度、可失败集合分配及唯一有效索引校验，通过 ExactSizeIterator 避免额外选择数组副本；公开 API 和事务边界不变。pdf 285 项单元测试通过，差异检查通过；不将重构视为新增功能或完整阶段验收，P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 字体继承与错误类型回归：新增将字体资源移至 Pages 祖先、叶页移除 Resources 后文字覆盖成功测试，验证实际继承读取；错误名称及 Type XObject 伪字体拒绝时 revision/页字典保持不变。pdf 285 项单元测试通过，差异检查通过；本轮未修改生产逻辑或重跑视觉/全部工作区，P2-06 仍未完成，fuzz 暂缓。

- 2026-10-04 P2-06 文字覆盖资源前置：overlay_page_text 解析最新有效继承 Resources/Font 及字体引用，要求页内名称已注册、字典对象及 Type Font，缺失资源在内容写入前拒绝；新增无字体资源 revision 不变回归。pdf 284 项单元测试通过，差异检查通过；README 明确此检查不等于完整字体/编码验证。未重跑实际视觉样本或全部工作区，P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 可重复样本检查：新增只读 docs/check-overlay-example.sh，qpdf 检查结构，MuPDF 分页提取并逐页精确核对原文/DRAFT/页码（仅忽略空行及首尾空白）。bash 语法和真实产物正向通过，原始无覆盖样本负向返回 1，无参数返回 2；验收文档补命令与不代替几何/颜色/图片视觉检查的边界。未改 Rust、未重跑工作区，P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 实际样本外部验收：运行 overlay_document 对自编两页英文/图片样本添加 DRAFT 和页码成功，写出前本库严格读回/全部 DisplayList 通过；qpdf 11.2.0 结构通过，MuPDF 1.27.0 提取原文及两页对应水印/页码。按 PDF 技能查看两张渲染 PNG，无裁切/重叠，原图片保留。产物 output/pdf/rzpdf-overlay-example.pdf，docs/overlay-validation.md 记录命令、哈希与范围；固定 A4 样本不代替旋转/裁剪/中文组合/预算完整验收，P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 水印/页码组合示例：新增 overlay_document INPUT.pdf OUTPUT.pdf，在同一外层事务为全部页添加斜向 DRAFT 与带总数的页脚，写文件前执行内存保存严格读回及全部 DisplayList。示例包含参数数量、相同路径和页索引转换/选择分配错误处理；cargo check -p pdf --example overlay_document --locked --quiet 与差异检查通过。README 补命令和固定用户空间坐标限制。本轮只验证编译，未运行示例/外部工具或视觉验收，不能据此视为 P2-06 完成；fuzz 暂缓。

- 2026-10-04 P2-06 标准字体水印快捷接口：新增 watermark_pages_standard，选中页使用同一编码后的标签，矩阵控制位置/旋转，逐页字体注册和隔离覆盖整体事务化。新增中文 WinAnsi 拒绝且 revision 不变、后续页非法内容整批恢复资源/页字典、两页 DRAFT 严格保存读回和 DisplayList 回归；pdf 283 项单元测试通过，差异检查通过。README 明确浅色不是透明度、原内容保留及 CJK 编码路径。独立视觉验收、中文组合接口及聚合预算仍待推进，P2-06 保持未勾选；fuzz 暂缓。

- 2026-10-04 P2-06 标准字体页码快捷接口：新增 number_pages_standard，将逐页标签生成/WinAnsi 编码/字体注册/隔离覆盖纳入同一事务，唯一有效页预校验，按传入页序编号，空选择无修改。新增第二页非法内容恢复两页字典（包含资源引用）及逆序编号保存严格读回/DisplayList 回归；pdf 282 项单元测试通过，差异检查通过。README 明确仅可打印 WinAnsi、中文需嵌入 CID 字体、定位不自动适配旋转/CropBox；水印快捷接口、中文组合接口和外部视觉验收仍待推进，P2-06 不勾选，fuzz 暂缓。

- 2026-10-04 P2-06 页码格式：新增 PageNumberFormat，按选中页序生成起始编号/Unicode 前后缀/可选总数，显式区分选择总数与末尾编号，标签需经字体编码后用于批量覆盖。检查序号范围、编号溢出、字节/字符额度及可失败分配；中文标签、无总数、最大编号、溢出与超限回归通过。pdf 281 项单元测试通过，差异检查通过；README 补使用契约。尚未将格式生成、字体注册和定位组合为完整快捷接口，外部视觉验收与聚合预算仍待完成，P2-06 不勾选；fuzz 暂缓。

- 2026-10-04 P2-06 批量定位文字覆盖：新增 overlay_pages_text，接受逐页 TextOverlay，预校验唯一/有效页索引并限制选择数量，整批在同一事务内按指定顺序覆盖。空集合无修改；后续页非法原内容会恢复此前页字典/内容，成功逆序选择可严格保存读回并解释全部页面。新增选择非法 revision 不变、第二页失败回滚及成功读回回归；pdf 280 项单元测试通过，差异检查通过。README 记录逐页字体/字符码和外层字体注册事务责任；聚合快照预算、格式模板、视觉验收仍待实施，P2-06 不勾选，fuzz 暂缓。

- 2026-10-04 覆盖接口后工作区回归：cargo test --workspace --locked --quiet 正常退出，pdf 单元 279、集成 81、render 7，共 367 项通过，0 忽略；git diff --check 通过。当前新增覆盖/定位文字接口与既有解析、编辑、保存及渲染回归兼容；测试通过不等于外部视觉验收或水印/批量页码完整交付，P2-06 保持未勾选。未执行 fuzz，既有编译警告未冒充清零。

- 2026-10-04 P2-06 定位文字覆盖：公开 TextOverlay 和 overlay_page_text，接受已注册字体名称、字体字符码、字号、文本矩阵及 RGB，检查有限几何/正字号/颜色范围后复用隔离覆盖路径。可用于文字水印和页码，不把 UTF-8 当字体码；字号非法无修改、正文码保存严格读回和 DisplayList 回归通过。pdf 279 项单元测试通过；README 明确注册/编码责任及无自动布局、透明度、批量格式模板边界。P2-06 仍待完整快捷功能与视觉验收，不勾选；fuzz 暂缓。

- 2026-10-04 P2-06 覆盖边界验收：增加空覆盖包装本身的五操作/十二字节精确限额与超限拒绝回归，拒绝时页字典和 revision 不变；增加原内容尾部注释及外层事务失败恢复页字典/内容回归。覆盖模块 3 项及 pdf 全部 278 项单元测试通过；水印/页码快捷接口和外部视觉验收仍未完成，P2-06 保持未勾选，fuzz 暂缓。

- 2026-10-04 P2-06 覆盖内容基础：新增 overlay_page_content，以 q/Q 包裹原内容和新增内容，清除遗留路径，隔离原 CTM/裁剪/颜色；修改前检查作用域配对、未执行裁剪和合并字节/操作/深度额度。通过页级事务化内容替换保持共享原流的其他页不变。新增变换隔离、共享页保持、严格保存读回及非法状态拒绝且 revision 不变回归；pdf 277 项单元测试通过。README 记录资源需单独注册、未知操作不透明、内联图片暂拒绝和非安全涂黑边界。水印/页码快捷接口、聚合峰值预算与外部视觉验收仍待推进，P2-06 不勾选；本轮未运行 fuzz 或全部工作区集成测试。

- 2026-10-04 P2-05 合并示例最高版本：merge_documents 预先打开全部输入，沿用同一上下文取最高有效版本，再显式创建目标后合并，取消默认 1.7 导致 PDF2.0 输入拒绝的问题。提取 merge_sources 公共示例内部流程供测试，新增 1.4+2.0 混合合并、输出2.0头/两页严格读回及源 revisions 不变回归；cargo check -p pdf --example merge_documents --locked 与示例测试 1 项通过，差异检查通过。本轮未重跑工作区或真实 PDF2.0 高级特性/视觉样本；README/合并验证文档更新当前行为。保留输入上下文不等于文件并发修改快照，全部输入/容器内存仍未聚合计费，完整页面关系未完成，不勾选相关项；fuzz 暂缓。

- 2026-10-04 AR-04/P2-05 显式创建版本与选页保持：PdfDocument/Document 新增 new_with_version/new_with_version_and_options，源空上下文按类型化 PdfVersion 初始化，旧默认 1.7 与上下文 wrapper 保持兼容，不构造假 Reader、不把版本当符合性配置。extract_pages 按源有效版本新建（包含 PDF2.0，Catalog 升版转为输出头版本），保留 ParseOptions。新增 1.0–2.0 全部枚举 legacy/owned 创建/保存严格读回、默认兼容、1.4/2.0 选页保持、Catalog2.0 升版及保存拒绝降至1.7回归。pdf 275 项单元测试通过，差异检查通过；未重跑全部集成或真实 PDF2.0 高级特性样本。README/合并验证文档撤回旧当前限制表述，历史记录保留；合并目标版本门控、完整特性扫描及页面关系仍待完善，不勾选相关项，fuzz 暂缓。

- 2026-10-04 P2-05 可复现外部验收：新增只读 docs/check-merge-extract-example.sh，以 qpdf JSON 检查固定样本合并/选页的页数、FontFile2 描述引用数和图片数，MuPDF 提取逐字核对正文及 3,0 选页顺序（仅规范化空行/分页符）。bash 语法、真实产物正向、交换输入负向及无参数返回 2 检查全部通过；验证文档加命令并明确数量检查不代替语义/视觉。未改 Rust、未重跑工作区测试，差异检查通过；通用合并验收及复杂页面关系仍待推进，不勾选相关项，fuzz 暂缓。

- 2026-10-04 P2-05 合并/选页闭环：新增 merge_document（批量原子追加全部源页）及 extract_pages（按唯一选择返回新 PDF 1.7，保留源 ParseOptions），增加 merge_documents/extract_pages 示例，保存后严格读回并构建全部 DisplayList。新增合并页序/旋转、选页顺序/独立身份/源选项/源 revision、重复拒绝与严格读回测试；pdf 273 项单元测试通过。实际合并既有中文两页与英文图片两页，再选 3 0 输出两页；qpdf 11.2.0 两产物结构通过，JSON 核对合并 4 页/1 FontFile2/2 Image，MuPDF 1.27.0 提取页序正确，按 PDF 技能检查 4+2 张渲染无缺字/重叠/裁切。产物 output/pdf/rzpdf-merged-example.pdf 与 rzpdf-extracted-example.pdf；README/docs/merge-extract-validation.md 记录命令、哈希、许可来源和边界。多源整体事务由调用方包裹，Info/XMP/书签命名目标/注释结构关系及非空 PDF2.0 选页仍不支持，P2-04/P2-05 不勾选；本轮未重跑全部集成，差异检查通过，fuzz 暂缓。

- 2026-10-04 P2 批量页面导入：新增 import_pages_from(source,&[indices],insertion_index)，按不重复选中页序导入，预分配目标页并登记全部源页映射，再共享 memo/节点/载荷预算复制可达图，保留同批共享字体/图片/内容及选中页之间循环引用。整批同一事务插入树/更新 Count；后续页失败恢复之前页、根 Kids/Count 和下一编号。合法空集合无写入，重复/越界与同文档批量明确拒绝（同文档仍用 copy_page）；单页跨文档接口委托批量实现，避免双份继承/预算逻辑。新增逆序选择/共享字体/选中页循环/严格读回/源不变、未选中页引用拒绝、非法选择无写入、第二页流额度失败整批回滚回归。最终 pdf 272 项单元测试通过，差异检查通过；本轮未重跑全部集成（上一轮工作区 358 项通过）。README 补范围，注释/标记结构/书签命名目标/同文档批量仍待推进，相关项不勾选；fuzz 暂缓。

- 2026-10-04 单页导入完整回归补记：首轮工作区回归暴露测试误把不同文档的裸对象号要求为不相等；源/目标允许相同数值编号，正确隔离证据是文档归属句柄。修正为比较带身份的内容句柄，保留 DisplayList/严格读回及源不变检查。最终 cargo test --workspace --locked --quiet 全部 358 项通过（pdf 单元 270、集成 81、render 7，0 忽略），差异检查通过。近期字体与页面复制/重排/旋转/裁剪/资源导入改动已有工作区回归；不等于注释、复杂结构和所有真实页面导入验收。

- 2026-10-04 P2 跨文档单页导入：新增 import_page_from(source,source_index,insertion_index)，返回目标页句柄，同文档委托 copy_page。固化源页继承 Resources/MediaBox/CropBox/Rotate，剥离源 Parent/空 Annots，预先登记源页→目标页 memo 后用同一对象图重映射内容和资源，事务内完成新页及目标树/祖先 Count；源文档不变。拒绝注释/标记结构/文章归属、加密源、较低目标版本和资源图牵引其他页面/文档树，未冒充完整关系导入。新增带 Helvetica 内容/旋转页的目标插入、原内容引用独立、DisplayList、本库严格保存读回、目标内容替换后源 revision/页字典不变、流额度失败恢复空页树/下一编号、归属/非法索引无写入回归。pdf 270 项单元测试通过，完整工作区回归正在运行；README 补接口及许可/克隆峰值/书签标签边界。页面关系完整导入仍待推进，相关项不勾选；fuzz 暂缓。

- 补充上述资源导入验证：最终资源专项 2 项通过（266 项过滤），加密源拒绝分支本轮仅编译覆盖，尚无专门加密源样本验收；差异检查通过。

- 2026-10-04 P2 跨文档资源图前置：新增 import_resource_from(source,handle,Font/Image)，校验源句柄/根类型与有效版本，事务内 placeholder+memo 重映射可达引用、保持同次共享/循环，克隆字典/数组并保留原编码流；返回目标归属句柄供 register_handle 绑定。拒绝加密源、较低目标版本、外部文件流、资源图 Page/Pages/Catalog（Type 引用同样解析）；按目标节点/深度、token、单流及累计键/标量/流载荷检查，source 克隆与容器峰值仍未整体计费。新增共享循环/原始 ASCIIHex 流/源 revision 不变/页绑定及严格读回、图节点额度失败回滚下一编号、错误源句柄/类型无修改；268 项 pdf 单元测试通过，追加原始流/token 限额回滚及加密源前置拒绝后资源专项重跑。README 补同次去重、外层事务和非完整页面导入边界。完整页面/注释/结构导入仍待实施，相关项不勾选；fuzz 暂缓。

- 2026-10-04 P2-03 同文档页面复制：新增 copy_page(source_index,insertion_index)，按插入前 0 起始页序解释，允许末尾追加；返回独立新页 ID，共享原内容/资源，固化继承 Resources/MediaBox/CropBox/Rotate，缺省 CropBox=原 MediaBox、Rotate=0，事务插入树并更新祖先 Count。对非空/非法 Annots、StructParents、文章 B 明确拒绝，空注释在副本省略，未冒充注释/标记结构复制。新增前插/ID独立/内容共享及页级替换后原页不变、继承裁剪、严格保存读回、非法索引与未支持归属无写入、外层回滚恢复页及下一对象号回归。最终 pdf 266 项单元测试通过，差异检查通过；本轮未重跑全部集成。README 补共享低层修改及回滚 ID 生命周期边界。注释/标记结构完整复制与跨文档导入仍待实施，P2-03 不勾选，fuzz 暂缓。

- 2026-10-04 P2-03 页面旋转/裁剪：新增 set_page_rotation（绝对顺时针、90 倍数、归一化）及 set_page_crop_box（未旋转用户空间坐标）；解析有效继承 MediaBox，复用 PageBuilder 有限正面积/相交验证。None 在叶页写入当时 MediaBox 覆盖继承裁剪，不修改父节点、兄弟页、内容或注释。修改事务化，非法参数/页面在写前拒绝。新增立即可见、负角度、非法/非有限/无交集、外层事务回滚、严格保存读回、继承裁剪重置及父/兄弟不变回归。pdf 263 项单元测试全部通过，差异检查通过；未重跑全部集成。README 说明 None 快照语义与不变换其他对象的边界。页面复制/跨文档导入及完整 P2-03 验收仍待推进，不勾选；fuzz 暂缓。

- 2026-10-04 P2-03 页面重排：新增 reorder_pages(&[u32])，要求当前 0 起始索引完整排列，重复/缺失/越界在修改前拒绝，恒等排列不改 revision。保留页 ID、既有树槽位和 Count；跨父节点在任何写入前固化 Resources/MediaBox/CropBox/Rotate，缺省 CropBox 固定原 MediaBox、Rotate=0/Resources 空字典，缺 MediaBox 拒绝，防止新父节点改变外观。事务内替换槽位并更新 Parent，不删除共享对象。新增完整排列/非法无修改/严格保存读回、嵌套父节点资源/旋转/缺省裁剪保持回归；pdf 全部 261 项单元测试通过，追加外层事务回滚比对页 ID/字典后专项 2 项通过（259 项过滤），差异检查通过。README 补接口/书签页序与标签结构树不自动调整边界；页面复制/跨文档导入及完整 P2-03 验收仍待推进，不勾选；未运行 fuzz。

- 2026-10-04 P1-10 中文示例可复现检查：新增只读 docs/check-cjk-example.sh，qpdf JSON 检查两页/同字体引用、Type0/Identity-H、CIDFontType2、FontDescriptor/FontFile2、字宽、CIDToGIDMap/ToUnicode 流及中文 Info 标题；MuPDF 提取逐字检查四行与顺序，仅规范化空行/分页符。bash 语法检查、真实中英文产物正向通过；原英文示例负向拒绝、无参数返回 2 均通过。README/中文验证文档加入命令与仅适用于该直接资源样本的边界；本轮未改 Rust、未重跑工作区测试，差异检查通过。脚本不替代视觉/字体轮廓/通用解析验收，P1-10 不勾选；fuzz 暂缓。

- 2026-10-04 P1-05/P1-10 真实中英文构建验收推进：新增 create_cjk 参数化示例，整字体嵌入、两页复用、CID 编码、中文标题，保存后本库严格读回并构建两页 DisplayList。下载带 OFL 的 Noto Sans SC 至独立临时目录，FontTools 4.39.4 固定 wght=400 准备静态输入（不代表库支持可变实例）；登记许可/哈希/实际名称/CapHeight，显式 StemV=80 仅是冒烟近似，自动准确指标仍待补。真实运行发现 cmap 14 长度误读与辅助 6 表导致整字体拒绝，按格式读取长度、校验最小头/范围/累计预算后跳过未实现表，保留有效 0/4/12，无可用表仍拒绝；新增共存/截断回归。pdf 259 项单元测试通过，create_cjk 实际运行成功；qpdf 11.2.0 结构通过，MuPDF 1.27.0 四行中英文提取符合预期，两页 PNG 检查可读无缺字/重叠/裁切。产物 output/pdf/rzpdf-cjk-example.pdf；新增 docs/cjk-creation-validation.md、README 入口。未重跑全部集成；该字体/简单横排样本不证明全部字体/复杂排版/PDF/A/P2 提取，P1-05/P1-10 保持未勾选，fuzz 暂缓。

- 2026-10-04 P1-03/P1-05 嵌入字体跨页复用：RegisteredCidFont 保留文档归属 ObjectHandle，register_on_page 绑定同一字体对象并返回页局部资源名；编码器私有、encoding() 只读，避免替换映射破坏字体编码一致性。回归验证第二页已占 F1 时返回 F2、重复注册同名、跨文档拒绝且目标 revision 不变、非法页拒绝、两页严格读回及保存中 FontFile2/CIDFontType2 各仅一个。pdf 全部 258 项单元测试通过，追加无重复字段断言后专项 1 项通过（257 项过滤），差异检查通过；未重跑全部集成。字体验证文档记录外层事务/低层修改边界。自动去重、跨文档导入和真实中文显示/提取仍未完成，P1-03/P1-05 保持未勾选，fuzz 暂缓。

- 2026-10-04 P1-05 整字体资源注册：新增 ResourceManager::embed_truetype_font、TrueTypeEmbeddingOptions 和 RegisteredCidFont，原字体字节写入 FontFile2，创建 FontDescriptor、CIDFontType2、Type0/Identity-H、CIDToGIDMap、W 与 ToUnicode，六对象和页绑定事务化。要求 PDF≥1.3、可编辑嵌入权限、水平/post 指标与固定字符集合，拒绝 CFF/可变字体、缺字及非法描述参数；PostScript 名称/CapHeight/StemV/serif 显式提供，不猜测。扩展自编 SFNT 样本验证资源名、编码、保存字段、严格读回、缺字/受限许可不变和中途对象额度失败后对象/下一编号恢复；小额度保存另受序列化节点预算限制，因此回滚验收直接比对对象而非使用该保存入口。cargo check -p pdf --locked 及最终 pdf 258 项单元测试通过，差异检查通过；未重跑全部集成。docs/font-cmap-validation.md 更新入口及边界；轮廓完整验证、真实中文显示/提取、复用/子集/自动指标仍待推进，P1-05 不勾选，fuzz 暂缓。

- 2026-10-04 P1-05 post 描述指标前置：新增 PostMetrics 公共头读取与 TrueTypeProgram 可选挂接，支持版本 1/2/2.5/3，有符号 16.16 italicAngle 保留分数、下划线位置/厚度与非零固定宽标志；缺表不伪造值，未知版本及截断拒绝。新增负分数/固定宽/全部头截断/版本回归，扩展 SFNT 样本验证读取。最终 pdf 全部 258 项单元测试通过，差异检查通过；本轮未重跑全部集成。docs/font-cmap-validation.md 补官方规范及公共头≠完整 post/FontDescriptor 的边界。完整字体资源注册及中文显示/提取仍待推进，P1-05 不勾选，fuzz 暂缓。

- 2026-10-04 P1-05 CID 与 ToUnicode 编码前置：新增 UnicodeCidEncoding，从 Unicode cmap/maxp/水平指标创建固定集合；排序分配两字节 CID、保留 0、重复字符复用、同 GID 不同字符仍区分 CID。提供受限 encode_text、CIDToGIDMap、1000 单位 widths 和 ToUnicode；输出 UTF-16BE 代理对，无 BOM，每组 bfchar 最多 100 项，缺字及集合外字符明确拒绝。新增重复/同 GID/中文/补充平面、分组及预算回归，并扩展 SFNT 样本验证 cmap→GID→字宽实际接入。补表后修正缺 glyf 测试目录位置，最终字体 46 项及 pdf 全部 257 项单元测试通过，差异检查通过；未在本轮重跑全部集成测试。docs/font-cmap-validation.md 补规范与范围；尚未完成 PDF 字体资源注册、FontDescriptor、许可入口及真实中文显示/提取，P1-05 未勾选，fuzz 暂缓。

- 2026-10-04 P1-05 字形范围前置：新增零拷贝 GlyphLocations，按 loca 短/长格式检查 numGlyphs+1 项、非递减和 glyf 边界，空字形允许、越界 GID 拒绝。TrueTypeProgram::checked_glyph_locations 挂接同源 maxp/head/loca/glyf，拒绝缺表、非法格式及 OTTO/CFF；不把范围校验冒充轮廓和 hinting 验证。新增两项范围回归，并扩展 SFNT 指标样本验证入口、空字形及缺表/CFF 拒绝；最终 cargo test -p pdf --lib --locked --quiet font::truetype 共 25 项通过（230 项过滤），差异检查通过。docs/font-cmap-validation.md 补官方规范与边界。下一步继续真实字体及 CID/ToUnicode 嵌入闭环；P1-05 保持未勾选，fuzz 暂缓。

- 2026-10-04 P1-05 水平指标前置：新增 head/hhea/hmtx 指标读取、unitsPerEm 与包围盒/ascent/descent，检查表长度、版本/魔数、指标数量及 maxp；支持末尾字形复用最后 advance、GID 越界拒绝和 1000 单位分数缩放。TrueTypeProgram 挂接可选指标，缺表不伪造值。新增指标与 SFNT 自编样本回归；补充断言发现旧 TimesNewRoman 测试实际读取 Type 1 PFB，修正为签名拒绝，撤回真实 TrueType 验收表述。按 OpenType 文件规范检查 SFNT 签名；这不是轮廓格式完整支持。工作区 341 项通过（pdf 单元 253、集成 81、render 7，0 忽略），差异检查通过。docs/font-cmap-validation.md 更新来源和范围；下一步仍需真实字体校验、FontDescriptor/CID 字宽与 ToUnicode、中文显示/提取闭环，P1-05 不勾选。未实现或运行 fuzz。

- 2026-10-04 P1-05 嵌入标志前置：新增 OS/2 FontEmbeddingRights，读取版本和 fsType 公共前缀，按官方规范处理旧版组合权限/未定义高位、现代互斥及保留位，保留原始标志且不修改源字体。validate_outline_embedding 明确检查受限、仅位图、禁止子集和预览打印用于可编辑文档；TrueTypeProgram 读取标志并提供验证入口，缺 OS/2 时保留旧读取但新嵌入验证拒绝，不推断许可。新增权限/限制、版本差异、截断与保留位回归。cargo check -p pdf --locked 与工作区 337 项通过（pdf 单元 249、集成 81、render 7），随后 SFNT 挂接及读取受限/拒绝嵌入/缺表测试单独 1 项通过（249 项过滤），差异检查通过。字体验证文档补官方来源和技术标志≠许可证的边界；完整嵌入入口及只读生命周期未实现，P1-05 不勾选。未运行 fuzz。

- 2026-10-04 P1-05 maxp 与构建用 Unicode 查询：TrueTypeProgram 读取 maxp 0.5/1.0 的 numGlyphs，检查版本、最小表长度和非零数量；旧缺表简化读取兼容。新增 unicode_glyph_id(char)，要求 maxp 与 Unicode cmap，按完整 Unicode/BMP 优先级查找，请求字符缺字返回 None，GID 越界拒绝且不截断。该入口为后续嵌入提供受检查的映射，不等于整个 cmap/轮廓验证。新增版本/长度/零数量、补充平面合法与越界、缺字/缺表回归。cargo check -p pdf --locked 与工作区 335 项全部通过（pdf 单元 247、集成 81、render 7，0 忽略），差异检查通过。docs/font-cmap-validation.md 补官方 maxp 来源和边界；许可、轮廓/字宽、CID/ToUnicode 与独立显示验收未完成，P1-05 未勾选。未运行 fuzz。

- 2026-10-04 P1-05 字体映射前置：按 Microsoft OpenType cmap 官方规范新增 Format 12，读取 32 位长度及分组、排序/重叠/范围/GID 溢出检查，二分查找避免展开范围；TCmap 新增 get_gid_u32，旧 u16 查询兼容且不截断。TrueTypeProgram 累计预算使用 Format 12 的 32 位长度，Unicode 选择优先完整表并保持符号优先，Unicode 能力检查覆盖 3/10 与平台 0。新增 BMP/补充平面、范围外、全部截断位置、倒序/重叠、计数/GID 溢出、空表及完整 SFNT+cmap 读取/选择回归。cargo test --workspace --locked --quiet 全部通过（pdf 单元 245、集成 81、render 7，共 333，0 忽略），差异检查通过。README 和 docs/font-cmap-validation.md 补范围及规范来源。尚未嵌入中文字体、检查许可/maxp、生成 CIDToGIDMap/ToUnicode 或独立显示/提取，P1-05 不勾选；未运行 fuzz。

- 2026-10-04 AR-04 显式流状态：PdfStream 增加 RawEncoded/Decoded 状态与 from_decoded_data；原始构造按字典编码，解码构造/替换清理编码参数并更新 Length，新建/追加/替换页面内容及 Metadata 统一接入。状态私有转换、克隆保留来源；保存读回为 RawEncoded，内容相等比较仍为字典＋字节。无过滤器及 Decoded 输出采用可失败预分配，检查解码额度。补直接页面创建/追加/替换的单流原始与解码额度检查，失败在事务前拒绝且状态标记不变。工作区 329 项通过（pdf 单元 241、集成 81、render 7），随后状态/克隆/读回语义/解码限额单元 1 项通过（241 项过滤），差异检查通过。README/架构文档更新；版本特性扫描与完整保存验收仍待实施，AR-04 保持未勾选；未运行 fuzz。

- 2026-10-04 AR-04 保存编码分类：FlateUnencoded 输出准备按最新上下文解析 Filter 连续引用，间接 null 视为未编码并压缩；真实非空 Filter 保留原字节及引用。未编码候选的间接 null DecodeParms 在输出副本清理，非空参数仍拒绝压缩；外部流与 Preserve 原策略不变。新增 PdfStream::into_raw_parts 以消费方式拆分字典/原始载荷，避免此步骤额外克隆载荷。回归验证 null 参数连续引用、最新 Filter 改为 ASCIIHex 后编码保留、内存对象不变、参数冲突/引用循环拒绝且 Writer 原内容不变。cargo check -p pdf --locked 及工作区 328 项通过（pdf 单元 241、集成 80、render 7），补拒绝断言后 stream_edit 7 项重跑通过，差异检查通过。完整流状态与版本特性验收仍待推进，AR-04 未勾选；未运行 fuzz。

- 2026-10-04 AR-02 引用解析与契约验收：流解码补 Filter/DecodeParms 顶层及数组元素的间接引用、连续引用与常用解码参数间接标量；引用链限深，数组检查对象额度，Filter=null 按无过滤器处理，JBIG2Globals 保持既有处理器但支持连续引用。扩展源对象流回归覆盖 Filter 名字/数组、参数连续引用、间接 Predictor、修改/删除，并验证退出源视图后普通解码仍受最新编辑影响；新增 ASCIIHex/null 参数、Filter 最新名及引用循环回归。cargo check -p pdf --locked 与工作区 327 项通过（pdf 单元 241、集成 79、render 7，0 忽略），差异检查通过。按原 AR-02 逐项核对提交修订、失效标记、重复页面/字体解释、嵌套错误与 unwind、源缓存隔离及低层修改覆盖，勾选该契约工作包；不同时勾选完整 P0-11，不表示缓存层优化、聚合预算、JBIG2 等算法完成。审计文档同步。未运行 fuzz。

- 2026-10-04 AR-02 对象流源视图隔离修复：确定性回归先复现失败，首次懒加载 Flate 对象流时，间接 DecodeParms 原 Predictor=1 被修改为 99，污染源展开并触发 FilterError。对象流展开解码改用限深源解析作用域，依赖解析忽略 Store 修改/删除；RAII 保证错误/unwind 恢复正常最新视图。回归验证源压缩对象仍为 11、普通参数读取仍看到 99 或删除错误；新增作用域错误/unwind 恢复测试。工作区 324 项通过（pdf 单元 238、集成 79、render 7），随后新增作用域恢复测试单独 1 项通过（238 项过滤），差异检查通过。缓存审计文档补复现与边界；多层/数组参数和其他解码器交互仍待验收，AR-02 未勾选。未运行 fuzz。

- 2026-10-04 AR-02 缓存生命周期审计：逐项检查源对象 OnceCell、对象流展开、页面尺寸、解释器/字体/图像状态、DisplayList 和全局静态表，新增 docs/cache-lifecycle.md 区分源缓存、派生结果与双修订标记。新增已打开文档先加载 Helvetica，再低层改为 Courier、事务临时恢复 Helvetica、错误回滚后恢复 Courier、旧 DisplayList 不变及严格保存读回回归；page_revision 4 项与工作区全部通过（pdf 单元 237、集成 79、render 7，共 323），差异检查通过。下一缺口明确为对象流首次懒加载前间接解码依赖修改与原始快照隔离，需要专项复现，不能用普通 Store 优先级证明；AR-02 继续未勾选。未运行 fuzz。

- 2026-10-04 AR-02 提交修订语义：DocumentState 新增 commit_revision 与事务 dirty 状态；事务外成功低层写入各计一次，嵌套写入仅最外层成功且保留修改时计一次。回滚/unwind/只读不计提交，内层回滚恢复外层 dirty；提交额度编辑前检查。缓存仍使用 revision/DocumentStamp，不把提交计数用于临时状态缓存；新建初始化写入也计数，调用方捕获基线比较。新增一次外层提交、内层与外层回滚、内层全部丢弃后外层只读、低层写入、计数耗尽及 unwind 断言。cargo test --workspace --locked --quiet 全部通过（pdf 单元 237、集成 78、render 7，共 322），随后补 unwind 提交不变断言并重跑 revision 6 项通过，差异检查通过。架构文档补双修订语义；派生缓存接入审计与完整 AR-02 验收仍待推进，未勾选。未运行 fuzz。

- 2026-10-04 AR-01/P1-01/A1 验收：新增非泛型 Document facade，内部擦除拥有所有权 Reader，new/open/from_bytes/from_reader 及 mode/options 返回统一类型；旧 PdfDocument<R> 保留，借用 Reader 可继续使用旧入口。提取通用 create_with_options，共用无源初始化且不构造 Reader；事务闭包接收 &Document。新建、严格读回、内容解释、低层服务委托、嵌套回滚、创建额度及文件读取后编辑保存均有验收。cargo check -p pdf --locked 通过；工作区 318 项通过（pdf 单元 236、集成 75、render 7），随后补文件路径并重跑 owned_document 4 项通过，差异检查通过。README/架构文档说明 'static 和兼容边界。逐项检查 DocumentState 的 Store/Trailer/版本/身份、统一状态服务、无源初始化及迁移说明，满足原 AR-01/P1-01/A1 退出条件，勾选这三项；不表示全部底层泛型、派生缓存、操作级预算或 P1 全阶段完成。未运行 fuzz。

- 2026-10-04 AR-01/02 独立事务服务迁移：将快照、回滚守卫、嵌套 savepoint 及修订号容量预留从 PDFContext 移至无 Reader 泛型的 DocumentState，原事务入口保持委托兼容；不改变错误/unwind 的恢复语义。新增直接构造状态、不创建 Reader/Parser 的独立事务测试，验证 Store/Trailer/分配号恢复、身份与源版本稳定、回滚修订号推进。工作区原 314 项测试通过，随后新增状态测试单独运行 1 项通过（其余 235 项单元过滤），差异检查通过。源对象存在性/计数仍结合读取侧处理，快照预算、公共泛型隐藏与完整架构验收未完成；未运行 fuzz。

- 2026-10-04 AR-01 DocumentState 第一阶段迁移：新增不含 Reader/Parser 的内部 document/state.rs，统一持有 ObjectStore、Trailer、源 Header 版本、文档身份和修订/事务深度；新建与源读取共用构造服务，PDFContext 的状态读写和回滚均转向该结构，PdfDocument 不再另存身份与源版本。打开时传入实际 Header，新建默认 1.7；最新 Catalog 的有效版本策略保持原逻辑。保留公开入口签名，xref、懒加载源对象缓存及 Parser 留在读取侧。cargo test --workspace --locked --quiet 全部通过（pdf 单元 235、集成 72、render 7，共 314，0 忽略），差异检查通过。架构文档更新实际分层与迁移边界；公共 Reader 泛型隐藏、状态服务进一步分离及完整 AR-01 验收仍待实施，任务不勾选。未运行 fuzz。

- 2026-10-04 AR-02 页面重复解释验收：新增 DocumentStamp（文档身份＋修订号）、stamp/is_stamp_current，供派生缓存统一判断状态，不复制或冻结对象。确认当前页面无 DisplayList 复用缓存，每次重新读取/解释；新增同一 PdfPage 跨内容修改与错误回滚、低层尺寸修改与 unwind 回滚的回归，验证临时内容/尺寸不会在恢复后被沿用，回滚前后标记失效且保存不失效。不同文档相同修订号不匹配，Hash/Eq 回归通过。cargo test --workspace --locked --quiet 全部通过（pdf 单元 235、集成 72、render 7，共 314，0 忽略），差异检查通过。架构文档区分源对象缓存与派生缓存；统一状态、提交修订语义和缓存层接入仍待实施，AR-01/02 保持未勾选。未运行 fuzz。

- 2026-10-04 AR-01/05 文档身份迁移：新增 DocumentIdentity 和 ObjectHandle，进程内身份保活且不序列化；重开同一文件产生独立身份。resolve_handle/update_handle/delete_handle/register_handle 在编辑前拒绝外来句柄，验证跨文档同号对象不会被误读、改写、删除或注册，拒绝时修订号不变；本地句柄跟随更新，删除后解析失败。兼容裸 ObjectId 入口仍要求调用方保证归属；事务新分配地址不可在提交前外泄，尚未提供稳定对象生命周期令牌。工作区测试全部通过（pdf 单元 235、集成 68、render 7，共 310），随后补同文档字体注册复用并重跑 identity 集成 4 项全部通过，差异检查通过。架构文档补迁移边界；统一 DocumentState、缓存接入和对象图复制未完成，AR-01/05 不勾选。未运行 fuzz。

- 2026-10-04 AR-02 修订号契约：新增 PdfDocument::revision 文档内单调失效标记，低层对象插入/更新/删除、Trailer 替换成功后推进；读取/保存不推进。事务中写入即时可见，错误及 unwind 回滚再推进，避免恢复旧号导致临时缓存误命中；嵌套 savepoint 预留回滚号段，耗尽时拒绝修改且保证恢复。修订号不是提交次数或跨文档身份。新增低层成功/失败、读取、空成功事务、嵌套双回滚、unwind、耗尽及页面修改/保存回归。工作区测试通过（pdf 单元 235、集成 64、render 7，共 306），随后新增页面验收并重跑 revision 集成 4 项全部通过，差异检查通过。统一 DocumentState、文档身份与派生缓存接入仍待实施，AR-01/02 不勾选；未运行 fuzz。

- 2026-10-04 AR-04 流替换往返补验：补齐原始/解码字节精确边界、超限与非流目标拒绝后对象不变、空字节替换、重新 Flate 保存不改内存对象、严格读回后再次替换的集成测试。修正 FlateUnencoded 对外部文件流的处理：遇 F/FFilter/FDecodeParms 保留字典和内联字节，不单独重写编码契约，并逐项验证保存与替换拒绝行为。架构文档补公开入口与共享引用边界。cargo test --workspace --locked --quiet 全部通过（pdf 单元 234、集成 61、render 7，共 302，0 忽略），git diff --check 通过。未执行 fuzz；完整 AR-04/P1-08 仍待后续验收。

- 2026-10-04 AR-04 流替换接口：新增 PdfStream::with_decoded_data 与 PdfDocument::replace_stream_decoded_data，替换解码字节时清理 Filter/DecodeParms/DL、重建精确 Length，保留其他属性；文档入口检查原始/解码字节限制。外部文件流明确拒绝且原对象保持不变。该低层接口更新所有共享引用，页面局部编辑继续走已有内容替换入口；不会自动修正图片尺寸等语义属性。完整流状态架构和阶段验收仍未完成。

- 2026-10-04 用户调整与 AR-04 版本收口：暂缓 fuzz 实现/运行/CI，保留既有代码和记录；普通 CI 移除 fuzz 编译/回放，手动 fuzz 任务置为禁用。近期主线改为保存/流状态、文档状态/事务、中文字体、常用页面编辑。writer 复用最新 effective_version，默认保留 Header/Catalog，显式输出版本不得低于有效版本，Flate 最低版本计入 Catalog 覆盖；非法类型/未知 Catalog 版本在写出前拒绝。新增间接版本对象更新、默认/显式保存严格读回、拒绝时 Writer 原字节不变及低 Header/Catalog 升级压缩回归。cargo test --workspace --locked --quiet 全部通过（pdf 单元 234、集成 55、render 7，0 忽略），两个 CI YAML 语法和差异检查通过。完整流状态、对象特性扫描、聚合预算及各阶段验收仍未完成，AR-04/P1-08 保持未勾选。

- 2026-10-04 content 资源路径插桩验收补录：原资源版复跑正常退出，固定 seed 784110385、25968 次/61 秒、cov3320/ft8314、最终 RSS522 MiB，未观测崩溃或 sanitizer 错误；fuzz/artifacts 无失败输入，锁文件哈希保持一致。正向三绘制对象测试及共享回放共两项通过，全目标 locked 编译与差异检查通过。docs/fuzz-smoke-validation.md 已补最终结果，前条“正在进行”为当时状态。仍不代表持续 fuzz、全部输入/字体/解码器或原生后端安全验收。

- 2026-10-04 P0-12 实际插桩冒烟：cargo-fuzz 0.13.2 项目内安装已成功，四目标各 61 秒 AddressSanitizer 覆盖率引导运行正常退出，syntax 固定种子复跑 514193、document 52385、content 初轮 79152、filters 734141 次，无观测崩溃；PDF 核心/harness fingerprint 确认 sancov 与 address 参数，锁文件哈希不变。自编完整 PDF 种子通过 qpdf 11.2.0 及 MuPDF 1.27.0 两页文本/PNG 查看；Poppler 缺依赖未改系统。新增 docs/fuzz-smoke-validation.md 记录环境、计数和边界。发现 content 初轮 F1 无资源会提前返回，已预注册字体/透明图片并增加正向三对象/无诊断回归，两项 harness 测试通过；修正工作目录后资源版 60 秒复跑正在进行。长时/真实样本、更多目标、原生插桩和远端 CI 仍未验收，P0-12 不勾选。

- 2026-10-03 fuzz CI 配置与本地 runner 获取：普通 CI 新增 fuzz 全目标编译和共享 harness 测试；新增手动触发的四目标 fuzz.yml，固定 cargo-fuzz 0.13.2，准备自编种子，显式 AddressSanitizer，每目标 60 秒/单输入 5 秒/进程 RSS 1024 MiB，上传演化语料与失败输入。核对 0.13.2 源码确认 run 不支持 --locked，改为前置 locked 编译及运行后锁文件差异检查。两个工作流 YAML 语法及差异检查通过，未触发远端任务。cargo install 正在将 runner 安装到 fuzz/target/tools（未改全局工具），原进程仍存活；本轮回放测试等待包缓存锁。尚无 runner 安装成功、实际覆盖率引导或 sanitizer 结果，P0-01/12 保持未验收。

- 2026-10-03 fuzz 入口编译与正向种子回放：原依赖获取/编译及测试进程均已完成，fuzz/Cargo.lock 锁定独立依赖；cargo check --manifest-path fuzz/Cargo.toml --locked --all-targets 通过，cargo test --manifest-path fuzz/Cargo.toml --locked --lib 的共享 harness 测试通过。新增两页文本/路径/RGBA 自编文档种子函数及拒绝覆盖的生成器，正向测试要求严格/兼容读取每页三个绘制对象；初次发现 64 KiB 解码/256 对象额度拒绝内置 AFM，已改为 256 KiB/1024，使文字解释路径实际可达，输入仍限 64 KiB。正常工作区测试全部通过（pdf 单元 231、集成 55、render 7），差异检查通过。前条待验证记录为当时状态；本轮未运行覆盖率引导或 sanitizer，不将编译/回放视为 P0-12 完成。

- 2026-10-03 P0-12 覆盖率引导入口待验证：新增独立 fuzz 工作区，libfuzzer-sys 锁定 0.4.13，四个 target 覆盖语法、完整文档、内容解析/解释及 Flate/LZW/ASCIIHex/ASCII85/RunLength；限制输入/输出/对象/深度，并保留成功解析推进与解码输出额度断言。新增共享 harness 回放测试、项目自编种子、来源说明和失败输入最小化命令。本机 nightly 可用、cargo-fuzz 未安装；cargo check --manifest-path fuzz/Cargo.toml --bins 正在获取依赖，harness 测试和工作区测试已启动等待缓存锁，均尚无最终结果。差异检查通过，但不能据此宣称编译、插桩、覆盖率引导运行或 P0-12 验收完成；需继续等待原进程并记录结果。

- 2026-10-03 P0 字体目录限额与 CI 超时：Sfnt 新增带 Limits 入口，在目录分配前检查字体字节和表记录数量，HashMap 使用可失败预分配；TrueTypeProgram 传递文档 Limits，不再只限制后续 cmap。新增精确字节/记录额度、少一额度、截断头超大声明及调用链超限回归。cargo test --workspace --locked --quiet 全部通过（pdf 单元 231、集成 55、render 7，0 忽略）。CI 增加 60 分钟任务超时，远端尚未运行。源码显式 panic/未实现宏搜索当前仅命中测试，但不证明隐式索引、分配及原生依赖均安全；P0-01/08/10/12 仍未整体验收。

- 2026-10-03 SMask 采样位深与保存往返：同尺寸 DeviceGray SMask 新增 1/2/4/16 位解析，复用逐行打包样本和原始精度 Decode 转换；非法位深返回 ObjectError，不匹配尺寸仍显式拒绝。补各位深 alpha 单元期望值及两行图像集成回归，验证反转 Decode、行填充、严格保存读回的逐像素 RGBA 和原始蒙版字节保留。cargo test --workspace --locked --quiet 全部通过（pdf 单元 230、集成 55、render 7，0 忽略），差异检查通过。Matte、不同尺寸重采样、嵌套蒙版、独立阅读器新增位深对照及聚合预算仍未完成，P4 不勾选。

- 2026-10-03 PDF 图像采样位深扩展：DeviceGray/RGB/CMYK 新增 1/2/4/16 位样本转换，逐行处理填充位，16 位使用大端；Decode 在原始样本精度上计算后量化，避免二次量化误差。检查打包输入长度、尺寸算术和输出额度。新增行填充、各位深期望字节、16 位反转/精度顺序、长度及额度错误回归；图像入口接受合法位深和对应长度。cargo test --workspace --locked --quiet 全部通过（pdf 单元 230、集成 54、render 7，0 忽略），差异检查通过。其他颜色空间、SMask 位深、PNG 16 位导入、独立像素验收及聚合预算未完成，P4 保持未勾选。

- 2026-10-03 图像/SMask Decode 线性映射：8 位 DeviceGray/RGB/CMYK 图像及同尺寸灰度 SMask 支持显式 Decode，解析间接数组/成员、检查分量数量与有限数值，按线性映射及设备范围裁剪后生成 8 位样本；SMask ImageMask false 接受，true/非法类型拒绝，Matte/嵌套蒙版仍拒绝。新增默认/反转/越界端点裁剪/固定值期望字节与错误类型/数量回归，蒙版 Decode [1 0] + ImageMask false 得到预期 alpha127。工作区 pdf 单元 229、集成 54、render 7 全部通过，0 忽略；差异检查通过。其他颜色空间、精度/位深、独立 Decode 像素验收及聚合预算仍待完成，P4 不勾选。
- 2026-10-03 PNG RGBA 独立像素闭环：create_transparency 示例增加 --png 参数，内存生成 RGBA PNG 后经 embed_png_image 导入并输出 Flate PDF；实际运行生成 output/pdf/rzpdf-png-transparency-example.pdf，qpdf 11.2.0 检查通过，MuPDF 1.27.0 渲染/PPM 三处中心像素检查通过（误差≤1），PNG 查看正常。工作区 pdf 单元 228、集成 54、render 7 全部通过，0 忽略；差异检查通过。README/透明验证记录补复现命令与结果；其他 PNG 变体、完整色彩管理及聚合预算仍待完善，P1-06 与整体目标未完成。
- 2026-10-03 PNG 预算与精度拒绝专项：新增原始 PNG 字节精确额度/少一拒绝、像素额度不足、RGBA 字节不足及第二对象额度失败回归，验证资源字典和下一对象号恢复；新增真实 16 位灰度 PNG 返回 UnsupportedFeature，不做静默降精度。工作区 pdf 单元 228、集成 54、render 7 全部通过，0 忽略；差异检查通过。解码器内部额度与输出共存峰值仍未聚合计费，16 位/ICC/交错/APNG 及外部像素专项仍待推进，P1-06 不勾选。
- 2026-10-03 PNG 首轮验证完成：原依赖获取进程已正常退出成功，Cargo.lock 锁定 png 0.18.1 及传递依赖；原工作区 --locked 测试继续运行并完成，pdf 单元 228、集成 52、render 7 全部通过，0 忽略。四种 PNG 像素类型、palette+tRNS、截断/坏 CRC/无效页号回归通过；rustfmt 与差异检查通过。前两条“待验证”记录为当时状态，不再是当前编译/测试状态；本轮仍未验证全部位深/交错/解码预算边界、外部像素或颜色管理，P1-06 不勾选。
- 2026-10-03 PNG 待验证补充：新增调色板+tRNS 展开为 RGB/SMask、IHDR CRC 损坏拒绝和无效页号不占对象号回归源码；无效页号前置到解码前检查。rustfmt 与差异检查通过。重新轮询确认原 cargo check/test 进程均存活，Cargo 仍获取依赖，未生成 png 锁定条目；crates.io 索引独立 HTTP 检查可访问，尚无编译错误或测试结果。本条不能作为 PNG 编译/运行验收依据，继续等待现有进程，不重新宣称通过。
- 2026-10-03 P1-06 PNG 导入实现待验证：新增 png 0.18.1 依赖与 embed_png_image，配置解码内部限额，在读取元数据/像素前检查尺寸和 RGBA 大小，EXPAND 后复用 RGB/RGBA 路径；灰度/灰度 alpha 转换，APNG/16 位/ICC 暂拒绝。新增四种像素类型保存读回及截断输入回归源码。目前 cargo check 正在获取依赖，cargo test --workspace --locked 等待中，锁文件尚未更新；本条不表示编译或测试已通过，不能据此验收 PNG 或勾选 P1-06。解码器内部限额为 best effort，输出/转换/事务等聚合峰值仍待统一计费。
- 2026-10-03 JPEG 解释/保存/预算回归：RGB/Gray JPEG 集成回归增加 DrawImage 内容解释与严格保存读回，启用 FlateUnencoded 后仍保持原始 JPEG/DCTDecode 字节；新增压缩字节精确通过/少一拒绝、RGBA 额度少一、像素额度不足、无效页号及 XObject 类别畸形导致注册失败回归，失败恢复页面与下一对象号。工作区 pdf 单元 228、集成 48、render 7 全部通过，0 忽略；差异检查通过。本轮验证本库解释与回滚，不代表完整 JPEG 数据验证、独立像素/文件格式导入验收或 CMYK/ICC/EXIF 支持，P1-06 不勾选。
- 2026-10-03 P1-06 JPEG 原样嵌入：新增 embed_jpeg_image，扫描标记拒绝不支持 SOF/非 8 位精度，再用 mozjpeg 读取尺寸/颜色头部，支持 Gray/RGB/YCbCr；检查 raw 字节、像素与 RGBA 解码额度，事务创建 DCTDecode Image 并注册页面。新增内存生成 RGB/Gray JPEG 原字节、颜色空间/Filter 保存严格读回一致及空/截断/非法长度/12 位输入失败不占对象号回归。工作区 pdf 单元 228、集成 46、render 7 全部通过，0 忽略；差异检查通过。仅头部验证，不保证熵编码完整，CMYK/YCCK、EXIF/ICC、完整解码/显示/外部验收和预算专项仍待完成，P1-06 不勾选。
- 2026-10-03 RGBA/SMask 独立像素闭环：新增 create_transparency 示例与只读固定 PPM 检查脚本，生成 300×100 PDF、Flate RGB/SMask；qpdf 11.2.0 检查通过，MuPDF 1.27.0 渲染 PNG/PPM 并核对 alpha 0/128/255 三处白底中心像素，各通道与理论值差≤1，PNG 查看正常。脚本正常输入通过、错误格式退出 1；工作区 pdf 单元 228、集成 44、render 7 全部通过，0 忽略，差异检查通过。README/独立验证文档记录实际颜色、命令及限制；不覆盖边缘/重采样/Matte/复杂透明组，P1-06/P4 与整体目标仍未完成。
- 2026-10-03 Skia SMask 像素回归：新增从零构建 30×10 点页面、嵌入三个直通 RGBA 样本（alpha 0/128/255），保存前及严格读回后都经过页面 Interpreter/Device/Skia 绘制，读取 RGBA8888 表面，核对白底合成期望像素（半透明 [227,147,137,255]，每通道容差 1）。工作区 pdf 单元 228、集成 44、render 7 全部通过，0 忽略；差异检查通过。该回归覆盖本库/Skia 路径而非独立阅读器、所有位置/混合模式或完整软蒙版兼容性，P1-06/P4 仍待完成。
- 2026-10-03 SMask 解析异常专项：新增直接构造图像/蒙版的回归，验证有效半透明样本的直通 RGBA 期望字节；Width/Height/BitsPerComponent/Subtype/ColorSpace 缺失返回 ObjectError，尺寸不匹配、非整数高度、16 位、错误颜色/类型、嵌套 SMask、Matte/Decode 和空/多余 alpha 字节均拒绝。工作区 pdf 单元 228、集成 44、render 6 全部通过，0 忽略；差异检查通过。此为解析与合成回归，不是循环对象图/全部过滤器/独立像素对照验收；完整软蒙版兼容性、预算与整体计划仍未完成。
- 2026-10-03 P4/P1-06 SMask 解释基础：PdfImage 增加可选 alpha 缓冲，解析 Image/DeviceGray、同尺寸、8 位 SMask，检查精确整数/数据长度并复用受限流解码；rgba_buffer 保持直通 RGB 并应用 alpha。不同尺寸/位深、Matte/Decode/嵌套蒙版等显式拒绝。Skia 输入格式由 Premul 修正为 Unpremul；扩展 RGBA 单元回归半透明/错误 alpha 长度，嵌入 RGBA 集成回归保存前后内容解释通过，原布尔 SMask 拒绝测试更新为格式错误。工作区 pdf 单元 227、集成 44、render 6 全部通过，0 忽略；差异检查通过。仍缺独立透明像素和 Skia 颜色对照、异常蒙版专项及聚合峰值预算，P4/P1-06 不勾选。
- 2026-10-03 RGBA 边界与错误语义：零宽/高统一 RGB 接口返回 ObjectError；新增输入少/多字节、无效页号、极大尺寸、精确四字节/一像素额度及 raw/decoded/pixel 任一不足、1.3 Header 版本拒绝回归，验证失败保持页面资源及下一对象号。工作区 pdf 单元 227、集成 44、render 6 全部通过，0 忽略；差异检查通过。此轮仅加固写入与失败状态，尚未证明透明像素外部显示、本库 SMask 渲染或全局分配预算，P1-06/P4 保持未完成。
- 2026-10-03 P1-06 RGBA 原始像素嵌入：新增 embed_rgba_image，直通（非预乘）RGBA 分离为 DeviceRGB 图像和 8 位 DeviceGray SMask；检查尺寸、像素/字节预算、精确长度及有效版本≥1.4，可失败分配通道缓冲；蒙版、RGB 对象、页资源注册与 SMask 更新在同一事务。新增零透明/半透明原始通道保存严格读回及第二对象额度失败恢复资源/下一对象号回归。工作区 pdf 单元 227、集成 42、render 6 全部通过，0 忽略；差异检查通过。当前本库 PdfImage 对 SMask 仍显式拒绝，外部像素验证、格式导入、维度/版本专项和聚合峰值预算待补齐，P1-06/P4 不勾选。
- 2026-10-03 创建示例外部元数据验收脚本：新增 docs/check-creation-example.sh，qpdf JSON 动态定位 Trailer/Info/Catalog/Metadata，核对中文标题、作者、Creator、类型和页数；MuPDF 解压 Metadata、xmllint（libxml 2.9.13）检查 XML 良构与按命名空间定位作者值，独立提取两页文本。实际示例检查通过，bash 语法检查及差异检查通过，无参数退出码 2；README/验证记录同步。Rust 源码未改，本轮未重跑工作区测试（上一轮 273 项通过）。脚本仅验证示例固定期望，不证明全部 XMP/PDF 符合性，不提供不可信外部工具沙箱；中文正文字体及整体计划仍未完成。
- 2026-10-03 P1-10 可运行创建示例与独立检查：新增 create_document 示例，构建两页 A4、WinAnsi 英文文字、两像素 RGB 图像、边框、中文 Info 与原始 Metadata packet，Flate 原子保存。实际生成 output/pdf/rzpdf-creation-example.pdf；qpdf 11.2.0 检查无语法/流编码错误，MuPDF 1.27.0 提取两页预期文字并渲染，逐页 PNG 检查显示正常。系统 Poppler 缺 libtiff.5.dylib，未修改系统依赖。工作区 pdf 单元 227、集成 40、render 6 全部通过，0 忽略；差异检查通过。README 与 docs/creation-example-validation.md 记录命令、结果及限制；中文正文字体、外部 Info/XMP 专项及完整 P1 仍未完成，P1-10 不勾选。
- 2026-10-03 P1-07 一致性策略与成对提交：新增 set_info_and_metadata，外层事务内同时替换/解除 Info 与 Metadata，第二步字节预算或对象分配失败恢复此前 Info/Trailer/Catalog 和对象编号。确定默认独立编辑不覆盖另一侧，显式成对提交保证原子性但不保证调用方两份数据语义一致；未知 XMP 不自动重写。新增第二步字节超限/对象数超限回滚、保存严格读回、下一对象号恢复和成对移除回归。工作区 pdf 单元 227、集成 40、render 6 全部通过，0 忽略；差异检查通过。XML/XMP 语义模型/自动字段映射、日期字段、独立工具验收和聚合预算仍未完成，P1-07 不勾选。
- 2026-10-03 Metadata 共享/异常/版本专项：新增共享旧流对象及另一引用字典在替换后不变、旧流保存读回与解除 Metadata 后仍保留回归；补 Type/Subtype 缺失/null/错误名称、非流 Metadata 返回 DocumentError；PDF 1.3 拒绝创建 Metadata 且下一对象号不被消耗，max_objects=2 分配失败不修改 Catalog、无 Metadata 移除成功。工作区 pdf 单元 227、集成 38、render 6 全部通过，0 忽略；差异检查通过。本轮增加验收证据，不代表 XML/XMP 字段同步、全部间接属性分支或独立工具检查完成，P1-07 保持未勾选。
- 2026-10-03 P1-07 文档 Metadata 流接口：新增 metadata_packet/set_metadata_packet，读取 Catalog Metadata、校验流及 Type Metadata/Subtype XML 后按现有过滤器解码；写入原始 packet 先检查 raw/decoded 字节限额及有效版本≥1.4，创建新流并事务更新 Catalog，移除仅解除引用。Info 与 packet 相互独立，不自动覆盖/同步；不解析 XML、不展开实体、不验证 XMP 语义。新增中文/非 BMP packet 经 Flate 保存严格读回、Info 保持、跨 Info/Metadata 错误回滚、移除保存读回和精确字节/超限状态不变回归。工作区 pdf 单元 227、集成 35、render 6 全部通过，0 忽略；差异检查通过。共享对象/错误类型/版本分支专项、XML/XMP 字段模型与同步、独立工具及聚合预算仍待完成，P1-07 不勾选。
- 2026-10-03 有效版本与 Info UTF-8 校验：新增 effective_version，读取最新 Catalog /Version（含间接值），与 Header 取较高版本，缺失/null 保留 Header，错误类型及未知版本显式错误；info_text 的 UTF-8 BOM 校验改用有效版本。新增 1.7 Header + 2.0 Catalog 允许中文 UTF-8、无升级拒绝、保存严格读回、编辑立即可见、低版本/null 不降级和非法值回归。工作区 pdf 单元 227、集成 33、render 6 全部通过，0 忽略；差异检查通过。SaveOptions 仍保持现有 Header 策略，符合性、未知版本恢复诊断及 Info/XMP 完整验收仍未完成。
- 2026-10-03 P1-07 标准 Info 文本字段：新增 InfoTextField（Title/Author/Subject/Keywords/Creator/Producer）与 info_text/set_info_text；按文档 Limits 编码/解码，解析间接字段并检查类型，设置/移除保留其他及未知字段，错误走事务回滚；删除不存在字段不分配对象。UTF-8 BOM 字段保守要求源 Header 为 2.0，Catalog /Version 的有效版本判断仍待统一。新增六字段中文/非 BMP、保留未知条目及旧对象、严格保存读回、单字段移除与对象额度失败/字段类型错误状态保持回归。工作区 pdf 单元 227、集成 32、render 6 全部通过，0 忽略；差异检查通过。日期/Trapped、XMP 同步、版本专项及独立工具验收仍未完成，P1-07 不勾选。
- 2026-10-03 P1-07 Info 字典与 Trailer 事务：PDFContext Trailer 改为 RefCell，事务同时快照/提交/错误及 unwind 回滚 Trailer 与对象存储，嵌套事务继续作为保存点；低层 trailer() 返回 Ref 借用守卫。新增 info_dict/set_info_dict/remove_info_dict，读取最新字典，替换分配新间接对象避免改动共享旧字典，移除仅解除引用；检查字典条目限额。新增中文/非 BMP 与自定义字段保存严格读回、删除读回、外层错误/panic 回滚及下一对象号恢复回归。工作区 pdf 单元 227、集成 30、render 6 全部通过，0 忽略。当前为低层 Info 字典 API，不校验日期/全部字段、不同步 XMP；事务快照克隆峰值与独立工具验收仍未完成，P1-07 不勾选。
- 2026-10-03 P1-07 PDF 文本字符串基础：PdfString 新增 from_text/with_limits，使用带 BOM 的 UTF-16BE 编码，支持中文、非 BMP 及 NUL；新增 text_content/with_limits，区分 PDFDocEncoding、UTF-16BE 与 PDF 2.0 UTF-8 BOM，拒绝奇数字节、非法代理项/UTF-8 和未定义编码条目。编码分配前检查字符/字节额度，解码逐字符检查并可失败扩容；旧 get_content 保持原 UTF-8 语义。新增独立期望字节、三种编码、异常和编码/解码不同长度边界回归；工作区 pdf 单元 227、集成 28、render 6 全部通过，0 忽略。当前仅为元数据基础，不是字体字符码编码；Info/XMP API、Trailer 事务回滚及 UTF-8 BOM 的文档版本校验尚未实现，P1-07 不勾选。
- 2026-10-03 AFM 借用解析与临时分配收口：parse_line 使用 SplitAsciiWhitespace 借用迭代器，移除每行收集和复制 Vec；新增 parse_afm_str_with_limits，原 String API 委托该入口。内置字体加载直接校验并借用静态 AFM UTF-8，不再复制整份输入，仍先检查文档字节限额。既有十四种标准字体、异常输入、预算边界和文档字体加载回归全部通过；工作区 pdf 单元 225、集成 28、render 6，0 忽略，差异检查通过。未做性能/峰值测量；名称持久化及复制、缓存和跨字体累计预算仍未统一计费，P0-10 不勾选。
- 2026-10-03 内置 AFM 接入文档 Limits：SimpleFont 加载内置度量前检查文档 max_decoded_bytes，使用 try_reserve_exact 复制输入；解析传入文档 Limits，不再使用默认配置。新增 Courier 字节与 315 条目精确额度通过、任一额度少一拒绝且不安装 AFM 缓存回归。工作区 pdf 单元 225、集成 28、render 6 全部通过，0 忽略；差异检查通过。max_objects 在 AFM 内作为度量条目上限，尚非跨字体累计计费；逐行 token/名称分配、缓存及其他输入路径仍待统一预算，P0-08/P0-10 不勾选。
- 2026-10-03 AFM 限额及标准字体兼容验证：新增 parse_afm_with_limits，在解析前检查输入字节，累计各 StartCharMetrics 声明数量并限制 max_objects（重复名称也计费），逐项 HashMap 可失败扩容；旧入口委托默认 Limits。新增精确字节/条目额度、少一字节、超大声明及重复段累计超限回归，验证全部十四种标准字体 AFM 可解析。工作区 pdf 单元 224、集成 28、render 6 全部通过，0 忽略。当前内置字体调用仍使用默认 AFM 限额，尚未接入文档操作级统一计费；名称/行 token 分配和聚合峰值未完成，P0-08/P0-10 不勾选。
- 2026-10-03 P0-08 AFM 解析安全补充：移除字符码、字宽、名称、边界框、字体名称和条目数量解析的 unwrap；非法/缺失字段与非有限度量返回 FontError，声明条目数多于输入时 EOF 返回错误，修复死循环。保留内置 AFM 条目间空行兼容；新增十二类异常输入回归。工作区 pdf 单元 222、集成 28、render 6 全部通过，0 忽略。当前 PDF 调用链主要解析内置 AFM；本项不代表所有 PDF 输入路径安全，AFM 聚合分配预算和全模块 panic 审计仍待完成，P0-08 不勾选。
- 2026-10-03 保存单输出缓冲验证：当前 writer 将间接对象、xref、Trailer 和结尾直接追加到同一个有界输出缓冲，不再建立完整对象块/结尾块后复制。新增三页文件逐项核对经典 xref 对象偏移、对象号/代次头与 startxref 指向，并严格读回；工作区 pdf 单元 221、集成 28、render 6 全部通过，0 忽略。该检查独立核对输出字节位置，但不是外部阅读器验收；未测量峰值内存，源对象/字典克隆与压缩器内部开销仍待统一预算，P0-10/P1-08 不勾选。
- 2026-10-03 Trailer/结尾逐项预算补充：生产 Trailer 通过有界字典 serializer 写出，前缀/换行与 startxref/偏移/EOF 也逐项检查保存剩余额度后追加；不再先生成完整超限结尾块。新增 Trailer 精确长度一致、少一字节拒绝缓冲不越界及千字节字符串在百字节额度下不写入 token 回归。工作区 pdf 单元 221、集成 27、render 6 全部通过，0 忽略；差异检查通过。README 同步当前保护范围；Trailer 原始字段克隆、字典键复制及多缓冲共存峰值仍未统一计费，P0-10/P1-08 与整体目标未完成。
- 2026-10-03 叶子 token 预算预检查：抽取共享名称/字符串准确转义长度函数，写出与限额校验使用同一计算；逐项对象 serializer 在分配叶子 token 前检查剩余额度，数字、布尔、null 与引用也计算输出长度。新增所有 256 字节字符串及特殊字符/中文名称精确输出一致、少一字节额度拒绝且旧输出不变回归。工作区 pdf 单元 220、集成 27、render 6 全部通过，0 忽略；差异检查通过。字典键 PdfName 先复制、对象图克隆、Trailer 和多缓冲峰值仍待完善，P0-10/P1-08 与整体目标未完成。
- 2026-10-03 间接对象逐项序列化预算：生产 writer 对间接对象头/尾、数组分隔和条目、字典分隔和成员、流字典与原始字节逐项按剩余最终额度检查后可失败追加；流数据复制前即拒绝超限，不再先构建完整超大对象块。流 Length 使用 try_insert 更新。新增嵌套数组超限缓冲不越界、精确对象输出一致及千字节流在百字节额度下未复制原始字节回归。工作区 pdf 单元 219、集成 27、render 6 全部通过，0 忽略；cargo check 和差异检查通过。名称/字符串叶子临时 token、对象/流字典克隆、Trailer 与共存缓冲峰值仍待统一预算，P0-10/P1-08 及整体目标未完成。
- 2026-10-03 xref 临时输出预算补充：xref 写出改为 PdfResult，每行追加前按最终输出剩余额度检查并可失败扩容，避免整个 xref 块构建后才拒绝。经典条目先校验十位偏移范围、代次≤65535 与 n/f 状态，非法条目在前缀写出前拒绝。新增精确块长度通过、少一字节拒绝且缓冲未超限及三类非法条目不改变旧输出回归。工作区 pdf 单元 218、集成 27、render 6 全部通过，0 忽略；差异检查通过。对象/Trailer 块临时分配、稀疏 xref 优化和全局峰值预算仍待完善，P0-10/P1-08 与完整目标未完成。
- 2026-10-03 P1-08 最终输出字节限额：SaveOptions 新增 max_output_bytes（默认 512 MiB），writer 按头、间接对象块和 xref/Trailer/结尾块安全累计检查后可失败扩容追加，保持 xref 位置正确。新增最终长度精确通过严格读回、零/头边界/少一字节拒绝且 Writer 旧内容不变、文件预算失败保留旧文件且无临时文件回归。工作区 pdf 单元 217、集成 27、render 6 全部通过，0 忽略；差异检查通过。单块序列化仍先构建临时 Vec，源克隆、xref 块及所有缓冲共存峰值不在此限额内；逐字节输出预算/全局内存保护和完整验收仍待推进，P0-10/P1-08 与整体目标未完成。
- 2026-10-03 压缩缓冲预算补充：Flate 输出改用有界可失败分配 sink，每次追加前安全检查 max_raw_stream_bytes，容量失败和字节超限映射 ResourceLimit；finish 阶段输出也受限。未编码输入在压缩前检查原始/解码字节限制，生产 writer 传入文档 Limits。新增空流压缩精确输出/少一字节 finish 超限、输入超限及 sink 失败无部分追加回归。工作区 pdf 单元 217、集成 25、render 6 全部通过，0 忽略；cargo check 和差异检查通过。源对象克隆、压缩器内部/native 开销、多缓冲共存峰值和最终 PDF 总输出预算仍未完成，P0-10/P1-08 与整体目标未完成。
- 2026-10-03 P1-08 Flate 输出策略：SaveOptions 新增 Preserve/FlateUnencoded，未编码间接流在输出副本上压缩并更新 Filter，writer 重新生成 Length；已有过滤器保持编码字节，未过滤却带非空 DecodeParms 时拒绝，空参数删除。转换前后均校验对象结构预算；Flate 输出要求头版本至少 1.2，旧版本必须显式升级。新增像素解压一致性、原对象不变、重复保存不双重压缩、已编码/参数边界与低版本升级回归。最终工作区 pdf 单元 215、集成 25、render 6 全部通过，0 忽略；差异检查通过。压缩缓冲和输出总预算未接入，非间接嵌套流、独立阅读器/像素验证及高级策略仍待完善，P1-08 和整体目标未完成。
- 2026-10-03 P1-08 SaveOptions 版本策略：新增 PdfVersion/SaveOptions 和 bytes/writer/file 三类 with_options 入口，默认保留源头版本，显式同版本/升级允许，未知源版本覆盖与降级在写出前返回 UnsupportedFeature；Catalog /Version 保留不修改。新增默认 1.7、升级 2.0 严格读回、降级拒绝且输出/文档源版本不变回归。工作区 pdf 单元 214、集成 23、render 6 全部通过，0 忽略；差异检查通过。更改头版本不代表 PDF 2.0 符合性；压缩、加密、增量策略与输出总预算仍待实现，P1-08 和整体目标未完成。
- 2026-10-03 P1-08 原子保存故障验证：抽取内部可注入写入动作的原子文件 helper，测试临时文件写入 partial 后返回 StorageFull，原目标字节保留且临时文件清理；新增 macOS/Unix 权限 0640 替换后保留回归。错误路径统一先关闭文件再传播错误，避免清理仍打开文件的顺序问题。最终工作区 pdf 单元 214、集成 22、render 6 全部通过，0 忽略；差异检查通过。仅注入 write 阶段故障，不代表 sync/rename/断电及 Windows 已验收；SaveOptions、输出总预算和完整目标仍待实施，P1-08 未勾选。
- 2026-10-03 P1-08 文件原子替换初版：save_to_file 先完整序列化，再同目录 create_new 独占临时文件、写完/保留普通文件权限/sync_all/关闭后 rename 替换；失败 guard 尝试删除临时文件，成功重命名后禁用清理，拒绝 symlink/目录目标。新增创建再替换严格读回、序列化失败保留旧文件及符号链接目标保护回归；修复并发测试目录时间戳碰撞，增加进程内序号。最终工作区 pdf 单元 213、集成 21、render 6 全部通过，0 忽略；差异检查通过。仅 macOS 验收，未做磁盘故障注入/权限专项/跨平台替换测试，不承诺断电目录持久性、ACL/所有权或并发协调；SaveOptions 与输出总预算仍待完善，P1-08 和整体计划未完成。
- 2026-10-03 P1-08 Writer 输出入口：新增 save_to_writer，完整序列化成功后 write_all 到调用方输出，不主动 flush/close；I/O 错误返回 PdfError::IOError，并明确调用方可能保留部分输出。新增严格读回、每次三字节短写成功、七字节后 BrokenPipe 返回及文档状态不变回归。工作区 pdf 单元 213、集成 18、render 6 全部通过，0 忽略；差异检查通过。当前仍先构造完整 Vec，不是流式有界输出；SaveOptions、原子文件替换与输出总预算尚未完成，P1-08 与整体目标未完成。
- 2026-10-03 标准字体编码预算补充：新增 encode_text_with_limits，按 Unicode 字符数量（等于 WinAnsi 输出字节）在分配前检查 max_text_chars/max_decoded_bytes；默认 encode_text 委托默认 Limits。新增多字节 UTF-8 é/欧元在两字符两字节额度精确通过、任一额度少一拒绝、零额度空文本通过与非空拒绝回归。工作区 pdf 单元 213、集成 16、render 6 全部通过，0 忽略；差异检查通过。预算是单次编码输出，不含输入字符串/后续十六进制展开/文档聚合峰值；字体嵌入、中文 CID/ToUnicode 与整体计划仍未完成。
- 2026-10-03 P1-05 标准字体编码基础：StandardFont::encode_text 复用现有 PDF WinAnsi 映射，将可表示 Unicode 转为字符码，拒绝控制字符、中文/非 BMP 与无精确映射字符，不做替换或归一化；输出可失败分配。新增独立 Café/欧元/破折号/Œ 期望字节、六类拒绝及全表可表示字符编码解码一致性回归。工作区 pdf 单元 213、集成 15、render 6 全部通过，0 忽略；差异检查通过。当前仅支持标准字体 WinAnsi，编码输出显式大小预算、字体嵌入、中文 CID/ToUnicode 和独立阅读器提取验证仍待完善，P1-05 与整体目标未完成。
- 2026-10-03 P1-03/05 标准字体创建入口：新增 StandardFont 的 Helvetica/TimesRoman/Courier 与 register_standard_font，生成未嵌入 Type1 + WinAnsiEncoding 字典，当前页面继承资源范围内按完整字典相等复用并确定性选择名称；插入与注册同一事务。新增三种字体重复注册复用、Hello PDF 已编码文字构建、严格保存读回再次复用及文本内容解释回归。工作区 pdf 单元 213、集成 14、render 6 全部通过，0 忽略；差异检查通过。此入口不嵌入字体、不实现 Unicode 编码或中文，不承诺跨页自动去重或 PDF/A；中文 CID、ToUnicode 和独立文本/像素验证仍待完成，P1-03/05 与整体目标未完成。
- 2026-10-03 P1-04 字体选择安全入口：新增 try_set_font/try_set_font_with_limits，检查有限字号和合法非空名称，写入前按名称转义规则计算 token/完整 Tf 长度、检查预算；复用 writer 转义并构造完整片段后提交，失败不追加内容。新增特殊字体名+已编码文字构建、非法名称/字号、十字节输出精确额度与不足预算回归。工作区 pdf 单元 213、集成 13、render 6 全部通过，0 忽略；收紧测试额度后专项复测通过，差异检查通过。旧 set_font 仍为不转义兼容入口，实际字体存在性、嵌入与 Unicode 编码仍待实现，P1-04/05 及整体计划未完成。
- 2026-10-03 P1-04/05 字体字符码输出基础：新增 show_encoded_text/show_encoded_text_with_limits，将调用方按字体编码的字节输出为十六进制 Tj，保留二进制与多字节码；安全计算展开长度、预检查 token/累计内容预算并可失败扩容，失败不修改内容。新增全部 256 字节解析一致性及六字节 token/十字节内容精确边界回归。工作区 pdf 单元 212、集成 13、render 6 全部通过，0 忽略；差异检查通过。README/源码明确旧 show_text 仅原样 UTF-8，不保证 Unicode 显示；字体编码器、嵌入、中文 CID 和 ToUnicode 尚未实现，P1-04/05 与整体目标未完成。
- 2026-10-03 P1-04 裁剪构建：新增 clip/clip_even_odd/end_path，完成校验要求 W/W* 紧接路径结束操作，拒绝未提交裁剪或中间插入其他指令；允许空路径裁剪及绘制时提交裁剪。新增四类错误顺序/三类合法结束回归，以及从零构建裁剪贝塞尔路径、严格保存读回和内容解释端到端回归。工作区 pdf 单元 210、集成 13、render 6 全部通过，0 忽略；差异检查通过。当前为内容结构/解释验证，未完成独立像素对照、全部状态语义和高层文字编码；P1-04 与整体目标保持未完成。
- 2026-10-03 P1-04 路径与变换构建：新增 move_to/line_to/curve_to/close_path/transform/set_line_width/fill_even_odd/fill_and_stroke 链式接口；完成校验跟踪当前路径点，拒绝没有起点的线/曲线/闭合及绘制或 n 清空后继续路径。新增完整变换/线宽/贝塞尔/闭合填充描边输出与五类非法路径回归。工作区 pdf 单元 209、集成 12、render 6 全部通过，0 忽略；差异检查通过。公共方法仍延迟到 try_build 检查参数，复杂裁剪/文字状态、字体编码、资源与独立绘图验收待补齐，P1-04 与整体目标未完成。
- 2026-10-03 P1-04 数值范围补充：构建完成校验拒绝负线宽、低于 1 的 miter limit、越界 flatness，J/j/Tr 要求精确整数并限制枚举范围；dash 拒绝负条目、非空全零、负相位、累计溢出及奇数周期倍增溢出。新增十二类非法与五组合合法边界回归。工作区 pdf 单元 208、集成 12、render 6 全部通过，0 忽略；差异检查通过。颜色/Pattern、全部图形状态语义、文档资源与字体编码及独立验证仍待实现，P1-04 与完整目标保持未完成。
- 2026-10-03 P1-04 操作数类型校验补充：构建完成校验集中检查常见路径/变换/文字状态/设备颜色数字参数、资源名称、Tf 名称与字号、文字字符串、TJ 字符串/有限数字数组、dash 数字数组/相位及标记内容属性类型。新增九类错误参数拒绝且原内容不变、五类合法内容通过回归。工作区 pdf 单元 207、集成 12、render 6 全部通过，0 忽略；差异检查通过。Pattern 动态分量、整数枚举/数值范围、完整绘图状态和文档资源/编码校验仍待完善，P1-04 与整体目标未完成。
- 2026-10-03 P1-04 构建完成校验：新增 ContentBuilder::try_build/try_build_with_limits，严格解析构建内容，拒绝未知操作符、固定参数数量错误、q/Q 下溢/未闭合、BT/ET 嵌套/不配对、部分文字操作越域及非有限数字；内容大小、操作数量、状态深度受 Limits 限制，复制缓冲采用可失败分配。文档 builder 替换/追加入口改用此校验，旧 build 保留原低层语义。数字格式化统一 f32 Display，修复大整数转 i64 饱和截断；新增错误状态/参数/数字、七操作精确限额与大有限值回归。工作区 pdf 单元 206、集成 12、render 6 全部通过，0 忽略；差异检查通过。未知 raw 操作或未闭合片段不再能经 builder 文档入口提交；资源/编码、全部操作数类型和状态语义、临时副本聚合预算仍待完善，P1-04 及整体计划未完成。
- 2026-10-03 P1-04 图像绘制构建：ContentBuilder 新增 draw_image/draw_image_with_limits，校验非空无零字节名称及有限矩阵，复用 writer 名称转义，输出 q/cm/Do/Q 隔离状态；保留大有限数值而不经 i64 截断。构建完整片段后检查转义名称 token 与累计原始内容限额，再可失败扩容提交，错误不追加片段。新增特殊名称、大数、非法参数、精确字节限额及错误保持旧内容回归；RGB 嵌入端到端用新 builder 生成内容并严格读回/解释。最终全工作区 pdf 单元 205、集成 12、render 6 全部通过，0 忽略；差异检查通过。片段临时分配/名称复制峰值、旧 builder 数值与状态配对检查和独立绘制验收仍待完善，P1-04 与整体计划未完成。
- 2026-10-03 P1-06 RGB 原始像素嵌入：ResourceManager::embed_rgb_image 检查正尺寸、像素数、RGB 字节安全乘法、原始/解码字节限额及精确数据长度，生成标准 8 位 DeviceRGB Image XObject，整数尺寸保留原始整数词法；对象插入和页面注册处于同一事务。新增两像素红绿图像嵌入、严格保存读回及 Do 内容解释回归，零尺寸/错误数据/畸形 XObject 类别导致失败后页面和下一对象号恢复回归。工作区 pdf 单元 203、集成 12、render 6 全部通过，0 忽略；差异检查通过。当前未压缩/去重，无文件格式导入、alpha、独立像素验证和全局预算；P1-06 及整体计划保持未完成。
- 2026-10-03 P1-03 共享与图像注册验收补充：新增两个页面共用间接 Font 类别字典回归，注册第二字体后目标页复制类别字典，原共享对象与非目标页引用不变，严格保存读回保持隔离；新增合法 RGB 单像素图像的间接 Subtype 解析、重复注册 Im1 复用以及保存读回资源 ID/原始像素字节保持回归。工作区 pdf 单元 203、集成 10、render 6 全部通过，0 忽略；差异检查通过。本轮为注册结构专项验证，不证明实际图像绘制、继承资源全部场景或完整字体嵌入；P1-03 仍待嵌入和完整资源模型，整体目标未完成。
- 2026-10-03 P1-03 资源注册初版：新增 document.resources()/ResourceManager::register_existing，支持同文档现有字体/图像对象的页面作用域注册、确定性 F/Im 名称分配、同 ID 名称复用；解析继承资源及间接类别字典后复制再修改，只提交目标页，错误走事务回滚。检查基本 Font/Image 类型与类别条目数量预算。新增字体重复注册、冲突避让、跨页同 ID 复用、严格保存读回及非法/不存在引用不修改页面回归。工作区 pdf 单元 203、集成 8、render 6 全部通过，0 忽略；差异检查通过。尚无嵌入、内容去重复用、文档身份句柄、完整资源结构验证及共享继承资源专项/图像分支验收；P1-03 保持未完成，整体计划继续。
- 2026-10-03 PageBuilder 构造分配补充：基础页面字典构造和坐标数组返回 PdfResult，全部容器追加/插入使用可失败入口；移除随后立即被配置覆盖的默认 MediaBox/Resources 分配，创建事务继续传播错误。新增原始内容限额四字节精确接受、五字节拒绝且不占对象 ID 回归。工作区 pdf 单元 203、集成 6、render 6 全部通过，0 忽略；差异检查通过。字符串/名称和事务快照克隆仍有未预算分配，无分配器故障注入；资源管理与独立验证及整体计划仍待完成，P1-02 未勾选。
- 2026-10-03 P1-02 PageBuilder 初版：新增页面尺寸、MediaBox/CropBox、旋转、Resources 与初始内容配置，add_page_with_builder 与既有 add_page 共用插入事务。配置前置检查有限正尺寸、页面框正面积相交、90 度倍数旋转及原始内容字节限额；旋转规范到 0/90/180/270，页面数量转换检查 u32 溢出。新增非零页面框/旋转/资源/内容严格保存读回，以及非法配置不分配 ID 回归。工作区 pdf 单元 202、集成 6、render 6 全部通过，0 忽略；差异检查通过。README 明确默认 Letter 尺寸及低层资源/内容边界。资源引用有效性、高层资源注册、部分构造分配可失败化与独立渲染验收仍待完善；P1-02 与整体计划保持未完成。
- 2026-10-03 P1-01 Trailer 状态解耦：PDFContext 独立持有文档 Trailer，打开文档时由 CrossRefTable::take_trailer 移交，避免新增根字典克隆；新建文档 xref 保持空表，Root 访问和 writer 使用文档 Trailer。新增解析 Trailer 自定义字段完整保留且源 xref 不再持有该字段回归，已有新建/严格保存读回、xref 历史及加密保存拒绝继续通过。工作区 pdf 单元 200、集成 6、render 6 全部通过，0 忽略；差异检查通过。公共泛型消除、文档状态抽取、缓存修订号及独立验证仍待推进，P1-01 保持未勾选。
- 2026-10-03 P1-01 无输入新建入口：PDFContext parser 改为可选并独立保存 ParseMode；新增 PdfDocument::new/new_with_options，直接初始化 Catalog、Pages、Trailer 和对象存储，不构造 Reader、不解析模板。新增空文档严格读回、多页创建/内容修改/删除保存读回、构造对象超限与失败页面创建回滚 ID 回归。工作区 pdf 单元 199、集成 6、render 6 全部通过，0 忽略；cargo check --workspace --locked 与差异检查通过。README 和架构文档同步更新。公共 R 泛型及 Trailer/xref 耦合仍保留，尚无独立阅读器验证、高层资源/页面构建和缓存修订号；P1-01/A1 与整体计划保持未完成。本轮在既有 P0 保护下推进可复用的创建路径，不代表 P0 全部验收。
- 2026-10-03 编辑对象数量预算补充：PDFContext 插入对象前按原始有效对象与新增保留记录的并集检查 max_objects，更新原对象不重复计费，删除记录保守保留计费；旧插入入口也委托该检查。初始化下一对象号使用 checked_add，拒绝 xref 最大号 u32::MAX，避免输入触发溢出。新增精确额度、重复超限、事务失败恢复 ID/额度、删除后不释放保留记录额度及原对象更新不重复计费回归。工作区 pdf 单元 199、集成 4、render 6 全部通过，0 忽略；差异检查通过。计数目前线性扫描，并非对象图字节聚合预算；缓存/克隆峰值、新建文档与整体计划仍未完成，P0-10 保持未勾选。
- 2026-10-03 对象更新/删除分配补充：DocumentStore update_object/delete_object 改为 PdfResult，首次变更记录在插入前 try_reserve；已有记录替换不申请额外表容量。PDFContext 直接传播结果，公共文档编辑事务可捕获分配错误。新增同 ID 更新、重复删除、恢复及新删除记录回归，验证条目数量、容量复用及下一对象号不变。工作区 pdf 单元 196、集成 4、render 6 全部通过，0 忽略；差异检查通过。无分配器故障注入，事务快照克隆、全局预算和创建架构仍待完善；低层 DocumentStore 两个方法返回类型变更，P0-08/P0-10 仍未完成。
- 2026-10-03 对象 ID 分配补充：DocumentStore/Context/Document 新增 try_insert_object，检查对象号递增溢出、保留号零及同号不同代次冲突，HashMap 可失败扩容，成功后才提交下一对象号。页面内容替换/追加和空页面创建改用可失败入口，使分配错误传播至事务回滚。新增重复失败状态不变、代次冲突及最后可分配对象号回归；工作区 pdf 单元 195、集成 4、render 6 全部通过，0 忽略；差异检查通过。旧 insert_object 保留兼容并明确可能 panic；冲突检查目前线性扫描，聚合对象预算、更新/删除分配与新建文档架构仍未完成，A2/P0-08/P0-10 保持未勾选。
- 2026-10-03 最终 Trailer 校验补充：writer 在过滤历史字段并生成 Size 后，再按根字典检查最终节点数和深度，通过后才写出 trailer 前缀；字典插入使用 try_insert。新增空原 Trailer 加入 Size 后精确两节点/两层通过、少一节点或一层拒绝且输出保持不变回归。工作区 pdf 单元 193、集成 4、render 6 全部通过，0 忽略；差异检查通过。克隆前预算、全量输出限额与文档聚合预算仍待完善，P0-10 及整体计划保持未完成。
- 2026-10-03 Trailer 与验证栈预算补充：共享迭代验证器支持借用根条目，Trailer 在 materialize_all_objects 前按根字典计数/层级校验；数组、字典与流字典扩栈前联合检查已处理和待处理节点，避免验证栈先膨胀后超限。扩展 Trailer 根层级精确边界与超限回归。工作区 pdf 单元 192、集成 4、render 6 全部通过，0 忽略；差异检查通过。验证包含原 Trailer 字段，writer 添加 Size 等后的最终结构尚未再统一验证；普通对象检查晚于克隆，全量文档聚合预算及整体计划未完成。
- 2026-10-03 序列化对象预检查补充：每个间接对象写出前使用迭代遍历检查直接嵌套深度及节点数（max_depth/max_objects），验证栈采用可失败分配；间接引用不展开，避免循环引用影响直接序列化。新增四层/四节点精确边界及深度/节点超限回归。工作区 pdf 单元 192、集成 4、render 6 全部通过，0 忽略；差异检查通过。预检查晚于 object_for_write 克隆且按单对象计费，Trailer 及完整文档聚合预算尚未接入，递归写出仍存在，整体计划未完成。
- 2026-10-03 P0-10 xref 输出分配补充：writer 最大对象号+1 使用 checked_add，经典 xref 按完整槽位数（含空闲项）在分配前检查 max_objects，try_reserve_exact 后初始化条目；Size 使用已验证值，拒绝稀疏大号导致的巨量分配。新增精确边界、空分配、超限与 u32::MAX 槽位回归。工作区 pdf 单元 191、集成 4、render 6 全部通过，0 忽略；差异检查通过。稀疏 xref 子表优化、序列化递归和输出总预算仍待实现，整体计划未完成。
- 2026-10-03 字符串输出分配补充：writer 字符串输出改为 PdfResult，按转义规则安全计算准确长度并 try_reserve_exact，容量/算术失败在写出前返回 ResourceLimit；write_object 传播结果。新增全部 256 种字节写出再解析一致性和空字符串回归。工作区 pdf 单元 190、集成 4、render 6 全部通过，0 忽略；差异检查通过。全量输出限额、其他 writer 分配及独立读者验证仍待完善，整体计划未完成。
- 2026-10-03 名称输出分配补充：writer 安全累计名称转义后的准确长度，输出前 try_reserve_exact，溢出/容量失败返回 ResourceLimit；#XX 转义改为固定十六进制字节表，避免逐转义字节创建 format 临时字符串。工作区 pdf 单元 189、集成 4、render 6 全部通过，0 忽略，特殊字符/二进制名称及保存往返回归继续通过；差异检查通过。其他 writer 分配、输出总限额与全局预算仍待完善，整体计划未完成。
- 2026-10-03 名称解析/序列化修复：解析拒绝原始或 #00 解码后的零字节名称；writer 名称输出改用原始 bytes 而非有损显示字符串，修复非 UTF-8 名称被替换字符污染。write_name 返回 PdfResult，并在输出前拒绝低层构造的零字节名称；字典键写出传播错误。调整长名称回归使用合法非零二进制字节，扩展 #FF/#80 精确保存和失败无输出回归。工作区 pdf 单元 189、集成 4、render 6 全部通过，0 忽略；差异检查通过。早期 P0-07 记录对原始二进制名称保留的证据不充分，本次补齐名称值；字典键仍为 String，二进制键身份与整体计划未完成。
- 2026-10-03 名称字节分配补充：PdfName::new_from_buffer 原始字节展开使用 try_reserve，失败返回 ResourceLimit；新增 4096 组转义名称回归验证完整原始字节保留。工作区 pdf 单元 189、集成 4、render 6 全部通过，0 忽略；差异检查通过。显示字符串 from_utf8_lossy 的分配、名称零字节策略和字典键二进制身份仍待完善，整体计划未完成。
- 2026-10-03 字符串缓冲分配补充：read_line/read_hex_string/read_string 在限额检查通过后对下一字节执行 try_reserve，容量失败返回对应 ResourceLimit，保留转义、奇数十六进制补零和换行语义。工作区 pdf 单元 188、集成 4、render 6 全部通过，0 忽略；既有字节限额、字符串/行截断及 I/O 异常回归继续通过；差异检查通过。本次无分配器故障注入，字符解码与对象克隆全局峰值预算仍待完善，整体计划未完成。
- 2026-10-03 token 分配与分隔符限额补充：新增 PdfWord::try_add_char，get_next_word 的名称、普通词和分隔符追加使用可失败扩容；修正 max_token_bytes=0 仍接受单字节分隔符、额度 1 仍接受 << 或 >> 的缺口。新增零/一/二字节额度下的单分隔符、双分隔符与名称边界回归。工作区 pdf 单元 188、集成 4、render 6 全部通过，0 忽略；差异检查通过。保留 add_char 兼容入口，其他字符串缓冲与全局峰值预算尚待完善，整体计划未完成。
- 2026-10-03 语法诊断载荷补充：PdfWord UTF-8 错误仅报告 Utf8Error，整数范围错误不展开原始 token；非法字典键、严格重复键和非法对象词改为带字节位置的短错误，避免复制/格式化大型载荷。新增十万字节整数及非法 UTF-8 错误文本长度回归。工作区 pdf 单元 187、集成 4、render 6 全部通过，0 忽略；差异检查通过。其他模块诊断、兼容 panic API 和全局预算仍待审查，整体计划未完成。
- 2026-10-03 字典解析分配补充：新增 PdfDict::try_insert，新键插入先 try_reserve，失败返回 ResourceLimit；已有键替换不额外申请表容量。语法字典解析接入可失败入口，保留 insert 兼容 API、严格重复键拒绝和兼容替换语义。新增插入/替换及键数量回归；工作区 pdf 单元 186、集成 4、render 6 全部通过，0 忽略；差异检查通过。字符串键二进制身份、对象克隆与全局预算仍待完善，整体计划未完成。
- 2026-10-03 数组追加分配补充：新增 PdfArray::try_add_obj，try_reserve 失败返回 ResourceLimit；语法数组解析和 append_page_contents 两类追加分派接入可失败入口，事务继续传播错误回滚。保留 add_obj 兼容入口。扩展数组追加回归；工作区 pdf 单元 185、集成 4、render 6 全部通过，0 忽略，既有语法限额、页面追加保存及事务回归继续通过；差异检查通过。未提供实际分配器故障注入，其他兼容入口与全局峰值预算仍待完善，整体计划未完成。
- 2026-10-03 数组编辑安全补充：PdfArray::insert 改为 PdfResult，先检查 index≤len 并 try_reserve，再执行插入，越界/容量失败不修改数组；页面树 Kids 插入传播结果以保持事务回滚。新增空数组非法索引、首尾插入、最大索引及失败状态不变回归。工作区 pdf 单元 185、集成 4、render 6 全部通过，0 忽略；差异检查通过。该低层公开 API 返回类型变更；add_obj 等其他分配和整体计划仍待完善。
- 2026-10-03 图形状态栈分配补充：q 在深度检查通过后 try_reserve，栈容量分配失败返回 ResourceLimit；移除过时重置状态注释。扩展现有深度回归验证恰好 8 层 q 再完整 Q 恢复成功，超过深度及操作数量仍拒绝。工作区 pdf 单元 184、集成 4、render 6 全部通过，0 忽略；差异检查通过。AllState 内部克隆及保存裁剪累计内存尚未预算，整体计划未完成。
- 2026-10-03 内容操作数分配补充：ContentParser 在单操作数数量检查后、解析下一对象前对数组 try_reserve，扩容失败返回 ResourceLimit。新增限额按操作符重置、精确两参数、无参数及超限优先于下一未闭合字符串解析回归。工作区 pdf 单元 184、集成 4、render 6 全部通过，0 忽略；差异检查通过。对象内部临时分配与全局峰值预算仍待补齐，整体计划未完成。
- 2026-10-03 诊断副本分配补充：兼容模式直接移动完整消息至诊断，不再生成用后丢弃的 message.clone；operator 字符串及严格模式错误副本均使用 try_reserve_exact，成功后才提交计费和诊断。保留严格模式错误及已记录诊断语义。工作区 pdf 单元 183、集成 4、render 6 全部通过，0 忽略；既有数量/字节边界回归继续通过，差异检查通过。DisplayList 诊断克隆及全局峰值预算仍未完成，整体计划未完成。
- 2026-10-03 P0-10 诊断字节补充：Limits 新增 max_content_diagnostic_bytes（默认 16 MiB），解释器在构造完整消息前安全计算并累计 operator/message 两份保留文本的字节；消息采用 try_reserve_exact。新增依据实际诊断长度的精确限额与少一字节累计超限回归。工作区 pdf 单元 183、集成 4、render 6 全部通过，0 忽略；差异检查通过。诊断克隆、字符串容量和临时副本峰值尚不在聚合预算内，整体计划未完成。
- 2026-10-03 P0-10 内容诊断数量补充：Limits 新增 max_content_diagnostics（默认每页 1024），未知/未支持操作符在构造诊断前检查数量，诊断数组 try_reserve；超限返回 ResourceLimit。新增上限 2 的精确两条诊断、第三条超限及后续合法内容恢复回归。工作区 pdf 单元 182、集成 4、render 6 全部通过，0 忽略；差异检查通过。该限制两种模式均生效，额度为零时未支持操作符优先返回 ResourceLimit；诊断字符串字节与全局峰值仍待预算，整体计划未完成。
- 2026-10-03 P0-10 Indexed 调色板补充：字符串 lookup 在复制前按 max_decoded_bytes 检查字节数并 try_reserve_exact，失败返回 ResourceLimit，避免字符串路径绕过流解码限额。扩展自定义上限 3 的精确 RGB 条目与超限字符串回归。工作区 pdf 单元 181、集成 4、render 6 全部通过，0 忽略；差异检查通过。ctx.resolve 的对象克隆仍早于此检查，完整对象图峰值预算及 Indexed 非归一化基色空间转换仍待推进，整体计划未完成。
- 2026-10-03 CMYK 分量裁剪补充：DeviceCMYK RGB 转换在有限性检查之后、乘法计算之前将四分量裁剪至 0–1，避免超范围有限值导致错误颜色或中间溢出；保留现有无配置转换公式。新增负值/超 1 值、超大有限值、黑白及 NaN 回归。工作区 pdf 单元 181、集成 4、render 6 全部通过，0 忽略；差异检查通过。该转换仍非 ICC 色彩管理，独立像素与颜色兼容验收及整体计划未完成。
- 2026-10-03 颜色切换默认值补充：cs/CS 在空间解析与默认值求取成功后同步替换对应空间和颜色，避免旧分量遗留；不修改另一类颜色。DeviceCMYK 默认值从白色 [0,0,0,0] 修正为黑色 [0,0,0,1]。新增实际路径事件状态回归验证 RGB 填充/CMYK 描边复位及另一类灰度不变。工作区 pdf 单元 180、集成 4、render 6 全部通过，0 忽略；差异检查通过。当前证据为绘制事件结构，独立像素与复杂颜色兼容验收仍待完成，整体计划未完成。
- 2026-10-03 颜色空间与动态参数修复：SC/sc 及非 Pattern SCN/scn 在提交颜色前按当前空间校验精确分量数。修正 find_colorspace 中 DeviceRGB/DeviceCMYK 大小写、内置空间误修改描边且返回灰度的问题；内置空间直接返回解析结果，非名称拒绝，移除载荷调试日志。新增缺失/多余分量、类型错误以及 Gray/RGB/CMYK 合法选择回归。工作区 pdf 单元 179、集成 4、render 6 全部通过，0 忽略；差异检查通过。空间切换默认颜色复位及独立像素验证仍待补充，整体计划未完成。
- 2026-10-03 P0 操作数数量补充：opcode 集中定义固定参数数量，Interpreter 在路径预算及状态更新前校验，拒绝合法操作符携带多余或缺失参数；动态颜色分量和未知操作符不强套固定数量。新增 q/w/m/l/re/cm/BT/T*/rg 九类错误与合法后续绘制回归。工作区 pdf 单元 178、集成 4、render 6 全部通过，0 忽略；差异检查通过。标准未实现操作符的畸形数量现在也返回 PageOperatorError，未知操作符诊断策略保持不变；动态类型/内容编辑与整体计划未完成。
- 2026-10-03 P0-10 内容流数量补充：Contents 数组在解码前按 max_objects 检查长度，避免低层编辑构造大量空流绕过语法容器和累计字节限额。新增自定义上限 2 的空数组、精确两流和三流超限回归。工作区 pdf 单元 177、集成 4、render 6 全部通过，0 忽略；差异检查通过。数量检查仍在对象解析/克隆后，完整对象图及峰值内存预算尚未完成，整体计划未完成。
- 2026-10-03 Contents 分派统一：先经统一 resolve 读取最新对象，再统一处理流、流数组与 null；移除直接数组/引用数组两套重复逻辑，低层直接流不再被误拒绝，null 作为空内容。扩展直接空流、非空流与 null 回归，原页面编辑与间接内容往返继续通过。工作区 pdf 单元 176、集成 4、render 6 全部通过，0 忽略；差异检查通过。非法数组条目仍拒绝，完整对象图预算及整体计划未完成。
- 2026-10-03 P0-10 页面内容拼接补充：content_bytes 逐流解码并直接拼接到单一结果，替代先保存所有解码流再二次拼接；每次追加前检查累计字节并 try_reserve_exact，容量失败返回 ResourceLimit。新增两个流在数字中间拆分后精确拼接回归，既有累计限额测试继续通过。工作区 pdf 单元 176、集成 4、render 6 全部通过，0 忽略；差异检查通过。仍有当前解码流与目标缓冲区共存及扩容成本，不代表全局峰值预算完成，整体计划未完成。
- 2026-10-03 页面几何刷新补充：update_dimensions 每次从默认框重建，避免缺失 MediaBox 时复用旧 CropBox 导致框无法扩大或移除；先在局部计算框和尺寸并验证旋转，成功后提交 bbox/尺寸/矩阵，失败不污染既有几何。扩展回归验证同一页面对象裁剪扩大、移除属性恢复默认及非法旋转后保留框/尺寸/矩阵平移。工作区 pdf 单元 176、集成 4、render 6 全部通过，0 忽略；差异检查通过。默认框仍为现有兼容策略，严格模式必需字段策略及整体计划未完成。
- 2026-10-03 页面几何修复：修正非零页面框原点在 0/90/270 度矩阵中的平移符号；180 度改为 X 反向、Y 正向并减去 lly，不再错误地重复普通页面 Y 翻转。扩展几何回归，验证非零 CropBox 两个对角点在四种旋转下映射到对应表面边界；已有负角度/超一周角度及非法框回归继续通过。工作区 pdf 单元 176、集成 4、render 6 全部通过，0 忽略；差异检查通过。当前证据为坐标结构，独立像素对比及完整页面/渲染验收仍待完成，整体计划未完成。
- 2026-10-03 P0 XObject 分派补充：Do 先确认 XObject 为流，Subtype 经统一 resolve 解析间接引用；Form 不再误报 PageXobjectNotFound，Form/其他不支持子类型返回 UnsupportedFeature。新增页面内容引用调用 Form/PS 的间接 Subtype 回归。工作区 pdf 单元 176、集成 4、render 6 全部通过，0 忽略；差异检查通过。Form 实际执行、递归限额和复杂资源作用域仍属后续任务；未支持 XObject 当前仍终止解释，整体计划未完成。
- 2026-10-03 P0-10 图像输出预算预检查：PdfImage 根据已验证尺寸在过滤器解码前检查 pixel_count×4 的溢出及 max_decoded_bytes，避免解码后才在 rgba_buffer 拒绝超预算输出；转换入口继续保留检查。新增超过 RGBA 预算且带未知过滤器优先返回 ResourceLimit、恰好 4 字节单像素 RGB 转换回归。工作区 pdf 单元 175、集成 4、render 6 全部通过，0 忽略；差异检查通过。PdfImage 为渲染图像模型，构造现在也受 RGBA 输出限额约束；多个缓冲区共存峰值与独立原始图像提取 API 仍待实现，整体计划未完成。
- 2026-10-03 P0 图像对象类型补充：PdfImage 解析后首先要求流对象，不再先读取非流字典的尺寸/资源；显式 Subtype 解析引用并要求原始名称字节为 Image，拒绝 Form、null 或其他类型。复用已验证 stream 解码。新增 null/普通字典、错误子类型及合法 Image 回归。工作区 pdf 单元 174、集成 4、render 6 全部通过，0 忽略；差异检查通过。缺失 Subtype 仍保留兼容入口行为，完整图像字典验收与整体计划未完成。
- 2026-10-03 P0 图像位深错误分类补充：显式 BitsPerComponent 必须为精确整数 1/2/4/8/16，错误类型、负值、零、小数或其他数值返回 ObjectError；合法但暂未实现的非 8 位返回 UnsupportedFeature。新增非法值、四种合法不支持位深与合法 8 位回归。工作区 pdf 单元 173、集成 4、render 6 全部通过，0 忽略；差异检查通过。缺失字段与过滤器特例仍保留原行为，低位深/16 位实际支持及整体计划未完成。
- 2026-10-03 P0-08 图像配置补充：ImageMask 解析引用并要求布尔类型，true 返回明确不支持；非空 Mask/SMask/Decode 在流解码前返回 UnsupportedFeature，不再静默忽略后输出错误像素。新增四类不支持配置、错误 ImageMask 类型与 ImageMask=false/空可选项普通 RGB 回归。工作区 pdf 单元 172、集成 4、render 6 全部通过，0 忽略；差异检查通过。包括默认 Decode 数组在内的显式非空 Decode 暂不支持；实际蒙版/解码映射支持仍属后续图像任务，整体计划未完成。
- 2026-10-03 字体查找开销补充：Format4::get_gid 利用已验证的有序非重叠区间对 endCode 二分定位，不再为每个字符逐段扫描，保留缺字、delta 及字形数组边界行为。新增全部 65,536 个 BMP 字符码的独立期望对照，覆盖正/负 delta、缺字间隙与终止码；已有字形数组偏移回归继续通过。工作区 pdf 单元 171、集成 4、render 6 全部通过，0 忽略；差异检查通过。此改动没有性能实测或原生后端预算证据，完整字体兼容与整体计划未完成。
- 2026-10-03 P0 Format4 区间补充：除 endCode 递增外检查后一 startCode 严格大于前一区间结束码，拒绝重叠区间；终止区间要求 startCode/endCode 均为 0xffff。新增三段重叠拒绝、合法相邻段端点映射、区间外缺字、终止码和不完整终止区间回归。工作区 pdf 单元 170、集成 4、render 6 全部通过，0 忽略；差异检查通过。独立字体兼容验证及完整字体支持仍待推进，整体计划未完成。
- 2026-10-03 P0 Format4 分配补充：endCode/startCode/idDelta/idRangeOffset/glyphIdArray 使用 try_reserve_exact 可失败预分配，容量错误返回 ResourceLimit；Format4 直接入口在消费输入前验证声明长度对应的剩余字节。新增直接入口全部截断长度、读取位置保持、完整输入与 usize::MAX 容量错误回归。工作区 pdf 单元 169、集成 4、render 6 全部通过，0 忽略；差异检查通过。完整字体格式、文档加载接入与原生后端内存预算仍待推进，整体计划未完成。
- 2026-10-03 P0 字体分配补充：Format0Cmap 直接填充固定 256 字节数组，移除临时 Vec 和 try_into unwrap；TrueTypeProgram 的 cmap 目录与解析结果根据已验证记录数 try_reserve_exact，分配失败返回 ResourceLimit。新增 Format0 全部字符码映射与 0–255 字节截断回归。工作区 pdf 单元 168、集成 4、render 6 全部通过，0 忽略；差异检查通过。Format4 内部数组、字体文档加载和全局预算仍待完善，整体计划未完成。
- 2026-10-03 P0-10 TrueType 展开预算补充：新增 TrueTypeProgram::try_new_with_limits，原 try_new 使用默认 Limits；字体输入字节、cmap 记录数与各记录累计声明子表字节在解析前检查，重复引用同一子表仍计费，限制重复展开。新增三个共享 Format0 记录的精确累计边界、少一字节超限、记录数超限及输入字节超限回归。工作区 pdf 单元 167、集成 4、render 6 全部通过，0 忽略；差异检查通过。此限制为低层解析入口，未接入文档字体加载或原生后端，非完整字体内存预算；P0-10 与整体计划未完成。
- 2026-10-03 字体映射选择补充：修复 TrueTypeProgram::selct_charmap 的符号/Unicode 优先级反向，符号字体先选择 Windows symbol（3/0），其他字体先选择 Windows Unicode（3/1），再按现有 Macintosh/首个映射回退；每次选择先重置 current。新增同时包含三类映射、切换符号标志、缺少首选映射及空映射状态回归。工作区 pdf 单元 166、集成 4、render 6 全部通过，0 忽略；差异检查通过。当前该选择 API 未接入页面绘制，不证明端到端文字映射正确，完整字体支持及整体计划仍未完成。
- 2026-10-03 P0-08 cmap 局部范围补充：TrueTypeProgram 为 cmap 建立独立表切片，子表偏移保持表内相对值，不再进行未经检查的文件偏移加法；检查版本为 0、编码目录完整性以及子表起点不落入目录或超出表。TableRecord 提供长度查询。新增完整 Format0 表、实际文件有后续字节但声明表长度截断、目录内/表外/最大偏移及非法版本回归，内置字体继续通过。工作区 pdf 单元 165、集成 4、render 6 全部通过，0 忽略；差异检查通过。共享子表允许复用；完整字体格式、子表重复解析开销及聚合预算仍需推进，整体计划未完成。
- 2026-10-03 P0-08 sfnt 目录补充：移除外部字体目录 rangeShift 断言与无条件日志，目录长度使用 usize 安全计算并在遍历前检查剩余字节；检查 searchRange/rangeShift 的长度关系、每个表的 offset+length 文件边界及重复标签。TrueTypeReader 提供输入长度查询。新增合法精确边界、所有截断长度、字段错误、最大表数量及重复标签回归；内置字体回归仍通过。工作区 pdf 单元 164、集成 4、render 6 全部通过，0 忽略；差异检查通过。子表局部边界、完整字体格式与聚合预算仍待推进，P0-08 和整体计划未完成。
- 2026-10-03 P0 字符串转换补充：PdfString::get_content 先对借用字节校验 UTF-8，失败不再先克隆载荷或 Debug 展开 FromUtf8Error 的完整字节；有效文本使用 try_reserve_exact 分配，容量失败返回 ResourceLimit。新增空串、ASCII、中文/emoji、10 万字节中部非法字节与截断编码回归，验证错误长度受控且原始字节不变。工作区 pdf 单元 163、集成 4、render 6 全部通过，0 忽略；差异检查通过。此方法仍为显式 UTF-8 转换，不是 PDFDocEncoding/UTF-16 文档文本解码器；整体计划未完成。
- 2026-10-03 P0 诊断分配补充：Operator::operand/get_as_f32 将 eager ok_or 改为惰性错误构造，移除对完整操作数/错误类型载荷的 Debug 展开，避免成功读取时也格式化整个字符串或数组。错误改为操作数索引、数量或期望类型；新增 10 万字节字符串回归验证正常访问、缺失/类型错误及错误文本大小。工作区 pdf 单元 162、集成 4、render 6 全部通过，0 忽略；差异检查通过。其他诊断及对象图聚合内存仍需审查，整体计划未完成。
- 2026-10-03 P0-08 加密辅助入口补充：Rc4::new/rc4_decrypt 与四个 AES 辅助函数改为 PdfResult，空或过长 RC4 密钥、错误 AES 密钥/IV、短或不对齐密文及非法填充返回错误，移除 assert/unwrap 和未检查切片。IV 前缀 AES 解密直接使用库返回的有效明文范围；零填充加密预留完整块空间并检查容量。新增 RC4 已知向量、AES-128/256 空/跨块明文往返与非法输入回归。工作区 pdf 单元 161、集成 4、render 6 全部通过，0 忽略；差异检查通过。低层公开 API 返回类型有变更；尚未接入安全处理器，不代表 P5 文档加密已实现，整体计划仍未完成。
- 2026-10-03 P0 图形标量补充：i 平滑度要求 0–100；d 在提交状态前检查虚线条目累计周期及奇数条目重复周期的有限性，避免有限单项合计溢出后传入后端。新增非法范围、两个超大条目、单个超大奇数条目、零数组、负相位错误与合法边界/空数组/奇数数组恢复回归。工作区 pdf 单元 158、集成 4、render 6 全部通过，0 忽略；差异检查通过。该检查不覆盖原生后端虚线细分开销，P0-08/P0-10 及整体计划仍未完成。
- 2026-10-03 P0-10 路径展开预算补充：Limits 新增 max_path_segments（默认每页 100 万），解释器在路径操作执行前累计检查构造段数；re 按 5 段计费，h/s/b/b* 仅对尚未闭合路径计入 Close，重复闭合不重复计费，绘制或丢弃不重置累计预算。新增精确边界、矩形展开、丢弃后累计超限、闭合超限及曲线组合回归。工作区 pdf 单元 157、集成 4、render 6 全部通过，0 忽略。该限制覆盖解释器构造的路径段，不覆盖事件快照克隆和原生后端内部细分的累计内存；P0-10 与整体计划仍未完成。
- 2026-10-03 P0-10 字符解码预检查补充：Font/SimpleFont/Type0/CMap 新增 chars_limited，解释器传入每页剩余字符额度；简单字体在数组分配前拒绝超限，CMap 每次展开前检查，使用 try_reserve 的可失败分配。保留原 chars API。扩展 CMap 回归验证双字节一个字符可用额度 1、混合字符超限、零额度与空输入，已有 Tj/TJ 累计边界回归继续通过。工作区 pdf 单元 156、集成 4、render 6 全部通过，0 忽略；差异检查通过。上一记录中的 chars 先无限展开问题已补齐；Vec 容量与 Unicode 字符串等总内存尚需聚合预算，完整计划未完成。
- 2026-10-03 P0-10 文字数量补充：Limits 新增 max_text_chars（默认每页 100 万），Tj/TJ 在构造字符绘制事件前共享累计字符预算，避免单操作符或多操作符无限展开事件。Helvetica 回归设置上限 2，验证单字符串、TJ 多字符串及多个 Tj 累计超限，精确两字符与失败后正常内容继续通过。工作区 pdf 单元 156、集成 4、render 6 全部通过，0 忽略；差异检查通过。检查仍在字体 chars 临时数组分配之后，字符解码预分配限制与全局预算未完成，P0-10 和整体计划未完成。
- 2026-10-03 P0-10 裁剪数量补充：Limits 新增 max_clip_paths（默认 1024），W/W* 在已有累计裁剪到上限时返回 ResourceLimit，避免无限增长的裁剪状态随事件反复克隆。新增自定义上限 2 的精确边界、两种裁剪超限、失败后合法内容以及 Q 恢复后继续裁剪回归。工作区 pdf 单元 156、集成 4、render 6 全部通过，0 忽略；差异检查通过。仅限制单状态裁剪交集数量，不覆盖路径段总量、事件快照累计内存或全局峰值；P0-10 与整体计划仍未完成。
- 2026-10-03 P0 曲线路径补充：l/c/v/y 缺少当前点时返回 PageOperatorError，不再静默忽略；修复 y 使用当前点作第一控制点的错误，改为显式第一控制点、第二控制点等于终点。新增 6 种无当前点/路径丢弃或绘制后继续操作的页面错误回归，以及 y 的三个控制/终点精确结构回归。工作区 pdf 单元 155、集成 4、render 6 全部通过，0 忽略；差异检查通过。独立渲染兼容验证、其余输入路径与总体预算仍待完成，整体计划未完成。
- 2026-10-03 P0 图形保存状态补充：内容结束时检查 q/Q 完整配对；Q 恢复图形状态但保留当前文字矩阵/文字行矩阵，修复不属于图形状态的文字定位被回退。扩展孤立 q、嵌套未关闭和孤立 Q 异常回归，并验证 BT 内 q/Tm/Q 后文字仍在设定位置。工作区 pdf 单元 154、集成 4、render 6 全部通过，0 忽略；差异检查通过。严格/兼容模式均拒绝未配对保存状态，完整状态/资源预算与整体计划仍未完成。
- 2026-10-03 P0 文字边界补充：解释器独立跟踪 BT/ET 状态，拒绝嵌套 BT、无匹配 ET 与内容末尾未关闭；Td/TD/Tm/T*/Tj/TJ/单双引号定位或显示要求处于文字对象内，文字状态参数仍允许对象外设置。扩展 9 种页面异常边界回归，已有合法文字/路径回归全部通过。工作区 pdf 单元 154、集成 4、render 6 全部通过，0 忽略；差异检查通过。此错误策略同时适用于严格/兼容模式；q/Q 完整配对及其他状态验收仍待推进，整体计划未完成。
- 2026-10-03 P0 点变换补充：Matrix 新增 checked_transform，检查原始点、矩阵及变换结果有限性；Skia 主路径和裁剪路径进入后端前逐端点/曲线控制点校验。新增 MoveTo/LineTo/曲线控制点有限值经 DPI 缩放溢出与 NaN 回归，验证失败前画布 save_count/矩阵不变且后续正常绘制。工作区 pdf 单元 154、集成 4、render 6 全部通过，0 忽略；差异检查通过。原生后端内部细分/描边运算及文字/图像几何预算不在此测试覆盖内，完整计划仍未完成。
- 2026-10-03 P0 Skia 矩阵与恢复补充：路径/裁剪/文字/图像与 DPI 矩阵组合使用 checked_mul；set_clip 返回 PdfResult，draw_path 把裁剪与路径设置纳入统一结果闭包，失败仍 restore，主路径矩阵失败则在 save 前拒绝。新增先修改裁剪再触发第二裁剪溢出回归，验证画布 save_count/矩阵恢复及后续正常绘制。工作区 pdf 单元 154、集成 4、render 5 全部通过，0 忽略；差异检查通过。几何点变换、其他后端状态和全局内存预算仍待完善，整体计划未完成。
- 2026-10-03 P0 空裁剪补充：n 与各绘制终止符统一清理路径，空路径仍提交待生效 W/W*；空填充并描边不产生无意义事件。新增 n/S/f/B/B*/b/b* 七种空路径终止回归，验证后续路径事件包含空裁剪，Q 恢复旧裁剪且待生效标记不泄漏。工作区 pdf 单元 154、集成 4、render 4 全部通过，0 忽略；差异检查通过。当前验证为事件结构，独立像素验证与全局资源预算等仍待完成；整体计划未完成。
- 2026-10-03 P0 裁剪生效顺序补充：f/S/B 统一在设备绘制成功后提交待生效 W/W* 裁剪并清空路径状态；修复 B/B* 漏裁剪及 f/S 提前用新裁剪绘制当前路径的问题。新增 f/S/B/B*/b/b* × W/W* 共 12 组合回归，验证首个事件使用旧裁剪、后两个事件保持新裁剪且不重复提交。工作区 pdf 单元 153、集成 4、render 4 全部通过，0 忽略；差异检查通过。空路径裁剪、独立像素对比及完整 P0/后续阶段仍未完成。
- 2026-10-03 P0 路径 Close 事件补充：Segment 新增显式 Close，变换保留事件；SubPath::close 在原始顺序插入关闭操作，继续追加段后闭合状态同步更新。re/h 不再用回到起点的普通线段代替 Close；Skia 的绘制/裁剪路径调用真正 close，Trace 输出 close 事件。调整结构回归，新增 Skia 路径闭合及 Close 后继续追加得到开放末尾轮廓的后端回归。工作区 pdf 单元 152、集成 4、render 4 全部通过，0 忽略；差异检查通过。公共 Segment 枚举新增变体，外部穷尽匹配需适配；独立像素对比及完整计划仍未完成。
- 2026-10-03 P0 路径状态补充：m 始终建立新子路径；h 保持当前点为子路径起点且重复 h 不追加冗余关闭线；n 无裁剪时也清空路径与当前点。新增页面路径结构回归，验证连续 m 分成两个子路径、关闭坐标与段数、h 后 v 的隐式第一控制点、n 后 S 不产生旧路径事件。工作区 pdf 单元 152、集成 4、render 3 全部通过，0 忽略；差异检查通过。闭合后继续追加路径的后端表达、其他路径状态及总体计划仍待完成。
- 2026-10-03 P0 矩形路径补充：re 在更新路径前检查 x+width/y+height 有限性，拒绝有限操作数相加溢出；子路径标记为闭合并更新 path_start，修复端点/后续关闭语义。新增 X/Y 溢出页面回归及负尺寸矩形闭合、5 段路径、右上角精确坐标回归。工作区 pdf 单元 151、集成 4、render 3 全部通过，0 忽略；差异检查通过。路径变换的后端数值检查、其他路径操作状态及完整 P0/后续阶段仍未完成。
- 2026-10-03 P0 文字操作补充：双引号操作符设置字距后执行换行，修复遗漏 T* 导致位置错误；Tj/TJ 字宽计算先除 1000 再乘字号，避免结果可表示但中间乘法溢出。扩展 Helvetica 页面回归，验证双字符换行后的显示 Y 位移、3×10^38 字号单字可正常解释及叠加超大字距后的真实溢出被拒绝。工作区 pdf 单元 150、集成 4、render 3 全部通过，0 忽略；差异检查通过。该验证不证明极大字号后端渲染安全，完整文字状态、数值与总体预算验收仍未完成。
- 2026-10-03 P0 解释器矩阵补充：其余 25 处矩阵组合接入 checked_mul，覆盖文字定位/推进、文字与 CTM/页面组合、路径/裁剪和图像绘制，非有限结果传播错误，不进入绘制事件。扩展页面端到端非法数值回归，验证有限 3×10^38 平移与文字换行累积溢出被拒绝，正常绘制继续成功。工作区 pdf 单元 150、集成 4、render 3 全部通过，0 忽略；差异检查通过。字号/字宽等标量及点变换、状态配对、完整预算仍需推进，整体计划未完成。
- 2026-10-03 P0 矩阵计算补充：修复 Matrix::concat 原地分量更新污染后续计算，改为完整乘积后替换；新增 is_finite/checked_mul，cm 在组合溢出时返回错误且不提交 CTM；字典矩阵拒绝非有限分量。新增六分量独立预期值、正常乘法、溢出/NaN 回归及两个有限 10^20 缩放矩阵导致溢出的页面端到端回归。工作区 pdf 单元 150、集成 4、render 3 全部通过，0 忽略；差异检查通过。文字矩阵、页面组合及点坐标计算的累计溢出仍需审查，整体计划未完成。
- 2026-10-03 P0 线型参数补充：J/j 要求精确整数 0–2，错误类型/小数/越界不再默认 0；w 拒绝负数，保留 0 hairline 和非负实数。扩展页面 DisplayList 端到端回归，新增 10 种非法线型输入，并覆盖 0/1/2 三种合法线帽/连接及 0/1.5 线宽和失败后正常绘制。工作区 pdf 单元 149、集成 4、render 3 全部通过，0 忽略；差异检查通过。图形状态累计溢出、配对校验及完整计划仍未完成。
- 2026-10-03 P0 LZW 参数补充：EarlyChange 仅缺失时默认 1；存在但非整数/越界/非 0–1 时返回 FilterError，不再静默选择默认值。新增 0/1 正常解码与小数、实数 token、负数、2、整数溢出及 null 参数回归。工作区 pdf 单元 149、集成 4、render 3 全部通过，0 忽略；差异检查通过。回归使用短流，不替代两种 EarlyChange 的大字典边界验证；线型参数及其余 P0/后续阶段仍待推进。
- 2026-10-03 P0 整数接口补充：PdfObject 的 u8/u16/u32/u64/i32 读取改为精确整数校验，不再舍入/截断/饱和；新增 PdfNumber 的检查式 u8/i32 接口，测试各整数最大值、小数/实数 token、负数及越界。适配 TilingPattern 步长为有限非零 f32，移除步长与流类型 unwrap；不表示 Pattern 绘制已完成。工作区 pdf 单元 148、集成 4、render 3 全部通过，0 忽略；差异检查通过。LZW EarlyChange 与线型参数等调用方的静默默认行为仍需审查，完整计划未完成。
- 2026-10-03 P0 数字构造补充：PdfNumber::new_from_lexeme 复用语法数字校验，拒绝空/符号/指数/NaN/非法后缀；浮点视图使用标准转换替代分子分母手工累计，避免合法长小数出现 Infinity/Infinity 得到 NaN。新增百位小数有限值/精度、PDF 特殊合法拼写及 u64::MAX 原文精确转换回归。工作区 pdf 单元 147、集成 4、render 3 全部通过，0 忽略；差异检查通过。数值计算仍使用 f32，不表示所有精度问题已解决；完整 P0 与后续阶段尚未完成。
- 2026-10-03 P0 名称转义补充：PdfName::new_from_buffer 要求 / 前缀，拒绝截断的 #/#0 转义，不再按普通字符接受；合法空名称 /、#23、混合大小写十六进制与高位字节继续支持。新增底层合法/非法名称回归，以及严格/兼容独立名称和字典键截断转义的端到端语法回归。工作区 pdf 单元 146、集成 4、render 3 全部通过，0 忽略；差异检查通过。二进制字典键保真、完整 P0 与后续阶段仍未完成。
- 2026-10-03 P0 字节分类补充：PDF_CHAR_TYPES 不再把 0x80/0xFF 误判为空白，保留二进制名称中的原始高位字节；空白限定为 NUL/TAB/LF/FF/CR/SPACE。用全 256 字节断言替换原打印式测试，新增严格/兼容原始高位名称保持及后续名称推进回归，同时验证十六进制串内 0xFF 不再被忽略。工作区 pdf 单元 144、集成 4、render 3 全部通过，0 忽略；差异检查通过。字典键仍使用字符串模型，完整二进制键保真与其余 P0/P1–P5 尚未完成。
- 2026-10-03 P0 EOF 与引用代次补充：get_object 在输入末尾返回 EndofFile，不再生成隐式 PdfNull，显式 null token 仍正常；间接引用代次使用精确 u16 校验，拒绝 65536 以上/负数/小数。新增严格/兼容空输入、尾部空白/注释、显式 null 后 EOF、代次 0/65535 合法边界及非法代次回归。工作区 pdf 单元 143、集成 4、render 3 全部通过，0 忽略；差异检查通过。底层 EOF 行为有语义修正；P0 总体验收及后续阶段仍未完成。
- 2026-10-03 P0 对象流尾部校验补充：有界对象解析要求恰好一个对象，后续仅允许空白/注释，拒绝额外对象和零长度条目；保留 EOF 注释与 CRLF 注释处理。新增严格/兼容两模式额外数字/布尔对象、空条目、合法注释回归，以及输入上界在成功/失败后恢复并实际读取后续布尔对象的回归。工作区 pdf 单元 142、集成 4、render 3 全部通过，0 忽略；差异检查通过。整体输入安全、全局预算和后续阶段仍未完成。
- 2026-10-03 P0 对象流条目边界补充：SyntaxParser 增加临时 input_end 与 get_object_until，字符/前瞻/EOF/流块读取均遵守该边界，成功或错误后恢复原始上界；ObjectStreams 逐条目限制到下一 offset，最后条目限制到解码数据末尾，不复制条目数据。新增严格/兼容字符串、数组、十六进制串跨入下一条目拒绝回归，现有正常多对象/数字缓存回归继续通过。工作区 pdf 单元 140、集成 4、render 3 全部通过，0 忽略；差异检查通过。条目尾随非空白内容校验、总体预算及完整计划仍待推进。
- 2026-10-03 P0 对象流缓存补充：填充缓存时仅选择当前 xref 指向本对象流的压缩条目，跳过普通/空闲及其他对象流来源；校验第三字段索引与实际条目序号，全部选中条目验证完成后再提交缓存。对象流号由 u64 到 u32 改为检查转换。新增旧对象流内同号对象不覆盖当前普通对象、错误索引重复加载不留下部分缓存/流字节计费/加载状态的回归。工作区 pdf 单元 139、集成 4、render 3 全部通过，0 忽略；差异检查通过。对象流逐对象边界、完整资源预算与其余 P0/P1–P5 仍待完成。
- 2026-10-03 P0-08 对象流头部补充：relative offset + First 使用 checked_add，拒绝整数溢出；对象号必须非零且唯一，读取头部不得越过 First。新增严格/兼容两模式的极大偏移、重复/零对象号、头部越界及正常双对象头回归。工作区 pdf 单元 138、集成 4、render 3 全部通过，0 忽略；差异检查通过。压缩对象索引与缓存来源的一致性、对象边界及其余 P0/后续阶段仍待推进。
- 2026-10-03 P0 对象头校验补充：PdfIndirect 提供 id，新增 checked 间接解析入口；PDFContext 在缓存/流处理前校验请求对象号与代次和真实对象头一致，错误返回 InvalidReference。间接 Length 同样校验实际对象头。新增错误对象号/代次重复加载回归，验证失败不填充缓存且 loading 状态恢复。工作区 pdf 单元 137、集成 4、render 3 全部通过，0 忽略；差异检查通过。完整安全审查、资源预算和后续阶段仍未完成。
- 2026-10-03 P0 引用代次补充：CrossRefTable::lookup_id 对压缩对象仅接受代次 0，不再忽略引用代次；明确压缩条目的第三字段是对象流索引而非代次。新增普通对象正确/错误代次、压缩对象零代次及非零/极端代次、空闲条目和缺失对象回归。工作区 pdf 单元 136、集成 4、render 3 全部通过，0 忽略；差异检查通过。完整输入安全、流回退伪标记识别、总体预算及后续阶段仍未完成。
- 2026-10-03 P0 流兼容回退补充：错误 Length 校验后回到真实数据起点搜索，避免丢失前缀；回退仅裁剪结束标记紧邻的 CR/LF/CRLF，不再从倒数第二字节误判，搜索选择较早的 endstream/endobj 边界。新增缺失/偏短/超长 Length 与三种换行/无换行组合、首部换行保留和合法 Length 包含尾部 CR 的回归。工作区 pdf 单元 135、集成 4、render 3 全部通过，0 忽略；差异检查通过。启发式搜索仍可能遇到编码数据内伪标记，不能视为完整损坏文件恢复，P4-06 与总体计划未完成。
- 2026-10-03 P0-08 流换行与间接长度补充：可选 LF 消费集中到统一辅助函数，流起始/结束、字符串续行与行读取均传播非 EOF 前瞻错误；间接 Length 对象解析失败后仍恢复原始流位置。扩展故障 Reader 回归覆盖流起始/结束，并新增截断间接长度对象失败后继续读取原位置 token 的回归。工作区 pdf 单元 134、集成 4、render 3 全部通过，0 忽略；差异检查通过。其余安全路径、全局预算及 P1–P5 尚未完成。
- 2026-10-03 P0-08 行读取补充：read_line 仅把 EndofFile 当作正常结束，传播普通读取与 CR 前瞻错误；to_next_line 复用同一有长度限制的行读取逻辑，修复单 CR 后仅回退逻辑偏移而底层读取源未回退的问题。新增 CR/CRLF/LF 后首字节保持、末尾换行/无换行及故障 Reader 的行读取/跳过错误回归。工作区 pdf 单元 133、集成 4、render 3 全部通过，0 忽略；差异检查通过。完整 P0 验收及其余阶段仍未完成。
- 2026-10-03 P0-08 I/O 状态补充：peek_char_at 对越界返回 EndofFile，读取失败也恢复底层位置；set_pos 仅在 seek 成功后提交偏移；read_block 仅成功后推进偏移，部分读取失败回到块起点，并检查 usize 转换。新增注入读取/seek 错误及部分读取的 Reader 回归，验证失败后位置不变、后续字节和块读取正确。工作区 pdf 单元 132、集成 4、render 3 全部通过，0 忽略；差异检查通过。底层 seek 自身失败后的源状态由 Reader 实现决定；其余输入安全和整体计划仍未完成。
- 2026-10-03 P0-08/P0-10 底层读取分配补充：StreamReader::read_bytes 使用 try_reserve_exact 后初始化，容量溢出/分配失败返回 ResourceLimit，不再直接 vec 分配。新增不分配巨量内存的合成超大长度 Reader 回归，验证 usize::MAX 请求失败且读取位置不变；补精确末尾、超限不消费、空读取与失败后继续读取。工作区 pdf 单元 131、集成 4、render 3 全部通过，0 忽略；差异检查通过。此项不替代全局内存预算或其余输入路径审查，P0 与整体计划仍未完成。
- 2026-10-03 P0-08 流标记搜索补充：find_word_pos 在 EOF/读取失败时恢复起始位置，非 EOF 错误不再静默当作未找到；避免首次失败搜索使后续 endobj 搜索从文件末尾开始。EOL 检查不再无条件读取第二字节，支持末尾单 CR/LF，并避免位置加法溢出。新增缺失/部分标记、后续搜索位置保持、末尾 EOL 和极端偏移回归。工作区 pdf 单元 129、集成 4、render 3 全部通过，0 忽略；差异检查通过。完整异常输入审查及总体计划仍未完成。
- 2026-10-03 P0-08 字符串解析补充：八进制转义按 PDF 规则保留低 8 位，合法的 \\777/\\400 不再错误拒绝，并移除中间字符串分配；未转义 CR/CRLF 统一为 LF，显式 \\r 和续行规则保持不变。新增严格/兼容模式的溢出、1–3 位边界、非八进制字符、嵌套括号、三种换行和截断回归。工作区 pdf 单元 127、集成 4、render 3 全部通过，0 忽略。P0-08 与整体计划仍未完成。
- 2026-10-03 P0-08 CryptFilter 补充：CFM 类型与已知方法检查替代 unwrap，Length 使用精确非负 u32 整数读取，拒绝小数/负数/超范围。新增 6 种非法字段和正常 AESV3 字典回归。工作区 pdf 单元 126、集成 4、render 3 全部通过，0 忽略；差异检查通过。此项仅加固参数构造，不表示加解密已实现，完整安全处理仍属 P5-04；整体计划未完成。
- 2026-10-03 P0-12 CCITT 回归补充：将缺失外部文件而忽略的 Group4 测试替换为程序生成 17×4 条纹数据，逐像素检查两种 BlackIs1 极性，不引入第三方样本。K 参数由 i8 改为精确 i32，拒绝小数/超范围，避免正 K 截断误变 Group4；新增边界回归。工作区 pdf 单元 125、集成 4、render 3 全部通过，0 忽略；差异检查通过。真实独立 CCITT 样本及其他 P0-12 验收仍待补齐。
- P0-12 解码回归补充：新增 2,048 组五类流解码字节扰动，以 0–64 字节限额验证成功输出不超限，未触发 panic。修复 Predictor 非数值参数静默使用默认值和零 Columns/Colors 被接受的问题，拒绝非法位深，新增 6 种非法参数专项回归。工作区 pdf 单元 123、集成 4、render 3 通过，1 忽略；差异检查通过。更新 docs/testing.md；覆盖率引导 fuzz 和其他完整验收仍未完成。
- P0-12 开始：新增 parser_robustness 集成回归，1,024 组固定算法语法/内容扰动在严格/兼容模式下验证无 panic 和对象成功读取推进；256 组既有 PDF 截断/单字节替换经过文档/页数/页面/DisplayList 入口，使用小 Limits。新增 docs/testing.md 记录运行方法、来源和验证边界。工作区 pdf 单元 123、集成 2、render 3 通过，1 忽略；差异检查通过。覆盖率引导 fuzz、真实样本许可登记及独立验证尚未完成，P0-12 未勾选。
- P0 内容文字补充：TJ 在字体分支前校验数组和条目类型/有限数值，Tj/TJ 缺少选定字体时返回操作符错误，不再静默漏文字；修复 TJ 数值调整矩阵的纵向缩放系数错误为 0 导致后续文字 Y 坐标塌陷。新增无字体/非法数组页面回归和 Helvetica 文字数组调整后 Y 保持、X 推进回归。工作区 pdf 123、render 3 通过，1 忽略；差异检查通过。完整文字状态配对、数值累计溢出与资源限制仍待补齐，整体计划未完成。
- P0 字体编码补充：Differences 必须为数组且以 0–255 整数开始，拒绝小数/负数/越界码和非法条目；修复 254/255 连续 glyph 名称不推进的缺陷。BaseEncoding 检查名称，未知预定义编码明确不支持；Differences 优先参与 Unicode 映射，未知 glyph 用替代字符。SimpleFont 编码构造解析 BaseEncoding/Differences 和数组条目引用。新增高位连续编码、Unicode 覆盖及 9 种非法字典回归。工作区 pdf 122、render 3 通过，1 忽略；差异检查通过。完整 glyph 名称多字符映射/CJK 兼容性仍待推进，整体计划未完成。
- P0-08 TrueType 补充：Reader 用剩余长度检查替代加法边界，允许精确末尾读取并防止 usize::MAX 偏移溢出。cmap 检查声明长度、Format0 精确长度、Format4 segment 数/顺序/宽度/终止标记；修复 Format4 剩余长度漏减 6 字节表头、glyph 索引下溢/越界（无有效 glyph 返回 0）与非零 glyph 的 delta 漏应用。Format0 不再截断超过 255 的代码。新增正常映射、每字节截断、坏 offset、末尾读取和极端 reader 偏移回归。工作区 pdf 121、render 3 通过，1 忽略；差异检查通过。其他字体表及输入路径继续审查，P0-08 未完成。
- P0 页面几何补充：资源主要入口已使用可失败类型检查；进一步修复 Rotate 错误类型默认 0/小数截断及负角度未归一化，要求精确整数且为 90 倍数，支持负角和多圈。发现并修复 Rectangle::intersect 未写回结果及 X 上界误用 Y 的缺陷，CropBox 现实际影响页面尺寸；矩形解析检查有限坐标并归一化对角，页面拒绝空/非法交集。新增负旋转、多圈、非正方形裁剪及非法角度/空交集回归。工作区 pdf 120、render 3 通过，1 忽略；差异检查通过。完整 P0 与后续功能仍未完成。
- P0-08 内容类型补充：Operator 提供可失败的名称/数组/字符串读取，替换解释器对应 unwrap；Tr 要求整数 0–7，字体资源必须为字典，XObject Subtype 缺失/错误返回资源错误。虚线数组校验有限非负项、非负相位和非空数组非全零；M 要求有限且至少 1，不再错误类型默认 10。新增 12 种非法类型/范围的页面端到端回归，合法虚线和路径继续成功。工作区 pdf 119、render 3 通过，1 忽略；差异检查通过。其余资源/状态处理仍需审查，P0-08 未完成。
- P0-08 内容数值补充：解释器 27 处矩阵/文字位置/字号/路径等数值 unwrap 改为 Operator::get_as_f32 传播错误，该入口同时拒绝非有限数值。新增 11 种非法数值内容操作符的页面 display_list 端到端回归，并验证后续正常页面可解释。工作区 pdf 118、render 3 通过，1 忽略；差异检查通过。数组/字符串/名称操作数及资源字典等 unwrap 仍待审查，P0-08 未完成。
- P0-08 ICCBased 补充：构造解析外层对象与 N/Range/Range 数值引用，N 使用精确整数校验而非浮点截断；两元素数组检查替代原始 unwrap。Alternate 拒绝非法基空间及组件数不匹配，Range 拒绝反向区间，转换检查有限组件数与 N 一致。新增正常默认替代空间、间接 N 及 8 种非法参数回归。工作区 pdf 117、render 3 通过，1 忽略；差异检查通过。当前转换仍使用 Alternate，并非完整 ICC profile 色彩管理；后者属 P4-03，整体计划未完成。
- P0-08 Indexed 构造补充：检查四条目数组，hival 解析引用并限定整数 0–255（不再截断/饱和转换），拒绝 Indexed/Pattern 基空间，构造时检查 lookup 足够覆盖声明条目。转换要求单个索引，索引改为最近整数而非总向上取整。新增合法查找表、非法整数/类型/短表/额外参数与舍入回归。工作区 pdf 116、render 3 通过，1 忽略；差异检查通过。整体 P0 安全审查和完整功能计划仍未完成。
- P0-08/P0-11 图像渲染补充：移除手动 unsafe Bitmap 像素安装，使用 Skia 自有 Data 与可失败的 raster_from_data，失败返回 ResourceLimit；先完成转换/尺寸检查/图像创建，再 save/set_matrix，避免错误留下画布状态。新增截断图像失败后 save_count/矩阵保持不变、后续正常绘制及成功后状态恢复回归。工作区 pdf 115、render 3 通过，1 忽略；差异检查通过。当前 Skia Data 复制会短暂与 RGBA 缓冲区共存，仍需纳入后续全局峰值预算，整体计划未完成。
- P0-08/P0-10 图像缓冲区补充：PdfImage 保存整数像素尺寸并提供 pixel_dimensions，构造解析图像/尺寸引用；明确拒绝声明的非 8 位组件。rgba_buffer 改为 PdfResult，转换前校验数据长度与组件/尺寸一致，checked 输出大小并受 max_decoded_bytes 限制，分配失败返回 ResourceLimit；不支持的颜色转换不再返回空缓冲区。Skia 传播转换错误，使用整数尺寸并检查 i32 范围，避免浮点尺寸精度影响裸像素缓冲区读取。新增正常像素、截断/额外数据和 RGBA 精确限额回归。工作区 pdf 115、render 2 通过，1 忽略；差异检查通过。位打包图像、Decode/蒙版与更多颜色转换仍待实现，完整目标未完成。
- P0-08/P0-10 颜色递归补充：ColorSpace 构造加入独立 RAII 深度保护，所有递归颜色基空间/替代空间受 max_depth 限制，失败后恢复计数。Separation 检查四条目数组、解析名称引用、拒绝不合法替代空间；tint 必须为单个有限值并裁剪到 0–1。新增正常转换/畸形参数和间接 Indexed 自引用重复失败、失败后恢复回归。工作区 pdf 114、render 2 通过，1 忽略；差异检查通过。其他输入安全路径与全局预算未完成，P0-08/P0-10 保持未勾选。
- P0-08 CalGray 补充：与 CalRGB 复用定长数值数组读取器，解析参数与数组条目引用，拒绝非法/非有限白黑点、非正/非有限 Gamma 和错误字典/数组结构；转换先验证单个有限组件，再裁剪输入，非有限结果返回错误。新增正常数组/默认转换、组件错误与 9 种非法参数回归。工作区 pdf 113、render 2 通过，1 忽略；差异检查通过。安全校验不替代 P4-03 的完整色彩精度验收，P0-08 保持未完成。
- P0-08 CalRGB 补充：构造参数通过统一定长读取器解析字典/数组及条目引用，Gamma/Matrix/WhitePoint/BlackPoint 检查精确长度和有限数值，拒绝非正 Gamma、非法白点与负黑点；移除相关输入 unwrap。转换检查三个有限分量，按输入区间裁剪后求幂，非有限转换结果返回错误。新增合法数组、默认转换、空/NaN 组件和 9 种非法参数回归。工作区 pdf 112、render 2 通过，1 忽略；差异检查通过。此项验证安全性，不证明完整校准颜色精度；颜色管理完整性仍属 P4-03，P0-08 未完成。
- P0-08 Lab 解析补充：修复 ColorSpace 将完整 Lab 数组传入仅接受字典的旧构造器而 panic 的问题；构造器接受两元素数组或参数字典，错误类型返回 ColorError。文档入口解析参数字典和数值数组的引用；检查有限白/黑点、WhitePoint 的正 X/Z 和 Y=1、非负 BlackPoint 及有序 Range。新增合法数组/默认值和 9 种畸形参数回归。工作区 pdf 111、render 2 通过，1 忽略；差异检查通过。Lab RGB 转换仍明确不支持（P4-03），其他颜色构造路径继续审查，P0-08 未完成。
- P0-10 渲染配置补充：Limits 新增 max_render_pixels（1 亿）和 max_render_bytes（4 亿，单个 RGBA 页面表面），SkiaRender::with_limits 可接入同一 Limits，new 保留默认行为。尺寸先用 f64 计算并检查后端整数范围，再检查像素/字节预算后分配；初始化改为 1 像素，页面分配失败不修改已有表面和尺寸。新增精确限额、字节独立超限、DPI 缩放、超限后恢复和旧页状态保持测试。工作区 pdf 110、render 2 通过，1 忽略；差异检查通过。表面预算不包含 Skia 内部开销、换页时旧/新表面短暂共存和编码输出；全局峰值预算仍待实现，P0-10 未完成。
- P0-10 函数聚合补充：create_function 与直接 Stitching 构造为每次构造创建 max_objects 节点预算，所有分支共享并逐节点扣减；子节点数量在递归前检查剩余预算，补足单层/深度限制无法约束累计树大小的问题。新增超限/精确边界、真实间接自引用重复失败、失败后正常构造回归。工作区 pdf 110、render 1 通过，1 忽略；差异检查通过。此前函数总节点预算项已补齐；全局文档对象图聚合内存预算和渲染 Limits 统一仍未完成，P0-10 保持未勾选。
- P0 Sampled 兼容补充：Domain/Range/Size/BitsPerSample/Order/Encode/Decode 支持间接对象，数组内条目也解析引用。扩展回归覆盖二维角点与中点、反向 Encode，以及间接数组和间接数值组合，均与预期结果一致。工作区 pdf 109、render 1 通过，1 忽略；差异检查通过。前条记录中的 Sampled 参数引用与基础多维/反向 Encode 验证已补齐；函数总节点预算和其他安全路径仍待推进。
- P0-08/P0-10 Sampled 补充：校验 Domain/Range/Size 维度、有限有序区间和正整数 Size；样本乘积/角点位移 checked 计算并在分配前限制 max_objects/max_decoded_bytes。BitsPerSample/Order 使用整数检查，Order 非 1 明确拒绝；Encode/Decode 校验长度，默认 Encode 上界修正为 Size-1。样本按预期数量读取无符号位（修复 32 位符号问题），截断数据返回错误；修复双重归一化、插值权重反向及最后样本边界，求值拒绝非有限输入/输出。新增默认域非 0–1 插值、端点/裁剪、32 位样本及畸形/超限/截断用例。工作区 pdf 109、render 1 通过，1 忽略；差异检查通过。多维/反向 Encode 兼容样本、参数引用解析及函数总预算仍待补齐，P0-08/P0-10 未完成。
- P0-08/P0-10 函数构造补充：Type3 检查字典、单个有序 Domain、非空 Functions、Bounds 顺序/域内位置和 Encode 长度，解析参数与子函数引用；子函数构造深度受 max_depth 限制，子函数数量受 max_objects 限制，防止循环引用无限构造。Type4 按真实流字典读取参数，拒绝非流及非有限/无序/不成对 Domain/Range；两个求值功能仍显式不支持，完整实现留 P4-03。新增无效构造及嵌套超限回归。工作区 pdf 108、render 1 通过，1 忽略；差异检查通过。函数总节点预算与 Sampled 安全性仍需推进，P0-08/P0-10 未完成。
- P0-08 PDF 函数补充：create_function 解析引用并从字典/流统一获取 FunctionType，拒绝非整数类型；Type2 构造不再 unwrap，检查单个 Domain 有序区间、有限参数、C0/C1 等长非空和 Range 输出维度。修复未指定 C0/C1 时产生零输出的错误，默认各一个分量；求值检查输入尺寸、默认未初始化结构、非有限值及结果溢出，拒绝 Domain 内未定义的幂运算。新增默认值、裁剪、多输出和畸形参数回归。工作区 pdf 107、render 1 通过，1 忽略；差异检查通过。Sampled/Stitching/PostScript 内部构造仍需审查，P0-08 未完成。
- P0-08 颜色 API 补充：ColorSpace::default_value/number_of_components 与 ICCBased::default_value 改为 PdfResult，未支持的 DeviceN/Pattern 查询及非法 ICC 组件数返回错误，移除对应 panic/unimplemented；Indexed 传播组件数量错误。DeviceGray/RGB/CMYK 在读取数组前验证精确组件数量和有限数值。新增默认值/正常转换、未支持查询、空/多余组件和 NaN 回归。工作区 pdf 106、render 1 通过，1 忽略；差异检查通过。此低层 API 有返回类型变更；其他颜色参数解析及函数模块仍待加固，P0-08 未完成。
- P0 语法回归补充：修复数字 token 在 EOF（含尾部空白/注释）被间接引用前瞻错误拒绝的问题；EOF 前瞻恢复数字末尾位置，不吞后续独立数字，仍校验真正引用的对象号/代次。新增严格/兼容末尾整数、实数、大整数、双数字和有效/非法引用用例。工作区 pdf 104、render 1 通过，1 忽略；差异检查通过。前一记录中的末尾独立数字问题已解决；完整 P0 验收仍待剩余安全路径、资源预算和样本集完成。
- P0-08 SimpleFont 补充：Subtype/BaseFont/FontDescriptor 检查类型，字符边界解析引用且限定整数 0–255；Widths 校验数组、必需首尾字符及精确长度、有限数值和条目引用，避免固定数组越界。缺失 ToUnicode 映射返回替代字符；Unicode/text_widths 拒绝超出简单字体范围的代码，累计宽度溢出返回错误。新增错误宽度数组、合法边界、非法字符边界及映射缺失回归。工作区 pdf 103、render 1 通过，1 忽略；差异检查通过。测试还发现独立数字 token 在文件末尾返回 EndofFile，语法解析后续需补专项回归；P0-08 未完成。
- P0-08 FontDescriptor 补充：Flags 改为完整 u32（flags() 返回 Option<&u32>），拒绝负数/小数/非数值，修复 symbolic 位判断反向；11 个度量字段解析引用并检查有限数值，嵌入字体必须为流，多个 FontFile 条目明确拒绝。新增非法字段、字体流类型与高位 flags 回归。工作区 pdf 102、render 1 通过，1 忽略；差异检查通过。SimpleFont 宽度和 Unicode 查找等输入路径仍待审查，P0-08 未完成。
- P0-08/P0-10 CIDFont 宽度补充：水平/竖排宽度共用安全解析，解析引用，校验非负整数 CID、有限数值、完整三元组及范围；包含范围上界，拒绝反向范围、CID 溢出和累计展开超过 max_objects，移除内部 unwrap。text_widths 通过 char_width 使用 DW 回退，无 DW 时使用 1000 默认值，累计溢出返回错误。新增正常数组/范围/竖排与 11 种异常数组回归、默认宽度回归。工作区 pdf 101、render 1 通过，1 忽略；差异检查通过。字体 Descriptor 与其他模块仍需继续审查，P0-08/P0-10 未完成。
- P0-08 字体字典补充：Type0 BaseFont 解析引用并检查名称；DescendantFonts 检查数组、恰好一个条目和后代字典类型。CIDFont Subtype/BaseFont、DW/DW2、W/W2 与 FontDescriptor 外层类型错误不再 unwrap，未知后代子类型明确拒绝。新增 12 种畸形字典回归。预定义 CMap 也接入文档 max_objects，并验证自定义低限额拒绝及默认 Identity-H 成功。工作区 pdf 99、render 1 通过，1 忽略；差异检查通过。CIDFont 宽度数组内部及 Descriptor 等路径仍待审查，P0-08 保持未完成。
- P0-08/P0-10 CMap 范围补充：CID/bfrange 改为包含上界，拒绝反向范围和 CID/目标字节溢出；bfrange 支持 UTF-16BE 代理对、多字符目标与数组目标，不再把目标直接当 Unicode 标量。累计展开条目受 max_objects 限制，Type0 编码流及两类字体 ToUnicode 继承文档 Limits；ToUnicode 非流返回字体错误。修正旧样本遗漏端点的数量断言，新增包含上界、代理对、多字符数组、超限及精确限额回归。工作区 pdf 97、render 1 通过，1 忽略；差异检查通过。预定义 CMap 的文档限额传递、usecmap、完整语法及其他未实现路径仍需推进。
- P0-08 CMap 解析补充：修复尾部空白/注释、未闭合字典/数组的越界，十六进制 token 不再额外跳过下一字节；必需条目缺失返回带偏移字体错误。字符码使用经校验的 1–4 字节十六进制或整数解析，避免溢出；Unicode 目标字符串校验边界与十六进制数字，奇数位按末位补零。新增截断/非法数字/超长码和相邻 token 回归。工作区 pdf 95、render 1 通过，1 忽略；差异检查通过。范围映射、完整语法拒绝策略和解析数量限制仍待推进，P0-08 未完成。
- P0-08 继续推进：Type3、Type1 程序解析、未知 TrueType cmap 格式、Lab/Pattern、PostScript/Stitching 函数及安全处理器的部分未实现入口改为明确不支持错误。Indexed 转换拒绝空/非有限索引及截断 lookup，默认值修正为单个索引 0；ICCBased Range 长度检查不再截断为 u8，非数值/非有限条目返回错误。未知数组颜色空间与未支持 RGB 转换不再 panic。新增 Indexed 回归通过；工作区 pdf 94、render 1 通过，1 个既有样本缺失测试忽略。其他 panic/unwrap 仍待审查，P0-08 保持未勾选。
- 2026-10-02：开始 P0。加入 GitHub Actions 工作区测试配置；本地平台 Darwin x86_64，rustc/cargo 1.96.0。工作区包含 FreeType、mozjpeg 和 Skia 原生依赖。
- P0-02 已验收：CMap 无效/截断编码返回带偏移错误，修复边界越界，支持 1–4 字节代码空间；Type0 传播错误；新增无效/截断和变长编码测试。
- P0-07 已验收：保存数字保留 lexeme，拒绝非法数值表达；名称按字节转义；输出加入二进制头注释；保留源文件 Header 版本与 Catalog /Version，不进行自动降级。流 Length/Trailer Size 按整数文本创建；大整数、非有限数值、特殊名称和源版本回归测试通过。
- P0-03 已验收：读取与编辑共用 PageTree，按 Kids 深度优先；继承属性解析加入循环/深度检查；校验每个 Pages 的 Count；新增混合深度顺序、循环、继承与错误 Count 回归测试。提供 try_page_count 返回结构错误，编辑入口不再吞掉错误。
- P0-05 已验收：resolve/resolve_owned/resolve_object_id 均读取最新修改和删除；新增对象可通过统一入口读取；resolve_original_object 显式读取原始快照。字体与颜色调用链已适配 owned 返回值，此低层 API 有签名变化。
- P0-06 已验收：页面删除/内容替换只解除相关引用，保守保留旧内容对象；真正回收由 P2-07 显式对象图清理处理。共享流在替换、删除、保存及重读后仍可访问与显示；既有自动删除测试改为验证保留策略。
- P0-11 已验收：提供 edit_transaction；页面增删、内容替换/追加自动事务化；错误和 unwind 恢复修改记录与对象编号分配，支持嵌套回滚点。Device 所有回调返回 PdfResult，Interpreter 和 DisplayList 均传播错误并停止；Trace XML 写出传播 I/O 错误。Skia 页面分配、字体加载、颜色转换与 PNG 保存返回错误；新增设备开始/绘制/结束失败、解释器失败及 Skia 无效参数/保存失败测试。当前事务使用修改记录快照，成本随已修改对象量增长；现有缓存仅保留原始对象，派生修订缓存留待引入缓存时实现。
- API 迁移：Device 实现必须返回 PdfResult<()>；SkiaRender::new 与 save 改为返回 PdfResult，示例已适配。渲染页面临时限制 1 亿像素，统一可配置 Limits 仍属 P0-10；其他未实现颜色/字体内部路径的 panic 清理仍属 P0-08。
- P0-10 部分完成：提供 ParseOptions/Limits 与 open/from_bytes/from_reader 的 options 入口；语法对象嵌套、页面树深度/节点数、加载后对象数量、Flate 解压输出及原始未过滤流大小、图像像素数已接入限制。默认对象数 100 万、深度 128、每流解码 256 MiB、图像 1 亿像素。字节读取在分配前检查剩余输入长度。新增超限、精确边界与深度恢复测试通过。其他解码器、xref 分配前检查、对象流内部解析、内容/Form 递归及聚合内存限制仍需补齐，因此未勾选。
- P0-04 已验收：统一 xref 表/流历史链，支持混合历史与 XRefStm 补充流；循环偏移返回 InvalidReference，历史深度/合并数量受 Limits 限制。新版本条目保留优先级，同版本补充流覆盖表条目；修复旧版本覆盖新版本的缺陷。对象加载加入 RAII 循环/深度保护；压缩对象自引用失败后加载栈恢复。页面树保护已在 P0-03 接入。新增 Prev 自循环、混合历史、混合引用优先级、数量/字段宽度超限与对象加载循环测试通过。
- P0-10 继续推进：xref Size/Index/子表数量与范围在迭代前校验，字段宽度防止算术溢出；未压缩 xref 字节复制前检查大小。ObjectStreams 继承文档模式与 Limits，检查 N/数据大小并限制内部语法深度。其他解码器、内容/Form 递归和聚合内存仍未完成，任务保持未勾选。
- P0-10 解码与内容补充：LZW/RunLength/ASCIIHex/ASCII85 在追加输出前检查限额；JPEG 从头部检查真实像素与组件输出大小；CCITT 检查行/声明尺寸，超限停止消费输入且不继续增长输出；Crypt Identity 复制前检查大小。修复 ASCII85 32 位溢出/非法尾组和 PNG predictor 行大小乘法及超大行缓冲区。页面内容累计大小受限，内容语法继承文档 Options；流解码深度使用 RAII 恢复，阻止循环 JBIG2Globals。新增限额、精确边界、JPEG/CCITT 预检查与递归恢复用例通过。原始流读取、总体预算、状态栈与未来 Form 执行限制仍待完成；P0-10 未勾选。
- P0-10 原始读取与执行补充：新增 max_raw_stream_bytes（256 MiB）、max_cached_stream_bytes（512 MiB，仅原始缓存流）、max_content_operations（100 万）、max_token_bytes（16 MiB）。流长度与 endstream 搜索受限，修复长度加法溢出；语法名称/普通词/字符串/行和数组/字典受限；内容操作数与图形状态栈受限，Q 下溢返回明确错误。缓存超限不提交该流，重复失败不重复计费。新增原始长度/扫描回退、缓存恢复、token/容器、操作数量和状态栈用例通过。整个对象图的预算、渲染参数统一和未来 Form 执行仍待补齐，未勾选。
- P0-09 已验收：内容解析保留未知操作符及操作数/字节偏移；兼容模式忽略该操作及其操作数并记录诊断，严格模式返回 UnsupportedFeature。Interpreter::diagnostics 与 DisplayList::diagnostics 可查询，偏移基于拼接后的页面内容。ri/gs、未实现标准操作符及 Pattern SCN/scn 走相同策略；非 Pattern SCN/scn 正常设置颜色值。内联图像无法可靠跳过数据，两个模式均显式拒绝，完整实现仍属 P4-01。新增操作符保留、兼容后续绘制、严格拒绝及内联图像拒绝用例通过。
- 严格模式往返测试发现并修复 xref 子表头后 EOL 未消费、合法尾随空格被拒绝的问题；本库输出可用于新增严格模式端到端测试。
- P1-09 已验收：非 null Encrypt Trailer 的保存返回 UnsupportedFeature，发生在加载对象/写出前。新增模拟 Encrypt 文档测试通过；完整密码读取和加解密仍属 P5-04。
- `cargo test --workspace --locked --quiet`：pdf 93 通过、render 1 通过、0 失败、1 忽略（既有 CCITT 样本缺失），包括 Skia 后端及示例编译；`git diff --check` 通过。CI 配置已有，但尚未在远端执行，P0-01 保持未勾选。

## 目标与范围

首个可交付版本为 v0.1：限定支持范围内的新建中英文、图片和路径 PDF、读取与基础提取、页面增删/复制/重排/旋转/裁剪、合并/选页拆分和可靠保存。正式支持需逐项验收；水印和覆盖内容先作为可选项，不阻塞 v0.1。详见 [v0.1 清单](docs/releases/v0.1.md)。

后续版本：内容级编辑、复杂渲染、批注、表单、附件、加解密、增量保存。PDF 2.0、PDF/A、PDF/UA、数字签名和 OCR 单独定义支持矩阵与里程碑；OCR 属于可选扩展。

## P0：稳定基础与架构收口

- [ ] P0-01 建立 `cargo test --workspace` 基线与 CI；记录平台、工具链、原生依赖和失败原因。
- [x] P0-02 修复 CMap 无法识别字节时不推进偏移的问题；增加无效编码回归用例。
- [x] P0-03 用统一 PageTree 服务替换读取和编辑的两套遍历；按 Kids 顺序深度优先遍历；验证继承属性和 Count。
- [x] P0-04 给页面树、对象引用和 Prev 链增加循环检测与深度/数量上限；补混合 xref 与 XRefStm 用例。
- [x] P0-05 统一对象解析，所有普通读取均能看到最新修改和删除；明确原始对象读取接口。
- [x] P0-06 删除页面或替换 Contents 时先解除引用；禁止直接删除可能共享的内容对象；增加共享流回归用例。
- [x] P0-07 保留数字整数精度；实现名称转义、合法实数输出、二进制头注释及版本策略；补大整数和特殊名称用例。
- [ ] P0-08 替换外部输入可触发的 panic/unimplemented；增加 UnsupportedFeature、InvalidReference、ResourceLimit 等错误及上下文。
- [x] P0-09 未支持操作符产生可查询诊断；严格模式返回错误，兼容模式按明确定义的策略继续。
- [ ] P0-10 引入 ParseOptions/Limits，限制解压字节数、图像尺寸、对象数量与递归深度。
- [x] P0-11 建立修改事务，失败不留下部分更新；设备回调返回 PdfResult，传播渲染错误。
- [ ] P0-12 建立真实文件、合成异常文件与读写回归样本集，记录样本来源与许可；加入解析/解码模糊测试。

验收：现有测试通过；新增缺陷用例通过；嵌套页面顺序正确；编辑立即可见；共享对象不被误删；已覆盖的异常输入不崩溃、不死循环；CI 能复现结果。

## P1：创建与保存闭环

依赖：P0 对象解析、页面树、数字序列化与错误模型。

- [x] P1-01 解耦输入源与文档状态，提供无需先解析 PDF 的新文档 API；初始化 Catalog、Pages 和 Trailer。
- [ ] P1-02 提供 PageBuilder：自定义尺寸、旋转、MediaBox/CropBox 和页资源。
- [ ] P1-03 提供 ResourceManager：字体/图片注册、名称分配、同文档资源复用。
- [ ] P1-04 扩展 ContentBuilder：路径、变换、线型、图像、文字状态；检查数值和图形/文字状态配对。
- [ ] P1-05 实现字体嵌入、中文 CID 字体、字符编码及 ToUnicode；禁止将任意 UTF-8 直接当作字体字符码。
- [ ] P1-06 实现常见图片嵌入、透明通道与压缩配置。
- [ ] P1-07 提供文档信息/XMP 接口；定义二者一致性策略。
- [ ] P1-08 增加 SaveOptions、save_to_writer、输出版本/压缩策略；文件保存采用临时文件写完再替换。
  - [x] 本地模拟write/sync/replace返回错误：旧目标保留、新目标不创建，临时文件清理；同步失败不进入替换。真实磁盘故障、断电目录持久性与跨平台替换仍需单独验收。
  - [x] 本地Rust unwind：新建/已有目标在write/sync及实际rename之前panic时，先关闭句柄后清理临时路径、原目标不变；不承诺成功rename后撤销或abort/断电恢复。
- [x] P1-09 对未支持的加密文档保存显式拒绝，防止生成与 Encrypt 字典不一致的输出。
- [ ] P1-10 提供新建中英文多页文档、插图和保存示例，更新 README 支持矩阵。
  - [x] 固定静态Noto Sans SC两页中英文、RGB/RGBA插图及可选中文水印/页码组合：本库严格读回、独立结构/提取/目视检查通过，README说明已同步；只覆盖此样本，不代替完整P1矩阵。

验收：从零创建的多页文件由本库读回，并通过独立结构检查工具和阅读器验证；文本可提取；字体与图片正确显示。记录验证工具版本。

## P2：常用读取与页面编辑

依赖：P1 创建/资源/保存闭环。

- [ ] P2-01 提供文本提取接口，输出字符/文本段位置、字体、方向；阅读顺序与原始绘制顺序分别定义。
- [ ] P2-02 提供图片原始数据与解码数据提取接口，保留编码和尺寸信息。
- [ ] P2-03 页面重排、复制、旋转与裁剪；统一 0 起始索引和错误语义。
- [ ] P2-04 实现跨文档对象图复制：旧 ID 到新 ID 映射、循环处理、继承资源显式化和资源名称冲突处理。
  - [x] 固定真实静态Noto Sans SC与RGB/RGBA软遮罩页面导入→复制→重排→源释放→保存读回组合：独立结构/文本及逐页渲染一致性通过；不代替循环、继承属性或完整失败矩阵验收。
- [ ] P2-05 在对象图复制之上提供合并、拆分和页面导入。
  - [x] 固定静态Noto Sans SC与RGB/RGBA软遮罩样本逆序拆分及两源合并：源释放后保存/严格重开，独立结构、精确文本/页序及七页渲染一致性通过；不代替所有页面树/归属/失败矩阵。
- [ ] P2-06 提供水印、页码与覆盖内容；隔离图形状态，避免受原页面状态影响。
- [ ] P2-07 提供显式可达性清理；保留未知扩展对象，定义删除策略。
- [ ] P2-08 增加端到端编辑回归和独立结构/渲染验证。

验收：编辑后页面顺序、引用、继承属性和资源正确；操作失败可回滚；合并拆分后的文字和图像可正确读取与渲染。

## P3：内容级编辑

依赖：P2 对象图与资源管理。

- [ ] P3-01 引入保留操作符、操作数、资源引用、流边界和来源位置的 ContentProgram；未知操作符可保留。
- [ ] P3-02 定义文本/图片/路径的来源映射、坐标系、命中测试和稳定定位方式。
- [ ] P3-03 提供文字替换与删除，明确字体覆盖、字距、换行和布局边界。
  - [x] 固定自动补偿矩阵：Courier Tj/两类引号及Identity-V/W2合成独立数值回归，真实静态Noto Sans SC水平整Tj补偿/无补偿对照经独立坐标/文本/目视验收；不代替真实竖排、全部字体、TJ数组、嵌套或排版适配。
  - [x] 固定Courier TJ单项补偿矩阵：首/中/末项与删除/增长/空格，原数值词法/注释、数组内邻接/后续事件/下一行位置，共享页/严格重开及错误/预算/unwind回归通过；仅此水平字体矩阵，不代替TJ跨项、嵌套或独立阅读器验收。
  - [x] 固定Courier TJ跨项范围补偿：跨三字符串、首尾部分保留、单项及零宽插入，逐项独立数值/后缀/后续文字位置、词法数字/注释、共享页/严格重开、预算所有权和外层错误/panic回滚通过；不代替复杂字体/嵌套或独立阅读器矩阵。
  - [x] 固定Courier嵌套推进补偿：两层四调用中单次Tj/引号/TJ单项/跨项长短删除，继承与局部q/Q字号/字距、独立调整/后缀/后续事件/下一行位置、共享页/旧Form/严格重开、外层错误/指定panic/额度拒绝与刷新重试均通过；不代替复杂字体、独立阅读器或完整P3。
  - [x] 固定混合字符码嵌套补偿矩阵：合成Type0一/两字节横竖排W/W2、五类目标、双字节空格、保留后缀、独立调整、严格重开/共享页/多字节边界拒绝通过；不代替实际竖排字形或全部字体。
  - [x] 固定真实静态Noto Sans SC两层Form整Tj补偿对照：qpdf结构/共享FontFile2、MuPDF独立P/D/F坐标/归一化文本、四页目视、同DPI顶层逐页PNG cmp及错误模式/交换对照页负例通过；不代替TJ范围、真实竖排、跨事件或完整P3。
  - [ ] 连续文字推进补偿：显式TJ预算原语、同池有效字体/字号/Tc/Tw/Tz与q/Q/gs/引号状态、新旧完整字符码度量及顶层及嵌套单字符串/TJ单项/跨项范围事务入口已接入；当前补偿专项14项、嵌套补偿3项及PDF单元636项通过。仍需跨文字事件、嵌套复杂字体/独立渲染、真实竖排/完整字体矩阵、完整unwind及独立渲染；不能以单字符数或Td估计/补偿，保留结束位置不代表字形装入原绘制边界。验收包括后续事件位置、行矩阵不变、共享隔离及失败所有权。
  - [x] 固定真实静态Noto Sans SC单Tj已映射中文字范围替换：复制后原页隔离、未映射字符拒绝、外层错误回滚/重试、独立结构/精确文本/目视验证通过；不代替新增字形、连续推进或复杂字体矩阵。
- [ ] P3-04 提供图片替换与移动、路径修改；处理共享 XObject 的复制后修改。
  - [x] 固定edit_content两页对照独立验证：qpdf结构检查、MuPDF文字提取及逐页渲染确认文字替换、图片旋转/单次删除、红路径删除和原页隔离。仅覆盖Helvetica英文与顶层图片/路径，复杂字体/Form/内联与完整视觉矩阵仍未验收。
- [ ] P3-05 实现字体子集化与字符映射更新；确保修改后文本提取正确。
  - [x] 固定反向Arial水平→NotoSC竖排B双CID双层跨调用：36066生成/严格重开及原共享父子Form隔离，95571十四XML负例，66059/25846四字体源GID/复合闭包/轮廓/度量/保留表/校验和/许可及三字体负例，72464源字典/八负例均退出0；qpdf结构、源绝对坐标/后缀补偿与第三调用位置、三个Identity-H/W及尾Identity-V/W/W2、三页96dpi实际逐页目视无缺字/重叠/裁切均通过。样本output/pdf/rzpdf-latin-subset-horizontal-vertical-nested-span-edit.pdf，日志/tmp/rzpdf-reverse-span-{generate,negatives,fonts,integrated,dictionary}.log。仅固定Tj/24pt与18pt/Tc场景，不代表全部状态/shaping/字体类型或完整P3-05。
  - [x] 固定NotoSC竖排→Arial水平双CID双层跨调用：79432生成/严格重开与共享父子Form隔离、19646坐标字典、83631四TTF源语义/轮廓/度量/保留表/校验和/许可及三字体负例、28113最终方向负例均退出0；qpdf、逐字符绝对坐标/各段书写方向/尾18pt与2Tc推进、十五XML负例、三个Identity-V的W/W2及一个Identity-H的W源度量与七字典负例、三页96dpi实际目视无缺字/重叠/裁切均通过。输出output/pdf/rzpdf-cjk-subset-vertical-cid-nested-span-edit.pdf，日志/tmp/rzpdf-vertical-cid-span-{generate,independent,fonts,final}.log。28113同时回归Courier混合十五XML/四字典负例通过。不代表反向水平→竖排或全部字体/状态矩阵。
  - [x] 固定真实NotoSC单层竖排跨调用：77678生成退出0，73314字体/字典联合校验退出0，69725最终绝对坐标复验退出0；三页严格重开/共享Form隔离、qpdf、源度量绝对及相对坐标、十一XML/四字典/三字体负例、三TTF语义与许可、三页96dpi逐页目视通过。样本output/pdf/rzpdf-cjk-subset-vertical-span-edit.pdf，日志/tmp/rzpdf-vertical-span-{generate,independent,absolute}.log。仅固定直接CID/Tj场景。
  - [x] 固定NotoSC竖排→Courier水平双层跨调用：69259生成、7758坐标/字典、33526抽取字体联合校验均退出0；三页严格重开与原Form隔离、qpdf、各段wmode/dir/字体/字号、竖排源原点及水平尾18pt/2Tc绝对位置/补偿、十三XML/四字典/三字体负例、三TTF源语义/保留表/许可、三页96dpi实际目视通过。样本output/pdf/rzpdf-cjk-subset-vertical-mixed-nested-span-edit.pdf，日志/tmp/rzpdf-vertical-mixed-span-{generate,independent,fonts}.log。6091校验器旧水平Arial/水平混合Courier/单层竖排/双层竖排回归退出0；不代表双CID混合尾源、反向模式或完整字体矩阵。
  - [x] 固定真实NotoSC双层竖排跨调用：edit_subset_span --cjk --nested --vertical三页自然/补偿对照，原父子Form/共享页隔离与严格重开文字、qpdf结构、精确文字/字体/字号/源vmtx推进/补偿及第三调用坐标、十XML负例、三抽取字体源GID/轮廓/度量/校验和/保留表/许可及三字体负例、Identity-V/W/W2源度量及四字典负例、三页96dpi逐页目视均通过。生成81856、字体36980、字典41255退出0；日志/tmp/rzpdf-vertical-nested-span-{generate,independent,dictionary}.log，样本output/pdf/rzpdf-cjk-subset-vertical-nested-span-edit.pdf。仅此固定Tj直接CID双层场景，不代表单层、混合书写、shaping或完整P3-05。
  - [x] 字体W/W2构建预算收口：checked_mul预检数值对象数/容量字节（W2每CID三对象）并使用显式Limits；预算化入口持有实际数组capacity/扩容共存所有者至发布完成。水平W与竖排W2共用机制，不以Unicode字集数量或字体字节预算代替；补精确/少一字节、相邻预算保留、失败不修改文档及释放重试。旧无预算API保留契约，明确尚未入池的完整字体解析/文档常驻对象成本。
    - [x] 竖排公开子集注册聚合预算边界：public_subset_registration_budget_boundary_rolls_back_and_retries现覆盖水平/竖排入口，使用有效vhea/vmtx字体分别测峰值；精确额度成功、少一字节失败且文档字节不变、相邻一字节租约导致失败且保留、释放后重试成功、返回live=0及严格重开Identity-V/W/W2均通过。最终专项95089退出0，日志/tmp/rzpdf-metric-vertical-boundary-final.log；仅此公开注册边界，不代替全部字体/自动编辑矩阵。
    - [x] 同时计入每个PdfNumber的lexeme字符串实际capacity与非空W/W2的外层数组，不能只计Vec<PdfObject>槽位。核对embedded_font.rs发现旧number路径创建拥有String的PdfNumber；构造器须在分配前保留额度，实际容量核对后持有租约直至事务发布/回滚结束。预算从子集入口显式传入共同内部构造路径，旧整字体无预算入口保留兼容契约；发布后的常驻成本与操作预算边界需写明。预分配固定长度数组避免逐项增长，同时覆盖字符串分配失败、水平/竖排合计额度及失败后重试。
    - [x] 租约所有权覆盖自动编辑的外层提交：subset_text_edit的PreparedSubsetEdit/PreparedSubsetFragment已经持有EmbeddedSubsetFontObjects，可复用该所有权链携带度量租约，不能在embed_truetype_font_impl内部字体注册事务返回时提前释放。公开注册入口需明确何时转为文档常驻内存，内部编辑入口须测试字体注册成功而后续内容提交失败的释放与回滚，以及后续分配与字体数组共存的预算峰值。
  - [x] 固定真实双CID源混合双层跨调用：Arial→NotoSC尾B和NotoSC→Arial尾B，独立TAILAA子集/18pt/2Tc，首源局部新子集/24pt/1Tc；严格重开、原父子Form隔离、qpdf、尾源FontTools度量的精确文字/坐标、八抽取字体语义/保留表/校验和/许可声明及各三字体/十XML负例、六页逐页目视通过。仅此水平Tj固定范围，不代替完整字体/状态/竖排矩阵。
  - [x] 固定真实Arial/NotoSC→Courier双层混合跨调用：不同字体/字号/字距及双字节CID→单字节尾码，三页自然/补偿对照；严格重开、原Form不变、qpdf、精确文字/字体/坐标与Courier尾段-12.8pt度量、六抽取字体及三字体/八XML负例、六页目视均通过。仅此水平Tj固定范围，不代替完整混合字体或竖排矩阵。
  - [x] 固定真实Arial/NotoSC双层跨调用自动子集：`edit_subset_span ... [--cjk] --nested`生成共享父子Form三调用、原/自然/补偿三页对照；严格重开、父子原对象不变、qpdf、独立精确文字/字体/字号/坐标、六份抽取字体语义/校验和/许可声明与负例、六页逐页目视均通过。抽取目录及终态见当前摘要。仅双层水平Tj固定范围，混合字体、竖排和完整P3-05仍待验收。
  - [x] 公共子集入口固定12场景表策略：ZZZZ/fvar/gvar/COLR在两种策略下拒绝且不改文档，GSUB只在显式省略时接受，DSIG省略准确报告，预算释放正常。证据`/tmp/rzpdf-subset-public-table-policy.log`退出0；不表示变量/颜色/布局字体表已实现，不代替完整SFNT独立验收。
  - [x] 固定合成 TrueType 跨调用自动子集八场景：自然/补偿 × BB/空删除 × 单/双层共享 Form；六种非法 CID 端点拒绝、预算释放、指定 error/panic 字节回滚、原父子 Form/另一页/第三调用隔离、严格重开文字和后缀坐标均通过。证据 `/tmp/rzpdf-cross-call-subset-multilevel.log` 退出 0；仅此矩阵，不代替真实字体、混合尾字体或完整 P3-05。
  - [x] 固定真实 Arial/NotoSC 跨调用自动子集独立验收：三次共享 Form 分离位置样本，跨前两调用保留起末后缀，原页/自然/补偿三页对照；严格重开、qpdf、MuPDF 精确文本/字体/字号/前后坐标、六份抽取子集字体语义/校验和/许可及各七 XML/两字体负例、六页逐页目视均通过。证据 /tmp/rzpdf-real-subset-span-{latin,cjk}-independent.log 退出 0，中文会话 42495 已退出 0。仅此固定单层水平 Tj 范围，不代替真实多层/混合尾字体/竖排；不声称跨调用文本自动重排。
  - [x] 固定Arial/NotoSC跨TJ新增字符三页对照：原共享字体页面、自然推进、补偿推进；跨三字符串到末项结束替换，原数值/注释保留，严格重开、qpdf、MuPDF精确文字/字体/字号/前缀及后续事件位置、六页目视、六份抽取子集字体语义/校验和/许可与XML/字体负例通过。仅此固定范围，不代替真实保留末端后缀、竖排、全边界或跨事件/Form矩阵。
  - [x] 范围规划器与一次发布架构：subset_text_edit共用build_scoped_subset_text_fragment及作用域范围构造器，复用text_event_edit原索引片段表；后段按各自原字体先规划旧→空，首段局部新子集及旧→新补偿随后准备，页面/同leaf Form/跨调用树共用事务发布。Owned计划、字体、绑定、复制scope与表预算保留到发布结束，字符和展开/编辑字节累计检查；不逐事件公共提交，不刷新快照绕过原选区认证。1048项工作区基线及702项PDF单元联合回归覆盖原索引/多层混合状态/共享资源/预算/error/panic回滚与刷新重试，新增聚合额度专项通过。此项仅验收规划发布架构，真实字体完整矩阵和P3-05总项另行验收。
  - [x] 固定静态Noto中文两页子集：44标量/45保留GID，独立FontTools校验和/映射/轮廓/度量/指令/许可与MuPDF精确文本、96DPI视觉通过；六检查器负例在Python -O下拒绝。不代表通用字体或自动覆盖扩充。
  - [x] 固定本机Arial实际复合子集：17标量/26保留GID/5所选复合，组件原GID/轮廓/指令/许可、独立文本/渲染与六负例通过；系统源未改动、不入库。不代表所有复合flags、shaping或其他字体。
  - [x] 固定单事件新增字符资源隔离：两页AA共享A子集，只在第一页首Tj切换B子集并恢复原字体，严格重开BA/AA、原字体及另一页资源源码不变、外层失败回滚/重试专项通过；仅显式操作序列流程，不是自动字体扩充或通用选区入口。
  - [x] 保留现有embed_truetype_font整字体/不可变字集契约，另设embed_truetype_subset_font显式子集入口；整字体与子集内部参数分别为subset=false/true，不静默扩充既有句柄或共享页。公共注册往返/拒绝不变、新字符局部资源隔离及真实样本均有证据，当前1048项完整门禁覆盖生产实现；不代表全部字体类型或完整P3-05。
  - [x] 子集权限预检：公共入口和build_subset均调用validate_outline_embedding(true,true)，公共注册测试以OS/2 v3的0x0100禁止子集位验证拒绝、保存字节及预算不变；702项联合回归该测试通过。静态TrueType为支持范围，公共表策略测试验证变量/颜色/未知表显式拒绝，必需glyf/loca检查不将CFF当作TrueType子集。许可仍须调用方确认，不代表提供变量实例化/CFF支持。
  - [x] 实现预算化字形依赖闭包：GlyphLocations::subset_closure按唯一GID收集，加入必需GID0，迭代检查组件范围、循环、缓存子树深度及聚合根/字形/边工作量；复合双遍输入计费，组件借用解析和实际容量预算所有者保留。1048项门禁中的closure去重/峰值失败/unwind/循环范围与长路径及glyf/loca重建测试均通过；不依赖SimpleGlyph栅格展开，不声称旧解析器所有分配均已入池。下列原设计细项继续逐项核对，不自动随总闭包实现勾选。
    - [x] 复用现有composite_glyf.rs记录解析：子集闭包调用共用try_new_with_budget/visit_components，不重复实现组件字节解析。validate_composite_dependencies保留原发生次数/深度摘要契约，不直接充当唯一集合；新subset_closure提供多根去重、必需GID0、迭代循环/缓存深度校验与实际容量预算，702项回归中的闭包去重/峰值/unwind/循环测试通过。expanded_glyf.rs仍是独立栅格展开路径，不宣称旧解析器全部入池。
    - [x] 组件解析预算所有权：共用校验的借用遍历及try_new_with_budget已实现，预算化闭包复用该入口；原非预算try_new契约保留。实际capacity、分配前预留、相邻额度/释放重试有专项及1048项完整门禁证据，最新组件专项5项再次通过；不能据此宣称旧解析器所有分配均在池内。
    - [x] 借用已检查loca/glyf并验证复合记录：子集直接复用GlyphLocations::try_new（checked_glyph_locations亦使用的范围验证器）及glyph_data，避免重复完整字体解析；复合体使用共用visit_components。负轮廓计数、字节/字参数、互斥变换、任意组件指令标志/末尾指令长度、首组件XY及保留位均有定向测试；逐字节截断、多组件、闭包嵌套/重复/空字形由704项联合日志覆盖。真实Arial复合子集及抽取字体语义/指令/组件原GID独立检查通过；不代表shaping或旧栅格路径全部预算化。
  - [x] 静态TrueType原GID策略的SFNT写出与验证：重建glyf/长loca/cmap/head/name，原GID与numGlyphs及度量表保留，组件和CIDToGIDMap一致；目录排序/对齐/各表校验与整体B1B0AFBA，未知依赖拒绝、已知布局显式省略并报告。704项单元日志含writer边界/峰值/重开与表策略通过；真实Arial复合/Noto中英单多层/混合及固定竖排抽取字体按源校验轮廓、指令、度量、原样表、许可声明，独立文字坐标/渲染及负例通过。不提供GID重编号、变量/CFF/shaping；完整字符扩充/真实状态矩阵另项仍未完成，工作区新门禁尚待终态。
    - [x] 修正head整体轮廓边界：来源绑定预算化闭包计算保留有轮廓字形头联合，忽略无轮廓/空复合，全空为零框，重建head及FontBBox。闭包5项、组装/公共注册10项专项通过；修正版Noto/Arial重生成后独立字段、七负例、精确文字及三页视觉通过。仅完成该字段修正，不代表全部字体语义或新版完整workspace验收。
    - [x] CID接入采用明确的原GID保留策略；每个Unicode标量保留独立CID/宽度/ToUnicode，即使多个标量共享GID也不合并。内部只读glyph_mappings供闭包与cmap写出直接使用，不反解析CIDToGIDMap。共享GID及非BMP的编码表测试已由1048项门禁覆盖；后补共享GID子集组装/公共插入/严格重开/另一页隔离专项退出0，证据`/tmp/rzpdf-subset-shared-gid-public.log`。不提供GID重编号；未来若新增该策略，须保持CID与Unicode稳定并另行验收。
  - [ ] 扩展编辑字符映射：新增字符先验证原字体覆盖，再以事务复制局部字体资源、更新宽度/CIDToGIDMap/ToUnicode及文本编码；原FontFile和共享页保持不变。拒绝、预算不足、外层error/panic回滚及刷新重试需覆盖；已有不可编码拒绝不等于新增字符支持。
    - [x] 自动覆盖扩充入口机制已接入：SubsetTextEditOptions显式完整源，validate_subset_source认证兼容字形/度量；单字符串/TJ/页面事件/同leaf/展开跨调用入口按原快照规划，保留前后缀字集、事务局部注册与字体恢复、按各自状态度量及失败回滚。702项PDF单元日志含automatic_subset_string/TJ/cross_event/nested/cross_call和聚合额度通过，真实单/双层及Courier混合固定样本独立验收通过；不自动修改旧不可变字体句柄，不是字体回退或自动排版。
    - [ ] 自动覆盖扩充完整验收：继续补两种嵌入CID源之间的真实混合、竖排及其余状态/边界矩阵；总字符映射任务保持未完成。上项只确认机制落地，不以固定水平样本代替完整交付。
  - [ ] 实际字体闭环：简单及复合字形、重复/空格/中文、多页共享与新增字符样本；严格保存重开、精确文本、独立渲染及资源隔离检查。记录支持范围和未知表策略，完整通过之前本项不勾选。
- [ ] P3-06 区分视觉覆盖与真正删除内容；如提供脱敏功能，建立残留内容检查。

验收：指定内容修改可追溯到原操作符；未修改的内容及未知操作保留；共享资源修改不影响非目标页面；明确无法编辑的场景。

## P4：解析与渲染兼容性

可在 P1/P2 后按样本需求推进部分任务；完整阶段依赖稳定的内容与资源模型。

- [ ] P4-01 完成内联图像边界识别与解码，避免将图像数据中的 EI 误判为结束。
- [ ] P4-02 完成 Type3、字体编码回退、复杂 CJK/竖排和更多 TrueType cmap 格式。
- [ ] P4-03 完成 Pattern、Shading、Lab/ICC 等颜色和 PDF 函数实现。
  - [ ] Type6 Coons 曲面：解析12个边界控制点与4角颜色、共享边标志及输入/保留预算；实现参数曲面与参数颜色插值后求函数，不以控制点多边形替代曲面。先验收四条边界和角点、直边退化矩形、非线性函数与共享边方向，再接入绘制。
    - [ ] 已认证边界根精度：按docs/architecture.md的非均匀直边样例验证原参数1/4及直接/平方函数颜色，复用有工作额度的区间细化；镜像、细化失败不修改输出和重试恢复均需通过，不能只验证唯一性或区间包含。
    - [ ] 一般边界根与域成员：扩展任意方向直边和弯曲边界存在证明，明确延拓根是否位于原参数域；保留控制区间不确定性，不能用epsilon或Newton候选代替证书。
    - [ ] 完整根覆盖与分割线去重：各子域全部证明有根或无根后才选择结果，处理根落在内部划分边界及重复证书，验证遗漏竞争根不会误选颜色；未解决时明确错误而非当域外。
  - [ ] Type6 折叠与重叠：曲面可折叠，投影绘制区域不一定等于补丁边界；同像素多个参数映射选择最大v，再最大u，后出现补丁覆盖前补丁。验证曲面内部折叠、重复映射与补丁叠加，采用有误差/工作量上限的策略，不以固定粗网格或单根反解冒充完整实现。
  - [ ] Type6完整交付：资源scope/矩阵/裁剪/透明度、显示来源、注册失败恢复、导入、两种保存策略严格重开及独立阅读器对照；保留Type4/5回归，之后实现Type7张量积补丁和Pattern，不把Type6单项当作整个P4完成。规范：[Adobe PDF Reference 1.5](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.5_v6.pdf)、[1.6控制点/共享边说明](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.6.pdf)。
  - [ ] Shading Type2/3 交付验收：已有参数/点采样、资源与显示事件、Skia 实际函数求值像素以及保存版本检查；补 Form 局部资源/来源、旋转与非均匀 DPI、Extend 与 Background 页面样本、独立阅读器对比和性能/临时分配预算，不能以现有专项代替完整验收。
  - [ ] Shading Type1 与 Type4–7：分别实现函数域映射和网格/补片解析、求值、资源/显示事件与保存读回，建立各类型独立像素样本。
  - [ ] Pattern：实现平铺与 Shading Pattern 的坐标/资源作用域、着色/非着色语义和背景绘制，与直接 sh 的 Background 语义区分。
  - [x] 零BlackPoint的Lab数值转换：白点适配到D65的固定sRGB策略、输入/Range裁剪、灰阶/主色及间接参数严格往返已验证；完整颜色管理与独立视觉矩阵仍未验收。
  - [x] Type3 Stitching单输入分段求值与构造维度校验：Domain/Encode/Range、嵌套/间接参数、端点与错误回归及Separation颜色转换保存重开已验证；不表示Type4、Shading/Pattern或聚合内存验收完成。
- [ ] P4-04 完成 ExtGState、透明组、软蒙版、混合模式与文字渲染模式；扩展 Device/DisplayList 事件。
- [ ] P4-05 评估并接入进程内 JPX/JBIG2 解码后端，记录许可、构建成本与支持范围。
- [ ] P4-06 完善损坏文件恢复和混合版本链解析；恢复产生诊断，不静默掩盖错误。
- [ ] P4-07 建立独立渲染对比、像素差异与人工复核流程；定义容差与已知差异。

验收：固定兼容性样本集有逐项结果；支持范围有明确记录；差异可追踪，不以单一截图证明完整兼容性。

## P5：高级功能与发布

2026-10-14：v0.4 冻结的交互子范围完成本地工程验收；用户确认附件限额测试修复后的完整工作区回归全部通过，定向及独立工具证据见 [验收记录](docs/releases/v0.4-acceptance.md)。未打标签或发布。P5-01/02 的完整父范围仍未完成，不因限定子范围验收而勾选；可进入 v0.5 实现。

v0.5 的 V05-00～18 和最终完成定义均已在[冻结范围](docs/releases/v0.5.md)内关闭；完整回归和 A/B/C 独立检查全部通过。最终证据统一归档到 [v0.5 验收记录](docs/releases/v0.5-acceptance.md)，不因限定子范围验收而勾选下列完整父任务；更广的算法、组合、验证服务及性能承诺仍需另行立项。

- [ ] P5-01 批注、链接、书签及命名目标的读写与外观流。
- [ ] P5-02 附件、AcroForm 填写及外观生成；XFA 单独定义范围。
- [ ] P5-03 实现增量保存：保留原字节、只追加变更、正确更新 Prev/xref/Trailer 和删除代次。
- [ ] P5-04 文档密码验证、权限信息、字符串/流加解密与 Encrypt/ID 管理；支持修订逐项验收。
- [ ] P5-05 数字签名单独推进：ByteRange、预留空间、验证与外部签名接口；声明签名后编辑的影响。
- [ ] P5-06 性能基准：大文件读取、页面访问、文本提取、渲染和保存；记录峰值内存。
- [ ] P5-07 稳定公共 API、文档、示例、版本策略及已知限制；明确线程模型。
- [ ] P5-08 如需 PDF/A、PDF/UA、PDF 2.0，另建符合性任务与外部验证流程。

验收：每个发布功能都有支持矩阵、正反用例和外部验证结果；未实现功能返回明确错误。

## 执行与完成规则

### 历史里程碑与依赖参考（不作为当前执行顺序）

本轮文档整理明确：当前处于基础能力加固阶段，不能将已有解析、渲染及低层对象修改接口视为完整 PDF 编辑器。当前工作区的既有测试记录见上方实施记录，本轮不重新宣称测试验收。

| 交付里程碑 | 对应任务 | 最小交付物 / 退出条件 |
|---|---|---|
| M0 基础收口 | P0 剩余项 | 输入错误可恢复、预算边界有回归、许可明确的样本集、远端 CI 与独立验证结果 |
| M1 从零构建闭环 | P1 | 无输入源创建文档；中文、英文、图片多页示例；保存后独立工具读取和渲染正确 |
| M2 常用文档编辑 | P2 | 提取、页面重排、导入、合并拆分、水印；共享资源与失败回滚用例通过 |
| M3 内容级编辑 | P3 | 带来源映射的可编辑内容模型；文字/图片修改不破坏未知内容及非目标页面 |
| M4 兼容性扩展 | P4 | 按真实样本逐项交付字体、复杂绘制与解码支持，有独立对比记录 |
| M5 高级能力与发布 | P5 | 批注、表单、增量保存、加密和签名分别验收；稳定 API 与明确支持矩阵 |

M4 的高频兼容性问题可与 M1/M2 并行推进；不能以“完整支持 PDF”作为无边界验收标准。每个里程碑固定样本、支持范围和已知限制后再估时。

- [x] 架构决策 A1：确定读取源隐藏方式及旧 API 兼容方案，再开始 P1-01；验收时新建文档不得依赖伪造空 PDF 或临时输入文件。
- [ ] 架构决策 A2：统一文档修订号、对象 ID 分配和派生缓存失效规则；编辑后重复读取、提取及渲染必须看见最新状态。
- [ ] 架构决策 A3：明确流的原始/解码状态转换、资源复用键和复制后修改规则，再开始图片嵌入及页面导入。
- [ ] 架构决策 A4：确定 ContentProgram 的来源定位与未知操作保留契约，再开始文字替换；DisplayList 不作为编辑源。

1. 收尾 P0-08：逐模块检查字体、颜色、函数与解析器的输入触发 panic；不支持功能返回明确错误，畸形输入增加回归用例。
2. 收尾 P0-10：补整个对象图的聚合预算、渲染配置与 Limits 的统一，明确未来 Form 执行的递归限制。
3. 推进 P0-12 与 P0-01 的真实样本登记、独立工具检查和普通 CI；模糊测试按用户要求暂缓，不作为后续功能工作的阻塞条件。未完成的验收仍不勾选。
4. 推进 P1-01～P1-04：先解耦读取源与文档状态，再实现新建文档、PageBuilder、ResourceManager 和 ContentBuilder。
5. 推进 P1-05～P1-10：实现中英文字体与图片嵌入、可靠保存及端到端示例；通过独立阅读器验证后再进入 P2。

上述顺序保留为历史依赖参考。当前按 v0.1 清单推进：先盘点与检查点，再补发布阻塞项、冻结候选版本、完成验收，最后在用户确认后提交/打标签/发布。版本计划本身不授权推送或外部发布。

### 通用完成规则

1. 首版只验收 P0/P1/P2 中映射到 v0.1 的子范围，不要求整个技术阶段清空。fuzz 按用户要求暂缓；与正式支持路径直接相关的崩溃、静默损坏、共享污染和失败回滚问题仍为发布阻塞项。
2. 每项完成后记录实现位置、验证命令、结果和限制，再勾选；不以已有函数名或测试数量代替验收。
3. 读写验证同时包含本库往返、独立结构检查和独立渲染；避免读写两端共享缺陷。
4. 新功能优先在现有 crate 内按模块实现；只在后端替换、依赖隔离或复用需求明确时拆 crate。
5. 排期以 ROADMAP 的粗估及 v0.1 差距盘点为准，不沿用历史首版估时作为交付承诺；冻结范围后再估算剩余工作。
