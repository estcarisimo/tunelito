use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::Utc;
use tunelito_core::access::AccessRequest;
use tunelito_core::cloudflared::{
    download_url, file_matches, install_asset_bytes, select_asset, tunnel_args, Arch, OsKind,
};
use tunelito_core::hello::read_hellos;
use tunelito_core::host::{Host, Settings, ViewModel};
#[cfg(target_os = "linux")]
use tunelito_core::logs::linux_journal_args;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use tunelito_core::logs::process_list_args;
#[cfg(target_os = "windows")]
use tunelito_core::logs::windows_event_log_command;
use tunelito_core::paths::AppPaths;
#[cfg(not(target_os = "linux"))]
use tunelito_core::plan::coverage_note;
use tunelito_core::plan::{planned_dropin, Probe};
use tunelito_core::Error;

pub struct Runtime {
    pub host: Mutex<Host>,
    pub tunnel: Mutex<Option<Child>>,
    pub paths: AppPaths,
    pub intentional_stop: AtomicBool,
    op: Mutex<()>,
}

impl Runtime {
    pub fn load() -> Self {
        let paths = AppPaths::resolve();
        let _ = fs::create_dir_all(&paths.state_dir);
        let (host, effects) = Host::load(&paths.state_file(), Utc::now())
            .unwrap_or_else(|_| (Host::new(), Vec::new()));
        let runtime = Self {
            host: Mutex::new(host),
            tunnel: Mutex::new(None),
            paths,
            intentional_stop: AtomicBool::new(false),
            op: Mutex::new(()),
        };
        if !effects.is_empty() {
            let _ = runtime.apply_effects(&effects);
        }
        #[cfg(windows)]
        crate::windows_host::spawn_setup();
        runtime
    }

    pub fn get_state(&self) -> ViewModel {
        self.refresh().unwrap_or_else(|_| self.view())
    }

    pub fn add_key(&self, text: &str) -> Result<ViewModel, Error> {
        let _guard = self.op.lock().unwrap_or_else(|err| err.into_inner());
        let effects = {
            let mut host = self.lock_host();
            let effects = host.add_pasted_keys(text, Utc::now())?;
            self.save(&host)?;
            effects
        };
        self.apply_effects(&effects)?;
        Ok(self.view())
    }

    pub fn remove_key(&self, fingerprint: &str) -> Result<ViewModel, Error> {
        let _guard = self.op.lock().unwrap_or_else(|err| err.into_inner());
        let effects = {
            let mut host = self.lock_host();
            let effects = host.remove_key(fingerprint)?;
            self.save(&host)?;
            effects
        };
        self.apply_effects(&effects)?;
        Ok(self.view())
    }

    pub fn turn_on(&self) -> Result<ViewModel, Error> {
        let _guard = self.op.lock().unwrap_or_else(|err| err.into_inner());
        let effects = {
            let mut host = self.lock_host();
            let effects = host.turn_on(Utc::now())?;
            host.set_activity(Some("starting_sshd"));
            self.save(&host)?;
            effects
        };
        if let Err(err) = self.apply_effects(&effects) {
            let mut host = self.lock_host();
            host.fail(&err);
            let _ = self.save(&host);
            return Err(err);
        }
        if effects
            .iter()
            .any(|effect| matches!(effect, tunelito_core::Effect::StartTunnel))
        {
            if let Err(err) = self.start_tunnel() {
                let mut host = self.lock_host();
                host.fail(&err);
                let _ = self.save(&host);
                return Err(err);
            }
        }
        Ok(self.view())
    }

    pub fn turn_off(&self, revoke_keys: bool) -> Result<ViewModel, Error> {
        let _guard = self.op.lock().unwrap_or_else(|err| err.into_inner());
        self.stop_tunnel();
        let effects = {
            let mut host = self.lock_host();
            let effects = host.turn_off(Utc::now(), revoke_keys);
            self.save(&host)?;
            effects
        };
        self.apply_effects(&effects)?;
        Ok(self.view())
    }

