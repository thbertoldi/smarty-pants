#!/usr/bin/env bash
# Prepare pinned sources and locked dependencies with the OBS source services.
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output=${1:-target/rpm-sources}
if [[ -d $output && -n $(ls -A "$output") ]]; then
    echo 'Choose an empty output directory for the OBS sources.' >&2
    exit 1
fi
mkdir -p "$output"
output=$(cd "$output" && pwd)
stage=$(mktemp -d -t smarty-pants-rpm-XXXXXXXX)
trap 'rm -rf -- "$stage"' EXIT
cp "$repo_root"/packaging/rpm/{smarty-pants.spec,smarty-pants.changes,_service,_constraints,README.openSUSE} "$stage/"
cd "$stage"
# Use osc's service runner without an OBS account or working-copy metadata.
# In an osc checkout the equivalent command is: osc service manualrun.
python3 - <<'PY'
from pathlib import Path
from xml.etree import ElementTree
from osc.obs_scm.serviceinfo import Serviceinfo

services = Serviceinfo()
services.read(ElementTree.parse('_service').getroot())
raise SystemExit(services.execute(str(Path.cwd()), callmode='manual'))
PY
# Keep only package inputs, not the checkout or intermediate obs_scm exports.
cp -- smarty-pants.spec smarty-pants.changes _service _constraints README.openSUSE \
    smarty-pants-*.tar.zst vendor.tar.zst "$output/"
printf 'RPM/OBS sources prepared in %s\n' "$output"
