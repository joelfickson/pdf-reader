use super::{
    REPOSITORY,
    release::{ApiRelease, Asset},
};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufWriter, Read, Write},
    path::Path,
    time::Duration,
};

const RELEASES_LIMIT: u64 = 4 * 1024 * 1024;
const SUMS_LIMIT: u64 = 64 * 1024;
const CHUNK: usize = 64 * 1024;
const PROGRESS_STEP: u64 = 256 * 1024;

pub(crate) struct Client {
    agent: ureq::Agent,
}

impl Client {
    fn build(https_only: bool) -> Self {
        let agent = ureq::Agent::config_builder()
            .https_only(https_only)
            .user_agent(concat!("pdf-reader/", env!("CARGO_PKG_VERSION")))
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_global(Some(Duration::from_secs(20 * 60)))
            .build()
            .into();
        Self { agent }
    }

    pub fn new() -> Self {
        Self::build(true)
    }

    #[cfg(test)]
    pub fn plain() -> Self {
        Self::build(false)
    }

    pub fn releases(&self) -> Result<Vec<ApiRelease>> {
        let url = format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=30");
        let text = self
            .agent
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .call()
            .context("Could not reach GitHub to check for updates")?
            .into_body()
            .with_config()
            .limit(RELEASES_LIMIT)
            .read_to_string()
            .context("GitHub's release list could not be read")?;
        serde_json::from_str(&text).context("GitHub's release list was not in the expected format")
    }

    pub fn text(&self, url: &str) -> Result<String> {
        self.agent
            .get(url)
            .call()
            .with_context(|| format!("Could not download {url}"))?
            .into_body()
            .with_config()
            .limit(SUMS_LIMIT)
            .read_to_string()
            .with_context(|| format!("Could not read {url}"))
    }

    pub fn download(
        &self,
        asset: &Asset,
        destination: &Path,
        mut progress: impl FnMut(u64),
    ) -> Result<String> {
        let response = self
            .agent
            .get(&asset.url)
            .call()
            .with_context(|| format!("Could not download {}", asset.name))?;
        let mut body = response.into_body().into_reader().take(asset.size + 1);
        let file = File::create(destination)
            .with_context(|| format!("Could not create {}", destination.display()))?;
        let mut file = BufWriter::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; CHUNK];
        let mut received = 0u64;
        let mut reported = 0u64;
        loop {
            let read = body
                .read(&mut buffer)
                .with_context(|| format!("The download of {} was interrupted", asset.name))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            file.write_all(&buffer[..read])
                .with_context(|| format!("Could not write {}", destination.display()))?;
            received += read as u64;
            if received > asset.size {
                bail!(
                    "{} was larger than the {} bytes GitHub listed",
                    asset.name,
                    asset.size
                );
            }
            if received - reported >= PROGRESS_STEP {
                reported = received;
                progress(received);
            }
        }
        file.flush()
            .with_context(|| format!("Could not write {}", destination.display()))?;
        if received != asset.size {
            bail!(
                "{} ended after {received} of {} bytes",
                asset.name,
                asset.size
            );
        }
        progress(received);
        Ok(hex(&hasher.finalize()))
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::BufRead, net::TcpListener, thread};

    fn serve(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });
        format!("http://{address}/archive.zip")
    }

    fn asset(url: String, size: u64) -> Asset {
        Asset {
            name: "archive.zip".into(),
            size,
            url,
        }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("pdf-reader-fetch-{name}-{}", std::process::id()))
    }

    #[test]
    fn downloads_hash_the_bytes_and_report_progress() {
        let body: Vec<u8> = (0..700_000u32).map(|index| (index % 251) as u8).collect();
        let expected = hex(&Sha256::digest(&body));
        let destination = scratch("ok");
        let mut seen = Vec::new();
        let hash = Client::plain()
            .download(
                &asset(serve(body.clone()), body.len() as u64),
                &destination,
                |received| seen.push(received),
            )
            .unwrap();
        assert_eq!(hash, expected);
        assert_eq!(std::fs::read(&destination).unwrap(), body);
        assert_eq!(seen.last(), Some(&(body.len() as u64)));
        assert!(seen.windows(2).all(|pair| pair[0] < pair[1]));
        let _ = std::fs::remove_file(destination);
    }

    #[test]
    fn downloads_that_do_not_match_the_listed_size_fail() {
        let destination = scratch("size");
        let longer =
            Client::plain().download(&asset(serve(vec![1; 2000]), 1000), &destination, |_| {});
        assert!(longer.unwrap_err().to_string().contains("larger"));
        let shorter =
            Client::plain().download(&asset(serve(vec![1; 500]), 1000), &destination, |_| {});
        assert!(shorter.unwrap_err().to_string().contains("ended after 500"));
        let _ = std::fs::remove_file(destination);
    }

    #[test]
    fn plain_http_is_refused_outside_tests() {
        let destination = scratch("https");
        let refused = Client::new().download(&asset(serve(vec![1; 10]), 10), &destination, |_| {});
        assert!(refused.is_err());
        let _ = std::fs::remove_file(destination);
    }
}
