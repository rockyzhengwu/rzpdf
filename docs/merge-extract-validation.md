# 页面合并与选页验证

2026-10-04：新增 merge_document（原子追加一个源文档全部页）、extract_pages（新文档中的唯一有序选页，沿用源有效版本和 ParseOptions），及 merge_documents/extract_pages 两个 Rust 示例。操作由本项目完成，不用外部 PDF 库代替导入/保存。示例保存后严格打开，核对页数并构建所有页 DisplayList。

## 复现

使用已有中英文两页样本（字体来源/许可见 [中文构建验证](cjk-creation-validation.md)）及英文/图片两页样本。

```sh
cargo run -p pdf --example merge_documents --locked -- output/pdf/rzpdf-merged-example.pdf output/pdf/rzpdf-cjk-example.pdf output/pdf/rzpdf-creation-example.pdf
cargo run -p pdf --example extract_pages --locked -- output/pdf/rzpdf-extracted-example.pdf output/pdf/rzpdf-merged-example.pdf 3 0
qpdf --check output/pdf/rzpdf-merged-example.pdf
qpdf --check output/pdf/rzpdf-extracted-example.pdf
bash docs/check-merge-extract-example.sh output/pdf/rzpdf-merged-example.pdf output/pdf/rzpdf-extracted-example.pdf
mutool draw -F txt output/pdf/rzpdf-merged-example.pdf
mutool draw -F txt output/pdf/rzpdf-extracted-example.pdf
mutool draw -r 72 -o merged-%d.png output/pdf/rzpdf-merged-example.pdf
mutool draw -r 72 -o extracted-%d.png output/pdf/rzpdf-extracted-example.pdf
```

先创建目标目录；输出普通文件会按保存策略替换。索引从 0 开始、不可重复；3 0 代表原第四页、第一页，正文内的页号不会自动改写。多个源文件按命令行顺序追加；共享映射范围为每个源文档的一次导入，不按不同文件的相同字体字节去重。单次 merge_document 失败会回滚该源导入；多次调用若要求整组回滚，应由调用方包入 edit_transaction。示例所有输入导入成功后才写输出。

## 实测结果

只读 `check-merge-extract-example.sh` 使用 qpdf/mutool/jq，检查合并 4 页/1 FontFile2 描述引用/2 Image、选页 2 页/1 FontFile2/1 Image，逐字核对提取文本及页序，仅去除空行/分页符。真实产物通过；交换输入顺序被拒绝；缺参数返回 2；bash 语法检查通过。脚本只针对这里的固定输入/选择，不是任意合并文件的验证器；字体引用语义和视觉质量不能只靠对象数量证明，渲染检查仍单独进行。

本库严格读回及每页 DisplayList 成功；qpdf 11.2.0 两产物均无语法/流编码错误。qpdf JSON 核对合并结果 4 个 Page、1 个 FontFile2 描述引用、2 个 Image，中文两页的嵌入字体没有重复复制。

MuPDF 1.27.0 提取合并结果的四页顺序为中文样本第 1/2 页、英文图片样本第 1/2 页。选页结果为英文 `Page 2 | Text, image and metadata`，然后中文 `你好，世界！Hello PDF`，与 3 0 相符。按 PDF 技能逐页检查合并 4 张及选页 2 张 72 DPI PNG：中文/英文可读，图片红蓝区域正确，无缺字方框、重叠或裁切。Poppler 既有依赖问题未修改系统，使用 MuPDF 替代。

本次合并 SHA-256：`b4254891af03d60c38dc352b6347dd5586d1388b167f6b95e7caa009914cf674`。
选页 SHA-256：`5b4deec9ec0ee4d36f485f01b1a4a06f632cbc881f3c8291e893a9894371b7d1`。
哈希仅标识本次文件，未承诺重复生成字节顺序一致。

单元测试覆盖追加页序/旋转、选页顺序/新身份/源选项、源 revision 不变、重复选择拒绝及严格读回。现有导入回归另覆盖循环共享和失败回滚。

## UserUnit 补充验证（2026-10-05）

`page_transfer_edit` 回归覆盖 PDF 1.6 UserUnit 的批量导入、合并、重排抽取、严格保存重读及低版本目标拒绝；另覆盖叶页缺失/直接 null/间接 null/间接数值，不继承父 Pages 的同名字段。当前该集成组全部 4 项通过。

独立显示样本由本项目生成，未使用外部库代替构建。复现前确认目标路径没有需要保留的文件（示例保存会替换目标）：

```sh
cargo run -p pdf --example create_user_unit --locked -- output/pdf/rzpdf-user-unit-validation.pdf
qpdf --check output/pdf/rzpdf-user-unit-validation.pdf
bash docs/check-user-unit-example.sh output/pdf/rzpdf-user-unit-validation.pdf
mkdir -p tmp/pdfs/user-unit
mutool draw -r 72 -o tmp/pdfs/user-unit/page-%d.png output/pdf/rzpdf-user-unit-validation.pdf
mutool draw -F txt output/pdf/rzpdf-user-unit-validation.pdf
```

样本使用非零 CropBox、Helvetica 文字、红绿 RGB 图片及蓝色路径。前四页 UserUnit=1.5、Rotate=0/90/180/270，后两页单位为0.5/2.5且不旋转。qpdf 11.2.0 检查通过；MuPDF 1.27.0 的72 DPI PNG尺寸依次为240×120、120×240、240×120、120×240、80×40、400×200，已逐页视觉检查文字、方向与图块位置；文本提取六条标签完整。Poppler因本机缺少libtiff.5.dylib退出134，未调整系统依赖。

只读脚本 `check-user-unit-example.sh` 使用qpdf/mutool/jq，按实际页树顺序核对PDF1.6、六页的单位/旋转/MediaBox/CropBox、逐页2×1 RGB图片属性与全部提取文本，不依赖固定对象编号或对象字典遍历顺序。原样本通过，交换前两页被拒绝，缺参数退出2，bash语法检查通过。脚本不渲染、不修改输入，不用这些字段检查代替视觉验收。

此独立样本验证的是构建输出，不代表已用独立阅读器验收所有导入/合并产物、任意单位、字体或图片类型。项目内的单位/DPI等价像素回归另覆盖0.5/1.5/2.5、四旋转及各向异性DPI；完整跨阅读器矩阵仍待完成。

## 完整验收边界

这是普通无注释、未标记页面的合并/选页闭环，不自动合并 Info/XMP、书签、命名目标、结构树或标签；注释、文章、标记结构、加密源仍拒绝。extract_pages 现通过显式创建版本保留源有效版本，包括 PDF 2.0；Catalog /Version 高于头时输出头采用有效版本。合成空内容页测试覆盖创建/严格读回及保存拒绝降级，真实 PDF 2.0 高级特性仍未验收。普通 merge_document 要求目标版本不低于源，调用方可用 new_with_version 选择；merge_documents 示例预先读取全部输入并按最高有效版本创建目标。并非通用 PDF 合并器或所有真实文件/复杂排版/PDF/A 验收；P2-04/P2-05 完整范围不勾选。源对象克隆/容器的峰值内存仍未完整计费。未运行 fuzz。
