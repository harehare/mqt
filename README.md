<h1 align="center">mq-tui</h1>

[![ci](https://github.com/harehare/mq-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/harehare/mq-tui/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/mq-tui?logo=rust)](https://crates.io/crates/mq-tui)
[![license](https://img.shields.io/crates/l/mq-tui)](LICENSE)

<div align="center">

Interactive terminal interface for querying and manipulating Markdown content

![demo](./assets/demo.gif)

</div>

## Overview

`mq-tui` is a Text-based User Interface for the [mq](https://github.com/harehare/mq) Markdown processor. It provides an interactive terminal experience for querying, filtering, and exploring Markdown documents using the mq query language.

## Key Features

- 🔍 **Interactive Query Mode** - Real-time Markdown querying with instant results
- 🌳 **Tree View** - Visual exploration of Markdown document structure
- 👀 **Rendered Preview** - View Markdown rendered close to its final look, right in the terminal
- 🪟 **Split Preview** - Show raw source side-by-side with the rendered preview
- 📺 **Watch Mode** - Automatically reload files when they change on disk
- 📑 **Multi-file Tabs** - Open several Markdown files at once and switch between them
- ⚡ **Vim-style Navigation** - Efficient keyboard shortcuts (j/k, hjkl)
- 📋 **Clipboard Integration** - Copy results directly to clipboard
- 🎨 **Syntax Highlighting** - Color-coded display of different Markdown elements
- 📖 **Detail View** - Inspect individual elements in depth
- 🔄 **Query History** - Navigate through previous queries
- 🎯 **fx-inspired UX** - Familiar interface for JSON query tool users

## Installation

### Quick Install

```bash
curl -fsSL https://raw.githubusercontent.com/harehare/mq-tui/main/bin/install.sh | bash
```

Downloads the latest release for your platform, verifies it with a SHA256 checksum, installs it to `~/.mq-tui/bin/`, and updates your shell profile (bash, zsh, or fish).

After installation, restart your terminal or run:

```bash
source ~/.bashrc  # or ~/.zshrc, or ~/.config/fish/config.fish
```

### Package Managers

| Method            | Command                 |
| ----------------- | ----------------------- |
| Cargo (crates.io) | `cargo install mq-tui`  |
| Cargo (binstall)  | `cargo binstall mq-tui` |

<details>
<summary>More install options: building from source, supported platforms</summary>

```bash
git clone https://github.com/harehare/mq-tui.git
cd mq-tui
cargo build --release
# Binary will be at target/release/mq-tui
```

Supported platforms:

- **Linux**: x86_64, aarch64
- **macOS**: x86_64 (Intel), aarch64 (Apple Silicon)
- **Windows**: x86_64

</details>

## Usage

### Basic Usage

```bash
# Open a Markdown file
mq-tui README.md

# Read from stdin
cat README.md | mq-tui

# Launch via mq's external subcommand mechanism
mq tui README.md
```

### Multiple Files (Tabs)

Open several Markdown files at once; each one becomes a tab:

```bash
mq-tui README.md CHANGELOG.md docs/*.md
```

A tab bar appears at the top whenever more than one file is open. Switch
tabs with `←` / `→` or `Tab` / `Shift+Tab`. The query box is shared across
all tabs: whatever query you run is applied to every open file at once, so
switching tabs shows that file's own filtered results without retyping the
query. Press `o` at any time to open another file as a new tab.

### Tree View

Press `t` to display the Markdown document structure as an expandable tree,
showing the hierarchy of headings, lists, and other elements, color-coded by
type:

- 🔵 **Blue**: Headings
- 🟢 **Green**: Lists
- 🔴 **Red**: Math expressions
- 🟣 **Magenta**: Links
- 🟡 **Yellow**: Images
- 🔵 **Cyan**: Code blocks

### Rendered Preview

Press `p` to switch to a rendered preview of the active document - headings,
bold/italic text, lists, blockquotes, code blocks, tables, and links are
styled to look close to their final rendered form instead of raw Markdown
syntax. Use `↑`/`k`, `↓`/`j`, `PageUp`/`PageDown`, or `g`/`G` to scroll, and
press `p` or `Esc` to return to normal mode. Press `s` while in preview mode
to split the view and show the raw Markdown source side-by-side with the
rendered output, scrolling in sync.

### Detail View

Press `d` to toggle between list view and split view. In split view, the
left pane shows the result list while the right pane displays detailed
information about the selected item.

### Query History

Every executed query is saved in history. Use `↑` and `↓` in query mode to
navigate through previous queries.

### Clipboard Integration

Press `y` to copy the current query results, or `Y` to copy just the
selected row, to your system clipboard in Markdown format.

### Watch Mode

Pass `--watch` (or `-w`) to automatically reload files when they change on disk:

```bash
mq-tui --watch README.md
```

The status line shows a `👀 watching` indicator while watch mode is active.
Each open file is watched using your OS's native file system notifications
(inotify on Linux, FSEvents on macOS, ReadDirectoryChangesW on Windows) -
no polling involved, so changes are picked up almost instantly. When a file
is modified externally (e.g. saved from your editor, including atomic
save-and-rename), its content is reloaded and the current query is re-run
automatically. Watch mode is not available when reading from stdin.

### Query Examples

Once in the TUI, press `:` to enter query mode and try these queries:

```mq
# Select all headings
.h

# Select level 2 headings
.h | select(.depth == 2)

# Select all links
.link

# Select code blocks with specific language
.code | select(.lang == "rust")

# Select list items
.list

# Complex filtering
.h | select(.depth >= 2 and .depth <= 3)
```

## Key Bindings

| Key         | Action                      |
| ----------- | --------------------------- |
| `:`         | Enter query mode            |
| `t`         | Toggle tree view            |
| `p`         | Toggle rendered preview     |
| `d`         | Toggle detail view          |
| `y` / `Y`   | Copy results / selected row |
| `o`         | Open a file as a new tab    |
| `?` / `F1`  | Show the full help screen   |
| `q` / `Esc` | Quit the application        |

Press `?` or `F1` at any time in the app for the complete, mode-specific list of keyboard shortcuts.

## Configuration

`mq-tui` works out of the box with sensible defaults. The UI adapts to your terminal's color scheme and size.

## Related Projects

- [mq](https://github.com/harehare/mq) - The underlying Markdown query processor
- [mq-view](https://github.com/harehare/mq-view) - Markdown viewer with syntax highlighting
- [mqlang.org](https://mqlang.org) - Documentation and language reference

## Support

- 🐛 [Report bugs](https://github.com/harehare/mq-tui/issues/new)
- 💡 [Request features](https://github.com/harehare/mq-tui/issues/new)
- ⭐ [Star the project](https://github.com/harehare/mq-tui) if you find it useful!

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
