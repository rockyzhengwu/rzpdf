# Web API（ABI 2，功能补齐进行中）

本页对应 WA-WEB-01～03 的当前接口。接口与阅读器已增加显式密码、解析模式、
页几何、默认 View 可见文本、跨页搜索及导航。字体服务、
批注/表单、编辑/保存仍在原目标内，尚未交付，不把本阶段计为整个 Web 专题完成。

首选异步 Worker 接口，取消已派发的任务会终止整个会话并使所有文档失效：

```ts
import {createWorkerEngine} from './dist/worker-client.js';
const engine = await createWorkerEngine();
try {
  const doc = await engine.open(bytes, {mode: 'compatible'});
  try {
    const controller = new AbortController();
    const image = await doc.render(0, 72, {signal: controller.signal});
    const text = await doc.extractText(0);
    const matches = await doc.searchText('查找内容');
  } finally {
    await doc.close();
  }
} finally {
  engine.dispose(); // 幂等；不再自动重建
}
```

下面的同步入口供底层诊断使用；阅读器不在主线程调用它来解析或渲染：

```ts
import {createEngine, PdfApiError} from './dist/client.js';
const engine = await createEngine();
const doc = engine.open(new Uint8Array(await file.arrayBuffer()), {
  mode: 'compatible',
  // password: 用户显式输入的密码
});
try {
  const info = doc.pageInfo(0);
  const text = doc.extractText(0);
  const matches = doc.searchText('查找内容', {maxResults: 1000});
  const bookmarks = doc.outlines();
  const links = doc.links(0);
  const pixels = doc.render(0, 72);
} finally {
  doc.close(); // 幂等；关闭后调用明确失败
}
```

页码从 0 开始。`pageInfo` 返回旋转、CropBox/UserUnit 处理后的显示宽高（1/72
英寸），以及 PDF 用户空间到左上角/y 向下显示空间的矩阵。链接矩形明确保留
PDF 用户坐标。目标保留页码和完整 DestinationView，阅读器当前支持页跳转与
XYZ 定位；其余精确 fit 视图行为仍待补齐。

`extractText` 是绘制顺序，非推断阅读顺序，按与渲染相同的默认 View 图层选择。
字符返回 Unicode（缺失时为 null）、原码、位置、em 矩阵和显示空间 advance。
advance 含字/词间距，不含 TJ 数值调整；字符位置保留 TJ 的实际影响。它们可用
于逻辑选择框，不是字形墨迹轮廓、完整裁剪测试或 OCR。阅读器文字层以 U+FFFD
提示未映射字符；搜索把缺映射视作边界，不把两侧文字拼成假命中。搜索按 Unicode
lowercase 匹配，默认最多 1,000 处，可显式设置 1～10,000；截断、已扫描页数、
缺映射及诊断数量随结果返回。同步底层接口保留给 Worker 与诊断，阅读器已使用异步 Worker 包装。

`PdfApiError` 返回 code、operation、可选 page 和 message，区分格式、所需密码、
错误密码、权限、不支持、字体、资源、句柄、参数和内部错误。密码错误不使引擎
失效，可以重试。输入请求释放时清除其受控 Rust 字节，密码不写入日志或缓存；
不承诺 JS 不可变字符串、原生库临时数据或整个浏览器内存均可安全擦除。

禁止复制的用户认证文档仍可显示页面；提取/搜索返回 permission，阅读器禁用
文字层与搜索。Owner 认证沿用核心权限语义。链接只返回数据，不执行动作；
阅读器只为 http/https/mailto 创建由用户点击的外部链接，其他 URI 不激活。

保留 P0 的 PDF/字体/像素/线性内存限额。JSON 请求最多 64 KiB，输出最多
16 MiB；PDF 来源字节计入保留缓冲额度，输出为独立类别的缓冲句柄，不能作为
PDF、字体或请求输入。JS 复制输出后立即释放。限额不是浏览器总 RSS 保证。
`capabilities()` 中批注、表单、首批来源内容编辑与保存均已开放；具体支持范围见下文。

## Worker 任务与资源

任务通过一个 Worker 串行执行，普通任务合计最多 32 个，另留 8 个 close 控制
位置，输入传输额度 64 MiB。任务接受独立的 `{signal}`；默认执行超时 60 秒，
初始化超时 120 秒。已排队任务取消不影响其他任务；已派发任务取消、超时、
Worker 错误或 trap 会终止会话。错误带任务 ID 和 session，旧响应被丢弃。

