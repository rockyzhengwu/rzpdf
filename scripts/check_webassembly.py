#!/usr/bin/env python3
"""WA-01: inspect WebAssembly prerequisites and optionally check the PDF core.

This is not a linker, browser, or P0 completion gate. No tools are installed.
Python 3.11+ is required. Exit 0=passed, 1=failed, 2=blocked/setup error.
"""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib

REPO = Path(__file__).resolve().parents[1]
EMSCRIPTEN = "wasm32-unknown-emscripten"
UNKNOWN = "wasm32-unknown-unknown"


def run(command, timeout=15):
    try:
        result = subprocess.run(command, cwd=REPO, capture_output=True,
                                text=True, errors="replace", timeout=timeout, check=False)
        return dict(status="passed" if result.returncode == 0 else "failed",
                    returncode=result.returncode,
                    output=(result.stdout + result.stderr)[:8192])
    except subprocess.TimeoutExpired:
        return dict(status="failed", reason="timeout")
    except OSError as error:
        return dict(status="blocked", reason=str(error))


def inspect_tools(target):
    required = ["rustc", "cargo", "rustup", "node"]
    required += ["emcc", "em++"] if target == EMSCRIPTEN else ["clang"]
    tools = {}
    for name in required:
        path = shutil.which(name)
        if path is None:
            tools[name] = dict(status="blocked", reason="not on PATH")
        else:
            tools[name] = dict(path=path, **run([path, "--version"]))
    return tools


