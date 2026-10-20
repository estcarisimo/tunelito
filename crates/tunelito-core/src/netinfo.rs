use std::fs;
use std::net::IpAddr;
use std::process::Command;

use serde_json::Value;

/// `ip=203.0.113.8` line from `https://cloudflare.com/cdn-cgi/trace`.
pub fn parse_cloudflare_trace(body: &str) -> Option<String> {
    for line in body.lines() {
        let Some(ip) = line.trim().strip_prefix("ip=") else {
            continue;
        };
        if ip.parse::<IpAddr>().is_ok() {
            return Some(ip.to_string());
        }
    }
    None
}

/// Team Cymru TXT: `"15169 | 203.0.113.0/24 | US | arin | 2000-03-30"`.
pub fn parse_cymru_txt(body: &str) -> Option<String> {
    let cleaned = body.trim().trim_matches('"').trim();
    let asn = cleaned.split('|').next()?.trim();
    let asn = asn.trim_matches('"').trim();
    if asn.is_empty() || !asn.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("AS{asn}"))
}

/// RIPE Stat `network-info` or `prefix-overview` JSON. Accepts numeric or `AS` prefixed values.
pub fn parse_ripe_asn(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let asns = value.pointer("/data/asns")?;
    let first = asns.as_array()?.first()?;
    if let Some(text) = first.as_str() {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        if text.to_ascii_uppercase().starts_with("AS") {
            return Some(text.to_ascii_uppercase());
        }
        if text.chars().all(|c| c.is_ascii_digit()) {
            return Some(format!("AS{text}"));
        }
    }
    if let Some(number) = first.as_u64() {
        return Some(format!("AS{number}"));
    }
    None
}

pub fn parse_os_release(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("PRETTY_NAME=") else {
            continue;
        };
        let name = rest.trim().trim_matches('"').trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

pub fn local_os() -> (String, Option<String>) {
    let os = std::env::consts::OS.to_string();
    let detail = fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|text| parse_os_release(&text))
        .or_else(uname_sr);
    (os, detail)
}

fn uname_sr() -> Option<String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let output = Command::new("uname").args(["-sr"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        let text = text.trim();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsers_accept_the_documented_payloads() {
        assert_eq!(
            parse_cloudflare_trace("fl=1\nip=203.0.113.8\nts=1\n").as_deref(),
            Some("203.0.113.8")
        );
        assert_eq!(
            parse_cymru_txt("\"64500 | 203.0.113.0/24 | US | arin |\"").as_deref(),
            Some("AS64500")
        );
        assert_eq!(
            parse_ripe_asn(r#"{"data":{"asns":["AS64500"]}}"#).as_deref(),
            Some("AS64500")
        );
        assert_eq!(
            parse_ripe_asn(r#"{"data":{"asns":[64500]}}"#).as_deref(),
            Some("AS64500")
        );
        assert_eq!(
            parse_os_release("NAME=Ubuntu\nPRETTY_NAME=\"Ubuntu 24.04.5 LTS\"\n").as_deref(),
            Some("Ubuntu 24.04.5 LTS")
        );
    }

    #[test]
    fn this_ci_host_reports_linux() {
        let (os, detail) = local_os();
        assert_eq!(os, "linux");
        assert!(detail.is_some(), "expected a linux os detail");
    }
}
