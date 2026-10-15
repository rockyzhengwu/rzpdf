# Type3 字体检查

当前接口是 v0.7 的实验性检查模型，不是 Type3 页面解释或绘制支持。`Font::try_new` 和页面文字解释仍明确拒绝 Type3；不能用检查成功判断页面可渲染。后续执行与提取工作见 [v0.7 计划](releases/v0.7.md)。

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

字符码是 0～255 的 PDF 单字节码，不是 Unicode 或 GID。`glyph_name` 按 Encoding 查找 CharProcs 的键；缺映射或缺字形返回错误，不自行替代字体。这个切片不解析 ToUnicode，也不推断阅读顺序。

`width` 返回 Widths 中的字形空间宽度；`text_space_advance` 用 FontMatrix 的线性部分变换宽度向量，不应用平移，也不固定除以 1000。字号、水平缩放及文字间距尚未由此 API 应用。

`inspect_type3_glyph` 有界解码并解析完整 CharProc，要求首个操作是两个参数的 d0 或六个参数的 d1；参数有限，纵向宽度为零，d1 包围框有序，后续不重复声明度量。`Type3GlyphMetrics` 保留 CharProc 自己的宽度与可选包围框，不覆盖 Widths。程序中的绘图操作只解析，不执行；d1 着色限制、资源调用、裁剪和字形递归仍待解释器阶段验证。

## 生命周期与预算

模型绑定文档身份与修订，跨文档或修改后读取其间接字形引用会拒绝，须重新检查字体。默认入口创建一次操作池；显式预算入口使资源/字体副本、编码、解码载荷和解析结果共用调用方的池。

字体和字形结果各自持有保留容量的 reservation，包装类型不提供 Clone；字形程序可以在字体结果释放后继续读取。若手动调用只读视图上的既有 `ContentProgram::clone`，克隆不继承 reservation，仍属于旧的非计账路径。源对象缓存、控制块及已有原生分配也不属于这个计账保证，不是 RSS 或 CPU 硬上限。Resources 暴露声明的字典，不在此阶段实现执行时的页面回退与嵌套查找。

## 示例与验证

示例只读输入。可选输出使用新文件创建，不覆盖已有文件：

```bash
cargo run -p pdf --locked --example inspect_type3_font -- input.pdf 0 F0 65
cargo run -p pdf --locked --example inspect_type3_font -- input.pdf 0 F0 65 roundtrip.pdf
cargo test -p pdf --locked --test v07_type3_model -- --test-threads=2
cargo build -p pdf --locked --example inspect_type3_font
PYTHONPATH=tmp/pdfs/v06-verifiers:tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v07-type3-model.py
```

独立脚本用 pikepdf 生成项目自有的 d0 彩色矩形与 d1 三角形，核对模型度量、程序操作数目及原字形字节；qpdf/pikepdf 检查结构，MuPDF 验证保存前后整页像素不变。它不调用本库 Type3 渲染，因此不是 V07-02 或整版验收。证据写入新的 `tmp/pdfs/v07-type3-model-*` 目录，不覆盖旧报告。
