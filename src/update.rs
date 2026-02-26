//! Self-update logic: check for new releases and replace the running binary.

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

// -----------------------------------------
// src/update.rs
//
// const LATEST_URL                      L18
// const RELEASE_BASE                    L19
// pub struct Updater                    L21
//   pub fn new()                        L32
//   pub fn run()                        L43
//   fn check_latest()                   L62
//   fn compare()                        L76
//   fn download()                       L90
//   fn verify()                        L113
//   fn replace()                       L141
// -----------------------------------------

const LATEST_URL: &str = "https://kdb.kernl.sh/latest";
const RELEASE_BASE: &str = "https://github.com/dremnik/kdb/releases/download";

/// Handles checking for updates and replacing the current binary.
pub struct Updater {
    /// Compile-time package version (without `v` prefix).
    current: &'static str,
    /// Compile-time target triple (e.g. `aarch64-apple-darwin`).
    target: &'static str,
}

impl Updater {
    /// Create an updater with compile-time version and target triple.
    pub fn new() -> Self {
        Self {
            current: env!("CARGO_PKG_VERSION"),
            target: env!("TARGET"),
        }
    }

    /// Run the update flow. When `check_only` is true, print version info
    /// without downloading or replacing.
    pub fn run(&self, check_only: bool) -> Result<()> {
        let latest = self.check_latest()?;
        let tag = format!("v{latest}");

        if self.compare(&latest) {
            println!("kdb {}: up to date", self.current);
            return Ok(());
        }

        println!("update available: v{} -> {tag}", self.current);

        if check_only {
            return Ok(());
        }

        let (archive, checksums) = self.download(&tag)?;
        self.verify(&archive, &checksums)?;
        self.replace(&archive)?;

        println!("updated kdb: v{} -> {tag}", self.current);
        Ok(())
    }

    /// Fetch the latest release version string from the endpoint.
    fn check_latest(&self) -> Result<String> {
        let body: String = ureq::get(LATEST_URL)
            .call()
            .context("failed to fetch latest version")?
            .body_mut()
            .read_to_string()
            .context("failed to read latest version response")?;

        let version = body.trim().trim_start_matches('v').to_string();
        assert!(!version.is_empty(), "empty version from {LATEST_URL}");
        Ok(version)
    }

    /// Return `true` if the current version is up to date (latest <= current).
    fn compare(&self, latest: &str) -> bool {
        let parse = |v: &str| -> Vec<u64> {
            v.split('.')
                .filter_map(|part| part.parse().ok())
                .collect()
        };

        let current_parts = parse(self.current);
        let latest_parts = parse(latest);
        assert!(
            !current_parts.is_empty() && !latest_parts.is_empty(),
            "invalid semver: current={}, latest={latest}",
            self.current
        );

        latest_parts <= current_parts
    }

    /// Download the release tarball and checksums file for the given tag.
    fn download(&self, tag: &str) -> Result<(Vec<u8>, String)> {
        let archive_name = format!("kdb-{}.tar.gz", self.target);
        let archive_url = format!("{RELEASE_BASE}/{tag}/{archive_name}");
        let checksums_url = format!("{RELEASE_BASE}/{tag}/checksums.txt");

        println!("downloading {archive_name}...");

        let mut archive = Vec::new();
        ureq::get(&archive_url)
            .call()
            .with_context(|| format!("failed to download {archive_url}"))?
            .body_mut()
            .as_reader()
            .read_to_end(&mut archive)
            .context("failed to read archive bytes")?;

        assert!(!archive.is_empty(), "downloaded archive is empty");

        let checksums: String = ureq::get(&checksums_url)
            .call()
            .with_context(|| format!("failed to download {checksums_url}"))?
            .body_mut()
            .read_to_string()
            .context("failed to read checksums")?;

        Ok((archive, checksums))
    }

    /// Verify the archive's SHA-256 checksum against the checksums manifest.
    fn verify(&self, archive: &[u8], checksums: &str) -> Result<()> {
        let archive_name = format!("kdb-{}.tar.gz", self.target);

        let expected = checksums
            .lines()
            .find_map(|line| {
                let mut parts = line.split_whitespace();
                let hash = parts.next()?;
                let name = parts.next()?;
                if name == archive_name {
                    Some(hash.to_string())
                } else {
                    None
                }
            })
            .with_context(|| format!("no checksum found for {archive_name} in checksums.txt"))?;

        let mut hasher = Sha256::new();
        hasher.update(archive);
        let actual = format!("{:x}", hasher.finalize());

        if actual != expected {
            bail!(
                "checksum mismatch for {archive_name}:\n  expected: {expected}\n  actual:   {actual}"
            );
        }

        Ok(())
    }

    /// Extract the binary from the tarball and atomically replace the current exe.
    fn replace(&self, archive: &[u8]) -> Result<()> {
        let current_exe =
            std::env::current_exe().context("failed to determine current executable path")?;
        let exe_dir = current_exe
            .parent()
            .context("current executable has no parent directory")?;

        let decoder = flate2::read::GzDecoder::new(io::Cursor::new(archive));
        let mut tar = tar::Archive::new(decoder);

        let binary = self.find_binary_in_tar(&mut tar)?;

        let tmp = tempfile::NamedTempFile::new_in(exe_dir)
            .context("failed to create tempfile for update")?;
        let tmp_path = tmp.path().to_path_buf();
        fs::write(&tmp_path, &binary).context("failed to write updated binary to tempfile")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755))
                .context("failed to set executable permissions")?;
        }

        fs::rename(&tmp_path, &current_exe).context("failed to replace current binary")?;
        // Persist so the NamedTempFile destructor doesn't try to remove the path
        // (we already renamed it away).
        tmp.into_temp_path().keep()?;

        Ok(())
    }

    /// Find and read the `kdb` binary from a tar archive.
    fn find_binary_in_tar<R: Read>(&self, tar: &mut tar::Archive<R>) -> Result<Vec<u8>> {
        for entry in tar.entries().context("failed to read tar entries")? {
            let mut entry = entry.context("failed to read tar entry")?;
            let path = entry
                .path()
                .context("failed to read tar entry path")?
                .into_owned();

            let is_kdb = path
                .file_name()
                .is_some_and(|name| name == "kdb");

            if is_kdb {
                let mut buf = Vec::new();
                entry
                    .read_to_end(&mut buf)
                    .context("failed to read binary from archive")?;
                assert!(!buf.is_empty(), "extracted binary is empty");
                return Ok(buf);
            }
        }

        bail!("kdb binary not found in archive")
    }
}

/// Return the path to the current executable.
pub fn exe_path() -> Result<PathBuf> {
    std::env::current_exe().context("failed to determine current executable path")
}
