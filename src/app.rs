use arboard::Clipboard;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};
use miette::IntoDiagnostic;
use mq_lang::Engine;
use mq_markdown::Markdown;
use ratatui::prelude::*;
use std::{
    fmt::Display,
    io::Stdout,
    time::{Duration, Instant},
};

use crate::{
    event::{EventHandler, EventHandlerExt},
    ui::{draw_ui, treeview::TreeView},
    util,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Query,
    Help,
    TreeView,
    OpenFile,
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Normal => write!(f, "NORMAL"),
            Mode::Query => write!(f, "QUERY"),
            Mode::Help => write!(f, "HELP"),
            Mode::TreeView => write!(f, "TREE VIEW"),
            Mode::OpenFile => write!(f, "OPEN FILE"),
        }
    }
}

/// A single open Markdown document (tab).
struct Document {
    /// The Markdown content to process
    content: String,
    /// Filename (if loaded from a file)
    filename: Option<String>,
    /// The current results from the query
    results: Vec<mq_markdown::Node>,
    /// Currently selected result index
    selected_idx: usize,
    /// Error message if the query/parse fails for this document
    error_msg: Option<String>,
    /// All parsed markdown nodes (for section extraction)
    all_nodes: Vec<mq_markdown::Node>,
    /// Sidebar tree view (headers only)
    sidebar_tree_view: Option<TreeView>,
}

impl Document {
    fn new(content: String, filename: Option<String>) -> Self {
        let mut doc = Self {
            content,
            filename,
            results: Vec::new(),
            selected_idx: 0,
            error_msg: None,
            all_nodes: Vec::new(),
            sidebar_tree_view: None,
        };
        doc.init_sidebar_tree_view();
        doc
    }

    fn display_name(&self) -> &str {
        self.filename.as_deref().unwrap_or("untitled")
    }

    fn init_sidebar_tree_view(&mut self) {
        let markdown_result = Markdown::from_markdown_str(&self.content);
        match markdown_result {
            Ok(markdown) => {
                // Store all nodes for section extraction
                self.all_nodes = markdown.nodes.clone();

                // Extract only heading nodes for the sidebar
                let headers: Vec<mq_markdown::Node> = markdown
                    .nodes
                    .into_iter()
                    .filter(|node| matches!(node, mq_markdown::Node::Heading(_)))
                    .collect();

                if !headers.is_empty() {
                    let mut tree_view = TreeView::new(headers);
                    tree_view.rebuild_items_with_all_documents(true);
                    self.sidebar_tree_view = Some(tree_view);
                }
            }
            Err(_) => {
                // Silently fail for sidebar initialization
            }
        }
    }

    /// Extract section content from heading to the next same-level heading
    fn extract_section_content(&self, heading: &mq_markdown::Heading) -> Vec<mq_markdown::Node> {
        let mut section_nodes = Vec::new();
        let mut found_heading = false;
        let target_depth = heading.depth;

        for node in &self.all_nodes {
            if !found_heading {
                // Check if this is our target heading
                if let mq_markdown::Node::Heading(h) = node
                    && h.depth == heading.depth
                {
                    let h_text: String = h.values.iter().map(|n| n.value()).collect();
                    let target_text: String = heading.values.iter().map(|n| n.value()).collect();
                    if h_text == target_text {
                        found_heading = true;
                        section_nodes.push(node.clone());
                    }
                }
            } else {
                // After finding the heading, collect nodes until next same-level heading
                if let mq_markdown::Node::Heading(h) = node
                    && h.depth <= target_depth
                {
                    // Found next same-level or higher-level heading, stop
                    break;
                }
                section_nodes.push(node.clone());
            }
        }

        section_nodes
    }
}

/// What the sidebar selection resolved to, used to update the active document's results.
enum SidebarSelection {
    AllDocuments,
    Heading(mq_markdown::Heading, String),
}

pub struct App {
    /// Open documents (tabs)
    documents: Vec<Document>,
    /// Index of the currently active document
    active_doc: usize,
    /// The query to run on the Markdown content (shared across all open documents)
    query: String,
    /// Last query execution time (total, across all open documents)
    last_exec_time: Duration,
    /// Last query execution timestamp
    last_exec: Instant,
    /// Should the application exit
    should_quit: bool,
    /// Transient error message not tied to a specific document (e.g. clipboard failures)
    transient_error: Option<String>,
    /// Current app mode
    mode: Mode,
    /// Show detailed view of selected item
    show_detail: bool,
    /// History of executed queries
    query_history: Vec<String>,
    /// Current position in query history
    history_position: Option<usize>,
    /// Current cursor position in query string
    cursor_position: usize,
    /// Tree view component for the active document (Mode::TreeView)
    tree_view: Option<TreeView>,
    /// Show tree sidebar in Normal mode
    show_tree_sidebar: bool,
    /// Whether a query execution is pending (for debouncing)
    query_pending: bool,
    /// Debounce duration for query execution
    debounce_duration: Duration,
    /// Path currently being typed in Mode::OpenFile
    open_file_path: String,
    /// Cursor position within open_file_path
    open_file_cursor: usize,
}

impl App {
    pub fn new(content: String) -> Self {
        Self::from_documents(vec![Document::new(content, None)])
    }

    pub fn with_file(content: String, filename: String) -> Self {
        Self::from_documents(vec![Document::new(content, Some(filename))])
    }

    /// Create an App with multiple open documents (tabs), each as (content, filename).
    pub fn with_files(files: Vec<(String, String)>) -> Self {
        let documents = files
            .into_iter()
            .map(|(content, filename)| Document::new(content, Some(filename)))
            .collect();
        Self::from_documents(documents)
    }

    fn from_documents(documents: Vec<Document>) -> Self {
        Self {
            documents,
            active_doc: 0,
            query: ".".to_string(),
            last_exec_time: Duration::from_millis(0),
            last_exec: Instant::now(),
            should_quit: false,
            transient_error: None,
            mode: Mode::Normal,
            show_detail: false,
            query_history: Vec::new(),
            history_position: None,
            cursor_position: 0,
            tree_view: None,
            show_tree_sidebar: false,
            query_pending: false,
            debounce_duration: Duration::from_millis(300),
            open_file_path: String::new(),
            open_file_cursor: 0,
        }
    }

