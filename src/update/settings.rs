use super::Channel;
use anyhow::{Context, Result};
use std::{fs, path::PathBuf};

const FILE_NAME: &str = "updates.conf";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Settings {
    pub automatic: bool,
    pub channel: Channel,
}

impl Settings {
    pub fn defaults(prerelease: bool) -> Self {
        Self {
            automatic: true,
            channel: if prerelease {
                Channel::Beta
            } else {
                Channel::Stable
            },
        }
    }

    fn parse(text: &str, defaults: Self) -> Self {
        text.lines().filter_map(|line| line.split_once('=')).fold(
            defaults,
            |mut settings, (key, value)| {
                match (key.trim(), value.trim()) {
                    ("automatic", "on") => settings.automatic = true,
                    ("automatic", "off") => settings.automatic = false,
                    ("channel", "stable") => settings.channel = Channel::Stable,
                    ("channel", "beta") => settings.channel = Channel::Beta,
                    _ => {}
                }
                settings
            },
        )
    }

    fn serialize(self) -> String {
        format!(
            "automatic={}\nchannel={}\n",
            if self.automatic { "on" } else { "off" },
            match self.channel {
                Channel::Stable => "stable",
                Channel::Beta => "beta",
            }
        )
    }

    pub fn load(store: Option<PathBuf>, defaults: Self) -> Result<Self> {
        let Some(store) = store else {
            return Ok(defaults);
        };
        match fs::read_to_string(store.join(FILE_NAME)) {
            Ok(text) => Ok(Self::parse(&text, defaults)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(defaults),
            Err(error) => Err(error).with_context(|| {
                format!("Could not read update settings from {}", store.display())
            }),
        }
    }

    pub fn save(self, store: Option<PathBuf>) -> Result<()> {
        let Some(store) = store else {
            return Ok(());
        };
        fs::create_dir_all(&store)
            .with_context(|| format!("Could not create {}", store.display()))?;
        let path = store.join(FILE_NAME);
        let staged = path.with_extension("conf.tmp");
        fs::write(&staged, self.serialize())
            .with_context(|| format!("Could not write {}", staged.display()))?;
        fs::rename(&staged, &path).with_context(|| format!("Could not replace {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prerelease_builds_default_to_the_beta_channel() {
        assert_eq!(Settings::defaults(true).channel, Channel::Beta);
        assert_eq!(Settings::defaults(false).channel, Channel::Stable);
        assert!(Settings::defaults(false).automatic);
    }

    #[test]
    fn saved_settings_round_trip_and_unknown_lines_are_ignored() {
        let store = std::env::temp_dir().join(format!("agenticpdf-updates-{}", std::process::id()));
        let _ = fs::remove_dir_all(&store);
        let defaults = Settings::defaults(false);
        assert_eq!(
            Settings::load(Some(store.clone()), defaults).unwrap(),
            defaults
        );
        let changed = Settings {
            automatic: false,
            channel: Channel::Beta,
        };
        changed.save(Some(store.clone())).unwrap();
        assert_eq!(
            Settings::load(Some(store.clone()), defaults).unwrap(),
            changed
        );
        assert_eq!(
            Settings::parse("automatic=maybe\nchannel=nightly\nnoise\n", defaults),
            defaults
        );
        let _ = fs::remove_dir_all(&store);
    }
}
