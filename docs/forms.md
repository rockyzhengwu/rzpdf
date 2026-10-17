# AcroForm 字段与 Widget

表单字段保存逻辑值，Widget 保存页面位置和外观。层级关系由 `/Fields`、`/Kids`、`/Parent` 连接；Widget 同时属于某一页的 `/Annots`。字段名是层级路径，稳定身份使用文档限定的 `ObjectHandle`，不能跨文档复用。

支持层级与继承、多 Widget、Text、Checkbox、Radio、Choice、PushButton，以及复用嵌入字体的中文外观。动作只保存、不执行。页面复制、导入和删除的字段关系策略见[页面关系指南](page-relationships.md)，整版结论见[验收记录](releases/v0.9-acceptance.md)。

## 创建与读取

```rust
use pdf::document::{PdfDocument, acroform::FormValue};

let doc = PdfDocument::new()?;
doc.add_page(0)?;
doc.add_page(1)?;
doc.add_text_field(0, [20., 40., 180., 70.], "person.address.city", "Shanghai")?;
doc.add_checkbox_field(0, [20., 90., 44., 114.], "person.accepted", false)?;

let tree = doc.form_field_tree()?;
let city = tree.iter().find(|f| f.name == "person.address.city").unwrap().handle.clone();
let second = doc.add_form_widget(&city, 1, [25., 150., 225., 190.])?;
doc.set_form_value("person.address.city", FormValue::Text("Suzhou".into()))?;
doc.remove_form_widget(&second)?;
```

带点的新增名称按完整路径解释，自动建立缺失的容器；不能穿过已有终端字段。部分名称不允许空串、点或控制字符。原 v0.4 接口保留并委托共用创建管线；旧文本创建入口默认 Helvetica、单行及 ASCII。`form_fields()` 返回所有支持类型的有效值，`page_index` 只是第一个 Widget 当前所在页。`form_field_details()` 另含类型、Choice 选项、按钮状态和导出标签；需要所有呈现位置时使用 `form_field_tree()`，不要把页号或字段名当身份。

`FormFieldNode` 按前序返回字段句柄、父句柄、部分名称、完整名称、子字段、Widget 列表以及继承后的 `FT/Ff`。每个 `FormWidget` 包含自己的句柄、页号和矩形。读取不要求 AP 存在，不执行动作；未知字段类型保留可检查，但未知类型和签名字段不能通过本接口编辑。XFA 字段图明确拒绝。

继承按 ISO 32000-1 的字段条目定义处理 `FT/Ff/V/DV/DA/Q/MaxLen`；按钮 `Opt` 可继承，Choice 的 `Opt` 不继承。`T/Kids/P/AP` 不继承。Widget 的 `P` 可以省略，读取时从页面成员关系推导；存在时必须与页面一致。相同完整名称的匿名终端字段必须保持相同 `FT/V/DV`；这一合法表示可检查，但兼容填值接口明确拒绝多终端名称歧义，不猜测更新哪个对象。

## 字段和 Widget 编辑

| 接口 | 语义 |
|---|---|
| `add_form_group(parent, partial)` | 显式添加空容器，返回稳定字段句柄；终端字段不能充当容器 |
| `rename_form_field(handle, partial)` | 修改一个部分名称，所有后代完整路径随之变化；碰撞或失败原子回滚 |
| `add_form_field(name, definition, widgets)` | 按明确类型、初始值和 Widget 列表创建字段；返回稳定字段句柄 |
| `add_form_widget(handle, page, rect)` | 为 Text/Checkbox/Choice/PushButton 增加呈现；必要时拆分旧合并对象，字段句柄保持不变 |
| `add_form_widget_with_state(handle, page, rect, on_state)` | 为 Radio 显式增加一个 on-state |
| `remove_form_widget(handle)` | 移除一个拆分 Widget，同时从字段和页面解除引用；必须保留至少一个 |
| `remove_form_field(handle)` | 从字段树移除整个子树，并从所有所属页面移除其 Widget |
| `set_form_value(name, value)` | 在同一事务写字段 V、Choice 的 I 或所有按钮的 AS；文字重建全部 Widget 正常外观；不改祖先值 |
| `set_radio_widget(widget_handle)` | 同名状态但不 InUnison 的 Radio，明确选中某个 Widget |
| `set_form_text_style(handle, style)` | 原子更新字体、正字号及左/中/右对齐，重建全部 Widget 正常外观 |
| `set_pushbutton_caption(handle, caption)` | 更新所有 PushButton Widget 的 MK/CA 和正常外观，不执行点击动作 |

