#!/usr/bin/env python3
"""Serial, quality-gated rzpdf / MuPDF / PDFium native API benchmark."""
import argparse
from datetime import datetime, timezone
import hashlib
import itertools
import json
import math
from pathlib import Path
import platform
import random
import re
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
ENGINES = ("rzpdf", "mupdf", "pdfium")


def percentile95(values):
    # Do not present tail estimates from a handful of observations.
    return sorted(values)[math.ceil(len(values) * 0.95) - 1] if len(values) >= 20 else None


def quality(left, right, nonblank):
    from PIL import Image, ImageChops, ImageFilter, ImageStat
    if left["width"] != right["width"] or left["height"] != right["height"]:
        return dict(passed=False, reason="dimensions")
    size = left["width"], left["height"]
    rgba = [Image.frombytes("RGBA", size, Path(v["file"]).read_bytes()) for v in (left, right)]
    if any(image.getchannel("A").getextrema() != (255, 255) for image in rgba):
        return dict(passed=False, reason="expected opaque white substrate")
    a, b = [image.convert("RGB") for image in rgba]
    mae = sum(ImageStat.Stat(ImageChops.difference(a, b)).mean) / (3 * 255)
    masks = [im.convert("L").point(lambda x: 255 if x < 245 else 0) for im in (a, b)]
    counts = [sum(mask.histogram()[1:]) for mask in masks]
    if not all(counts):
        passed = not nonblank and counts == [0, 0]
        return dict(passed=passed, mae=mae, ink=counts, reason="blank")
    overlap = []
    for i in range(2):
        match = ImageChops.multiply(masks[i], masks[1-i].filter(ImageFilter.MaxFilter(3)))
        overlap.append(sum(match.histogram()[1:]) / counts[i])
    f1 = 2 * overlap[0] * overlap[1] / sum(overlap) if sum(overlap) else 0
    ratio = counts[0] / counts[1]
    return dict(passed=mae <= 0.03 and f1 >= 0.97 and 0.75 <= ratio <= 1.3334,
                mae=mae, ink_f1=f1, ink_ratio=ratio)


def invoke(engine, job, timeout, stderr_path):
    command = ([str(ROOT / "target/release/examples/benchmark_render")] if engine == "rzpdf"
               else [sys.executable, str(ROOT / "scripts/benchmark_pdf_worker.py"), engine])
    # The OS peak includes adapter/runtime overhead; this is not engine-only allocation.
    timed = Path("/usr/bin/time").exists()
    if timed:
        command = ["/usr/bin/time", "-l" if sys.platform == "darwin" else "-v", *command]
    start = time.perf_counter()
    try:
        # A new process group lets timeout terminate the wrapper and its worker.
        import os
        import signal
        with subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, text=True, start_new_session=True) as proc:
            try:
                stdout, stderr = proc.communicate(json.dumps(job), timeout=timeout)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, signal.SIGKILL)
                stdout, stderr = proc.communicate()
                stderr_path.write_text(stderr)
                return dict(status="timeout", error=f"exceeded {timeout}s")
        stderr_path.write_text(stderr)
        if proc.returncode:
            return dict(status="crash", returncode=proc.returncode, error=stderr[-2000:])
        result = json.loads(stdout)
        result["process_wall_ms"] = (time.perf_counter() - start) * 1000
        pattern = (r"(\d+)\s+maximum resident set size" if sys.platform == "darwin"
                   else r"Maximum resident set size \(kbytes\):\s*(\d+)")
        rss = re.search(pattern, stderr)
        result["worker_peak_rss_bytes"] = int(rss[1]) * (1 if sys.platform == "darwin" else 1024) if rss else None
        return result
    except (OSError, ValueError) as exc:
        return dict(status="error", error=str(exc))


