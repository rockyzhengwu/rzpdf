#!/usr/bin/env python3
"""Recursively check PDF page interpretation; JSONL contains one result per file.

Exit codes: 0 = all clean, 1 = failures/warnings, 2 = runner/setup error.
This is a compatibility check, not a visual or semantic correctness oracle.
"""
import argparse
import collections
import concurrent.futures
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[1]


def check(path, args):
    start = time.monotonic()
    # Disk-backed logs prevent a noisy or panicking parser from filling RAM.
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        try:
            process = subprocess.run(
                getattr(args, "command_prefix", []) + [str(args.binary), str(path), args.mode], stdout=out, stderr=err,
                timeout=args.timeout, check=False,
            )
            code = process.returncode
            status = {0: "ok", 1: "error", 2: "warning"}.get(code, "crash")
        except subprocess.TimeoutExpired:
            code, status = None, "timeout"
        out.seek(0)
        err.seek(0)
        output = out.read(65536).decode("utf-8", errors="replace")
        details = err.read(65536).decode("utf-8", errors="replace")
    metrics = {}
    for line in output.splitlines():
        key, sep, value = line.partition("=")
        if sep and value.isdigit():
            metrics[key] = int(value)
    return dict(path=str(path), status=status, returncode=code,
                seconds=round(time.monotonic() - start, 3), mode=args.mode,
                **metrics, details=details, output=output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, nargs="?", default=Path("/data2/zhengwu_workspace/dataset/pdfs"))
    parser.add_argument("--output", type=Path, default=Path("corpus-results.jsonl"))
    parser.add_argument("--mode", choices=["strict", "compatible"], default="strict")
    parser.add_argument("--manifest", type=Path, help="Recheck the paths in an earlier JSONL report")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--limit", type=int, help="Only check the first N discovered files")
    parser.add_argument("--binary", type=Path, default=REPO / "target/release/examples/check_pdf")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    if args.jobs < 1 or args.timeout <= 0 or (args.limit is not None and args.limit < 1):
        parser.error("jobs, timeout and limit must be positive")
    if args.manifest is None and not args.root.is_dir():
        parser.error(f"Not a directory: {args.root}")
    args.binary = args.binary.resolve()
    if not args.skip_build:
        subprocess.run(["cargo", "build", "--release", "-p", "pdf", "--example", "check_pdf"], cwd=REPO, check=True)
    def walk_error(error):
        raise error
    paths = []
    if args.manifest:
        with args.manifest.open(encoding="utf-8") as manifest:
            paths = list(dict.fromkeys(Path(json.loads(line)["path"]) for line in manifest if line.strip()))
        if args.limit:
            paths = paths[:args.limit]
    else:
        print(f"Scanning {args.root} ...", flush=True)
        for base, dirs, names in os.walk(args.root, onerror=walk_error):
            dirs.sort()
            for name in sorted(names):
                if name.lower().endswith(".pdf"):
                    paths.append(Path(base) / name)
                    if args.limit and len(paths) >= args.limit:
                        break
            if args.limit and len(paths) >= args.limit:
                break
    if not paths:
        parser.error("No PDFs found")
    counts = collections.Counter()
    print(f"Checking {len(paths)} PDFs; mode={args.mode}", flush=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    # Exclusive creation protects reports from accidental replacement.
    with args.output.open("x", encoding="utf-8") as report:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
            pending = iter(paths)
            futures = {pool.submit(check, path, args) for path in [next(pending, None) for _ in range(args.jobs)] if path is not None}
            while futures:
                done, futures = concurrent.futures.wait(futures, return_when=concurrent.futures.FIRST_COMPLETED)
                for future in done:
                    result = future.result()
                    counts[result["status"]] += 1
                    report.write(json.dumps(result, ensure_ascii=False) + "\n")
                    report.flush()
                    n = sum(counts.values())
                    if result["status"] != "ok" or n % 10 == 0:
                        print(f"[{n}/{len(paths)}] {result['status']}: {result['path']}", flush=True)
                    path = next(pending, None)
                    if path is not None:
                        futures.add(pool.submit(check, path, args))
    summary = dict(total=len(paths), counts=dict(counts), report=str(args.output.resolve()))
    args.output.with_suffix(".summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False), flush=True)
    return 0 if counts["ok"] == len(paths) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"Runner error: {error}", file=sys.stderr)
        sys.exit(2)
