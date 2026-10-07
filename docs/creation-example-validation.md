# 构建示例验证

日期：2026-10-03；平台：macOS；源码：[create_document.rs](../pdf/examples/create_document.rs)。

## 生成与检查

```bash
mkdir -p output/pdf tmp/pdfs
cargo run --example create_document --locked -- output/pdf/rzpdf-creation-example.pdf
qpdf --check output/pdf/rzpdf-creation-example.pdf
bash docs/check-creation-example.sh output/pdf/rzpdf-creation-example.pdf
mutool draw -F txt output/pdf/rzpdf-creation-example.pdf
mutool draw -r 72 -o tmp/pdfs/rzpdf-creation-%d.png output/pdf/rzpdf-creation-example.pdf
```

qpdf 11.2.0：PDF 1.7，未加密，无语法或流编码错误报告；该检查不证明全部符合性。

MuPDF/mutool 1.27.0：成功渲染两页；文本提取分别包含 `rzpdf - Creation example` 与 `Page 1 | Text, image and metadata` / `Page 2 | Text, image and metadata`。逐页查看 72 dpi PNG，标题、说明、红蓝 RGB 图片及边框正常，无可见重叠或裁切。

工作区测试：pdf 单元 227、集成 40、render 6 全部通过，0 忽略。示例可编译运行。

### 外部元数据检查补充

qpdf JSON 从 Trailer 动态定位 Info（不硬编码对象号），解码 Title 为 `rzpdf 构建示例`、Author 为 `rzpdf`、Creator 为 `rzpdf create_document example`；动态定位 Catalog Metadata 并确认 Type/Subtype 和两页 Count。MuPDF 提取并解压原始 packet，libxml 2.9.13 的 xmllint 检查 XML 良构，通过命名空间 URI 定位 dc:creator/rdf:Seq/rdf:li，作者值与 Info 相同。

[check-creation-example.sh](check-creation-example.sh) 固化上述检查和两页文本期望，实际运行通过；无参数时退出码为 2。依赖 qpdf、mutool、jq、xmllint；仅针对本仓库示例的固定期望，不是通用 XMP/PDF 校验器，不对不可信输入的外部工具执行提供安全隔离。XML 良构和一个字段一致不等于全部 XMP schema/语义验证。

## 环境与限制

系统 Poppler 22.06.0_1 的 pdfinfo/pdftotext/pdftoppm 因缺少 libtiff.5.dylib 无法启动，改用独立 MuPDF 验证；未修改系统依赖。

示例仅覆盖英文标准字体、两页 A4、RGB 图片、基础路径、Info 和原始 Metadata packet；不嵌入中文正文字体，不验证 XMP 全部语义、自动同步、字体嵌入、透明图片或 PDF/A。中文 Info 标题已通过 qpdf 外部读取，但未覆盖全部元数据编码。生成 PDF 与 PNG 是本次本地运行产物，不作为大范围兼容性样本或稳定二进制快照。