新建与生成外观的矩形宽高至少 18，必须有限。不相关的未知字段不阻止填充一个支持的字段。`FormFieldDefinition` 使用 kind/value、Choice options、按钮 button_exports、text_style、Text max_len 和 PushButton caption；用 `FormFieldDefinition::new(kind, value)` 建立默认值。`FormWidgetSpec.on_state` 指定按钮的选中名称，Radio 必须显式提供，普通 Checkbox 默认 Yes。

## 字段类型与状态

| 类型/值 | 语义与边界 |
|---|---|
| `Text(String)` | 单行或显式 CR/LF 多行；MaxLen 按 Unicode 标量计数，初始值也检查；不支持 password、comb、rich text |
| `Checkbox(bool)` | 同一字段的 Widget 共用一个 on-state；可以自定义，不限 Yes |
| `ButtonState(Option<String>)` | Checkbox 不同 Widget 有不同状态时明确指定 V 名称；None 清除 |
| `Radio(Option<String>)` | V 是状态名，不是导出标签；支持 NoToggleToOff/InUnison；None 清除，NoToggleToOff 时拒绝 |
| `Choice(Vec<String>)` | 按导出值选择，Opt display 用于绘制；普通/可编辑 Combo、单/多选 List |
| `ChoiceIndices(Vec<u32>)` | 按 Opt 索引选择，解决重复导出值；保存时排序并保持 V/I 一致，重复索引拒绝 |
| `PushButton` | 没有可填永久值，不写 V/DV；改 caption 不等于执行动作 |

Choice 的 V：空为 null，单项为字符串，多项为字符串数组；多选或单选重复导出值保留必要 I。读取时 I 与 V 不一致以 V 为准；不能消歧则拒绝。可编辑 Combo 允许列表外值，普通 Combo 不允许；List 根据 TI 只画可见行，必要时移动 TI 使首个选中项可见，不承诺全部选项同时显示。Sort 保留，不在读取时重新排序选项。

按钮填值更新 V 和所有 Widget AS，保留已有 on-state 图形及 R/D。导入文件可没有 Off 外观，写入时补显式空白 Off。Radio 同名状态且不 InUnison 时，按状态名选择会拒绝歧义，改用 `set_radio_widget`。

`button_exports` 为 Unicode 导出标签数组，长度与 Widgets 一致，各 Widget on-state 必须为从 0 起的索引名称；V/AS 仍是这些名称，不是标签。按钮 Opt、多选和必要 I 要求 PDF 1.4，InUnison 要求 1.5；版本不足不静默升级。按钮含 Opt 时，Widget 增删涉及导出索引重编号，暂时明确拒绝；此类字段仍可创建、读取和填值。移除选中的普通 Radio 会清除值，NoToggleToOff 时拒绝；至少保留一个 Widget 的规则不变。

## 中文与文字外观

`FormTextStyle` 指定文档内字体句柄、正字号和 Left/Center/Right，font=None 为标准 Helvetica。中文通过现有 `resources().embed_truetype_font` 或子集接口取得注册字体，再指定 `definition.text_style.font = Some(font.object_handle().clone())`。字体 repertoire 必须包含实际文字，换行不需要字形。完整创建示例见 `pdf/examples/v09_form_types.rs`；定义用 `FormFieldKind::Text { multiline: true }` 和 `FormValue::Text(...)`，Widget 用 `FormWidgetSpec::new(page, rect)`。

