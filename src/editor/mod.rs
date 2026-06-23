// src/editor/mod.rs
use crate::prelude::*;
use std::{
    env,
    io::Error,
    panic::{set_hook, take_hook},
};

mod annotatedstring;
pub mod annotationtype;
mod buffers;
mod command;
mod documentstatus;
pub mod events;
mod line;
mod plugins;
mod terminal;
mod uicomponents;

pub use annotationtype::AnnotationType;
mod annotation;
use annotatedstring::AnnotatedString;
use annotation::Annotation;
use buffers::{Buffer, BufferManager};
use documentstatus::DocumentStatus;
use line::Line;
mod filetype;
use filetype::FileType;
mod layout;
use layout::{LayoutTree, Pane, PaneContent, PaneManager};
use plugins::PluginManager;
use terminal::Terminal;
use uicomponents::{BufferBar, CommandBar, MessageBar, StatusBar, UIComponent, View};

mod command_dispatcher;
use command_dispatcher::{EditorContext, HandlerRegistry, PromptType};

pub struct Editor {
    should_quit: bool,
    layout_tree: LayoutTree,
    pane_manager: PaneManager,
    buffer_manager: BufferManager,
    plugin_manager: PluginManager,
    buffer_bar: BufferBar,
    status_bar: StatusBar,
    message_bar: MessageBar,
    command_bar: CommandBar,
    prompt_type: PromptType,
    terminal_size: Size,
    title: String,
    quit_times: u8,
    dragging_split: Option<usize>,
    dragging_pane: Option<usize>,
    drag_offset: Position,
    command_handler: HandlerRegistry,
}

impl Editor {
    pub fn new() -> Result<Self, Error> {
        let current_hook = take_hook();
        set_hook(Box::new(move |panic_info| {
            let _ = Terminal::terminate();
            current_hook(panic_info);
        }));

        Terminal::initialize()?;

        let terminal_size = Terminal::size().unwrap_or_default();

        let root_rect = Rect {
            position: Position { row: 1, col: 0 },
            size: Size {
                height: terminal_size.height.saturating_sub(3),
                width: terminal_size.width,
            },
        };

        let mut buffer_manager = BufferManager::new();
        let initial_buffer_id = buffer_manager.add(Buffer::default());

        let initial_pane_id = 0;
        let mut initial_view = View::default();
        initial_view.set_id(initial_pane_id);
        initial_view.set_buffer_id(initial_buffer_id);

        let initial_pane = Pane {
            pane_id: initial_pane_id,
            content: PaneContent::TextView(initial_view),
            active: true,
            is_floating: false,
            z_index: 0,
            is_minimized: false,
            rect: root_rect,
        };

        let pane_manager = PaneManager::new(initial_pane);
        let layout_tree = LayoutTree::new(0, root_rect);

        let mut editor = Self {
            should_quit: false,
            layout_tree,
            pane_manager,
            buffer_manager,
            plugin_manager: PluginManager::default(),
            buffer_bar: BufferBar::default(),
            status_bar: StatusBar::default(),
            message_bar: MessageBar::default(),
            command_bar: CommandBar::default(),
            prompt_type: PromptType::None,
            terminal_size,
            title: String::new(),
            quit_times: 0,
            dragging_split: None,
            dragging_pane: None,
            drag_offset: Position::default(),
            command_handler: HandlerRegistry::default(),
        };

        editor.handle_resize_command(terminal_size);
        editor.update_message("HELP: Ctrl-F = find | Ctrl-S = save | Ctrl-Q = quit");

        let args: Vec<String> = env::args().collect();
        if let Some(file_name) = args.get(1) {
            debug_assert!(!file_name.is_empty());
            match Buffer::load(file_name) {
                Ok(buffer) => {
                    let buffer_id = editor.buffer_manager.add(buffer);
                    if let Some(view) = editor
                        .pane_manager
                        .active_pane_mut()
                        .and_then(|p| p.view_mut())
                    {
                        view.set_buffer_id(buffer_id);
                    }
                }
                Err(_) => {
                    editor.update_message(&format!("ERR: Could not open file: {file_name}"));
                }
            }
        }

        editor.refresh_status();
        Ok(editor)
    }

    // ── Event loop ───────────────────────────────────────────────────────────

