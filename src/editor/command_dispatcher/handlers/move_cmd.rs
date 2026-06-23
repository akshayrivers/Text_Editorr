use super::{CommandHandler, EditorContext};
use crate::editor::command::Command;
use crate::editor::layout::PaneContent;

pub struct MoveHandler;

impl CommandHandler for MoveHandler {
    fn can_handle(&self, command: &Command) -> bool {
        matches!(command, Command::Move(_))
    }

    fn handle(&mut self, command: &Command, ctx: &mut EditorContext) -> Result<(), String> {
        if let Command::Move(move_cmd) = command {
            if let Some(pane) = ctx.pane_manager.active_pane_mut() {
                match &mut pane.content {
                    PaneContent::TextView(view) => {
                        let buffer_id = view.buffer_id();
                        if let Some(buffer) = ctx.buffer_manager.get(buffer_id) {
                            view.handle_move_command(*move_cmd, buffer);
                        }
                    }
                    PaneContent::FileExplorer(explorer) => {
                        explorer.handle_move_command(*move_cmd);
                    }
                    _ => {}
                }
            }
            Ok(())
        } else {
            Err("Not a Move command".to_string())
        }
    }
}
