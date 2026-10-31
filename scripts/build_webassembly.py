#!/usr/bin/env python3
"""WA-02: build with the pinned Emscripten toolchain; never install tools.

Exit 0=build passed, 1=build failed, 2=toolchain/setup blocked.
This does not perform browser acceptance or claim P0 completion.
"""
import argparse
import datetime
import hashlib
import json
import os
import platform
from pathlib import Path
import subprocess
import sys
import time

from check_webassembly import EMSCRIPTEN, REPO, source_inventory, write_report


def capture(command, env=None):
    return subprocess.check_output(command, cwd=REPO, env=env, text=True,
                                   stderr=subprocess.STDOUT).strip()


def capture_json(command, env=None):
    """Keep tool diagnostics out of machine-readable stdout; preserve warnings."""
    result = subprocess.run(command, cwd=REPO, env=env, text=True,
                            capture_output=True, check=True)
    if result.stderr:
        print(result.stderr, file=sys.stderr, end="")
    return json.loads(result.stdout)


def matches_emcc_version(output, version):
    # First invocation may emit SDK sanity-check diagnostics before the version.
    return any(line.startswith("emcc ") and f" {version} " in line
               for line in output.splitlines())


def sdk_environment(sdk):
    # Pass paths as arguments, never interpolate them into shell source.
    result = subprocess.check_output(
        ["bash", "-c", 'source "$1/emsdk_env.sh" >/dev/null 2>&1 && env -0',
         "rzpdf-sdk", str(sdk)], cwd=REPO)
    return dict(entry.decode().split("=", 1) for entry in result.split(b"\0") if entry)


