use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::cloudflared::{parse_quick_tunnel_hostname, validate_hostname};
use crate::error::Error;
use crate::hello::Hello;
use crate::keys::{parse_public_keys, TrustedKey, DEDICATED_USER};
use crate::sessions::{attach_hellos, derive_sessions, Session, SessionView};
use crate::ssh_config::{render_host_block, SshHostEntry};

pub const DEFAULT_AUTO_OFF_SECS: u64 = 2 * 60 * 60;
pub const MIN_AUTO_OFF_SECS: u64 = 15 * 60;
pub const MAX_AUTO_OFF_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub auto_off_secs: u64,
    pub revoke_keys_on_off: bool,
    pub language: String,
    pub stop_sshd_if_started_by_us: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_off_secs: DEFAULT_AUTO_OFF_SECS,
            revoke_keys_on_off: false,
            language: "en".into(),
            stop_sshd_if_started_by_us: true,
        }
    }
}

impl Settings {
    pub fn normalized(mut self) -> Self {
        self.auto_off_secs = self
            .auto_off_secs
            .clamp(MIN_AUTO_OFF_SECS, MAX_AUTO_OFF_SECS);
        if self.language != "es" {
            self.language = "en".into();
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Phase {
    Off,
    Starting {
        started_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    },
    On {
        hostname: String,
        started_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Install or start sshd, create the dedicated user, write drop-in, keys, and the hello dir.
    PrepareAccess,
    /// Rewrite authorized_keys from the in-memory list (empty while the switch is off).
    SyncAuthorizedKeys,
    StartTunnel,
    StopTunnel,
    StopSshd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyView {
    pub fingerprint: String,
    pub comment: String,
    pub key_type: String,
    pub added_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ViewModel {
    pub phase: String,
    pub hostname: Option<String>,
    pub ssh_snippet: Option<String>,
    pub started_at: Option<String>,
    pub expires_at: Option<String>,
    pub seconds_remaining: Option<i64>,
    pub someone_connected: bool,
    pub active_session_count: usize,
    pub keys: Vec<KeyView>,
    pub sessions: Vec<SessionView>,
    pub unmatched_hellos: Vec<crate::sessions::HelloView>,
    pub settings: Settings,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub activity: Option<String>,
    pub dedicated_user: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub settings: Settings,
    pub keys: Vec<TrustedKey>,
    pub phase: Phase,
    pub sessions: Vec<Session>,
    pub unmatched_hellos: Vec<Hello>,
    pub sshd_started_by_us: bool,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub activity: Option<String>,
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Host {
    pub fn new() -> Self {
        Self {
            settings: Settings::default(),
            keys: Vec::new(),
            phase: Phase::Off,
            sessions: Vec::new(),
            unmatched_hellos: Vec::new(),
            sshd_started_by_us: false,
            error_code: None,
            error_detail: None,
            activity: None,
        }
    }

    pub fn add_pasted_keys(
        &mut self,
        text: &str,
        now: DateTime<Utc>,
    ) -> Result<Vec<Effect>, Error> {
        let parsed = parse_public_keys(text)?;
        if parsed.is_empty() {
            return Err(Error::invalid_public_key());
        }
        let mut added = 0;
        for key in parsed {
            if self
                .keys
                .iter()
                .any(|existing| existing.fingerprint == key.fingerprint)
            {
                continue;
            }
            self.keys.push(TrustedKey {
                public_key: key.public_key,
                fingerprint: key.fingerprint,
                comment: key.comment,
                key_type: key.key_type,
                added_at: now,
            });
            added += 1;
        }
        if added == 0 {
            return Err(Error::duplicate_key());
        }
        self.error_code = None;
        self.error_detail = None;
        if self.is_engaged() {
            Ok(vec![Effect::SyncAuthorizedKeys])
        } else {
            Ok(vec![])
        }
    }

    pub fn remove_key(&mut self, fingerprint: &str) -> Result<Vec<Effect>, Error> {
        let before = self.keys.len();
        self.keys.retain(|key| key.fingerprint != fingerprint);
        if self.keys.len() == before {
            return Err(Error::key_not_found());
        }
        if self.is_engaged() {
            Ok(vec![Effect::SyncAuthorizedKeys])
        } else {
            Ok(vec![])
        }
    }

    pub fn turn_on(&mut self, now: DateTime<Utc>) -> Result<Vec<Effect>, Error> {
        if self.keys.is_empty() {
            return Err(Error::key_required());
        }
        if self.is_engaged() {
            return Ok(vec![]);
        }
        let expires_at = now + Duration::seconds(self.settings.auto_off_secs as i64);
        self.phase = Phase::Starting {
            started_at: now,
            expires_at,
        };
        self.error_code = None;
        self.error_detail = None;
        self.activity = Some("starting_sshd".into());
        Ok(vec![Effect::PrepareAccess, Effect::StartTunnel])
    }

    pub fn turn_off(&mut self, now: DateTime<Utc>, revoke_keys: bool) -> Vec<Effect> {
        self.phase = Phase::Off;
        self.activity = None;
        let mut effects = vec![Effect::StopTunnel, Effect::SyncAuthorizedKeys];
        if self.sshd_started_by_us && self.settings.stop_sshd_if_started_by_us {
            effects.push(Effect::StopSshd);
            self.sshd_started_by_us = false;
        }
        if revoke_keys {
            self.keys.clear();
        }
        for session in &mut self.sessions {
            if session.active {
                session.active = false;
                session.ended_at.get_or_insert(now);
            }
        }
        effects
    }

    pub fn poll(&mut self, now: DateTime<Utc>) -> Vec<Effect> {
        let expires_at = match &self.phase {
            Phase::On { expires_at, .. } | Phase::Starting { expires_at, .. } => *expires_at,
            Phase::Off => return vec![],
        };
        if now >= expires_at {
            let revoke = self.settings.revoke_keys_on_off;
            return self.turn_off(now, revoke);
        }
        vec![]
    }

    pub fn update_settings(&mut self, settings: Settings, now: DateTime<Utc>) -> Vec<Effect> {
        self.settings = settings.normalized();
        let (started_at, hostname) = match &self.phase {
            Phase::On {
                started_at,
                hostname,
                ..
            } => (*started_at, Some(hostname.clone())),
            Phase::Starting { started_at, .. } => (*started_at, None),
            Phase::Off => return vec![],
        };
        let expires_at = started_at + Duration::seconds(self.settings.auto_off_secs as i64);
        self.phase = match hostname {
            Some(hostname) => Phase::On {
                hostname,
                started_at,
                expires_at,
            },
            None => Phase::Starting {
                started_at,
                expires_at,
            },
        };
        if now >= expires_at {
            let revoke = self.settings.revoke_keys_on_off;
            return self.turn_off(now, revoke);
        }
        vec![]
    }

    pub fn ingest_tunnel_line(&mut self, line: &str) -> Option<String> {
        let host = parse_quick_tunnel_hostname(line)?;
        if validate_hostname(&host).is_err() {
            return None;
        }
        let (started_at, expires_at) = match &self.phase {
            Phase::Starting {
                started_at,
                expires_at,
            }
            | Phase::On {
                started_at,
                expires_at,
                ..
            } => (*started_at, *expires_at),
            Phase::Off => return None,
        };
        self.phase = Phase::On {
            hostname: host.clone(),
            started_at,
            expires_at,
        };
        self.activity = Some("ready".into());
        Some(host)
    }

    pub fn note_tunnel_exit(&mut self, intentional: bool) {
        if intentional {
            return;
        }
        if !matches!(self.phase, Phase::Off) {
            self.phase = Phase::Off;
            self.activity = None;
            self.error_code = Some("tunnel_exited".into());
        }
    }

    pub fn fail(&mut self, error: &Error) {
        self.phase = Phase::Off;
        self.activity = None;
        self.error_code = Some(error.code.to_string());
        self.error_detail = error.detail.clone();
    }

    pub fn set_activity(&mut self, activity: Option<&str>) {
        self.activity = activity.map(str::to_string);
    }

    pub fn mark_sshd_started_by_us(&mut self) {
        self.sshd_started_by_us = true;
    }

    pub fn refresh_observations(
        &mut self,
        log: &str,
        processes: &str,
        process_ok: bool,
        hellos: &[Hello],
        now: DateTime<Utc>,
    ) {
        let events = crate::logs::parse_log(log);
        let procs = crate::logs::parse_processes(processes);
        let mut sessions = derive_sessions(&events, &procs, now, process_ok);
        // Keep hellos already attached when a refresh re-derives the same session id.
        for session in &mut sessions {
            if let Some(previous) = self.sessions.iter().find(|old| old.id == session.id) {
                if session.hello.is_none() {
                    session.hello = previous.hello.clone();
                }
            }
        }
        let unmatched = attach_hellos(&mut sessions, hellos);
        if sessions.len() > 100 {
            sessions.truncate(100);
        }
        self.sessions = sessions;
        self.unmatched_hellos = unmatched;
    }

    pub fn key_lines(&self) -> Vec<String> {
        self.keys.iter().map(|key| key.public_key.clone()).collect()
    }

    pub fn is_engaged(&self) -> bool {
        !matches!(self.phase, Phase::Off)
    }

    pub fn view(&self, now: DateTime<Utc>) -> ViewModel {
        let (phase, hostname, started_at, expires_at) = match &self.phase {
            Phase::Off => ("off", None, None, None),
            Phase::Starting {
                started_at,
                expires_at,
            } => ("starting", None, Some(*started_at), Some(*expires_at)),
            Phase::On {
                hostname,
                started_at,
                expires_at,
            } => (
                "on",
                Some(hostname.clone()),
                Some(*started_at),
                Some(*expires_at),
            ),
        };
        let seconds_remaining = expires_at.map(|exp| (exp - now).num_seconds().max(0));
        let ssh_snippet = hostname
            .as_ref()
            .and_then(|host| render_host_block(&SshHostEntry::for_host(host.clone())).ok());
        let active_session_count = self.sessions.iter().filter(|s| s.active).count();
        ViewModel {
            phase: phase.into(),
            hostname,
            ssh_snippet,
            started_at: started_at.map(|t| t.to_rfc3339()),
            expires_at: expires_at.map(|t| t.to_rfc3339()),
            seconds_remaining,
            someone_connected: active_session_count > 0,
            active_session_count,
            keys: self
                .keys
                .iter()
                .map(|key| KeyView {
                    fingerprint: key.fingerprint.clone(),
                    comment: key.comment.clone(),
                    key_type: key.key_type.clone(),
                    added_at: key.added_at.to_rfc3339(),
                })
                .collect(),
            sessions: self
                .sessions
                .iter()
                .map(SessionView::from_session)
                .collect(),
            unmatched_hellos: self
                .unmatched_hellos
                .iter()
                .map(crate::sessions::HelloView::from_hello)
                .collect(),
            settings: self.settings.clone(),
            error_code: self.error_code.clone(),
            error_detail: self.error_detail.clone(),
            activity: self.activity.clone(),
            dedicated_user: DEDICATED_USER.into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-20T20:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn host_with_key() -> Host {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let mut host = Host::new();
        let dir = std::env::temp_dir().join(format!(
            "tunelito-host-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (parsed, _) = crate::keys::ensure_keypair(&dir.join("k"), "ana")
            .unwrap_or_else(|err| panic!("keypair: {err}"));
        host.add_pasted_keys(&parsed.public_key, now())
            .unwrap_or_else(|err| panic!("add key: {err}"));
        let _ = std::fs::remove_dir_all(&dir);
        host
    }

    #[test]
    fn switch_requires_a_key_then_auto_off_clears_the_tunnel() {
        let mut host = Host::new();
        assert_eq!(host.turn_on(now()).unwrap_err().code, "key_required");
        let mut host = host_with_key();
        let effects = host.turn_on(now()).unwrap();
        assert_eq!(effects, vec![Effect::PrepareAccess, Effect::StartTunnel]);
        let host_name = host
            .ingest_tunnel_line("https://lime-eagle-4821.trycloudflare.com")
            .unwrap();
        assert_eq!(host_name, "lime-eagle-4821.trycloudflare.com");
        let view = host.view(now());
        assert_eq!(view.phase, "on");
        assert!(view
            .ssh_snippet
            .unwrap()
            .contains("ProxyCommand cloudflared access ssh --hostname %h"));
        assert_eq!(view.seconds_remaining, Some(7200));
        let later = now() + Duration::seconds(7200);
        let effects = host.poll(later);
        assert!(effects.contains(&Effect::StopTunnel));
        assert!(effects.contains(&Effect::SyncAuthorizedKeys));
        assert!(!effects.contains(&Effect::StopSshd) || host.settings.stop_sshd_if_started_by_us);
        assert_eq!(host.phase, Phase::Off);
        assert_eq!(
            host.keys.len(),
            1,
            "keys stay in the app unless revoke is set"
        );
    }

    #[test]
    fn revoke_forgets_keys_and_off_always_drops_them_from_the_file_plan() {
        let mut host = host_with_key();
        host.sshd_started_by_us = true;
        let _ = host.turn_on(now()).unwrap();
        let effects = host.turn_off(now(), true);
        assert!(effects.contains(&Effect::StopSshd));
        assert!(effects.contains(&Effect::SyncAuthorizedKeys));
        assert!(host.keys.is_empty());
        assert!(!host.sshd_started_by_us);
    }

    #[test]
    fn someone_connected_follows_the_log() {
        let mut host = host_with_key();
        let _ = host.turn_on(now()).unwrap();
        host.ingest_tunnel_line("| https://lime-eagle-4821.trycloudflare.com |");
        let log = "2026-10-20T20:01:00+0000 h sshd[1]: Accepted publickey for tunelito from 127.0.0.1 port 42 ssh2: ED25519 SHA256:zzz\n";
        host.refresh_observations(log, "tunelito 7 sshd: tunelito@notty\n", true, &[], now());
        assert!(host.view(now()).someone_connected);
        let log = format!(
            "{log}2026-10-20T20:05:00+0000 h sshd[1]: Disconnected from user tunelito 127.0.0.1 port 42\n"
        );
        host.refresh_observations(&log, "", true, &[], now() + Duration::minutes(10));
        assert!(!host.view(now()).someone_connected);
    }

    #[test]
    fn settings_clamp_and_language() {
        let mut host = Host::new();
        host.update_settings(
            Settings {
                auto_off_secs: 1,
                revoke_keys_on_off: true,
                language: "fr".into(),
                stop_sshd_if_started_by_us: false,
            },
            now(),
        );
        assert_eq!(host.settings.auto_off_secs, MIN_AUTO_OFF_SECS);
        assert_eq!(host.settings.language, "en");
    }

    #[test]
    fn unexpected_tunnel_exit_turns_the_switch_off() {
        let mut host = host_with_key();
        let _ = host.turn_on(now()).unwrap();
        host.note_tunnel_exit(false);
        assert_eq!(host.phase, Phase::Off);
        assert_eq!(host.error_code.as_deref(), Some("tunnel_exited"));
        host.turn_on(now()).unwrap();
        host.note_tunnel_exit(true);
        assert!(host.is_engaged());
    }
}
