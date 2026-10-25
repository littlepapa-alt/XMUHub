//! Configuration from environment variables (systemd `EnvironmentFile=` in production).

use std::net::SocketAddr;
use std::path::PathBuf;

fn var(k: &str) -> Option<String> {
    std::env::var(k)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn var_or(k: &str, d: &str) -> String {
    var(k).unwrap_or_else(|| d.to_string())
}

pub const DEFAULT_MIRRORS: &[&str] = &[
    "https://gh-proxy.com",
    "https://ghfast.top",
    "https://ghproxy.net",
    "https://gh.llkk.cc",
    "https://ghproxy.cc",
    "https://gh.ddlc.top",
    "https://gh.idayer.com",
    "https://github.moeyy.xyz",
    "https://gh-proxy.org",
    "https://hk.gh-proxy.com",
    "https://cdn.gh-proxy.com",
    // Send CORS headers (checked 2026-09-25: byte-exact, Access-Control-Allow-Origin: *).
    "https://cors.isteed.cc",
    "https://ghpxy.hwinzniej.top",
    "https://gh.monlor.com",
    "https://gh.927223.xyz",
    "https://ghm.078465.xyz",
];

pub enum StorageKind {
    Local,
    GitHub,
    Cos,
}

pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    pub web_dir: PathBuf,
    pub storage: StorageKind,
    pub gh_owner: String,
    pub gh_token: String,
    pub gh_repo_prefix: String,
    pub gh_backup_repo: String,
    /// Optional proxy for server → GitHub API calls (small requests only).
    pub gh_api_proxy: Option<String>,
    pub cos_secret_id: String,
    pub cos_secret_key: String,
    pub cos_bucket: String,
    pub cos_region: String,
    pub cos_endpoint: String,
    pub cos_signed_url_ttl: u64,
    pub cos_upload_url_ttl: u64,
    pub worker_url: String,
    pub ticket_secret: Vec<u8>,
    pub mirrors: Vec<String>,
    pub db_cache_mb: usize,
    /// Emails that always get the admin role (bootstrap; no CLI needed while running).
    pub admins: Vec<String>,
    /// File with the maintained admin list (one email per line, `#` comments); re-read every minute.
    pub admins_file: Option<std::path::PathBuf>,
    /// Bearer token for automation (the archive importer); acts as the system admin.
    pub script_token: Option<String>,
    /// Mark the session cookie `Secure` (production is behind HTTPS).
    pub secure_cookie: bool,
    pub smtp: Option<crate::mailer::SmtpConfig>,
    pub smtp_backup: Option<crate::mailer::SmtpConfig>,
    /// Cap on bytes the server relays to GitHub per day (only when no Worker is used).
    pub relay_daily_bytes: u64,
    pub relay_concurrency: usize,
    /// Cloudflare Turnstile keys (site key is public, secret isn't); both empty = no human check.
    pub turnstile_sitekey: String,
    pub turnstile_secret: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Config> {
        let storage = match var_or("XMUHUB_STORAGE", "local").as_str() {
            "github" => StorageKind::GitHub,
            "local" => StorageKind::Local,
            "cos" => StorageKind::Cos,
            other => anyhow::bail!("XMUHUB_STORAGE must be `github`, `cos` or `local`, got `{other}`"),
        };
        let secret = var("UPLOAD_TICKET_SECRET").unwrap_or_default();
        if matches!(storage, StorageKind::GitHub) {
            for k in ["GH_STORE_USER", "GH_STORE_TOKEN", "UPLOAD_TICKET_SECRET"] {
                if var(k).is_none() {
                    anyhow::bail!("{k} is required when XMUHUB_STORAGE=github");
                }
            }
        }
        let cos_secret_id = var_or("COS_SECRET_ID", "");
        let cos_secret_key = var_or("COS_SECRET_KEY", "");
        let cos_bucket = var_or("COS_BUCKET", "");
        let cos_region = var_or("COS_REGION", "ap-beijing");
        if matches!(storage, StorageKind::Cos) {
            for (name, value) in [
                ("COS_SECRET_ID", &cos_secret_id),
                ("COS_SECRET_KEY", &cos_secret_key),
                ("COS_BUCKET", &cos_bucket),
            ] {
                if value.is_empty() {
                    anyhow::bail!("{name} is required when XMUHUB_STORAGE=cos");
                }
            }
        }
        let cos_endpoint = var("COS_ENDPOINT")
            .unwrap_or_else(|| format!("https://{}.cos.{}.myqcloud.com", cos_bucket, cos_region));
        Ok(Config {
            bind: var_or("XMUHUB_BIND", "127.0.0.1:8089").parse()?,
            data_dir: var_or("XMUHUB_DATA", "data").into(),
            web_dir: var_or("XMUHUB_WEB", "web").into(),
            storage,
            gh_owner: var_or("GH_STORE_USER", ""),
            gh_token: var_or("GH_STORE_TOKEN", ""),
            gh_repo_prefix: var_or("GH_REPO_PREFIX", "XMUHub-store"),
            gh_backup_repo: var_or("GH_BACKUP_REPO", "XMUHub-backup"),
            gh_api_proxy: var("GH_API_PROXY"),
            cos_secret_id,
            cos_secret_key,
            cos_bucket,
            cos_region,
            cos_endpoint,
            cos_signed_url_ttl: var_or("COS_SIGNED_URL_TTL", "1800").parse()?,
            cos_upload_url_ttl: var_or("COS_UPLOAD_URL_TTL", "3600").parse()?,
            worker_url: var_or("UPLOAD_WORKER_URL", ""),
            // Local dev still signs its tickets; a random per-process secret is fine there.
            ticket_secret: if secret.is_empty() {
                rand_secret()
            } else {
                secret.into_bytes()
            },
            mirrors: var("MIRRORS")
                .map(|m| {
                    m.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_else(|| DEFAULT_MIRRORS.iter().map(|s| s.to_string()).collect()),
            db_cache_mb: var_or("XMUHUB_DB_CACHE_MB", "32").parse()?,
            admins: var("XMUHUB_ADMINS")
                .map(|v| {
                    v.split(',')
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            admins_file: var("XMUHUB_ADMINS_FILE").map(Into::into),
            script_token: var("XMUHUB_SCRIPT_TOKEN").filter(|t| t.len() >= 32),
            secure_cookie: var_or("XMUHUB_SECURE_COOKIE", "1") != "0",
            smtp: smtp_config("SMTP_", "MAIL_FROM")?,
            smtp_backup: smtp_config("SMTP_BACKUP_", "MAIL_BACKUP_FROM")?,
            relay_daily_bytes: var_or("RELAY_DAILY_MB", "5120").parse::<u64>()? * 1024 * 1024,
            relay_concurrency: var_or("RELAY_CONCURRENCY", "4").parse()?,
            turnstile_sitekey: var_or("TURNSTILE_SITEKEY", ""),
            turnstile_secret: var_or("TURNSTILE_SECRET", ""),
        })
    }
}

fn smtp_config(prefix: &str, from_key: &str) -> anyhow::Result<Option<crate::mailer::SmtpConfig>> {
    let host = var(&format!("{prefix}HOST"));
    let user = var(&format!("{prefix}USER"));
    let password = var(&format!("{prefix}PASSWORD"));
    // An empty password leaves a prepared account disabled until its authorization code is set.
    if password.is_none() {
        return Ok(None);
    }
    let host = host
        .ok_or_else(|| anyhow::anyhow!("{prefix}HOST is required when {prefix}PASSWORD is set"))?;
    let user = user
        .ok_or_else(|| anyhow::anyhow!("{prefix}USER is required when {prefix}PASSWORD is set"))?;
    Ok(Some(crate::mailer::SmtpConfig {
        host,
        port: var_or(&format!("{prefix}PORT"), "465").parse()?,
        from: var(from_key).unwrap_or_else(|| user.clone()),
        user,
        password: password.unwrap(),
    }))
}

fn rand_secret() -> Vec<u8> {
    use std::hash::{BuildHasher, RandomState};
    (0..4)
        .flat_map(|i| RandomState::new().hash_one(i).to_le_bytes())
        .collect()
}

/// Reads the admin list file; a missing file means an empty list.
pub fn read_admins(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|s| {
            s.lines()
                .map(|l| l.split('#').next().unwrap_or("").trim().to_lowercase())
                .filter(|l| l.contains('@'))
                .collect()
        })
        .unwrap_or_default()
}
