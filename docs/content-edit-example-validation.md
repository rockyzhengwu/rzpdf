# 内容编辑示例验证

日期：2026-10-05。示例：`pdf/examples/edit_content.rs`；产物：`output/pdf/rzpdf-edit-content-example.pdf`。

自动检查：`bash docs/check-content-edit-example.sh output/pdf/rzpdf-edit-content-example.pdf`，依赖qpdf/mutool/jq/python3，退出0。只读取输入PDF，不修改或生成其页面。检查两页顺序/MediaBox、独立Contents、跨页同一图片/Helvetica字体引用、图片尺寸、精确提取标题，以及MuPDF绘制trace的文字/图片/填充事件数、图片矩阵和邻接蓝路径一致性。trace XML由Python标准库解析，不依赖字符串匹配计数。

负对照：使用qpdf另行生成仅第一页的合法PDF，qpdf结构检查仍通过，但样本脚本退出1；bash -n及git diff --check通过。说明脚本不是仅检查PDF可打开。此脚本针对当前示例结构，不接受任意等价编码或页面树布局；绘制事件检查不替代像素/视觉/字体符合性验收。当前PDF未改写，无需因新增只读脚本重新生成视觉产物，前次两页PNG目检证据仍适用。

运行：`cargo run -p pdf --example edit_content --locked --quiet`。默认输出存在时拒绝覆盖，可传新的文件路径。第一页为原稿，第二页复制后在事务内局部替换标题、显示空间旋转第一图、删除第二图绘制并取消红矩形填充；蓝矩形保持。每次编辑刷新来源快照，保存前项目自身严格重开并解释两页。

独立检查：`qpdf --check output/pdf/rzpdf-edit-content-example.pdf` 退出0，未发现语法/流编码错误。`mutool draw -q -F txt ...` 退出0，依次提取 `Original content` 与 `Edited content`。

按PDF技能渲染流程先尝试系统pdftoppm；因Poppler缺失libtiff.5动态库退出134，未修改系统依赖。改用 `mutool draw -q -r 100 -o tmp/pdfs/edit-content-validation/page-%d.png ...`，退出0。已目检两页PNG：标题可读、无裁剪重叠；第一页横向红绿纹理及重复图、红蓝矩形均在；第二页红绿纹理转为纵向、重复图及红填充消失、蓝矩形保持，原稿不受编辑影响。

这是自编两页样本的qpdf/MuPDF证据，不证明全部字体、任意仿射/透明裁剪、Form/内联或安全擦除符合性，不替代完整P1/P3样本矩阵。Poppler验证未成功，fuzz未运行。
