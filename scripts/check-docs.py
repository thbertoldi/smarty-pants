#!/usr/bin/env python3
"""Check current documentation's relative file links, without network requests."""
from pathlib import Path
import re
import sys
from urllib.parse import unquote

root = Path(__file__).resolve().parents[1]
# Historical planning documents reference work that was never shipped; don't treat them as user guides.
paths = [root / 'README.md', root / 'CONTRIBUTING.md', root / 'CHANGELOG.md']
paths += list((root / 'docs').glob('*.md')) + list((root / 'packaging').rglob('*.md'))
errors = []
for path in paths:
    for match in re.finditer(r'\]\(([^)]+)\)', path.read_text()):
        link = match[1].split('#', 1)[0]
        if not link or '://' in link or link.startswith(('mailto:', '/')):
            continue
        if not (path.parent / unquote(link)).exists():
            errors.append(f'{path.relative_to(root)}: missing {link}')
if errors:
    sys.exit('\n'.join(errors))
print(f'Local links passed in {len(paths)} documentation files')
