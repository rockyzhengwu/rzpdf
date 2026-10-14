"""Read-only exact-version OSV query for Cargo.lock registry dependencies.

Results are time-sensitive evidence, not a claim about undisclosed bugs.
Writes a new report under ignored tmp/pdfs; never edits Cargo.lock or dependencies.
"""
import json
from pathlib import Path
import tempfile
import tomllib
import urllib.request
from datetime import datetime, timezone

root=Path(__file__).resolve().parents[1]
lock=tomllib.loads((root/"Cargo.lock").read_text())
packages=[p for p in lock["package"] if p.get("source","").startswith("registry+")]
findings=[]
for start in range(0,len(packages),100):
    batch=packages[start:start+100]
    body=json.dumps({"queries":[{"package":{"name":p["name"],"ecosystem":"crates.io"},"version":p["version"]} for p in batch]}).encode()
    request=urllib.request.Request("https://api.osv.dev/v1/querybatch",data=body,headers={"Content-Type":"application/json"})
    with urllib.request.urlopen(request,timeout=30) as response:
        results=json.load(response)["results"]
    if len(results)!=len(batch):
        raise RuntimeError("OSV result count mismatch")
    for package,result in zip(batch,results):
        if result.get("vulns"):
            findings.append({"name":package["name"],"version":package["version"],"vulnerabilities":result["vulns"]})
report={"queried_at":datetime.now(timezone.utc).isoformat(),"queried":len(packages),"findings":findings}
with tempfile.NamedTemporaryFile(prefix="dependency-osv-",suffix=".json",dir=root/"tmp/pdfs",mode="w",delete=False) as output:
    json.dump(report,output,indent=2)
    report["report_file"]=output.name
print(json.dumps(report,indent=2))
raise SystemExit(1 if findings else 0)