字体资源存入 AcroForm DR，字段 DA 持久化资源名和字号；保存重开后可以直接填值，不需要原注册令牌。复用既有 Font、编码、ToUnicode、CID 宽度及共用 AP，不新增字体框架，不依赖系统 CJK 回退。实际验收为 Base14 与仓库许可明确的静态 TrueType Identity-H 样本。

单/多行按字宽和字号行盒检查横向/纵向溢出，并裁剪到内边距；这不是任意字体墨迹边界的自动排版。控制字符和缺字明确拒绝，不自动缩放、换行、shaping 或回退。竖排和 Type3 字段文字不在范围。导入 DA 支持一次 Tf 与 g/rg/k 文字色；零字号自动适配、非默认文字状态及其他运算符需显式改 style，否则拒绝。新外观为白底黑框，List 选中行蓝底白字；不承诺源 AP、MK 配色和高级边框原样重建。Unicode 字段外观不扩大 FreeText 的范围。

拆分时保留原字段对象身份，另建原 Widget 呈现对象并替换原页面引用。旧 AP 的 R/D 保留，填值只替换 N。合并对象的未知键无法可靠判定属于字段还是 Widget，因此拆分会拒绝；已经拆分的字段在编辑时保留未知键。已有任意自定义外观的视觉设计不保证原样重建，生成外观是本库的简单白底、黑框样式。

结构编辑遇到计算顺序、目录/页面动作、JavaScript 名称树、批注动作、不支持的书签动作或指向表单的非字段批注关系时保守拒绝，不重写脚本中的字段名。字段/Widget 动作、只读和锁定标志、可选内容、Tagged 所有权及签名来源也有保护。新增/改名/移除、style 和 caption 需要批注及通用修改权限；填值和 `set_radio_widget` 独立使用 fill_forms 权限，不因此取得通用修改权限。不运行计算或其他动作。

所有操作保留事务回滚：失败不改变 `commit_revision` 和可见对象状态，内部 `revision` 会递增以失效旧快照。检查字段/Widget 数、页面成员总数、树深度、累计名称字节，以及同次文字外观生成跨 Widget 的字符、运算和输出字节。字体解析、编码和输出缓冲复用已有 OperationBudget；文档驻留对象、旧解析路径和部分原生分配不属于全引擎聚合内存池，不能解释为 RSS 上限。删除是解除可达关系，不等于安全擦除历史字节。页面关系迁移支持范围及显式拒绝策略见[指南](page-relationships.md)。

## 外观与验证

Widget 预览使用共用 AP 管线和显式 `display_list_with_annotations`，不会混入正文提取或正文编辑来源，详见[批注指南](annotations.md)。默认页面渲染仍只处理正文。

测试在 `pdf/tests/v09_forms.rs` 和 `pdf/tests/v09_form_types.rs`。独立门禁复现入口：

```sh
cargo build --locked -j2 -p pdf --example v09_forms --example v09_form_types -p render --example render_annotations
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/check-v09-forms.py tmp/pdfs/NEW-forms-gate
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/check-v09-form-types.py tmp/pdfs/NEW-form-types-gate
```

脚本使用已有 pikepdf、Pillow、qpdf、MuPDF；输出目录必须不存在，不安装依赖。关系门禁四页；类型门禁包含本库三页和独立生产者一页，检查 V/AS/AP、Opt/V/I、继承、多 Widget、R/D、导出/显示文字、嵌入字体、ToUnicode、中文码点及基于 CID 宽度的绝对对齐，再比较像素并目视检查。阶段结果见[验收记录](releases/v0.9-acceptance.md)。

规范依据：[ISO 32000-1 12.7.3 字段字典及 12.5.6.19 Widget](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf)。实现基于规范，不复制第三方引擎代码。
