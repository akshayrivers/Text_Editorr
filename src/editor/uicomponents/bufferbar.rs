use super::UIComponent;
use crate::editor::buffers::BufferManager;
use crate::editor::layout::PaneManager;
use crate::editor::Terminal;
use crate::prelude::*;
use std::io::Error;

#[derive(Default)]
pub struct BufferBar {
    rect: Rect,
    needs_redraw: bool,
}

impl BufferBar {
    pub fn render(
        &mut self,
        buffer_manager: &BufferManager,
        pane_manager: &PaneManager,
    ) -> Result<(), Error> {
        let width = self.rect.size.width;
        let mut current_col = 0;

        Terminal::clear_rect_line(self.rect, self.rect.position.row)?;

        // 1. Render Open Buffers
        let mut buffer_tabs = String::new();
        for (id, buffer) in buffer_manager.iter() {
            let name = buffer
                .get_file_info()
                .get_path()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("untitled");
            let tab = format!(" [{}: {}] ", id, name);
            buffer_tabs.push_str(&tab);
        }

        if !buffer_tabs.is_empty() {
            Terminal::print_at(
                self.rect.position,
                &format!(
                    "{}{}{}",
                    crossterm::style::Attribute::Reverse,
                    buffer_tabs,
                    crossterm::style::Attribute::Reset,
                ),
            )?;
            current_col += buffer_tabs.len();
        }

        // 2. Render Minimized Panes
        let minimized_panes: Vec<_> = pane_manager.iter().filter(|p| p.is_minimized).collect();
        if !minimized_panes.is_empty() {
            let mut min_str = String::from(" | MIN: ");
            for pane in minimized_panes {
                min_str.push_str(&format!("<P{}> ", pane.pane_id));
            }

            if current_col + min_str.len() < width as usize {
                Terminal::print_at(
                    Position {
                        row: self.rect.position.row,
                        col: self.rect.position.col + current_col as usize,
                    },
                    &min_str,
                )?;
            }
        }

        self.needs_redraw = false;
        Ok(())
    }
}

impl UIComponent for BufferBar {
    fn mark_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }
    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn set_size(&mut self, rect: Rect) {
        self.rect = rect;
    }
    fn draw(&mut self) -> Result<(), Error> {
        // This requires BufferManager and PaneManager, so we use the custom render method instead
        Ok(())
    }
}
