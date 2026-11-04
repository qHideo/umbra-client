//! Shared license and signed-lease contract for the client and license service.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{ED25519, Ed25519KeyPair, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const API_VERSION: u8 = 1;
pub const OFFLINE_GRACE_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const MAX_LEASE_LIFETIME_SECONDS: u64 = OFFLINE_GRACE_SECONDS;
pub const SIGNING_SEED_BYTES: usize = 32;
pub const LICENSE_KEY_RANDOM_BYTES: usize = 32;
pub const PUBLIC_KEY_BYTES: usize = 32;

/// Public verification key shipped in every client build. The server seed stays private.
pub const EMBEDDED_PUBLIC_KEY_B64: &str = include_str!("../keys/public.key");

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeasePayload {
    pub version: u8,
    pub license_id: String,
    pub device_id: String,
    pub issued_at: u64,
    pub offline_until: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignedLease {
    /// Base64url without padding of the exact JSON bytes covered by `signature`.
    pub payload: String,
    /// Base64url without padding Ed25519 signature over `payload`'s decoded bytes.
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseRequest {
    pub license_key: String,
    pub device_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseResponse {
    pub lease: SignedLease,
}

/// Read-only rank query accepted by the license/rank service.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RankLookupRequest {
    pub identities: Vec<String>,
}

/// Server-signed owner identities returned for one bounded roster query.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignedOwnerRanks {
    pub payload: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerRanksPayload {
    pub version: u8,
    pub issued_at: u64,
    pub expires_at: u64,
    pub owners: Vec<String>,
}

pub const MAX_RANK_LOOKUP_IDENTITIES: usize = 128;
pub const MAX_RANK_CLAIM_LIFETIME_SECONDS: u64 = 5 * 60;

#[derive(Debug, Error)]
pub enum LeaseError {
    #[error("license service signing key is invalid")]
    InvalidSigningKey,
    #[error("license lease encoding is invalid")]
    InvalidEncoding,
    #[error("license lease signature is invalid")]
    InvalidSignature,
    #[error("license lease fields are invalid")]
    InvalidFields,
    #[error("license lease is for another device")]
    WrongDevice,
    #[error("license lease is not active")]
    Expired,
    #[error("owner-rank response is invalid")]
    InvalidRankResponse,
}

/// Accepts only the product's generated key format and hashes it with a server-only pepper.
pub fn hash_license_key(pepper: &[u8], license_key: &str) -> Result<String, LeaseError> {
    let key = license_key.trim();
    let (prefix, encoded) = key.split_once('-').ok_or(LeaseError::InvalidFields)?;
    if !prefix.eq_ignore_ascii_case("UMBRA")
        || encoded.len() != LICENSE_KEY_RANDOM_BYTES * 2
        || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit())
        || pepper.len() < 32
    {
        return Err(LeaseError::InvalidFields);
    }
    let normalized = format!("UMBRA-{}", encoded.to_ascii_uppercase());
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, pepper);
    let digest = ring::hmac::sign(&key, normalized.as_bytes());
    Ok(digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Creates an Ed25519 identity from the server-only 32-byte seed.
pub fn signing_key(seed: &[u8]) -> Result<Ed25519KeyPair, LeaseError> {
    if seed.len() != SIGNING_SEED_BYTES {
        return Err(LeaseError::InvalidSigningKey);
    }
    Ed25519KeyPair::from_seed_unchecked(seed).map_err(|_| LeaseError::InvalidSigningKey)
}

/// Signs the canonical JSON serialization shared by the client and service.
pub fn sign_lease(payload: &LeasePayload, key: &Ed25519KeyPair) -> SignedLease {
    let bytes = serde_json::to_vec(payload).expect("license lease fields serialize");
    SignedLease {
        payload: URL_SAFE_NO_PAD.encode(&bytes),
        signature: URL_SAFE_NO_PAD.encode(key.sign(&bytes).as_ref()),
    }
}

/// Signs a short-lived owner-rank response. Normal users have no entry in `owners`.
pub fn sign_owner_ranks(
    owners: Vec<String>,
    issued_at: u64,
    key: &Ed25519KeyPair,
) -> SignedOwnerRanks {
    let payload = OwnerRanksPayload {
        version: API_VERSION,
        issued_at,
        expires_at: issued_at.saturating_add(MAX_RANK_CLAIM_LIFETIME_SECONDS),
        owners,
    };
    let bytes = serde_json::to_vec(&payload).expect("owner-rank fields serialize");
    SignedOwnerRanks {
        payload: URL_SAFE_NO_PAD.encode(&bytes),
        signature: URL_SAFE_NO_PAD.encode(key.sign(&bytes).as_ref()),
    }
}

/// Verifies a rank response's signature, expiry and that every claim belongs to this query.
pub fn verify_owner_ranks(
    signed: &SignedOwnerRanks,
    public_key: &[u8],
    requested: &[String],
    now: u64,
) -> Result<Vec<String>, LeaseError> {
    if public_key.len() != PUBLIC_KEY_BYTES || requested.len() > MAX_RANK_LOOKUP_IDENTITIES {
        return Err(LeaseError::InvalidRankResponse);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(&signed.payload)
        .map_err(|_| LeaseError::InvalidEncoding)?;
    let signature = URL_SAFE_NO_PAD
        .decode(&signed.signature)
        .map_err(|_| LeaseError::InvalidEncoding)?;
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(&bytes, &signature)
        .map_err(|_| LeaseError::InvalidSignature)?;
    let payload: OwnerRanksPayload =
        serde_json::from_slice(&bytes).map_err(|_| LeaseError::InvalidEncoding)?;
    if payload.version != API_VERSION
        || payload.expires_at <= now
        || payload.issued_at > now.saturating_add(30)
        || payload.expires_at.saturating_sub(payload.issued_at)
            > MAX_RANK_CLAIM_LIFETIME_SECONDS
        || payload.owners.len() > requested.len()
    {
        return Err(LeaseError::InvalidRankResponse);
    }
    let owner_count = payload.owners.len();
    let mut owners = payload.owners;
    owners.sort_unstable();
    owners.dedup();
    if owners.len() != owner_count
        || owners.iter().any(|owner| !requested.contains(owner))
    {
        return Err(LeaseError::InvalidRankResponse);
    }
    Ok(owners)
}

/// Verifies signature, shape, device binding and the bounded offline lifetime.
pub fn verify_lease(
    lease: &SignedLease,
    public_key: &[u8],
    expected_device_id: &str,
    now: u64,
) -> Result<LeasePayload, LeaseError> {
    if public_key.len() != PUBLIC_KEY_BYTES {
        return Err(LeaseError::InvalidSigningKey);
    }
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(&lease.payload)
        .map_err(|_| LeaseError::InvalidEncoding)?;
    let signature = URL_SAFE_NO_PAD
        .decode(&lease.signature)
        .map_err(|_| LeaseError::InvalidEncoding)?;
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(&payload_bytes, &signature)
        .map_err(|_| LeaseError::InvalidSignature)?;
    let payload: LeasePayload =
        serde_json::from_slice(&payload_bytes).map_err(|_| LeaseError::InvalidEncoding)?;
    if payload.version != API_VERSION
        || payload.license_id.is_empty()
        || payload.device_id.is_empty()
        || payload.offline_until <= payload.issued_at
        || payload.offline_until - payload.issued_at > MAX_LEASE_LIFETIME_SECONDS
        || payload.issued_at > now.saturating_add(300)
    {
        return Err(LeaseError::InvalidFields);
    }
    if payload.device_id != expected_device_id {
        return Err(LeaseError::WrongDevice);
    }
    if now >= payload.offline_until {
        return Err(LeaseError::Expired);
    }
    Ok(payload)
}
