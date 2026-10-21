use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::hello::Hello;
use crate::host::{Effect, Host, Phase, Settings};
use crate::keys::TrustedKey;
use crate::sessions::Session;

const STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Persisted {
    version: u32,
    settings: Settings,
    keys: Vec<TrustedKey>,
    phase: Phase,
    sshd_started_by_us: bool,
    sessions: Vec<Session>,
    unmatched_hellos: Vec<Hello>,
}

impl Host {
    /// Load state and force the switch off. A dead process must not leave the tunnel "on",
    /// and it must not start one at launch. If we had started sshd, the caller stops it.
    pub fn load(path: &Path, now: DateTime<Utc>) -> Result<(Self, Vec<Effect>), Error> {
        if !path.exists() {
            return Ok((Self::new(), vec![]));
        }
        let text = fs::read_to_string(path)?;
        let persisted: Persisted =
            serde_json::from_str(&text).map_err(|err| Error::with_detail("io", err.to_string()))?;
        if persisted.version != STATE_VERSION {
            return Err(Error::with_detail("io", "unknown state version"));
        }
        let mut host = Self {
            settings: persisted.settings.normalized(),
            keys: persisted.keys,
            phase: Phase::Off,
            sessions: persisted.sessions,
            unmatched_hellos: persisted.unmatched_hellos,
            sshd_started_by_us: persisted.sshd_started_by_us,
            error_code: None,
            error_detail: None,
            activity: None,
        };
        for session in &mut host.sessions {
            if session.active {
                session.active = false;
                session.ended_at.get_or_insert(now);
            }
        }
        let mut effects = Vec::new();
        if !matches!(persisted.phase, Phase::Off) {
            host.error_code = Some("tunnel_exited".into());
            effects.push(Effect::SyncAuthorizedKeys);
            if host.sshd_started_by_us && host.settings.stop_sshd_if_started_by_us {
                effects.push(Effect::StopSshd);
                host.sshd_started_by_us = false;
            }
        }
        Ok((host, effects))
    }

    pub fn save(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let persisted = Persisted {
            version: STATE_VERSION,
            settings: self.settings.clone(),
            keys: self.keys.clone(),
            phase: self.phase.clone(),
            sshd_started_by_us: self.sshd_started_by_us,
            sessions: self.sessions.clone(),
            unmatched_hellos: self.unmatched_hellos.clone(),
        };
        let json = serde_json::to_string_pretty(&persisted)
            .map_err(|err| Error::with_detail("io", err.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Host;

    #[test]
    fn reload_disarms_an_on_switch_and_does_not_autostart() {
        let dir = std::env::temp_dir().join(format!("tunelito-state-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        let mut host = Host::new();
        let key_dir = dir.join("k");
        fs::create_dir_all(&key_dir).unwrap();
        let (parsed, _) = crate::keys::ensure_keypair(&key_dir.join("id"), "ana").unwrap();
        let now = DateTime::parse_from_rfc3339("2026-10-20T20:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        host.add_pasted_keys(&parsed.public_key, now).unwrap();
        host.turn_on(now).unwrap();
        host.ingest_tunnel_line("https://lime-eagle-4821.trycloudflare.com");
        host.mark_sshd_started_by_us();
        host.save(&path).unwrap();
        let (loaded, effects) = Host::load(&path, now).unwrap();
        assert_eq!(loaded.phase, crate::host::Phase::Off);
        assert!(effects.contains(&Effect::StopSshd));
        assert!(effects.contains(&Effect::SyncAuthorizedKeys));
        assert!(!effects.contains(&Effect::StartTunnel));
        assert_eq!(loaded.keys.len(), 1);
        assert!(!loaded.sshd_started_by_us);
        let _ = fs::remove_dir_all(&dir);
    }
}
