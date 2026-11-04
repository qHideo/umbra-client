//! Server-verified Umbra access leases and OS-protected license credentials.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use launcher::install_layout::InstallLayout;
use license::{EMBEDDED_PUBLIC_KEY_B64, LeaseRequest, LeaseResponse, SignedLease, verify_lease};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use url::Url;

const CREDENTIAL_SERVICE: &str = "umbra-client-license-v1";
const CREDENTIAL_ACCOUNT: &str = "license";
const CREDENTIAL_LIMIT: usize = 4096;

#[derive(Debug, Error)]
pub enum LicenseAccessError {
    #[error("no license is activated; run the client with --activate-license")]
    NotActivated,
    #[error("the license credential store is unavailable; activate the license again")]
    CredentialStore,
    #[error("license service URL is missing or invalid; set UMBRA_LICENSE_API_URL")]
    ServiceUrl,
    #[error("license service rejected this license or device")]
    Rejected,
    #[error("no valid online lease or unexpired offline lease is available")]
    NoValidLease,
    #[error("license lease cache could not be read or saved")]
    Cache,
    #[error("license activation failed; check the key, connection, and service URL")]
    Activation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredCredential {
    license_key: String,
    device_id: String,
}

/// Checks or refreshes the current user's device-bound lease before the game starts.
pub fn ensure_authorized(layout: &InstallLayout) -> Result<(), LicenseAccessError> {
    let credential = read_credential()?.ok_or(LicenseAccessError::NotActivated)?;
    match service_url() {
        Ok(url) => match request_lease(&url, &credential) {
            Ok(lease) => save_lease(layout, &lease, &credential.device_id),
            Err(RequestFailure::Rejected) => Err(LicenseAccessError::Rejected),
            Err(RequestFailure::Offline) => verify_cached(layout, &credential.device_id),
        },
        Err(_) => verify_cached(layout, &credential.device_id),
    }
}

/// Activates a user-entered key and stores it in the platform credential vault.
pub fn activate(layout: &InstallLayout, license_key: &str) -> Result<(), LicenseAccessError> {
    let license_key = license_key.trim();
    if license::hash_license_key(&[0; 32], license_key).is_err() {
        return Err(LicenseAccessError::Activation);
    }
    let device_id = match read_credential()? {
        Some(credential) => credential.device_id,
        None => new_device_id()?,
    };
    let credential = StoredCredential {
        license_key: license_key.to_owned(),
        device_id,
    };
    let url = service_url().map_err(|_| LicenseAccessError::ServiceUrl)?;
    let lease = request_lease(&url, &credential).map_err(|failure| match failure {
        RequestFailure::Rejected => LicenseAccessError::Rejected,
        RequestFailure::Offline => LicenseAccessError::Activation,
    })?;
    validate_lease(&lease, &credential.device_id).map_err(|_| LicenseAccessError::Activation)?;
    write_credential(&credential)?;
    save_lease(layout, &lease, &credential.device_id)
}

fn read_credential() -> Result<Option<StoredCredential>, LicenseAccessError> {
    let entry = credential_entry()?;
    match entry.get_password() {
        Ok(secret) if secret.len() <= CREDENTIAL_LIMIT => serde_json::from_str(&secret)
            .map(Some)
            .map_err(|_| LicenseAccessError::CredentialStore),
        Ok(_) => Err(LicenseAccessError::CredentialStore),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(LicenseAccessError::CredentialStore),
    }
}

fn write_credential(credential: &StoredCredential) -> Result<(), LicenseAccessError> {
    let secret =
        serde_json::to_string(credential).map_err(|_| LicenseAccessError::CredentialStore)?;
    credential_entry()?
        .set_password(&secret)
        .map_err(|_| LicenseAccessError::CredentialStore)
}

fn credential_entry() -> Result<keyring::Entry, LicenseAccessError> {
    keyring::Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_ACCOUNT)
        .map_err(|_| LicenseAccessError::CredentialStore)
}

fn new_device_id() -> Result<String, LicenseAccessError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| LicenseAccessError::CredentialStore)?;
    Ok(STANDARD.encode(bytes))
}

fn service_url() -> Result<Url, LicenseAccessError> {
    let raw = std::env::var("UMBRA_LICENSE_API_URL").map_err(|_| LicenseAccessError::ServiceUrl)?;
    let mut url = Url::parse(raw.trim()).map_err(|_| LicenseAccessError::ServiceUrl)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(LicenseAccessError::ServiceUrl);
    }
    let local_http =
        url.scheme() == "http" && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.scheme() != "https" && !local_http {
        return Err(LicenseAccessError::ServiceUrl);
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url.join("v1/lease")
        .map_err(|_| LicenseAccessError::ServiceUrl)
}

enum RequestFailure {
    Offline,
    Rejected,
}

fn request_lease(url: &Url, credential: &StoredCredential) -> Result<SignedLease, RequestFailure> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| RequestFailure::Offline)?;
    let response = client
        .post(url.clone())
        .json(&LeaseRequest {
            license_key: credential.license_key.clone(),
            device_id: credential.device_id.clone(),
        })
        .send()
        .map_err(|_| RequestFailure::Offline)?;
    if !response.status().is_success() {
        return Err(RequestFailure::Rejected);
    }
    let body: LeaseResponse = response.json().map_err(|_| RequestFailure::Rejected)?;
    validate_lease(&body.lease, &credential.device_id).map_err(|_| RequestFailure::Rejected)?;
    Ok(body.lease)
}

fn validate_lease(lease: &SignedLease, device_id: &str) -> Result<(), license::LeaseError> {
    let public_key = STANDARD
        .decode(EMBEDDED_PUBLIC_KEY_B64.trim())
        .map_err(|_| license::LeaseError::InvalidSigningKey)?;
    verify_lease(lease, &public_key, device_id, now_seconds())?;
    Ok(())
}

fn save_lease(
    layout: &InstallLayout,
    lease: &SignedLease,
    device_id: &str,
) -> Result<(), LicenseAccessError> {
    validate_lease(lease, device_id).map_err(|_| LicenseAccessError::Rejected)?;
    let path = layout.license_lease_file();
    let bytes = serde_json::to_vec(lease).map_err(|_| LicenseAccessError::Cache)?;
    write_atomic(&path, &bytes)?;
    Ok(())
}

fn verify_cached(layout: &InstallLayout, device_id: &str) -> Result<(), LicenseAccessError> {
    let lease: SignedLease = fs::read(layout.license_lease_file())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or(LicenseAccessError::NoValidLease)?;
    validate_lease(&lease, device_id).map_err(|_| LicenseAccessError::NoValidLease)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), LicenseAccessError> {
    let parent = path.parent().ok_or(LicenseAccessError::Cache)?;
    fs::create_dir_all(parent).map_err(|_| LicenseAccessError::Cache)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|_| LicenseAccessError::Cache)?;
    fs::rename(&temporary, path).map_err(|_| LicenseAccessError::Cache)
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
