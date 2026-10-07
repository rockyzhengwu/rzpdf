# RGBA/SMask 独立像素验证

日期：2026-10-03，macOS。示例：[create_transparency.rs](../pdf/examples/create_transparency.rs)。

```bash
mkdir -p output/pdf tmp/pdfs
cargo run --example create_transparency --locked -- output/pdf/rzpdf-transparency-example.pdf
qpdf --check output/pdf/rzpdf-transparency-example.pdf
mutool draw -r 72 -o tmp/pdfs/rzpdf-transparency.png output/pdf/rzpdf-transparency-example.pdf
mutool draw -r 72 -F ppm -o tmp/pdfs/rzpdf-transparency.ppm output/pdf/rzpdf-transparency-example.pdf
python3 docs/check-transparency-pixels.py tmp/pdfs/rzpdf-transparency.ppm
```

示例创建 300×100 点页面，使用直通 RGB [200,40,20] 与 alpha 0/128/255；RGB 和 SMask 流按 Flate 输出。qpdf 11.2.0 未报告语法/流编码错误。MuPDF 1.27.0 72 dpi 输出三处中心像素：

| 像素坐标 | 理论白底 RGB | MuPDF 实测 RGB |
|---|---|---|
| (50,50)，alpha 0 | [255,255,255] | [255,255,255] |
| (150,50)，alpha 128 | [227,147,137] | [226,146,136] |
| (250,50)，alpha 255 | [200,40,20] | [200,40,20] |

每通道容差 1，检查脚本实际通过；给脚本 PDF 而非 PPM 时退出码 1。逐页查看 PNG，白色、半透明、原色三区域显示正常。该脚本仅接受固定尺寸/头格式的示例 PPM，读取长度有上限，不是通用图像解析器。

Skia 单元回归使用同样的 RGB/alpha 和白底期望，保存前及严格读回后均满足每通道容差 1。工作区 pdf 单元 228、集成 44、render 7 全部通过，0 忽略。

范围限制：仅验证标准同尺寸 8 位 DeviceGray SMask 与基础白底合成，不覆盖边缘插值、其他底色、颜色管理、重采样、Matte、自定义 Decode、混合模式或完整透明组。Poppler 因本地依赖问题仍不可用，本次未修改系统依赖；不能把单例通过视为完整 PDF 透明度兼容性。

## PNG 导入路径补充

示例增加可选 `--png`，先在内存生成 RGBA PNG，再调用 embed_png_image，后续 PDF 输出及验证相同：

```bash
cargo run --example create_transparency --locked -- output/pdf/rzpdf-png-transparency-example.pdf --png
qpdf --check output/pdf/rzpdf-png-transparency-example.pdf
mutool draw -r 72 -o tmp/pdfs/rzpdf-png-transparency.png output/pdf/rzpdf-png-transparency-example.pdf
mutool draw -r 72 -F ppm -o tmp/pdfs/rzpdf-png-transparency.ppm output/pdf/rzpdf-png-transparency-example.pdf
python3 docs/check-transparency-pixels.py tmp/pdfs/rzpdf-png-transparency.ppm
```

实际运行通过，MuPDF 三处像素与上表实测值一致，逐页 PNG 查看正常，qpdf 无语法/流编码错误；工作区 pdf 单元 228、集成 54、render 7 全部通过，0 忽略。此验证覆盖 PNG 解码→RGB/SMask→Flate PDF→独立渲染，不证明调色板/灰度/交错/色彩管理等全部 PNG 路径。
