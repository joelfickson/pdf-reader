use super::{Channel, REPOSITORY, version::Version};
use serde::Deserialize;

pub(crate) const SUMS_NAME: &str = "SHA256SUMS.txt";
const NOTE_LIMIT: usize = 3;
const NOTE_KINDS: [&str; 3] = ["feat", "fix", "perf"];

#[derive(Deserialize)]
pub(crate) struct ApiRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    html_url: String,
    body: Option<String>,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Asset {
    pub name: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Release {
    pub version: Version,
    pub tag: String,
    pub page_url: String,
    pub notes: Vec<String>,
    pub archive: Asset,
    pub sums: Asset,
}

fn asset(release: &ApiRelease, name: &str) -> Option<Asset> {
    let expected = format!(
        "https://github.com/{REPOSITORY}/releases/download/{}/{name}",
        release.tag_name
    );
    release
        .assets
        .iter()
        .find(|asset| asset.name == name && asset.browser_download_url == expected)
        .map(|asset| Asset {
            name: asset.name.clone(),
            size: asset.size,
            url: asset.browser_download_url.clone(),
        })
}

pub(crate) fn select(
    releases: Vec<ApiRelease>,
    current: &Version,
    channel: Channel,
    archive_name: &str,
) -> Option<Release> {
    releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = Version::parse_tag(&release.tag_name)?;
            let prerelease = release.prerelease || version.is_prerelease();
            if (channel == Channel::Stable && prerelease) || version <= *current {
                return None;
            }
            let archive = asset(&release, archive_name)?;
            let sums = asset(&release, SUMS_NAME)?;
            Some(Release {
                version,
                notes: notes(release.body.as_deref().unwrap_or_default()),
                tag: release.tag_name,
                page_url: release.html_url,
                archive,
                sums,
            })
        })
        .max_by(|left, right| left.version.cmp(&right.version))
}

fn note(line: &str) -> Option<String> {
    let entry = line.trim().strip_prefix("* ")?;
    let entry = entry.rsplit_once(" by @").map_or(entry, |(title, _)| title);
    let (kind, title) = entry.split_once(": ")?;
    let kind = kind.split_once('(').map_or(kind, |(kind, _)| kind);
    if !NOTE_KINDS.contains(&kind) {
        return None;
    }
    let mut letters = title.trim().chars();
    let first = letters.next()?;
    Some(first.to_uppercase().chain(letters).collect())
}

pub(crate) fn notes(body: &str) -> Vec<String> {
    body.lines().filter_map(note).take(NOTE_LIMIT).collect()
}