不缓存密码、不自动重开文档；硬取消后可用同一个未 dispose 的引擎再次 `open`
创建新会话，旧文档始终无效。字体也需重新注册。用户输入 PDF/字体的原始数组
不被 detach；返回的 RGBA 是转移后由调用方拥有的独立 ArrayBuffer。请求选项
在入队时取快照，不受调用方之后修改影响。

`close()` 立即禁止该文档后续操作，并撤销该文档的排队任务；如果它正在执行，
将硬取消整个会话。关闭其他文档会按序释放，队列满时仍有控制位置。重复 close
等待同一释放结果。`dispose()` 立即终止 Worker 并拒绝未完成任务，之后调用失败。
同步底层也提供 dispose，释放跟踪资源且不复用历史句柄。

`statistics()` 仅记录 native 句柄、来源/缓冲/字体字节和 Worker 文档数，不包含
JS 返回值、所有原生缓存或总 RSS。12 次关闭/重开后这些跟踪值为零，不表示 Wasm
线性内存必须缩小。页面隐藏时阅读器 dispose；模拟页面恢复事件会新建会话，
要求用户显式重新打开，密码不恢复。真实 BFCache 和其他浏览器仍需平台矩阵验证。


## 批注、表单与保存

Worker 文档提供异步 `state()`、`annotations(page)`、`formFields()`、`apply()`
和 `save()`。同步诊断文档提供相应同步入口（save 的限额直接作为数值参数）。
每份读取快照带模块/文档/revision 的 stamp，修改必须原样携带；成功修改和事务
回滚都会使旧选择失效。失败后重新读取快照，不在旧 revision 上盲目重试。

```ts
const fields = await doc.formFields();
const field = fields.records.find(f => f.name === 'name' && f.details);
if (field) {
  await doc.apply({operation: 'setFormValue', stamp: fields.stamp,
    object: field.object, value: {kind: 'text', text: 'New value'}});
}
const annotations = await doc.annotations(0);
await doc.apply({operation: 'addAnnotation', stamp: annotations.stamp, page: 0,
  annotation: {kind: 'Square', rect: [20, 20, 120, 60], contents: 'Note'}});
const saved = await doc.save({maxBytes: 8 * 1024 * 1024});
// saved.bytes 独立拥有 PDF 字节；saved.stamp 标识保存状态。
```

未知字段/批注保留只读记录，禁止动作执行；字段 flags 和每个 Widget 的 flags、
visible、page、rect、onState 可用于构建页面交互。visible 是默认 View 图层及
Screen 标志的结果。填写受只读字段、字体表达、权限、签名和原生支持范围约束。
更新/删除批注使用该页记录中的 object 和当前 stamp；子类型不可随意更换。

`render(page, dpi, {annotations, includeWidgets, viewZoom})` 默认启用批注/Widget，
viewZoom 默认 1，范围 0.1～4，用于 NoZoom 外观补偿；DPI 与显示缩放分别传递。
阅读器在默认选择模式提供页面填写，绘制模式提供便笺/矩形/椭圆/文字框/高亮/
线段/手写。自由文字不自动排版，内容放不下时明确失败并刷新快照。

保存默认请求最多 64 MiB，实际输出仍受共享缓冲池剩余额度限制；返回完整结果后才提供下载。R6 保留密码与权限，R4 为只读；
签名等保护沿用核心规则，不自动移除。下载成功仍须与严格重开和独立检查区分。


## 来源内容与页面编辑

`content(page)` 返回默认 View 可见的字符记录、图片显示矩阵和 stamp。字符
start/end 是该快照中字符记录的半开索引，非 JS UTF-16 或 Unicode 字符串偏移；
一个 PDF 字符可能映射多个 Unicode 码点。当前替换范围必须属于同一绘制事件，
支持 TJ/嵌套 Form，使用原字体编码；默认保留后续文字位置，空字符串可删除。
普通替换不新增字形；新字形与跨事件范围使用下述显式接口。不提供 shaping、自动排版或安全脱敏。

```ts
const content = await doc.content(0);
await doc.apply({operation: 'replaceText', stamp: content.stamp,
  page: 0, start: 0, end: 3, text: 'New', preserveAdvance: true});
const fresh = await doc.content(0);
await doc.apply({operation: 'transformImage', stamp: fresh.stamp,
  page: 0, index: 0, matrix: [1, 0, 0, 1, 12, -8]});
const next = await doc.content(0);
await doc.apply({operation: 'replaceImage', stamp: next.stamp,
  page: 0, index: 0, format: 'png'}, {asset: pngBytes});
```

