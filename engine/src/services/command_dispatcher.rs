use crate::{
    models::command::{CommandRequest, CommandType},
    services::{event_bus::EventBus, module_manager::ModuleManager},
};

#[derive(Clone)]
pub struct CommandDispatcher {
    module_manager: ModuleManager,
    event_bus: EventBus,
}

impl CommandDispatcher {
    pub fn new(module_manager: ModuleManager, event_bus: EventBus) -> Self {
        Self {
            module_manager,
            event_bus,
        }
    }

    pub async fn dispatch(&self, command: CommandRequest) -> bool {
        match command.command_type {
            CommandType::ListModules => {
                let _ = self.module_manager.list();
                true
            }
            CommandType::StartModule => {
                if let Some(module_name) = command.module_name {
                    let _ = self.module_manager.start(&module_name);
                    true
                } else {
                    false
                }
            }
            CommandType::StopModule => {
                if let Some(module_name) = command.module_name {
                    let _ = self.module_manager.stop(&module_name);
                    true
                } else {
                    false
                }
            }
            CommandType::RunExperiment => true,
        }
    }
}