    pub fn update_settings(&self, settings: Settings) -> Result<ViewModel, Error> {
        let _guard = self.op.lock().unwrap_or_else(|err| err.into_inner());
        let effects = {
            let mut host = self.lock_host();
            let effects = host.update_settings(settings, Utc::now());
            self.save(&host)?;
            effects
        };
        if effects
            .iter()
            .any(|effect| matches!(effect, tunelito_core::Effect::StopTunnel))
        {
            self.stop_tunnel();
        }
        self.apply_effects(&effects)?;
        Ok(self.view())
    }

    pub fn refresh(&self) -> Result<ViewModel, Error> {
        self.poll_tunnel();
        ingest_hostname_file(self);
        let (log, processes, process_ok) = collect_observations();
        let hellos = read_hellos(&self.paths.hellos_read_dirs);
        let mut host = self.lock_host();
        let effects = host.poll(Utc::now());
        host.refresh_observations(&log, &processes, process_ok, &hellos, Utc::now());
        if effects
            .iter()
            .any(|effect| matches!(effect, tunelito_core::Effect::StopTunnel))
        {
            drop(host);
            self.stop_tunnel();
            let host = self.lock_host();
            let _ = self.save(&host);
            drop(host);
            self.apply_effects(&effects)?;
            return Ok(self.view());
        }
        self.save(&host)?;
        Ok(host.view(Utc::now()))
    }

    pub fn shutdown(&self) {
        self.stop_tunnel();
        let effects = {
            let mut host = self.lock_host();
            if !host.is_engaged() {
                return;
            }
            let revoke = host.settings.revoke_keys_on_off;
            let effects = host.turn_off(Utc::now(), revoke);
            let _ = self.save(&host);
            effects
        };
        let _ = self.apply_effects(&effects);
    }

    fn view(&self) -> ViewModel {
        self.lock_host().view(Utc::now())
    }

    fn lock_host(&self) -> std::sync::MutexGuard<'_, Host> {
        self.host.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn save(&self, host: &Host) -> Result<(), Error> {
        host.save(&self.paths.state_file())
    }

    fn apply_effects(&self, effects: &[tunelito_core::Effect]) -> Result<(), Error> {
        let needs_files = effects.iter().any(|effect| {
            matches!(
                effect,
                tunelito_core::Effect::PrepareAccess | tunelito_core::Effect::SyncAuthorizedKeys
            )
        });
        if needs_files {
            let (request, started_before) = {
                let host = self.lock_host();
                let prepare = effects
                    .iter()
                    .any(|effect| matches!(effect, tunelito_core::Effect::PrepareAccess));
                (access_request(&host, prepare), sshd_is_active())
            };
            apply_access(&request)?;
            if request.prepare && request.start_sshd && !started_before {
                self.lock_host().mark_sshd_started_by_us();
                let host = self.lock_host();
                self.save(&host)?;
            }
        }
        if effects
            .iter()
            .any(|effect| matches!(effect, tunelito_core::Effect::StopSshd))
        {
            let host = self.lock_host();
            let request = AccessRequest {
                authorized_key_lines: Vec::new(),
                dropin: None,
                write_keys: false,
                prepare: false,
                start_sshd: false,
                stop_sshd: true,
                reload_sshd: false,
                windows_account_is_admin: false,
                windows_set_global_max_sessions: false,
                windows_add_sftp: false,
                windows_owner_sid: None,
            };
            drop(host);
            apply_access(&request)?;
        }
        Ok(())
    }

