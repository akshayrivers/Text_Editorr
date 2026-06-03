use crate::{editor::layout::SplitDirection, prelude::*};
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
mod line;
mod terminal;
mod uicomponents;
pub use annotationtype::AnnotationType;
mod annotation;
use annotatedstring::AnnotatedString;
use annotation::Annotation;
use buffers::{Buffer, BufferManager};
use crossterm::event::{read, Event, KeyEvent, KeyEventKind};
use documentstatus::DocumentStatus;
use line::Line;
mod filetype;
use filetype::FileType;
mod layout;
use self::command::{
    Command::{self, Edit, Mouse, Move, System},
    Edit::InsertNewLine,
    MouseCommand::{LeftClick, LeftDrag, LeftRelease, ScrollDown, ScrollUp},
    Move::{Down, Left, Right, Up},
    System::{
        Dismiss, OpenCommandBar, Quit, Redo, Resize, Save, Search, SplitHorizontal, SplitVertical,
        Undo,
    },
};
use layout::{LayoutTree, Pane, PaneContent, PaneManager};
use terminal::Terminal;
use uicomponents::{CommandBar, MessageBar, StatusBar, UIComponent, View};

const QUIT_TIMES: u8 = 3;

#[derive(Eq, PartialEq, Default)]
enum PromptType {
    Search,
    Save,
    #[default]
    None,
    FocusPane,
    ClosePane,
}
impl PromptType {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
}
#[derive(Default)]
pub struct Editor {
    should_quit: bool,
    layout_tree: LayoutTree,
    pane_manager: PaneManager,
    buffer_manager: BufferManager,
    status_bar: StatusBar,
    message_bar: MessageBar,
    command_bar: CommandBar,
    prompt_type: PromptType,
    terminal_size: Size,
    title: String,
    quit_times: u8,
    dragging_split: Option<usize>,
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
            position: Position { row: 0, col: 0 },