    fn active_doc(&self) -> &Document {
        &self.documents[self.active_doc]
    }

    fn active_doc_mut(&mut self) -> &mut Document {
        &mut self.documents[self.active_doc]
    }

    pub fn run(&mut self) -> miette::Result<()> {
        let mut terminal = util::setup_terminal()?;
        let events = EventHandler::new(Duration::from_millis(100));

        self.exec_query();

        while !self.should_quit {
            self.draw(&mut terminal)?;

            if let Some(event) = events.next()? {
                self.handle_event(event)?;
            }

            // Check if we should execute a pending query (debounce)
            if self.query_pending && self.last_exec.elapsed() >= self.debounce_duration {
                self.exec_query();
            }
        }

        util::restore_terminal()?;

        Ok(())
    }

    fn draw(&self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> miette::Result<()> {
        terminal
            .draw(|frame| draw_ui(frame, self))
            .into_diagnostic()?;
        Ok(())
    }

    pub fn handle_event(&mut self, event: Event) -> miette::Result<()> {
        self.transient_error = None;
        self.active_doc_mut().error_msg = None;
        match self.mode {
            Mode::Normal => self.handle_normal_mode_event(event),
            Mode::Query => self.handle_query_mode_event(event),
            Mode::Help => self.handle_help_mode_event(event),
            Mode::TreeView => self.handle_tree_view_mode_event(event),
            Mode::OpenFile => self.handle_open_file_mode_event(event),
        }
    }

    fn handle_normal_mode_event(&mut self, event: Event) -> miette::Result<()> {
        if let Event::Mouse(mouse_event) = event {
            match mouse_event.kind {
                MouseEventKind::ScrollDown if !self.active_doc().results.is_empty() => {
                    let idx = self.next_visible_from_current(true);
                    self.active_doc_mut().selected_idx = idx;
                }
                MouseEventKind::ScrollUp if !self.active_doc().results.is_empty() => {
                    let idx = self.next_visible_from_current(false);
                    self.active_doc_mut().selected_idx = idx;
                }
                _ => {}
            }
            return Ok(());
        }

        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            match (code, modifiers) {
                // Quit on Escape or q
                (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => {
                    self.should_quit = true;
                }
                // Toggle detailed view
                (KeyCode::Char('d'), _) => {
                    self.show_detail = !self.show_detail;
                }
                // Enter query mode
                (KeyCode::Char(':'), _) => {
                    self.mode = Mode::Query;
                    self.cursor_position = self.query.len();
                }
                // Show help
                (KeyCode::Char('?'), _) | (KeyCode::F(1), _) => {
                    self.mode = Mode::Help;
                }
                // Toggle tree view
                (KeyCode::Char('t'), _) => {
                    self.mode = Mode::TreeView;
                    self.init_tree_view();
                }
                // Toggle tree sidebar
                (KeyCode::Char('s'), _) => {
                    self.toggle_tree_sidebar();
                    // Update selection when sidebar is shown
                    if self.show_tree_sidebar {
                        self.update_sidebar_selection();
                    }
                }
                // Open a new file as an additional tab
                (KeyCode::Char('o'), _) => {
                    self.mode = Mode::OpenFile;
                    self.open_file_path.clear();
                    self.open_file_cursor = 0;
                }
                // Switch tabs
                (KeyCode::Right, _) | (KeyCode::Tab, _) => {
                    self.next_tab();
                }
                (KeyCode::Left, _) | (KeyCode::BackTab, _) => {
                    self.prev_tab();
                }
                // Navigate results or sidebar
                (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                    if self.show_tree_sidebar && self.active_doc().sidebar_tree_view.is_some() {
                        if let Some(sidebar) = &mut self.active_doc_mut().sidebar_tree_view {
                            sidebar.move_down();
                        }
                        self.update_sidebar_selection();
                    } else if !self.active_doc().results.is_empty() {
                        let idx = self.next_visible_from_current(true);
                        self.active_doc_mut().selected_idx = idx;
                    }
                }
                (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                    if self.show_tree_sidebar && self.active_doc().sidebar_tree_view.is_some() {
                        if let Some(sidebar) = &mut self.active_doc_mut().sidebar_tree_view {
                            sidebar.move_up();
                        }
                        self.update_sidebar_selection();
                    } else if !self.active_doc().results.is_empty() {
                        let idx = self.next_visible_from_current(false);
                        self.active_doc_mut().selected_idx = idx;
                    }
                }
                // Select header in sidebar (Enter key - content already updated by navigation)
                (KeyCode::Enter, _) => {
                    // Content is already updated by update_sidebar_selection()
                    // Enter key can be used for future actions if needed
                }
                (KeyCode::PageDown, _) if !self.active_doc().results.is_empty() => {
                    let next =
                        (self.active_doc().selected_idx + 10).min(self.active_doc().results.len() - 1);
                    let idx = self.next_visible(next, true);
                    self.active_doc_mut().selected_idx = idx;
                }
                (KeyCode::PageUp, _) if !self.active_doc().results.is_empty() => {
                    let prev = self.active_doc().selected_idx.saturating_sub(10);
                    let idx = self.next_visible(prev, false);
                    self.active_doc_mut().selected_idx = idx;
                }
                (KeyCode::Home, _) if !self.active_doc().results.is_empty() => {
                    let idx = self.next_visible(0, true);
                    self.active_doc_mut().selected_idx = idx;
                }
                (KeyCode::End, _) if !self.active_doc().results.is_empty() => {
                    let last = self.active_doc().results.len() - 1;
                    let idx = self.next_visible(last, false);
                    self.active_doc_mut().selected_idx = idx;
                }
                // Jump to first/last result (vim-style)
                (KeyCode::Char('g'), _) if !self.active_doc().results.is_empty() => {
                    let idx = self.next_visible(0, true);
                    self.active_doc_mut().selected_idx = idx;
                }
                (KeyCode::Char('G'), _) if !self.active_doc().results.is_empty() => {
                    let last = self.active_doc().results.len() - 1;
                    let idx = self.next_visible(last, false);
                    self.active_doc_mut().selected_idx = idx;
                }
                // Clear query with Ctrl+L
                (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                    self.query.clear();
                    self.cursor_position = 0;
                    self.exec_query();
                }
                (KeyCode::Char('y'), _) if !self.active_doc().results.is_empty() => {
                    let result_text =
                        mq_markdown::Markdown::new(self.active_doc().results.clone()).to_string();
                    if let Ok(mut clipboard) = Clipboard::new() {
                        if clipboard.set_text(result_text).is_ok() {
                        } else {
                            self.transient_error =
                                Some("Error: Could not copy to clipboard".to_string());
                        }
                    } else {
                        self.transient_error = Some("Error: Could not access clipboard".to_string());
                    }
                }
                (KeyCode::Char('Y'), _) if !self.active_doc().results.is_empty() => {
                    let current_text = self
                        .active_doc()
                        .results
                        .get(self.active_doc().selected_idx)
                        .map(|node| node.to_string())
                        .unwrap_or_default();
                    if let Ok(mut clipboard) = Clipboard::new() {
                        if clipboard.set_text(current_text).is_ok() {
                        } else {
                            self.transient_error =
                                Some("Error: Could not copy to clipboard".to_string());
                        }
                    } else {
                        self.transient_error = Some("Error: Could not access clipboard".to_string());
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_query_mode_event(&mut self, event: Event) -> miette::Result<()> {
        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            match (code, modifiers) {
                // Exit query mode on Escape
                (KeyCode::Esc, _) => {
                    self.mode = Mode::Normal;
                    self.history_position = None;
                }
                // Execute query on Enter
                (KeyCode::Enter, _) => {
                    self.mode = Mode::Normal;
                    if !self.query.is_empty() {
                        // Add query to history if it's not a duplicate
                        if self.query_history.is_empty()
                            || self.query_history.last() != Some(&self.query)
                        {
                            self.query_history.push(self.query.clone());
                        }
                    }
                    self.history_position = None;
                    self.query_pending = false;
                    self.exec_query();
                }
                // Switch tabs without leaving query mode
                (KeyCode::Tab, _) => {
                    self.next_tab();
                }
                (KeyCode::BackTab, _) => {
                    self.prev_tab();
                }
                // Edit query
                (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    self.query.insert(self.cursor_position, c);
                    self.cursor_position += 1;
                    self.last_exec = Instant::now();
                    self.query_pending = true;
                }
                (KeyCode::Backspace, _) if self.cursor_position > 0 => {
                    self.query.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                    self.last_exec = Instant::now();
                    self.query_pending = true;
                }
                (KeyCode::Delete, _) if self.cursor_position < self.query.len() => {
                    self.query.remove(self.cursor_position);
                    self.last_exec = Instant::now();
                    self.query_pending = true;
                }
                // Move cursor
                (KeyCode::Left, _) if self.cursor_position > 0 => {
                    self.cursor_position -= 1;
                }
                (KeyCode::Right, _) if self.cursor_position < self.query.len() => {
                    self.cursor_position += 1;
                }
                (KeyCode::Home, _) => {
                    self.cursor_position = 0;
                }
                (KeyCode::End, _) => {
                    self.cursor_position = self.query.len();
                }
                // Navigate history
                (KeyCode::Up, _) if !self.query_history.is_empty() => {
                    match self.history_position {
                        None => {
                            self.history_position = Some(self.query_history.len() - 1);
                            self.query = self.query_history[self.history_position.unwrap()].clone();
                        }
                        Some(pos) if pos > 0 => {
                            self.history_position = Some(pos - 1);
                            self.query = self.query_history[self.history_position.unwrap()].clone();
                        }
                        _ => {}
                    }
                    self.cursor_position = self.query.len();
                }
                (KeyCode::Down, _) => {
                    if let Some(pos) = self.history_position {
                        if pos < self.query_history.len() - 1 {
                            self.history_position = Some(pos + 1);
                            self.query = self.query_history[self.history_position.unwrap()].clone();
                        } else {
                            self.history_position = None;
                            self.query.clear();
                        }
                        self.cursor_position = self.query.len();
                    }
                }

                _ => {}
            }
        }

        Ok(())
    }

    fn handle_help_mode_event(&mut self, event: Event) -> miette::Result<()> {
        if let Event::Key(KeyEvent {
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            self.mode = Mode::Normal;
        }

        Ok(())
    }

    fn handle_tree_view_mode_event(&mut self, event: Event) -> miette::Result<()> {
        if let Event::Mouse(mouse_event) = event {
            match mouse_event.kind {
                MouseEventKind::ScrollDown => {
                    if let Some(tree_view) = &mut self.tree_view {
                        tree_view.move_down();
                    }
                }
                MouseEventKind::ScrollUp => {
                    if let Some(tree_view) = &mut self.tree_view {
                        tree_view.move_up();
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            match (code, modifiers) {
                // Exit tree view mode
                (KeyCode::Esc, _) | (KeyCode::Char('t'), _) => {
                    self.mode = Mode::Normal;
                }
                // Quit
                (KeyCode::Char('q'), _) => {
                    self.should_quit = true;
                }
                // Switch tabs
                (KeyCode::Right, _) | (KeyCode::Tab, _) => {
                    self.next_tab();
                    self.init_tree_view();
                }
                (KeyCode::Left, _) | (KeyCode::BackTab, _) => {
                    self.prev_tab();
                    self.init_tree_view();
                }
                // Navigation
                (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                    if let Some(tree_view) = &mut self.tree_view {
                        tree_view.move_down();
                    }
                }
                (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                    if let Some(tree_view) = &mut self.tree_view {
                        tree_view.move_up();
                    }
                }
                // Toggle expand/collapse
                (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => {
                    if let Some(tree_view) = &mut self.tree_view {
                        tree_view.toggle_expand();
                    }
                }
                // Jump to first/last item (vim-style)
                (KeyCode::Char('g'), _) => {
                    if let Some(tree_view) = &mut self.tree_view {
                        tree_view.move_to_first();
                    }
                }
                (KeyCode::Char('G'), _) => {
                    if let Some(tree_view) = &mut self.tree_view {
                        tree_view.move_to_last();
                    }
                }
                // Show help
                (KeyCode::Char('?'), _) | (KeyCode::F(1), _) => {
                    self.mode = Mode::Help;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_open_file_mode_event(&mut self, event: Event) -> miette::Result<()> {
        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            match (code, modifiers) {
                // Cancel
                (KeyCode::Esc, _) => {
                    self.mode = Mode::Normal;
                    self.open_file_path.clear();
                    self.open_file_cursor = 0;
                }
                // Open the file
                (KeyCode::Enter, _) => {
                    self.open_file();
                }
                // Edit path
                (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    self.open_file_path.insert(self.open_file_cursor, c);
                    self.open_file_cursor += 1;
                }
                (KeyCode::Backspace, _) if self.open_file_cursor > 0 => {
                    self.open_file_path.remove(self.open_file_cursor - 1);
                    self.open_file_cursor -= 1;
                }
                (KeyCode::Delete, _) if self.open_file_cursor < self.open_file_path.len() => {
                    self.open_file_path.remove(self.open_file_cursor);
                }
                (KeyCode::Left, _) if self.open_file_cursor > 0 => {
                    self.open_file_cursor -= 1;
                }
                (KeyCode::Right, _) if self.open_file_cursor < self.open_file_path.len() => {
                    self.open_file_cursor += 1;
                }
                (KeyCode::Home, _) => {
                    self.open_file_cursor = 0;
                }
                (KeyCode::End, _) => {
                    self.open_file_cursor = self.open_file_path.len();
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Read the typed path, open it as a new tab, and switch to it.
    fn open_file(&mut self) {
        let path = self.open_file_path.trim().to_string();
        self.mode = Mode::Normal;

        if path.is_empty() {
            return;
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let filename = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&path)
                    .to_string();
                self.documents.push(Document::new(content, Some(filename)));
                self.active_doc = self.documents.len() - 1;
                self.open_file_path.clear();
                self.open_file_cursor = 0;
                self.exec_query();
            }
            Err(err) => {
                self.transient_error = Some(format!("Could not open '{}': {}", path, err));
            }
        }
    }

    fn init_tree_view(&mut self) {
        let markdown_result = Markdown::from_markdown_str(&self.active_doc().content);
        match markdown_result {
            Ok(markdown) => {
                self.tree_view = Some(TreeView::new(markdown.nodes));
            }
            Err(_) => {
                self.transient_error = Some("Failed to parse markdown for tree view".to_string());
            }
        }
    }

    /// Update results based on current sidebar selection
    fn update_sidebar_selection(&mut self) {
        if !self.show_tree_sidebar {
            return;
        }

        let active = self.active_doc;
        let selection = self.documents[active]
            .sidebar_tree_view
            .as_ref()
            .and_then(|sidebar| {
                let selected_item = sidebar.items().get(sidebar.selected_index())?;
                if selected_item.is_all_documents {
                    return Some(SidebarSelection::AllDocuments);
                }
                let selected_node = sidebar.get_selected_node()?;
                if let mq_markdown::Node::Heading(heading) = selected_node {
                    Some(SidebarSelection::Heading(
                        heading.clone(),
                        selected_node.value(),
                    ))
                } else {
                    None
                }
            });

        match selection {
            Some(SidebarSelection::AllDocuments) => {
                self.query.clear();
                self.cursor_position = 0;
                self.exec_query();
            }
            Some(SidebarSelection::Heading(heading, value)) => {
                let section_content = self.documents[active].extract_section_content(&heading);
                self.query = format!(
                    r#"import "section" | nodes | section::split({}) | section::title_contains("{}") | section::collect()"#,
                    heading.depth, value
                );
                self.documents[active].results = section_content;
                self.documents[active].selected_idx = 0;
                self.cursor_position = self.query.len();
            }
            None => {}
        }
    }

    /// Execute the current query against every open document.
    pub fn exec_query(&mut self) {
        self.query_pending = false;
        let start = Instant::now();

        for doc in self.documents.iter_mut() {
            let mut engine: Engine = Engine::default();
            engine.load_builtin_module();
            let markdown_result = Markdown::from_markdown_str(&doc.content);
            match markdown_result {
                Ok(markdown) => {
                    if !self.query.is_empty() {
                        let md_nodes = markdown
                            .nodes
                            .into_iter()
                            .map(mq_lang::RuntimeValue::from)
                            .collect::<Vec<_>>();

                        match engine.eval(&self.query, md_nodes.into_iter()) {
                            Ok(results) => {
                                doc.results = results
                                    .into_iter()
                                    .map(|runtime_value| match runtime_value {
                                        mq_lang::RuntimeValue::Markdown(node, _) => *node,
                                        _ => runtime_value.to_string().into(),
                                    })
                                    .collect();
                                doc.error_msg = None;
                            }
                            Err(err) => {
                                doc.error_msg = Some(format!("Query error: {}", err));
                                // Keep previous results
                            }
                        }
                    } else {
                        // Show all nodes when query is empty
                        doc.results = markdown.nodes;
                        doc.error_msg = None;
                    }
                }
                Err(err) => {
                    doc.error_msg = Some(format!("Markdown parse error: {}", err));
                    doc.results = Vec::new();
                }
            }

            // Reset selected index if it's now out of bounds
            if doc.selected_idx >= doc.results.len() {
                doc.selected_idx = doc.results.len().saturating_sub(1);
            }

            // Advance past invisible nodes (nodes that render to nothing in combined output)
            if !doc.results.is_empty() {
                doc.selected_idx = Self::next_visible_in(&doc.results, doc.selected_idx, true);
            }
        }

        self.last_exec_time = start.elapsed();
        self.last_exec = Instant::now();
    }

    /// Get the current query string
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Get the current results (for the active document)
    pub fn results(&self) -> &[mq_markdown::Node] {
        &self.active_doc().results
    }

    /// Get the currently selected result index (for the active document)
    pub fn selected_idx(&self) -> usize {
        self.active_doc().selected_idx
    }

    /// Get the last execution time
    pub fn last_exec_time(&self) -> Duration {
        self.last_exec_time
    }

    /// Get the current error message, if any (for the active document)
    pub fn error_msg(&self) -> Option<&str> {
        self.transient_error
            .as_deref()
            .or_else(|| self.active_doc().error_msg.as_deref())
    }

    /// Get the current app mode
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Check if detailed view is enabled
    pub fn show_detail(&self) -> bool {
        self.show_detail
    }

    /// Get the cursor position in the query
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Get the filename of the active document, if any
    pub fn filename(&self) -> Option<&str> {
        self.active_doc().filename.as_deref()
    }

    /// Get the query history
    pub fn query_history(&self) -> &[String] {
        &self.query_history
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.cursor_position = self.query.len();
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    #[cfg(test)]
    pub fn set_results(&mut self, results: Vec<mq_markdown::Node>) {
        self.active_doc_mut().results = results;
    }

    #[cfg(test)]
    pub fn set_last_exec_time(&mut self, duration: Duration) {
        self.last_exec_time = duration;
    }

    #[cfg(test)]
    pub fn set_error_msg(&mut self, msg: String) {
        self.transient_error = Some(msg);
    }

    #[cfg(test)]
    pub fn set_cursor_position(&mut self, position: usize) {
        self.cursor_position = position;
    }

    /// Get the tree view, if available
    pub fn tree_view(&self) -> Option<&TreeView> {
        self.tree_view.as_ref()
    }

    /// Check if tree sidebar is shown
    pub fn show_tree_sidebar(&self) -> bool {
        self.show_tree_sidebar
    }

    /// Get the sidebar tree view of the active document, if available
    pub fn sidebar_tree_view(&self) -> Option<&TreeView> {
        self.active_doc().sidebar_tree_view.as_ref()
    }

    /// Get mutable reference to the active document's sidebar tree view
    pub fn sidebar_tree_view_mut(&mut self) -> Option<&mut TreeView> {
        self.active_doc_mut().sidebar_tree_view.as_mut()
    }

    /// Toggle tree sidebar visibility
    pub fn toggle_tree_sidebar(&mut self) {
        self.show_tree_sidebar = !self.show_tree_sidebar;
    }

    /// Number of currently open documents (tabs)
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Index of the currently active document (tab)
    pub fn active_doc_index(&self) -> usize {
        self.active_doc
    }

    /// Display names (filenames) for all open documents, in order
    pub fn document_names(&self) -> Vec<&str> {
        self.documents.iter().map(|d| d.display_name()).collect()
    }

    /// Path currently being typed in Mode::OpenFile
    pub fn open_file_path(&self) -> &str {
        &self.open_file_path
    }

    /// Cursor position within the open-file path input
    pub fn open_file_cursor(&self) -> usize {
        self.open_file_cursor
    }

    /// Switch to the next tab (document), wrapping around
    pub fn next_tab(&mut self) {
        if self.documents.len() <= 1 {
            return;
        }
        self.active_doc = (self.active_doc + 1) % self.documents.len();
    }

    /// Switch to the previous tab (document), wrapping around
    pub fn prev_tab(&mut self) {
        if self.documents.len() <= 1 {
            return;
        }
        self.active_doc = if self.active_doc == 0 {
            self.documents.len() - 1
        } else {
            self.active_doc - 1
        };
    }

    /// Move to next/previous visible node from current position.
    /// This skips invisible nodes until finding a visible one.
    fn next_visible_from_current(&self, forward: bool) -> usize {
        Self::next_visible_from_current_in(
            &self.active_doc().results,
            self.active_doc().selected_idx,
            forward,
        )
    }

    fn next_visible_from_current_in(
        results: &[mq_markdown::Node],
        start_idx: usize,
        forward: bool,
    ) -> usize {
        let len = results.len();
        if len == 0 {
            return 0;
        }

        // Calculate the current rendered line position
        let current_line = if start_idx == 0 {
            0
        } else {
            Markdown::new(results[..start_idx + 1].to_vec())
                .to_string()
                .lines()
                .count()
                .saturating_sub(
                    Markdown::new(vec![results[start_idx].clone()])
                        .to_string()
                        .lines()
                        .count()
                        .max(1),
                )
        };

        // Find next node that renders to a different line position
        let mut idx = start_idx;
        for _ in 0..len {
            // Move to next/previous position
            if forward {
                idx = (idx + 1) % len;
            } else {
                idx = if idx == 0 { len - 1 } else { idx - 1 };
            }

            // Check if this node is visible (renders to non-empty content)
            let rendered = Markdown::new(vec![results[idx].clone()]).to_string();
            if !rendered.trim().is_empty() {
                // Calculate the line position of this node
                let node_line = if idx == 0 {
                    0
                } else {
                    Markdown::new(results[..idx + 1].to_vec())
                        .to_string()
                        .lines()
                        .count()
                        .saturating_sub(rendered.lines().count().max(1))
                };

                // Return this node if it's at a different line position
                if node_line != current_line {
                    return idx;
                }
            }
        }

        start_idx // No different position found, stay put
    }

    /// Find the nearest visible result index from `start`, searching in `forward` direction.
    /// A node is invisible when `render_with_theme` skips it (renders to "" or only whitespace).
    /// Returns `start` unchanged if all results are invisible.
    fn next_visible(&self, start: usize, forward: bool) -> usize {
        Self::next_visible_in(&self.active_doc().results, start, forward)
    }

    fn next_visible_in(results: &[mq_markdown::Node], start: usize, forward: bool) -> usize {
        let len = results.len();
        if len == 0 {
            return 0;
        }
        let mut idx = start;
        let mut checked = 0;

        while checked < len {
            let rendered = Markdown::new(vec![results[idx].clone()]).to_string();

            // A node is visible if it renders to non-empty, non-whitespace content
            // We need to check both the raw length and trimmed length
            let is_visible = !rendered.trim().is_empty();

            if is_visible {
                return idx;
            }

            // Move to next position
            if forward {
                idx = (idx + 1) % len;
            } else {
                idx = if idx == 0 { len - 1 } else { idx - 1 };
            }
            checked += 1;
        }

        start // all invisible: stay put
    }
}
#[cfg(test)]
mod tests {
    use mq_markdown::Node;

    use super::*;

    fn create_test_app() -> App {
        App::new("# Test\nSome content".to_string())
    }

    fn create_test_app_with_file() -> App {
        App::with_file("# Test\nSome content".to_string(), "test.md".to_string())
    }

    #[test]
    fn test_app_creation() {
        let app = create_test_app();
        assert_eq!(app.query(), ".");
        assert_eq!(app.selected_idx(), 0);
        assert_eq!(app.mode(), Mode::Normal);
        assert!(!app.show_detail());
        assert_eq!(app.cursor_position(), 0);
        assert!(app.filename().is_none());
        assert!(app.error_msg().is_none());
    }

    #[test]
    fn test_app_with_file() {
        let app = create_test_app_with_file();
        assert_eq!(app.filename(), Some("test.md"));
    }

    #[test]
    fn test_app_with_multiple_files() {
        let app = App::with_files(vec![
            ("# One".to_string(), "one.md".to_string()),
            ("# Two".to_string(), "two.md".to_string()),
        ]);
        assert_eq!(app.document_count(), 2);
        assert_eq!(app.active_doc_index(), 0);
        assert_eq!(app.filename(), Some("one.md"));
        assert_eq!(app.document_names(), vec!["one.md", "two.md"]);
    }

    #[test]
    fn test_tab_switching() {
        let mut app = App::with_files(vec![
            ("# One".to_string(), "one.md".to_string()),
            ("# Two".to_string(), "two.md".to_string()),
            ("# Three".to_string(), "three.md".to_string()),
        ]);

        assert_eq!(app.active_doc_index(), 0);
        app.next_tab();
        assert_eq!(app.active_doc_index(), 1);
        app.next_tab();
        assert_eq!(app.active_doc_index(), 2);
        // Wraps around
        app.next_tab();
        assert_eq!(app.active_doc_index(), 0);

        app.prev_tab();
        assert_eq!(app.active_doc_index(), 2);
    }

    #[test]
    fn test_tab_switching_keys() {
        let mut app = App::with_files(vec![
            ("# One".to_string(), "one.md".to_string()),
            ("# Two".to_string(), "two.md".to_string()),
        ]);

        app.handle_event(Event::Key(KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }))
        .unwrap();
        assert_eq!(app.active_doc_index(), 1);

        app.handle_event(Event::Key(KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }))
        .unwrap();
        assert_eq!(app.active_doc_index(), 0);

        app.handle_event(Event::Key(KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }))
        .unwrap();
        assert_eq!(app.active_doc_index(), 1);

        app.handle_event(Event::Key(KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }))
        .unwrap();
        assert_eq!(app.active_doc_index(), 0);
    }

    #[test]
    fn test_query_applies_to_all_documents() {
        let mut app = App::with_files(vec![
            ("# One".to_string(), "one.md".to_string()),
            ("# Two\n\nbody\n\n## Three".to_string(), "two.md".to_string()),
        ]);

        // Identity query, executed once, should populate results for every
        // open document independently (based on each document's own content),
        // not just the active one.
        app.set_query(".".to_string());
        app.exec_query();

        assert_eq!(app.results().len(), 1);
        assert!(app.error_msg().is_none());

        app.next_tab();
        assert_eq!(app.results().len(), 3);
        assert!(app.error_msg().is_none());
    }

    #[test]
    fn test_mode_switching() {
        let mut app = create_test_app();

        // Normal to Query mode
        app.set_mode(Mode::Query);
        assert_eq!(app.mode(), Mode::Query);

        // Query to Help mode
        app.set_mode(Mode::Help);
        assert_eq!(app.mode(), Mode::Help);

        // Help to Normal mode
        app.set_mode(Mode::Normal);
        assert_eq!(app.mode(), Mode::Normal);
    }

    #[test]
    fn test_query_setting() {
        let mut app = create_test_app();
        let test_query = "select(.type == 'heading')";

        app.set_query(test_query.to_string());
        assert_eq!(app.query(), test_query);
        assert_eq!(app.cursor_position(), test_query.len());
    }

    #[test]
    fn test_normal_mode_navigation() {
        let mut app = create_test_app();
        let test_results = vec![
            Node::from("result1"),
            Node::from("result2"),
            Node::from("result3"),
        ];
        app.set_results(test_results);

        // Test down navigation
        let down_event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(down_event).unwrap();
        assert_eq!(app.selected_idx(), 1);

        // Test up navigation
        let up_event = Event::Key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(up_event).unwrap();
        assert_eq!(app.selected_idx(), 0);
    }

    #[test]
    fn test_normal_mode_vim_navigation() {
        let mut app = create_test_app();
        let test_results = vec![Node::from("result1"), Node::from("result2")];
        app.set_results(test_results);

        // Test j (down)
        let j_event = Event::Key(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(j_event).unwrap();
        assert_eq!(app.selected_idx(), 1);

        // Test k (up)
        let k_event = Event::Key(KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(k_event).unwrap();
        assert_eq!(app.selected_idx(), 0);
    }

    #[test]
    fn test_normal_mode_page_navigation() {
        let mut app = create_test_app();
        let test_results = (0..20)
            .map(|i| Node::from(format!("result{}", i)))
            .collect();
        app.set_results(test_results);

        // Test PageDown
        let page_down_event = Event::Key(KeyEvent {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(page_down_event).unwrap();
        assert_eq!(app.selected_idx(), 10);

        // Test PageUp
        let page_up_event = Event::Key(KeyEvent {
            code: KeyCode::PageUp,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(page_up_event).unwrap();
        assert_eq!(app.selected_idx(), 0);
    }

    #[test]
    fn test_normal_mode_home_end_navigation() {
        let mut app = create_test_app();
        let test_results = vec![
            Node::from("result1"),
            Node::from("result2"),
            Node::from("result3"),
        ];
        app.set_results(test_results);
        app.active_doc_mut().selected_idx = 1;

        // Test End
        let end_event = Event::Key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(end_event).unwrap();
        assert_eq!(app.selected_idx(), 2);

        // Test Home
        let home_event = Event::Key(KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(home_event).unwrap();
        assert_eq!(app.selected_idx(), 0);
    }

    #[test]
    fn test_normal_mode_toggle_detail() {
        let mut app = create_test_app();
        assert!(!app.show_detail());

        let detail_event = Event::Key(KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(detail_event.clone()).unwrap();
        assert!(app.show_detail());

        // Toggle again
        app.handle_event(detail_event).unwrap();
        assert!(!app.show_detail());
    }

    #[test]
    fn test_normal_mode_enter_query_mode() {
        let mut app = create_test_app();

        let colon_event = Event::Key(KeyEvent {
            code: KeyCode::Char(':'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(colon_event).unwrap();
        assert_eq!(app.mode(), Mode::Query);
    }

    #[test]
    fn test_normal_mode_enter_help_mode() {
        let mut app = create_test_app();

        let help_event = Event::Key(KeyEvent {
            code: KeyCode::Char('?'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(help_event).unwrap();
        assert_eq!(app.mode(), Mode::Help);

        // Test F1 as well
        app.set_mode(Mode::Normal);
        let f1_event = Event::Key(KeyEvent {
            code: KeyCode::F(1),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(f1_event).unwrap();
        assert_eq!(app.mode(), Mode::Help);
    }

    #[test]
    fn test_normal_mode_clear_query() {
        let mut app = create_test_app();
        app.set_query("test query".to_string());

        let clear_event = Event::Key(KeyEvent {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(clear_event).unwrap();
        assert_eq!(app.query(), "");
        assert_eq!(app.cursor_position(), 0);
    }

    #[test]
    fn test_query_mode_text_input() {
        let mut app = create_test_app();
        app.set_mode(Mode::Query);

        let char_event = Event::Key(KeyEvent {
            code: KeyCode::Char('t'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(char_event).unwrap();
        assert_eq!(app.query(), "t.");
        assert_eq!(app.cursor_position(), 1);
    }

    #[test]
    fn test_query_mode_backspace() {
        let mut app = create_test_app();
        app.set_mode(Mode::Query);
        app.set_query("test".to_string());

        let backspace_event = Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(backspace_event).unwrap();
        assert_eq!(app.query(), "tes");
        assert_eq!(app.cursor_position(), 3);
    }

    #[test]
    fn test_query_mode_delete() {
        let mut app = create_test_app();
        app.set_mode(Mode::Query);
        app.set_query("test".to_string());
        app.set_cursor_position(2);

        let delete_event = Event::Key(KeyEvent {
            code: KeyCode::Delete,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(delete_event).unwrap();
        assert_eq!(app.query(), "tet");
        assert_eq!(app.cursor_position(), 2);
    }

    #[test]
    fn test_query_mode_cursor_movement() {
        let mut app = create_test_app();
        app.set_mode(Mode::Query);
        app.set_query("test".to_string());

        // Test left arrow
        let left_event = Event::Key(KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(left_event).unwrap();
        assert_eq!(app.cursor_position(), 3);

        // Test right arrow
        let right_event = Event::Key(KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(right_event).unwrap();
        assert_eq!(app.cursor_position(), 4);
    }

    #[test]
    fn test_query_mode_home_end() {
        let mut app = create_test_app();
        app.set_mode(Mode::Query);
        app.set_query("test".to_string());
        app.set_cursor_position(2);

        // Test Home
        let home_event = Event::Key(KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(home_event).unwrap();
        assert_eq!(app.cursor_position(), 0);

        // Test End
        let end_event = Event::Key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(end_event).unwrap();
        assert_eq!(app.cursor_position(), 4);
    }

    #[test]
    fn test_query_mode_exit_on_escape() {
        let mut app = create_test_app();
        app.set_mode(Mode::Query);

        let escape_event = Event::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(escape_event).unwrap();
        assert_eq!(app.mode(), Mode::Normal);
    }

    #[test]
    fn test_query_mode_execute_on_enter() {
        let mut app = create_test_app();
        app.set_mode(Mode::Query);
        app.set_query("test query".to_string());

        let enter_event = Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(enter_event).unwrap();
        assert_eq!(app.mode(), Mode::Normal);
        assert!(app.query_history().contains(&"test query".to_string()));
    }

    #[test]
    fn test_help_mode_exit_on_any_key() {
        let mut app = create_test_app();
        app.set_mode(Mode::Help);

        let any_key_event = Event::Key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(any_key_event).unwrap();
        assert_eq!(app.mode(), Mode::Normal);
    }

    #[test]
    fn test_quit_on_q_or_escape() {
        let mut app = create_test_app();

        let q_event = Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(q_event).unwrap();
        assert!(app.should_quit);

        // Reset and test escape
        app.should_quit = false;
        let escape_event = Event::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(escape_event).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_error_message_cleared_on_event() {
        let mut app = create_test_app();
        app.set_error_msg("Test error".to_string());
        assert!(app.error_msg().is_some());

        let any_event = Event::Key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(any_event).unwrap();
        assert!(app.error_msg().is_none());
    }

    #[test]
    fn test_navigation_with_empty_results() {
        let mut app = create_test_app();
        app.set_results(vec![]);

        let down_event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(down_event).unwrap();
        assert_eq!(app.selected_idx(), 0);
    }

    #[test]
    fn test_navigation_wraparound() {
        let mut app = create_test_app();
        let test_results = vec!["result1".into(), Node::from("result2")];
        app.set_results(test_results);
        app.active_doc_mut().selected_idx = 1;

        let down_event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(down_event).unwrap();
        assert_eq!(app.selected_idx(), 0);

        let up_event = Event::Key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(up_event).unwrap();
        assert_eq!(app.selected_idx(), 1);
    }

    #[test]
    fn test_execution_time_tracking() {
        let mut app = create_test_app();
        let test_duration = Duration::from_millis(100);
        app.set_last_exec_time(test_duration);
        assert_eq!(app.last_exec_time(), test_duration);
    }

    #[test]
    fn test_query_history_functionality() {
        let app = create_test_app();
        assert!(app.query_history().is_empty());
    }

    #[test]
    fn test_tree_view_mode() {
        let mut app = create_test_app();
        assert_eq!(app.mode(), Mode::Normal);
        assert!(app.tree_view().is_none());

        // Switch to tree view mode
        app.set_mode(Mode::TreeView);
        app.init_tree_view();

        assert_eq!(app.mode(), Mode::TreeView);
        assert!(app.tree_view().is_some());
    }

    #[test]
    fn test_tree_view_mode_navigation() {
        let mut app = create_test_app();
        app.set_mode(Mode::TreeView);
        app.init_tree_view();

        // Test tree view navigation keys
        let down_event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(down_event).unwrap();
        assert_eq!(app.mode(), Mode::TreeView);

        // Test exiting tree view
        let escape_event = Event::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(escape_event).unwrap();
        assert_eq!(app.mode(), Mode::Normal);
    }

    #[test]
    fn test_tree_view_toggle_from_normal_mode() {
        let mut app = create_test_app();

        let tree_toggle_event = Event::Key(KeyEvent {
            code: KeyCode::Char('t'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(tree_toggle_event).unwrap();
        assert_eq!(app.mode(), Mode::TreeView);
        assert!(app.tree_view().is_some());
    }

    #[test]
    fn test_open_file_mode_enter_and_cancel() {
        let mut app = create_test_app();

        let o_event = Event::Key(KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(o_event).unwrap();
        assert_eq!(app.mode(), Mode::OpenFile);

        let escape_event = Event::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(escape_event).unwrap();
        assert_eq!(app.mode(), Mode::Normal);
        assert_eq!(app.document_count(), 1);
    }

    #[test]
    fn test_open_file_mode_opens_new_tab() {
        let mut app = create_test_app();

        let tmp_dir = std::env::temp_dir();
        let file_path = tmp_dir.join(format!("mq_tui_test_{}.md", std::process::id()));
        std::fs::write(&file_path, "# Opened\n").unwrap();

        app.handle_event(Event::Key(KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }))
        .unwrap();

        for c in file_path.to_string_lossy().chars() {
            app.handle_event(Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }))
            .unwrap();
        }

        app.handle_event(Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }))
        .unwrap();

        assert_eq!(app.mode(), Mode::Normal);
        assert_eq!(app.document_count(), 2);
        assert_eq!(app.active_doc_index(), 1);

        std::fs::remove_file(&file_path).ok();
    }

    #[test]
    fn test_normal_mode_ignores_release_events() {
        let mut app = create_test_app();
        let test_results = vec![
            Node::from("result1"),
            Node::from("result2"),
            Node::from("result3"),
        ];
        app.set_results(test_results);
        let initial_idx = app.selected_idx();

        // Send a Release event for Down key
        let release_event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(release_event).unwrap();

        // Index should not change because Release events are ignored
        assert_eq!(app.selected_idx(), initial_idx);

        // Verify Press event still works
        let press_event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(press_event).unwrap();
        assert_eq!(app.selected_idx(), initial_idx + 1);
    }

    #[test]
    fn test_normal_mode_ignores_repeat_events() {
        let mut app = create_test_app();
        app.mode = Mode::Normal;
        let initial_mode = app.mode();

        // Send a Repeat event for mode switch key
        let repeat_event = Event::Key(KeyEvent {
            code: KeyCode::Char(':'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Repeat,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(repeat_event).unwrap();

        // Mode should not change because Repeat events are ignored
        assert_eq!(app.mode(), initial_mode);
    }

    #[test]
    fn test_query_mode_ignores_release_events() {
        let mut app = create_test_app();
        app.set_mode(Mode::Query);
        app.set_query("test".to_string());
        let initial_query = app.query().to_string();

        // Send a Release event for backspace
        let release_event = Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(release_event).unwrap();

        // Query should not change because Release events are ignored
        assert_eq!(app.query(), initial_query);

        // Verify Press event still works
        let press_event = Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(press_event).unwrap();
        assert_eq!(app.query(), "tes");
    }

    #[test]
    fn test_query_mode_char_input_ignores_release() {
        let mut app = create_test_app();
        app.set_mode(Mode::Query);
        app.set_query("".to_string());

        // Send Release event for character input
        let release_event = Event::Key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(release_event).unwrap();

        // Query should remain empty because Release events are ignored
        assert_eq!(app.query(), "");

        // Verify Press event adds the character
        let press_event = Event::Key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(press_event).unwrap();
        assert_eq!(app.query(), "x");
    }

    #[test]
    fn test_help_mode_ignores_release_events() {
        let mut app = create_test_app();
        app.set_mode(Mode::Help);

        // Send a Release event
        let release_event = Event::Key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(release_event).unwrap();

        // Mode should not change because Release events are ignored
        assert_eq!(app.mode(), Mode::Help);

        // Verify Press event exits help mode
        let press_event = Event::Key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(press_event).unwrap();
        assert_eq!(app.mode(), Mode::Normal);
    }

    #[test]
    fn test_tree_view_mode_ignores_release_events() {
        let mut app = create_test_app();
        app.set_mode(Mode::TreeView);
        app.init_tree_view();

        // Send a Release event for navigation
        let release_event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(release_event).unwrap();

        // Should still be in tree view mode
        assert_eq!(app.mode(), Mode::TreeView);

        // Send Release event for Escape (exit tree view)
        let escape_release = Event::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(escape_release).unwrap();

        // Mode should not change because Release events are ignored
        assert_eq!(app.mode(), Mode::TreeView);

        // Verify Press event exits tree view
        let escape_press = Event::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(escape_press).unwrap();
        assert_eq!(app.mode(), Mode::Normal);
    }

    #[test]
    fn test_windows_double_input_scenario() {
        // This test simulates the Windows environment where both Press and Release
        // events are generated for a single key press (issue #1)
        let mut app = create_test_app();
        let test_results = vec![Node::from("result1"), Node::from("result2")];
        app.set_results(test_results);
        assert_eq!(app.selected_idx(), 0);

        // Simulate Windows: Press event followed by Release event
        let press_event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(press_event).unwrap();
        assert_eq!(app.selected_idx(), 1); // Should move to next item

        let release_event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(release_event).unwrap();
        assert_eq!(app.selected_idx(), 1); // Should NOT move again

        // Verify the fix: only one navigation occurred instead of two
    }

    #[test]
    fn test_query_mode_windows_double_input_scenario() {
        // Test query mode with Windows-style Press/Release events
        let mut app = create_test_app();
        app.set_mode(Mode::Query);

        // Simulate typing 'j' with Press event
        let press_j = Event::Key(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(press_j).unwrap();
        assert_eq!(app.query(), "j.");

        // Simulate Release event for 'j'
        let release_j = Event::Key(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(release_j).unwrap();
        assert_eq!(app.query(), "j."); // Should still be "j", not "jj"

        // Simulate typing 'k' with Press event
        let press_k = Event::Key(KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(press_k).unwrap();
        assert_eq!(app.query(), "jk.");

        // Simulate Release event for 'k'
        let release_k = Event::Key(KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        });
        app.handle_event(release_k).unwrap();
        assert_eq!(app.query(), "jk."); // Should still be "jk", not "jkk"

        // Verify the fix: query is "jk" not "jjkk"
    }
}
