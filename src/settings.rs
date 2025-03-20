use config::{Config, ConfigError, Environment, File};
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

        let s = Config::builder()
            .add_source(File::with_name("config"))
            .add_source(Environment::with_prefix("APP"))
            .build()?;

        s.try_deserialize()
    }
}