def source_inventory():
    cargo = tomllib.loads((REPO / "pdf/Cargo.toml").read_text())
    locked = tomllib.loads((REPO / "Cargo.lock").read_text())
    names = ["skia-safe", "skia-bindings", "freetype-rs", "freetype-sys",
             "mozjpeg", "mozjpeg-sys", "getrandom", "ring",
             "hayro-jbig2", "hayro-jpeg2000", "moxcms", "cc", "rc4", "aes", "cbc", "md-5"]
    packages = {name: sorted({p["version"] for p in locked["package"]
                             if p["name"] == name}) for name in names}
    root = tomllib.loads((REPO / "Cargo.toml").read_text())
    files = ["Cargo.toml", "Cargo.lock", "pdf/Cargo.toml", "pdf/build.rs",
             "pdf/src/filter/jbig2_native.rs", "pdf/src/font/font_program.rs",
             "pdf/vendor/jbig2dec/UPSTREAM.json", "render/Cargo.toml",
             "font-data/Cargo.toml", "font-data/build.rs", "font-data/src/lib.rs", "docs/webassembly/spec.md",
             "docs/webassembly/plan.md", "docs/webassembly/toolchain.json",
             "scripts/build_webassembly.py", "render/examples/wasm_build_probe.rs",
             "scripts/check_webassembly.py"]
    optional = ["web/tests/outline-recovery.mjs", "web/tests/large-input.mjs", "pdf/src/derived_cache.rs", "pdf/src/document/prepared_page.rs", "pdf/src/document/recovery.rs", "pdf/tests/prepared_page.rs", "pdf/src/page/display_list.rs", "pdf/src/font/cmap/mod.rs", "pdf/src/font/afm.rs", "pdf/src/font/cid_font.rs", "pdf/src/font/descriptor.rs", "pdf/src/font/encoding.rs", "pdf/src/font/simple_font.rs", "pdf/src/font/type0.rs", "web/tests/render-cache.mjs", "web/reader.css", "web/src/reader-shell.ts", "web/tests/reader-shell.mjs", "web/tests/assets-browser.mjs", "scripts/package_web_assets.mjs", "web/tests/package-assets.mjs", "web/tests/viewport-parity.mjs", "web/tests/viewport-browser.mjs", "web/src/viewport-raster.ts", "web/tests/viewport-raster.mjs", "web/tests/viewer.mjs", "pdf/src/page/annotations.rs", "render/src/annotation_tests.rs", "render/src/text_clip_tests.rs", "render/src/knockout.rs", "render/src/raster_path.rs", "render/src/region_tests.rs", "render/src/group_tests.rs", "web/src/raster.ts", "web/tests/regions.mjs", "scripts/prepare_web_region_fixtures.py", "scripts/prepare_web_font_matrix.py", "web/tests/font-matrix.mjs", ".github/workflows/webassembly.yml", "web/tests/language.mjs", "pdf/tests/font_language.rs", "pdf/src/page/text_object.rs", "pdf/src/device/trace.rs", "pdf/src/page/content_interpreter/pattern.rs", "web-wasm/examples/language_fixtures.rs", "web/src/font-resources.ts", "web/src/font-controls.ts", "web/tests/font-resources.mjs", "web/tests/font-resources-browser.mjs", "render/src/font_preflight.rs", "render/src/soft_mask.rs", "web/tests/font-service.mjs", "pdf/src/font/resolver.rs", "pdf/src/font/mod.rs", "pdf/src/font/pdf_font.rs", "pdf/src/font/cid_font.rs", "render/src/font_provider.rs", "render/src/lib.rs", "web-wasm/src/fonts.rs", "web/src/fonts.ts", "pdf/src/document/optional_content_edit.rs", "pdf/tests/v013_optional_content_edit.rs", "web-wasm/examples/font_write_fixtures.rs", "web/tests/font-write.mjs", "pdf/src/document/embedded_font.rs", "pdf/tests/v03_font_edit.rs", "web/tests/import.mjs", "web-wasm/examples/content_fixtures.rs", "web/tests/content.mjs", "pdf/src/document/content_edit.rs", "pdf/src/document/nested_edit.rs", "pdf/src/document/text_event_edit.rs", "pdf/src/document/subset_text_edit.rs", "pdf/tests/content_edit_transaction.rs", "web/src/content-editor.ts", "web-wasm/src/content.rs", "web/src/geometry.ts", "web/tests/geometry.mjs", "web/src/editor.ts", "web/src/interaction.ts", "web-wasm/src/interaction.rs", "web-wasm/examples/interaction_fixtures.rs", "pdf/src/document/acroform/model.rs", "pdf/src/document/acroform/types.rs", "pdf/src/document/owned.rs", "web/tests/interaction.mjs", "web/tests/fixtures/trap-worker.mjs", "web/src/worker-client.ts", "web/src/protocol.ts", "web/src/pdf-worker.ts", "web/tests/worker.mjs", "web/src/text.ts", "web/tests/text.mjs", "web-wasm/src/api.rs", "pdf/src/page/text_extraction.rs", "pdf/src/page/diagnostic.rs", "pdf/tests/form_diagnostics.rs", "pdf/tests/v010_optional_content.rs", "pdf/src/document/optional_content.rs", "pdf/tests/image_extraction.rs", "pdf/tests/v010_marked_text.rs",
                "pdf/src/color/shading_patch.rs", "pdf/src/color/shading_patch_raster.rs",
                "render/examples/check_render.rs", "pdf/src/parser/syntax.rs", "pdf/src/filter/dct.rs", "pdf/src/device/gray_group.rs", "pdf/src/page/transparency.rs",
                "pdf/src/filter/ccittfax/mod.rs", "pdf/src/page/inline_image.rs",
                "pdf/src/page/content_interpreter/soft_mask.rs", "pdf/tests/v07_type3_model.rs",
                "pdf/src/font/type0.rs", "pdf/src/color/pattern/tiling.rs",
                "pdf/tests/fixtures/v06/gray-indices-2bit.jp2", "pdf/tests/fixtures/v06/generate_indexed.c",
                "pdf/src/color/shading.rs", "pdf/tests/fixtures/v06/manifest.json",
                "pdf/tests/fixtures/v06/gray-alpha.jp2", "pdf/tests/fixtures/v06/rgb-alpha.jp2",
                "pdf/tests/fixtures/v06/cmyk-lossless.jp2", "pdf/tests/fixtures/v06/generate_channels.py",
                "pdf/src/filter/jpx.rs", "pdf/src/function/postscript.rs",
                "pdf/src/page/pdf_image.rs", "pdf/src/page/text_object.rs",
                "pdf/src/filter/mod.rs", "pdf/src/font/simple_font.rs",
                "pdf/src/objects/pdf_stream.rs", "pdf/src/parser/parser.rs",
                "pdf/src/parser/diagnostic.rs", "pdf/src/document/page_tree.rs",
                "pdf/tests/v06_codecs.rs", "render/src/pattern_tests.rs", "render/src/soft_mask_tests.rs",
                "pdf/src/page/all_state.rs", "pdf/src/page/content_interpreter.rs",
                "pdf/src/page/general_state.rs", "pdf/src/security/r4.rs", "pdf/src/security/r6.rs",
                "pdf/src/security/transform.rs", "pdf/src/security/mod.rs", "pdf/src/pdf_context.rs",
                "pdf/src/writer.rs", "pdf/src/writer/incremental/security.rs", "pdf/src/document/mod.rs",
                "pdf/tests/r4_security.rs", "pdf/src/page/content_interpreter/marked_text.rs",
                "pdf/tests/fixtures/r4/manifest.json", "pdf/tests/fixtures/r4/empty-user.pdf",
                "pdf/tests/fixtures/r4/password-user.pdf",
                "render/src/render_device.rs", "web-wasm/Cargo.toml", "web-wasm/src/lib.rs",
                "web-wasm/src/main.rs", "web/package.json", "web/package-lock.json",
                "web/tsconfig.json", "web/index.html", "web/src/client.ts", "web/src/demo.ts",
                "web/tests/browser.mjs", "render/examples/rgba_reference.rs",
                "scripts/prepare_webassembly_fixtures.py"]
    optional.extend([
        "web-wasm/src/resources.rs", "pdf/src/pdf_context.rs", "pdf/src/error.rs",
        "scripts/test_build_webassembly.py",
        "pdf/src/document/mod.rs", "pdf/examples/resource_scan.rs",
        "pdf/tests/render_image_resolution.rs", "web/tests/resources.mjs",
        "web/tests/resources-browser.mjs", "docs/webassembly/resources.md",
        "docs/webassembly/resource-plan.md",
    ])
    optional.extend([
        "pdf/src/document/annotations.rs", "pdf/src/document/annotations/model.rs",
        "pdf/src/document/annotations/edit.rs", "pdf/src/document/annotations/paint.rs",
        "pdf/src/document/annotations/unicode_freetext.rs", "pdf/src/document/embedded_font.rs",
        "pdf/src/document/appearance.rs", "pdf/src/font/cid_encoding.rs",
        "pdf/examples/unicode_freetext.rs", "pdf/tests/unicode_freetext.rs",
        "pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf", "docs/licenses/NotoSansSC-OFL.txt",
        "web-wasm/src/interaction.rs", "web-wasm/src/content.rs", "web/src/interaction.ts",
        "web/tests/unicode-freetext-protocol.mjs", "web/tests/unicode-freetext-browser.mjs",
        "docs/check-unicode-freetext.py", "docs/unicode-freetext.md",
    ])
    optional.extend([
        "pdf/src/io/source_stream.rs", "pdf/src/io/stream_reader.rs", "pdf/src/io/mod.rs",
        "pdf/src/signatures/changes.rs",
        "pdf/src/parser/syntax.rs", "pdf/src/parser/parser.rs", "pdf/src/objects/pdf_stream.rs",
        "pdf/src/security/transform.rs", "pdf/src/writer.rs", "pdf/src/writer/incremental.rs",
        "pdf/tests/source_streams.rs", "scripts/source-stream-fixture.mjs",
        "web/tests/source-streams-browser.mjs", "docs/source-streams.md",
    ])
    optional.extend([
        "pdf/src/filter/flate_image.rs", "pdf/src/filter/predictor.rs", "pdf/src/page/image_decode.rs", "pdf/src/page/mod.rs",
        "pdf/src/budget.rs", "pdf/tests/image_preview.rs", "scripts/image-preview-fixtures.py",
        "web/tests/image-preview-browser.mjs", "docs/image-preview.md", "docs/webassembly/api.md",
        "pdf/tests/fixtures/image-preview/manifest.json", "pdf/tests/fixtures/image-preview/README.md",
        "pdf/tests/fixtures/image-preview/preview-flate-corrupt.pdf",
        "pdf/tests/fixtures/image-preview/preview-flate.pdf",
        "pdf/tests/fixtures/image-preview/preview-jpeg.pdf",
        "pdf/tests/fixtures/image-preview/preview-progressive.pdf",
    ])
    files.extend(name for name in optional if (REPO / name).exists())
    files.extend(str(path.relative_to(REPO)) for directory in ["fonts", "afm", "cmaps", "licenses"]
                 for path in sorted((REPO / "font-data" / directory).iterdir()) if path.is_file())
    files.extend(["font-data/NOTICE.md", "font-data/cmap-provenance.json"])
    hashes = {name: hashlib.sha256((REPO / name).read_bytes()).hexdigest()
              for name in files}
    git = shutil.which("git")
    revision = run([git, "rev-parse", "HEAD"]) if git else dict(status="blocked")
    dirty = run([git, "diff", "HEAD", "--name-only"]) if git else dict(status="blocked")
    return dict(workspace_members=root["workspace"]["members"],
                source_revision=revision.get("output", "").strip(),
                tracked_changes=bool(dirty.get("output")) if dirty["status"] == "passed" else None,
                source_sha256=hashes, locked_dependencies=packages,
                pdf_direct_dependencies=sorted(cargo["dependencies"]),
                jbig2dec=json.loads((REPO / "pdf/vendor/jbig2dec/UPSTREAM.json").read_text()),
                planned_web_crate_exists=(REPO / "web-wasm/Cargo.toml").is_file())


