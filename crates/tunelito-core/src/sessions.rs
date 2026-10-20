use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::hello::{Hello, HELLO_LABEL, VERIFICATION};
use crate::logs::{is_loopback, LogEvent, LogKind, ProcessSession};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub active: bool,
    pub user: String,
    pub fingerprint: Option<String>,
    pub key_type: Option<String>,
    pub observed_peer: String,
    pub port: Option<u16>,
    pub pid: Option<u32>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub hello: Option<Hello>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionView {
    pub id: String,
    pub active: bool,
    pub user: String,
    pub fingerprint: Option<String>,
    pub observed_peer: String,
    pub loopback: bool,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub hello: Option<HelloView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HelloView {
    pub public_ip: Option<String>,
    pub asn: Option<String>,
    pub rdns: Option<String>,
    pub os: String,
    pub os_detail: Option<String>,
    pub reported_at: String,
    pub verification: &'static str,
    pub label: &'static str,
}

impl SessionView {
    pub fn from_session(session: &Session) -> Self {
        Self {
            id: session.id.clone(),
            active: session.active,
            user: session.user.clone(),
            fingerprint: session.fingerprint.clone(),
            observed_peer: session.observed_peer.clone(),
            loopback: is_loopback(&session.observed_peer),
            started_at: session.started_at.map(|t| t.to_rfc3339()),
            ended_at: session.ended_at.map(|t| t.to_rfc3339()),
            hello: session.hello.as_ref().map(HelloView::from_hello),
        }
    }
}

impl HelloView {
    pub fn from_hello(hello: &Hello) -> Self {
        Self {
            public_ip: hello.public_ip.clone(),
            asn: hello.asn.clone(),
            rdns: hello.rdns.clone(),
            os: hello.os.clone(),
            os_detail: hello.os_detail.clone(),
            reported_at: hello.reported_at.to_rfc3339(),
            verification: VERIFICATION,
            label: HELLO_LABEL,
        }
    }
}

struct OpenAccept {
    user: String,
    peer: String,
    port: u16,
    fingerprint: Option<String>,
    key_type: Option<String>,
    started_at: Option<DateTime<Utc>>,
}

pub fn derive_sessions(
    events: &[LogEvent],
    processes: &[ProcessSession],
    now: DateTime<Utc>,
    process_inspection_ok: bool,
) -> Vec<Session> {
    let mut open: Vec<OpenAccept> = Vec::new();
    let mut sessions = Vec::new();
    for event in events {
        match &event.kind {
            LogKind::Accepted {
                user,
                peer,
                port,
                key_type,
                fingerprint,
            } => open.push(OpenAccept {
                user: user.clone(),
                peer: peer.clone(),
                port: *port,
                fingerprint: fingerprint.clone(),
                key_type: key_type.clone(),
                started_at: event.at,
            }),
            LogKind::Disconnected { user, peer, port } => {
                let pos = open.iter().rposition(|item| {
                    item.peer == *peer
                        && item.port == *port
                        && user
                            .as_ref()
                            .is_none_or(|candidate| candidate == &item.user)
                });
                if let Some(pos) = pos {
                    let item = open.remove(pos);
                    sessions.push(finish(item, false, event.at));
                }
            }
        }
    }
    for item in open {
        let mut active = true;
        let mut ended_at = None;
        if process_inspection_ok {
            let seen = processes.iter().any(|proc| proc.user == item.user);
            let recent = item
                .started_at
                .map(|start| now.signed_duration_since(start).num_seconds() < 20)
                .unwrap_or(false);
            if !seen && !recent {
                active = false;
                ended_at = Some(now);
            }
        }
        sessions.push(finish(item, active, ended_at));
    }
    if process_inspection_ok {
        for proc in processes {
            let already = sessions
                .iter()
                .any(|session| session.active && session.user == proc.user);
            if already {
                continue;
            }
            sessions.push(Session {
                id: format!("proc:{}:{}", proc.pid, proc.user),
                active: true,
                user: proc.user.clone(),
                fingerprint: None,
                key_type: None,
                observed_peer: "unknown".into(),
                port: None,
                pid: Some(proc.pid),
                started_at: None,
                ended_at: None,
                hello: None,
            });
        }
    }
    sessions.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then(b.started_at.cmp(&a.started_at))
    });
    sessions
}

