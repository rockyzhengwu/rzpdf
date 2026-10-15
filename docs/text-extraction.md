# 文本提取与顺序

`extract_characters_paint_order_report()` 返回原始解释绘制顺序的字符事件与内容诊断。`extract_text_runs_paint_order_report()` 在同次解释结果上移动字符，返回文本段与诊断，不复制Unicode负载。`extract_characters_paint_order()` 与 `extract_text_runs_paint_order()` 是简便入口，会丢弃诊断；文本段分组的是解释文字对象，不是段落或文本流来源范围。

每个事件保留字符码/码长、可选 Unicode、声明字号、字体名、书写模式、水平缩放、显示空间文字原点与文字段矩阵。一个 PDF 字符码可以映射多个 Unicode 字符，缺映射保留 `None`，不与显式 U+FFFD 混淆。BaseFont 名不是资源键或稳定字体身份。

位置使用旋转/裁剪后的显示空间，左上角原点、y向下。矩阵包含 Ts 抬升，平移是文字段初始原点，不随各字符推进。Tz 单独保留为因子；`writing_axis()` 不是单位向量，也不是带字宽/字距的实际推进。

竖排的 `position` 仍为文字原点，尚未扣除 W2 字形偏移；`vertical_origin` 保存从水平字形原点到竖排文字原点的向量（千分之一文字空间，水平字体为零）。`glyph_origin_display()` 单独给出实际水平轮廓原点，`em_to_display_matrix()`、`display_to_em_point()` 与文字对象轮廓辅助接口使用该原点。不要重复扣除偏移。接口示例和迁移说明见 [字形运行指南](glyph-runs.md)；当前字体组合见 [v0.7 矩阵](releases/v0.7-font-matrix.md)。

阅读顺序与绘制顺序分开：

```rust
let report = page.extract_characters_paint_order_report()?;
let order = report.single_column_reading_order_indices(
    2.0, // 显示空间基线容差，由调用方选择
    &limits,
)?;
for index in order {
    let character = &report.characters[index];
    // 按选定策略访问，不改变原始绘制事件及诊断。
}
```

该接口是调用方显式选择的单栏、显示水平、从左到右空间策略：先按 y 排序，按最上方基线与容差分行，再按 x 排列，同位置保持原绘制索引顺序。不是语义阅读顺序、Tagged PDF 结构顺序、双向文字重排、多栏检测或段落恢复。旋转后的非水平文字、竖排、负/零水平轴明确拒绝；容差为负或非有限也拒绝。Unicode 双向语义不在该空间排序策略中解释。

索引结果保留对原事件的引用，不复制文字/字体负载；分配受字符数与索引字节额度限制。负/零/非有限字号不适用此策略，明确拒绝。基线分组以每行最上方基线为固定锚点，不通过相邻字符差值连锁扩大；容差0只合并相同基线。诊断必须由调用方检查，排序不会修复跳过的操作或缺失 Unicode。

当前不处理注释外观、完整字体回退、ActualText/结构树语义；提取和容器峰值仍待统一预算。`cargo test -p pdf --test overlay_workflow --locked` 覆盖构建、保存严格读回、字距/抬升/方向及空间顺序专项；不代表完整 P2-01 验收。
