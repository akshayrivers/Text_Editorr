/*
So what should a pane contain?
If I think from the perspective of PHASE I the view for now is just one bigass Pane and it supports and need
    1. Buffer
    2. viewport and scrolling
    3. cursor tracking

Ultimately in this pane we will also track the 4. paneID 5. Active status and buffer will be just a reference or will have only buffer ID
hmmmm so a pane should just not support the buffer view and its functionality that is already being handled by the View, so pane should only
be concerned with what is inside and what size and geometry.
Pane should know:
    - what it displays
    - whether it is focused
*/
use crate::{
    editor::buffers::BufferManager,
    editor::uicomponents::{FileExplorer, UIComponent, View},
    editor::Terminal,
    prelude::*,
};

pub enum PaneContent {
    TextView(View),
    PluginView(View),
    FileExplorer(FileExplorer),
    Popup(View),
}

pub struct Pane {
    pub pane_id: usize,
    pub content: PaneContent,
    pub active: bool,
    pub is_floating: bool,
    pub z_index: usize,
    pub is_minimized: bool,
    pub rect: Rect,
}

impl Pane {
    pub fn view(&self) -> Option<&View> {
        match &self.content {
            PaneContent::TextView(view)
            | PaneContent::PluginView(view)
            | PaneContent::Popup(view) => Some(view),
            PaneContent::FileExplorer(_) => None,
        }
    }

    pub fn view_mut(&mut self) -> Option<&mut View> {
        match &mut self.content {
            PaneContent::TextView(view)
            | PaneContent::PluginView(view)
            | PaneContent::Popup(view) => Some(view),
            PaneContent::FileExplorer(_) => None,
        }
    }

    pub fn component(&self) -> &dyn UIComponent {
        match &self.content {
            PaneContent::TextView(view)
            | PaneContent::PluginView(view)
            | PaneContent::Popup(view) => view,
            PaneContent::FileExplorer(explorer) => explorer,
        }
    }

    pub fn component_mut(&mut self) -> &mut dyn UIComponent {
        match &mut self.content {
            PaneContent::TextView(view)
            | PaneContent::PluginView(view)
            | PaneContent::Popup(view) => view,
            PaneContent::FileExplorer(explorer) => explorer,
        }
    }

    pub fn close_button_col(&self) -> usize {
        self.rect.position.col + self.rect.size.width.saturating_sub(4)
    }

    pub fn min_button_col(&self) -> usize {
        self.rect.position.col + self.rect.size.width.saturating_sub(8)
    }

    pub fn is_on_close_button(&self, pos: Position) -> bool {
        pos.row == self.rect.position.row
            && pos.col >= self.close_button_col()
            && pos.col < self.close_button_col() + 3
    }

    pub fn is_on_min_button(&self, pos: Position) -> bool {
        pos.row == self.rect.position.row
            && pos.col >= self.min_button_col()
            && pos.col < self.min_button_col() + 3
    }

    pub fn is_on_title_bar(&self, pos: Position) -> bool {
        pos.row == self.rect.position.row
            && pos.col >= self.rect.position.col
            && pos.col < self.rect.position.col + self.rect.size.width
    }

    pub fn resize(&mut self, rect: Rect) {
        self.rect = rect;
        self.component_mut().set_size(rect);
    }

    pub fn render(&mut self, buffer_manager: &BufferManager) {
        if !self.component().needs_redraw() && !self.active {
            // maybe we still need to redraw the frame if focus changed?
            // for now let's always check redraw
        }

        let rect = self.rect;
        let Size { height, width } = rect.size;

        if height < 1 || width < 4 {
            return;
        }

        // 1. Draw Border (Top only if minimized)
        if self.is_minimized {
            let _ = Terminal::print_at(rect.position, &"─".repeat(width as usize));
        } else {
            let _ = Terminal::draw_border(rect);
        }

        // 2. Draw Title & Buttons
        let label = if self.active {
            format!("|PANE {} (ACTIVE)|", self.pane_id)
        } else {
            format!("|PANE {}|", self.pane_id)
        };

        let truncated_label = if label.len() > width.saturating_sub(10) as usize {
            format!("{}..", &label[..width.saturating_sub(12) as usize])
        } else {
            label
        };

        let _ = Terminal::print_at(
            Position {
                col: rect.position.col.saturating_add(2),
                row: rect.position.row,
            },
            &truncated_label,
        );

        // Buttons: [-] [x] at the top right
        let close_btn_pos = Position {
            col: self.close_button_col(),
            row: rect.position.row,
        };
        let min_btn_pos = Position {
            col: self.min_button_col(),
            row: rect.position.row,
        };

        let _ = Terminal::print_at(min_btn_pos, if self.is_minimized { "[+]" } else { "[-]" });
        let _ = Terminal::print_at(close_btn_pos, "[x]");

        // 3. Draw Content
        if !self.is_minimized {
            let content_rect = Rect {
                position: Position {
                    row: rect.position.row + 1,
                    col: rect.position.col + 1,
                },
                size: Size {
                    height: height.saturating_sub(2),
                    width: width.saturating_sub(2),
                },
            };

            match &mut self.content {
                PaneContent::TextView(view)
                | PaneContent::PluginView(view)
                | PaneContent::Popup(view) => {
                    view.set_active(self.active);
                    if let Some(buffer) = buffer_manager.get(view.buffer_id()) {
                        let _ = view.draw_content_with_buffer(content_rect, buffer);
                    }
                }
                PaneContent::FileExplorer(explorer) => {
                    if explorer.rect() != content_rect {
                        explorer.set_size(content_rect);
                    }

                    if self.active != explorer.is_active() {
                        explorer.set_active(self.active);
                    }

                    if explorer.needs_redraw() || self.active {
                        let _ = explorer.draw();
                    }
                }
            }
        }

        self.component_mut().mark_redraw(false);
    }
}
