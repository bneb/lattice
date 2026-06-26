#!/usr/bin/env bash
# Render blog posts to HTML with shared stylesheet and open in browser.
# Usage: ./tools/preview-blog.sh              # index page listing all posts
#        ./tools/preview-blog.sh --all        # render all posts + index
#        ./tools/preview-blog.sh --open FILE  # render and open a single post
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLOG_DIR="$PROJECT_ROOT/docs/blog"
OUT_DIR="/tmp/salt-blog-preview"
STYLE="$PROJECT_ROOT/tools/blog-style.css"

mkdir -p "$OUT_DIR"

# Shared stylesheet — exact colors and fonts from salt-lang.dev
# Served from script (not file) to avoid linter reverts
cat > "$STYLE" << 'CSS'
*,
*::before,
*::after {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

:root {
    --bg: #f8f5f0;
    --bg-card: #ffffff;
    --bg-card-hover: #f0ede8;
    --card-bg: #ffffff;
    --card-border: #e0ddd5;
    --border: #e0ddd5;
    --text: #1a1a2e;
    --text-muted: #6b6b7b;
    --text-secondary: #4a4a5a;
    --accent: #7ba7c9;
    --accent-glow: rgba(123, 167, 201, 0.15);
    --green: #2a9d8f;
    --orange: #c9a962;
    --red: #c0574f;
    --blue: #7ba7c9;
    --khaki: #f0ece4;
    --radius: 12px;
    --font: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
    --heading: 'DM Serif Display', Georgia, 'Times New Roman', serif;
    --mono: 'JetBrains Mono', 'SF Mono', 'Fira Code', monospace;
    --inline-code-bg: #f0e9db;
}

@media (prefers-color-scheme: dark) {
    :root {
        --bg: #0a0a0f;
        --bg-card: #12121a;
        --bg-card-hover: #1a1a28;
        --card-bg: #12121a;
        --card-border: #1e1e30;
        --border: #1e1e30;
        --text: #e4e4ef;
        --text-muted: #8888a0;
        --text-secondary: #aaaabc;
        --accent: #6c5ce7;
        --accent-glow: rgba(108, 92, 231, 0.15);
        --green: #00d68f;
        --orange: #ff9f43;
        --red: #ff6b6b;
        --blue: #54a0ff;
        --khaki: #0d0d14;
        --inline-code-bg: #1a1a28;
    }
}

html { scroll-behavior: smooth; }

body {
    font-family: var(--font);
    background: var(--bg);
    color: var(--text);
    line-height: 1.6;
    -webkit-font-smoothing: antialiased;
    overflow-x: hidden;
}

/* ── Typography ────────────────────── */
h1, h2, h3, h4 { font-family: var(--heading); font-weight: 400; letter-spacing: -0.01em; }
.blog-body h1 { font-size: 2.5rem; line-height: 1.15; margin: 0 0 0.5rem; }
.blog-body h2 { font-size: 1.6rem; margin: 2.5rem 0 0.75rem; padding-bottom: 0.35rem; border-bottom: 1px solid var(--border); }
.blog-body h3 { font-size: 1.25rem; margin: 2rem 0 0.5rem; }
p { margin: 0.75rem 0; }
strong { font-weight: 600; }
a { color: var(--accent); text-decoration: none; transition: color 0.15s ease; }
a:hover { color: var(--blue); }

/* ── Code ──────────────────────────── */
code {
    font-family: var(--mono);
    font-size: 0.88em;
    font-weight: 500;
    background: var(--inline-code-bg);
    color: var(--text-secondary);
    padding: 0.15em 0.4em;
    border-radius: 4px;
}
pre {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 20px 24px;
    overflow-x: auto;
    margin: 1.25rem 0;
    line-height: 1.6;
}
pre code {
    background: none;
    padding: 0;
    font-size: 0.85rem;
    font-weight: 400;
    color: var(--text);
}

/* ── Tables ────────────────────────── */
table {
    width: 100%;
    border-collapse: collapse;
    margin: 1.25rem 0;
    font-size: 0.92rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
}
th, td { text-align: left; padding: 10px 14px; border-bottom: 1px solid var(--border); }
th {
    font-family: var(--font);
    font-weight: 600;
    font-size: 0.82rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
    background: var(--khaki);
}
tr:last-child td { border-bottom: none; }

/* ── Blockquote, HR ────────────────── */
blockquote {
    border-left: 3px solid var(--accent);
    padding-left: 18px;
    color: var(--text-muted);
    margin: 1.25rem 0;
    font-style: italic;
}
hr { border: none; border-top: 1px solid var(--border); margin: 2.5rem 0; }

/* ── Blog layout ───────────────────── */
.blog-body {
    max-width: 680px;
    margin: 0 auto;
    padding: 64px 24px 120px;
}
.blog-body pre { margin: 1.25rem -24px; }
@media (min-width: 728px) {
    .blog-body pre { margin: 1.25rem 0; border-radius: var(--radius); }
}

/* ── Index / post cards ────────────── */
.post-list { max-width: 680px; margin: 80px auto 0; padding: 0 24px; }
.post-card {
    display: block;
    padding: 1.5rem;
    margin-bottom: 1rem;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    text-decoration: none !important;
    transition: border-color 0.2s ease, box-shadow 0.2s ease;
}
.post-card:hover {
    border-color: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-glow);
    text-decoration: none !important;
}
.post-card h2 {
    font-family: var(--heading);
    font-weight: 400;
    font-size: 1.3rem;
    letter-spacing: -0.01em;
    margin: 0 0 0.25rem;
    color: var(--text);
}
.post-card p {
    margin: 0.5rem 0 0;
    color: var(--text-secondary);
    font-size: 0.95rem;
    line-height: 1.5;
}

