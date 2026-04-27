# Notes TUI

Terminal viewer for a markdown vault with notes, images, and LaTeX formulas.

## Run

```bash
# Dev
cargo run -p notes-tui -- --vault ./vault

# Release
cargo build --release -p notes-tui
./target/release/notes-tui --vault ./vault

# With debug log at ~/.cache/notes-rs/log/notes-tui.log
cargo run -p notes-tui -- --vault ./vault --debug
```

## Controls

| Action | Keys / mouse |
|--------|--------------|
| Search by tags | `/` or click the search field |
| Exit search | `Esc` or `Enter` |
| Quit | `q` outside search, `Ctrl+Q` anywhere |
| Show all tags | `t` |
| Select note | `↑/↓`, `j/k`, click |
| Page through note list | `PgUp/PgDn` (with list focused) |
| Scroll preview | scroll wheel, `Space/b`, `Ctrl+D/Ctrl+U` |
| Focus the note list | `Ctrl+L` |
| Reload vault | `Ctrl+R` |
| Cycle images in the note | `[` / `]` |
| Open selected image externally | `o` or `Ctrl+O` |
| Fullscreen image | click image, `Esc` to close |

## Vault layout

```text
vault/
├── notes/
│   ├── 01-example.md
│   └── ...
└── images/
    ├── example.png
    └── ...
```

Notes are markdown files with optional YAML frontmatter:

```markdown
---
title: GraphRAG
tags: [graphrag, retrieval]
---

# GraphRAG

Text, tables, images, and formulas.
```

## Terminal support

Inline images require a Kitty graphics protocol or Sixel-capable host (Ghostty on macOS, Windows Terminal 1.22+ on Windows). Other terminals fall back to halfblocks automatically.

## License

MIT – see [LICENSE](LICENSE).
