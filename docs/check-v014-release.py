"""Local v0.14 frozen-source release gates. No CI, installs or network.
Build the documented native drivers first. Pass a NEW output directory.
Keeps all logs, exact return codes, input/binary hashes and durations.
"""
from concurrent.futures import ThreadPoolExecutor, wait, FIRST_COMPLETED
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
os.chdir(ROOT)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = subprocess.check_output(["git", "ls-files", "-c", "-o", "--exclude-standard", "-z"]).decode().split("\0")
    return {p: digest(ROOT / p) for p in sorted(set(paths)) if p and not p.endswith(".md") and (ROOT / p).is_file()}


drivers = ["v014_associated_files", "v014_benchmark", "v013_layers", "v012_layout", "v012_security_layout",
           "v012_incremental", "v012_preservation", "v011_sign_verify", "v011_field_source",
           "v011_encrypted_incremental", "inspect_optional_content", "render_optional_content", "v012_recovery", "render_annotations"]
before = sources()
binaries = {name: digest(ROOT / "target/debug/examples" / name) for name in drivers}
(OUT / "sources-before.json").write_text(json.dumps(before, indent=2))
(OUT / "binaries-before.json").write_text(json.dumps(binaries, indent=2))
env = dict(os.environ)
env["PYTHONPATH"] = str(ROOT / "tmp/pdfs/v05-verifiers") + ":" + str(ROOT / "tmp/pdfs/python")
env311 = dict(env)
env311["PYTHONPATH"] = str(ROOT / "tmp/pdfs/v05-verifiers-py311") + ":" + str(ROOT / "tmp/pdfs/python")
py311 = shutil.which("python3.11")
if not py311:
    raise RuntimeError("Existing Python 3.11 verifier environment is required; no automatic install")

targets = ["v014_associated_files", "v09_annotations", "v09_page_relationships", "v010_structure_transfer", "metadata",
           "v012_preservation", "v012_recovery", "v012_incremental_stream", "v012_layout_security", "v013_optional_content_edit",
           "v011_controlled_updates", "v011_signature_fields", "v011_encrypted_incremental", "v05_security", "v05_signatures"]
joint = ["cargo", "test", "-p", "pdf", "--locked"]
for target in targets:
    joint += ["--test", target]
jobs = [("workspace", ["cargo", "test", "--workspace", "--locked"], env),
        ("benchmark", ["/usr/bin/time", "-l", str(ROOT / "target/debug/examples/v014_benchmark")], env),
        ("associated-files", [sys.executable, "docs/check-v014-associated-files.py", str(OUT / "associated-files")], env)]
for label, script in [("object-streams", "v012-object-streams"), ("layout-security", "v012-layout-security"),
                      ("incremental", "v012-incremental"), ("preservation", "v012-preservation"), ("recovery", "v012-recovery"),
                      ("layers", "v013-layers"), ("optional-content", "v010-optional-content"),
                      ("workflows", "v011-workflows"), ("multisign", "v011-multisign"), ("signatures", "v011-signatures")]:
    jobs.append((label, [sys.executable, "docs/check-" + script + ".py", str(OUT / label)], env))
jobs.append(("encrypted-incremental", [py311, "docs/check-v011-encrypted-incremental.py", str(OUT / "encrypted-incremental")], env311))
jobs.append(("joint", joint, env))


def execute(job):
    label, command, environment = job
    print("START " + label, flush=True)
    start = time.monotonic()
    with (OUT / (label + ".log")).open("w") as stdout, (OUT / (label + "-stderr.log")).open("w") as stderr:
        result = subprocess.run(command, env=environment, stdout=stdout, stderr=stderr)
    record = dict(label=label, command=command, exit_code=result.returncode, seconds=time.monotonic() - start)
    (OUT / (label + "-receipt.json")).write_text(json.dumps(record, indent=2))
    print(f"END {label} exit={result.returncode} seconds={record['seconds']:.2f}", flush=True)
    return record


results = []
with ThreadPoolExecutor(max_workers=4) as executor:
    pending = {executor.submit(execute, job) for job in jobs}
    while pending:
        finished, pending = wait(pending, timeout=30, return_when=FIRST_COMPLETED)
        results.extend(f.result() for f in finished)
        if not finished:
            print(f"RUNNING {len(pending)} jobs pending (see per-job logs)", flush=True)

after = sources()
after_binaries = {name: digest(ROOT / "target/debug/examples" / name) for name in drivers}
(OUT / "sources-after.json").write_text(json.dumps(after, indent=2))
(OUT / "binaries-after.json").write_text(json.dumps(after_binaries, indent=2))
summary = dict(head=subprocess.check_output(["git", "rev-parse", "HEAD"]).decode().strip(), results=results,
               source_count=len(before), sources_unchanged=before == after, binaries_unchanged=binaries == after_binaries,
               independent_gates=12, completed=True)
(OUT / "summary.json").write_text(json.dumps(summary, indent=2))
print(json.dumps(summary), flush=True)
assert before == after, "Frozen inputs changed"
assert binaries == after_binaries, "Gate driver binaries changed"
assert all(r["exit_code"] == 0 for r in results), "One or more gates failed; inspect receipts"