def prerequisite_blockers(tools, target_probe, target):
    blockers = [f"{name}: {info['status']}" for name, info in tools.items()
                if info["status"] != "passed"]
    if target_probe["status"] != "passed":
        blockers.append("cannot enumerate installed Rust targets")
    elif target not in target_probe.get("output", "").splitlines():
        blockers.append(f"Rust target not installed: {target}")
    return blockers


def build_check(target, tools, blockers, output, timeout):
    if blockers:
        return dict(status="blocked", reason="prerequisites unavailable", blockers=blockers)
    command = [tools["cargo"]["path"], "check", "-p", "pdf", "--lib",
               "--locked", "--offline", "--target", target]
    log_path = output.with_suffix(".build.log")
    log_path.parent.mkdir(parents=True, exist_ok=True)
    start = time.monotonic()
    try:
        with log_path.open("wb") as log:
            result = subprocess.run(command, cwd=REPO, stdout=log,
                                    stderr=subprocess.STDOUT, timeout=timeout, check=False)
        status = "passed" if result.returncode == 0 else "failed"
        result_info = dict(status=status, returncode=result.returncode)
    except subprocess.TimeoutExpired:
        result_info = dict(status="failed", reason="timeout")
    except OSError as error:
        result_info = dict(status="blocked", reason=str(error))
    with log_path.open("rb") as log:
        log.seek(0, os.SEEK_END)
        size = log.tell()
        log.seek(max(0, size - 8192))
        tail = log.read().decode("utf-8", errors="replace")
    return dict(**result_info, command=command, log=str(log_path), output_tail=tail,
                seconds=round(time.monotonic() - start, 3))


