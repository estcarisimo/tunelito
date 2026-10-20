use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use chrono::Utc;
use clap::{Parser, Subcommand};
use tunelito_core::cloudflared::{
    download_url, extract_cloudflared_tgz, file_matches, install_verified_bytes, select_asset,
    Arch, OsKind,
};
use tunelito_core::hello::{
    hello_filename, hello_remote_command, hello_ssh_args, shell_join, should_send_once, Hello,
    HelloSsh,
};
use tunelito_core::keys::{ensure_keypair, validate_user, DEDICATED_USER};
use tunelito_core::netinfo::{local_os, parse_cloudflare_trace, parse_cymru_txt, parse_ripe_asn};
use tunelito_core::paths::{identity_path, ssh_config_path, AppPaths};
use tunelito_core::ssh_config::{upsert_host_block, SshHostEntry};

#[derive(Debug, Parser)]
#[command(
    name = "tunelito-connect",
    version,
    about = "Share a public key and connect to a Tunelito host through Cloudflare"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Print the helper public key, creating an ed25519 keypair if needed.
    Key,
    /// Write the ~/.ssh/config Host block for a trycloudflare hostname.
    Install(InstallArgs),
    /// Send unverified hello metadata to the host.
    Hello(HelloArgs),
    /// Install the Host block, send hello, and print the snippet.
    Connect(ConnectArgs),
    /// Download a checksum-verified cloudflared into the user bin directory.
    InstallCloudflared,
}

#[derive(Debug, clap::Args)]
struct InstallArgs {
    hostname: String,
    #[arg(long, default_value = DEDICATED_USER)]
    user: String,
    #[arg(long)]
    identity: Option<PathBuf>,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    no_hello_hook: bool,
}

#[derive(Debug, clap::Args)]
struct HelloArgs {
    #[arg(long)]
    hostname: String,
    #[arg(long, default_value = DEDICATED_USER)]
    user: String,
    #[arg(long)]
    identity: Option<PathBuf>,
    #[arg(long)]
    ip: Option<String>,
    #[arg(long)]
    asn: Option<String>,
    #[arg(long)]
    rdns: Option<String>,
    #[arg(long)]
    os: Option<String>,
    #[arg(long)]
    os_detail: Option<String>,
    #[arg(long)]
    once: bool,
    #[arg(long)]
    quiet: bool,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, clap::Args)]
struct ConnectArgs {
    hostname: String,
    #[arg(long, default_value = DEDICATED_USER)]
    user: String,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("tunelito-connect: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), tunelito_core::Error> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Cmd::Key) {
        Cmd::Key => print_key(),
        Cmd::Install(args) => install(&args),
        Cmd::Hello(args) => hello(&args),
        Cmd::Connect(args) => connect(&args),
        Cmd::InstallCloudflared => install_cloudflared(),
    }
}

fn print_key() -> Result<(), tunelito_core::Error> {
    let path = identity_path();
    let (parsed, created) = ensure_keypair(&path, "tunelito")?;
    if created {
        eprintln!("Created {}", path.display());
    }
    println!("{}", parsed.public_key);
    eprintln!("Fingerprint {}", parsed.fingerprint);
    eprintln!(
        "Send this public key to the person who will turn Tunelito on. Never send the private key."
    );
    Ok(())
}

