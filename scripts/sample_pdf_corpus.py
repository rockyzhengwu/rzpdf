#!/usr/bin/env python3
"""Sample new PDFs, interpret every page, and compare sampled renders with MuPDF.

Pixel differences are review signals, not a proof of incorrect rendering.
Reports and PNGs are written to a new output directory; inputs remain read-only.
"""
import argparse
import hashlib
import shutil
import collections
import concurrent.futures
import json
import math
import os
from pathlib import Path
import random
import re
import subprocess
import tempfile
import time

from PIL import Image, ImageChops, ImageStat
import check_pdf_corpus as parsing

REPO = Path(__file__).resolve().parents[1]


def run(command, timeout):
    start = time.monotonic()
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        try:
            process = subprocess.run(command, stdout=out, stderr=err, timeout=timeout, check=False)
            code = process.returncode
            status = "ok" if code == 0 else "crash" if code < 0 or code == 101 else "error"
        except subprocess.TimeoutExpired:
            code, status = None, "timeout"
        out.seek(0)
        err.seek(0)
        return dict(status=status, returncode=code, seconds=round(time.monotonic() - start, 3),
                    stdout=out.read(65536).decode("utf-8", "replace"),
                    stderr=err.read(65536).decode("utf-8", "replace"))


def rgb_on_white(path):
    with Image.open(path) as source:
        rgba = source.convert("RGBA")
    background = Image.new("RGBA", rgba.size, "white")
    return Image.alpha_composite(background, rgba).convert("RGB")


def compare(actual_path, reference_path):
    actual = rgb_on_white(actual_path)
    reference = rgb_on_white(reference_path)
    sizes = dict(actual_size=list(actual.size), reference_size=list(reference.size))
    dimension_delta = max(abs(a - b) for a, b in zip(actual.size, reference.size))
    # Backends round PDF point dimensions differently. Resample both onto a
    # modest common grid to reduce antialiasing and one-pixel rounding noise.
    grid = reference.copy()
    grid.thumbnail((512, 512), Image.Resampling.LANCZOS)
    actual = actual.resize(grid.size, Image.Resampling.LANCZOS)
    reference = reference.resize(grid.size, Image.Resampling.LANCZOS)
    diff = ImageChops.difference(actual, reference)
    mae = sum(ImageStat.Stat(diff).mean) / (3 * 255)
    gray_a, gray_b = actual.convert("L"), reference.convert("L")
    ha, hb = gray_a.histogram(), gray_b.histogram()
    pixels = math.prod(grid.size)
    ink_a, ink_b = sum(ha[:245]) / pixels, sum(hb[:245]) / pixels
    blank_mismatch = ink_a < 0.0005 and ink_b > 0.005
    ink_ratio = ink_a / ink_b if ink_b else None
    ink_mismatch = ink_b > 0.01 and (ink_ratio < 0.5 or ink_ratio > 2)
    review = dimension_delta > 2 or blank_mismatch or ink_mismatch or mae > 0.08
    return dict(**sizes, normalized_mae=round(mae, 5), ink_fraction=round(ink_a, 5),
                reference_ink_fraction=round(ink_b, 5), blank_mismatch=blank_mismatch,
                ink_ratio=round(ink_ratio, 5) if ink_ratio is not None else None,
                needs_visual_review=review)


