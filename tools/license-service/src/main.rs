mod store;

use anyhow::{Context, Result, anyhow, bail};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::{Parser, Subcommand};
use license::{
    LeaseRequest, LeaseResponse, MAX_RANK_LOOKUP_IDENTITIES, RankLookupRequest,
    SignedOwnerRanks, sign_owner_ranks, signing_key,
};
use ring::{
    rand::{SecureRandom, SystemRandom},
    signature::KeyPair,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, net::SocketAddr, path::PathBuf, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use store::LicenseStore;

#[derive(Debug, Parser)]
#[command(
    name = "umbra-license",
    about = "Issue Umbra access keys and run the license service"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create the private signing seed and database pepper; updates the client's public key.
    Init {
        #[arg(long, default_value = ".local/license")]
        secrets_dir: PathBuf,
        #[arg(long, default_value = "crates/license/keys/public.key")]
        public_key: PathBuf,
    },
    /// Issue a new one-time display key.
    Create {
        #[arg(long)]
        label: String,
        #[arg(long, default_value_t = 1)]
        max_devices: usize,
        #[arg(long)]
        expires_at: Option<u64>,
        #[arg(long, default_value = ".local/license")]
        secrets_dir: PathBuf,
        #[arg(long, default_value = ".local/license/licenses.redb")]
        database: PathBuf,
    },
    /// Revoke a license by id. Existing offline leases remain valid until their signed expiry.
    Revoke {
        id: String,
        #[arg(long, default_value = ".local/license")]
        secrets_dir: PathBuf,
        #[arg(long, default_value = ".local/license/licenses.redb")]
        database: PathBuf,
    },
    /// List license metadata without displaying keys or key hashes.
    List {
        #[arg(long, default_value = ".local/license")]
        secrets_dir: PathBuf,
        #[arg(long, default_value = ".local/license/licenses.redb")]
        database: PathBuf,
    },
    /// Manage server-owned visual ranks. These commands require access to the server host.
    Rank {
        #[command(subcommand)]
        command: RankCommand,
    },
    /// Serve the lease API on loopback. Put an HTTPS reverse proxy in front for remote clients.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8788")]
        bind: SocketAddr,
        #[arg(long, default_value = ".local/license")]
        secrets_dir: PathBuf,
        #[arg(long, default_value = ".local/license/licenses.redb")]
        database: PathBuf,
        #[arg(long, default_value = ".local/license/ranks.json")]
        ranks: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum RankCommand {
    /// Grant the owner badge to one stable XUID or UUID.
    Grant {
        #[arg(long)]
        identity: String,
        #[arg(long)]
        label: String,
        #[arg(long, default_value = ".local/license/ranks.json")]
        ranks: PathBuf,
    },
    /// Revoke an owner badge by stable XUID or UUID.
    Revoke {
        #[arg(long)]
        identity: String,
        #[arg(long, default_value = ".local/license/ranks.json")]
        ranks: PathBuf,
    },
    /// List owner identities configured by the operator.
    List {
        #[arg(long, default_value = ".local/license/ranks.json")]
        ranks: PathBuf,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnerRecord {
    identity: String,
    label: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnerRegistry {
    owners: Vec<OwnerRecord>,
}

struct AppState {
    licenses: Arc<LicenseStore>,
    signer: Arc<ring::signature::Ed25519KeyPair>,
    ranks_path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    match Args::parse().command {
        Command::Init {
            secrets_dir,
            public_key,
        } => initialize(&secrets_dir, &public_key),
        Command::Create {
            label,
            max_devices,
            expires_at,
            secrets_dir,
            database,
        } => {
            let store = load_store(&secrets_dir, &database)?;
            let (key, record) = store.issue_key(label, max_devices, expires_at)?;
            println!("License ID: {}\nKey (shown once): {}", record.id, key);
            Ok(())
        }
        Command::Revoke {
            id,
            secrets_dir,
            database,
        } => {
            load_store(&secrets_dir, &database)?.revoke(&id)?;
            println!("Revoked license {id}. Existing offline leases last up to seven days.");
            Ok(())
        }
        Command::List {
            secrets_dir,
            database,
        } => {
            for record in load_store(&secrets_dir, &database)?.list()? {
                println!(
                    "{}\t{}\t{} device(s)\t{}\t{}",
                    record.id,
                    record.label,
                    record.devices.len(),
                    if record.revoked { "revoked" } else { "active" },
                    record
                        .expires_at
                        .map_or_else(|| "no expiry".to_owned(), |time| format!("expires {time}"))
                );
            }
            Ok(())
        }
        Command::Rank { command } => match command {
            RankCommand::Grant { identity, label, ranks } => {
                validate_identity(&identity)?;
                let mut registry = load_owner_registry(&ranks)?;
                if let Some(record) = registry.owners.iter_mut().find(|record| record.identity == identity) {
                    record.label = label;
                } else {
                    registry.owners.push(OwnerRecord { identity, label });
                }
                registry.owners.sort_by(|left, right| left.identity.cmp(&right.identity));
                save_owner_registry(&ranks, &registry)?;
                println!("Owner rank granted. The server will sign this rank on its next lookup.");
                Ok(())
            }
            RankCommand::Revoke { identity, ranks } => {
                validate_identity(&identity)?;
                let mut registry = load_owner_registry(&ranks)?;
                let previous = registry.owners.len();
                registry.owners.retain(|record| record.identity != identity);
                if registry.owners.len() != previous {
                    save_owner_registry(&ranks, &registry)?;
                }
                println!("Owner rank revoked for {identity}.");
                Ok(())
            }
            RankCommand::List { ranks } => {
                for record in load_owner_registry(&ranks)?.owners {
                    println!("{}\t{}", record.label, record.identity);
                }
                Ok(())
            }
        },
        Command::Serve {
            bind,
            secrets_dir,
            database,
            ranks,
        } => serve(bind, &secrets_dir, &database, ranks).await,
    }
}

fn initialize(secrets_dir: &std::path::Path, public_key_path: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(secrets_dir)?;
    let seed_path = secrets_dir.join("signing.seed");
    let pepper_path = secrets_dir.join("pepper");
    let random = SystemRandom::new();
    let mut seed = [0_u8; 32];
    let mut pepper = [0_u8; 32];
    random
        .fill(&mut seed)
        .map_err(|_| anyhow!("secure signing-key generation failed"))?;
    random
        .fill(&mut pepper)
        .map_err(|_| anyhow!("secure pepper generation failed"))?;
    let key_pair = signing_key(&seed)?;
    let public = STANDARD.encode(key_pair.public_key().as_ref());
    if seed_path.exists() || pepper_path.exists() {
        bail!("secret material already exists; refusing to rotate it")
    }
    let current = std::fs::read_to_string(public_key_path).unwrap_or_default();
    if !current.trim().is_empty()
        && current.trim() != "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
    {
        bail!("public key already exists; refusing to rotate it")
    }
    write_secret_once(&seed_path, &seed)?;
    write_secret_once(&pepper_path, &pepper)?;
    if let Some(parent) = public_key_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(public_key_path, format!("{public}\n"))?;
    println!(
        "Created local license secrets in {} and updated {}.",
        secrets_dir.display(),
        public_key_path.display()
    );
    println!(
        "Keep the secrets directory private; rebuild the client after its public key changes."
    );
    Ok(())
}

fn write_secret_once(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("create secret file {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn load_store(
    secrets_dir: &std::path::Path,
    database: &std::path::Path,
) -> Result<Arc<LicenseStore>> {
    let pepper =
        std::fs::read(secrets_dir.join("pepper")).context("run `umbra-license init` first")?;
    let seed = std::fs::read(secrets_dir.join("signing.seed"))
        .context("run `umbra-license init` first")?;
    LicenseStore::open(database, pepper, seed)
}

async fn serve(
    bind: SocketAddr,
    secrets_dir: &std::path::Path,
    database: &std::path::Path,
    ranks_path: PathBuf,
) -> Result<()> {
    if !bind.ip().is_loopback() {
        bail!("license service binds only to loopback; terminate HTTPS at a reverse proxy");
    }
    let licenses = load_store(secrets_dir, database)?;
    let seed = std::fs::read(secrets_dir.join("signing.seed"))?;
    let state = Arc::new(AppState {
        licenses,
        signer: Arc::new(signing_key(&seed)?),
        ranks_path,
    });
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/lease", post(issue_lease))
        .route("/v1/ranks/lookup", post(lookup_ranks))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    println!("License API listening at http://{bind} (loopback only)");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn issue_lease(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LeaseRequest>,
) -> Result<Json<LeaseResponse>, StatusCode> {
    let lease = state.licenses
        .issue_lease(&request.license_key, &request.device_id)
        .map_err(|_| StatusCode::FORBIDDEN)?;
    Ok(Json(LeaseResponse { lease }))
}

async fn lookup_ranks(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RankLookupRequest>,
) -> Result<Json<SignedOwnerRanks>, StatusCode> {
    if request.identities.is_empty()
        || request.identities.len() > MAX_RANK_LOOKUP_IDENTITIES
        || request.identities.iter().any(|identity| validate_identity(identity).is_err())
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let requested: BTreeSet<&str> = request.identities.iter().map(String::as_str).collect();
    if requested.len() != request.identities.len() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let registry = load_owner_registry(&state.ranks_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let owners = registry.owners.into_iter()
        .filter(|record| requested.contains(record.identity.as_str()))
        .map(|record| record.identity)
        .collect();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_secs();
    Ok(Json(sign_owner_ranks(owners, now, &state.signer)))
}

fn validate_identity(identity: &str) -> Result<()> {
    let xuid = !identity.is_empty()
        && identity.len() <= 20
        && identity.bytes().all(|byte| byte.is_ascii_digit());
    let uuid = identity.len() == 36
        && identity.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        });
    if xuid || uuid {
        Ok(())
    } else {
        bail!("identity must be a decimal XUID or UUID")
    }
}

fn load_owner_registry(path: &std::path::Path) -> Result<OwnerRegistry> {
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(OwnerRegistry::default());
    };
    let registry: OwnerRegistry = serde_json::from_slice(&bytes)
        .with_context(|| format!("read rank registry {}", path.display()))?;
    if registry.owners.len() > 10_000 {
        bail!("owner registry exceeds 10000 entries");
    }
    let mut seen = BTreeSet::new();
    for record in &registry.owners {
        validate_identity(&record.identity)?;
        if record.label.trim().is_empty() || record.label.len() > 64 || !seen.insert(&record.identity) {
            bail!("owner registry contains an invalid label or duplicate identity");
        }
    }
    Ok(registry)
}

fn save_owner_registry(path: &std::path::Path, registry: &OwnerRegistry) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(registry)?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)?;
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("replace rank registry {}", path.display()))?;
    }
    std::fs::rename(&temporary, path)
        .with_context(|| format!("replace rank registry {}", path.display()))
}