图片 index 属于本次可见图片列表，非对象编号；变换使用页面显示空间。PNG/JPEG
替换最多 8 MiB，与 JSON 分开传输，副本计入 Worker 队列预算，调用方输入不被
转移。同步诊断入口的第二参数直接为图片字节；Worker 第二参数为 `{asset,signal}`。
所有修改都用新快照，失败后也重新读取；清理临时输入前保留结构化错误分类。

页面操作为 `rotatePage(page,degrees)`、`copyPage(page,insertion,namePrefix)`、
`deletePage(page)`、`reorderPages(order)` 对应的类型化 apply 请求；均携带 stamp。
复制位置使用修改前的页序号，字段冲突可显式提供名称前缀；删除默认拒绝被导航
引用的页，Web 不删除最后一页。重排必须是完整排列。复制的共享内容由后续来源
编辑隔离，页面关系、权限和签名保护沿用原生规则。跨文档导入见下一节，新字体子集写入见下节，完整字体服务仍待补齐。

本轮 Worker 协议升为 2，拒绝旧协议消息；部署须将 TS 客户端、Worker 和 Wasm
作为同一构建交付。

## 跨文档页面导入

来源必须先在同一引擎/Worker 打开。目标与来源都携带当前 state stamp，页码和插入
位置从 0 开始；非空页列表最多 10,000 项，顺序按请求保留，不允许重复。

```ts
const source = await engine.open(sourceBytes);
try {
  const targetState = await target.state();
  const sourceState = await source.state();
  await target.apply({operation: 'importPages', stamp: targetState.stamp,
    source: sourceState.stamp, pages: [1, 0], insertion: 0,
    namePrefix: 'external', includeNavigation: true});
} finally {
  await source.close();
}
```

导入不修改来源，来源关闭后目标资源仍有效。字段/命名目标冲突使用显式前缀，
默认不迁移来源目录书签；显式迁移导航时必须包含全部所需目标页，页内链接同样
不得留下悬空引用。冲突、越界或关系不支持会整体回滚，旧目标 stamp 失效后重新
查询。源或目标失效、源版本过期、跨模块来源均拒绝。

当前沿用原生导入白名单：加密来源、较新 PDF 版本来源及不支持的签名/关系组合
明确拒绝，能够打开不代表能够导入；不自动解密或放宽保护。目标需修改/组装权限，
来源复制权限不绕过；加密目标可导入受支持的明文来源，保存保留目标密码与权限。
阅读器通过独立文件/密码输入和导入按钮操作，迟到的来源读取不会改变新目标。


## 新字形写入与显式文字跨度

`replaceTextSpan` 显式允许跨绘制事件，索引仍来自当前 content 快照。跨越隐藏
图层文字会拒绝，避免扩大删除范围。普通 `replaceText` 保持单事件限制。

两个入口均可携带 `font`：`postscriptName`、`capHeight`、`stemV`、`serif`，以及
显式选择的 `omitLayoutAndAuxiliary`。第二参数传入原始静态 TrueType 字体字节
（Worker 使用 `{asset: bytes}`），最多 32 MiB，独立传输并计入队列预算。
必须提供具有合法嵌入权限且与原字体字形、度量匹配的源字体；任意替代字体不能
用作原字体的写入来源。fsType 禁止子集、错误来源或缺字会回滚。每次写入自动
生成文档内不冲突的六字母子集标签，共享页面及未选调用不随之修改。

阅读器提供原字体文件和字体描述参数输入，提交后立即锁定保存及编辑控件；切换
文件后迟到的字体读取不会修改新文档。显示用字体回退不等同于此写入入口。

## 显式多字体注册

同步与 Worker 引擎均提供 `registerFont(bytes, metadata)`，返回源字节 SHA-256
及集合索引。metadata 包含 family、weight、italic、serif，以及可选 aliases、
languages、index。同身份同元数据幂等；冲突或错误字体失败后原注册仍有效。

```ts
await engine.registerFont(cjkBytes, {family: 'Noto Sans CJK', aliases: ['MissingFace'],
  weight: 400, italic: false, serif: false, languages: ['zh-Hans'], index: 0});
// Worker 可在第三参数传入 {signal}。
await engine.clearFonts();
```

字体仅用于显示，不修改原文档或授权字体嵌入。候选必须实际覆盖映射出的字符，
支持同段不同 face，保持 PDF 字宽/推进。旧 `setSubstituteFont` 是最后的显式
回退；`clearFonts` 清除多字体注册，旧单字体用 `setSubstituteFont()` 单独清除。
字体随 dispose/Worker 失效释放。自动请求、下载与离线缓存仍在后续阶段。