def check_file(index, path, args):
    parsed = parsing.check(path, args)
    pages = parsed.get("pages")
    if pages is None:
        info = run(["pdfinfo", str(path)], args.render_timeout)
        match = re.search(r"^Pages:\s+(\d+)", info["stdout"], re.MULTILINE)
        if match:
            pages = int(match[1])
    folder = args.output / f"{index:03d}"
    folder.mkdir()
    renders = []
    if pages:
        selected = sorted({1, (pages + 1) // 2, pages})
        for page in selected:
            actual = folder / f"page-{page}-rzpdf.png"
            reference = folder / f"page-{page}-mupdf.png"
            rendered = run([str(args.render_binary), str(path), str(actual), str(page), args.mode, str(args.dpi)], args.render_timeout)
            baseline = run(["mutool", "draw", "-q", "-r", str(args.dpi), "-c", "rgb", "-o", str(reference), str(path), str(page)], args.render_timeout)
            row = dict(page=page, rzpdf=rendered, mupdf=baseline,
                       actual=str(actual.resolve()), reference=str(reference.resolve()))
            if rendered["status"] == "ok" and baseline["status"] == "ok":
                try:
                    row["comparison"] = compare(actual, reference)
                except (OSError, ValueError) as error:
                    row["comparison_error"] = str(error)
            renders.append(row)
    return dict(index=index, path=str(path), parsing=parsed, renders=renders,
                render_skipped_reason=None if renders else "Page count unavailable or zero")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, nargs="?", default=Path("/data2/zhengwu_workspace/dataset/pdfs"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, help="Repeat an exact prior sample")
    parser.add_argument("--count", type=int, default=30)
    parser.add_argument("--seed", type=int, default=20261021)
    parser.add_argument("--exclude", type=Path, action="append", default=[])
    parser.add_argument("--jobs", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--render-timeout", type=float, default=90)
    parser.add_argument("--dpi", type=float, default=48)
    parser.add_argument("--mode", choices=["strict", "compatible"], default="compatible")
    parser.add_argument("--binary", type=Path, default=REPO / "target/release/examples/check_pdf")
    parser.add_argument("--render-binary", type=Path, default=REPO / "target/release/examples/check_render")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    if any(value <= 0 or not math.isfinite(value) for value in (args.count, args.jobs, args.timeout, args.render_timeout, args.dpi)):
        parser.error("count, jobs, timeouts and DPI must be positive and finite")
    if not args.skip_build:
        subprocess.run(["cargo", "build", "--release", "-p", "pdf", "--example", "check_pdf", "-p", "render", "--example", "check_render"], cwd=REPO, check=True)
    excluded = set()
    for report in args.exclude:
        with report.open(encoding="utf-8") as source:
            excluded.update(json.loads(line)["path"] for line in source if line.strip())
    rng = random.Random(args.seed)
    chosen, candidates = [], 0
    def walk_error(error):
        raise error
    print(f"Scanning {args.root}; seed={args.seed}", flush=True)
    if not args.manifest:
        for base, dirs, names in os.walk(args.root, onerror=walk_error):
            dirs.sort()
            for name in sorted(names):
                path = Path(base) / name
                if not name.lower().endswith(".pdf") or str(path) in excluded:
                    continue
                candidates += 1
                if len(chosen) < args.count:
                    chosen.append(path)
                else:
                    slot = rng.randrange(candidates)
                    if slot < args.count:
                        chosen[slot] = path
    if args.manifest:
        chosen = [Path(json.loads(line)["path"]) for line in args.manifest.read_text().splitlines() if line.strip()]
        candidates = len(chosen)
    if not chosen:
        parser.error("No eligible PDFs")
    args.output.mkdir(parents=True, exist_ok=False)
    chosen.sort()
    # Pin executable bytes: a concurrent cargo build must not change the
    # implementation halfway through a long sample run.
    workers = args.output / "workers"
    workers.mkdir()
    fingerprints = {}
    for attribute in ("binary", "render_binary"):
        source = getattr(args, attribute)
        destination = (workers / source.name).resolve()
        shutil.copy2(source, destination)
        fingerprints[attribute] = hashlib.sha256(destination.read_bytes()).hexdigest()
        setattr(args, attribute, destination)
    (args.output / "manifest.jsonl").write_text("".join(json.dumps(dict(path=str(path)), ensure_ascii=False) + "\n" for path in chosen), encoding="utf-8")
    metadata = dict(worker_sha256=fingerprints, substitute_font=str(os.environ.get("RZPDF_SUBSTITUTE_FONT", "")), candidates=candidates, count=len(chosen), seed=args.seed if not args.manifest else None, source_manifest=str(args.manifest) if args.manifest else None, mode=args.mode,
                    dpi=args.dpi, render_pages="first, middle, last", excluded_reports=[str(p) for p in args.exclude])
    (args.output / "settings.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
    print(f"Selected {len(chosen)} of {candidates} eligible PDFs", flush=True)
    rows = []
    with (args.output / "results.jsonl").open("x", encoding="utf-8") as report:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
            futures = [pool.submit(check_file, index, path, args) for index, path in enumerate(chosen, 1)]
            for future in concurrent.futures.as_completed(futures):
                row = future.result()
                rows.append(row)
                report.write(json.dumps(row, ensure_ascii=False) + "\n")
                report.flush()
                successes = sum(r["rzpdf"]["status"] == "ok" for r in row["renders"])
                print(f"[{len(rows)}/{len(chosen)}] parse={row['parsing']['status']} render={successes}/{len(row['renders'])}: {row['path']}", flush=True)
    rendered = [r for row in rows for r in row["renders"]]
    summary = dict(**metadata,
        parsing_counts=dict(collections.Counter(row["parsing"]["status"] for row in rows)),
        reported_pages=sum(row["parsing"].get("pages", 0) for row in rows),
        completed_page_scans=sum("errors" in row["parsing"] for row in rows),
        pages_in_completed_scans=sum(row["parsing"].get("pages", 0) for row in rows if "errors" in row["parsing"]),
        page_errors=sum(row["parsing"].get("errors", 0) for row in rows),
        missing_unicode=sum(row["parsing"].get("missing_unicode", 0) for row in rows),
        render_counts=dict(collections.Counter(r["rzpdf"]["status"] for r in rendered)),
        reference_counts=dict(collections.Counter(r["mupdf"]["status"] for r in rendered)),
        comparison_pages=sum("comparison" in r for r in rendered),
        comparison_errors=sum("comparison_error" in r for r in rendered),
        missing_unicode_documents=sum(bool(row["parsing"].get("missing_unicode")) for row in rows),
        visual_review_pages=sum(r.get("comparison", {}).get("needs_visual_review", False) for r in rendered),
        report=str((args.output / "results.jsonl").resolve()))
    (args.output / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
