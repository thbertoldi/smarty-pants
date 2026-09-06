#!/usr/bin/env python3
"""Exercise a real release bundle without touching the current user's installation."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

bundle = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="smarty-install-") as temporary:
    root = Path(temporary)
    prefix = root / "prefix with spaces"
    data, config = root / "data", root / "config"
    stub = root / "stubs"
    stub.mkdir()
    (stub / "systemctl").write_text("#!/bin/sh\nexit 0\n")
    (stub / "systemctl").chmod(0o755)
    env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(data), PATH=f'{stub}:{os.environ["PATH"]}')
    def install():
        subprocess.run(["bash", str(bundle / "install.sh"), "--prefix", str(prefix)], env=env, check=True, stdout=subprocess.DEVNULL)
    install()
    assert (prefix / "bin/smarty-pants").is_file()
    unit = config / "systemd/user/smarty-pants.service"
    assert f'ExecStart="{prefix}/bin/smarty-pants-daemon"' in unit.read_text()
    subprocess.run(["desktop-file-validate", str(data / "applications/computer.smarty-pants.desktop")], check=True)
    # Upgrades preserve user configuration, including credentials and custom prompts.
    cfg = config / "smarty-pants/config.toml"
    cfg.parent.mkdir(exist_ok=True)
    cfg.write_text("# existing user config\n[inference]\nprovider='deepseek'\n")
    unit.write_text("# user-owned service\n")
    install()
    assert cfg.read_text().startswith("# existing user config")
    assert unit.read_text() == "# user-owned service\n"
    assert unit.with_suffix(".service.new").exists()
    subprocess.run(["bash", str(data / "smarty-pants/uninstall.sh"), "--prefix", str(prefix)], env=env, check=True, stdout=subprocess.DEVNULL)
    assert not (prefix / "bin/smarty-pants").exists()
    assert cfg.exists() and unit.exists()
print("Installer: clean install, paths with spaces, config/service preservation and uninstall passed")
