# 解析回归与异常输入验证

## 代码质量门禁

从仓库根目录运行，使用锁定依赖；检查范围包含三个 crate、单元测试、集成测试和示例：

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
git diff --check
```

清理时优先删除无用导入／私有空壳，简化转换、条件和重复查找，用命名类型说明复杂返回值；只供单元测试使用的内部参考入口放在 `cfg(test)`，测试模块放在生产实现之后。公共记录、预算所有者与返回值存活期不可为了消除警告而改写语义。反向范围等故意非法输入用明确的 `Range { start, end }` 构造，不改成合法迭代器，不删除原有负例或放宽断言。

禁止 crate／workspace 级别一揽子 `allow(warnings)`。现有宽参数签名、按控制点索引表达的几何公式、公开枚举的内联布局、带线程亲和资源的共享快照，以及待 API 审查的少量不透明记录，可以使用局部 `#[expect(..., reason = "...")]`：必须说明保留原因，后续条件消失时让 unfulfilled expectation 暴露出来。新增代码不应照抄这些例外；参数确实属于一个概念时应优先用命名结构体，而不是隐藏身份／权限／预算。

编译器／Clippy 的项目诊断与用户环境或依赖的原生链接警告分开登记。`~/.cargo/config` 的弃用提示和 mozjpeg 汇编对象的 macOS load-command 警告不通过全局屏蔽、修改用户配置或更换系统库来隐藏；它们不构成项目 Rust 源码零警告的反例，也不能因此宣传构建过程完全没有任何 warning。清理不自动升级 MSRV、依赖、公开 API 或 PDF 支持范围。

### 2026 年 10 月 19 日维护验证

基线 `613044b`，使用本地 Rust 1.99.1、Clippy 0.1.99、macOS x86_64。全仓格式检查、严格全目标 Clippy 和 `git diff --check` 均退出 0；严格 Clippy 的 JSON 中无 compiler-message。Cargo 包版本、锁文件及原生依赖未改，原先的源码差距审计文档保留。本轮不扩大 MSRV 或平台承诺。

整理包括统一 Rust 格式、移到文件尾部的测试模块、命名复杂返回类型、补 `Default`／`is_empty`、简化转换／条件／查表和明确所有权释放。删除未使用的私有 CCITT `group4.rs`／`fax_table.rs` 及私有空壳／包装函数；实际 CCITT 仍走原 fax 后端，测试参考辅助入口仅在测试构建中启用。两个已删除文件可从基线 Git 提交恢复，未删除字体、CMap、fixture、负例或既有测试函数。

最终测试覆盖完整工作区的 90 个测试程序及三个 crate 的文档测试：先 `cargo test --workspace --locked --no-run --message-format=json` 构建，再按 Cargo metadata 中各 package 的工作目录，以最多四个进程执行所有测试程序，命令不加过滤参数；文档执行 `cargo test --workspace --locked --doc`。共 93 组／1633 passed，0 failed／ignored／filtered；PDF 单元 776、font-data 1、render 57、集成 793、文档 6。这里不是宣称另行执行了单条 `cargo test --workspace --locked`，而是登记同一完整目标清单的并发执行证据。

证据目录为 `/tmp/rzpdf-quality.2JgRv2/`：`clippy-receipt.json`、`format-receipt.json`、`diff-receipt.json` 及 `final-build-receipt.json`；最终测试仅取 `correct-cwd/` 下的各目标日志／receipt 和 `summary.json`，runner 已收取退出 0。早期调度误用工作区根目录，PDF 单元出现 50 项 fixture 路径失败，已终止该轮（143）并保留原始日志；修正 cwd 后，未改源码重新执行全部目标，776 项 PDF 单元及整轮均通过，不沿用错误调度轮的通过计数。

652 项非 Markdown 输入重新枚举前后一致（较 v0.14 减少两个未使用的私有源码文件），90 个测试程序前后 hash 一致。`correct-cwd/sources-before.json` SHA-256 为 `fc58ced87de83b4a372d5b97968c611df4adf2ae558011019169d840976d6157`，`test-binaries.json` 为 `489d68355f42802327da6e0688908436d553770f0cd9d1e8b54ef731d95d79f0`，`summary.json` 为 `6d2d73842ee760ef96c5311bdc45d3c61f6dfb2928319adfadb12ce677677df1`。原有测试身份清单未减少；没有新增通过数、重跑十二组外部发布门禁、性能基线或目视页数，维护结果不替代原始版本验收。

局部保留 51 处带原因的 `expect`：25 个宽参数签名、13 个控制点索引公式／参考检查、3 个公开大枚举、3 个线程亲和共享快照、6 个旧不透明记录字段及 1 个公开模块路径。它们是明确的兼容与计费约束，不是全局压制告警；稳定 API 审查时再决定是否合并参数或废弃旧记录。

## v0.14 关联文件整版门禁

V14-00～09／三个阶段已完成限定验收：完整工作区 93 组／1633 项、十五目标联合 189 项、十二组独立门禁、162 页目视及 30 条性能／预算基线通过；654 项非 Markdown 输入和十四驱动一致。见[验收记录](releases/v0.14-acceptance.md)、[边界](releases/v0.14-boundaries.md)和[指南](associated-files.md)。本版仅 macOS x86_64 验证。

```sh
cargo test -p pdf --locked --test v014_associated_files
cargo build -p pdf --locked --example v014_associated_files --example v014_benchmark \
  --example v013_layers --example v012_layout --example v012_security_layout \
  --example v012_incremental --example v012_preservation --example v011_sign_verify \
  --example v011_field_source --example v011_encrypted_incremental \
  --example inspect_optional_content --example v012_recovery \
  -p render --example render_optional_content --example render_annotations
python3 docs/check-v014-release.py NEW_RELEASE_DIRECTORY
```

runner 的完整命令、日志、退出码及输入／驱动 hash 保存在新目录内；使用已有 Python 验证环境，不自动安装。其内部运行未过滤的 `cargo test --workspace --locked`、联合、基线及十二组独立门禁。AF 门禁从零使用 pikepdf 和 MuPDF 原始生产者，不以重写输入充当第二来源；结构／文字／payload 与 MuPDF、Skia 同后端像素均需核对，162 页逐页目视另行完成。

最终运行目录 `tmp/pdfs/v014-final1/`。完整工作区约 46 分 48 秒（包含编译／启动），长测未 ignore。曾对 68 个新集成 harness 只运行 `--list` 观测启动；不计通过、不替代正式测试、不修改系统安全设置，细节见验收记录。大批量开销见[性能记录](releases/v0.14-performance.md)；不沿用其他版本通过数证明本版。

## v0.13 图层编写门禁

2026-10-19，V13-00～08／阶段 A、B、C 冻结范围已收口：最终 `v013-final3-` 完整工作区 92 组／1609 项、联合 23 组／273 项及十一组必选独立门禁退出 0，0 failed／ignored／filtered。间接配置补充审计、432 页目视复核和 9 条固定规模基线完成，645 项源清单／重新枚举列表、十三个驱动及 1676 PDF／PNG／JSON 前后核对一致。仅 macOS x86_64 已验证；未提交／推送／改包版本或发布。

本版接口／限额见[冻结边界](releases/v0.13-boundaries.md)，真实结束结果见[验收记录](releases/v0.13-acceptance.md)。先运行新增定向，再按冻结源码完成完整回归；不能把下方 v0.12 的数字视为本版已通过。

```sh
cargo test --locked -p pdf --test v013_optional_content_edit
cargo build --locked -p pdf --example v013_layers --example v013_benchmark \
  --example inspect_optional_content -p render --example render_optional_content
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python \
  python3 docs/check-v013-layers.py NEW_LAYERS_DIRECTORY
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python \
  python3 docs/check-v010-optional-content.py NEW_OLD_LAYERS_DIRECTORY
/usr/bin/time -l target/debug/examples/v013_benchmark > benchmark.jsonl 2> benchmark-rss.log
cargo test --workspace --locked
```

新增门禁使用独立 pikepdf 与 MuPDF create 输入，三种全量布局、两种追加及 R6，共三种可见性配置和两页共享 Form 对照。qpdf／pikepdf 检查配置及对象共享；Skia 对照 MuPDF 期望文档，MuPDF 还直接绘制本库输出默认状态。所有输出新建目录，固定几何逐像素一致，跨后端颜色仅允许旧门禁已有的 1 级量化误差；还要目视复核，不只检查文本。旧结构／迁移、内容编辑、v0.11 安全和 v0.12 文件保存门禁仍需复验，命令见对应下节。

基线固定 1／100／1000 组及三种保存布局，JSON 分列创建、状态／内容编辑、保存耗时、文件大小和编辑预算峰值；进程 RSS 另列。保守 reservation 不冒充实际内存。无新依赖／远端 CI／Poppler／fuzz，其他平台仅按实测登记。

间接配置／旧别名的补充审计可在现有 Python 环境按下述配方重建；从仓库根运行，替换新目录名称。正例的 24 份输出再按主门禁的 qpdf 检查和 default 期望页面进行 MuPDF 对照；输出是相同的几何配方。此配方记录在文档中，不是新增的 Rust 测试目标。

```python
from pathlib import Path
import subprocess
import pikepdf as q

out = Path("NEW_INDIRECT_DIRECTORY")
out.mkdir(exist_ok=False)
for variant in ["indirect", "ui-rule", "design-default"]:
    source = out / (variant + ".pdf")
    with q.Pdf.new() as p:
        for _ in range(2):
            page = p.add_blank_page(page_size=(200, 200))
            page.Contents = p.make_stream(b"q 0 g 10 10 20 20 re f Q\n")
        d = p.make_indirect(q.Dictionary(Name=q.String("Legacy"),
            Creator=q.String("External producer"), BaseState=q.Name.ON,
            Intent=q.Name.View))
        if variant == "ui-rule":
            d.RBGroups = q.Array([])
        if variant == "design-default":
            d.Intent = q.Name.Design
        properties = p.make_indirect(q.Dictionary(
            OCGs=p.make_indirect(q.Array([])), D=d,
            Configs=p.make_indirect(q.Array([])), Private=q.String("opaque")))
        p.Root.OCProperties = properties
        p.Root.OldProperties = properties
        p.Root.OldDefault = d
        p.save(source, force_version="2.0")
    destination = out / (variant + "-authored")
    result = subprocess.run(["target/debug/examples/v013_layers",
                             str(source), str(destination)])
    assert (result.returncode == 0) == (variant == "indirect")
    if variant != "indirect":
        assert not destination.exists()
        continue
    assert len(list(destination.glob("*.pdf"))) == 24
    for saved in destination.glob("*.pdf"):
        with q.open(saved, password="user" if saved.stem.endswith("-r6") else "") as p:
            assert len(p.Root.OldProperties.OCGs) == 0
            assert len(p.Root.OldProperties.Configs) == 0
            assert str(p.Root.OldDefault.Name) == "Legacy"
            assert str(p.Root.OCProperties.D.Name) == "Default"
            assert str(p.Root.OCProperties.D.Creator) == "External producer"
            assert str(p.Root.OCProperties.Private) == "opaque"
```

## v0.12 整版冻结门禁

2026-10-19，最终 `v012-final2-` 冻结源码完整工作区 91 组、1591 passed，0 failed／ignored／filtered；18 组联合复验 217 项、九组必选独立门禁、422 组精确像素和 437 页目视复核通过。50 条固定规模预算／性能记录完成，640 项输入及十个二进制前后一致，1101 项产物 hash 核对通过。V12-00～13／阶段 A、B、C 全部按限定范围收口，详细数字、候选问题与真实退出状态见[验收记录](releases/v0.12-acceptance.md)。未提交、推送、改包版本或发布。

完整工作区不加过滤、不跳过已知嵌套字体长用例：

```sh
cargo test --workspace --locked
cargo test --locked -p pdf \
  --test v012_recovery --test v012_preservation --test v09_page_relationships \
  --test v010_structure_transfer --test v010_optional_content \
  --test v012_incremental_stream --test v012_layout_security \
  --test v012_object_streams --test v012_xref_output --test stream_structure \
  --test v05_incremental --test v05_security --test v05_signatures \
  --test v011_encrypted_incremental --test v011_controlled_updates --test v011_multisign \
  --test v011_signature_fields --test v011_signature_history
```

外部门禁先构建示例。目录必须不存在，不自动安装依赖；全部凭证和密钥均为公开测试值／临时合成，无网络信任取证。

