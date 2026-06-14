use crate::editor::command::Move;
use crate::editor::uicomponents::UIComponent;
use crate::editor::Terminal;
use crate::prelude::*;
use std::fs;
use std::io::Error;
use std::path::PathBuf;

pub enum FileExplorerAction {
    DirectoryChanged,
    OpenFile(PathBuf),
    None,
}

pub struct FileExplorer {
    current_dir: PathBuf,
    entries: Vec<String>,
    selected_idx: usize,
    scroll_top: usize,
    rect: Rect,
    needs_redraw: bool,
    active: bool,
}

impl Default for FileExplorer {
    fn default() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut explorer = Self {
            current_dir,
            entries: Vec::new(),
            selected_idx: 0,
            scroll_top: 0,
            rect: Rect::default(),
            needs_redraw: true,
            active: false,
        };
        explorer.refresh_entries();
        explorer
    }
}

impl FileExplorer {
    pub fn current_dir(&self) -> &PathBuf {
        &self.current_dir
    }

    pub fn refresh_entries(&mut self) {
        self.entries.clear();
        self.entries.push("..".to_string());
        if let Ok(read_dir) = fs::read_dir(&self.current_dir) {
            for entry in read_dir {
                if let Ok(entry) = entry {
                    if let Some(name) = entry.file_name().to_str() {
                        self.entries.push(name.to_string());
                    }
                }
            }
        }
        self.selected_idx = self.selected_idx.min(self.entries.len().saturating_sub(1));
        self.adjust_scroll();
        self.needs_redraw = true;
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn set_active(&mut self, active: bool) {
        if self.active != active {
            self.active = active;
            self.needs_redraw = true;
        }
    }

    fn adjust_scroll(&mut self) {
        let height = self.rect.size.height as usize;

        if height == 0 || self.entries.is_empty() {
            return;
        }

        let max_visible = height.saturating_sub(1);

        if self.selected_idx < self.scroll_top {
            self.scroll_top = self.selected_idx;
        } else if self.selected_idx > self.scroll_top + max_visible {
            self.scroll_top = self.selected_idx.saturating_sub(max_visible);
        }

        // clamp scroll_top
        let max_scroll = self.entries.len().saturating_sub(1);
        if self.scroll_top > max_scroll {
            self.scroll_top = max_scroll;
        }
    }

    pub fn handle_move_command(&mut self, command: Move) {
        let prev = self.selected_idx;
        match command {
            Move::Up => {
                self.selected_idx = self.selected_idx.saturating_sub(1);
            }
            Move::Down => {
                if self.selected_idx + 1 < self.entries.len() {
                    self.selected_idx += 1;
                }
            }
            _ => {}
        }
        self.adjust_scroll();

        if prev != self.selected_idx {
            self.needs_redraw = true;
        }
    }

    pub fn perform_selection(&mut self) -> FileExplorerAction {
        if self.entries.is_empty() {
            return FileExplorerAction::None;
        }
        if let Some(selected_name) = self.entries.get(self.selected_idx) {
            let target_path = if selected_name == ".." {
                self.current_dir.parent().map(|p| p.to_path_buf())
            } else {
                Some(self.current_dir.join(selected_name))
            };

            if let Some(path) = target_path {
                if path.is_dir() {
                    self.current_dir = path;
                    self.selected_idx = 0;
                    self.refresh_entries();
                    return FileExplorerAction::DirectoryChanged;
                } else if path.is_file() {
                    return FileExplorerAction::OpenFile(path);
                }
            }
        }
        FileExplorerAction::None
    }

    pub fn go_to_parent(&mut self) -> bool {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.selected_idx = 0;
            self.refresh_entries();
            true
        } else {
            false
        }
    }
}

impl UIComponent for FileExplorer {
    fn mark_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, rect: Rect) {
        let old_rect = self.rect;
        self.rect = rect;

        if old_rect.size != rect.size {
            self.adjust_scroll();
        }

        let size_changed = self.rect.size != rect.size;
        self.rect = rect;

        if size_changed {
            self.adjust_scroll();
            self.needs_redraw = true;
        }
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn draw(&mut self) -> Result<(), Error> {
        let rect = self.rect;
        let height = rect.size.height;
        let width = rect.size.width;

        if height < 1 || width < 1 {
            return Ok(());
        }

        for row in 0..height {
            let entry_idx = self.scroll_top + row as usize;
            if let Some(name) = self.entries.get(entry_idx) {
                let text = if entry_idx == self.selected_idx && self.active {
                    format!("> {}", name)
                } else {
                    format!("  {}", name)
                };
                let truncated_text = if text.len() > width {
                    format!("{}...", &text[..width.saturating_sub(3)])
                } else {
                    text
                };
                Terminal::print_rect(rect, row, &truncated_text)?;
            } else {
                Terminal::print_rect(rect, row, "")?;
            }
        }
        //self.mark_redraw(false);
        self.needs_redraw = false;
        Ok(())
    }
}