## 字体预检和迟到注册保护

`doc.fontRequirements(page, renderOptions)` 返回带 stamp 的字体需求。默认选择与
render 一致（默认 View、批注及 Widget），并检查实际使用的 Form、图案、软蒙版
和 Type3 内层图形。fontId 只属于该 stamp/page/选择；不要按 BaseFont 合并资源。

```ts
const report = await doc.fontRequirements(0);
for (const font of report.fonts) {
  // missingCodepoints 是可证实映射但尚无覆盖的 Unicode 标量。
  // missingUnicode / needsShaping 无法靠猜测代码或任意字体解决。
}
await doc.registerFont(fontBytes, configuredMetadata, report.stamp);
const updated = await doc.fontRequirements(0);
```

注册在原生调用内检查文档仍存活且 revision 相同，跨文档、跨引擎和过期请求
拒绝。Worker 预检第三参数、文档注册第四参数可传 `{signal}`；复制输入和排队
预算沿用引擎注册。注册不改变 PDF revision。异步回调完成后应使用文档级入口，
避免只在下载前检查状态。

无复制权限时 `codepointsRedacted` 为 true，codepoints 和 missingCodepoints 均为空，保留需求
数量与字体元数据用于配置完整字体。selected 返回已选字体哈希/集合身份或 legacy，
count 统计被预检的绘制项；图案重复铺画不重复累计。complete 仅表示当前字体
映射和覆盖满足，仍需实际 render 验证其他绘制能力。资源加载和缓存见下节。

预检 stamp 的 fontEpoch 是字体清除代次；保留原字段传给文档注册。clearFonts 或
成功设置/清空旧单字体会使旧请求失效，需重新预检。同一代次中的追加注册不会
互相作废。没有 fontEpoch 的普通文档 stamp 仍允许显式注册，但不用于异步加载。

## 按需字体、许可清单和离线缓存

`FontResourceService` 接收应用提供的版本 1 清单，最多 128 项。每项必填 id、资源
version、format、精确 sizeBytes、SHA-256、font 注册元数据和 license.spdx；可附
license.notice/url、Unicode ranges 提示及资源 url。许可字段由配置者声明，服务
校验结构与来源身份，不把显示字体注册当作 PDF 嵌入授权。清单中的大小和哈希
针对 Fetch 解码后的程序，支持 HTTP gzip 等传输压缩。

```ts
import {FontResourceService} from './font-resources.js';
const fonts = new FontResourceService(manifest, {
  root: 'https://example.com/fonts/', persistent: true,
  onEvent: event => console.log(event.type, event.resourceId)
});
const requirements = await fonts.ensure(doc, 0, {viewZoom: 1}, {signal});
if (requirements.complete) await doc.render(0);
// 保留持久缓存，取消本服务任务并释放其内存。
fonts.dispose();
```

root 必须是绝对 HTTP(S) 地址并以斜线结尾，资源 URL 限于该根，重定向拒绝且
不发送凭据。也可显式提供 `load(resource, signal)` 回调返回字体字节；返回值会
复制并校验。PDF 名称只参与候选匹配，不组成 URL。未知映射/需要 shaping 的
项目保留诊断，不通过网络猜字。CMap 继续使用核心现有语义资源，不混入字形缓存。

`network:false` 仅用已验证缓存，不调用网络或 load 回调；`persistent:false` 仅保留
内存缓存。持久缓存不可用/超额会发出 cache-error，已注册字体仍可显示；只有实际
命中才能证明离线可用。cacheName 限于本服务命名空间，clearCache 取消任务并按
顺序删除下载缓存，cancel 取消当前任务并保留已验证内存，dispose 不隐式清除引擎
共享的原生字体。引擎字体仍通过 clearFonts/dispose 管理。

默认每次最多尝试 16 项（maxLoads 1～128）、16 个页面请求、2 个同时读取，资源
读取超时默认 15 秒（timeoutMs 100～60000）。memoryBytes/persistentBytes 分别
最多 64 MiB；读取暂存与内存缓存还受共享 64 MiB 保守额度约束，超限淘汰或拒绝。
事件可观察请求、命中来源、注册、取消、失败和缓存错误；requestId 0 的缓存错误
属于合并读取阶段。调用方 AbortSignal 取消正在执行的 Worker 原生任务仍可能
终止会话，沿用既有硬取消规则；仅取消下载可保留文档。