```sh
cargo build --locked -p pdf \
  --example v012_layout --example v012_security_layout --example v012_incremental \
  --example v012_preservation --example v012_benchmark \
  --example v011_sign_verify --example v011_field_source --example v011_encrypted_incremental \
  -p render --example v012_recovery --example render_annotations
export PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python
python3 docs/check-v012-object-streams.py NEW_OBJECT_STREAMS_DIRECTORY
python3 docs/check-v012-layout-security.py NEW_SECURITY_LAYOUT_DIRECTORY
python3 docs/check-v012-incremental.py NEW_INCREMENTAL_DIRECTORY
python3 docs/check-v012-preservation.py NEW_PRESERVATION_DIRECTORY
python3 docs/check-v012-recovery.py NEW_RECOVERY_DIRECTORY
python3 docs/check-v011-workflows.py NEW_WORKFLOWS_DIRECTORY
python3 docs/check-v011-multisign.py NEW_MULTISIGN_DIRECTORY
python3 docs/check-v011-signatures.py NEW_SIGNATURES_DIRECTORY
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/check-v011-encrypted-incremental.py NEW_ENCRYPTION_DIRECTORY
/usr/bin/time -l target/debug/examples/v012_benchmark > benchmark.jsonl 2> benchmark-rss.log
```

ObjStm 门禁内部还执行 xref-output 门禁，不重复相加其案例。恢复门禁不写修复 PDF，比较分离的 Skia 绘制与无损来源并保留 MuPDF 参考。全量／增量保留与 R6 验证需检查对象／来源投影及最新渲染页，签名工作流还须跨引擎文字范围与八组精确像素；不能只看首个 PASS 或沿用旧截图。[最终验收记录](releases/v0.12-acceptance.md)登记各任务真实终态、源码／二进制清单和平台，不把定向数量加到完整回归中冒称更多独立测试。

标准映射见[覆盖索引](pdf-standard-coverage.md)，恢复 API／拒绝与预算见[指南](file-recovery.md)，固定规模和 RSS 实测见[性能记录](releases/v0.12-performance.md)。不运行 fuzz 或安装 Poppler，平台不具备时明确记未验证。下方各子阶段记录保留当时口径，不代表本轮整版运行状态。

## v0.12 对象与关系保留 V12-06

