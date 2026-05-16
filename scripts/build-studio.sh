#!/usr/bin/env bash
# scripts/build-studio.sh — Manually assemble studio/index.html from source files
#
# ┌─────────────────────────────────────────────────────────────────┐
# │  You usually don't need to run this directly.                   │
# │                                                                 │
# │  `cargo build` and `cargo run` call a1-gateway/build.rs        │
# │  automatically, which rebuilds index.html whenever any source   │
# │  file in studio/src/ changes.                                   │
# │                                                                 │
# │  Use THIS script when you want to preview Studio changes in     │
# │  the browser without doing a full Rust compile.                 │
# └─────────────────────────────────────────────────────────────────┘
#
# Usage (run from project root or any directory):
#   ./scripts/build-studio.sh          Build once, then exit
#   ./scripts/build-studio.sh --watch  Rebuild on every file change
#
# Source layout (all files picked up automatically — just add files):
#   studio/src/css/*.css               Assembled A-Z
#   studio/src/js/[0-9]*.js           Root JS files, assembled A-Z
#   studio/src/js/components/*.js      Components, assembled A-Z after root
#   studio/src/js/99-app.js            Always last
#   studio/src/index.template.html     HTML skeleton
#
# Output:
#   studio/index.html  — embedded by the Rust gateway via include_str!

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
SRC="$ROOT/studio/src"
OUT="$ROOT/studio/index.html"

build() {
  echo -n "  Building studio/index.html..."

  python3 - "$OUT" <<PYEOF
import sys, glob, os

src = '$SRC'
template = open(os.path.join(src, 'index.template.html')).read()

css_parts = sorted(glob.glob(os.path.join(src, 'css', '*.css')))
css = ''.join(open(f).read() for f in css_parts)

js_root  = [f for f in sorted(glob.glob(os.path.join(src, 'js', '[0-9]*.js')))
            if '99-' not in os.path.basename(f)]
js_comps = sorted(glob.glob(os.path.join(src, 'js', 'components', '*.js')))
js_app   = os.path.join(src, 'js', '99-app.js')
all_js   = js_root + js_comps + [js_app]

js = ''.join(open(f).read() for f in all_js)

html = template.replace('/* {{CSS}} */', css).replace('// {{JS}}', js)
with open(sys.argv[1], 'w') as out:
    out.write(html)

print(f" done  ({len(css_parts)} CSS + {len(all_js)} JS → {len(html):,} bytes)")
PYEOF
}

if [[ "${1:-}" == "--watch" ]]; then
  echo "  Watching studio/src/ for changes... (Ctrl+C to stop)"
  build
  if command -v fswatch &>/dev/null; then
    fswatch -o "$SRC" | while read -r; do build; done
  elif command -v entr &>/dev/null; then
    find "$SRC" -type f | entr -s "bash '$SCRIPT_DIR/build-studio.sh'"
  else
    echo "  Tip: install 'fswatch' (Mac: brew install fswatch) or 'entr' (Linux) for watch mode."
    echo "  Falling back to polling every 2s..."
    while true; do
      sleep 2
      CHANGED=$(find "$SRC" -type f -newer "$OUT" 2>/dev/null | wc -l)
      if [ "$CHANGED" -gt 0 ]; then build; fi
    done
  fi
else
  build
fi
