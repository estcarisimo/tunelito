use std::fs;
use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;
use tar::Archive;

use crate::checksum::{sha256_hex, verify_sha256};
use crate::error::Error;

/// Pinned cloudflared release. Checksums come from the GitHub release notes for this tag.
pub const CLOUDFLARED_VERSION: &str = "2026.10.2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsKind {
    Linux,
    Macos,
    Windows,
}

impl OsKind {
    pub fn current() -> Self {
        match std::env::consts::OS {
            "linux" => Self::Linux,
            "macos" => Self::Macos,
            "windows" => Self::Windows,
            _ => Self::Linux,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
    X86,
    Arm,
}

impl Arch {
    pub fn current() -> Result<Self, Error> {
        match std::env::consts::ARCH {
            "x86_64" => Ok(Self::X86_64),
            "aarch64" => Ok(Self::Aarch64),
            "x86" => Ok(Self::X86),
            "arm" => Ok(Self::Arm),
            other => Err(Error::unsupported_platform(other)),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
            Self::X86 => "x86",
            Self::Arm => "arm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloudflaredAsset {
    pub os: OsKind,
    pub arch: Arch,
    pub filename: &'static str,
    pub sha256_hex: &'static str,
    pub archive: bool,
}

const ASSETS: &[CloudflaredAsset] = &[
    CloudflaredAsset {
        os: OsKind::Linux,
        arch: Arch::X86_64,
        filename: "cloudflared-linux-amd64",
        sha256_hex: "35fbdd2beb378436c7da377d7358cea8e33f9c94a6705d1d8036bf9b0d6d8005",
        archive: false,
    },
    CloudflaredAsset {
        os: OsKind::Linux,
        arch: Arch::Aarch64,
        filename: "cloudflared-linux-arm64",
        sha256_hex: "b7e55b4d9702c2ab5f8348f780bb1db044527fc555367714c78e0595750a4665",
        archive: false,
    },
    CloudflaredAsset {
        os: OsKind::Linux,
        arch: Arch::X86,
        filename: "cloudflared-linux-386",
        sha256_hex: "e11f1a244567ff4d144f13533c6a0a29833f0d93e9a3586099e708f1028affb4",
        archive: false,
    },
    CloudflaredAsset {
        os: OsKind::Linux,
        arch: Arch::Arm,
        filename: "cloudflared-linux-armhf",
        sha256_hex: "592de7e5cf1a0532c938486db88c159547f988559423d6bf3864820aabe21750",
        archive: false,
    },
    CloudflaredAsset {
        os: OsKind::Macos,
        arch: Arch::X86_64,
        filename: "cloudflared-darwin-amd64.tgz",
        sha256_hex: "4be2a4c0b76f6be4a84bea590743e85041949c294efcee4de86a140a9f11869a",
        archive: true,
    },
    CloudflaredAsset {
        os: OsKind::Macos,
        arch: Arch::Aarch64,
        filename: "cloudflared-darwin-arm64.tgz",
        sha256_hex: "823f993ef362fba5e0e288db6f3452d8de34291632971c7a40b79548420e9a18",
        archive: true,
    },
    CloudflaredAsset {
        os: OsKind::Windows,
        arch: Arch::X86_64,
        filename: "cloudflared-windows-amd64.exe",
        sha256_hex: "12c7c887f8aaab779aef1dfa9b60673530a5adb88ecc55dd6df8f56e0b70f199",
        archive: false,
    },
    CloudflaredAsset {
        os: OsKind::Windows,
        arch: Arch::X86,
        filename: "cloudflared-windows-386.exe",
        sha256_hex: "988758588a6fc8706aa26f5b34ee41c5902175f874998a93c1c954d8aabfecba",
        archive: false,
    },
];

pub fn select_asset(os: OsKind, arch: Arch) -> Result<&'static CloudflaredAsset, Error> {
    ASSETS
        .iter()
        .find(|asset| asset.os == os && asset.arch == arch)
        .ok_or_else(|| Error::unsupported_platform(format!("{}-{}", os.as_str(), arch.as_str())))
}

pub fn download_url(asset: &CloudflaredAsset) -> String {
    format!(
        "https://github.com/cloudflare/cloudflared/releases/download/{CLOUDFLARED_VERSION}/{}",
        asset.filename
    )
}

/// Arguments for a quick tunnel. `--no-autoupdate` keeps the checksum we verified.
/// This is not `cloudflared service install` and does not register a boot service.
pub fn tunnel_args() -> Vec<String> {
    vec![
        "tunnel".into(),
        "--no-autoupdate".into(),
        "--url".into(),
        "ssh://localhost:22".into(),
    ]
}

pub fn parse_quick_tunnel_hostname(line: &str) -> Option<String> {
    let marker = ".trycloudflare.com";
    let idx = line.find(marker)?;
    let start = line[..idx]
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .map(|i| i + 1)
        .unwrap_or(0);
    let host = &line[start..idx + marker.len()];
    if host == "trycloudflare.com" || validate_hostname(host).is_err() {
        return None;
    }
    Some(host.to_string())
}

pub fn validate_hostname(host: &str) -> Result<(), Error> {
    if host.len() > 253 || host.len() < 3 || !host.contains('.') {
        return Err(Error::invalid_hostname());
    }
    if host.starts_with('.') || host.ends_with('.') || host.contains("..") {
        return Err(Error::invalid_hostname());
    }
    let ok = host
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.');
    if !ok || host.starts_with('-') || host.ends_with('-') {
        return Err(Error::invalid_hostname());
    }
    Ok(())
}

pub fn file_matches(path: &Path, expected_hex: &str) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    verify_sha256(&bytes, expected_hex).is_ok()
}

/// Write `bytes` to `dest` only after the checksum matches. A bad download never replaces a good file.
pub fn install_verified_bytes(bytes: &[u8], expected_hex: &str, dest: &Path) -> Result<(), Error> {
    verify_sha256(bytes, expected_hex)?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("download");
    fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&tmp, dest)?;
    Ok(())
}

pub fn extract_cloudflared_tgz(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let decoder = GzDecoder::new(bytes);
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|err| Error::download_failed(err.to_string()))?;
    for entry in entries {
        let mut entry = entry.map_err(|err| Error::download_failed(err.to_string()))?;
        let path = entry
            .path()
            .map_err(|err| Error::download_failed(err.to_string()))?;
        if path.file_name().and_then(|name| name.to_str()) == Some("cloudflared") {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|err| Error::download_failed(err.to_string()))?;
            if buf.is_empty() {
                return Err(Error::download_failed(
                    "empty cloudflared binary in archive",
                ));
            }
            return Ok(buf);
        }
    }
    Err(Error::download_failed(
        "archive did not contain cloudflared",
    ))
}

