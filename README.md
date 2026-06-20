<h1 align="center">mq-tui</h1>

[![ci](https://github.com/harehare/mq-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/harehare/mq-tui/actions/workflows/ci.yml)
[![mq language](https://img.shields.io/badge/mq-language-orange.svg)](https://github.com/harehare/mq)

<div align="center">

Interactive terminal interface for querying and manipulating Markdown content

![demo](./assets/demo.gif)

</div>

## Overview

`mq-tui` is a Text-based User Interface for the [mq](https://github.com/harehare/mq) Markdown processor. It provides an interactive terminal experience for querying, filtering, and exploring Markdown documents using the mq query language.

### Key Features

- 🔍 **Interactive Query Mode** - Real-time Markdown querying with instant results
- 🌳 **Tree View** - Visual exploration of Markdown document structure
- 👀 **Rendered Preview** - View Markdown rendered close to its final look, right in the terminal
- 📺 **Watch Mode** - Automatically reload files when they change on disk
- 📑 **Multi-file Tabs** - Open several Markdown files at once and switch between them
- ⚡ **Vim-style Navigation** - Efficient keyboard shortcuts (j/k, hjkl)
- 📋 **Clipboard Integration** - Copy results directly to clipboard
- 🎨 **Syntax Highlighting** - Color-coded display of different Markdown elements
- 📖 **Detail View** - Inspect individual elements in depth
- 🔄 **Query History** - Navigate through previous queries
- 🎯 **fx-inspired UX** - Familiar interface for JSON query tool users

## Installation

### Using the Installation Script (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/harehare/mq-tui/main/bin/install.sh | bash
# Install from crates.io
cargo install mq-tui
# Install using binstall
cargo binstall mq-tui@0.1.3
```

The installer will:
- Download the latest release for your platform
- Verify the binary with SHA256 checksum
- Install to `~/.mq-tui/bin/`
- Update your shell profile (bash, zsh, or fish)

After installation, restart your terminal or run:
```bash
source ~/.bashrc  # or ~/.zshrc, or ~/.config/fish/config.fish
```

### From Source

```bash
git clone https://github.com/harehare/mq-tui.git
cd mq-tui
cargo build --release
# Binary will be at target/release/mq-tui
```

### Supported Platforms

- **Linux**: x86_64, aarch64
- **macOS**: x86_64 (Intel), aarch64 (Apple Silicon)
- **Windows**: x86_64

## Usage

### Basic Usage

```bash
# Open a Markdown file
mq-tui README.md
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

### Rendered Preview

Press `p` to switch to a rendered preview of the active document - headings,
bold/italic text, lists, blockquotes, code blocks, tables, and links are
styled to look close to their final rendered form instead of raw Markdown
syntax. Use `↑`/`k`, `↓`/`j`, `PageUp`/`PageDown`, or `g`/`G` to scroll, and
press `p` or `Esc` to return to normal mode.

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

### Normal Mode

| Key         | Action                               |
| ----------- | ------------------------------------ |
| `q` / `Esc` | Quit the application                 |
| `:`         | Enter query mode                     |
| `?` / `F1`  | Show help screen                     |
| `t`         | Toggle tree view mode                |
| `p`         | Toggle rendered preview mode          |
| `d`         | Toggle detail view for selected item |
| `y`         | Copy results to clipboard            |
| `Ctrl+L`    | Clear current query                  |
| `o`         | Open a file as a new tab             |
| `←` / `→`   | Switch tabs (when multiple files are open) |
| `Tab` / `Shift+Tab` | Switch tabs (when multiple files are open) |

### Navigation

| Key        | Action               |
| ---------- | -------------------- |
| `↑` / `k`  | Move up              |
| `↓` / `j`  | Move down            |
| `PageUp`   | Page up (10 items)   |
| `PageDown` | Page down (10 items) |
| `Home`     | Jump to first item   |
| `End`      | Jump to last item    |

### Query Mode

| Key                    | Action                                  |
| ---------------------- | --------------------------------------- |
| `Enter`                | Execute query and return to normal mode |
| `Esc`                  | Exit query mode without executing       |
| `↑` / `↓`              | Navigate query history                  |
| `←` / `→`              | Move cursor in query string             |
| `Home` / `End`         | Jump to start/end of query              |
| `Backspace` / `Delete` | Edit query text                         |

### Tree View Mode

| Key               | Action               |
| ----------------- | -------------------- |
| `↑` / `k`         | Move up in tree      |
| `↓` / `j`         | Move down in tree    |
| `Enter` / `Space` | Expand/collapse node |
| `←` / `→`         | Switch tabs (when multiple files are open) |
| `Esc` / `t`       | Exit tree view       |
| `?` / `F1`        | Show help            |

### Open File Mode

| Key                    | Action                          |
| ---------------------- | -------------------------------- |
| `Enter`                | Open the typed path as a new tab |
| `Esc`                  | Cancel                           |
| `←` / `→`              | Move cursor in path string       |
| `Home` / `End`         | Jump to start/end of path        |
| `Backspace` / `Delete` | Edit path text                   |

### Preview Mode

| Key             | Action                       |
| --------------- | ----------------------------- |
| `↑` / `k`       | Scroll up                     |
| `↓` / `j`       | Scroll down                   |
| `PageUp`        | Scroll up (10 lines)          |
| `PageDown`      | Scroll down (10 lines)        |
| `g`             | Jump to top                   |
| `G`             | Jump to bottom                |
| `←` / `→`       | Switch tabs (when multiple files are open) |
| `Esc` / `p`     | Exit preview                  |

## Modes

### Normal Mode

Default mode for navigating and viewing query results. Use arrow keys or Vim-style navigation to browse through results.

### Query Mode

Activated by pressing `:`. Type your mq query and press Enter to execute. The query is evaluated in real-time as you type.

### Tree View Mode

Activated by pressing `t`. Displays the Markdown document structure as an expandable tree, showing the hierarchy of headings, lists, and other elements.

### Preview Mode

Activated by pressing `p`. Renders the active document's Markdown source close to its final look - styled headings, emphasis, lists, blockquotes, code blocks, tables, and links - instead of raw Markdown syntax.

### Help Mode

Activated by pressing `?` or `F1`. Displays all available keyboard shortcuts and commands.

## Features in Detail

### Real-time Query Execution

Queries are executed as you type, providing immediate feedback and results.

### Detail View

Press `d` to toggle between list view and split view. In split view, the left pane shows the result list while the right pane displays detailed information about the selected item.

### Query History

All executed queries are saved in history. Use `↑` and `↓` in query mode to navigate through previous queries.

### Clipboard Support

Press `y` to copy the current query results to your system clipboard in Markdown format.

### Tree Visualization

The tree view mode provides a visual representation of your Markdown document's structure, with color-coded elements:

- 🔵 **Blue**: Headings
- 🟢 **Green**: Lists
- 🔴 **Red**: Math expressions
- 🟣 **Magenta**: Links
- 🟡 **Yellow**: Images
- 🔵 **Cyan**: Code blocks

### Watch Mode

Run with `--watch` to keep open files in sync with disk. This is handy when
editing a Markdown file in another editor while exploring it with `mq-tui` -
save your changes and they show up immediately, with the current query
re-applied to the updated content.

## Configuration

`mq-tui` works out of the box with sensible defaults. The UI adapts to your terminal's color scheme and size.

## Related Projects

- [mq](https://github.com/harehare/mq) - The underlying Markdown query processor
- [mq-view](https://github.com/harehare/mq-view) - Markdown viewer with syntax highlighting
- [mqlang.org](https://mqlang.org) - Documentation and language reference

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
