use std::path::Path;
use std::{cmp, fmt, thread};

use anyhow::format_err;
use serde::{Deserialize, Serialize};
use tracing::debug;
use url::Url;

fn default_min_sleep_secs() -> u64 {
    15
}

fn default_max_sleep_secs() -> u64 {
    120
}

fn default_max_sleep_after_hours() -> u64 {
    24
}

/// Persistent config (`/var/lib/npcnix/config.json`)
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    remote: Option<Url>,
    configuration: Option<String>,
    last_reconfiguration: chrono::DateTime<chrono::Utc>,
    last_etag: String,
    #[serde(default = "default_min_sleep_secs")]
    min_sleep_secs: u64,
    #[serde(default = "default_max_sleep_secs")]
    max_sleep_secs: u64,
    #[serde(default = "default_max_sleep_after_hours")]
    max_sleep_after_hours: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            remote: None,
            configuration: None,
            last_reconfiguration: chrono::Utc::now(),
            last_etag: "".into(),
            min_sleep_secs: default_min_sleep_secs(),
            max_sleep_secs: default_max_sleep_secs(),
            max_sleep_after_hours: default_max_sleep_after_hours(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        Ok(serde_json::from_reader(std::fs::File::open(path)?)?)
    }

    pub fn store(&self, path: &Path) -> anyhow::Result<()> {
        crate::misc::store_json_pretty_to_file(path, self)
    }

    pub fn with_configuration(self, configuration: &str) -> Self {
        Self {
            configuration: Some(configuration.into()),
            ..self
        }
    }

    /// Like [`Self::with_configuration`] but if `init` is `true` will not
    /// overwrite the existing value
    pub fn with_configuration_maybe_init(self, configuration: &str, init: bool) -> Self {
        if !init || self.configuration.is_none() {
            self.with_configuration(configuration)
        } else {
            self
        }
    }

    pub fn with_remote(self, remote: &Url) -> Self {
        Self {
            remote: Some(remote.clone()),
            ..self
        }
    }

    /// Like [`Self:with_remote`] but if `init` is `true` will not overwrite the
    /// existing value
    pub fn with_remote_maybe_init(self, remote: &Url, init: bool) -> Self {
        if !init || self.remote.is_none() {
            self.with_remote(remote)
        } else {
            self
        }
    }

    pub fn with_updated_last_reconfiguration(self, etag: &str) -> Self {
        Self {
            last_etag: etag.to_owned(),
            last_reconfiguration: chrono::Utc::now(),
            ..self
        }
    }

    pub fn remote(&self) -> anyhow::Result<&Url> {
        self.remote
            .as_ref()
            .ok_or_else(|| format_err!("Remote not set"))
    }

    pub fn configuration(&self) -> anyhow::Result<&str> {
        self.configuration
            .as_deref()
            .ok_or_else(|| format_err!("configuration not set"))
    }

    pub fn cur_rng_sleep_time(&self) -> chrono::Duration {
        use rand::Rng;

        let since_last_update = cmp::max(
            chrono::Duration::seconds(1),
            chrono::Utc::now() - self.last_reconfiguration,
        );

        let duration_ratio = (since_last_update.num_seconds() as f32
            / self.max_sleep_after_hours.saturating_mul(60 * 60) as f32)
            .clamp(0f32, 1f32);
        assert!(0f32 <= duration_ratio);

        let avg_duration_secs = (self.min_sleep_secs as f32
            + duration_ratio * self.max_sleep_secs.saturating_sub(self.min_sleep_secs) as f32)
            .clamp(0.01, 60f32 * 60f32);
        let rnd_time =
            rand::thread_rng().gen_range(avg_duration_secs * 0.5..=avg_duration_secs * 1.5);
        assert!(0f32 < rnd_time);

        chrono::Duration::seconds(cmp::max(self.min_sleep_secs as i64, rnd_time as i64))
    }

    pub fn rng_sleep(&self) {
        let duration = self.cur_rng_sleep_time();
        debug!(duration = %duration, "Sleeping");
        thread::sleep(duration.to_std().expect("Can't be negative"));
    }

    pub fn last_etag(&self) -> &str {
        &self.last_etag
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&serde_json::to_string_pretty(self).map_err(|_e| fmt::Error)?)
    }
}
