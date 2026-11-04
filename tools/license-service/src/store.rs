use anyhow::{Context, Result, anyhow, bail};
use license::{LeasePayload, SignedLease, hash_license_key, sign_lease, signing_key};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

const LICENSES: TableDefinition<&str, &[u8]> = TableDefinition::new("licenses_v1");
const KEY_INDEX: TableDefinition<&str, &str> = TableDefinition::new("key_hash_index_v1");

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LicenseRecord {
    pub id: String,
    pub label: String,
    pub max_devices: usize,
    pub expires_at: Option<u64>,
    pub revoked: bool,
    pub created_at: u64,
    pub devices: BTreeMap<String, u64>,
}

pub struct LicenseStore {
    database: Database,
    pepper: Vec<u8>,
    signing_seed: Vec<u8>,
}

impl LicenseStore {
    pub fn open(path: &Path, pepper: Vec<u8>, signing_seed: Vec<u8>) -> Result<Arc<Self>> {
        if pepper.len() < 32 {
            bail!("license pepper must contain at least 32 bytes");
        }
        let _ = signing_key(&signing_seed).context("invalid license signing seed")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let database = Database::create(path)
            .with_context(|| format!("open license database {}", path.display()))?;
        {
            let txn = database.begin_write()?;
            txn.open_table(LICENSES)?;
            txn.open_table(KEY_INDEX)?;
            txn.commit()?;
        }
        Ok(Arc::new(Self {
            database,
            pepper,
            signing_seed,
        }))
    }

    pub fn issue_key(
        &self,
        label: String,
        max_devices: usize,
        expires_at: Option<u64>,
    ) -> Result<(String, LicenseRecord)> {
        if max_devices == 0 || max_devices > 32 {
            bail!("max devices must be between 1 and 32");
        }
        if label.trim().is_empty() || label.len() > 120 {
            bail!("label must contain 1 to 120 bytes");
        }
        if expires_at.is_some_and(|expiry| expiry <= now()) {
            bail!("expiry must be in the future");
        }
        let random = SystemRandom::new();
        let mut key_bytes = [0u8; 32];
        let mut id_bytes = [0u8; 16];
        random
            .fill(&mut key_bytes)
            .map_err(|_| anyhow!("secure key generation failed"))?;
        random
            .fill(&mut id_bytes)
            .map_err(|_| anyhow!("secure id generation failed"))?;
        let key = format!("UMBRA-{}", hex(&key_bytes));
        let id = hex(&id_bytes);
        let digest = hash_license_key(&self.pepper, &key)?;
        let record = LicenseRecord {
            id: id.clone(),
            label,
            max_devices,
            expires_at,
            revoked: false,
            created_at: now(),
            devices: BTreeMap::new(),
        };
        let encoded = serde_json::to_vec(&record)?;
        let txn = self.database.begin_write()?;
        {
            let mut licenses = txn.open_table(LICENSES)?;
            let mut index = txn.open_table(KEY_INDEX)?;
            licenses.insert(id.as_str(), encoded.as_slice())?;
            index.insert(digest.as_str(), id.as_str())?;
        }
        txn.commit()?;
        Ok((key, record))
    }

    pub fn issue_lease(&self, license_key: &str, device_id: &str) -> Result<SignedLease> {
        if device_id.is_empty() || device_id.len() > 128 {
            bail!("device id is invalid");
        }
        let digest = hash_license_key(&self.pepper, license_key)?;
        let now = now();
        let txn = self.database.begin_write()?;
        let id = {
            let index = txn.open_table(KEY_INDEX)?;
            index
                .get(digest.as_str())?
                .map(|value| value.value().to_owned())
        }
        .context("license was not found")?;
        let record = {
            let mut licenses = txn.open_table(LICENSES)?;
            let stored = licenses
                .get(id.as_str())?
                .map(|entry| entry.value().to_vec())
                .context("license record is missing")?;
            let mut record: LicenseRecord = serde_json::from_slice(&stored)?;
            if record.revoked {
                bail!("license is revoked");
            }
            if record.expires_at.is_some_and(|expiry| now >= expiry) {
                bail!("license is expired");
            }
            if !record.devices.contains_key(device_id) && record.devices.len() >= record.max_devices
            {
                bail!("license device limit reached");
            }
            record.devices.insert(device_id.to_owned(), now);
            let encoded = serde_json::to_vec(&record)?;
            licenses.insert(id.as_str(), encoded.as_slice())?;
            record
        };
        txn.commit()?;
        let offline_until = record.expires_at.map_or(
            now.saturating_add(license::OFFLINE_GRACE_SECONDS),
            |expiry| expiry.min(now.saturating_add(license::OFFLINE_GRACE_SECONDS)),
        );
        let payload = LeasePayload {
            version: license::API_VERSION,
            license_id: id,
            device_id: device_id.to_owned(),
            issued_at: now,
            offline_until,
        };
        let signing_key = signing_key(&self.signing_seed)?;
        Ok(sign_lease(&payload, &signing_key))
    }

    pub fn list(&self) -> Result<Vec<LicenseRecord>> {
        let txn = self.database.begin_read()?;
        let table = txn.open_table(LICENSES)?;
        let mut records = table
            .iter()?
            .map(|entry| {
                let (_, value) = entry?;
                Ok(serde_json::from_slice(value.value())?)
            })
            .collect::<Result<Vec<LicenseRecord>>>()?;
        records.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(records)
    }

    pub fn revoke(&self, id: &str) -> Result<()> {
        let txn = self.database.begin_write()?;
        {
            let mut licenses = txn.open_table(LICENSES)?;
            let stored = licenses
                .get(id)?
                .map(|entry| entry.value().to_vec())
                .context("license id was not found")?;
            let mut record: LicenseRecord = serde_json::from_slice(&stored)?;
            record.revoked = true;
            let encoded = serde_json::to_vec(&record)?;
            licenses.insert(id, encoded.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
