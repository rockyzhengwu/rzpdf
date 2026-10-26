#!/usr/bin/env python3
"""Interleave two Release benchmark_render binaries; compare timings and pixels.

The workers must implement the benchmark_render JSON protocol. Diagnostics and
warm replay are disabled: this measures document opening and cold page preparation
plus first rendering, rather than preparation of an already cached page.
"""
import argparse
import hashlib
import json
import platform
import random
import statistics
import subprocess
from pathlib import Path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', type=Path, required=True)
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--dpi', type=int, nargs='+', default=[72, 144])
    parser.add_argument('--trials', type=int, default=5)
    parser.add_argument('--timeout', type=float, default=60)
    args = parser.parse_args()
    if not 1 <= args.trials <= 20 or any(not 1 <= dpi <= 1200 for dpi in args.dpi):
        parser.error('trials must be 1..20 and DPI 1..1200')
    args.output.mkdir(parents=True, exist_ok=False)
    binaries = {label: path.resolve() for label, path in [('before', args.before), ('after', args.after)]}
    manifest = json.loads(args.manifest.read_text())
    for case in manifest['cases']:
        path = Path(case['path'])
        path = path if path.is_absolute() else args.manifest.parent / path
        case['path'] = str(path.resolve())
        actual = digest(path)
        if case.get('sha256', actual) != actual:
            raise ValueError(f"input hash mismatch: {case['name']}")
        case['sha256'] = actual
    metadata = dict(platform=platform.platform(), trials=args.trials, dpi=args.dpi,
                    seed=20261026, diagnostics=False, manifest=manifest,
                    binary_sha256={label: digest(path) for label, path in binaries.items()},
                    driver_sha256=digest(Path(__file__)))
    (args.output / 'metadata.json').write_text(json.dumps(metadata, indent=2))
    rng = random.Random(metadata['seed'])
    rows, summaries, comparisons = [], [], []
    with (args.output / 'raw.jsonl').open('w') as raw:
        for case in manifest['cases']:
            for dpi in args.dpi:
                print(case['name'], dpi, 'DPI', flush=True)
                pair = {}
                for trial in range(args.trials):
                    order = ['before', 'after']
                    rng.shuffle(order)
                    for label in order:
                        directory = args.output / f"{case['name']}-{dpi}-{label}"
                        job = dict(path=case['path'], dpi=dpi, repeats=0, warmup=0)
                        if trial == 0:
                            directory.mkdir()
                            job['output_dir'] = str(directory.resolve())
                        result = subprocess.run([str(binaries[label])], input=json.dumps(job),
                                                text=True, capture_output=True, timeout=args.timeout)
                        (args.output / f"{case['name']}-{dpi}-{trial}-{label}.stderr.txt").write_text(result.stderr)
                        record = json.loads(result.stdout)
                        if result.returncode != 0 or record.get('status') != 'ok':
                            raise RuntimeError(f"{case['name']}/{dpi}/{label}: {record}")
                        row = dict(case=case['name'], dpi=dpi, trial=trial, label=label, **record)
                        rows.append(row)
                        raw.write(json.dumps(row) + '\n')
                        raw.flush()
                        if trial == 0:
                            pair[label] = record['outputs']
                if len(pair['before']) != len(pair['after']):
                    raise AssertionError('different output page counts')
                for before, after in zip(pair['before'], pair['after']):
                    equal = all(before[key] == after[key] for key in ['page', 'width', 'height'])
                    equal = equal and digest(Path(before['file'])) == digest(Path(after['file']))
                    comparisons.append(dict(case=case['name'], dpi=dpi, page=before['page'], equal=equal))
                summary = dict(case=case['name'], dpi=dpi, nonblank=case.get('nonblank', True))
                for metric in ['open_ms', 'prepare_ms', 'raster_ms', 'first_ms']:
                    values = {label: statistics.median(row[metric] for row in rows
                              if row['case'] == case['name'] and row['dpi'] == dpi and row['label'] == label)
                              for label in binaries}
                    summary[metric] = {**values, 'after_before': values['after'] / values['before']}
                summaries.append(summary)
    (args.output / 'summary.json').write_text(json.dumps(summaries, indent=2))
    (args.output / 'pixels.json').write_text(json.dumps(comparisons, indent=2))
    if not all(row['equal'] for row in comparisons):
        raise AssertionError('before/after pixels differ; see pixels.json')
    print(f"Passed: {len(rows)} trials; {len(comparisons)} identical page outputs", flush=True)


if __name__ == '__main__':
    main()
