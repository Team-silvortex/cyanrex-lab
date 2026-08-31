use crate::{
    models::command::{CommandRequest, CommandResponse, CommandType},
    services::module_manager::ModuleManager,
};

#[derive(Clone)]
pub struct CommandDispatcher {
    module_manager: ModuleManager,
}

impl CommandDispatcher {
    pub fn new(module_manager: ModuleManager) -> Self {
        Self { module_manager }
    }

    pub async fn dispatch(&self, command: CommandRequest) -> CommandResponse {
        match command.command_type {
            CommandType::ListModules => {
                let modules = self.module_manager.list();
                CommandResponse {
                    ok: true,
                    command_type: CommandType::ListModules,
                    message: format!("listed {} module(s)", modules.len()),
                    modules: Some(modules),
                    module: None,
                    next_path: None,
                }
            }
            CommandType::StartModule => {
                let Some(module_name) = valid_module_name(command.module_name) else {
                    return invalid_module_name(CommandType::StartModule);
                };
                match self.module_manager.start(&module_name) {
                    Ok(module) => CommandResponse {
                        ok: true,
                        command_type: CommandType::StartModule,
                        message: format!("module {} started", module.name),
                        modules: None,
                        module: Some(module),
                        next_path: None,
                    },
                    Err(error) => module_error(CommandType::StartModule, error.to_string()),
                }
            }
            CommandType::StopModule => {
                let Some(module_name) = valid_module_name(command.module_name) else {
                    return invalid_module_name(CommandType::StopModule);
                };
                match self.module_manager.stop(&module_name) {
                    Ok(module) => CommandResponse {
                        ok: true,
                        command_type: CommandType::StopModule,
                        message: format!("module {} stopped", module.name),
                        modules: None,
                        module: Some(module),
                        next_path: None,
                    },
                    Err(error) => module_error(CommandType::StopModule, error.to_string()),
                }
            }
            CommandType::RunExperiment => CommandResponse {
                ok: true,
                command_type: CommandType::RunExperiment,
                message: "open the eBPF workspace to configure and run an experiment".to_string(),
                modules: None,
                module: None,
                next_path: Some("/ebpf".to_string()),
            },
        }
    }
}

fn valid_module_name(raw: Option<String>) -> Option<String> {
    let name = raw?.trim().to_string();
    if name.is_empty()
        || name.len() > 128
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return None;
    }
    Some(name)
}

fn invalid_module_name(command_type: CommandType) -> CommandResponse {
    CommandResponse {
        ok: false,
        command_type,
        message: "module name is required and may contain only letters, numbers, '.', '-' or '_'"
            .to_string(),
        modules: None,
        module: None,
        next_path: None,
    }
}

fn module_error(command_type: CommandType, message: String) -> CommandResponse {
    CommandResponse {
        ok: false,
        command_type,
        message,
        modules: None,
        module: None,
        next_path: None,
    }
}