同一冻结源码 21 组、288 项不重复定向测试通过：writer 73、安全 6（排除 writer）、读取 7、资源导入 8、页面复制 3、页面顺序 2、15 组联合 189。新增 preservation 12 项已包含在联合中。独立门禁 62 正例／120 组源页像素对照／7 页全白检查通过，127 张输出页的 8 张联系表已目视复核。634 项非 Markdown 输入内容和清单前后相同；日志统一前缀 `tmp/pdfs/v012-06-final-`。详见[验收记录](releases/v0.12-acceptance.md#v12-06-对象保留与页面关系)。

```sh
cargo build --locked -p pdf --example v012_preservation --example v012_layout
cargo test --locked -p pdf --lib writer::
cargo test --locked -p pdf --lib security:: -- --skip writer::
cargo test --locked -p pdf --lib objects::object_streams::
cargo test --locked -p pdf --lib document::resource_import::
cargo test --locked -p pdf --lib document::page_copy::
cargo test --locked -p pdf --lib document::page_order::
cargo test --locked -p pdf \
  --test v012_preservation --test v09_page_relationships \
  --test v010_structure_transfer --test v010_optional_content \
  --test v012_incremental_stream --test v012_layout_security \
  --test v012_object_streams --test v012_xref_output --test stream_structure \
  --test v05_incremental --test v05_security --test v05_signatures \
  --test v011_encrypted_incremental --test v011_controlled_updates --test v011_multisign
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-preservation.py NEW_PRESERVATION_DIRECTORY
```

外部依赖须已存在，不自动安装；目录必须新建。脚本自建合成输入和公开凭证，检查三种全量／两种增量、R6、页面操作、业务图投影、源前缀及 MuPDF 像素，另查 Info／私有 trailer 反向引用、未选中页归属、结构容器与失败重试。未知旧 XRef 字段重打包仍拒绝，不因通过样本就放宽保留边界。qpdf／pikepdf 共用后端，MuPDF 独立绘制；不运行附件／脚本、fuzz 或 Poppler。

关闭 V12-06，不关闭 M3／阶段 B 或整版。下方 V12-05 的三个独立门禁是历史结果，本轮未重跑；完整工作区、四组 v0.11 独立安全门禁和其他平台仍待 V12-12。

## v0.12 unsigned 流式增量 V12-05 历史验收

最终同一冻结源码 197 项不重复定向测试通过：writer 73、安全 6（排除 writer）、读取 7、联合 11 组 111。增量 66／132、全量 R6 48／92、M1 16／32 + 10／20 门禁通过，276 页目视复核与 630 项输入前后核对一致；日志统一前缀 `tmp/pdfs/v012-05-final2-`。关闭 V12-05／M2／阶段 A，不关闭阶段 B／C 或整版。

专项复现命令如下。三个独立门禁必须使用新目录，并在 examples 构建完成后执行；不自动安装依赖。凭证全部为公开合成值，native 驱动从环境读取，不能拿本例日志处理真实凭证。

```sh
cargo build --locked -p pdf --example v012_layout --example v012_security_layout --example v012_incremental
cargo test --locked -p pdf --lib writer::
cargo test --locked -p pdf --lib security:: -- --skip writer::
cargo test --locked -p pdf --lib objects::object_streams::
cargo test --locked -p pdf \
  --test v012_incremental_stream --test v012_layout_security \
  --test v012_object_streams --test v012_xref_output --test stream_structure \
  --test v05_incremental --test v05_security --test v05_signatures \
  --test v011_encrypted_incremental --test v011_controlled_updates --test v011_multisign
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-incremental.py NEW_INCREMENTAL_DIRECTORY
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-layout-security.py NEW_R6_DIRECTORY
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-object-streams.py NEW_M1_DIRECTORY
```

增量脚本为 11 种未加密／R6 的传统、流、ObjStm、hybrid 与直接 Encrypt 来源，分别跑两条三轮混合布局链及精确 no-op；每个真实追加对照两页 MuPDF 像素，另查 qpdf、原字节前缀、成员身份、无新 ObjStm、Prev／Index／self、保护身份和业务投影。Rust 专项覆盖压缩成员删除、hybrid 补充流覆盖、限额／RNG／host 故障及重试；此脚本不替代完整工作区或 v0.11 四组独立安全门禁。最终结果和源码清单见[验收记录](releases/v0.12-acceptance.md#v12-05-实现与验收范围)。fuzz 暂缓，不安装 Poppler。

## v0.12 全量 R6 新布局 V12-04 历史验收

最终冻结源码通过 writer 72、安全 6、对象流读取 7、联合 10 组 95 项，共 180 项不重复定向测试，0 failed/ignored。安全专项排除 writer 的三项重叠；各 lib 专项分别有 701／767／766 filtered。联合包含新增 `v012_layout_security` 12 项，不重复计入总数。

```sh
cargo build -p pdf --example v012_security_layout --example v012_layout --locked -j2
cargo test -p pdf --lib writer:: --locked -j2
cargo test -p pdf --lib security:: --locked -j2 -- --skip writer::
cargo test -p pdf --lib object_streams:: --locked -j2
cargo test -p pdf --locked -j2 \
  --test v012_layout_security --test v012_object_streams --test v012_xref_output \
  --test stream_structure --test v05_incremental --test v05_security --test v05_signatures \
  --test v011_encrypted_incremental --test v011_controlled_updates --test v011_multisign
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-layout-security.py NEW_R6_DIRECTORY
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-object-streams.py NEW_M1_DIRECTORY
```

R6 脚本的 48 个组合覆盖两种流布局的新设／保留／移除／替换、用户／所有者／错密码、外部传统／对象流、PDF 1.7 扩展／PDF 2.0、直接 Encrypt、明文元数据和不同默认 filter 的真实字符串。qpdf 不需结构重建，pikepdf 以独立读取句柄核对原始投影和语法，MuPDF 解密绘制 92 组全页像素完全一致。两个 Identity 结构样本为零页，不虚算像素；原 M1 另有 16／32 与 10／20 复验。全部 144 张最新输出页目视复核通过，R5／权限／目标已存在等负例明确拒绝。

证据 `tmp/pdfs/v012-04-final-{example,writer,security,reader,regression,independent,m1}.log`，626 项非 Markdown 输入前后指纹及重新枚举一致；命令、版本、来源和 hash 见[验收记录](releases/v0.12-acceptance.md#v12-04-全量新布局与-r6-验收)及各独立目录 manifest。仅关闭 V12-04，不关闭 M2／阶段 A；未重新运行完整工作区、四组 v0.11 独立安全门禁或跨平台验证。fuzz 继续暂缓，不安装 Poppler。

## v0.12 未加密文件结构输出 M1 历史验收

V12-03 最终冻结源码通过 writer 72、对象流读取 7、联合 8 组 75 项，共 154 项定向测试，0 failed/ignored。联合包含新增打包 16 项与原 xref 输出 11 项，不重复计数；两个 lib 专项分别有 700／765 filtered。

```sh
cargo test -p pdf --lib writer:: --locked -j2
cargo test -p pdf --lib object_streams:: --locked -j2
cargo test -p pdf --locked -j2 \
  --test v012_object_streams --test v012_xref_output --test stream_structure \
  --test v05_incremental --test v05_security --test v05_signatures \
  --test v011_controlled_updates --test v011_multisign
cargo build -p pdf --example v012_layout --locked -j2
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-object-streams.py NEW_DIRECTORY
```

最后的脚本也在 `input-gate/` 运行下方未修改的 V12-02 独立门禁。新增八类输入 × 两种设置共 16 次打包／32 组 MuPDF 全页精确像素，原门禁另 10 次／20 组；业务 ID／值、共享循环、元数据／附件、N／First／成员偏移和 type 2 索引核对通过，并覆盖至少三组大量小对象、旧容器重建及 Extends／未知字段／活跃引用拒绝。所有 52 张最新输出页均目视复核通过。

日志为 `tmp/pdfs/v012-03-final-{example,writer,reader,regression,independent}.log`，独立目录有 manifest 和视觉总览；623 项非 Markdown 输入指纹前后及重新枚举相同。此轮仅关闭 V12-03／M1，未运行完整工作区、四组 v0.11 独立安全门禁或跨平台验证，不代替 V12-12。详见[验收记录](releases/v0.12-acceptance.md#v12-03-实现与验收)。

### V12-00 至 V12-02 历史验收

V12-00～02 使用同一冻结源码执行以下专项，均退出 0：writer 70 项（700 filtered），联合 7 组 59 项（无 filtered），零失败／忽略。新增 `v012_xref_output` 的 11 项包含在 59 项内，不能重复计数。

```sh
cargo test -p pdf --lib writer:: --locked -j2
cargo test -p pdf --locked -j2 \
  --test v012_xref_output --test stream_structure \
  --test v05_incremental --test v05_security --test v05_signatures \
  --test v011_controlled_updates --test v011_multisign
cargo build -p pdf --example v012_layout --locked -j2
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-xref-output.py NEW_DIRECTORY
```

构建完成后再运行独立脚本，避免链接期间调用尚未生成的二进制。脚本复用已有工具，不安装依赖；十个重写案例覆盖五种输入和两种流压缩设置，独立检查全部原有对象投影、xref 自身及元数据／附件内容，MuPDF 二十组全页像素相同，另有线性化声明拒绝和示例目标不覆盖。最新二十页渲染结果目视复核通过。

日志为 `tmp/pdfs/v012-02-final-writer.log`、`v012-02-final-regression.log`、`v012-02-final-independent.log` 及同名独立目录。614 项冻结输入前后列表和内容一致，细节见[验收记录](releases/v0.12-acceptance.md)。此为子阶段验证，未跑完整工作区或四组 v0.11 独立安全门禁，不冒充整版通过；不运行 fuzz 或安装 Poppler。

## v0.11 字段、外观与受控修改

新增 `v011_signature_fields`（7 项）与 `v011_controlled_updates`（6 项），覆盖层级/多 Widget、Unicode/零面积 AP、保留 Matrix、继承字段、Widget 标志、签名和填值链、权限交集、令牌单次/外来/rollback/panic、累计预算、恰好不足及失败重试。仓库内合成 PDF/CMS 夹具不需运行时私钥或外部签名器。

联合回归命令：

```sh
cargo test -p pdf --locked -j2 \
  --test v011_controlled_updates --test v011_signature_fields \
  --test v011_multisign --test v011_signature_history --test v011_encrypted_incremental \
  --test v05_signatures --test v05_security --test v05_incremental \
  --test v09_form_types --test v09_forms --test v09_page_relationships \
  --test v010_optional_content --test v010_structure_transfer
```

最终冻结源码该命令退出 0，13 组 156 项，无失败/忽略/过滤，日志 `tmp/pdfs/v011-verified-regression.log`。简单字体专项另有 10 项通过，最终完整工作区仍须使用 `cargo test --workspace --locked -j2`，不得把上述定向结果冒充全量。

最终同一冻结源码的完整工作区命令已退出 0：85 组、1,501 passed，0 failed/ignored/filtered，日志 `tmp/pdfs/v011-verified-workspace.log`。包括 769 项 PDF 单元和 57 项 render 单元，嵌套字体子集长用例正常通过，未跳过。609 项源码/配置/测试资产的前、运行中及后指纹与重新枚举列表一致。

先构建 PDF 的 `v011_sign_verify`、`v011_field_source`、`v011_encrypted_incremental` 和 render 的 `render_annotations`。构建结束后，使用既有环境运行四个独立脚本，每次指定新目录：

```sh
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v011-workflows.py NEW_WORKFLOW_DIRECTORY
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v011-multisign.py NEW_MULTISIGN_DIRECTORY
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v011-signatures.py NEW_HISTORY_DIRECTORY
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/check-v011-encrypted-incremental.py NEW_R6_DIRECTORY
```

最终四门禁分别退出 0，27/21/42/30 案例；工作流另有八组精确像素、原生产者 Courier 文字跨引擎范围及十个页面渲染结果目视复核。P=3 批注与缺省 P 的独立验证器限制、指纹及最终全量结果详见[验收记录](releases/v0.11-acceptance.md)，不安装工具、Poppler 或运行 fuzz。

## v0.11 多签追加历史检查点

专项为 `cargo test --locked -j2 -p pdf --test v011_multisign --test v011_signature_history --test v05_signatures --test v011_encrypted_incremental --test v05_incremental --test v05_security`，冻结源码运行退出 0，6 组 57 项、无失败/忽略/过滤，日志 `tmp/pdfs/v011-07-verified-regression.log`。新增 9 项多签测试使用仓库内公开合成 PDF/CMS 十六进制夹具，不依赖运行时外部签名器；来源和 decoded hash 见 `pdf/tests/fixtures/v011/README.md`。

先构建 `v011_sign_verify`，再执行 `PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v011-multisign.py <new-directory>`。21 个案例覆盖 classic/ObjStm、两类首签生产者、四签链、旧 CMS/ByteRange 保留及精确对象变更，以及六种 MDP/字段锁和三种归属歧义的拒绝；已有输出不得覆盖。与历史读取门禁不同，本门禁开启 pyHanko diff-policy，限定验证本入口生成的审批变更，不替代阶段 C 的任意输入授权判定。

该检查点 `cargo test --workspace --locked -j2` 退出 0：83 组、1,483 passed、0 failed/ignored/filtered，日志 `tmp/pdfs/v011-07-verified-workspace.log`。471 个 Rust/Cargo/Python/hex 输入在前、运行中及后校验通过，重新枚举文件列表及内容一致。独立多签 21 案例、历史 CMS 42 案例及 R6 30 案例均退出 0。此为 V11-07 的历史范围，不代替上方新增工作流的最终源码验证。

## v0.11 历史签名与公开验签

V11-04～06 的集成专项为 `cargo test --locked -j2 -p pdf --test v011_signature_history --test v05_signatures --test v011_encrypted_incremental --test v05_incremental --test v05_security`，最终退出 0，5 组 48 项通过、0 failed/ignored/filtered。历史专项 9 项及 PSS 参数单元均包含在全量中。单独执行参数单元使用 `cargo test --locked -j2 -p pdf --lib signatures::algorithms::tests`，只算专项；勿省略 `--lib` 后将过滤后的逐集成启动算全量。

先构建 `v011_sign_verify`，再运行 `PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v011-signatures.py <new-directory>`。现有 Python 3.14 环境提供 pyHanko 0.37.0；不要与仅含加密工具的 Python 3.11 目录混用。候选 42 案例覆盖 OpenSSL/pyHanko 两类生产者、RSA/PSS/ECDSA、层级字段与三签链、签名字典替换/删除及摘要、证书、算法、DER、填充、版本负例。此门禁明确关闭独立 diff-policy，只验证 CMS/历史，不作签后修改授权结论。

该阶段冻结源码完整工作区 `cargo test --workspace --locked -j2` 退出 0：82 组、1,474 passed、0 failed/ignored/filtered，日志 `tmp/pdfs/v011-b-verified-workspace.log`。联合、示例构建、42 签名案例、30 R6 案例及两份旧 ObjStm 样本复验均退出 0；466 个输入在前后内容指纹和文件列表一致，见[验收记录](releases/v0.11-acceptance.md)。前两轮 `v011-b-read-workspace.log`、`v011-b-read-final-workspace.log` 分别因字段集合预算和 Contents 别名补强退出 130，不计全量。新指南见[离线验签](signature-verification.md)。当时尚未实现多签写入；当前 V11-07 的结果见上节，不以历史多签读取替代多签保存验证。

## v0.11 R6 加密增量验证

专项命令为 `cargo test --locked -j2 -p pdf --test v011_encrypted_incremental --test v05_incremental --test v05_security --test v05_signatures`，已退出 0，4 组 39 项通过；3 项内部 Encrypt/ID 保护另入完整工作区，不把过滤其他单元的专项算全量。独立入口为 `PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/check-v011-encrypted-incremental.py <new-directory>`，先构建 `v011_encrypted_incremental` 示例。最终独立验证退出 0，30 个追加案例及拒绝负例通过。使用现有 qpdf/MuPDF/pikepdf，生成命令、版本、hash 和认证/轮次在输出目录登记；没有生产密码或密钥，不安装依赖/Poppler，不运行 fuzz。

完整工作区 `cargo test --workspace --locked -j2` 已退出 0：81 组、1,464 passed、0 failed/ignored/filtered，日志 `tmp/pdfs/v011-a-workspace.log`；461 个 Rust/Cargo/Python 输入在回归前后指纹和文件列表一致。冻结输入及真实退出状态见[阶段记录](releases/v0.11-acceptance.md)。A 的验收不覆盖尚未实现的历史验签/多签/签后修改。只改加密保存、未改绘制算法，独立门禁核对页面内容字节，不将其描述为新像素验收。

## v0.10 可选内容与整版验证

专项为 `cargo test --locked -j2 -p pdf --test v010_optional_content`。最终联合回归还包括 v010 结构/标记专题、`v03_marked_content`、v09 批注/字段类型/字段关系/导航/页面关系及 `v05_security`，不以专项代替完整工作区。

独立图层入口为 `PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/check-v010-optional-content.py <new-directory>`。先构建 PDF 的 `inspect_optional_content` 和 render 的 `render_optional_content` 示例，再执行；使用既有 pikepdf、qpdf、MuPDF、Pillow，不安装 Poppler、不运行 fuzz。独立样本检查严格保存重开、源内容保留、四份语义反例、显式逻辑文字，以及默认/替代/打印三组 Skia 与独立 MuPDF 预期的像素对照。几何要求逐像素一致，颜色允许至多一个 RGB 取整级别差；对照图需目视复核。

整版命令 `cargo test --workspace --locked -j2` 已退出 0：80 组、1,453 passed、0 failed/ignored/filtered，日志 `tmp/pdfs/v010-final-workspace-verified.log`。联合回归 14 组、206 项及八个示例构建退出 0；七组独立门禁通过，三配置最新对照图已目视检查。457 个 Rust/Cargo/Python 输入在回归前后指纹及文件列表一致；源码清单、具体命令和退出状态见[验收记录](releases/v0.10-acceptance.md)。首次最终候选因消除重复流复制而中断，不计作全量通过；此前阶段证据均为历史记录。

## v0.10 结构迁移与阶段 B 历史验证

新增 `v010_structure_transfer` 25 项。联合命令为 `cargo test --locked -j2 -p pdf --test v010_structure_transfer --test v010_structure_edit --test v010_structure --test v010_structure_associations --test v010_structure_semantics --test v010_marked_text --test v03_marked_content --test v09_page_relationships --test v09_navigation --test v05_security`，最终退出 0，10 组、148 passed、0 failed/ignored/filtered，日志 `tmp/pdfs/v010-stage-b-regression-verified.log`。

独立入口新增 `docs/check-v010-structure-transfer.py <new-directory>`，先构建 `transfer_structure` 及下方五个已有结构示例。既有 Python 3.11/pikepdf、qpdf、MuPDF 环境即可；六组独立门禁通过，迁移链严格重开/来源保留和四份反例均有结果。两页 144 dpi MuPDF 渲染与独立输入仅 A→Z 的预期逐像素一致，并完成目视核对；不把 ActualText 当作绘制字形。接口及范围见[迁移指南](structure-transfers.md)，证据见[验收记录](releases/v0.10-acceptance.md)。

阶段 B 最终全量 `cargo test --workspace --locked -j2` 已退出 0：79 组、1,426 passed、0 failed/ignored/filtered，日志 `tmp/pdfs/v010-stage-b-workspace-verified.log`。编译 7 分 18 秒，PDF 单元 97.49 秒，render 52.42 秒；419 个 Rust/Cargo/Python 文件前后指纹和列表一致。V10-05 与阶段 B 的限定范围已验收，不表示阶段 C 或整版完成。较早 `v010-stage-b-workspace.log` 因补充图层拒绝保护退出 130；`v010-stage-b-workspace-final.log` 因非法源输入拒绝晚于目标事务而退出 101，旧预检契约已修复并在最终全量通过。不将这两轮计作通过，也不把失败归因于长用例。不安装 Poppler、不运行 fuzz。

## v0.10 显式结构编辑

V10-04 专项为 `v010_structure_edit`，新增 22 项。最终联合命令为 `cargo test --locked -j2 -p pdf --test v010_structure_edit --test v010_structure --test v010_structure_associations --test v010_structure_semantics --test v010_marked_text --test v03_marked_content --test v09_page_relationships --test v09_navigation --test v05_security`，退出 0，9 组共 123 项通过，0 failed/ignored/filtered。日志为 `tmp/pdfs/v010-edit-regression-final.log`；`cargo check --workspace --all-targets --locked -j2` 也退出 0，但不是全量测试。

先构建 `edit_structure` 和下方四个读取示例，再用新的目录执行 `PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/check-v010-structure-edit.py <new-output-directory>`。独立写入及四组旧门禁复验均通过，覆盖引用/属性、来源保留、严格重开及 MuPDF 标记替代文字观察；不作像素/视觉或 PDF/UA 结论。接口见[编辑指南](structure-editing.md)，源码和证据见[验证记录](releases/v0.10-acceptance.md)。阶段 B 全量留到 V10-05 集成后，不以旧阶段 A 的数字冒充本次全量。

## v0.10 阶段 A 的历史验证

阶段 A 专项包含 `v010_structure`、`v010_structure_associations`、`v010_structure_semantics` 和 `v010_marked_text`，兼容回归保留 `v03_marked_content`、`v09_navigation`、`v09_page_relationships`。逻辑提取新增 26 项，当前专项共 92 项；阶段集成完整命令为 `cargo test --workspace --locked -j2`，需等待进程退出，不能将专项数字当作全量结果。

2026-10-16，阶段 A 最终源码完整工作区已退出 0：77 组、1,379 passed、0 failed/ignored/filtered，日志为 `tmp/pdfs/v010-stage-a-workspace.log`；编译 12 分 36 秒，PDF 单元 130.34 秒，render 单元 62.56 秒。长用例提示超过 60 秒后正常通过，未过滤或跳过；411 个 Rust/Cargo/Python 输入在全量前后指纹及文件列表一致。阶段 A 的限定范围已验收，不表示阶段 B/C 或整版完成。

独立脚本为 `docs/check-v010-structure.py`、`check-v010-associations.py`、`check-v010-semantics.py` 和 `check-v010-marked-text.py`，分别使用新的输出目录。先构建对应 `inspect_structure`、`validate_structure_associations`、`inspect_structure_semantics` 和 `extract_marked_text` 示例；使用既有 Python 3.11/pikepdf、qpdf、MuPDF，不安装依赖、Poppler 或运行 fuzz。最后一个门禁对照声明文本及 K 顺序，检查严格改写/来源保留，并观察 MuPDF 标记 ActualText；不等同于像素或 PDF/UA 验证。当前门禁结果和源码指纹见[v0.10 验证记录](releases/v0.10-acceptance.md)。

## v0.9 最终门禁

2026-10-16，最终源码完整工作区 73 组、1,312 passed、0 failed/ignored/filtered；导航、批注、字段层级、类型/Unicode 外观和页面关系五组独立门禁通过。最终 23 页主样本完成视觉或已目视基线的逐像素复核。源码指纹、原始日志、历史中断及精确限制见[v0.9 验收记录](releases/v0.9-acceptance.md)与[支持矩阵](releases/v0.9-support.md)。

完整回归命令：`cargo test --workspace --locked -j2 -- --test-threads=2`。本次编译 3 分 10 秒，PDF 单元 105.37 秒，render 58.60 秒；字体子集历史长用例超过 60 秒后正常结束，不能仅凭提示判断死锁。最终日志为 `tmp/pdfs/v09-release-workspace.log`，已确认进程退出 0，不以启动或局部测试通过替代整版结论。

独立脚本为 `docs/check-v09-navigation.py`、`check-v09-annotations.py`、`check-v09-forms.py`、`check-v09-form-types.py` 和 `check-v09-page-relationships.py`，参数均为新的输出目录。先构建对应 `v09_*` 示例及 `render_annotations`；使用已有 Python 3.11/pikepdf/Pillow、qpdf、MuPDF，不安装 Poppler、不运行 fuzz。页面关系的专项入口为 `cargo test --locked -j2 -p pdf --test v09_page_relationships`，共 14 项；详见[页面关系指南](page-relationships.md)。当前仅声明本地 macOS x86_64 验证，未提交、推送或发布。

## v0.8 最终门禁

2026-10-15，限定范围完整工作区 68 组、1,251 passed、0 failed/ignored/filtered，八组独立检查和 91 张主样本视觉复核通过。源码指纹、失败迁移记录、阅读器差异及最终证据见 [验收记录](releases/v0.8-acceptance.md)，边界见 [支持矩阵](releases/v0.8-support.md)。

完整入口：`PYTHONPATH=tmp/pdfs/v05-verifiers-py311 sh docs/check-v08.sh`。它先执行不带过滤的完整回归并重建示例，再运行 Pattern、Type6/7、组、蒙版、ICC/Lab/专色、文字裁剪、组合/16BM 和 Type1～5 复验。Python 需预先有 Pillow/LittleCMS、pikepdf，另需 qpdf 和 MuPDF；不安装工具，不使用 Poppler 或 fuzz。每次产生新的 PDF/PNG/报告目录，自动通过后仍应复核图片。

正向样本保留严格保存、注册/导入、局部资源和共享隔离；负向与邻值预算测试保留错误恢复。颜色容差、字形 AA、低分辨率 shading 端点和原始阅读器差异分别记录，不统一放宽阈值。全局 OperationBudget/RSS、任意 ICC/印刷组空间及所有字体裁剪仍不能从本版矩阵推导。

## v0.7 最终门禁

最终候选 `8a901e0` 的完整工作区 64 组、1,208 项通过，0 失败/忽略；八页字体/字形运行矩阵、两份公开文件 12 页、静态 H/V/子集与 Type3 独立门禁及视觉复核通过。具体命令、时间、工具版本和证据见 [v0.7 验收记录](releases/v0.7-acceptance.md)。新入口与预算边界见 [字形运行指南](glyph-runs.md)。不使用 Poppler，不运行 fuzz，不将明确拒绝组合计为通过。

## v0.7 Type3 历史执行检查点

历史源码 `47e2383` 的 23 项专项、独立 12 页绘制/保存检查及完整工作区 1,188 项回归通过，0 失败/忽略。准确命令、环境、容差、视觉复核和证据见 [A 阶段验收](releases/v0.7.md#type3-执行阶段验收)，独立脚本用法见 [Type3 指南](type3-fonts.md)。只代表 Tr=0、不透明图形字形固定矩阵；本版最终状态以上节为准。

## v0.7 Type3 模型历史检查点

源码 `9d0dc9b` 的 10 项模型专项、独立 pikepdf/MuPDF 保存检查及完整工作区 1,175 项回归通过，0 失败/忽略。命令、环境及原始证据见 [v0.7 模型切片记录](releases/v0.7.md#type3-模型切片验证记录)。此历史源码当时尚未接入 Type3 执行；当前执行范围以上节为准。

## v0.6 图像门禁

最终源码 `0f647af` 的完整工作区 61 组、1,165 项及独立图像/视觉门禁均已通过；准确命令、工具版本、耗时和限制见 [v0.6 验收记录](releases/v0.6-acceptance.md)。

普通专项不需要安装系统编解码器；二进制样本均已登记来源和 hash。

```bash
cargo test -p pdf --locked --test v06_inline --test v06_codecs -j 2 -- --test-threads=2
cargo build -p pdf --example v06_images -p render --example render_document --locked -j 2
PYTHONPATH=tmp/pdfs/v06-verifiers:tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v06-images.py
python3 docs/check-dependency-advisories.py
cargo test --workspace --locked -j 2 --timings -- --test-threads=2
```

独立检查需要 Pillow、PyMuPDF、pikepdf、qpdf 和 MuPDF；版本见验收记录，不用 Poppler。依赖告警查询只代表查询时已登记的精确版本结果。PyMuPDF/MuPDF 负责 JPX/JBIG2 像素，qpdf/pikepdf 负责结构；pikepdf 的缺专用 JBIG2 decoder 提示不得当成像素通过。Skia 与独立解码像素生成的最近邻整页基线精确比较，MuPDF 另验几何和内部采样点。模糊测试继续暂缓。

## v0.5 专项和独立门禁

当前完成状态以 [v0.5 验收记录](releases/v0.5-acceptance.md) 为准。
普通 Rust 回归不依赖 Poppler、qpdf 或 OpenSSL；独立互操作门禁需另行运行，
缺工具不算通过。没有开启 fuzz，也不需要修复本机 Poppler 的旧 libtiff 依赖。

```bash
cargo test -p pdf --locked --test v05_incremental --test v05_security --test v05_signatures
cargo build -p pdf --locked --example v05_incremental --example v05_security --example v05_sign
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v05-incremental.py
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v05-security.py
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v05-signatures.py
python3 docs/check-cmap-data.py
cargo test --workspace --locked -j 2 --timings
```

示例的独立环境是本地隔离的 pikepdf/libqpdf、pypdf 和 pyHanko；脚本会生成
新的 `tmp/pdfs/v05-*` 证据目录及 report.json，不覆盖旧证据。签名门禁只生成
合成测试证书/私钥；信任根显式指定为该本地测试证书，关闭网络证书抓取，
不能解释为生产证书可信。原始日志、退出码、固定源码提交和视觉复核需登记。

编译的 `-j 2` 与测试 `-- --test-threads=2` 是两种不同并发控制。
本机 macOS 曾在进程启动和过程宏动态库加载时长时间无输出；先看是否已出现
`running N tests`、CPU 和采样栈，不把尚未启动的测试当作通过或 PDF 死循环。
临时签名仅用于本地生成产物，未修改系统安全策略；不要求使用者禁用安全检查。

## 操作级对象表预算接入（2026-10-08，回归通过）

工作区最终验收：原session1967退出0，`cargo test --workspace --locked --offline --quiet` 共46组、938项通过、0失败、0忽略，日志 `/tmp/rzpdf-table-budget-full-workspace.log`。Rust源码在验收期间保持冻结；此结果取代下段的待确认状态，不表示完整AR-03已完成。

当前PDF全部单元633项通过，原session7874退出0，日志 `/tmp/rzpdf-table-budget-pdf-unit.log`；此前对象表4项、事务最新7项及表分配器2项专项均通过。验证实际Layout、扩容旧新共存、三入口拒绝不改原表、回滚对象号恢复、同池嵌套/普通内层继承、异池拒绝、外层unwind保留邻居额度。工作区全量已启动，日志 `/tmp/rzpdf-table-budget-full-workspace.log`，终态待确认，源码冻结。低层entries改只读视图，迁移说明见architecture；当前证据不代表完整AR-03、完整PDF兼容性或所有外部下游编译通过，未运行fuzz。

## Type6 部分接入检查点（2026-10-08）

当前提交53a9011的全量终态：原session39247退出0，`cargo test --workspace --locked --quiet` 共46组、933项通过、0失败、0忽略，日志 `/tmp/rzpdf-coons-retained-domain-full-workspace.log`。包括628项PDF单元、8项着色流程及38项render（53.40秒）；验收期间Rust源码保持冻结。本记录取代下方本轮运行中的状态，不证明一般曲面边界、任意折叠或完整Type6及整个TODO交付。未运行fuzz。

区间映射版render终态：原session3626退出0，38项通过、0失败/忽略，54.68秒，日志 `/tmp/rzpdf-coons-retained-domain-render.log`。当前源码对应模型87项、流程8项、render38项专项通过；全工作区复验已启动，原session39247、日志 `/tmp/rzpdf-coons-retained-domain-full-workspace.log`，Rust源码冻结至终态，最终结果待确认。此证据仍不代表任意边界/折叠及完整Type6交付，不沿用927项旧版全量结果。

区间映射版流程终态：原session73555退出0，8项通过，日志 `/tmp/rzpdf-coons-retained-domain-workflow.log`；本结果覆盖当前三根分割线修订的导入和两保存严格重开。render全量原session3626仍运行，日志 `/tmp/rzpdf-coons-retained-domain-render.log`，终态待确认，不沿用前版38项结果。

三根分割线模型修订终态：原session87069退出0，87项通过，日志 `/tmp/rzpdf-coons-retained-domain-mapping.log`；单轴遍历保留首轮较宽的全根存活域、递归携带区间仿射映射，新增v=0.6875三根覆盖与既有折叠排序回归通过。取代下方该样例失败状态，不代表任意折叠完整兼容。当前流程/render重测日志 `/tmp/rzpdf-coons-retained-domain-workflow.log`、`/tmp/rzpdf-coons-retained-domain-render.log`，终态待确认，全工作区仍未复验。

最新多根分割线回归失败：原session63291退出101，shading86项通过、1项失败，日志 `/tmp/rzpdf-coons-contraction-fold-model.log`。扩展折叠三根样例至v=0.6875后完整遍历仍Unresolved；保留失败回归，需让未唯一认证的收缩域继续参与多根遍历。此前render全量原session24620退出0、38项通过（55.14秒），日志 `/tmp/rzpdf-coons-contraction-render.log`；此证据仅覆盖既有渲染样例，不覆盖新三根失败或完整Type6交付。

格式整理与生产采样失败契约复验：原session7305退出0，shading87项通过，日志 `/tmp/rzpdf-coons-contraction-atomic-model.log`，含分割线采样全部额度前缀失败不改输出、足额恢复及参数包围；原session59180退出0，流程8项通过，日志 `/tmp/rzpdf-coons-contraction-workflow.log`。render全量原session24620运行中，日志 `/tmp/rzpdf-coons-contraction-render.log`，终态待确认；当前工作区全量仍未复验。

候选域收缩模型终态：原session38303退出0，87项shading相关测试通过，日志 `/tmp/rzpdf-coons-contraction-margin-model.log`；分割线无细分认证、额度耗尽、域外无根和连续根退化回归通过，取代下方本专项待确认状态。当前修订流程和全工作区仍未复验。

分割线修订的精确边界像素终态：原session54795退出0，专项1项通过、37项筛选，54.55秒，日志 `/tmp/rzpdf-coons-contraction-margin-pixels.log`。完整50×50采样和渲染通过，直接/PDF平方函数上下边灰度64/16、两种保存严格重开整页像素一致；本记录取代下方该固定样例失败状态。源码新增候选域收缩保留全部根，扩大包围留余量后仍严格认证存在/唯一性；不是任意曲面/折叠完整验收。新模型专项及当前全量/流程复验尚待确认，性能仍需改进。

精确像素失败已定位：原session64723退出101，`/tmp/rzpdf-coons-boundary-location.log`，pixel(0,16)对应shading(4,1056)，参数v=0.6875为内部子域分割线。不是仅外边界证书失败，需完整根候选域收缩/分割线处理；新增逐像素坐标诊断保留完整50×50采样范围。当前该测试仍失败，不认定像素验收通过。

最新精确边界像素回归尚未通过：原session79940退出101，日志 `/tmp/rzpdf-coons-boundary-precision-pixels.log`，新增`nonuniform_coons_boundary_pixels_survive_function_and_strict_save`在整片渲染阶段遇到Unresolved几何/覆盖错误，未到达u=1/4边界像素断言。工作区保留该失败用例；不能用下方既有矩形像素专项、86项模型或8项流程通过推断新样例可渲染，需继续完整根覆盖/边界唯一性实现。

整数恒等Decode修订：1..32位正向/反向整数映射精确区间回归及shading整体86项通过，原session61452退出0，日志 `/tmp/rzpdf-coons-exact-decode-shading.log`；8项流程测试通过，原session27367退出0，日志 `/tmp/rzpdf-coons-exact-decode-workflow.log`。非均匀边界矩阵包含Decode内部整数1536及端点65535两尺度，真实平方函数/镜像/导入/两种严格保存读回均正确。取代此前恒等Decode内部整数边界无法认证的限制；一般非恒等Decode的不确定边界仍未完成，新解码修订尚未像素/全量复验。

精度修订流程/像素终态：shading_workflow 8项通过，原session52735退出0，日志 `/tmp/rzpdf-coons-nonuniform-workflow-recheck.log`；新增非均匀边界使用Decode端点作为固定坐标，精确Bernstein权重计算目标，实际PDF平方函数、四镜像、导入和两保存策略严格重开全部通过。既有12组合Skia像素专项1项通过，原session1317退出0，日志 `/tmp/rzpdf-coons-precision-pixels.log`。不覆盖非端点不确定边界或新精确边界像素；当前精度源码仍未全量复验，不能沿用927项旧结果。

非均匀直边精度修订的镜像/额度专项已确认：原session72968退出0，2项通过、624项筛选，日志 `/tmp/rzpdf-coons-edge-precision-budget.log`。上下两边×无/水平/垂直/双镜像共8组合，全部额度前缀耗尽保持输出、足额重试成功，直接及平方数值灰度64/16正确；平方检查是采样参数后的数值运算，不代替PDF Function流程验收。取代下方本专项镜像/失败契约待确认状态；当前修订尚需整体与流程/像素复验。

927项全量之后发现并修复非均匀直边精度缺陷：专项首次退出101，灰度62而非64；直边唯一根改用八轮有工作额度的区间细化后，原session36551退出0，专项1项通过，上下边直接/平方灰度64/16正确，日志 `/tmp/rzpdf-coons-edge-precision-recheck.log`。本修改尚未全量复验，不能沿用下方927项结果；额度耗尽输出保持、镜像和重试仍需补齐。

当前直边修订全量终态：原session19703退出0，共46组、927项通过、0失败、0忽略，含624项PDF单元与37项render测试，日志 `/tmp/rzpdf-coons-edge-full-workspace.log`。Rust源码在验收期间冻结；本结果取代下方该轮等待状态，不证明一般边界、折叠或完整Type6交付，也不覆盖后续精度修改。

镜像专项之后的当前直边修订：既有Skia像素专项1项通过、36项筛选，原session36993退出0，日志 `/tmp/rzpdf-coons-edge-pixels.log`；覆盖Type4/5/6固定12组合和两种保存策略严格重开整页像素一致性，不是新增精确边界像素验收。全工作区复验已启动，原session19703，日志 `/tmp/rzpdf-coons-edge-full-workspace.log`，最终结果待确认。

轴向直边修订：shading单元82项和导入/两种保存策略严格重开流程7项通过，日志分别为 `/tmp/rzpdf-coons-production-edges.log`、`/tmp/rzpdf-coons-edge-roundtrip.log`，对应提交fb4ff54。后续镜像方向与不确定控制区间拒绝专项2项通过，原session90190退出0，日志 `/tmp/rzpdf-coons-edge-direction.log`。当前修订尚未像素及全工作区复验，不能沿用角点版925项结果；任意方向直边、弯曲边界和折叠仍未完成。

角点修订版全量终态：原session43908退出0，`cargo test --workspace --locked --quiet` 共46组结果、925项通过、0失败、0忽略，含622项PDF单元和37项render测试，日志 `/tmp/rzpdf-coons-corner-full-workspace.log`。此结果取代下方角点修订“尚未全工作区复验”的状态；仍不表示一般边界、连续根、任意折叠或完整Type6验收完成。

提交ba4ac29之后的角点修订：张量转换直接保留边界控制点区间；精确角点见证结合覆盖整个原域的延拓唯一性证书，认证成功才用于生产采样，失败继续原完整搜索。shading单元81项（`/tmp/rzpdf-coons-production-corners.log`）、加入四角原文档/导入/两保存策略严格重开断言的流程7项（`/tmp/rzpdf-coons-corner-roundtrip.log`）及既有12组合Skia像素专项1项（`/tmp/rzpdf-coons-corners-pixels.log`）均退出0。仅证明可认证角点及固定样本无回归；一般边界、连续根、任意折叠和独立阅读器验收仍未完成，后续修订尚未全工作区复验，不能套用下方920项旧源码结果。

全工作区终态补充：原session5708最终退出0，`cargo test --workspace --locked --quiet` 共46组结果、920项通过、0失败、0忽略，日志 `/tmp/rzpdf-coons-workspace-tests.log`。验收源码对应提交ba4ac29，包含617项PDF单元与37项render测试；本段取代下方该轮“尚待终态”状态，但不代表完整Type6或整个实施计划完成。

生产 Shading 构造/点采样已接入 Coons 补丁。预算修订版 `cargo test -p pdf --lib color::shading --locked --quiet` 退出0，76项相关单元通过、541项筛选，日志 `/tmp/rzpdf-coons-sample-budget-tests.log`；包含直接分量/非线性函数顺序、域外及采样内存/工作额度拒绝后恢复。`cargo test -p pdf --test shading_workflow --locked --quiet` 原版退出0、6项通过，日志 `/tmp/rzpdf-coons-workflow-tests.log`；资源矩阵保留Type4/5并加入Type6，覆盖显示来源、导入和两种压缩策略严格重开。随后新增截断流注册失败恢复测试，修订专项仍待确认，不能计入该6项结果。

后续专项确认：函数域修订模型专项退出0、76项通过，日志 `/tmp/rzpdf-coons-function-domain-tests.log`，补过窄Domain拒绝；注册恢复修订专项退出0、7项通过，日志 `/tmp/rzpdf-coons-workflow-recovery-tests.log`，补截断流注册拒绝不修改revision/保存字节及修复后成功。前段“仍待确认”是原6项检查点状态，由本段终态取代。

Skia专项 `cargo test -p render triangle_and_coons_mesh_pixels_follow_parameter_interpolation --locked --quiet` 退出0，1项通过、36项筛选，日志 `/tmp/rzpdf-coons-pixel-tests.log`。单测试内保留Type4/5八组合并加入Type6四组合，覆盖直接灰度/平方函数×正常/镜像、明确像素中心值、BBox外空白及两保存策略重开后整页像素相同。

这些是固定矩形的定向模型/流程/像素证据，不是完整Type6支持：边界根、连续根、任意曲面折叠全覆盖及独立阅读器验收仍未完成。未解决几何返回UnsupportedFeature，工作额度耗尽返回ResourceLimit，不能把报错当作域外不绘制。逐点额度和可见Vec容量检查不等同整页OperationBudget或完整临时函数输出计账。当前版本全工作区测试已启动，日志 `/tmp/rzpdf-coons-workspace-tests.log`，尚待终态；未运行fuzz。

## 三角网格 Shading 验证检查点（2026-10-08）

Flate 本库对照补充：session 90428 退出0，十四页严格读取/渲染成功；第9–14页最新 Skia PNG 均已目视检查，与同文件 Preserve 的对应六页 RGBA 逐像素相同（200×120）。连同 MuPDF 的保存策略内一致性证据，仅证明此固定网格样本的压缩保存显示不变，不消除两后端原有色阶差异。

Type4 自由网格与 Type5 格点网格已接入流解析、Shading 模型、资源注册、显示列表和 Skia 点采样。Type4 处理新三角形/共享边标志；Type5 按完整行生成索引。两者先插值颜色分量或单个函数参数，再转换颜色或求值函数；Function Domain 必须包含 Decode 参数区间，Indexed 只允许无 Function 的网格。

已通过的定向证据：

- Shading 流程6项（`/tmp/rzpdf-shading-lattice-registration.log`）：含 Type4/5 直接/非线性组合、RGB直接分量、导入与 Preserve/Flate 严格读回；非法网格/残缺格点注册不改变修订号和保存字节，修复后可以注册。
- 像素专项1项、八种参数组合（`/tmp/rzpdf-shading-lattice-pixels.log`）：Type4/5 × 直接/非线性 × 原始/镜像，检查像素中心、BBox 外白色、两保存策略严格重开全页 RGBA 相同。不按参数组合重复计算测试数量。
- Type5 模型专项19项（`/tmp/rzpdf-shading-lattice-model-tests.log`）及输入容量专项12项（`/tmp/rzpdf-shading-lattice-capacity.log`）通过，范围包括此前网格单元；不是19项全部只针对Type5。

独立样本：`create_shading` 第9–11页为 Type4，第12–14页为 Type5；各自使用2×2灰度顶点、直接/平方函数/镜像。MuPDF info/draw 与本库严格读取渲染成功，新增各页两后端 PNG 均目视检查。两类型三页的最大通道差均为2/4/4，平均最大差0.6965/0.959833/0.852875，差>3像素为0/400/388；内部选点本库值与解析公式一致，但非线性整页不能称为完全一致。证据分别在 `tmp/pdfs/shading-mesh.LNagm8/` 与 `tmp/pdfs/shading-lattice.TcXO6V/`，Poppler 缺 libtiff.5.dylib，使用现有 MuPDF，未修改系统依赖。

限制：这些固定样本不能证明任意复杂网格兼容性。边缘抗锯齿、内部逐像素分配/线性三角形扫描、工作量预算、颜色空间保留数据/解码器临时峰值/原生开销的完整共享预算仍未完成；Type6/7 和 Pattern 尚不支持。当前工作区全量运行中，结论见 TODO，不将早于源码变化的门禁宣称为当前完整绿色。

## Shading 参数模型回归（2026-10-07）

网格 Flate 补充（2026-10-08）：十四页压缩样本生成成功，MuPDF 渲染第9–14页无诊断，六张最新 PNG 已目视检查；六个网格页面与 Preserve 对应页面逐像素完全相同。证据在 `tmp/pdfs/shading-lattice.TcXO6V/flate.pdf` 和 `flate-9.png` 至 `flate-14.png`。本库 Flate 独立渲染仍在运行，尚不据此宣称其终态。

Type1 固定独立样本：create_shading 第八页使用 Type4 双输入 RGB 与 Matrix [160 0 0 80 20 20]。MuPDF info/draw 和 Skia 严格读取/渲染成功，最新两张图均目视检查；200×120、最大通道差2、平均最大差0.6、差>3为0。Skia 的 (60,60)/(100,40)/(140,80) RGB 分别 (65,126,95)/(128,190,159)/(192,62,127)，与像素中心反变换后的解析函数一致；MuPDF 选点存在1色阶采样差。临时证据 `tmp/pdfs/shading-type1.l7PEIo/`，不能泛化为整个 Type1/Pattern 验收。

最新代码门禁：`/tmp/rzpdf-shading-current-final.log` 退出 0，544 单元、25 颜色流程、17 版本与14资源测试通过。`/tmp/rzpdf-shading-form-import.log` 退出 0，Form 同名局部 Shading 覆盖、两次调用来源/矩阵/裁剪、页面状态恢复、跨文档导入及两保存策略严格读回的集成组合通过（1 个测试函数，不按参数组合重复计数）。此结果不证明尚未完成的整个工作区和全部 Shading 类型。

Form 修正样本：Shading.ColorSpace 改用显式 DeviceGray（资源别名不是合法定义），局部 Shading/S 保留。session 46424/36690 均退出 0；MuPDF 第七页无原先 LocalGray 诊断，MuPDF/Skia 双调用图像已目视检查，均 200×120。两次平移调用内各两个采样点的 Skia RGB 与 N=2 解析值一致，MuPDF 相差 1–2 色阶；调用间空白不被涂色。证据位于 `tmp/pdfs/shading-form-fixed.ULHu9o/`；尚不包含来源编辑、两策略读回和整个 Form 矩阵验收。

非均匀 DPI 补充：`render_document` 接受可选 xdpi/ydpi，参数必须有限且正。session 19394 退出 0，144×108 DPI 六页图像已逐页目视检查；未旋转页 400×180、旋转页 240×300。轴向/同心径向/剪切径向各三个选点经像素中心逆缩放与解析 N=2 求值均与实际 RGB 精确一致；证据在 `tmp/pdfs/shading-transforms.nN0A8N/skia-dpi/`。此项覆盖尺寸与选点，不代替跨后端整页和边缘验收。

### 独立阅读器验收待执行

固定样本需由本库创建并分别 Preserve/Flate 保存，使用系统 Poppler 的 `pdfinfo` 检查版本/页数，再以 `pdftoppm -r 72 -png` 渲染到 `tmp/pdfs/`；稳定的最终样本放在 `output/pdf/`。不得覆盖工作区已有 `test.png`。同一 PDF 还需通过本库 Skia 渲染，并以页面内部明确采样点比较颜色；边缘抗锯齿差异单列，不用整页字节相等作为跨后端唯一判据。

样本矩阵：轴向 N=2 非线性、同心与移动/缩小径向、两端 Extend、BBox 与当前裁剪交集、直接 sh 忽略 Background、透明度与混合模式、Form 局部命名颜色资源以及旋转/非均匀 DPI。逐页目视检查最新 PNG，记录颜色容差、坐标和已知差异；原生临时分配与性能限制单独列明。当前仅确认 Poppler 工具可用，尚未生成或验收这组独立样本，不记为完成。

2026-10-07 首轮实测更新：Poppler 启动缺 libtiff.5.dylib，改用 MuPDF 1.27.0 和 Ghostscript 9.23。`create_shading` 四页、两保存策略已经生成；MuPDF 两策略 PNG 对应哈希一致。Skia、MuPDF 和 Ghostscript 的四页图像均逐页目视检查，形状一致但未通过跨后端差异验收。200×120 的同心径向页，像素中心反变换后 `t=hypot(x+0.5-100,120-(y+0.5)-60)/50`；N=2 的解析 RGB 在 (100,60)/(140,60)/(100,40) 分别为 (0,0,255)/(167,0,88)/(39,0,216)，与 Skia 完全一致。MuPDF 在第二点为 (161,0,93)，Ghostscript 为 (167,0,87)，因此不能仅凭某一个参考后端的逐像素差异修改解析颜色求值。

覆盖范围也需独立处理：MuPDF/Skia 四页白色/非白色分类不同的像素为 120/141/121/0，其中轴向页差异集中于 x=19 的边界列。同心径向距中心小于 45 的内部区域最大通道差仍为 8，不属于覆盖边缘。暂不认定这些差异的具体实现原因；下一步补边缘面积覆盖策略、采样位置与参考后端近似算法审计，并扩大 Form/变换/透明度矩阵。临时证据位于 `tmp/pdfs/shading-validation.XVQgC4/`，不得视为整个 P4 已完成。

变换扩展样本（`tmp/pdfs/shading-transforms.nN0A8N/`）第 5 页叠加 CTM/页面旋转，第 6 页径向矩阵为 `[1 0.2 0.3 0.6 -18 4]`，MuPDF/Skia 均已逐页目视检查。旋转页两后端尺寸 120×200、最大通道差 3；剪切页尺寸 200×120、最大差 255（含覆盖边界），平均差 1.779，差>3 共 2854 像素。剪切页四个选点经 f64 矩阵逆变换与 N=2 解析计算的 RGB 与 Skia 精确一致；这证明选点的变换/求值，不等于整页跨后端验收或非均匀 DPI 验收。

Type2 轴向/Type3 径向 Shading 现有独立参数模型和归一化位置颜色求值。单元检查单函数/组件函数数组、Domain 映射、Extend/BBox/背景/抗锯齿提示、非法参数、Pattern/Indexed 拒绝及跨组件函数树额度失败后重试；依据 [Adobe PDF Reference 1.4 的 Axial/Radial Shadings](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf)。

模型阶段 `/tmp/rzpdf-shading-model-final.log` 退出 0：541 单元、22 颜色流程通过。后续已接入 sh 资源解释、显示事件和 Skia 实际绘制；最终模型/资源回归为 544 单元、23 颜色流程、14 资源测试通过。`/tmp/rzpdf-shading-state.log` 退出 0：2 个专项检查轴向 N=2/径向明确像素与两保存策略严格重开、裁剪/BBox/透明度和预算失败恢复。渲染全量结果见 TODO，数值与库内像素测试不代替独立阅读器验收；Type1/4–7 和 Pattern 尚未完成。

## Type4 计算函数回归（2026-10-07）

最终 render `/tmp/rzpdf-type4-render-final.log` 退出 0：33 通过、0 失败/忽略（52.73 秒），包含扩展后的 Type4 像素组合；取代本节下方早于扩展的全量记录。当前 PDF 最终为 539 单元、22 颜色流程通过；全工作区与独立阅读器仍未重跑。

Skia 专项 `/tmp/rzpdf-type4-pixels.log` 退出 0：1 通过、32 filtered；既有图像像素测试增加 Type4 Separation 和六分量 DeviceN，检查明确黑白、默认/反转以及两保存策略严格重开全页一致。该测试与原校准/专色组合共 16 种参数情形，仍是一个测试函数。此前既有 render 全量 `/tmp/rzpdf-type4-render.log` 退出 0：33 通过（53.10 秒），早于本组合扩展。更新下方尚无 Skia Type4 像素验证的历史说明，独立阅读器和当前全工作区仍未完成。

Type4 已接入 PDF 计算函数的 40 个运算符与 true/false：数值、比较/逻辑/位运算、条件过程和栈操作。程序必须符合 PDF 数字/注释/花括号语法，拒绝一般 PostScript 命令与 radix/exponent 数字；构造和执行均使用显式帧与存储/深度/步数限额，copy/roll 另计工作量。输入按 Domain 裁剪，结束时严格要求 Range 指定数量的数字输出，再裁剪为 f32。依据 [Adobe PDF Reference 1.4 的 PostScript Calculator Functions](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf)。

单元覆盖所有运算符类别、整数溢出提升实数、逻辑右移、负数 round/roll、条件分支、错误类型/未定义计算、预算失败后重试、100 项栈、字面量上界以及超过 f32 的有限中间值裁剪。间接参数严格重开验证实际 add 求值；真实 Separation/DeviceN 条件函数路径和图像输出绿/红，两保存策略严格读回保持一致。

最终 `/tmp/rzpdf-type4-boundaries-final.log` 终态退出 0：539 单元、22 流程通过，0 失败/忽略。不是 Skia Type4 像素、独立阅读器或全工作区验收；程序/参数/求值缓冲分别受现有限额，不代表同池 OperationBudget 的共存峰值已贯通。

## 普通专色图像回归（2026-10-07）

最终包含扩展像素组合的 render `/tmp/rzpdf-spot-image-render-final.log` 退出 0：33 通过、0 失败/忽略（53.78 秒）。PDF 最终为 530 单元、21 流程通过；不是新全工作区或独立阅读器门禁。

像素专项已补齐选定范围：既有校准图像测试加入 Separation Spot、六分量混合 None DeviceN、全 None DeviceN、Separation None；与 Lab/ICC 合计 12 个默认/反转组合。检查明确像素、None 全白、对象仍保留及两保存策略重开后全页一致。`/tmp/rzpdf-spot-image-pixels.log` 退出 0：1 通过、32 filtered。此结果更新下方“无新专色像素专项”的历史状态，不是独立阅读器或原生色版验收。

补充最终 PDF `/tmp/rzpdf-spot-image-budget-final.log` 退出 0：530 单元、21 流程通过，0 失败/忽略。新增错误/预算单元覆盖 Decode 参数、采样长度、端点预留临界值、保留容量及输出额度；此项取代下方额外错误/额度测试待补齐的历史状态，但不代表完整 OperationBudget。render `/tmp/rzpdf-spot-image-render.log` 退出 0：33 通过、0 失败/忽略（52.16 秒），仅既有回归，无新专色像素专项。

Separation/DeviceN 现已复用 packed 高精度采样到 tint-transform 的显示转换，动态 Decode 数组支持超过四分量。`six_component_devicen_images_preserve_rows_decode_and_save` 使用六分量、混合 None 名称、Sampled tint-transform，验证一位采样的两行填充与第六分量反转，黑/红/绿/黄 RGBA，以及两保存策略严格读回。16 位 Separation 反转加入既有采样单元。

`/tmp/rzpdf-spot-image-regressions.log` 终态退出 0：529 单元、21 流程通过，0 失败/忽略。额外错误/额度测试、专色图像 Skia 新专项与当前全工作区仍待补齐；不是原生色版/叠印或独立阅读器验收。下方校准图像历史记录中普通 Separation/DeviceN 尚未支持的状态由本节更新。

## 普通校准颜色图像回归（2026-10-07）

最终 render 全量 `/tmp/rzpdf-calibrated-image-render-final.log` 退出 0：33 通过、0 失败/忽略（48.66 秒），包含新增 Lab/ICC 像素专项；取代本节下方全量仍运行的历史说明。PDF 最终回归仍为 529 单元、20 流程通过；尚未重跑全工作区。

新增 `calibrated_image_pixels_and_reverse_decode_survive_save`：普通 Lab/ICC 16 位图像的默认/反转共四组合，分别核对黑白与 64/191 灰度，Preserve/FlateUnencoded 严格重开后全页像素一致。专项 `/tmp/rzpdf-calibrated-image-pixels.log` 退出 0：1 通过、32 filtered。这是本库 Skia 后端专项，不是独立阅读器对照；包含新增测试的全量 render 回归仍在运行。

补充：`/tmp/rzpdf-calibrated-image-render.log` 终态退出 0，既有 render 32 测试全部通过、0 失败/忽略（47.28 秒）。此结果取代本节下方的“Skia 回归仍待确认”，但没有新增普通校准图像像素专项，不能声称新颜色空间的视觉验收已完成。

CalGray/CalRGB/Lab/ICCBased 图像保留 packed 采样，RGBA 转换直接应用 Decode 后进入颜色转换，支持 1/2/4/8/16 位逐行采样。Lab 与 ICC 默认 Decode 使用各自分量范围，依据 [Adobe PDF Reference 1.4 的 Default Decode arrays](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf)。ICC 仍走 Alternate，不是 profile 管理。

新增单元覆盖 Lab 默认范围、16 位 CalGray 反转与 CalRGB 行填充；资源流程扩展普通 Lab/ICC 16 位默认/反转及两保存策略严格读回，ICC fixture 使用空 profile 专测 Alternate。最终 `/tmp/rzpdf-calibrated-image-final.log` 退出 0：529 单元、20 流程通过，0 失败/忽略。不是独立阅读器或全工作区验收；Skia 回归仍待确认。

## Indexed 调色板缓存回归（2026-10-07）

RGBA 转换新增局部固定 256 槽惰性缓存，重复索引复用已转换颜色，不提前求值未使用表项。既有索引采样单元增加跨行重复与 hival 外索引用例，颜色流程覆盖 Lab/ICC/DeviceN 基空间及严格保存读回。`cargo test -p pdf --lib --test color_workflow --locked --quiet` 终态退出 0：528 单元和 20 流程通过，0 失败/忽略；日志 `/tmp/rzpdf-indexed-palette-cache.log`。没有新增测试函数，测试数不增加；没有性能测量或独立阅读器验收。

下方 812 全工作区结果对应缓存修改前的提交，不替代当前修改的全量验证。

## 当前全工作区门禁（2026-10-07）

本地提交 `853ae38` 的生产代码与测试已通过 `cargo test --workspace --locked --quiet`：进程终态退出 0，45 条测试结果合计 812 通过、0 失败/忽略，包含 PDF 单元 528、39 个集成测试目标与 render 32；文档测试正常结束。日志 `/tmp/rzpdf-workspace-image-colors.log`。提交前被测源差异 SHA-256 为 `732f29e15d894cba31f088155e5c9f42604e2542e3c5b212a4c66466652596dd`，提交未改变代码内容。

此结果更新下方各专项当时尚未重跑全工作区的状态，不代表远端 CI、独立阅读器对照或全部 TODO 完成。fuzz 不在 workspace 成员内，未执行。

## Indexed DeviceN / None 回归（2026-10-07）

Indexed 可使用 DeviceN 基空间，参考 [Adobe PDF Reference 1.6 的 Indexed/DeviceN 示例](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.6.pdf)。混合 None 分量可能携带替代颜色数据，显示转换仍接收完整 tint 数组；本库只支持已有函数求值器和替代空间近似，不实现原生色版或 Type4 求值。

`indexed_devicen_maps_all_palette_tints_including_mixed_none` 通过双输入 Sampled 函数核对白/青/品红/蓝的路径颜色和低位深图像 RGBA，分别使用 Cyan/Magenta 和 Cyan/None 名称，两种保存策略严格读回保持一致。

`indexed_none_bases_suppress_paths_and_images_without_losing_objects` 覆盖 Separation None 和全 None DeviceN 基空间：DisplayList 三个对象保留，路径/图像回调抑制，后续红色路径仍绘制，两种保存严格读回相同。已有 Skia None 像素测试增加这两种 Indexed 包装，核对白色页面、文字保留及组合绘制可见侧；未新建测试函数，render 总数不因此增加。

日志 `/tmp/rzpdf-indexed-devicen-none.log` 终态退出 0：528 单元、20 颜色流程通过；`/tmp/rzpdf-indexed-none-render.log` 终态退出 0：32 通过（48.47 秒）。均无失败/忽略。Indexed 图像的新独立像素/阅读器对照、全部函数/原生多色版和全链路预算仍未完成；未运行全工作区/fuzz。

## Indexed 基空间范围回归（2026-10-07）

调色板字节 0/255 分别映射基空间各分量范围的下/上界，不能统一除以 255 后直接作为 Lab 分量。依据 [Adobe PDF Reference 1.4，Indexed Color Spaces](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf)。Lab 使用 L=0..100 与参数 a/b Range，ICCBased 使用声明 Range；ICC 输出仍为 Alternate 近似。

新增 Lab 黑/白端点及非对称 a/b 范围对照，ICC 灰度与 RGB 非默认范围的明确字节期望；另通过真实 Image 资源注册/页面解释，核对 Lab 黑白和 ICC 64/191 灰度，两种保存策略严格读回保持一致。ICC fixture 使用空 profile 流，专门验证现有 Alternate 路径，不声称 profile 管理验收。

Lab 专项 `/tmp/rzpdf-indexed-lab-range.log` 退出 0：1 通过、526 filtered；早于 ICC 新增测试。最终 PDF 回归 `/tmp/rzpdf-indexed-base-ranges-final.log` 退出 0：528 单元、18 颜色流程通过；render 回归 `/tmp/rzpdf-indexed-base-render.log` 退出 0：32 通过（47.23 秒）。均无失败/忽略。分量缓冲改为可失败预留，映射采用 f64 中间值；不是全链路 OperationBudget 贯通。独立阅读器/Skia 新专项、全 ICC 与非零 Lab BlackPoint 仍未完成；未运行 fuzz。

## Indexed image 采样回归（2026-10-07）

Indexed 的默认 Decode 让原采样索引保持不变，不是 [0 1] 灰度归一化；参考 [Adobe PDF Reference 1.3，Decode Arrays](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.3.pdf)。显式 Decode 映射后取整，再通过 hival 裁剪访问调色板。逐行位填充、1/2/4/8 位及 legacy 16 位采样、反转/越界端点与错误长度都有确定性字节测试；不表示已实现 PDF 2.0 严格合规判定。

`indexed_images_preserve_indices_across_depths_rows_and_decode` 专项首轮退出 0：1 通过、525 filtered（该轮早于 16 位追加用例）。`indexed_image_palette_and_decode_survive_strict_save` 经真实 Image 资源注册/页面解释，覆盖两行低位深、反转 Decode 与 Preserve/FlateUnencoded 严格读回相同 RGBA。

首轮 `/tmp/rzpdf-indexed-image-regressions.log` 退出 0：526 单元、17 颜色流程通过，0 失败/忽略；16 位追加后的最终 `/tmp/rzpdf-indexed-image-final.log` 同样退出 0：526 单元、17 颜色流程通过，0 失败/忽略。此轮没有新增 Skia 像素或独立阅读器验收，Lab/ICC 基空间映射、其他颜色空间图像及聚合预算仍待补齐，未运行 fuzz。

## Stencil image mask 回归（2026-10-07）

一位 ImageMask XObject 用当前非描边颜色绘制；默认 Decode [0 1] 的零采样覆盖、一采样透明，[1 0] 反转。规则依据 [Adobe PDF Reference 1.4，Stencil Masking](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf)。无 ColorSpace，BitsPerComponent 缺省为 1；非法位深/颜色空间/SMask/Decode、错误数据长度与 RGBA 预算不足均拒绝。

`stencil_samples_use_current_color_and_reverse_decode` 检查逐行填充位、默认/反转 alpha、同一图像红/绿重着色及非有限颜色拒绝。专项日志 `/tmp/rzpdf-stencil-unit.log`：退出 0，1 通过、524 filtered。

`stencil_pixels_use_current_fill_decode_and_none_after_save` 在蓝背景绘制同一 mask 的红/绿实例，检查默认/反转覆盖像素、None 不绘制，两保存策略严格读回逐字节一致。日志 `/tmp/rzpdf-stencil-pixels.log`：退出 0，1 通过、31 filtered。是本库 Skia 像素验证，非独立阅读器对照。

完整 PDF 单元与 color_workflow 回归日志 `/tmp/rzpdf-stencil-regressions-final.log`：退出 0，525 单元、16 颜色流程通过；render 单元日志 `/tmp/rzpdf-stencil-render-all.log`：退出 0，32 通过（52.69 秒）。均无失败/忽略，未重跑整个工作区。首轮旧错误类型预期已更新：带 ColorSpace 的 ImageMask=true 属参数错误，而非所有 stencil 一概不支持。

普通灰度是 stencil 的内部样本载体，不表示 stencil 有 PDF ColorSpace。`rgba_buffer_with_stencil_color` 必须显式提供有限 RGB；无颜色转换报错。Pattern 着色、内联图像、独立读者对照与完整 P4 未完成；未运行 fuzz。

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

## 完整工作区验证（2026-10-07）

```sh
cargo test --workspace --locked --quiet
```

同一次进程完整运行后退出码0；pdf单元515项、39个pdf集成目标合计230项、render测试30项，合计775项全部通过、0失败/忽略。日志共45条测试目标汇总，包含零测试目标；不把零测试目标计为额外测试用例。最近的Stitching/Type4参数/Lab与颜色工作流回归，以及当前工作区其余测试均包含在本次运行内。

平台Darwin x86_64；rustc 1.96.0 (ac68faa20 2026-05-25)，cargo 1.96.0 (30a34c682 2026-05-25)。git diff --check通过。构建输出仍有未使用项及本机`.cargo/config`弃用警告，未改用户全局配置或系统依赖。

本次只验证本地默认工作区测试，不运行独立fuzz工作区，不触发远端CI，也不代表独立阅读器视觉/颜色管理、全部平台或TODO全部功能验收。`.github/workflows/ci.yml`仍配置Ubuntu原生依赖与同类工作区命令，远端执行证据待补；P0-01保持未完成。本轮未提交。

## Stitching 函数回归（2026-10-07）

实现依据为 [ISO 32000-1:2008 §7.10.4 / Table 41](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf)：Type3 拼接单输入子函数，统一输出维度，按区间映射输入并裁剪到可选 Range。内部边界归右侧区间；末界等于 Domain 上界时以该子函数 Encode 起值求值。规范搜索摘要可检索，链接直接读取当前返回404；没有把失败下载计为独立验证。

```sh
cargo test -p pdf --lib --locked --quiet
```

最终504项单元测试全部通过、0失败/忽略，包含6项Stitching回归和1项Separation颜色转换回归。覆盖非0–1域、上下界输入裁剪、分段不连续边界、反向Encode、单子函数/嵌套多输出、Range裁剪、端点Bounds、宽域插值、非法输入/默认对象、子函数维度/Range拒绝、Sampled子函数、间接参数及子函数错误传播。Separation样本保存后严格重开，红/蓝/白RGB端点与预期一致。

首轮宽域样本使用指数数字语法导致解析失败，已改为PDF合法完整十进制并重新运行全部pdf单元。此处为确定性数值与库内往返，不是独立阅读器或完整Shading渲染验收；Type4求值、Pattern/Shading和操作级聚合预算仍未完成。本轮未重跑集成目标/全工作区，也未运行fuzz。

## Type4 参数构造回归（2026-10-07）

`cargo test -p pdf --lib --locked --quiet` 最终508项全部通过、0失败/忽略，包含新增4项Type4构造回归：

- 间接Domain/Range及间接数值，保存后严格重开保留二输入/一输出维度；求值仍返回UnsupportedFeature。
- 非法/缺失参数在未知滤镜解码前返回FunctionError；合法参数的未知滤镜保留FilterError契约。
- 参数单数组请求字节8成功、7拒绝，4个参数条目在max_objects=3时拒绝；解码失败后短输入可重试，文档修订号不变。
- 数组数值引用成环时可重复返回错误，随后合法构造恢复，修订号不变。

首轮只有未知滤镜错误类型断言失败（507通过/1失败）；按现有FilterError契约修正后重跑全部pdf单元通过。此处验证参数构造与库内往返，不验证Type4程序语法/执行、独立渲染或聚合内存；两数组与解码结果共存仍未纳入OperationBudget。本轮未运行集成目标、全工作区或fuzz，未提交。

## Lab 颜色数值回归（2026-10-07）

`cargo test -p pdf --lib --locked --quiet` 最终513项全部通过、0失败/忽略，包含新增5项Lab数值/往返回归：D50、D65与等能白点的黑/白/中灰及低亮度；D65的RGB主色（8位通道允许1级误差）；L/a/b及自定义Range裁剪；组件数量/非有限输入、非零BlackPoint和不可适配白点错误；间接WhitePoint/数值/参数字典保存严格重开后中灰为119/119/119。

Lab反变换、XYZ/sRGB矩阵和传递函数参照 [W3C CSS Color 4转换资料](https://www.w3.org/TR/css-color-4/#color-conversion-code)；Bradford白点变换核对 [LittleCMS原作者实现](https://raw.githubusercontent.com/mm2/Little-CMS/master/src/cmswtpnt.c)。Lab的源白点来自PDF字典，不固定为CSS Lab的D50。这里的回归是数值预期和库内往返，不是独立ICC色彩引擎/阅读器或视觉验证。

当前仅零BlackPoint，固定sRGB通道裁剪；非零BlackPoint补偿仍明确不支持。OutputIntent/ICC意图、感知色域映射和CalRGB/CalGray策略统一尚未完成。本轮未运行集成目标、全工作区或fuzz，未提交。

## Lab 参数路径收口（2026-10-07）

`cargo test -p pdf --lib --locked --quiet` 最终515项全部通过、0失败/忽略。新增2项回归验证WhitePoint/BlackPoint/Range的畸形长度数组包含循环引用时，固定数组读取器先返回ColorError、不进入成员引用解析；直接与上下文构造的默认值、合法参数及非法白点/黑点/Range验证一致。前轮间接参数保存严格重开与颜色数值测试同时继续通过。

此次去除参数字典回写和逐成员PdfArray重建，返回参数采用固定数组，普通resolve仍可能分配拥有副本；不表示整个颜色构造的聚合内存已计费。未运行集成目标、全工作区、独立视觉验证或fuzz，未提交。

## 颜色公共工作流集成回归（2026-10-07）

```sh
cargo test -p pdf --test color_workflow --locked --quiet
```

新增3项全部通过、0失败/忽略。使用PageBuilder注册页内Lab与Separation/Stitching颜色资源，真实内容操作符设置填充/描边、切换专色及q/Q恢复；DisplayList向自定义颜色设备回放得到灰119/119/119、蓝0/0/255和白255/255/255。Preserve/FlateUnencoded两种保存严格重开后结果一致，保存不改变修订号；复制页后只编辑副本保持原页灰色/副本白色。不支持的非零BlackPoint通过设备错误传播停止回放，后续红色路径不被绘制。

初次编译使用错误的SaveOptions引用参数，按现有按值API修正后重新执行成功。本轮未改生产代码，未重跑单元/全工作区，未做独立阅读器或Skia像素验证；自定义设备调用本库RGB转换，不能据此排除共享转换缺陷。未提交，fuzz仍暂缓。

## 共享颜色函数修订回归（2026-10-07）

`cargo test -p pdf --test color_workflow --locked --quiet` 最终4项全部通过、0失败/忽略。新增用例从严格打开的源文档加载两页共享间接Stitching函数；蓝色函数写入新红色函数后旧文档stamp失效，两页新解释均为红色，已有DisplayList设备回放仍为蓝色旧快照。再次保存严格重开后两页红色保持。

这里特意验证低层修改共享对象影响所有引用页，而不是复制后局部编辑；如仅修改单页，仍须先复制资源及重新绑定。已有DisplayList不会自动重新解释，应检查修订号并重新读取。测试使用本库自定义颜色设备，不是独立阅读器/像素验证。本轮生产代码未改，未重跑全工作区；前轮775项完整通过不包含本次新增用例。未提交，fuzz暂缓。

## Lab 数组标识校验（2026-10-07）

`cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出码0：pdf单元516项、颜色工作流集成4项全部通过，0失败/忽略。新增回归验证两种Lab构造入口拒绝非Lab名称、数字/null/字符串/布尔首项；正确直接名称保持兼容，上下文入口支持间接名称，直接入口不解析引用。间接名称修改为DeviceRGB或成环后拒绝，恢复为Lab可重新构造。参数字典入口仍保留。

本轮未重跑全工作区、render或独立视觉验证；先前775项完整通过记录不应视为包含此新改动。git diff --check通过，未提交，fuzz仍暂缓。

## 颜色空间资源注册与导入（2026-10-07）

`cargo test -p pdf --lib --test color_workflow --test resources --locked --quiet` 退出码0：pdf单元516项、color_workflow7项、resources14项全部通过，0失败/忽略。新增3项集成回归覆盖ColorSpace类别注册的CS1冲突避让与CS2复用、重复注册不改修订号、共享间接类别表不被修改；Lab数组及间接白点跨文档导入、外文档句柄拒绝、修改源参数不影响目标、同文档资源复用和严格保存重开；非法资源注册及外层事务失败不改变页面保存字节，失败后CS1仍可重试。

注册使用现有颜色空间解析器，不能据此证明所有类型可渲染。枚举新增ColorSpace分支，下游穷举match需要迁移。本轮未运行全工作区/render或独立视觉验证，不把先前775项完整通过当作本次变更验收；未提交，fuzz暂缓。

## 颜色空间导入失败边界（2026-10-07）

`cargo test -p pdf --test color_workflow --locked --quiet` 退出码0，8项全部通过、0失败/忽略。新增用例先以非法Lab白点验证ColorError拒绝不改目标，恢复白点后以max_objects=4触发间接参数复制中途ResourceLimit；两次失败均保持目标保存字节和源文档不变。随后DeviceRGB名称资源导入成功，对象号为3，证明此前临时对象和编号未泄漏；严格重开后对象值保持。

本轮仅新增确定性集成回归，不改生产代码；未重跑单元/全工作区或独立视觉验证，不将节点数量限制宣称为OperationBudget。未提交，fuzz仍暂缓。

## 颜色空间直接构建入口（2026-10-07）

`cargo test -p pdf --lib --test color_workflow --test resources --locked --quiet` 退出码0：pdf单元516项、color_workflow9项、resources14项全部通过，0失败/忽略。新增回归验证register_color_space原子创建Lab、重复相等值和间接根复用已有名称且不改修订、非法参数/页面保持字节、外层失败后资源字节/CS2名称恢复并可重试、保存严格重开中灰保持。

值复用采用现有PdfObject相等规则，不比较不同间接图的语义等价；裸引用须属于当前文档，不能用此入口替代跨文档导入。注册解析不表示全部颜色类型可渲染，普通对象副本与事务仍未完整计入OperationBudget。本轮未运行全工作区/render或独立视觉验证，未提交，fuzz暂缓。

## 直接颜色注册对象额度边界（2026-10-07）

`cargo test -p pdf --test color_workflow --locked --quiet` 退出码0，10项全部通过、0失败/忽略。新增用例在max_objects=16下创建页及DeviceRGB资源，再填满对象额度；重复注册已有值复用CS1且stamp保持，新DeviceGray两次创建均ResourceLimit拒绝，页资源字典与CS1原值保持不变，拒绝后已有值仍可复用。

此项验证数量上限下的复用/拒绝和资源状态，不证明OperationBudget、保存峰值或解除删除对象后额度回收。本轮未改生产代码、未重跑单元/全工作区/独立视觉验证，未提交，fuzz暂缓。

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

## 编辑示例独立验证（2026-10-07）

使用本项目现有 `edit_content` 示例，生成原页/复制后编辑页的两页对照 PDF。样本完全由项目生成，没有引入第三方文件；没有覆盖既有验证产物。

```sh
cargo run -p pdf --example edit_content --locked --quiet -- output/pdf/edit-validation.Udmdci/edited-comparison.pdf
qpdf --check output/pdf/edit-validation.Udmdci/edited-comparison.pdf
qpdf --show-npages output/pdf/edit-validation.Udmdci/edited-comparison.pdf
mutool draw -F txt -o - output/pdf/edit-validation.Udmdci/edited-comparison.pdf
mutool draw -r 72 -o output/pdf/edit-validation.Udmdci/page-%d.png output/pdf/edit-validation.Udmdci/edited-comparison.pdf
```

- qpdf 11.2.0：无语法或流编码错误，页数为2，不代表完整规范符合性。
- MuPDF/mutool 1.27.0：第一页提取 `Original content`，第二页提取 `Edited content`。
- 按 PDF 技能逐页检查72 DPI渲染图：原页文字、两次红绿图片绘制和红/蓝矩形保持；编辑页文字替换正确，第一张图片旋转为竖向红上绿下，第二次图片绘制消失，红色路径绘制消失而蓝矩形保留。未见明显裁切、重叠、异常黑块或不可读字符。
- SHA-256：`b2712b6c78c5c1e2761a0c4aa4925e8b928873e8abc245b20304be9d9b6ee953`。

产物：[PDF](../output/pdf/edit-validation.Udmdci/edited-comparison.pdf)、[原页](../output/pdf/edit-validation.Udmdci/page-1.png)、[编辑页](../output/pdf/edit-validation.Udmdci/page-2.png)。复现时使用新目录；示例拒绝覆盖已有PDF。

Poppler仍因缺少 `libtiff.5.dylib` 无法启动，不计为通过；使用已安装MuPDF作为PDF技能渲染回退，未修改系统依赖。此验证覆盖固定Helvetica英文、顶层图片/路径与复制页隔离，不覆盖真实复杂字体、Form/内联图片、裁剪透明或完整P3验收。fuzz未运行。
# 函数最低版本定向回归（2026-10-07）

None 直接执行与像素回归：`cargo test -p pdf --lib none_path_dispatch_matches_direct_interpretation_without_losing_objects --locked --quiet` 退出 0，1 通过/523 filtered；直接解释与回放 callback 相同、原对象不丢失。`cargo test -p render --lib none_colorants_do_not_paint_text_or_paths_and_preserve_visible_combined_sides --locked --quiet` 退出 0，1 通过/30 filtered。随后完整 `cargo test -p render --lib --locked --quiet` 退出 0，31 全通过，0 失败/忽略。日志依次 `/tmp/rzpdf-none-direct-dispatch.log`、`/tmp/rzpdf-none-render-pixels.log`、`/tmp/rzpdf-none-render-all.log`。

Separation None/全 None DeviceN 的 Skia RGBA 像素确认隐藏路径和文字不绘制、文字仍可提取，混合填描保留可见侧和白色内部；两种保存策略严格读回的像素逐字节一致。此为内部后端像素验收，不替代独立阅读器/真实样本对照，也不覆盖 stencil、全部图像空间或文字裁剪后端。仅新增测试，未全 pdf 单元或全工作区重跑；渲染测试通过后仅局部格式整理。未运行 fuzz。

None 绘制分派：`cargo test -p pdf --lib --test color_workflow --test resources --locked --quiet` 最终退出 0，523 单元、15 颜色流程、14 资源通过，0 失败/忽略；日志 `/tmp/rzpdf-none-paint-dispatch.log`。原对象 13 个/文字 A×8 保留、None 填描组合可见侧、文字八模式保留裁剪位、普通图像不受 None 填充色影响及两种保存策略严格读回覆盖。旧 None 错误回放用例现验证成功跳过与后续绘制。首轮 stencil fixture 因既有解析不支持而失败，保留该限制并改用普通图像；不据此宣称 stencil 文件/像素支持。直接 RGB None 仍 UnsupportedFeature；真实渲染像素、None 专项直接解释一致性和文字裁剪后端尚未验收，未全工作区重跑或 fuzz。

Separation 特殊色名显示：`cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0，523 单元、14 颜色流程通过，0 失败/忽略；日志 `/tmp/rzpdf-separation-special-colorants.log`。All 使用 Type4 fixture（执行仍不支持）证明绕过 tint-transform，覆盖灰度填充/描边、tint 裁剪、严格保存读回；None 直接 RGB 错误在颜色记录前传播。此测试不证明 None 的设备级不绘制、原生分色或叠印；这些仍待跨路径/文字/图像处理。未重跑全工作区或 fuzz。

Separation 构造一致性：`cargo test -p pdf --lib --test color_workflow --test resources --locked --quiet` 退出 0：523 单元、13 颜色流程、14 资源测试通过，0 失败/忽略；日志 `/tmp/rzpdf-separation-dimensions.log`。低层错误家族、tint-transform 输入/输出维度不匹配、注册失败字节级不变及名称重试、合法灰度页面严格读回覆盖。Type4 双输入 fixture 仅用于构造维度校验；不表示 Type4 求值、All/None 绘制语义或原生专色支持。未全工作区重跑或 fuzz。

Type2 宽系数求值：依据 [Adobe Reference 1.3 §3.9.2](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.3.pdf)，f64 计算中间值、按可选 Range 裁剪后返回 f32。常量输出不依赖溢出的幂；NaN/最终非有限输出仍报错。`cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0，522 单元、12 颜色流程通过，0 失败/忽略，日志 `/tmp/rzpdf-type2-wide-evaluation.log`。宽有限系数端点/中点、大幂裁剪/常量、真正超范围错误及 Type3 错误传播覆盖；旧 Type3 误把中间精度溢出当作错误的 fixture 已替换为真正不可表示结果。未全工作区重跑、独立视觉验证或 fuzz；仍不是任意精度函数计算。

Type2 参数局部限额：`cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0，521 单元、12 颜色流程通过，0 失败/忽略；日志 `/tmp/rzpdf-type2-parameter-limits.log`。数值数组在分配前检查节点数/字节上限、Domain 长度，参数及输出使用可失败预分配。覆盖 8/7 字节边界、C0/C1/Range 超限与恢复。对象/数组的普通 resolver 副本仍先于这些检查，局部上限不代表 OperationBudget 共存峰值。此前全工作区 792 项终态早于本次修改；未重跑全工作区或 fuzz。

DeviceN 构造入口与额度精确边界：`cargo test -p pdf --lib --test color_workflow --locked --quiet` 退出 0，520 单元、12 颜色流程通过，0 失败/忽略，日志 `/tmp/rzpdf-devicen-constructor-limits.log`。首项错误家族/类型拒绝、间接家族修改、名称槽位和名称字节精确边界、组件超限及反复失败恢复均覆盖。生产新增家族校验，因此下面 792 项门禁代表此前代码状态，不代表这一新增修改已全工作区重跑；局部字节上限也不代表共存峰值或 OperationBudget 验收。

## 本批改动的全工作区门禁

`cargo test --workspace --locked --quiet` 同一进程终态退出 0。完整日志 `/tmp/rzpdf-workspace-colors-functions.log`：518 个 PDF 单元、39 个 PDF 集成目标共 244 项、30 个 render 单元，总计 **792 通过，0 失败，0 忽略**。45 条结果包含四个零测试目标。最终计数来自全部结果行求和，取代过程更新中的手工累计数。

工具链：rustc 1.96.0 (ac68faa20 2026-05-25)、cargo 1.96.0 (30a34c682 2026-05-25)，现有 macOS 本地环境。包含本批 DeviceN、函数/颜色版本检查及已有资源/编辑/保存/渲染测试；测试期间未继续修改源码。仍有既有 unused/deprecated-config 警告。没有运行独立 fuzz 工程、远端 CI 或独立阅读器视觉对照；本地绿色不等于完整 PDF 支持或跨平台验收完成。

DeviceN 间接字段与恢复：`cargo test -p pdf --lib --test color_workflow --locked --quiet` 最终退出 0，518 单元、12 颜色流程测试通过，0 失败/忽略；日志 `/tmp/rzpdf-devicen-indirect.log`。间接名称数组/名称/替代空间/函数/属性/Subtype、连续失败后修复恢复、旧模型隔离及严格保存读回覆盖。普通 resolver 只跟随一层引用：alias 为 ColorError，实际颜色数组循环为 ResourceLimit；首次错误期望已修正，失败结果不计入通过。未重跑版本集成或全工作区，未运行 fuzz。

DeviceN 实现回归：依据 [Adobe PDF Reference 1.6 §4.5.5](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.6.pdf)，实现 tint-transform 替代颜色空间显示近似。`cargo test -p pdf --lib --test color_workflow --test feature_versions --locked --quiet` 最终退出 0：517 单元、12 颜色流程、15 版本测试通过，0 失败/忽略，日志 `/tmp/rzpdf-devicen-workflow.log`。单分量默认/组件数/裁剪/非有限拒绝，双分量采样函数到 RGB 的默认蓝色及白/青/品红结果、Preserve/FlateUnencoded 严格读回，非法名称/维度/替代空间/属性注册回滚及全 None UnsupportedFeature 传播均通过。未证明独立像素一致性、原生专色/叠印/NChannel 混色、聚合操作预算或最新全工作区门禁；未运行 fuzz。初次编译失败已修正，不计入通过结果。

嵌套基空间补充：迭代解析 Indexed/I、Pattern、Separation、DeviceN 基空间，限制 max_depth/max_objects；Indexed 使用 Separation 基空间要求 PDF 1.3，依据 Adobe Reference 1.3 §4.5.5。`cargo test -p pdf --lib --test feature_versions --test color_workflow --locked --quiet` 退出 0：516 单元、15 版本、10 颜色流程测试通过，0 失败/忽略；日志 `/tmp/rzpdf-nested-colorspace-version.log`。新增 Indexed→ICCBased/Separation/DeviceN 版本组合及引用自循环错误后恢复保存回归。高版本仍跳过该定向预检；它不是颜色合法性/循环全面验证，也未覆盖 ICC Alternate、DeviceN 属性、全部默认替代空间或内容操作符。未重跑全工作区或 fuzz。

颜色空间版本补充：依据 [Adobe PDF Reference 1.3，Table 4.12](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.3.pdf)，ColorSpace 字段及命名资源表项中的 Pattern/Separation 要求 1.2，ICCBased/DeviceN 要求 1.3。`cargo test -p pdf --lib --test feature_versions --test color_workflow --locked --quiet` 退出 0：516 单元、14 版本、10 颜色流程测试通过，0 失败/忽略，日志 `/tmp/rzpdf-colorspace-version.log`。四家族、资源表/单值字段和直接/间接引用组合覆盖拒绝前输出/revision 不变及显式升级严格读回。用例采用版本识别字段而非完整有效颜色空间；不证明渲染能力、嵌套基空间/替代空间/操作符审计或最新全工作区门禁。

边界补充：`cargo test -p pdf --test feature_versions --locked --quiet` 终态退出 0，13 测试通过，0 失败/忽略；日志 `/tmp/rzpdf-function-version-boundaries.log`。间接 Catalog /Version 1.4 可满足函数最低版本，默认保存保留 1.2 Header，严格读回后再次保存成功，revision 不变。未知、负数、超 u64、实数 token（包括 f32 舍入为 2 的长小数）不会被定向检查误判成已知整数类型；直接和间接字段均覆盖。允许保留不等于函数合法性通过。本次仅新增两项回归，未重跑全工作区。

依据 [Adobe PDF Reference 1.3，§3.9](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.3.pdf)，Type0 引入于 PDF 1.2，Type2/3/4 引入于 PDF 1.3。保存预检在输出前检查已知 FunctionType，包括间接字段；未知类型不由该版本检查判定合法性。

验证命令：`cargo test -p pdf --lib --test feature_versions --test color_workflow --locked --quiet`。终态退出 0；516 单元、11 版本集成、10 颜色流程测试通过，0 失败/忽略。日志：`/tmp/rzpdf-function-version.log`。新增测试使用保留对象验证版本拒绝、原输出/revision 不变和显式升级后严格读回，不将这些对象视为合法可执行函数。不涵盖完整符合性、Type4 执行、预算峰值或最新全工作区门禁；未运行 fuzz。
