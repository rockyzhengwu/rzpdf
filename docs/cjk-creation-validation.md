# 中英文整字体构建验证

2026-10-04：新增 `pdf/examples/create_cjk.rs`，创建两页 A4 中英文，使用本项目字体注册、CID 编码、保存接口；第二页复用同一字体。保存后严格打开并构建两页 DisplayList。按 PDF 技能要求渲染检查，不使用外部库替代本项目构建。

## 字体来源和范围

验证输入是 [Google Fonts Noto Sans SC](https://github.com/google/fonts/tree/main/ofl/notosanssc) 的 `NotoSansSC[wght].ttf`，采用 [SIL OFL 1.1](https://raw.githubusercontent.com/google/fonts/main/ofl/notosanssc/OFL.txt)，保留原许可。仅在临时验证目录下载，不把字体源加入仓库。FontTools 4.39.4 固定全部轴为 wght=400，输出静态 TrueType；这是外部样本准备，不是 rzpdf 已支持可变字体实例化。原实例仍保留 name ID 6 的 `NotoSansSC-Thin` 名称，因此传入该实际名称，不把文件名当作字体名称。不要据此安装或发布为官方 Regular 字体。

输入 variable SHA-256：`a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da`。
静态实例 SHA-256：`0516f3d1fb52efaf83f1d7bc540d779d1eb405f37e0dbb7696537444a0931087`。
实例有 31036 字形，fsType=0，OS/2 CapHeight 归一化后为 733；StemV=80 是本次显示冒烟采用的显式近似值，不声称已从轮廓测量或足以证明描述字典完全准确。真实指标自动推导仍待实施。OFL 保留名称为 Source；静态实例未使用该保留名称。

## 复现

先自行取得许可允许使用的字体，创建输出目录；保存会替换指定现有普通文件。

```sh
fonttools varLib.instancer 'NotoSansSC[wght].ttf' wght=400 --no-recalc-timestamp -o NotoSansSC-static.ttf
cargo run -p pdf --example create_cjk --locked -- NotoSansSC-static.ttf NotoSansSC-Thin 733 80 output/pdf/rzpdf-cjk-example.pdf
qpdf --check output/pdf/rzpdf-cjk-example.pdf
bash docs/check-cjk-example.sh output/pdf/rzpdf-cjk-example.pdf
mutool draw -F txt output/pdf/rzpdf-cjk-example.pdf
mutool draw -r 72 -o page-%d.png output/pdf/rzpdf-cjk-example.pdf
```

远端字体可能更新；以哈希和实际字体名称/指标为准，不盲用上述参数。交付输出本次 SHA-256 为 `8524d35a242f2248915b3b757fcf9659d21b68f5caec71f0b8e43a38beb53487`；文件哈希用于标识本次产物，不保证重复生成的字节顺序完全一致。

## 结果与修复

`check-cjk-example.sh` 是只读的精确示例验收脚本，依赖 qpdf/mutool/jq：检查两页、两页字体指向同一对象、Type0/Identity-H、CIDFontType2、FontDescriptor/FontFile2、字宽、映射流和 Unicode 标题，逐字核对四行及顺序（仅移除空行/分页符）。真实产物通过；既有英文创建样本被拒绝；缺参数返回 2。脚本只适用于本示例的直接页面资源结构，不是通用字体分析器，不验证完整字体轮廓或视觉质量；PNG 检查仍单独进行。

真实字体包含 cmap 4/12/14/6。运行发现旧读取对 14 误用 16 位长度并因额外未支持表拒绝字体，现按 OpenType 声明长度校验边界/最小头及累计预算，再跳过未实现表，保留 0/4/12；无受支持表仍显式拒绝。新回归验证辅助表共存和截断，不声称支持变体序列或未实现表内部语义。

qpdf 11.2.0：两页 PDF 1.7 无语法或流编码错误；不表示全部 PDF 语义正确。
MuPDF 1.27.0：文本提取包含以下四行（允许提取器增加空行/分页）：

```text
rzpdf - 中文字体嵌入
你好，世界！Hello PDF
第二页 - 同一字体跨页复用
中文可显示，也应正确提取。
```

两页 72 DPI PNG 均已目视检查：中文和拉丁文字可读，无缺字方框、重叠或裁切。Poppler 的既有 libtiff 依赖问题未修改系统，使用 MuPDF 替代。样本限于这款字体和水平简单文字，不覆盖所有中文/补充平面字形、shaping、变体、子集、CFF、竖排、PDF/A 或本库 P2 文本提取接口；P1-05/P1-10 完整验收仍不勾选。未运行 fuzz。
