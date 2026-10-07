# 水印与页码样本验收

2026-10-04，使用本项目自编的两页英文/图片样本运行：

```sh
cargo run -p pdf --example overlay_document --locked -- \
  output/pdf/rzpdf-creation-example.pdf OUTPUT.pdf
qpdf --check OUTPUT.pdf
mutool draw -F txt OUTPUT.pdf
mutool draw -r 72 -o page-%d.png OUTPUT.pdf
```

实际产物：`output/pdf/rzpdf-overlay-example.pdf`。
SHA-256：`d23401d5dad3636d82b85ec0b532ac2cef24ed9894428daf088378fb947800b4`。
PDF 字典排序可能导致再次生成的字节哈希不同，哈希仅标识本次检查产物。

示例实际运行成功，写出前本库严格读回并构建两页 DisplayList。
qpdf 11.2.0 未发现语法或流编码错误；MuPDF 1.27.0 提取每页原有标题/正文、DRAFT 和对应 Page 1 / 2、Page 2 / 2，页序正确。
逐一查看两张 72 DPI PNG：原文字与双色图片保留，斜向浅灰水印和底部页码清晰，没有裁切或重叠。
依照 PDF 技能进行了渲染后视觉检查；沿用当前可用 MuPDF，未修改缺依赖的系统 Poppler。

此结果仅覆盖固定 A4、标准字体和默认用户空间定位。没有验证所有旋转/CropBox、内联图片、未知操作或复杂图形状态；不表示自动布局、透明度、中文组合快捷接口或整体内存预算完成。覆盖保留原内容，不可用于安全涂黑。fuzz 按用户要求暂缓。

## 可重复只读检查

```sh
bash docs/check-overlay-example.sh output/pdf/rzpdf-overlay-example.pdf
```

脚本运行 qpdf 结构检查，再按 MuPDF 分页提取结果逐页核对四行文字；只忽略空行和行首尾空白，不忽略页序或分页。依赖 qpdf、mutool、python3，不修改 PDF。脚本语法、真实产物正向检查通过；未覆盖的原始样本负向检查返回 1，无参数返回 2。文字检查不能证明水印的位置、角度、颜色或图片正确，不替代上面的渲染检查。

## 真实中文字体覆盖

2026-10-04 使用 `docs/cjk-creation-validation.md` 已登记的许可字体静态实例（实际 PostScript 名 NotoSansSC-Thin，CapHeight 733，示例近似 StemV 80）：

```sh
cargo run -p pdf --example create_cjk --locked -- \
  FONT.ttf NotoSansSC-Thin 733 80 OUTPUT.pdf --overlay
```

实际产物 `output/pdf/rzpdf-cjk-overlay-example.pdf`，SHA-256 `7d95a619b5ac9ecf716eec8cbf8069c60d9ab605eeeab2f07080e3e6ef5e922c`。
本库严格保存读回并解释两页通过，qpdf 结构检查通过，JSON 确认只有一个 FontFile2 描述引用。
MuPDF 按页提取原有四行正文、每页“草稿”和对应“第 1 页 / 共 2 页”“第 2 页 / 共 2 页”，没有缺字。
依照 PDF 技能逐页查看两张 72 DPI PNG，标题、正文、斜向浅灰草稿及页脚正确，无重叠、缺字或裁切。
字体来源、转换、许可和描述度量局限沿用中文创建验收文档；本次仍不证明自动旋转布局、透明度、复杂内容及聚合预算完成。

本库提取验证：`cargo run -p pdf --example extract_text --locked -- output/pdf/rzpdf-cjk-overlay-example.pdf` 成功；输出逐页原文、水印和中文页码，与 MuPDF 分页提取逐字一致（仅规范化空行和首尾空白）。这是绘制顺序/文字运行分隔，不表示阅读顺序重建；本轮只读提取、未改 PDF，沿用先前 PDF 技能视觉验收。
