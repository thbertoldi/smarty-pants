#!/usr/bin/env python3
"""Bundle license files and a manifest of resolved Cargo dependencies."""
import json
from pathlib import Path
import shutil
import subprocess
import sys

flavor, output = sys.argv[1:]
features = "smarty-pants-daemon/tray"
if flavor == "cpu":
    features += ",smarty-pants-daemon/local"
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--format-version", "1", "--no-default-features",
    "--features", features, "--filter-platform", "x86_64-unknown-linux-gnu",
]))
output = Path(output)
output.mkdir(parents=True)
index = ["Resolved Cargo dependencies (including build/test dependencies).", "", "Package\tLicense expression\tRepository"]
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if not package["source"]:
        continue
    label = f'{package["name"]}-{package["version"]}'
    index.append(f'{label}\t{package["license"] or "See bundled license"}\t{package["repository"] or ""}')
    root = Path(package["manifest_path"]).parent
    for path in root.rglob("*"):
        if path.is_file() and path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT")):
            target = output / label / path.relative_to(root)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)
(output / "DEPENDENCIES.tsv").write_text("\n".join(index) + "\n")
