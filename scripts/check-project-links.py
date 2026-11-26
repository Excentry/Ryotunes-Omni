#!/usr/bin/env python3
"""Project-level static checks that do not need Node modules or Cargo."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / 'ui' / 'src'
SKIP = {'.git', 'target', 'node_modules', 'build', '.svelte-kit'}
errors: list[str] = []

# Svelte control blocks. This deliberately checks only structural pairs; svelte-check remains the
# semantic authority in CI and on a developer machine.
open_re = re.compile(r'\{#(if|each|key|await|snippet)\b')
close_re = re.compile(r'\{/(if|each|key|await|snippet)\}')
for p in UI.rglob('*.svelte'):
    text = p.read_text(errors='replace')
    stack: list[tuple[str, int]] = []
    events = [(m.start(), 'open', m.group(1)) for m in open_re.finditer(text)]
    events += [(m.start(), 'close', m.group(1)) for m in close_re.finditer(text)]
    for pos, kind, name in sorted(events):
        line = text.count('\n', 0, pos) + 1
        if kind == 'open':
            stack.append((name, line))
        elif not stack or stack[-1][0] != name:
            got = stack[-1][0] if stack else 'nothing'
            errors.append(f'{p.relative_to(ROOT)}:{line}: closes {name}, top of stack is {got}')
        else:
            stack.pop()
    for name, line in stack:
        errors.append(f'{p.relative_to(ROOT)}:{line}: unclosed Svelte {name} block')

# Local imports. Package imports are intentionally ignored.
extensions = ['', '.ts', '.js', '.svelte', '.json']
index_exts = ['/index.ts', '/index.js', '/index.svelte']
source_files = list(UI.rglob('*.ts')) + list(UI.rglob('*.svelte'))
imp = re.compile(r'''(?:from\s+|import\s*\()\s*["']([^"']+)["']''')
for p in source_files:
    text = p.read_text(errors='replace')
    for spec in imp.findall(text):
        if spec.startswith('$lib/'):
            base = UI / 'lib' / spec[5:]
        elif spec.startswith('.'):
            base = (p.parent / spec).resolve()
        else:
            continue
        candidates = [Path(str(base) + e) for e in extensions] + [Path(str(base) + e) for e in index_exts]
        # TypeScript/SvelteKit source commonly imports a TypeScript module using its emitted .js
        # specifier (for example $lib/utils.js -> $lib/utils.ts, ./index.js -> ./index.ts).
        # Accept the source-side equivalents so this check matches the resolver used by the build.
        if base.suffix == '.js':
            stem = base.with_suffix('')
            candidates += [stem.with_suffix('.ts'), stem.with_suffix('.svelte')]
        if not any(c.is_file() for c in candidates):
            errors.append(f'{p.relative_to(ROOT)}: unresolved local import {spec!r}')

# Frontend invoke names must be registered in the Tauri handler. Dynamic wrapper names are skipped.
api_text = (UI / 'lib' / 'api.ts').read_text()
invokes = set(re.findall(r'''invoke(?:<[^>]+>)?\(\s*["']([a-zA-Z0-9_]+)["']''', api_text))
lib_text = (ROOT / 'src-tauri' / 'src' / 'lib.rs').read_text()
handler = re.search(r'generate_handler!\[(.*?)\]', lib_text, re.S)
registered = set(re.findall(r'\b[a-z_][a-z0-9_]*::([A-Za-z0-9_]+)', handler.group(1) if handler else ''))
missing = sorted(invokes - registered)
if missing:
    errors.append('Tauri commands invoked but not registered: ' + ', '.join(missing))

# CSS braces/comments/strings: lightweight parser catches truncated release edits.
def css_balanced(path: Path) -> None:
    text = path.read_text(errors='replace')
    depth = 0
    state = 'code'
    i = 0
    while i < len(text):
        c = text[i]
        n = text[i+1] if i + 1 < len(text) else ''
        if state == 'comment':
            if c == '*' and n == '/': state = 'code'; i += 1
        elif state in {'sq', 'dq'}:
            if c == '\\': i += 1
            elif (state == 'sq' and c == "'") or (state == 'dq' and c == '"'): state = 'code'
        else:
            if c == '/' and n == '*': state = 'comment'; i += 1
            elif c == "'": state = 'sq'
            elif c == '"': state = 'dq'
            elif c == '{': depth += 1
            elif c == '}':
                depth -= 1
                if depth < 0:
                    errors.append(f'{path.relative_to(ROOT)}: extra closing brace')
                    return
        i += 1
    if state == 'comment': errors.append(f'{path.relative_to(ROOT)}: unterminated CSS comment')
    if state in {'sq','dq'}: errors.append(f'{path.relative_to(ROOT)}: unterminated CSS string')
    if depth: errors.append(f'{path.relative_to(ROOT)}: CSS brace depth {depth}')

for p in (UI / 'lib').rglob('*.css'): css_balanced(p)
for p in (UI / 'routes').rglob('*.css'): css_balanced(p)

if errors:
    print('\n'.join(errors), file=sys.stderr)
    sys.exit(1)
print(f'Project links: {len(source_files)} frontend files, {len(invokes)} Tauri invokes, structure OK')
