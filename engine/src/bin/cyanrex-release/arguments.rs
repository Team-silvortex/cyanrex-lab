use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub struct ParsedArguments {
    pub positionals: Vec<String>,
    options: BTreeMap<String, String>,
}

impl ParsedArguments {
    pub fn new(arguments: &[String], allowed: &[&str]) -> Result<Self, String> {
        let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
        let mut positionals = Vec::new();
        let mut options = BTreeMap::new();
        let mut index = 0;
        while index < arguments.len() {
            let argument = &arguments[index];
            if !argument.starts_with("--") {
                positionals.push(argument.clone());
                index += 1;
                continue;
            }
            if !allowed.contains(argument.as_str()) {
                return Err(format!("unknown option {argument:?}"));
            }
            let value = arguments
                .get(index + 1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| format!("{argument} requires a value"))?;
            if options.insert(argument.clone(), value.clone()).is_some() {
                return Err(format!("option {argument} may only be supplied once"));
            }
            index += 2;
        }
        Ok(Self {
            positionals,
            options,
        })
    }

    pub fn required(&self, name: &str) -> Result<&str, String> {
        self.options
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("missing required option {name}"))
    }

    pub fn required_path(&self, name: &str) -> Result<PathBuf, String> {
        self.required(name).map(PathBuf::from)
    }

    pub fn optional(&self, name: &str) -> Option<String> {
        self.options.get(name).cloned()
    }

    pub fn optional_path(&self, name: &str) -> Option<PathBuf> {
        self.options.get(name).map(PathBuf::from)
    }
}
