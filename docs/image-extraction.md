# 图片提取接口

内联 BI/ID/EI 和 JPX/JBIG2 基础入口见 [图像编解码指南](image-codecs.md)，具体输入范围见 [v0.6 矩阵](releases/v0.6-matrix.md)。`extract_inline_image_source(program, operation)` 返回经过页/修订认证的原载荷；内联来源没有命名资源，绘制事件通过 `image.is_inline()` 区分，解码结果仍走 paint-order API。

页面目前提供以下入口，语义不同，不应混用：

| 入口 | 返回内容 | 顺序与范围 |
| --- | --- | --- |
| `image_resource_names()` | 命名图片资源名 | 稳定字典序，包含未绘制资源 |
| `extract_image_source(name)` | 原始流及字典、宽高、可选 BPC | 指定有效页资源，编码可以尚不支持 |
| `extract_inline_image_source(program, operation)` | 类型化内联原字典、编码及数据范围 | 当前页和修订的一个 BI 内容项 |
| `extract_decoded_image(name)` | `PdfImage` 规范化样本模型 | 指定有效页资源，不要求页面实际绘制 |
| `extract_images_paint_order()` | `Vec<ImageObject>` | 解释器绘制顺序，重复绘制分别返回 |
| `extract_images_paint_order_report()` | 图片绘制及诊断 | 同次解释保留被跳过操作的警告 |

```rust
use pdf::document::owned::Document;

let document = Document::open("input.pdf")?;
let page = document.get_page(0)?;
for name in page.image_resource_names()? {
    let source = page.extract_image_source(&name)?;
    let original_bytes = source.stream.raw_data();
    let original_dictionary = source.stream.dict();
    // 可检查 Filter/DecodeParms；不是所有流字节都能直接作为 PNG/JPEG 文件。
    let decoded = page.extract_decoded_image(&name)?;
    let rgba = decoded.rgba_buffer()?;
}
```

`PdfImage::data()` 是解码并规范化后的颜色样本，不能假定与原始位深/编码一致。`rgba_buffer()` 转换到 RGBA，并应用已支持的软遮罩；颜色键遮罩、模板图片及其他不支持的路径会明确报错。

原始流字典中的间接引用仍指向源文档。返回值是源快照，不是可独立导入其他文档的对象图，也不是已经导出的独立图像文件。未知 Filter 可以原样提取和保存，但调用解码接口会失败。

绘制提取的矩阵把单位图片方形变换到页面显示空间：裁剪/旋转后的左上角原点、y 向下。它不是未经页面变换的 PDF 用户坐标。未使用资源只在资源列表中出现。

额度包括资源扫描条目、名称字节、原始流字节，以及绘制提取返回的规范化样本与软遮罩累计字节；重复绘制按重复负载计费。现有页面资源解析和 DisplayList 构建期间的克隆、容器及峰值内存尚不具备统一预算，不能把结果额度当成整个操作的内存上限。

当前不递归发现 Form 内资源，不支持内联图片。绘制提取覆盖范围与 DisplayList 一致；兼容解析模式应优先使用 `extract_images_paint_order_report()` 并检查其 `diagnostics`。简便的 Vec 返回接口会丢弃诊断。不能把返回图片数或没有诊断当成页面全部特性已支持的证明。

回归入口：`cargo test -p pdf --test image_extraction --locked`。包括未使用资源、未知编码保留、严格保存读回、同图重复绘制、透明度及累计样本预算精确边界。P2-02 仍未整体验收。