    fn start_tunnel(&self) -> Result<(), Error> {
        {
            let mut host = self.lock_host();
            host.set_activity(Some("downloading_cloudflared"));
        }
        let bin = ensure_cloudflared(&self.paths)?;
        {
            let mut host = self.lock_host();
            host.set_activity(Some("starting_tunnel"));
            self.save(&host)?;
        }
        self.intentional_stop.store(false, Ordering::SeqCst);
        clear_hostname(&self.paths);
        let mut cmd = Command::new(&bin);
        cmd.args(tunnel_args())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd
            .spawn()
            .map_err(|err| Error::prepare_failed(err.to_string()))?;
        if let Some(stdout) = child.stdout.take() {
            spawn_reader(stdout, self.paths.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_reader(stderr, self.paths.clone());
        }
        *self.tunnel.lock().unwrap_or_else(|err| err.into_inner()) = Some(child);
        Ok(())
    }

    fn stop_tunnel(&self) {
        self.intentional_stop.store(true, Ordering::SeqCst);
        clear_hostname(&self.paths);
        let mut slot = self.tunnel.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(mut child) = slot.take() {
            kill_child(&mut child);
        }
    }

    fn poll_tunnel(&self) {
        let mut slot = self.tunnel.lock().unwrap_or_else(|err| err.into_inner());
        let exited = slot
            .as_mut()
            .and_then(|child| child.try_wait().ok())
            .flatten()
            .is_some();
        if !exited {
            return;
        }
        let intentional = self.intentional_stop.load(Ordering::SeqCst);
        *slot = None;
        drop(slot);
        let mut host = self.lock_host();
        host.note_tunnel_exit(intentional);
        let _ = self.save(&host);
    }
}

fn spawn_reader<R>(pipe: R, paths: AppPaths)
where
    R: Read + Send + 'static,
{
    // The reader must not lock `Runtime.host`: refresh holds that lock while it waits
    // on nothing the reader needs, but a line-by-line lock would fight the UI thread.
    // The hostname file is ingested on refresh.
    let log_path = paths.log_file.with_file_name("tunnel.log");
    std::thread::spawn(move || {
        let _ = paths;
        let reader = BufReader::new(pipe);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if let Some(hostname) = tunelito_core::cloudflared::parse_quick_tunnel_hostname(&line) {
                let _ = fs::write(log_path.with_file_name("hostname"), &hostname);
            }
            let _ = append_line(&log_path, &line);
        }
    });
}