fn install(args: &InstallArgs) -> Result<(), tunelito_core::Error> {
    validate_user(&args.user)?;
    let identity = args
        .identity
        .clone()
        .unwrap_or_else(identity_path)
        .display()
        .to_string();
    let mut entry = SshHostEntry::for_host(&args.hostname);
    entry.user = args.user.clone();
    entry.identity_file = identity;
    entry.hello_hook = !args.no_hello_hook;
    let path = args.config.clone().unwrap_or_else(ssh_config_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let updated = upsert_host_block(&existing, &entry)?;
    fs::write(&path, updated)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    eprintln!("Wrote {}", path.display());
    print!("{}", render_snippet(&entry)?);
    Ok(())
}

fn render_snippet(entry: &SshHostEntry) -> Result<String, tunelito_core::Error> {
    tunelito_core::ssh_config::render_host_block(entry)
}

fn connect(args: &ConnectArgs) -> Result<(), tunelito_core::Error> {
    print_key()?;
    install(&InstallArgs {
        hostname: args.hostname.clone(),
        user: args.user.clone(),
        identity: None,
        config: args.config.clone(),
        no_hello_hook: false,
    })?;
    hello(&HelloArgs {
        hostname: args.hostname.clone(),
        user: args.user.clone(),
        identity: None,
        ip: None,
        asn: None,
        rdns: None,
        os: None,
        os_detail: None,
        once: false,
        quiet: false,
        dry_run: args.dry_run,
    })?;
    Ok(())
}

fn hello(args: &HelloArgs) -> Result<(), tunelito_core::Error> {
    validate_user(&args.user)?;
    let paths = AppPaths::resolve();
    let stamp = paths.stamp_dir().join(stamp_name(&args.hostname));
    if args.once && !should_send_once(&stamp, SystemTime::now(), Duration::from_secs(600)) {
        return Ok(());
    }
    let identity = args.identity.clone().unwrap_or_else(identity_path);
    let (parsed, _) = ensure_keypair(&identity, "tunelito")?;
    let (fallback_os, fallback_detail) = local_os();
    let public_ip = args.ip.clone().or_else(|| {
        if args.dry_run {
            None
        } else {
            fetch_public_ip()
        }
    });
    let asn = args.asn.clone().or_else(|| {
        if args.dry_run {
            None
        } else {
            public_ip.as_deref().and_then(fetch_asn)
        }
    });
    let rdns = args.rdns.clone().or_else(|| {
        if args.dry_run {
            None
        } else {
            public_ip.as_deref().and_then(fetch_rdns)
        }
    });
    let hello = Hello::new(
        public_ip,
        asn,
        rdns,
        args.os.clone().unwrap_or(fallback_os),
        args.os_detail.clone().or(fallback_detail),
        parsed.fingerprint,
        Utc::now(),
    );
    let json = hello.to_json()?;
    let filename = hello_filename(&hello.key_fingerprint, hello.reported_at);
    let remote = hello_remote_command(&filename)?;
    let ssh_args = hello_ssh_args(&HelloSsh {
        hostname: args.hostname.clone(),
        user: args.user.clone(),
        identity_file: identity.display().to_string(),
        remote_command: remote,
    })?;
    if args.dry_run {
        if !args.quiet {
            println!("{json}");
            eprintln!("ssh {}", shell_join(&ssh_args));
        }
        return Ok(());
    }
    let mut child = Command::new("ssh");
    child.args(&ssh_args).stdin(std::process::Stdio::piped());
    let mut child = child
        .spawn()
        .map_err(|err| tunelito_core::Error::with_detail("prepare_failed", err.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(json.as_bytes())?;
    }
    let status = child
        .wait()
        .map_err(|err| tunelito_core::Error::with_detail("prepare_failed", err.to_string()))?;
    if !status.success() {
        return Err(tunelito_core::Error::prepare_failed(format!(
            "hello ssh exited with {status}"
        )));
    }
    if let Some(parent) = stamp.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&stamp, hello.reported_at.to_rfc3339())?;
    if !args.quiet {
        eprintln!(
            "Sent hello for {} (reported by client, unverified)",
            hello.key_fingerprint
        );
    }
    Ok(())
}

fn stamp_name(hostname: &str) -> String {
    hostname
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn install_cloudflared() -> Result<(), tunelito_core::Error> {
    let os = OsKind::current();
    let arch = Arch::current()?;
    let asset = select_asset(os, arch)?;
    let paths = AppPaths::resolve();
    let dest = paths.cloudflared_path();
    let marker = dest.with_extension(format!(
        "{}.sha256",
        dest.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("bin")
    ));
    if dest.exists()
        && fs::read_to_string(&marker).ok().as_deref() == Some(asset.sha256_hex)
        && (asset.archive || file_matches(&dest, asset.sha256_hex))
    {
        println!("{}", dest.display());
        return Ok(());
    }
    let url = download_url(asset);
    let bytes = http_get(&url)?;
    let bytes = if asset.archive {
        tunelito_core::checksum::verify_sha256(&bytes, asset.sha256_hex)?;
        extract_cloudflared_tgz(&bytes)?
    } else {
        bytes
    };
    if asset.archive {
        // The checksum above covered the archive. The raw binary has no separate pin.
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
        }
    } else {
        install_verified_bytes(&bytes, asset.sha256_hex, &dest)?;
    }
    fs::write(&marker, asset.sha256_hex)?;
    println!("{}", dest.display());
    eprintln!(
        "Add {} to PATH if it is not already there.",
        paths.bin_dir.display()
    );
    Ok(())
}

fn fetch_public_ip() -> Option<String> {
    let body = http_get("https://cloudflare.com/cdn-cgi/trace").ok()?;
    let text = String::from_utf8(body).ok()?;
    parse_cloudflare_trace(&text)
}

fn fetch_asn(ip: &str) -> Option<String> {
    if let Some(asn) = fetch_ripe(ip) {
        return Some(asn);
    }
    fetch_cymru(ip)
}

fn fetch_ripe(ip: &str) -> Option<String> {
    let url = format!("https://stat.ripe.net/data/network-info/data.json?resource={ip}");
    let body = http_get(&url).ok()?;
    let text = String::from_utf8(body).ok()?;
    parse_ripe_asn(&text)
}

fn fetch_cymru(ip: &str) -> Option<String> {
    let reversed = reverse_ipv4(ip)?;
    let output = Command::new("dig")
        .args(["+short", &format!("{reversed}.origin.asn.cymru.com"), "TXT"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_cymru_txt(&String::from_utf8(output.stdout).ok()?)
}

fn reverse_ipv4(ip: &str) -> Option<String> {
    let addr: std::net::Ipv4Addr = ip.parse().ok()?;
    let octets = addr.octets();
    Some(format!(
        "{}.{}.{}.{}",
        octets[3], octets[2], octets[1], octets[0]
    ))
}

fn fetch_rdns(ip: &str) -> Option<String> {
    let output = Command::new("getent").args(["hosts", ip]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    // `203.0.113.9 helper.example`
    let mut parts = text.split_whitespace();
    let _ip = parts.next()?;
    let name = parts.next()?.trim().trim_end_matches('.').to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn http_get(url: &str) -> Result<Vec<u8>, tunelito_core::Error> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(60))
        .build();
    let response = agent
        .get(url)
        .set("user-agent", "tunelito/0.1.0")
        .call()
        .map_err(|err| tunelito_core::Error::download_failed(err.to_string()))?;
    let mut buf = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut buf)
        .map_err(|err| tunelito_core::Error::download_failed(err.to_string()))?;
    Ok(buf)
}
