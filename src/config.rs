use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbOpts {
    /// if Some, enable compression and set factor to this
    pub compression_factor: Option<i32>,
    /// if true, print profile stats when database is dropped
    pub debug: bool,
    pub mode: Option<DbMode>,
    pub path: String,
    /// size of system page cache in bytes
    pub system_page_cache: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DbMode {
    LowSpace,
    Fast,
}

impl Default for DbMode {
    fn default() -> Self {
        Self::Fast
    }
}

impl Into<sled::Mode> for DbMode {
    fn into(self) -> sled::Mode {
        match self {
            DbMode::LowSpace => sled::Mode::LowSpace,
            DbMode::Fast => sled::Mode::HighThroughput,
        }
    }
}

impl Into<sled::Config> for &DbOpts {
    fn into(self) -> sled::Config {
        let mut sled_config = sled::Config::new();
        sled_config = sled_config.path(self.path.clone());
        if let Some(cache) = self.system_page_cache.as_ref() {
            sled_config = sled_config.cache_capacity(*cache);
        }
        if let Some(compression) = self.compression_factor.as_ref() {
            sled_config = sled_config.use_compression(true);
            sled_config = sled_config.compression_factor(*compression);
        }
        if let Some(mode) = self.mode {
            sled_config = sled_config.mode(mode.into());
        }
        if self.debug {
            sled_config = sled_config.print_profile_on_drop(true);
        }
        sled_config
    }
}

impl Default for DbOpts {
    fn default() -> Self {
        Self {
            path: "test_infos.db".to_string(),
            system_page_cache: None,
            compression_factor: None,
            mode: Default::default(),
            debug: false,
        }
    }
}