fn append_line(path: &Path, line: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn access_request(host: &Host, prepare: bool) -> AccessRequest {
    let mut probe = current_probe();
    if host.is_engaged() {
        probe.openssh_installed = probe.openssh_installed || prepare;
    }
    let dropin = if prepare {
        planned_dropin(&probe).ok().flatten()
    } else {
        None
    };
    let engaged = host.is_engaged();
    let reload_sshd = prepare && dropin.is_some();
    AccessRequest {
        authorized_key_lines: if engaged {
            host.key_lines()
        } else {
            Vec::new()
        },
        dropin,
        write_keys: true,
        prepare,
        start_sshd: prepare,
        stop_sshd: false,
        reload_sshd,
        windows_account_is_admin: probe.windows_user_is_admin,
        windows_set_global_max_sessions: false,
        windows_add_sftp: false,
        windows_owner_sid: None,
    }
}

fn current_probe() -> Probe {
    let mut probe = Probe::linux_fresh();
    probe.os = OsKind::current();
    probe.groups_present.clear();
    #[cfg(target_os = "linux")]
    {
        probe.openssh_installed = Command::new("sh")
            .args([
                "-c",
                "command -v sshd >/dev/null 2>&1 || dpkg -s openssh-server >/dev/null 2>&1",
            ])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        probe.sshd_active = sshd_is_active();
        probe.user_exists = Command::new("id")
            .args(["-u", "tunelito"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if let Ok(output) = Command::new("sshd").arg("-T").output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let parsed = tunelito_core::sshd::parse_sshd_effective(&text);
                probe.sshd_config_read = true;
                probe.max_sessions = parsed.max_sessions;
                probe.sftp_enabled = parsed.sftp_enabled;
            }
        }
        for group in tunelito_core::keys::DEVICE_GROUPS {
            if Command::new("getent")
                .args(["group", group])
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
            {
                probe.groups_present.push((*group).to_string());
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        // UNTESTED ON CI: Windows and macOS probes are filled by the OS-specific executor.
        let _ = coverage_note(probe.os);
    }
    probe
}

fn apply_access(request: &AccessRequest) -> Result<(), Error> {
    if let Some(root) = std::env::var_os("TUNELITO_ROOT") {
        let root = PathBuf::from(root);
        if root != Path::new("/") {
            tunelito_core::access::write_linux_access_files(&root, request)?;
            return Ok(());
        }
    }
    #[cfg(target_os = "linux")]
    {
        return apply_linux(request);
    }
    #[cfg(target_os = "macos")]
    {
        // UNTESTED ON CI: macOS Remote Login and file copies are not executed here.
        return apply_macos(request);
    }
    #[cfg(target_os = "windows")]
    {
        // UNTESTED ON CI: the Windows OpenSSH capability and ACL-sensitive key file are not executed here.
        return apply_windows(request);
    }
    #[allow(unreachable_code)]
    Err(Error::unsupported_platform(std::env::consts::OS))
}

#[cfg(target_os = "linux")]
fn apply_linux(request: &AccessRequest) -> Result<(), Error> {
    let bin = priv_binary().ok_or_else(Error::needs_admin)?;
    let path = std::env::temp_dir().join(format!("tunelito-request-{}.json", std::process::id()));
    fs::write(
        &path,
        serde_json::to_string(request).map_err(|err| Error::with_detail("io", err.to_string()))?,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    let args = [
        "apply",
        "--request",
        path.to_str().unwrap_or(""),
        "--root",
        "/",
        "--execute",
    ];
    let output = if euid_is_root() {
        Command::new(&bin).args(args).output()
    } else if desktop_session() && command_exists("pkexec") {
        Command::new("pkexec").arg(&bin).args(args).output()
    } else if command_exists("sudo") {
        Command::new("sudo").arg(&bin).args(args).output()
    } else {
        return Err(Error::needs_admin());
    };
    let output = output.map_err(|err| Error::prepare_failed(err.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(Error::prepare_failed(format!("{stderr}{stdout}")))
    }
}

#[cfg(target_os = "macos")]
fn apply_macos(request: &AccessRequest) -> Result<(), Error> {
    // UNTESTED ON CI. The helper's macOS steps are unit-tested in `tunelito_core::macos`.
    let bin = priv_binary().ok_or_else(Error::needs_admin)?;
    // $TMPDIR on macOS is per user and mode 0700.
    let path = std::env::temp_dir().join(format!("tunelito-request-{}.json", std::process::id()));
    fs::write(
        &path,
        serde_json::to_string(request).map_err(|err| Error::with_detail("io", err.to_string()))?,
    )?;
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    // Paths go in as argv and through `quoted form of`, never spliced into the script.
    let output = Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "do shell script (quoted form of item 1 of argv) & \" apply --macos --root / --execute --request \" & (quoted form of item 2 of argv) with administrator privileges",
            "-e",
            "end run",
        ])
        .arg(&bin)
        .arg(&path)
        .output();
    let _ = fs::remove_file(&path);
    let output = output.map_err(|err| Error::prepare_failed(err.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("-128") {
            // The person pressed Cancel on the password prompt.
            return Err(Error::needs_admin());
        }
        Err(Error::prepare_failed(stderr.trim().to_string()))
    }
}

#[cfg(target_os = "windows")]
fn apply_windows(request: &AccessRequest) -> Result<(), Error> {
    // UNTESTED ON CI. The Windows job builds this; it does not run it.
    crate::windows_host::apply(request)
}

fn priv_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TUNELITO_PRIV_BIN") {
        return Some(PathBuf::from(path));
    }
    let name = if cfg!(windows) {
        "tunelito-priv.exe"
    } else {
        "tunelito-priv"
    };
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidates = [
        dir.join(name),
        dir.join("..").join("lib").join("tunelito").join(name),
        PathBuf::from("/usr/lib/tunelito").join(name),
    ];
    candidates.into_iter().find(|path| path.exists())
}

fn euid_is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|text| text.trim() == "0")
}