阅读器提供 JSON 清单（最多 1 MiB）、资源根、仅离线/保存离线缓存、应用/停用及
清除下载缓存入口，并展示许可标识。配置前先校验，成功应用时清除旧原生多字体；
新文件也清除原生多字体，按需使用已验证下载缓存。取消下载后可重试；切换文件
和离开页面取消旧任务，恢复页面后重新应用保留的配置表单。该平台行为当前仅
有 Chromium 与合成 pagehide/pageshow 证据，真实 BFCache/其他浏览器仍待验收。

字体需求按实际字体和 language 分组；codepoints 包含可证实的单 Unicode 标量，
missingCodepoints 是其中尚无覆盖的子集。selected.preference 是名称、语言、斜体、
衬线和字重的排名元组。FontResourceService.ensure 在覆盖满足后仍会有界尝试
配置中更优的语言候选；不改变 PDF 字符位置，也不执行 shaping。

## 有界像素区域输出

当前接口仍为 experimental。已冻结的 29 输入／170 场景区域基础
矩阵通过；完整平台、真实语料及最终 API 门禁尚未完成。

`doc.renderRegion(page, {x,y,width,height}, dpi=72, options={})` 返回独立
`{x,y,width,height,pixels}`。坐标为已有页面旋转/CropBox/UserUnit 后、该 DPI 的
左上角整数像素；区域必须完全在页面内。单边最多 1024，逻辑页面单边最多
32768，DPI 仍为 1～288。`rasterSize(info.width, info.height, dpi)` 可从
`raster.js` 导入，按原生 f32 缩放/f64 取整计算完整像素尺寸，不分配整页。

```ts
const info = await doc.pageInfo(0);
const tile = await doc.renderRegion(0, {x:0, y:0, width:256, height:256}, 144);
context.putImageData(new ImageData(tile.pixels, tile.width, tile.height), tile.x, tile.y);
```

示例要求页面至少有 256×256 个像素，末块需使用实际剩余宽/高。Worker 版本
可在 options 中传 `signal`，排队参数已复制；取消/失效规则与整页 render 相同。
绘制选项包括批注/Widget 和 viewZoom。失败不返回半块图像，RGBA 输入不跨调用
悬空。每块解释完整页面并受既有预算约束，原图解码或复杂图案仍可能超限。

内部两像素留白与离屏分配计入原有 aggregate 限额，输出只包含请求区域。
正式目标保持整数倍率零差异、非整数倍率单通道最多 1 级；平坦区域和 alpha
一致，全部记录统计。曲线全局细分有小于 1/32 像素的几何误差证明和工作额度，
固定全局覆盖窗口已覆盖曲线、多边形与倾斜矩形的填充／裁剪，
具体冻结源码与通过范围见 acceptance；不以旧矩阵通过替代当前源码验收。
阅读器已接入 fit、额外视图旋转和有界可见分块，仍需完成后续平台／
真实语料验证。每块仍解释完整页面，不能理解成只解析可见对象。

### 额外视图旋转参数

RenderOptions.viewRotation 可取 0/90/180/270，默认 0。它描述调用方
随后施加的顺时针视图旋转，只用于 NoRotate 批注外观及字体预检补偿；
render/renderRegion 的 RGBA、页信息及 region 坐标仍处于原 display
坐标系。调用方需将画面和交互层一并变换。Worker 同样接受该参数。
C ABI flags 的位 2～3 编码角度除以 90，低两位保持原语义，其他位拒绝。

### 阅读器可见分块

阅读器按视口请求 512 像素块并复用有限 LRU，显示画布只覆盖当前
窗口及有界预取。滚动会替换待派发需求，普通滚动不终止 Worker。
RGBA 缓存上限 16 MiB／128 项、显示画布上限 32 MiB，单个在途块最多
1 MiB；这些是独立口径，不是全浏览器内存上限。可见窗口超限
明确失败，不降低所请求 DPI。resize、DPR、视图角度、文档修改
和字体注册影响相应来源；关闭／更换文档及 pagehide 清理缓存。


### PDF 输入大小

PDF 打开和页面导入不再设置独立的 8 MiB 文件大小上限。`capabilities.maxInputBytes` 为 `null`，`maxBufferBytes` 为 64 MiB：输入传输、打开时的源码副本、其他活跃文档、输出和字体共享缓冲预算，因此 64 MiB 不是对单个 PDF 可打开大小的保证。超出剩余预算返回 `resource_limit`，不启动不可满足的复制。保存默认 `maxBytes` 同步调整到 64 MiB，调用方仍可指定更低值。

此变更不调整字体程序 32 MiB、替换图片 8 MiB、内容元数据 8 MiB、解码流 16 MiB、绘制像素与线性内存等独立资源预算。