def summarize(records, gates):
    rows = []
    keys = sorted({(r["case"], r["dpi"]) for r in records})
    for case, dpi in keys:
        for engine in ENGINES:
            runs = [r for r in records if (r["case"], r["dpi"], r["engine"]) == (case, dpi, engine)]
            good = [r for r in runs if r["status"] == "ok"]
            if not good:
                continue
            warm = [v for r in good for v in r["warm_ms"]]
            next_pages = [v for r in good for v in r["next_ms"]]
            rss = [r["worker_peak_rss_bytes"] for r in good if r.get("worker_peak_rss_bytes")]
            diagnostics = [r for r in good if r.get("diagnostics")]
            def memory_median(stage):
                values = [r["diagnostics"][stage]["rss_bytes"] for r in diagnostics
                          if r["diagnostics"][stage].get("rss_bytes") is not None]
                return statistics.median(values) / 2**20 if values else None
            def cpu_ratio(cpu, wall):
                values = [r["diagnostics"][cpu] / (r["first_ms"] if wall == "first_ms" else r["diagnostics"][wall])
                          for r in diagnostics if r["diagnostics"].get(cpu) is not None
                          and (r["first_ms"] if wall == "first_ms" else r["diagnostics"][wall]) > 0]
                return statistics.median(values) if values else None
            rows.append(dict(case=case, dpi=dpi, engine=engine,
                             baseline=all(r.get("baseline", False) for r in runs),
                             eligible=gates.get((case, dpi), False) and len(good) == len(runs),
                             trials=len(good), first_ms=statistics.median(r["first_ms"] for r in good),
                             warm_ms=statistics.median(warm), warm_p95_ms=percentile95(warm),
                             next_ms=statistics.median(next_pages) if next_pages else None,
                             peak_rss_mib=max(rss) / 2**20 if rss else None,
                             initialized_rss_mib=memory_median("initialized_memory"),
                             after_first_rss_mib=memory_median("after_first_memory"),
                             after_replay_rss_mib=memory_median("after_replay_memory"),
                             first_cpu_wall_ratio=cpu_ratio("first_cpu_ms", "first_ms"),
                             replay_cpu_wall_ratio=cpu_ratio("replay_cpu_ms", "replay_wall_ms")))
    return rows