fn desktop_session() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn sshd_is_active() -> bool {
    #[cfg(target_os = "linux")]
    {
        Command::new("systemctl")
            .args(["is-active", "--quiet", "ssh"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
            || Command::new("systemctl")
                .args(["is-active", "--quiet", "sshd"])
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

fn ensure_cloudflared(paths: &AppPaths) -> Result<PathBuf, Error> {
    let asset = select_asset(OsKind::current(), Arch::current()?)?;
    let dest = paths.cloudflared_path();
    if file_matches(&dest, asset.sha256_hex) || marker_matches(&dest, asset.sha256_hex) {
        return Ok(dest);
    }
    if let Some(bytes) = crate::embed::embedded_cloudflared() {
        install_asset_bytes(bytes, asset, &dest)?;
        let _ = fs::write(marker_path(&dest), asset.sha256_hex);
        return Ok(dest);
    }
    let bytes = http_get(&download_url(asset))?;
    install_asset_bytes(&bytes, asset, &dest)?;
    let _ = fs::write(marker_path(&dest), asset.sha256_hex);
    Ok(dest)
}

fn marker_path(dest: &Path) -> PathBuf {
    let ext = dest
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("bin");
    dest.with_extension(format!("{ext}.sha256"))
}

fn marker_matches(dest: &Path, expected: &str) -> bool {
    dest.exists() && fs::read_to_string(marker_path(dest)).ok().as_deref() == Some(expected)
}

fn http_get(url: &str) -> Result<Vec<u8>, Error> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(90))
        .build();
    let response = agent
        .get(url)
        .set("user-agent", "tunelito/0.1.0")
        .call()
        .map_err(|err| Error::download_failed(err.to_string()))?;
    let mut buf = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut buf)
        .map_err(|err| Error::download_failed(err.to_string()))?;
    Ok(buf)
}

fn collect_observations() -> (String, String, bool) {
    #[cfg(target_os = "linux")]
    {
        let log = Command::new("journalctl")
            .args(linux_journal_args())
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .or_else(|| {
                tunelito_core::access::read_limited(Path::new("/var/log/auth.log"), 400_000).ok()
            })
            .unwrap_or_default();
        let processes = Command::new("ps").args(process_list_args()).output();
        if let Ok(output) = processes {
            if output.status.success() {
                return (
                    log,
                    String::from_utf8_lossy(&output.stdout).into_owned(),
                    true,
                );
            }
        }
        return (log, String::new(), false);
    }
    #[cfg(target_os = "macos")]
    {
        // UNTESTED ON CI: `log show` text is parsed by the shared log parser.
        let log = Command::new("log")
            .args([
                "show",
                "--style",
                "syslog",
                "--last",
                "1d",
                "--predicate",
                "process == \"sshd\"",
            ])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default();
        let processes = Command::new("ps").args(process_list_args()).output();
        if let Ok(output) = processes {
            if output.status.success() {
                return (
                    log,
                    String::from_utf8_lossy(&output.stdout).into_owned(),
                    true,
                );
            }
        }
        return (log, String::new(), false);
    }
    #[cfg(target_os = "windows")]
    {
        // UNTESTED ON CI: OpenSSH Operational log via wevtutil.
        let mut args = windows_event_log_command();
        let program = args.remove(0);
        let log = Command::new(program)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default();
        return (log, String::new(), false);
    }
    #[allow(unreachable_code)]
    (String::new(), String::new(), false)
}

fn kill_child(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id();
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn clear_hostname(paths: &AppPaths) {
    let _ = fs::remove_file(hostname_path(paths));
}

fn hostname_path(paths: &AppPaths) -> PathBuf {
    paths.log_file.with_file_name("hostname")
}

fn ingest_hostname_file(runtime: &Runtime) {
    let path = hostname_path(&runtime.paths);
    let Ok(text) = fs::read_to_string(&path) else {
        return;
    };
    let mut host = runtime.lock_host();
    if host.ingest_tunnel_line(&text).is_some() {
        let _ = runtime.save(&host);
    }
}