pub(crate) fn checksum<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        let file = file.trim_start();
        let file = file.strip_prefix('*').unwrap_or(file);
        (file == name && hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then_some(hash)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARCHIVE: &str = "pdf-reader-macos-arm64.zip";

    fn release(tag: &str, prerelease: bool, assets: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "draft": false,
            "prerelease": prerelease,
            "html_url": format!("https://github.com/{REPOSITORY}/releases/tag/{tag}"),
            "body": "## What's Changed\n* feat: scroll pages as one continuous column by @joelfickson in https://github.com/x/pull/52\n",
            "assets": assets.iter().map(|name| serde_json::json!({
                "name": name,
                "size": 100,
                "browser_download_url": format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{name}"),
            })).collect::<Vec<_>>(),
        })
    }

    fn parse(values: Vec<serde_json::Value>) -> Vec<ApiRelease> {
        serde_json::from_value(serde_json::Value::Array(values)).unwrap()
    }

    fn pick(values: Vec<serde_json::Value>, current: &str, channel: Channel) -> Option<String> {
        select(
            parse(values),
            &Version::parse_tag(current).unwrap(),
            channel,
            ARCHIVE,
        )
        .map(|release| release.tag)
    }

    fn both() -> [&'static str; 2] {
        [ARCHIVE, SUMS_NAME]
    }

    #[test]
    fn stable_channel_ignores_prereleases() {
        let releases = vec![
            release("v0.2.0-beta.1", true, &both()),
            release("v0.1.3", false, &both()),
            release("v0.1.2", false, &both()),
        ];
        assert_eq!(
            pick(releases, "v0.1.2", Channel::Stable).as_deref(),
            Some("v0.1.3")
        );
    }

    #[test]
    fn beta_channel_takes_the_highest_version_of_either_kind() {
        let releases = vec![
            release("v0.1.3", false, &both()),
            release("v0.2.0-beta.1", true, &both()),
        ];
        assert_eq!(
            pick(releases, "v0.1.2-beta.2", Channel::Beta).as_deref(),
            Some("v0.2.0-beta.1")
        );
        let releases = vec![
            release("v0.1.2", false, &both()),
            release("v0.1.2-beta.3", true, &both()),
        ];
        assert_eq!(
            pick(releases, "v0.1.2-beta.2", Channel::Beta).as_deref(),
            Some("v0.1.2")
        );
    }

    #[test]
    fn nothing_newer_or_incomplete_releases_are_skipped() {
        assert_eq!(
            pick(
                vec![release("v0.1.2", false, &both())],
                "v0.1.2",
                Channel::Stable
            ),
            None
        );
        let releases = vec![
            release("v0.1.4", false, &[ARCHIVE]),
            release("v0.1.5", false, &[SUMS_NAME, "pdf-reader-windows-x64.zip"]),
            release("not-a-version", false, &both()),
        ];
        assert_eq!(pick(releases, "v0.1.2", Channel::Stable), None);
        let mut draft = release("v0.1.6", false, &both());
        draft["draft"] = true.into();
        assert_eq!(pick(vec![draft], "v0.1.2", Channel::Stable), None);
        let mut flagged = release("v0.1.7", true, &both());
        flagged["prerelease"] = true.into();
        assert_eq!(pick(vec![flagged], "v0.1.2", Channel::Stable), None);
    }

    #[test]
    fn assets_hosted_elsewhere_are_rejected() {
        let mut moved = release("v0.1.3", false, &both());
        moved["assets"][0]["browser_download_url"] =
            "https://example.com/pdf-reader-macos-arm64.zip".into();
        assert_eq!(pick(vec![moved], "v0.1.2", Channel::Stable), None);
    }

    #[test]
    fn notes_keep_user_facing_changes_only() {
        let body = "## What's Changed\n\
            * chore: keep design mocks local by @a in https://x/pull/23\n\
            * feat: open PDFs from Finder and remember recent files by @a in https://x/pull/38\n\
            * chore(deps): bump react from 19.3.0 to 19.3.1 by @dependabot[bot] in https://x/pull/37\n\
            * fix(reader): restore the scroll offset by @a in https://x/pull/60\n\
            * perf: benchmark launch by @a in https://x/pull/44\n\
            * feat: one more by @a in https://x/pull/70\n";
        assert_eq!(
            notes(body),
            [
                "Open PDFs from Finder and remember recent files",
                "Restore the scroll offset",
                "Benchmark launch",
            ]
        );
        assert!(notes("").is_empty());
    }

    #[test]
    fn checksums_are_found_by_exact_file_name() {
        let hash = "a".repeat(64);
        let other = "b".repeat(64);
        let sums = format!(
            "{other}  pdf-reader-macos-x64.zip\n{hash}  {ARCHIVE}\n{other} *pdf-reader-windows-x64.zip\nshort  bad.zip\n"
        );
        assert_eq!(checksum(&sums, ARCHIVE), Some(hash.as_str()));
        assert_eq!(
            checksum(&sums, "pdf-reader-windows-x64.zip"),
            Some(other.as_str())
        );
        assert_eq!(checksum(&sums, "bad.zip"), None);
        assert_eq!(checksum(&sums, "missing.zip"), None);
    }
}
