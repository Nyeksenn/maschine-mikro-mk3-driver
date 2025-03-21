use config::{Config, ConfigError, Environment, File};
use dirs::config_dir;
use serde::{Deserialize};

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub(crate) struct Main {
    pub base_key: String,
    pub use_aftertouch: bool
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub(crate) struct Settings {
    pub main: Main,
}

impl Settings {
    pub(crate) fn new() -> Result<Self, ConfigError> {
        let mut s = Config::builder()
            .add_source(Environment::with_prefix("APP"));
        let conf_dir = config_dir();
        s = if let Some(mut conf_dir) = conf_dir {
            conf_dir.push("apparat");
            s.add_source(File::with_name(conf_dir.to_str().unwrap()))
        } else {
            s
        };
        let config = s.build()?;

        config.try_deserialize()
    }
}
