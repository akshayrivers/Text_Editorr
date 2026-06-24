// FileExplorerPlugin — the file explorer as a first-class async plugin.
//
// On Ctrl+E (or whatever keybind triggers it), the plugin responds to
// the EditorEvent::Key and sends back an OpenFloatingPane response.
// The FileExplorer UIComponent lives in the pane — the plugin just
// manages lifecycle and reacts to events.

use crate::editor::events::keyboard::{KeyCode, KeyModifiers};
use crate::editor::events::EditorEvent;
use crate::editor::layout::{Pane, PaneContent};
use crate::editor::plugins::{BufferSnapshot, Plugin, PluginResponse};
use crate::editor::uicomponents::FileExplorer;
use crate::prelude::*;
use async_trait::async_trait;

pub struct FileExplorerPlugin {
    /// Track pane id so we can toggle (close if already open).
    open_pane_id: Option<usize>,
}

impl FileExplorerPlugin {
    pub fn new() -> Self {
        Self { open_pane_id: None }
    }
}

impl Default for FileExplorerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for FileExplorerPlugin {
    fn name(&self) -> &str {
        "file_explorer"
    }

    async fn on_load(&mut self) {
        // Nothing to init for file explorer
    }

    async fn on_event(&mut self, event: &EditorEvent) -> Option<PluginResponse> {
        // Trigger on Ctrl+E
        if let EditorEvent::Key(key) = event {
            let is_ctrl_e =
                key.modifiers == KeyModifiers::CTRL && key.key_code == KeyCode::Char('e');

            if is_ctrl_e {
                // If already open, close it
                if let Some(pane_id) = self.open_pane_id.take() {
                    return Some(PluginResponse::ClosePane { pane_id });
                }

                // Otherwise open a new floating pane
                return Some(PluginResponse::OpenFloatingPane {
                    plugin_name: self.name().to_string(),
                    content_factory: Box::new(|| {
                        PaneContent::Plugin(Box::new(FileExplorer::default()))
                    }),
                    rect: Rect {
                        position: Position { row: 2, col: 2 },
                        size: Size {
                            height: 20,
                            width: 40,
                        },
                    },
                });
            }
        }
        None
    }

    async fn on_buffer_change(&mut self, _snapshot: BufferSnapshot) -> Option<PluginResponse> {
        // File explorer doesn't care about buffer changes
        None
    }
}
