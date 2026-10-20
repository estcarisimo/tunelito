use chrono::{DateTime, Datelike, TimeZone, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEvent {
    pub at: Option<DateTime<Utc>>,
    pub kind: LogKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogKind {
    Accepted {
        user: String,
        peer: String,
        port: u16,
        key_type: Option<String>,
        fingerprint: Option<String>,
    },
    Disconnected {
        user: Option<String>,
        peer: String,
        port: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSession {
    pub user: String,
    pub pid: u32,
    pub destination: String,
}

pub fn parse_log(text: &str) -> Vec<LogEvent> {
    let mut events = Vec::new();
    for line in text.lines() {
        if let Some(event) = parse_log_line(line) {
            events.push(event);
        }
    }
    events
}

pub fn parse_log_line(line: &str) -> Option<LogEvent> {
    let at = parse_datetime_prefix(line);
    if let Some(idx) = line.find("Accepted publickey for ") {
        let rest = &line[idx + "Accepted publickey for ".len()..];
        let mut parts = rest.split_whitespace();
        let user = parts.next()?.to_string();
        if parts.next() != Some("from") {
            return None;
        }
        let peer = parts.next()?.to_string();
        if parts.next() != Some("port") {
            return None;
        }
        let port: u16 = parts.next()?.parse().ok()?;
        let mut key_type = None;
        let mut fingerprint = None;
        let tokens: Vec<&str> = parts.collect();
        for window in tokens.windows(2) {
            if window[1].starts_with("SHA256:") {
                key_type = Some(window[0].trim_end_matches(':').to_string());
                fingerprint = Some(window[1].trim_end_matches([',', ';']).to_string());
            }
        }
        return Some(LogEvent {
            at,
            kind: LogKind::Accepted {
                user,
                peer,
                port,
                key_type,
                fingerprint,
            },
        });
    }
    if let Some(idx) = line.find("Disconnected from user ") {
        let rest = &line[idx + "Disconnected from user ".len()..];
        let mut parts = rest.split_whitespace();
        let user = parts.next()?.to_string();
        let peer = parts.next()?.to_string();
        if parts.next() != Some("port") {
            return None;
        }
        let port = parts.next()?.trim_end_matches(':').parse().ok()?;
        return Some(LogEvent {
            at,
            kind: LogKind::Disconnected {
                user: Some(user),
                peer,
                port,
            },
        });
    }
    if let Some(idx) = line.find("Disconnected from ") {
        let rest = &line[idx + "Disconnected from ".len()..];
        if rest.starts_with("user ") {
            return None;
        }
        let mut parts = rest.split_whitespace();
        let peer = parts.next()?.to_string();
        if parts.next() != Some("port") {
            return None;
        }
        let port = parts.next()?.trim_end_matches(':').parse().ok()?;
        return Some(LogEvent {
            at,
            kind: LogKind::Disconnected {
                user: None,
                peer,
                port,
            },
        });
    }
    if let Some(idx) = line.find("Received disconnect from ") {
        let rest = &line[idx + "Received disconnect from ".len()..];
        let mut parts = rest.split_whitespace();
        let peer = parts.next()?.to_string();
        if parts.next() != Some("port") {
            return None;
        }
        let port_token = parts.next()?;
        let port = port_token.trim_end_matches(':').parse().ok()?;
        return Some(LogEvent {
            at,
            kind: LogKind::Disconnected {
                user: None,
                peer,
                port,
            },
        });
    }
    None
}

pub fn parse_processes(text: &str) -> Vec<ProcessSession> {
    let mut sessions = Vec::new();
    for line in text.lines() {
        if let Some(session) = parse_process_line(line) {
            sessions.push(session);
        }
    }
    sessions
}

pub fn parse_process_line(line: &str) -> Option<ProcessSession> {
    let marker = "sshd:";
    let idx = line.find(marker)?;
    let after = line[idx + marker.len()..].trim();
    let (user, destination) = if let Some((user, dest)) = after.split_once('@') {
        let user = user.trim();
        if user.is_empty() || user.starts_with('[') {
            return None;
        }
        (
            user.to_string(),
            dest.split_whitespace().next()?.to_string(),
        )
    } else {
        return None;
    };
    let before = line[..idx].trim();
    let mut pid = 0;
    for token in before.split_whitespace() {
        if let Ok(value) = token.parse::<u32>() {
            pid = value;
        }
    }
    Some(ProcessSession {
        user,
        pid,
        destination,
    })
}

/// `sshd -T` style and OpenSSH text logs are shared. Windows `wevtutil` text is the same
/// once the message is on its own line; execution of `wevtutil` is not run in CI.
pub fn windows_event_log_command() -> Vec<String> {
    vec![
        "wevtutil".into(),
        "qe".into(),
        "OpenSSH/Operational".into(),
        "/q:*[System[TimeCreated[timediff(@SystemTime) <= 86400000]]]".into(),
        "/f:text".into(),
        "/c:400".into(),
    ]
}

pub fn linux_journal_args() -> Vec<String> {
    vec![
        "-u".into(),
        "ssh".into(),
        "-u".into(),
        "sshd".into(),
        "--since".into(),
        "24 hours ago".into(),
        "-o".into(),
        "short-iso".into(),
        "--no-pager".into(),
    ]
}

pub fn process_list_args() -> Vec<String> {
    // macOS and Linux `ps` accept this form. Windows uses a different command.
    // UNTESTED ON CI: macOS execution. The parser is covered with fixtures.
    vec!["-ax".into(), "-o".into(), "user=,pid=,command=".into()]
}

fn parse_datetime_prefix(line: &str) -> Option<DateTime<Utc>> {
    let trimmed = line.trim_start();
    if let Some(dt) = parse_iso_prefix(trimmed) {
        return Some(dt);
    }
    let mut parts = trimmed.split_whitespace();
    let first = parts.next()?;
    if first.len() == 10 && first.as_bytes().get(4) == Some(&b'-') {
        if let Some(second) = parts.next() {
            let joined = format!("{first}T{second}");
            if let Some(dt) = parse_iso_prefix(&joined) {
                return Some(dt);
            }
        }
    }
    parse_syslog_prefix(trimmed)
}

fn parse_iso_prefix(line: &str) -> Option<DateTime<Utc>> {
    let token = line.split_whitespace().next()?.trim_end_matches(':');
    if token.len() < 19 || token.as_bytes().get(4) != Some(&b'-') {
        return None;
    }
    let (body, zone) = split_zone(token);
    let body = body.split('.').next()?;
    let naive = chrono::NaiveDateTime::parse_from_str(body, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(body, "%Y-%m-%d %H:%M:%S"))
        .ok()?;
    let offset = parse_offset(zone)?;
    Some(DateTime::<Utc>::from_naive_utc_and_offset(
        naive - offset,
        Utc,
    ))
}

fn split_zone(token: &str) -> (&str, &str) {
    if let Some(idx) = token.rfind(['+', '-', 'Z']) {
        if idx > 10 {
            return (&token[..idx], &token[idx..]);
        }
    }
    (token, "Z")
}

fn parse_offset(zone: &str) -> Option<chrono::Duration> {
    if zone == "Z" || zone.is_empty() {
        return Some(chrono::Duration::zero());
    }
    let sign = if zone.starts_with('+') {
        1
    } else if zone.starts_with('-') {
        -1
    } else {
        return None;
    };
    let digits: String = zone[1..].chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return None;
    }
    let hours: i64 = digits[..2].parse().ok()?;
    let mins: i64 = digits[2..4].parse().ok()?;
    Some(chrono::Duration::minutes(sign * (hours * 60 + mins)))
}

fn parse_syslog_prefix(line: &str) -> Option<DateTime<Utc>> {
    // Naive syslog timestamps are interpreted as UTC. The host prefers `journalctl -o short-iso`,
    // which carries an offset. UNTESTED ON CI: macOS unified-log prefixes beyond the fixture below.
    let mut parts = line.split_whitespace();
    let month = month_number(parts.next()?)?;
    let day: u32 = parts.next()?.parse().ok()?;
    let time = parts.next()?;
    let mut clock = time.split(':');
    let hour: u32 = clock.next()?.parse().ok()?;
    let min: u32 = clock.next()?.parse().ok()?;
    let sec: u32 = clock.next()?.parse().ok()?;
    let now = Utc::now();
    let mut year = now.year();
    let candidate = Utc
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .single()?;
    if candidate > now + chrono::Duration::days(2) {
        year -= 1;
    }
    Utc.with_ymd_and_hms(year, month, day, hour, min, sec)
        .single()
}

fn month_number(name: &str) -> Option<u32> {
    Some(match name {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    })
}

pub fn is_loopback(peer: &str) -> bool {
    peer == "127.0.0.1" || peer == "::1" || peer.starts_with("127.")
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINUX: &str = "\
2026-10-20T22:01:02+0000 host sshd[1234]: Accepted publickey for tunelito from 127.0.0.1 port 53512 ssh2: ED25519 SHA256:abc+def
2026-10-20T22:15:02+00:00 host sshd[1234]: Disconnected from user tunelito 127.0.0.1 port 53512
";

    #[test]
    fn parses_linux_iso_accept_and_disconnect() {
        let events = parse_log(LINUX);
        assert_eq!(events.len(), 2);
        match &events[0].kind {
            LogKind::Accepted {
                user,
                peer,
                port,
                fingerprint,
                ..
            } => {
                assert_eq!(user, "tunelito");
                assert_eq!(peer, "127.0.0.1");
                assert_eq!(*port, 53512);
                assert_eq!(fingerprint.as_deref(), Some("SHA256:abc+def"));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(events[0].at.is_some());
        assert!(is_loopback("127.0.0.1"));
        assert!(is_loopback("::1"));
    }

    #[test]
    fn parses_macos_style_and_windows_event_text() {
        // UNTESTED ON CI: these strings are representative fixtures, not captured on those OSes.
        let macos = "2026-10-20 22:01:02.123456-0500 localhost sshd[9]: Accepted publickey for tunelito from 127.0.0.1 port 22 ssh2: ED25519 SHA256:mac";
        let event = parse_log_line(macos).unwrap();
        match event.kind {
            LogKind::Accepted { fingerprint, .. } => {
                assert_eq!(fingerprint.as_deref(), Some("SHA256:mac"))
            }
            other => panic!("{other:?}"),
        }
        let windows = "\
Event[0]
  Date: 2026-10-20T22:01:02.000Z
  Description:
  Accepted publickey for tunelito from 127.0.0.1 port 53512 ssh2: ED25519 SHA256:win
";
        let events = parse_log(windows);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn parses_sshd_process_lines_and_ignores_the_priv_parent() {
        let text = "\
root      2190  sshd: tunelito [priv]
tunelito  2201  sshd: tunelito@pts/0
tunelito  2202  sshd: tunelito@notty
";
        let sessions = parse_processes(text);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].destination, "pts/0");
        assert_eq!(sessions[0].pid, 2201);
        assert_eq!(sessions[1].destination, "notty");
    }

    #[test]
    fn live_ps_output_parses_without_error() {
        let output = std::process::Command::new("ps")
            .args(process_list_args())
            .output()
            .expect("ps");
        assert!(output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        let _ = parse_processes(&text);
    }
}