            size: Size {
                height: terminal_size.height.saturating_sub(2),
                width: terminal_size.width,
            },
        };

        // Phase III: Buffer Manager
        let mut buffer_manager = BufferManager::new();
        let initial_buffer_id = buffer_manager.add(Buffer::default());

        let initial_pane_id = 0;
        let mut initial_view = View::default();
        initial_view.set_id(initial_pane_id);
        initial_view.set_buffer_id(initial_buffer_id);

        // Initial Pane
        let initial_pane = Pane {
            pane_id: initial_pane_id,
            content: PaneContent::TextView(initial_view),
            active: true,
        };

        // Phase II systems
        let pane_manager = PaneManager::new(initial_pane);

        let layout_tree = LayoutTree::new(0, root_rect);

        let mut editor = Self {
            should_quit: false,

            // new systems
            layout_tree,
            pane_manager,
            buffer_manager,

            status_bar: StatusBar::default(),
            message_bar: MessageBar::default(),
            command_bar: CommandBar::default(),

            prompt_type: PromptType::None,

            terminal_size,

            title: String::new(),

            quit_times: 0,
            dragging_split: None,
        };

        editor.handle_resize_command(terminal_size);

        editor.update_message("HELP: Ctrl-F = find | Ctrl-S = save | Ctrl-Q = quit");

        let args: Vec<String> = env::args().collect();

        if let Some(file_name) = args.get(1) {
            debug_assert!(!file_name.is_empty());

            match Buffer::load(file_name) {
                Ok(buffer) => {
                    let buffer_id = editor.buffer_manager.add(buffer);
                    let view = editor
                        .pane_manager
                        .active_pane_mut()
                        .and_then(|p| p.view_mut())
                        .unwrap();
                    view.set_buffer_id(buffer_id);
                }
                Err(_) => {
                    editor.update_message(&format!("ERR: Could not open file: {file_name}"));
                }
            }
        }

        editor.refresh_status();

        Ok(editor)
    }

    // endregion

    // region: Event Loop
    pub fn run(&mut self) {
        loop {
            self.refresh_screen();
            if self.should_quit {
                break;
            }
            match read() {
                Ok(event) => self.evaluate_event(event),
                Err(err) => {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Could not read event:{err:?}");
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        let _ = err;
                    }
                }
            }
            self.refresh_status();
        }
    }

    fn refresh_screen(&mut self) {
        if self.terminal_size.height == 0 || self.terminal_size.width == 0 {
            return;
        }

        let Size { height, width } = self.terminal_size;

        let _ = Terminal::hide_caret();

        // Bottom UI
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
            for (pane_id, _) in self.layout_tree.collect_leaf_layouts() {
                if let Some(pane) = self.pane_manager.get_pane_mut(pane_id) {
                    pane.render(&self.buffer_manager);
                }
            }
        }

        // Caret
        let active_pane = self.pane_manager.active_pane().unwrap();
        let active_view = active_pane.view().unwrap();
        let buffer = self.buffer_manager.get(active_view.buffer_id()).unwrap();

        let new_caret_pos = if self.in_prompt() {
            self.command_bar.caret_position()
        } else {
            active_view.caret_position(buffer)
        };

        debug_assert!(new_caret_pos.col <= width);
        debug_assert!(new_caret_pos.row <= height);

        let _ = Terminal::move_caret_to(new_caret_pos);

        let _ = Terminal::show_caret();
        let _ = Terminal::execute();
    }
    pub fn refresh_status(&mut self) {
        let active_pane = self.pane_manager.active_pane().unwrap();
        let active_view = active_pane.view().unwrap();
        let buffer = self.buffer_manager.get(active_view.buffer_id()).unwrap();

        let status = active_view.get_status(buffer);
        let title = format!("{} - {NAME}", status.file_name);
        self.status_bar.update_status(status);
        if title != self.title && matches!(Terminal::set_title(&title), Ok(())) {
            self.title = title;
        }
    }
    fn evaluate_event(&mut self, event: Event) {
        let should_process = match &event {
            Event::Key(KeyEvent { kind, .. }) => kind == &KeyEventKind::Press,
            Event::Resize(_, _) => true,
            Event::Mouse(_) => true,
            _ => false,
        };
        if should_process {
            if let Ok(command) = Command::try_from(event) {
                self.process_command(command);
            }
        }
    }
    // endregion

    // region: command handling

    fn process_command(&mut self, command: Command) {
        if let System(Resize(size)) = command {
            self.handle_resize_command(size);
            return;
        }
        match self.prompt_type {
            PromptType::Search => self.process_command_during_search(command),
            PromptType::Save => self.process_command_during_save(command),
            PromptType::None => self.process_command_no_prompt(command),
            PromptType::FocusPane => self.handle_pane_commands(command),
            PromptType::ClosePane => self.handle_pane_commands(command),
        }
    }
    fn process_command_no_prompt(&mut self, command: Command) {
        if matches!(command, System(Quit)) {
            self.handle_quit_command();
            return;
        }
        self.reset_quit_times();
        match command {
            System(Quit | Resize(_) | Dismiss) => {}
            System(Search) => self.set_prompt(PromptType::Search),
            System(Save) => self.handle_save_command(),
            System(Redo) => self.handle_redo_command(),
            System(Undo) => self.handle_undo_command(),
            Edit(edit_command) => {
                let buffer_id = self
                    .pane_manager
                    .active_pane()
                    .unwrap()
                    .view()
                    .unwrap()
                    .buffer_id();
                let buffer = self.buffer_manager.get_mut(buffer_id).unwrap();
                let view = self
                    .pane_manager
                    .active_pane_mut()
                    .unwrap()
                    .view_mut()
                    .unwrap();
                view.handle_edit_command(edit_command, buffer);
            }

            Move(move_command) => {
                let buffer_id = self
                    .pane_manager
                    .active_pane()
                    .unwrap()
                    .view()
                    .unwrap()
                    .buffer_id();
                let buffer = self.buffer_manager.get(buffer_id).unwrap();
                let view = self
                    .pane_manager
                    .active_pane_mut()
                    .unwrap()
                    .view_mut()
                    .unwrap();
                view.handle_move_command(move_command, buffer);
            }
            System(SplitHorizontal) => self.split_active_pane(SplitDirection::Horizontal),

            System(SplitVertical) => self.split_active_pane(SplitDirection::Vertical),
            System(OpenCommandBar) => self.set_prompt(PromptType::FocusPane),
            Mouse(LeftClick(position)) => {
                self.pane_left_click(position);
            }

            Mouse(LeftDrag(position)) => {
                self.pane_left_drag(position);
            }

            Mouse(LeftRelease(position)) => {
                self.pane_left_release();
            }
            Mouse(ScrollDown(position)) => {
                self.pane_scroll_down();
            }
            Mouse(ScrollUp(position)) => {
                self.pane_scroll_up();
            }
        }
    }
    fn pane_left_click(&mut self, position: Position) {
        // check if user clicked on a split divider
        if let Some(split) = self.layout_tree.find_split(position) {
            self.dragging_split = Some(split.id);
            return;
        }

        // otherwise focus pane under cursor
        for (pane_id, rect) in self.layout_tree.collect_leaf_layouts() {
            let inside = position.row >= rect.position.row
                && position.row < rect.position.row + rect.size.height
                && position.col >= rect.position.col
                && position.col < rect.position.col + rect.size.width;

            if inside {
                self.pane_manager.set_active_pane(pane_id);
                break;
            }
        }
    }

    fn pane_left_drag(&mut self, position: Position) {
        if let Some(split_id) = self.dragging_split {
            self.layout_tree.resize_split(split_id, position);

            let editor_rect = Rect {
                position: Position { row: 0, col: 0 },
                size: Size {
                    height: self.terminal_size.height.saturating_sub(2),
                    width: self.terminal_size.width,
                },
            };

            self.layout_tree.compute_layout(editor_rect);
            self.sync_pane_rects();
        }
    }
    fn pane_left_release(&mut self) {
        self.dragging_split = None;
    }

    fn pane_scroll_down(&mut self) {
        let buffer_id = self
            .pane_manager
            .active_pane()
            .unwrap()
            .view()
            .unwrap()
            .buffer_id();
        let buffer = self.buffer_manager.get(buffer_id).unwrap();
        let view = self
            .pane_manager
            .active_pane_mut()
            .unwrap()
            .view_mut()
            .unwrap();
        view.handle_move_command(command::Move::PageDown, buffer);
    }

    fn pane_scroll_up(&mut self) {
        let buffer_id = self
            .pane_manager
            .active_pane()
            .unwrap()
            .view()
            .unwrap()
            .buffer_id();
        let buffer = self.buffer_manager.get(buffer_id).unwrap();
        let view = self
            .pane_manager
            .active_pane_mut()
            .unwrap()
            .view_mut()
            .unwrap();
        view.handle_move_command(command::Move::PageUp, buffer);
    }
    fn split_active_pane(&mut self, direction: SplitDirection) {
        let active_pane_id = self
            .pane_manager
            .active_pane()
            .map(|pane| pane.pane_id)
            .expect("No active pane");

        let current_buffer_id = self
            .pane_manager
            .active_pane()
            .unwrap()
            .view()
            .unwrap()
            .buffer_id();

        // create new pane
        let mut new_view = View::default();
        new_view.set_buffer_id(current_buffer_id);

        let new_pane_id = self
            .pane_manager
            .create_pane(PaneContent::TextView(new_view));

        if let Some(view) = self
            .pane_manager
            .get_pane_mut(new_pane_id)
            .and_then(|p| p.view_mut())
        {
            view.set_id(new_pane_id);
        }
        // mutate layout tree
        if self
            .layout_tree
            .split_pane(active_pane_id, new_pane_id, direction, 0.5)
            .is_err()
        {
            self.update_message("Failed to split pane");
            return;
        }

        // recompute geometry
        let editor_rect = Rect {
            position: Position { row: 0, col: 0 },
            size: Size {
                height: self.terminal_size.height.saturating_sub(2),
                width: self.terminal_size.width,
            },
        };

        self.layout_tree.compute_layout(editor_rect);
        self.sync_pane_rects();

        // focus new pane
        self.pane_manager.set_active_pane(new_pane_id);
    }
    pub fn handle_resize_command(&mut self, size: Size) {
        self.terminal_size = size;

        let Size { height, width } = size;

        let editor_rect = Rect {
            position: Position { row: 0, col: 0 },
            size: Size {
                height: height.saturating_sub(2),
                width,
            },
        };

        self.layout_tree.compute_layout(editor_rect);
        self.sync_pane_rects();

        let bottom_bar_rect = Rect {
            position: Position {
                row: height.saturating_sub(1),
                col: 0,
            },
            size: Size { height: 1, width },
        };

        let status_bar_rect = Rect {
            position: Position {
                row: height.saturating_sub(2),
                col: 0,
            },
            size: Size { height: 1, width },
        };

        self.message_bar.resize(bottom_bar_rect);
        self.command_bar.resize(bottom_bar_rect);
        self.status_bar.resize(status_bar_rect);
    }

    // endregion

    // region : quit command handling
    #[allow(clippy::arithmetic_side_effects)]
    fn handle_quit_command(&mut self) {
        let buffer_id = self
            .pane_manager
            .active_pane()
            .unwrap()
            .view()
            .unwrap()
            .buffer_id();
        let buffer = self.buffer_manager.get(buffer_id).unwrap();
        let view = self
            .pane_manager
            .active_pane_mut()
            .unwrap()
            .view_mut()
            .unwrap();
        let status = view.get_status(buffer);
        if !status.is_modified || self.quit_times + 1 == QUIT_TIMES {
            self.should_quit = true;
        } else if status.is_modified {
            self.update_message(&format!(
                "Warning! File has unsaved changes. Press Ctrl-Q {} more times to quit.",
                QUIT_TIMES - self.quit_times - 1
            ));
            self.quit_times += 1;
        }
    }
    fn reset_quit_times(&mut self) {
        if self.quit_times > 0 {
            self.quit_times = 0;
            self.update_message("");
        }
    }
    //endregion
    // region : undo & redo
    fn handle_redo_command(&mut self) {
        let buffer_id = self
            .pane_manager
            .active_pane()
            .unwrap()
            .view()
            .unwrap()
            .buffer_id();
        let buffer = self.buffer_manager.get_mut(buffer_id).unwrap();
        let view = self
            .pane_manager
            .active_pane_mut()
            .unwrap()
            .view_mut()
            .unwrap();
        view.redo(buffer);
    }
    fn handle_undo_command(&mut self) {
        let buffer_id = self
            .pane_manager
            .active_pane()
            .unwrap()
            .view()
            .unwrap()
            .buffer_id();
        let buffer = self.buffer_manager.get_mut(buffer_id).unwrap();
        let view = self
            .pane_manager
            .active_pane_mut()
            .unwrap()
            .view_mut()
            .unwrap();
        view.undo(buffer);
    }

    // region : save command & prompt handling

    fn handle_save_command(&mut self) {
        let buffer_id = self
            .pane_manager
            .active_pane()
            .unwrap()
            .view()
            .unwrap()
            .buffer_id();
        let buffer = self.buffer_manager.get(buffer_id).unwrap();
        if buffer.is_file_loaded() {
            self.save(None);
        } else {
            self.set_prompt(PromptType::Save);
        }
    }

    fn process_command_during_save(&mut self, command: Command) {
        match command {
            System(
                Quit | Resize(_) | Search | Save | Undo | Redo | SplitHorizontal | SplitVertical
                | OpenCommandBar,
            )
            | Move(_) => {} //already handled
            System(Dismiss) => {
                self.set_prompt(PromptType::None);
                self.update_message("Save aborted");
            }
            Edit(InsertNewLine) => {
                let file_name = self.command_bar.value();
                self.save(Some(&file_name));
                self.set_prompt(PromptType::None);
            }
            Edit(edit_command) => self.command_bar.handle_edit_command(edit_command),
            Mouse(_) => {}
        }
    }
    fn save(&mut self, file_name: Option<&str>) {
        let buffer_id = self
            .pane_manager
            .active_pane()
            .unwrap()
            .view()
            .unwrap()
            .buffer_id();
        let buffer = self.buffer_manager.get_mut(buffer_id).unwrap();
        let result = if let Some(name) = file_name {
            buffer.save_as(name)
        } else {
            buffer.save()
        };
        if result.is_ok() {
            self.update_message("File saved successfully.");
        } else {
            self.update_message("Error writing file!");
        }
    }
    // endregion

    // region: Search command and prompt handling

    fn process_command_during_search(&mut self, command: Command) {
        match command {
            System(Dismiss) => {
                self.set_prompt(PromptType::None);
                let buffer_id = self
                    .pane_manager
                    .active_pane()
                    .unwrap()
                    .view()
                    .unwrap()
                    .buffer_id();
                let buffer = self.buffer_manager.get(buffer_id).unwrap();
                let view = self
                    .pane_manager
                    .active_pane_mut()
                    .unwrap()
                    .view_mut()
                    .unwrap();
                view.dismiss_search(buffer);
            }
            Edit(InsertNewLine) => {
                self.set_prompt(PromptType::None);
                self.pane_manager
                    .active_pane_mut()
                    .unwrap()
                    .view_mut()
                    .unwrap()
                    .exit_search();
            }

            Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
                let query = self.command_bar.value();
                let buffer_id = self
                    .pane_manager
                    .active_pane()
                    .unwrap()
                    .view()
                    .unwrap()
                    .buffer_id();
                let buffer = self.buffer_manager.get(buffer_id).unwrap();
                let view = self
                    .pane_manager
                    .active_pane_mut()
                    .unwrap()
                    .view_mut()
                    .unwrap();
                view.search(&query, buffer);
            }
            Move(Right | Down) => {
                let buffer_id = self
                    .pane_manager
                    .active_pane()
                    .unwrap()
                    .view()
                    .unwrap()
                    .buffer_id();
                let buffer = self.buffer_manager.get(buffer_id).unwrap();
                let view = self
                    .pane_manager
                    .active_pane_mut()
                    .unwrap()
                    .view_mut()
                    .unwrap();
                view.search_next(buffer);
            }
            Move(Up | Left) => {
                let buffer_id = self
                    .pane_manager
                    .active_pane()
                    .unwrap()
                    .view()
                    .unwrap()
                    .buffer_id();
                let buffer = self.buffer_manager.get(buffer_id).unwrap();
                let view = self
                    .pane_manager
                    .active_pane_mut()
                    .unwrap()
                    .view_mut()
                    .unwrap();
                view.search_prev(buffer);
            }
            System(
                Quit | Resize(_) | Search | Save | Undo | Redo | SplitHorizontal | SplitVertical
                | OpenCommandBar,
            )
            | Move(_) => {} // Not applicable during save, Resize already handled at this stage
            Mouse(_) => {}
        }
    }

    // endregion
    // region: pane focus and close
    fn handle_pane_commands(&mut self, command: Command) {
        match command {
            System(Dismiss) => {
                self.set_prompt(PromptType::None);
            }
            Edit(InsertNewLine) => {
                let input = self.command_bar.value();
                self.execute_pane_command(&input);
                self.set_prompt(PromptType::None);
            }

            Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
            }
            _ => {}
        }
    }
    fn execute_pane_command(&mut self, input: &str) {
        let parts: Vec<&str> = input.split_whitespace().collect();
        match parts.as_slice() {
            ["focus", id_str] => {
                if let Ok(id) = id_str.parse::<usize>() {
                    if self.pane_manager.get_pane(id).is_some() {
                        self.pane_manager.set_active_pane(id);
                    } else {
                        self.update_message(&format!("Pane {} not found", id));
                    }
                }
            }
            ["close", id_str] => {
                if let Ok(id) = id_str.parse::<usize>() {
                    self.close_pane(id);
                }
            }
            ["close"] => {
                // we will close the current active pane
                let id = self.pane_manager.active_pane().unwrap().pane_id;
                self.close_pane(id);
            }

            _ => self.update_message("Invalid command! Try 'focus 1' or 'close 1' or close"),
        }
    }
    fn close_pane(&mut self, id: usize) {
        // Remove from the layout tree
        if self.layout_tree.remove_node(id).is_ok() {
            // Remove from pane manager
            self.pane_manager.remove_pane(id);

            // need to assign new pane_id in the pane manager
            if self.pane_manager.active_pane().is_none() {
                if let Some((any_id, _)) = self.layout_tree.collect_leaf_layouts().first() {
                    self.pane_manager.set_active_pane(*any_id);
                }
            }
            // we resize, ultimately we should improve the above logic in future
            self.handle_resize_command(self.terminal_size);
            self.update_message(&format!("Pane {} closed", id));
        } else {
            self.update_message("Cannot close the last pane!");
        }
    }
    // region: message & command bar
    fn update_message(&mut self, new_message: &str) {
        self.message_bar.update_message(new_message);
    }
    // endregion

    // region: prompt handling
    fn in_prompt(&self) -> bool {
        !self.prompt_type.is_none()
    }

    fn set_prompt(&mut self, prompt_type: PromptType) {
        match prompt_type {
            PromptType::None => self.message_bar.mark_redraw(true),
            PromptType::Save => self.command_bar.set_prompt("Save as: "),
            PromptType::Search => {
                self.pane_manager
                    .active_pane_mut()
                    .unwrap()
                    .view_mut()
                    .unwrap()
                    .enter_search();
                self.command_bar
                    .set_prompt("Search (Esc to cancel, Arrows to navigate): ");
            }
            PromptType::FocusPane => self
                .command_bar
                .set_prompt("focus [Pane ID] to focus on that pane"),
            PromptType::ClosePane => self.command_bar.set_prompt(
                "close [Pane ID] to closethat pane, if no Pane ID the current active will close",
            ),
        }
        self.command_bar.clear_value();
        self.prompt_type = prompt_type;
    }
    fn sync_pane_rects(&mut self) {
        for (pane_id, rect) in self.layout_tree.collect_leaf_layouts() {
            if let Some(pane) = self.pane_manager.get_pane_mut(pane_id) {
                pane.resize(rect);
            }
        }
    }
    // endregion
}
impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::terminate();
        if self.should_quit {
            let _ = Terminal::print("Goodbye.\r\n");
        }
    }
}
