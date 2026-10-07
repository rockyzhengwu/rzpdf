# 解析回归与异常输入验证

## 运行

```sh
cargo test --workspace --locked
cargo test -p pdf --test parser_robustness --locked
```

`pdf/tests/parser_robustness.rs` 不引入随机库或额外工具链：固定种子与生成算法保证失败可复现。

- 1,024 组手写语法/内容种子的删除、插入和替换扰动，在严格/兼容模式下检查对象解析和内容操作符解析不 panic。对象解析成功必须推进输入位置。
- 256 组现有 PDF 的截断/单字节替换，在两个模式下经过完整文档入口；打开成功时继续验证页数、页面访问和 DisplayList 构建。
- 2,048 组 Flate/LZW/ASCIIHex/ASCII85/RunLength 编码扰动，使用 0–64 字节输出限额；成功输出必须不超过限额。另有 Predictor 非法类型、零尺寸及非法位深专项回归。
- 使用小 Limits 约束 token、深度、流和对象数量；接受明确解析错误，不要求异常输入成功。

语法种子由项目测试代码生成，不含外部样本文本。完整文件测试复用仓库既有 `tests/path-test.pdf`，来源及许可仍需追溯；没有新增第三方 PDF。该测试不证明所有异常输入均安全，不检测视觉正确性，也不能代替覆盖率引导模糊测试。

## 待补齐的验收

1. 为真实样本登记来源、许可、支持特性和预期行为。CCITT 原缺失样本测试已改为程序生成 Group4 条纹数据，检查逐像素解码和 BlackIs1 两种极性，不再忽略；仍需独立真实编码样本以避免编解码器共享缺陷。
2. 已新增独立 [fuzz 工作区](../fuzz/README.md)，分别覆盖语法、完整文档、内容解析/解释和五类流解码；四目标本地 AddressSanitizer 短时覆盖率引导结果见 [验证记录](fuzz-smoke-validation.md)。更多种子登记、失败输入最小化和持续执行仍需验收，短时无崩溃不代表完整安全性。
3. CI 已配置 60 分钟任务超时，包含原生依赖构建；仍需远端执行验证并按运行数据调整。本地确定性测试不使用线程内超时，不能据此宣称所有死循环都已排除。
4. 增加独立结构工具和阅读器验证，及固定渲染对比样本。

P0-12 是否完成以 TODO 的全部验收为准，而不是单个冒烟测试通过。按用户要求，fuzz 实现、执行及 CI 接入暂缓，保留已有文件和历史记录。

## 构建示例独立验证（2026-10-06）

使用现有 `create_document` 示例生成两页 PDF，再由独立工具检查结构、提取文字和渲染。样本为本项目生成，无新增第三方 PDF。

```sh
cargo run -p pdf --example create_document --locked --quiet -- output/pdf/build-validation.XQhHb5/created-document.pdf
qpdf --check output/pdf/build-validation.XQhHb5/created-document.pdf
qpdf --show-npages output/pdf/build-validation.XQhHb5/created-document.pdf
mutool draw -F txt -o - output/pdf/build-validation.XQhHb5/created-document.pdf
mutool draw -r 72 -o output/pdf/build-validation.XQhHb5/mupdf-page-%d.png output/pdf/build-validation.XQhHb5/created-document.pdf
```

- qpdf 11.2.0：结构和流编码检查通过，页数为 2；不等于完整规范符合性证明。
- MuPDF/mutool 1.27.0：两页文字均成功提取，标题 `rzpdf - Creation example` 及各页 `Page 1/2 | Text, image and metadata` 正确。
- 按 PDF 技能逐页人工查看 72 DPI 渲染图：标题、页码及红蓝图片可见，无明显裁切、重叠或异常黑块。
- PDF SHA-256：`d0a718f94de2d2a53917792b108b488cb138c78621363ea79d12b189c159e008`。

本地验证产物：[PDF](../output/pdf/build-validation.XQhHb5/created-document.pdf)、[第一页](../output/pdf/build-validation.XQhHb5/mupdf-page-1.png)、[第二页](../output/pdf/build-validation.XQhHb5/mupdf-page-2.png)。复现时请使用新目录，不覆盖既有产物。

环境限制：本机 Poppler 因缺少 `libtiff.5.dylib` 无法启动，Swift/PDFKit 回退因 JIT 缺少 `PDFDocument` 类符号失败，均不计为通过，未修改系统依赖。成功验证来自 qpdf 和 MuPDF。

范围仅为当前标准字体英文、RGB 图片及带 Info/XMP 的两页构建样本；未独立校验全部元数据内容，不覆盖真实复杂字体、编辑后文件、导入重排、加密签名或完整样本矩阵。整体任务保持未完成，本次未运行 fuzz。
