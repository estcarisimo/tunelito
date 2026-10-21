use std::fmt;

/// Stable codes the UI translates. Details are for the technical disclosure, not the banner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
    pub detail: Option<String>,
}

impl Error {
    pub fn new(code: &'static str) -> Self {
        Self { code, detail: None }
    }

    pub fn with_detail(code: &'static str, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        let detail = if detail.len() > 400 {
            format!("{}…", &detail[..400])
        } else {
            detail
        };
        Self {
            code,
            detail: Some(detail),
        }
    }

    pub fn invalid_public_key() -> Self {
        Self::new("invalid_public_key")
    }

    pub fn private_key_rejected() -> Self {
        Self::new("private_key_rejected")
    }

    pub fn duplicate_key() -> Self {
        Self::new("duplicate_key")
    }

    pub fn key_not_found() -> Self {
        Self::new("key_not_found")
    }

    pub fn key_required() -> Self {
        Self::new("key_required")
    }

    pub fn checksum_mismatch() -> Self {
        Self::new("checksum_mismatch")
    }

    pub fn unsupported_platform(detail: impl Into<String>) -> Self {
        Self::with_detail("unsupported_platform", detail)
    }

    pub fn invalid_hostname() -> Self {
        Self::new("invalid_hostname")
    }

    pub fn invalid_hello() -> Self {
        Self::new("invalid_hello")
    }

    pub fn unsafe_sshd_dropin() -> Self {
        Self::new("unsafe_sshd_dropin")
    }

    pub fn needs_admin() -> Self {
        Self::new("needs_admin")
    }

    pub fn refused_user() -> Self {
        Self::new("refused_user")
    }

    pub fn prepare_failed(detail: impl Into<String>) -> Self {
        Self::with_detail("prepare_failed", detail)
    }

    pub fn download_failed(detail: impl Into<String>) -> Self {
        Self::with_detail("download_failed", detail)
    }

    pub fn io(err: std::io::Error) -> Self {
        Self::with_detail("io", err.to_string())
    }

    pub fn ssh_key(err: impl fmt::Display) -> Self {
        Self::with_detail("invalid_public_key", err.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detail {
            Some(detail) => write!(f, "{}: {detail}", self.code),
            None => write!(f, "{}", self.code),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::io(value)
    }
}

/// Every code the interface knows how to translate. Keep this in sync with the locale files.
pub fn all_error_codes() -> &'static [&'static str] {
    &[
        "invalid_public_key",
        "private_key_rejected",
        "duplicate_key",
        "key_not_found",
        "key_required",
        "checksum_mismatch",
        "unsupported_platform",
        "invalid_hostname",
        "invalid_hello",
        "unsafe_sshd_dropin",
        "needs_admin",
        "refused_user",
        "prepare_failed",
        "download_failed",
        "tunnel_exited",
        "io",
        "generic",
    ]
}
