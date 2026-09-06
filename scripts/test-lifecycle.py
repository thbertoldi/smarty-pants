#!/usr/bin/env python3
"""Run CLI/daemon lifecycle with temporary XDG paths, no clipboard operations or API calls."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

binary = Path(sys.argv[1]).resolve() / 'smarty-pants'
with tempfile.TemporaryDirectory(prefix='smarty-lifecycle-') as temporary:
    root = Path(temporary)
    config = root / 'config/smarty-pants/config.toml'
    config.parent.mkdir(parents=True)
    env = dict(os.environ, XDG_CONFIG_HOME=str(root / 'config'), XDG_RUNTIME_DIR=str(root), XDG_DATA_HOME=str(root / 'data'), XDG_STATE_HOME=str(root / 'state'))
    env.pop('SMARTY_PANTS_SOCKET', None)
    env.pop('DEEPSEEK_API_KEY', None)
    def cli(*args):
        return subprocess.run([str(binary), *args], env=env, text=True, capture_output=True, timeout=20)
    config.write_text('[inference]\nprovider="deepseek"\n[shortcuts]\nenabled=false\n[tray]\nenabled=false\n')
    try:
        started = cli('daemon', 'start')
        assert started.returncode == 0 and 'daemon ready' in started.stdout, (started.stdout, started.stderr)
        assert 'already running' in cli('daemon', 'start').stdout
        assert 'provider: deepseek' in cli('status').stdout
        log = root / 'state/smarty-pants/daemon.log'
        assert log.stat().st_mode & 0o777 == 0o600
        assert not (root / 'data').exists()
        assert cli('daemon', 'stop').returncode == 0
        deadline = time.monotonic() + 5
        while (root / 'smarty-pants.sock').exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        assert not (root / 'smarty-pants.sock').exists()
        config.write_text('not valid TOML')
        failed = cli('daemon', 'start')
        assert failed.returncode != 0 and 'daemon exited' in failed.stderr, (failed.stdout, failed.stderr)
        assert str(log) in failed.stderr
    finally:
        cli('daemon', 'stop')
print('Lifecycle: readiness, repeated launch, private log, clean stop and invalid-config startup passed')
