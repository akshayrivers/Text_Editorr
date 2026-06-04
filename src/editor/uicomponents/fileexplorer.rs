use crate::editor::command::Move;
use crate::editor::uicomponents::UIComponent;
use crate::prelude::*;
use std::fs;
use std::io::Error;
use std::path::PathBuf;

pub struct FileExplorer {
    current_dir: PathBuf,
    entries: Vec<String>,
    selected_idx: usize,
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
            rect: Rect::default(),
            needs_redraw: true,
            active: false,
        };
        explorer.refresh_entries();
        explorer
    }
}

impl FileExplorer {
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
        self.mark_redraw(true);
    }

    pub fn set_active(&mut self, active: bool) {
        if self.active != active {
            self.active = active;
            self.mark_redraw(true);
        }
    }

    pub fn handle_move_command(&mut self, command: Move) {
        match command {
            Move::Up => {
                self.selected_idx = self.selected_idx.saturating_sub(1);
            }
            Move::Down => {
                if self.selected_idx < self.entries.len().saturating_sub(1) {
                    self.selected_idx += 1;
                }
            }
            _ => {}
        }
        self.mark_redraw(true);
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
        self.rect = rect;
        self.mark_redraw(true);
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn draw(&mut self) -> Result<(), Error> {
        use crate::editor::Terminal;

        let rect = self.rect;
        let height = rect.size.height;
        let width = rect.size.width;

        if height < 1 || width < 1 {
            return Ok(());
        }

        for row in 0..height {
            let entry_idx = row as usize; // Simple listing for now
            if let Some(name) = self.entries.get(entry_idx) {
                let text = if entry_idx == self.selected_idx && self.active {
                    format!("> {}", name)
                } else {
                    format!("  {}", name)
                };
                Terminal::print_rect(rect, row, &text)?;
            } else {
                Terminal::print_rect(rect, row, "")?;
            }
        }
        self.mark_redraw(false);
        Ok(())
    }
}
