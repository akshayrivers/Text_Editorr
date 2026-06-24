pub mod builtin;
pub mod runtime;

pub use runtime::PluginRuntime;

use crate::editor::events::EditorEvent;
use crate::{editor, prelude::*};
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct BufferSnapshot {
    pub buffer_id: usize,
    pub lines: Vec<String>,
    pub file_name: Option<String>,
    pub is_dirty: bool,
}

// PluginMessage

// NOTE: Box<dyn Plugin> is Send + Sync so this is safe to send across threads.
// We can't derive Clone/Debug because Box<dyn Plugin> doesn't impl them,
// so we don't derive — the channel doesn't need it.
pub enum PluginMessage {
    /// Load a new plugin into the runtime.
    LoadPlugin(Box<dyn Plugin>),
    /// A raw EditorEvent cloned after core has handled it.
    Event(EditorEvent),
    /// A buffer was modified — here is a snapshot.
    BufferChanged(BufferSnapshot),
    /// Shut down the runtime.
    Shutdown,
}

// PluginResponse

pub enum PluginResponse {
    OpenFloatingPane {
        plugin_name: String,
        content_factory: Box<dyn FnOnce() -> crate::editor::layout::PaneContent + Send>,
        rect: Rect,
    },
    ClosePane {
        pane_id: usize,
    },
    UpdateMessage(String),
    EmitCustomEvent(editor::events::customevent::CustomEvent),
    RequestSnapshot {
        buffer_id: usize,
    },
}

impl std::fmt::Debug for PluginResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenFloatingPane {
                plugin_name, rect, ..
            } => {
                write!(f, "OpenFloatingPane({plugin_name}, {rect:?})")
            }
            Self::ClosePane { pane_id } => write!(f, "ClosePane({pane_id})"),
            Self::UpdateMessage(m) => write!(f, "UpdateMessage({m})"),
            Self::EmitCustomEvent(_) => write!(f, "EmitCustomEvent"),
            Self::RequestSnapshot { buffer_id } => write!(f, "RequestSnapshot({buffer_id})"),
        }
    }
}

// Plugin trait

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;

    async fn on_load(&mut self) {}
    async fn on_unload(&mut self) {}

    async fn on_event(&mut self, _event: &EditorEvent) -> Option<PluginResponse> {
        None
    }

    async fn on_buffer_change(&mut self, _snapshot: BufferSnapshot) -> Option<PluginResponse> {
        None
    }
}