/* ── Chrome (nav + footer) ─────────── */
.blog-nav {
    max-width: 680px;
    margin: 0 auto;
    padding: 24px 24px 0;
    display: flex;
    justify-content: space-between;
    align-items: baseline;
}
.blog-nav .home {
    font-family: var(--heading);
    font-size: 1.15rem;
    color: var(--text);
    text-decoration: none;
}
.blog-nav .home:hover { color: var(--accent); text-decoration: none; }
.blog-nav .links a {
    margin-left: 1.25rem;
    color: var(--text-muted);
    font-size: 0.9rem;
    font-weight: 500;
    text-decoration: none;
}
.blog-nav .links a:hover { color: var(--text); text-decoration: none; }
.blog-footer {
    max-width: 680px;
    margin: 0 auto;
    padding: 0 24px 80px;
    color: var(--text-muted);
    font-size: 0.85rem;
}
.blog-footer a { color: var(--text-secondary); text-decoration: none; }
.blog-footer a:hover { color: var(--accent); }
CSS

# HTML template header
header() {
  local title="$1"
  cat << HEADER
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${title} — Salt</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=DM+Serif+Display:ital@0;1&family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
<link rel="stylesheet" href="file://$STYLE">
</head>
<body>
<nav class="blog-nav">
  <a class="home" href="file://$OUT_DIR/index.html">Salt</a>
  <div class="links">
    <a href="file://$OUT_DIR/index.html">Blog</a>
    <a href="https://github.com/bneb/lattice">GitHub</a>
  </div>
</nav>
HEADER
}

footer() {
  cat << FOOTER
<footer class="blog-footer">
  <p>Salt is a systems language with Z3 theorem proving in the compiler. <a href="https://github.com/bneb/lattice">github.com/bneb/lattice</a></p>
</footer>
</body>
</html>
FOOTER
}

render_post() {
  local src="$1"
  local out="$2"
  local title
  title=$(head -5 "$src" | grep '^# ' | head -1 | sed 's/^# //')
  [ -z "$title" ] && title="Untitled"

  header "$title" > "$out"
  echo '<div class="blog-body">' >> "$out"
  pandoc "$src" \
    --from gfm \
    --to html5 \
    --no-highlight \
    2>/dev/null >> "$out"
  echo '</div>' >> "$out"
  footer >> "$out"
  echo "  → $out"
}

render_index() {
  local out="$OUT_DIR/index.html"
  header "Salt Blog" > "$out"
  cat << INDEX >> "$out"
<div class="blog-body">
  <h1>Salt Blog</h1>
  <p style="color: var(--text-muted); font-size: 1.05rem; margin-bottom: 2rem;">
    Technical deep-dives on compiler-integrated formal verification, microkernel IPC, and memory safety without borrow checking.
  </p>

  <div class="post-list" style="margin:0;padding:0;max-width:none;">
INDEX

  for md in "$BLOG_DIR"/*.md; do
    local title
    title=$(head -5 "$md" | grep '^# ' | head -1 | sed 's/^# //')
    [ -z "$title" ] && title="$(basename "$md" .md)"
    local base; base="$(basename "$md" .md)"
    local desc
    desc=$(head -30 "$md" | grep -A1 '^\*\*Published' | tail -1 | sed 's/^[^A-Za-z]*//' | head -1)
    [ -z "$desc" ] && desc=$(head -20 "$md" | grep '^[A-Z]' | head -1 | cut -c1-120)
    cat << ENTRY >> "$out"
    <a class="post-card" href="file://$OUT_DIR/${base}.html">
      <h2>$title</h2>
      <p>$desc</p>
    </a>
ENTRY
  done

  echo '  </div>' >> "$out"
  echo '</div>' >> "$out"
  footer >> "$out"
  echo "  → $out"
}

# ── Main ────────────────────────────────────────────────
command -v pandoc >/dev/null 2>&1 || {
  echo "pandoc is required. Install it: brew install pandoc"
  exit 1
}

case "${1:-}" in
  --all)
    for md in "$BLOG_DIR"/*.md; do
      render_post "$md" "$OUT_DIR/$(basename "$md" .md).html"
    done
    render_index
    open "$OUT_DIR/index.html"
    ;;
  --open)
    src="${2:-}"
    [ -z "$src" ] && { echo "Usage: $0 --open <file.md>"; exit 1; }
    [ ! -f "$src" ] && src="$BLOG_DIR/$src"
    render_post "$src" "$OUT_DIR/$(basename "$src" .md).html"
    open "$OUT_DIR/$(basename "$src" .md).html"
    ;;
  *)
    render_index
    open "$OUT_DIR/index.html"
    ;;
esac
