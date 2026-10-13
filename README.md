# rzpdf

**用 Rust 构建、读取和处理 PDF。**

rzpdf 是一个持续演进的 PDF 工具库：从创建带文字和图片的文档，到提取内容、组织页面，再到逐步完善内容编辑与渲染，让 PDF 工作流在同一套对象模型中连接起来。

适合在 Rust 应用中集成 PDF 生成、内容提取和页面处理，也为需要深入操作文档对象与资源的开发者提供基础能力。

## 从创建到处理

- **创建文档**：配置页面尺寸，绘制文字、路径和图片，写入 Info / XMP 元数据。
- **读取与提取**：解析文档，获取页面几何，提取文字、图片及绘制位置，并查看解析诊断。
- **组织页面**：增删、复制、重排、旋转和裁剪页面；跨文档导入、合并和按指定顺序选页。
- **读写闭环**：从文件或字节打开文档，修改后保存到字节、Writer 或文件，支持保留编码流和压缩未编码流。
- **渲染与编辑**：提供独立的 Skia 渲染后端；文字、图片和路径的内容编辑接口正在完善，当前按实验能力使用。

中文创建支持嵌入静态 TrueType 字体；标准西文字体提供显式编码接口。文字提取保留绘制顺序和坐标，方便构建后续处理流程。

## 为什么是 rzpdf

**不止生成，也能处理已有文档。** 创建、解析、页面操作与保存共享文档模型，不必把 PDF 仅仅当作最终输出。

**核心与渲染分离。** PDF 核心不依赖 Skia，需要页面预览或图像输出时再接入渲染后端。

**把修改正确性放在前面。** 高层修改接口使用事务、修订检查与共享资源隔离，帮助避免一次编辑意外影响其他页面。

**支持范围有据可查。** 通过回归测试、独立工具检查和固定样本视觉验证建立边界，不把“已有接口”当作“完整兼容”。

## 快速开始

当前建议从源码运行示例。需要支持 Rust 2024 edition 的工具链；首次构建涉及 FreeType、JPEG 等原生依赖，渲染模块还涉及 Skia，请预留构建时间。

```bash
git clone https://github.com/rockyzhengwu/rzpdf.git
cd rzpdf

# 创建两页包含文字、图片和元数据的 PDF
cargo run -p pdf --locked --example create_document -- created.pdf

# 按绘制顺序提取文字
cargo run -p pdf --locked --example extract_text -- created.pdf
```

### 在代码中创建 PDF

```rust
use pdf::{
    document::{PdfDocument, resource_manager::StandardFont},
    error::PdfResult,
    page::content_builder::ContentBuilder,
};

fn main() -> PdfResult<()> {
    let doc = PdfDocument::new()?;
    doc.add_page(0)?;

    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)?;

    let mut content = ContentBuilder::new();
    content.begin_text();
    content.try_set_font(&font, 24.0)?;
    content.move_text(48.0, 720.0);
    content.show_encoded_text(&StandardFont::Helvetica.encode_text("Hello, PDF!")?)?;
    content.end_text();

    doc.replace_page_content_builder(0, &content)?;
    doc.save_to_file("hello.pdf")
}
```

页面坐标使用 PDF 用户空间，默认页面原点在左下角。文字必须按所用字体编码，不能直接把中文 UTF-8 当作 PDF 字符码；中文示例见 [create_cjk.rs](pdf/examples/create_cjk.rs)。

### 更多工作流

```bash
# 合并文档
cargo run -p pdf --locked --example merge_documents -- merged.pdf first.pdf second.pdf

# 按指定顺序提取页面（索引从 0 开始）
cargo run -p pdf --locked --example extract_pages -- selected.pdf input.pdf 3 0

# 将页面渲染为 PNG（输出目录中的目标文件须不存在）
cargo run -p render --locked --example render_document -- input.pdf preview 144 144
```

创建、合并和选页示例会替换指定的现有普通输出文件，请使用新的输出路径。更多示例见 [pdf/examples](pdf/examples) 和 [render/examples](render/examples)。

## 当前适用范围

项目处于早期开发阶段：v0.1 基础工作流已在冻结范围内验收，尚未正式发布，API 仍可能调整。用于实际业务前，请先用自己的样本验证。

文字提取不是通用阅读顺序或段落还原；中文支持不等于完整字体排版；渲染也不承诺覆盖所有 PDF 特性。完整批注与表单、增量保存、签名、真正脱敏及 OCR 不属于当前版本范围。覆盖文字或删除绘制操作不等于安全擦除。

具体输入限制、字体与图片支持及平台验证范围见 [支持矩阵](docs/releases/v0.1-support.md)。

## 文档与开发

- 使用指南：[文字提取](docs/text-extraction.md) · [图片提取](docs/image-extraction.md) · [内容编辑（实验）](docs/content-editing.md)
- 项目设计：[架构](docs/architecture.md) · [缓存与生命周期](docs/cache-lifecycle.md)
- 版本演进：[路线图](ROADMAP.md) · [变更记录](CHANGELOG.md)
- 开发与验证：[任务与实施记录](TODO.md) · [测试说明](docs/testing.md) · [v0.1 验收记录](docs/releases/v0.1-acceptance.md)

工作区由三个 crate 组成：`pdf` 提供核心能力，`render` 提供 Skia 渲染后端，`font-data` 提供内置字体与映射数据。

运行回归测试：

```bash
cargo test --workspace --locked
```

欢迎提交包含最小复现和预期结果的 issue。贡献代码时请附相应测试；涉及字体或 PDF 样本时，请确认可以公开分发。

## 许可证

项目使用 [GNU GPL v3](LICENSE)。内置字体、字宽数据及 CMap 另有来源和许可要求，见 [第三方数据声明](font-data/NOTICE.md)；分发时不能仅附项目根许可证。

---

原 README 中的详细接口说明和逐次开发状态已移至 [历史归档](docs/readme-history.md)，当前使用与开发请以上述专题文档为准。
