# todui

TUI todo + time tracking in rust, using [ratatui](https://ratatui.rs).

## Features

- Multiple lists with sidebar navigation
- Add, edit, delete, reorder todos and lists
- Mark todos done/not done, toggle visibility of completed items
- Multi-select items with batch delete, toggle done, and move
- Move items between lists with type-ahead-find picker
- Tag system with `@tag` syntax and autocomplete
- Filter by tag (OR logic, multi-select)
- Fuzzy search across all lists, tags, and items
- Focus mode with pause/resume timer and per-item time tracking
- Vim-style keybindings (j/k, gg/G, Ctrl+D/U)
- Undo/redo
- ASCII mode fallback
- File-based persistence (Markdown)
- Single-instance lock file
- Items in lists marked as _daily_ will reset each day

## License

Copyright (c) Stephen Beck <2529298+altphi@users.noreply.github.com>

This project is licensed under the MIT license ([LICENSE] or <http://opensource.org/licenses/MIT>)

[LICENSE]: ./LICENSE

