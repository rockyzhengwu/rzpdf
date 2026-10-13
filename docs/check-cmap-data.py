#!/usr/bin/env python3
"""Offline provenance and effective CID regression checks (not a PS interpreter).

Only the cidchar/cidrange/codespacerange subset used by the registered maps is
accepted. Mapping fingerprints are regression checks, not security primitives.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "font-data/cmap-provenance.json"
BLOCK = re.compile(r"(\d+)\s+begin(cidchar|cidrange|codespacerange)\b(.*?)end\2", re.S)
HEX = r"<([0-9a-fA-F]+)>"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def parse(directory, name, chain=()):
    if name in chain or not re.fullmatch(r"[A-Za-z0-9-]+", name):
        raise ValueError(f"Invalid/cyclic CMap: {chain!r}, {name!r}")
    text = re.sub(r"%[^\n]*", "", (directory / name).read_text())
    if re.search(r"\bbegin(?:bfchar|bfrange)\b", text):
        raise ValueError(f"Unicode mapping not supported by this CID checker: {name}")
    bases = re.findall(r"/([A-Za-z0-9-]+)\s+usecmap\b", text)
    if len(bases) > 1:
        raise ValueError(f"Multiple bases: {name}")
    mapping, spaces, wmode = parse(directory, bases[0], (*chain, name)) if bases else ({}, [], None)
    local_spaces = []
    for count, kind, body in BLOCK.findall(text):
        pattern = HEX + (r"\s+" + HEX if kind != "cidchar" else "")
        if kind != "codespacerange":
            pattern += r"\s+(\d+)"
        entries = re.findall(pattern, body)
        if len(entries) != int(count) or re.sub(pattern, "", body).strip():
            raise ValueError(f"Malformed {kind} block: {name}")
        for entry in entries:
            low = int(entry[0], 16)
            high = low if kind == "cidchar" else int(entry[1], 16)
            if low > high or (kind != "codespacerange" and high - low > 1_000_000):
                raise ValueError(f"Invalid/oversized range: {name}")
            if kind == "codespacerange":
                if len(entry[0]) != len(entry[1]) or len(entry[0]) % 2:
                    raise ValueError(f"Invalid codespace width: {name}")
                local_spaces.append((len(entry[0]) // 2, low, high))
            else:
                first_cid = int(entry[-1])
                for code in range(low, high + 1):
                    mapping[code] = first_cid + code - low
    if local_spaces:
        spaces = local_spaces
    mode = re.search(r"/WMode\s+(\d+)\s+def", text)
    if mode:
        wmode = int(mode[1])
    return mapping, spaces, wmode


def fingerprint(directory, name):
    mapping, spaces, wmode = parse(directory, name)
    effective = sorted((code, cid) for code, cid in mapping.items()
                       if cid and any(low <= code <= high for _, low, high in spaces))
    digest = hashlib.md5(usedforsecurity=False)
    for code, cid in effective:
        digest.update(code.to_bytes(4, "big"))
        digest.update(cid.to_bytes(4, "big"))
    return {"entries": len(effective), "mapping_md5": digest.hexdigest(),
            "codespaces": sorted(spaces), "wmode": wmode}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-dir", type=Path,
                        help="Additionally compare against a local pre-replacement snapshot")
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text())
    directory = ROOT / "font-data/cmaps"
    for item in manifest["replacements"]:
        data = (directory / item["name"]).read_bytes()
        require(hashlib.sha256(data).hexdigest() == item["sha256"], item["name"])
        require(b"Redistribution and use" in data, item["name"])
        require(b"usecmap" not in data, item["name"])
    for item in manifest["regressions"]:
        actual = fingerprint(directory, item["name"])
        expected = {key: value for key, value in item.items() if key != "name"}
        # JSON represents tuples as arrays.
        require(json.loads(json.dumps(actual)) == expected, item["name"])
        if args.baseline_dir:
            require(fingerprint(args.baseline_dir, item["name"]) == actual, item["name"])
        print(f"PASS {item['name']}: {actual['entries']} effective CID mappings")
    for name in manifest["removed_helpers"]:
        require(not (directory / name).exists(), name)
    for path in directory.iterdir():
        text = path.read_text()
        require("%%Copyright:" in text and "Redistribution and use" in text, path.name)
        for base in re.findall(r"/([A-Za-z0-9-]+)\s+usecmap\b", text):
            require((directory / base).is_file() or base == "Identity-H", (path.name, base))
    license_data = (ROOT / manifest["license_file"]).read_bytes()
    require(hashlib.sha256(license_data).hexdigest() == manifest["license_sha256"], "License hash")
    print("CMap provenance, dependencies, codespaces, WMode and CID regressions passed.")


if __name__ == "__main__":
    main()