def write_report(path, report):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix=path.name + ".", suffix=".tmp", delete=False) as out:
            temporary = Path(out.name)
            json.dump(report, out, ensure_ascii=False, indent=2)
            out.write("\n")
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", choices=[EMSCRIPTEN, UNKNOWN], default=EMSCRIPTEN)
    parser.add_argument("--build-check", action="store_true", help="Run locked/offline core cargo check")
    parser.add_argument("--timeout", type=float, default=180)
    args = parser.parse_args(argv)
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("timeout must be positive and finite")
    tools = inspect_tools(args.target)
    rustup = tools["rustup"].get("path")
    target_probe = run([rustup, "target", "list", "--installed"]) if rustup else dict(status="blocked")
    blockers = prerequisite_blockers(tools, target_probe, args.target)
    output = args.output.resolve()
    build = (build_check(args.target, tools, blockers, output, args.timeout)
             if args.build_check else dict(status="not_run"))
    status = ("blocked" if blockers or build["status"] == "blocked" else
              "failed" if build["status"] == "failed" else "passed")
    report = dict(schema_version=1, task="WA-01", acceptance_case="WA-T01",
                  generated_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  target=args.target, route="primary" if args.target == EMSCRIPTEN else "alternative_diagnostic",
                  status=status, p0_complete=False, prerequisites_ready=not blockers,
                  blockers=blockers, tools=tools, installed_targets=target_probe,
                  source=source_inventory(), core_check=build,
                  pending_gates=["WA-T02: final artifact link and real browser rendering",
                                 "WA-T03: font, image codec and transparency runtime probes"],
                  notes=["No software is installed or activated by this script.",
                         "A passed preflight/core check is not browser or P0 acceptance.",
                         "The alternative target never counts as a primary-route pass."])
    write_report(output, report)
    print(json.dumps(dict(status=status, target=args.target, p0_complete=False,
                          blockers=blockers, core_check=build["status"], report=str(output)),
                     ensure_ascii=False))
    return {"passed": 0, "failed": 1, "blocked": 2}[status]


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, tomllib.TOMLDecodeError) as error:
        print(f"WebAssembly preflight setup error: {error}", file=sys.stderr)
        sys.exit(2)
