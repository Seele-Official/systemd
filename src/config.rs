use std::collections::HashMap;
use serde::Deserialize;
use toml::Value;
use once_cell::sync::Lazy;
use std::sync::RwLock;
use std::fs;
use core::fmt;

#[derive(Deserialize, Debug, Clone)]
pub struct Unit {
    pub name: String,
    pub description: Option<String>,

    #[serde(flatten)]
    pub other: HashMap<String, Value>,
}

#[derive(Deserialize, PartialEq, Debug, Clone)]
pub enum ServiceType {
    Simple,
    Startup
}

#[derive(Deserialize, Debug, Clone)]
pub struct Service {
    #[serde(rename = "type")]
    pub style: ServiceType,
    pub path: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,

    #[serde(flatten)]
    pub other: HashMap<String, Value>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub unit: Unit,
    pub service: Service,
    
    #[serde(flatten)]
    pub other: HashMap<String, Value>,
}




static CONFIG_MAP: Lazy<RwLock<HashMap<String, Config>>> = Lazy::new(|| { RwLock::new(HashMap::new()) });

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Toml(toml::de::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "Io({})", e),
            Error::Toml(e) => write!(f, "Toml({})", e),
        }
    }
    
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Self {
        Error::Toml(err)
    }
}

pub fn load() -> Result<(), Error> {
    let mut config_map = CONFIG_MAP.write().unwrap();

    config_map.clear();

    let config_path = 
        std::env::current_exe()?
            .parent()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Could not find parent directory")
            })?
            .join("configs");
    log::info!("Loading configuration from: {:?}", config_path);
    for entry in fs::read_dir(config_path)? {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() {
                    let content = fs::read_to_string(&path)?;
                    let config: Config = toml::from_str(&content)?;
                    config_map.insert(config.unit.name.clone(), config);
                }
            }
            Err(err) => {
                return Err(Error::Io(err));
            }
        }
    }

    Ok(())
}



pub fn get<F, R>(name: &str, f: F) -> Option<R>
where
    F: FnOnce(&Config) -> R,
{
    let config_map = CONFIG_MAP.read().unwrap(); 
    config_map.get(name).map(f)
}



pub fn for_each<F>(mut f: F)
where
    F: FnMut(&Config),
{
    let config_map = CONFIG_MAP.read().unwrap();
    for config in config_map.values() {
        f(config);
    }
}
