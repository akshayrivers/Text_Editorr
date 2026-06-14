use crate::editor::command::Command;
use crate::editor::Editor;
use crate::prelude::*;

pub trait Plugin {
    fn name(&self) -> &str;
    fn on_command(&mut self, _editor: &mut Editor, _command: &Command) -> bool {
        false
    }
    fn update(&mut self, _editor: &mut Editor) {}
}

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }
}

impl PluginManager {
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub fn handle_command(&mut self, editor: &mut Editor, command: &Command) -> bool {
        for plugin in &mut self.plugins {
            if plugin.on_command(editor, command) {
                return true;
            }
        }
        false
    }

    pub fn update_all(&mut self, editor: &mut Editor) {
        for plugin in &mut self.plugins {
            plugin.update(editor);
        }
    }
}
