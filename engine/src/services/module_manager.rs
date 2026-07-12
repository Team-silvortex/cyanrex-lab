use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::models::module::ModuleInfo;

#[derive(Clone, Default)]
pub struct ModuleManager {
    inner: Arc<RwLock<HashMap<String, ModuleInfo>>>,
}

impl ModuleManager {
    pub fn list(&self) -> Vec<ModuleInfo> {
        let guard = self.inner.read().expect("module manager lock poisoned");
        guard.values().cloned().collect()
    }

    pub fn start(&self, name: &str) -> ModuleInfo {
        let mut guard = self.inner.write().expect("module manager lock poisoned");
        let module = ModuleInfo {
            name: name.to_string(),
            status: "running".to_string(),
        };
        guard.insert(name.to_string(), module.clone());
        module
    }

    pub fn stop(&self, name: &str) -> ModuleInfo {
        let mut guard = self.inner.write().expect("module manager lock poisoned");
        let module = ModuleInfo {
            name: name.to_string(),
            status: "stopped".to_string(),
        };
        guard.insert(name.to_string(), module.clone());
        module
    }
}
