#!/usr/bin/env python3
"""Cache the separately licensed real-document benchmark inputs, with fixed hashes."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
MAX_BYTES = 20 * 1024 * 1024


def prepare(directory):
    corpus = json.loads((ROOT / "docs/benchmark-real-corpus.json").read_text())
    directory.mkdir(parents=True, exist_ok=True)
    for case in corpus["cases"]:
        path = directory / case["path"]
        if path.exists():
            if path.stat().st_size > MAX_BYTES or hashlib.sha256(path.read_bytes()).hexdigest() != case["sha256"]:
                raise ValueError(f"cached corpus hash mismatch: {path}")
            continue
        request = urllib.request.Request(case["url"], headers={"User-Agent": "rzpdf-benchmark/1.0"})
        with urllib.request.urlopen(request, timeout=40) as response:
            data = response.read(MAX_BYTES + 1)
        if len(data) > MAX_BYTES or not data.startswith(b"%PDF-"):
            raise ValueError(f"invalid or oversized PDF: {case['name']}")
        if hashlib.sha256(data).hexdigest() != case["sha256"]:
            raise ValueError(f"upstream file changed; review provenance before updating: {case['name']}")
        path.write_bytes(data)
    corpus["generator"] = "licensed-real-documents-v1"
    manifest = directory / "manifest.json"
    manifest.write_text(json.dumps(corpus, indent=2) + "\n")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    print(prepare(parser.parse_args().directory))
