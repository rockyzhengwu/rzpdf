# Type3 字体与字形执行

v0.7 已接通受限 Type3 图形字形执行、文字提取和来源隔离；独立检查模型仍可单独使用。当前执行范围是 Tr=0、不透明、普通混合模式的图形字形，不支持任意 Type3 程序。版本进度见 [v0.7 计划](releases/v0.7.md)。

## 字体与字形程序

通过 `PdfPage::inspect_type3_font` 或 `inspect_type3_font_with_budget` 取得 `Type3Font`。模型校验 Type/Subtype、FontMatrix、FontBBox、FirstChar/LastChar、Widths、Encoding、CharProcs 和可选 Resources。Encoding 复用现有预定义编码与 Differences；字段和数组元素可为间接对象。

```rust,ignore
let page = doc.get_page(0)?;
let font = page.inspect_type3_font("F0")?;
let advance = font.text_space_advance(65)?;
let glyph = page.inspect_type3_glyph(&font, 65)?;
let metrics = glyph.metrics();
let operators = glyph.program().operations();
```

字符码是 0～255 的 PDF 单字节码，不是 Unicode 或 GID。`glyph_name` 按 Encoding 查找 CharProcs 的键；缺映射或缺字形返回错误，不自行替代字体。ToUnicode 复用现有 CMap，保留多标量及代理对；存在 ToUnicode 时缺映射返回 None，不再按字形名猜测。没有 ToUnicode 时可用已有 Encoding 的字形名映射。不推断阅读顺序。

`width` 返回 Widths 中的字形空间宽度；`text_space_advance` 只取经 FontMatrix 变换后的水平分量，不应用平移，也不固定除以 1000。即使 FontMatrix 旋转字形，文字推进仍在文本空间的水平方向，见 [PDF 1.7 §5.5.4，Widths 条目](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.7old.pdf)。此方法不应用字号和文字间距；解释器另行应用 Tf/Tz/Tc/Tw/TJ、文字上移及页面变换。

`inspect_type3_glyph` 有界解码并解析完整 CharProc，要求首个操作是两个参数的 d0 或六个参数的 d1；参数有限，纵向宽度为零，d1 包围框有序，后续不重复声明度量。`Type3GlyphMetrics` 保留 CharProc 自己的宽度与可选包围框，不覆盖 Widths。检查成功不代表能执行所有后续操作；实际执行还要求 CharProc 宽度与 Widths 一致。

## 执行与编辑边界

| 组合 | 当前行为 |
|---|---|
| d0 普通图形 | 执行现有路径、颜色、q/Q/cm、裁剪、普通图像和非 Group Form 路径；不新增一般色彩管理承诺 |
| d1 不着色字形 | 继承调用时填充/描边颜色；拒绝字形内改色及普通图像，只接受 stencil 图像 |
| 资源 | 显式字体 Resources 为独立作用域；省略时回退到页面资源，即使文字位于 Form 中；显式空字典不回退。被调用 Form 遵循已有 Form 资源契约 |
| 来源与状态 | 每个字形隔离图形状态，保留父裁剪。内部路径/图像没有页面根或普通 Form 编辑来源；不允许将其当作页面操作修改 |
| 文字提取 | 保留原文字操作的字符码、Unicode、位置、TJ 数组项和代码字节范围；不将内部图形当成额外字符 |
| 文字编辑 | 已有原始代码替换/整次文字绘制删除沿用来源、共享页隔离及权限保护；不提供 Type3 Unicode 编码、保留 advance 的宽度聚合或字形内部编辑 |
| 明确未支持 | Tr 非 0、透明度/非 Normal 混合、字形内 BT/ET 文字、ExtGState、sh、透明组和其他未列出的操作 |

执行与 Form 共用累计操作、解码工作量、路径及文本额度；Form 循环被拒绝，深度另受 128 层实现限制。字形内文字递归目前直接拒绝，不声称已支持任意递归字体。R6 的 CharProcs 随各自流解密，不重复解密外层内容；编辑继续受既有密码权限及签后保护约束。

非默认 Tr 的独立渲染在本机 MuPDF 与规范说明间出现差异，当前保留为明确拒绝用例，不计入正向像素通过。这不是一般 Type3 标准禁止使用这些模式的声明。

## 绘制设备适配

`Font` 新增 `Type3(Rc<Type3Font>)` 变体，穷举匹配的下游需增加分支。它没有可交给 FreeType/Skia 的 `fontfile`，其嵌入程序是 CharProcs。

解释器先输出字形图形，再输出带 `TextObject::is_type3_metadata()` 标记的文字事件供提取和来源检查。正常 `display`/display-list `replay` 的绘制适配器过滤该文字元数据，避免重复绘制；直接消费 display-list 的自定义设备应检查该标记。Skia 单独收到未解释的原始 Type3 TextObject 时会拒绝，不把缺字形图形当成空白成功。

## 生命周期与预算

模型绑定文档身份与修订，跨文档或修改后读取其间接字形引用会拒绝，须重新检查字体。默认入口创建一次操作池；显式预算入口使资源/字体副本、编码、解码载荷和解析结果共用调用方的池。

字体和字形结果各自持有保留容量的 reservation，模型包装类型不提供 Clone；字形程序可以在字体结果释放后继续读取。`Font::clone` 为 Type3 共享同一模型及 reservation，而不是复制全部字体数据。若手动调用只读视图上的既有 `ContentProgram::clone`，克隆不继承 reservation，仍属于旧的非计账路径。资源作用域/图形状态的旧克隆、源对象缓存、控制块及原生分配不属于完整计账保证，不是 RSS 或 CPU 硬上限。

## 示例与验证

示例只读输入。可选输出使用新文件创建，不覆盖已有文件：

```bash
cargo run -p pdf --locked --example inspect_type3_font -- input.pdf 0 F0 65
cargo run -p pdf --locked --example inspect_type3_font -- input.pdf 0 F0 65 roundtrip.pdf
cargo test -p pdf --locked --test v07_type3_model -- --test-threads=2
cargo build -p pdf --locked --example inspect_type3_font
PYTHONPATH=tmp/pdfs/v06-verifiers:tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v07-type3-model.py
cargo build -p render --locked --example render_document
PYTHONPATH=tmp/pdfs/v06-verifiers:tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v07-type3-render.py
```

模型脚本用 pikepdf 生成项目自有字形，核对度量、操作数目及保存，仍不调用本库渲染。渲染脚本另用 12 页项目自有 PDF 比较 Type3 与直接展开的等价图形，包含 4 个页面方向、水平缩放/上移、非标准 FontMatrix、负字号/缩放、裁剪、图像、stencil 和 Form；同时核对原编码保存、Skia/MuPDF 保存前后整页像素以及明确拒绝的 Tr=3/7。栅格基线保持图像形式，不用抗锯齿矢量边缘替代最近邻图像采样。证据写入新的 `tmp/pdfs/v07-type3-*` 目录，不覆盖旧报告；这些检查不代表 v0.7 整版验收。