/// Verify the published checksum, unpack a macOS archive if needed, and install the binary.
pub fn install_asset_bytes(
    bytes: &[u8],
    asset: &CloudflaredAsset,
    dest: &Path,
) -> Result<(), Error> {
    verify_sha256(bytes, asset.sha256_hex)?;
    let payload = if asset.archive {
        extract_cloudflared_tgz(bytes)?
    } else {
        bytes.to_vec()
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("download");
    fs::write(&tmp, payload)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&tmp, dest)?;
    Ok(())
}

/// Used by tests that want a stable digest of the pinned table.
pub fn pinned_digest() -> String {
    sha256_hex(CLOUDFLARED_VERSION.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn linux_amd64_asset_is_pinned() {
        let asset = select_asset(OsKind::Linux, Arch::X86_64).unwrap();
        assert_eq!(asset.filename, "cloudflared-linux-amd64");
        assert_eq!(asset.sha256_hex.len(), 64);
        assert!(!asset.archive);
        assert!(download_url(asset).contains("2026.10.2"));
        assert!(download_url(asset).ends_with("cloudflared-linux-amd64"));
    }

    #[test]
    fn windows_arm_is_unsupported() {
        assert_eq!(
            select_asset(OsKind::Windows, Arch::Aarch64)
                .unwrap_err()
                .code,
            "unsupported_platform"
        );
    }

    #[test]
    fn tunnel_args_do_not_install_a_service() {
        let args = tunnel_args();
        let joined = args.join(" ");
        assert!(joined.contains("--no-autoupdate"));
        assert!(joined.contains("ssh://localhost:22"));
        assert!(!joined.contains("service"));
        assert!(!joined.contains("install"));
    }

    #[test]
    fn parses_quick_tunnel_box_and_ignores_the_banner_domain() {
        let banner = "INF Requesting new quick Tunnel on trycloudflare.com...";
        assert_eq!(parse_quick_tunnel_hostname(banner), None);
        let ready = "INF |  https://lime-eagle-4821.trycloudflare.com  |";
        assert_eq!(
            parse_quick_tunnel_hostname(ready).as_deref(),
            Some("lime-eagle-4821.trycloudflare.com")
        );
        assert!(validate_hostname("not a host").is_err());
    }

    #[test]
    fn refuses_to_install_when_the_checksum_is_wrong() {
        let dir = std::env::temp_dir().join(format!("tunelito-sum-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("cloudflared");
        let err = install_verified_bytes(b"not-the-binary", &"ab".repeat(32), &dest).unwrap_err();
        assert_eq!(err.code, "checksum_mismatch");
        assert!(!dest.exists());
        let good = b"cloudflared-bytes";
        let sum = sha256_hex(good);
        install_verified_bytes(good, &sum, &dest).unwrap();
        assert!(dest.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn extracts_cloudflared_from_a_tgz() {
        let dir = std::env::temp_dir().join(format!("tunelito-tgz-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let inner = dir.join("cloudflared");
        {
            let mut file = fs::File::create(&inner).unwrap();
            file.write_all(b"#!/bin/sh\necho cloudflared\n").unwrap();
        }
        let tgz_path = dir.join("c.tgz");
        {
            let file = fs::File::create(&tgz_path).unwrap();
            let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut builder = tar::Builder::new(enc);
            builder
                .append_path_with_name(&inner, "cloudflared")
                .unwrap();
            builder.finish().unwrap();
        }
        let bytes = fs::read(&tgz_path).unwrap();
        let extracted = extract_cloudflared_tgz(&bytes).unwrap();
        assert!(extracted.starts_with(b"#!/bin/sh"));
        let _ = fs::remove_dir_all(&dir);
    }
}
