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

    pub fn resize(&mut self, rect: Rect) {
        self.component_mut().set_size(rect);
    }
    pub fn render(&mut self, buffer_manager: &BufferManager) {
        if !self.component().needs_redraw() && !self.active {
            // maybe we still need to redraw the frame if focus changed?
            // for now let's always check redraw
        }

        let rect = self.component().rect();
        let Size { height, width } = rect.size;

        if height < 2 || width < 4 {
            return;
        }

        // 1. Draw Border
        let _ = Terminal::draw_border(rect);

        // 2. Draw Title & Buttons
        let label = if self.active {
            format!("|PANE {} (ACTIVE)|", self.pane_id)
        } else {
            format!("|PANE {}|", self.pane_id)
        };
        let _ = Terminal::print_at(
            Position {
                col: rect.position.col.saturating_add(2),
                row: rect.position.row,
            },
            &label,
        );

        // Buttons: [_] [x] at the top right
        let close_btn_pos = Position {
            col: rect.position.col + width.saturating_sub(6),
            row: rect.position.row,
        };
        let min_btn_pos = Position {
            col: rect.position.col + width.saturating_sub(12),
            row: rect.position.row,
        };

        let _ = Terminal::print_at(min_btn_pos, "[-]");
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
                    explorer.set_active(self.active);
                    explorer.set_size(content_rect);
                    let _ = explorer.draw();
                }
            }
        }

        self.component_mut().mark_redraw(false);
    }
}