def report(rows, gates, directory):
    lines = ["# Native PDF rendering benchmark", "",
             "Times are milliseconds; RSS is whole worker process peak (MiB). Lower is better.",
             "Quality gates are diagnostic agreement checks, not PDF conformance certification.", "",
             "| Case | DPI | Engine | Eligible | First page | Warm replay | Next page | Peak RSS |",
             "|---|---:|---|---|---:|---:|---:|---:|"]
    for r in rows:
        fmt = lambda value: f"{value:.2f}" if value is not None else "—"
        lines.append(f"| {r['case']} | {r['dpi']} | {r['engine']} | {r['eligible']} | " +
                     " | ".join(fmt(r[k]) for k in ("first_ms", "warm_ms", "next_ms", "peak_rss_mib")) + " |")
    ratios = {}
    for reference in ("mupdf", "pdfium"):
        ratios[reference] = {}
        for metric in ("first_ms", "warm_ms", "next_ms"):
            pairs = []
            for case, dpi in gates:
                group = {r["engine"]: r for r in rows if r["case"] == case and r["dpi"] == dpi}
                if len(group) == 3 and all(r["eligible"] and not r["baseline"] for r in group.values()):
                    a, b = group["rzpdf"][metric], group[reference][metric]
                    if a is not None and b:
                        pairs.append(a / b)
            ratios[reference][metric] = dict(n=len(pairs), ratio=math.exp(statistics.mean(map(math.log, pairs))) if pairs else None)
    lines += ["", "rzpdf / reference ratios: >1 means rzpdf takes longer. Geometric means of paired eligible nonblank cases.", "", "```json", json.dumps(ratios, indent=2), "```", ""]
    (directory / "report.md").write_text("\n".join(lines))
    return ratios


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--dpi", nargs="+", type=int, default=[72, 144])
    parser.add_argument("--case", nargs="+", help="run only these manifest case names")
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--repeats", type=int, default=10)
    parser.add_argument("--warmup", type=int, default=2)
    parser.add_argument("--diagnostics", action="store_true", help="record phase CPU and initialized/first/replay RSS outside operation timers")
    parser.add_argument("--timeout", type=float, default=60)
    args = parser.parse_args()
    if not (1 <= args.trials <= 100 and 1 <= args.repeats <= 1000 and 0 <= args.warmup <= 100
            and args.timeout > 0 and all(1 <= dpi <= 1200 for dpi in args.dpi)):
        parser.error("invalid limits")
    if len(set(args.dpi)) != len(args.dpi):
        parser.error("DPI values must be unique")
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = json.loads(args.manifest.read_text())
    names = [case["name"] for case in manifest["cases"]]
    if not names or len(set(names)) != len(names) or any(not name for name in names):
        parser.error("manifest needs nonempty, unique case names")
    if args.case:
        if set(args.case) - set(names):
            parser.error("unknown case name")
        manifest["cases"] = [case for case in manifest["cases"] if case["name"] in args.case]
    for case in manifest["cases"]:
        path = Path(case["path"])
        if not path.is_absolute():
            path = args.manifest.parent / path
        case["path"] = str(path.resolve())
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if case.get("sha256", digest) != digest:
            raise ValueError(f"corpus hash mismatch: {case['name']}")
        case["sha256"] = digest
    metadata = dict(schema=1, utc=datetime.now(timezone.utc).isoformat(), platform=platform.platform(),
                    python=sys.version, arguments=vars(args) | {"manifest":str(args.manifest), "output":str(args.output)},
                    git_head=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                    git_status=subprocess.check_output(["git", "status", "--short"], cwd=ROOT, text=True),
                    corpus=manifest, binary_sha256=hashlib.sha256((ROOT / "target/release/examples/benchmark_render").read_bytes()).hexdigest(),
                    source_sha256={path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in (
                        "scripts/benchmark_pdf.py", "scripts/benchmark_pdf_worker.py",
                        "scripts/prepare_benchmark_corpus.py", "render/examples/benchmark_render.rs",
                        "Cargo.lock", "pdf/src/page/pdf_image.rs", "pdf/src/color/icc_profile.rs",
                        "render/src/render_device.rs", "render/src/raster_path.rs",
                        "render/src/coverage_spans.rs", "render/src/performance_tests.rs",
                        "render/src/render_session.rs", "render/src/render_session_tests.rs",
                        "pdf/src/device/mod.rs", "pdf/src/device/painting.rs", "pdf/src/page/display_list.rs",
                        "pdf/src/io/stream_reader.rs", "pdf/src/parser/syntax.rs",
                        "pdf/src/page/content_parser.rs", "pdf/src/page/content_interpreter.rs",
                        "pdf/src/page/preparation_profile.rs", "pdf/Cargo.toml")})
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2))
    gates, checks, records = {}, [], []
    rng = random.Random(20261026)
    with (args.output / "raw.jsonl").open("w") as raw:
        for index, (case, dpi) in enumerate(itertools.product(manifest["cases"], args.dpi)):
            print(f"[{index+1}/{len(manifest['cases'])*len(args.dpi)}] {case['name']} {dpi} DPI", flush=True)
            validation = {}
            for engine in ENGINES:
                directory = args.output / f"{index:03d}-{engine}"
                directory.mkdir()
                job = dict(path=case["path"], dpi=dpi, repeats=0, warmup=0, output_dir=str(directory.resolve()))
                validation[engine] = invoke(engine, job, args.timeout, directory / "stderr.txt")
                if validation[engine]["status"] == "ok":
                    from PIL import Image
                    for output in validation[engine]["outputs"]:
                        Image.frombytes("RGBA", (output["width"], output["height"]), Path(output["file"]).read_bytes()).save(Path(output["file"]).with_suffix(".png"))
            passed = all(v["status"] == "ok" for v in validation.values())
            comparisons = []
            if passed:
                for a, b in itertools.combinations(ENGINES, 2):
                    left, right = validation[a]["outputs"], validation[b]["outputs"]
                    passed &= len(left) == len(right)
                    for x, y in zip(left, right):
                        comparison = quality(x, y, case.get("nonblank", True))
                        comparisons.append(dict(engines=[a, b], page=x["page"], **comparison))
                        passed &= comparison["passed"]
            gates[case["name"], dpi] = passed
            checks.append(dict(case=case["name"], dpi=dpi, passed=passed, validation=validation, comparisons=comparisons))
            for trial in range(args.trials):
                order = list(ENGINES)
                rng.shuffle(order)
                for engine in order:
                    job = dict(path=case["path"], dpi=dpi, repeats=args.repeats, warmup=args.warmup, diagnostics=args.diagnostics)
                    result = invoke(engine, job, args.timeout, args.output / f"{index:03d}-{trial}-{engine}.stderr.txt")
                    record = result | dict(case=case["name"], dpi=dpi, engine=engine, trial=trial,
                                           baseline=not case.get("nonblank", True))
                    records.append(record)
                    raw.write(json.dumps(record) + "\n")
                    raw.flush()
    (args.output / "quality.json").write_text(json.dumps(checks, indent=2))
    rows = summarize(records, gates)
    ratios = report(rows, gates, args.output)
    (args.output / "summary.json").write_text(json.dumps(dict(rows=rows, ratios=ratios), indent=2))
    print(f"Quality agreement: {sum(gates.values())}/{len(gates)}; report: {args.output / 'report.md'}")


if __name__ == "__main__":
    main()