fn finish(item: OpenAccept, active: bool, ended_at: Option<DateTime<Utc>>) -> Session {
    let start_id = item
        .started_at
        .map(|t| t.timestamp().to_string())
        .unwrap_or_else(|| "na".into());
    Session {
        id: format!("log:{}:{}:{start_id}", item.peer, item.port),
        active,
        user: item.user,
        fingerprint: item.fingerprint,
        key_type: item.key_type,
        observed_peer: item.peer,
        port: Some(item.port),
        pid: None,
        started_at: item.started_at,
        ended_at: if active { None } else { ended_at },
        hello: None,
    }
}

pub fn attach_hellos(sessions: &mut [Session], hellos: &[Hello]) -> Vec<Hello> {
    let mut unmatched = Vec::new();
    for hello in hellos {
        let mut best: Option<(usize, i64)> = None;
        for (index, session) in sessions.iter().enumerate() {
            if session.fingerprint.as_deref() != Some(hello.key_fingerprint.as_str()) {
                continue;
            }
            let mut score = session
                .started_at
                .map(|start| (start - hello.reported_at).num_seconds().abs())
                .unwrap_or(10_000);
            if !session.active {
                score += 1_000_000;
            }
            if best.map(|(_, current)| score < current).unwrap_or(true) {
                best = Some((index, score));
            }
        }
        if let Some((index, _)) = best {
            let replace = sessions[index]
                .hello
                .as_ref()
                .map(|existing| existing.reported_at <= hello.reported_at)
                .unwrap_or(true);
            if replace {
                sessions[index].hello = Some(hello.clone());
            }
        } else {
            unmatched.push(hello.clone());
        }
    }
    unmatched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logs::{parse_log, parse_processes};

    #[test]
    fn pairs_accept_with_disconnect_and_labels_hello_unverified() {
        let log = "\
2026-10-20T22:01:02+0000 h sshd[1]: Accepted publickey for tunelito from 127.0.0.1 port 53512 ssh2: ED25519 SHA256:abc
2026-10-20T22:20:02+0000 h sshd[1]: Accepted publickey for tunelito from 127.0.0.1 port 54000 ssh2: ED25519 SHA256:abc
2026-10-20T22:40:02+0000 h sshd[1]: Disconnected from user tunelito 127.0.0.1 port 53512
";
        let events = parse_log(log);
        let procs = parse_processes("tunelito 9 sshd: tunelito@notty\n");
        let now = DateTime::parse_from_rfc3339("2026-10-20T22:41:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut sessions = derive_sessions(&events, &procs, now, true);
        assert!(sessions.iter().any(|s| s.active && s.port == Some(54000)));
        assert!(sessions.iter().any(|s| !s.active && s.port == Some(53512)));
        let hello = Hello::new(
            Some("203.0.113.9".into()),
            Some("AS64500".into()),
            Some("helper.example".into()),
            "linux",
            None,
            "SHA256:abc",
            now,
        );
        let unmatched = attach_hellos(&mut sessions, &[hello]);
        assert!(unmatched.is_empty());
        let view = SessionView::from_session(sessions.iter().find(|s| s.active).unwrap());
        assert!(view.loopback);
        let reported = view.hello.expect("hello attached");
        assert_eq!(reported.verification, "unverified");
        assert_eq!(reported.label, "reported by client (unverified)");
        assert_eq!(reported.public_ip.as_deref(), Some("203.0.113.9"));
    }

    #[test]
    fn process_inspection_closes_a_session_the_log_left_open() {
        let log =
            "Accepted publickey for tunelito from 127.0.0.1 port 9 ssh2: ED25519 SHA256:abc\n";
        let events = parse_log(log);
        let now = Utc::now();
        let sessions = derive_sessions(&events, &[], now, true);
        assert_eq!(sessions.len(), 1);
        assert!(!sessions[0].active);
    }
}