    pub fn run(&mut self) {
        loop {
            self.refresh_screen();
            if self.should_quit {
                break;
            }
            match Terminal::wait_for_event() {
                Ok(event) => {
                    if let Ok(command) = command::Command::try_from(event) {
                        if let command::Command::System(command::System::Resize(size)) = command {
                            self.handle_resize_command(size);
                        }
                        // Take handler out so we can mutably borrow the rest of self
                        let mut handler = std::mem::take(&mut self.command_handler);
                        let mut ctx = self.make_context();
                        let _ = handler.dispatch(&command, &mut ctx);
                        self.command_handler = handler;
                    }
                }
                Err(err) => {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Could not read event: {err:?}");
                    }
                }
            }
            self.refresh_status();
        }
    }
    #[warn(mismatched_lifetime_syntaxes)]
    fn make_context(&'_ mut self) -> EditorContext<'_> {
        EditorContext {
            prompt_type: &mut self.prompt_type,
            pane_manager: &mut self.pane_manager,
            layout_tree: &mut self.layout_tree,
            buffer_manager: &mut self.buffer_manager,
            command_bar: &mut self.command_bar,
            message_bar: &mut self.message_bar,
            terminal_size: self.terminal_size,
            should_quit: &mut self.should_quit,
            quit_times: &mut self.quit_times,
            dragging_split: &mut self.dragging_split,
            dragging_pane: &mut self.dragging_pane,
            drag_offset: &mut self.drag_offset,
        }
    }

    // Rendering

    fn refresh_screen(&mut self) {
        if self.terminal_size.height == 0 || self.terminal_size.width == 0 {
            return;
        }

        let Size { height, width } = self.terminal_size;

        let _ = Terminal::hide_caret();

        // Top bar
        let _ = self
            .buffer_bar
            .render(&self.buffer_manager, &self.pane_manager);

        // Bottom bar — command bar during prompts, message bar otherwise
        if self.in_prompt() {
            self.command_bar.render();
        } else {
            self.message_bar.render();
        }

        if height > 1 {
            self.status_bar.render();
        }

        // Panes
        if height > 2 {
            // 1. Tiled panes first (layer 0)
            for (pane_id, _) in self.layout_tree.collect_leaf_layouts() {
                if let Some(pane) = self.pane_manager.get_pane_mut(pane_id) {
                    if !pane.is_floating {
                        pane.render(&self.buffer_manager);
                    }
                }
            }

            // 2. Floating panes on top (sorted by z-index ascending)
            let mut floating_panes = self.pane_manager.get_floating_panes_sorted_mut();
            for pane in floating_panes.iter_mut() {
                pane.render(&self.buffer_manager);
            }
        }

        // Caret
        let active_pane = self.pane_manager.active_pane().unwrap();
        let new_caret_pos = if self.in_prompt() {
            self.command_bar.caret_position()
        } else if let Some(view) = active_pane.view() {
            let buffer = self.buffer_manager.get(view.buffer_id()).unwrap();
            view.caret_position(buffer)
        } else {
            let rect = active_pane.component().rect();
            Position {
                row: rect.position.row.saturating_add(1),
                col: rect.position.col.saturating_add(1),
            }
        };

        debug_assert!(new_caret_pos.col <= width);
        debug_assert!(new_caret_pos.row <= height);

        let _ = Terminal::move_caret_to(new_caret_pos);
        let _ = Terminal::show_caret();
        let _ = Terminal::execute();
    }

    pub fn refresh_status(&mut self) {
        let active_pane = self.pane_manager.active_pane().unwrap();
        let status = if let Some(view) = active_pane.view() {
            let buffer = self.buffer_manager.get(view.buffer_id()).unwrap();
            view.get_status(buffer)
        } else {
            DocumentStatus {
                file_name: "Explorer".to_string(),
                total_lines: 0,
                current_line_idx: 0,
                is_modified: false,
                file_type: FileType::Text,
            }
        };

        let title = format!("{} - {NAME}", status.file_name);
        self.status_bar.update_status(status);
        if title != self.title && matches!(Terminal::set_title(&title), Ok(())) {
            self.title = title;
        }
    }

    // ── Resize (still on Editor — it owns buffer_bar/status_bar/etc.) ────────
    //
    // EditorContext::handle_resize() handles the layout_tree + pane rects.
    // This method handles the UI bars that only Editor knows about.
    // Called from Editor::new() and whenever a Resize event needs full handling.

    pub fn handle_resize_command(&mut self, size: Size) {
        self.terminal_size = size;

        let Size { height, width } = size;

        // Buffer bar: row 0
        self.buffer_bar.resize(Rect {
            position: Position { row: 0, col: 0 },
            size: Size { height: 1, width },
        });

        // Editor pane area: rows 1..height-2
        let editor_rect = Rect {
            position: Position { row: 1, col: 0 },
            size: Size {
                height: height.saturating_sub(3),
                width,
            },
        };
        self.layout_tree.compute_layout(editor_rect);
        self.sync_pane_rects();

        // Clamp floating panes to screen
        for pane in self.pane_manager.iter_mut() {
            if pane.is_floating {
                let mut rect = pane.component().rect();
                rect.position.col = rect.position.col.min(width.saturating_sub(4));
                rect.position.row = rect.position.row.min(height.saturating_sub(2));
                pane.resize(rect);
            }
        }

        // Status bar: row height-2
        self.status_bar.resize(Rect {
            position: Position {
                row: height.saturating_sub(2),
                col: 0,
            },
            size: Size { height: 1, width },
        });

        // Message / command bar: row height-1
        let bottom_rect = Rect {
            position: Position {
                row: height.saturating_sub(1),
                col: 0,
            },
            size: Size { height: 1, width },
        };
        self.message_bar.resize(bottom_rect);
        self.command_bar.resize(bottom_rect);

        self.mark_all_panes_for_redraw();
    }

    // helpers that refresh_screen / handle_resize_command still need

    fn in_prompt(&self) -> bool {
        !self.prompt_type.is_none()
    }

    fn update_message(&mut self, new_message: &str) {
        self.message_bar.update_message(new_message);
    }

    fn sync_pane_rects(&mut self) {
        for (pane_id, rect) in self.layout_tree.collect_leaf_layouts() {
            if let Some(pane) = self.pane_manager.get_pane_mut(pane_id) {
                pane.resize(rect);
            }
        }
    }

    fn mark_all_panes_for_redraw(&mut self) {
        for pane in self.pane_manager.iter_mut() {
            if let Some(view) = pane.view_mut() {
                view.mark_redraw(true);
            }
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::terminate();
        if self.should_quit {
            let _ = Terminal::print("Goodbye.\r\n");
        }
    }
}
