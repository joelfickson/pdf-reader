use std::{cmp::Ordering, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Stage {
    Beta,
    Rc,
    Stable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Version {
    core: [u64; 3],
    stage: Stage,
    number: u64,
}

impl Version {
    pub fn parse_tag(tag: &str) -> Option<Self> {
        let text = tag.strip_prefix('v')?;
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        let mut parts = core.split('.');
        let mut numbers = [0u64; 3];
        for slot in &mut numbers {
            *slot = number(parts.next()?)?;
        }
        if parts.next().is_some() {
            return None;
        }
        let (stage, count) = match pre {
            None => (Stage::Stable, 0),
            Some(pre) => {
                let (name, count) = pre.split_once('.')?;
                let stage = match name {
                    "beta" => Stage::Beta,
                    "rc" => Stage::Rc,
                    _ => return None,
                };
                (stage, number(count)?)
            }
        };
        Some(Self {
            core: numbers,
            stage,
            number: count,
        })
    }

    pub fn is_prerelease(&self) -> bool {
        self.stage != Stage::Stable
    }
}

fn number(text: &str) -> Option<u64> {
    let valid = !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    valid.then(|| text.parse().ok()).flatten()
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then(self.stage.cmp(&other.stage))
            .then(self.number.cmp(&other.number))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [major, minor, patch] = self.core;
        write!(formatter, "{major}.{minor}.{patch}")?;
        match self.stage {
            Stage::Stable => Ok(()),
            Stage::Beta => write!(formatter, "-beta.{}", self.number),
            Stage::Rc => write!(formatter, "-rc.{}", self.number),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(tag: &str) -> Version {
        Version::parse_tag(tag).unwrap()
    }

    #[test]
    fn release_tags_order_prereleases_before_their_stable_version() {
        let ordered = [
            "v0.1.1",
            "v0.1.2-beta.1",
            "v0.1.2-beta.2",
            "v0.1.2-beta.10",
            "v0.1.2-rc.1",
            "v0.1.2",
            "v0.1.10-beta.1",
            "v0.2.0",
            "v1.0.0",
        ];
        for pair in ordered.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn display_drops_the_tag_prefix() {
        assert_eq!(v("v0.1.2-beta.2").to_string(), "0.1.2-beta.2");
        assert_eq!(v("v1.20.3").to_string(), "1.20.3");
        assert!(v("v0.1.2-rc.1").is_prerelease());
        assert!(!v("v0.1.2").is_prerelease());
    }

    #[test]
    fn malformed_tags_are_rejected() {
        for tag in [
            "0.1.2",
            "v0.1",
            "v0.1.2.3",
            "v0.1.2-alpha.1",
            "v0.1.2-beta",
            "v0.1.2-beta.x",
            "v01.1.2",
            "v0.1.2-beta.01",
            "v0.1.+2",
            "",
        ] {
            assert!(Version::parse_tag(tag).is_none(), "{tag}");
        }
    }
}