def archive_inventory(root, env, profile="debug"):
    """Check a member of every native archive, recording precisely that scope."""
    result = []
    for path in sorted(root.glob(f"{EMSCRIPTEN}/{profile}/build/**/*.a")):
        members = capture(["emar", "t", str(path)], env).splitlines()
        member = next((name for name in members if name.endswith(".o")), None)
        if member is None:
            raise ValueError(f"no object member in {path}")
        data = subprocess.check_output(["emar", "p", str(path), member], env=env)
        if not data.startswith(b"\0asm"):
            raise ValueError(f"non-Wasm object sampled: {path}: {member}")
        result.append(dict(path=str(path.relative_to(root)), sampled_member=member,
                           object_format="wasm", members=len(members),
                           sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    if not result:
        raise ValueError("no native archives found")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--emsdk", type=Path,
                        default=Path(os.environ.get("EMSDK", REPO / "output/webassembly/toolchains/emsdk")))
    parser.add_argument("--toolchain", default="nightly-2026-08-26",
                        help="installed toolchain name; its exact rustc commit is checked")
    parser.add_argument("--package", choices=["pdf", "render", "web-wasm"], default="render")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--profile", choices=["dev", "wasm-release"], default="wasm-release",
                        help="optimized browser build by default; dev is for diagnostics")
    parser.add_argument("--run", action="store_true", help="run the linked example under Node")
    parser.add_argument("--example", help="link a named example instead of building the library")
    args = parser.parse_args()
    if args.run and not args.example:
        parser.error("--run requires --example")
    if args.package == "web-wasm" and (args.run or args.example):
        parser.error("web-wasm builds its browser binary; --run/--example are probe-only")
    output = args.output.resolve()
    artifact_profile = "debug" if args.profile == "dev" else args.profile
    lock = json.loads((REPO / "docs/webassembly/toolchain.json").read_text())
    report = dict(task="WA-03" if args.package == "web-wasm" else "WA-02", schema_version=1, target=EMSCRIPTEN,
                  status="blocked", p0_complete=False, browser="not_run",
                  generated_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  toolchain_lock=lock, platform=platform.platform())
    try:
        env = sdk_environment(args.emsdk.resolve())
        rust = capture(["rustup", "run", args.toolchain, "rustc", "-Vv"], env)
        if f"commit-hash: {lock['rust_commit']}" not in rust:
            raise ValueError("rustc commit differs from toolchain.json")
        emcc = capture(["emcc", "--version"], env)
        if not matches_emcc_version(emcc, lock["emscripten_version"]):
            raise ValueError("emcc version differs from toolchain.json")
        sdk_revision = capture(["git", "-C", str(args.emsdk.resolve()), "rev-parse", "HEAD"], env)
        if sdk_revision != lock["emsdk_commit"]:
            raise ValueError("emsdk commit differs from toolchain.json")
        node = capture(["node", "--version"], env)
        sdk_node = capture([env["EMSDK_NODE"], "--version"], env)
        if node != lock["runtime_node_version"] or sdk_node != lock["emsdk_node_version"]:
            raise ValueError("Node version differs from toolchain.json")
        # Override host CC/CXX only for this target. No pkg-config host opt-in.
        env.update(CC_wasm32_unknown_emscripten="emcc",
                   CXX_wasm32_unknown_emscripten="em++",
                   AR_wasm32_unknown_emscripten="emar",
                   CFLAGS_wasm32_unknown_emscripten=lock["cflags"],
                   CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_RUSTFLAGS=lock["rustflags"],
                   CARGO_TARGET_DIR=str(REPO / "output/webassembly/target"))
        if args.profile == "wasm-release":
            env["CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_RUSTFLAGS"] += " -Clink-arg=-O2"
        if args.package == "web-wasm":
            env["CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_RUSTFLAGS"] += " " + " ".join(
                "-Clink-arg=" + value for value in lock["web_link_args"])
        # Reject hidden global flags that could weaken linkage or change the ABI.
        for name in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "EMCC_CFLAGS", "PKG_CONFIG_ALLOW_CROSS",
                     "SKIA_SOURCE_DIR", "SKIA_LIBRARY_SEARCH_PATH", "SKIA_USE_SYSTEM_LIBRARIES",
                     "FORCE_SKIA_BINARIES_DOWNLOAD"]:
            if env.get(name):
                raise ValueError(f"unset {name} before running the pinned build")
        metadata = capture_json(["rustup", "run", args.toolchain, "cargo", "metadata",
                                 "--locked", "--format-version", "1", "--filter-platform", EMSCRIPTEN], env)
        skia = next(p for p in metadata["packages"] if p["name"] == "skia-bindings")
        if skia["metadata"]["skia"] != lock["skia_source"]:
            raise ValueError("Skia source revision differs from toolchain.json")
        ft = next(p for p in metadata["packages"] if p["name"] == "freetype-sys")
        ft_include = str(Path(ft["manifest_path"]).parent / "freetype2/include")
        env["FORCE_SKIA_BUILD"] = "1"
        env["SKIA_DEBUG"] = "0"
        env["SKIA_GN_ARGS"] = ("skia_use_system_freetype2=true "
                               "skia_system_freetype2_include_path=" + json.dumps(ft_include))
        report["skia_gn_args"] = env["SKIA_GN_ARGS"]
        command = ["rustup", "run", args.toolchain, "cargo", "--config",
                   f"profile.{args.profile}.package.skia-bindings.opt-level=" + str(lock["skia_opt_level"]), "build",
                   "-Zbuild-std=std,panic_abort", "--locked", "--target", EMSCRIPTEN,
                   "-p", args.package, "--profile", args.profile]
        command += (["--example", args.example] if args.example else
                    ["--bin", "rzpdf_web"] if args.package == "web-wasm" else ["--lib"])
        log_path = output.with_suffix(".build.log")
        log_path.parent.mkdir(parents=True, exist_ok=True)
        inventory = source_inventory()
        if inventory["locked_dependencies"]["skia-bindings"] != [lock["skia_bindings"]]:
            raise ValueError("locked Skia version differs from toolchain.json")
        report.update(source=inventory, rustc=rust, emcc=emcc,
                      node=node, emsdk_node=sdk_node,
                      command=command, profile=args.profile, log=str(log_path))
        start = time.monotonic()
        with log_path.open("wb") as log:
            code = subprocess.call(command, cwd=REPO, env=env, stdout=log, stderr=subprocess.STDOUT)
        report.update(status="passed" if code == 0 else "failed", returncode=code,
                      seconds=round(time.monotonic() - start, 3))
        if code == 0:
            # Do not publish an artifact with a source manifest captured before
            # concurrent edits. Include newly added modules, not just old paths.
            final_hashes = source_inventory()["source_sha256"]
            original_hashes = inventory["source_sha256"]
            changed = sorted(name for name in original_hashes.keys() | final_hashes.keys()
                             if original_hashes.get(name) != final_hashes.get(name))
            if changed:
                report["changed_sources_during_build"] = changed
                raise ValueError("source changed during build; rebuild from stable inputs")
            if args.package in ["render", "web-wasm"]:
                skia_root = Path(skia["manifest_path"]).parent / "skia"
                deps_hash = hashlib.sha256((skia_root / "DEPS").read_bytes()).hexdigest()
                if deps_hash != lock["skia_deps_sha256"]:
                    raise ValueError("Skia DEPS differs from toolchain.json")
                report["skia_deps_sha256"] = deps_hash
            report["native_archive_scope"] = "one object member from each cached target native archive"
            report["native_archives"] = archive_inventory(Path(env["CARGO_TARGET_DIR"]), env, artifact_profile)
            if args.package == "web-wasm":
                import shutil
                destination = REPO / "web/dist"
                destination.mkdir(parents=True, exist_ok=True)
                artifact_root = Path(env["CARGO_TARGET_DIR"]) / EMSCRIPTEN / artifact_profile
                report["artifacts"] = {}
                for ext in ["js", "wasm"]:
                    source = artifact_root / ("rzpdf_web." + ext)
                    target = destination / ("rzpdf_web." + ("mjs" if ext == "js" else ext))
                    shutil.copyfile(source, target)
                    report["artifacts"][str(target)] = hashlib.sha256(target.read_bytes()).hexdigest()
            if args.example:
                artifact = Path(env["CARGO_TARGET_DIR"]) / EMSCRIPTEN / artifact_profile / "examples" / (args.example + ".js")
                report["artifacts"] = {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                                       for p in [artifact, artifact.with_suffix(".wasm")]}
                if args.run:
                    runtime = subprocess.run(["node", str(artifact)], cwd=REPO, env=env,
                                             capture_output=True, text=True, timeout=60)
                    report["runtime"] = dict(returncode=runtime.returncode,
                                             stdout=runtime.stdout[-8192:], stderr=runtime.stderr[-8192:])
                    if runtime.returncode != 0:
                        report["status"] = "failed"
    except (OSError, ValueError, KeyError, StopIteration, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        report.update(status="failed" if report.get("returncode") == 0 else "blocked",
                      error=str(error))
    write_report(output, report)
    print(json.dumps(dict(status=report["status"], report=str(output), p0_complete=False)))
    return {"passed": 0, "failed": 1, "blocked": 2}[report["status"]]


if __name__ == "__main__":
    sys.exit(main())
