//! Server-signed owner-rank lookups for the current Bedrock player roster.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use crossbeam_channel::{Receiver, Sender, unbounded};
use license::{
    EMBEDDED_PUBLIC_KEY_B64, MAX_RANK_LOOKUP_IDENTITIES, RankLookupRequest, SignedOwnerRanks,
    verify_owner_ranks,
};
use url::Url;

use super::{Snapshot, publish};

pub(super) struct Roster {
    pub session: u64,
    pub revision: u64,
    pub identities: Vec<String>,
}

pub(super) fn start(snapshot: Arc<Mutex<Snapshot>>) -> Sender<Roster> {
    let (send, receive) = unbounded();
    thread::spawn(move || run(snapshot, receive));
    send
}

fn run(snapshot: Arc<Mutex<Snapshot>>, receive: Receiver<Roster>) {
    while let Ok(roster) = receive.recv() {
        let roster = receive.try_iter().last().unwrap_or(roster);
        let owners = lookup(&roster.identities).unwrap_or_default();
        publish(&snapshot, |state| {
            if state.owner_rank_roster_key == Some((roster.session, roster.revision))
                && *state.owner_ranks != owners
            {
                state.owner_ranks = Arc::new(owners);
            }
        });
    }
}

fn lookup(identities: &[String]) -> Option<HashSet<String>> {
    if identities.is_empty() {
        return Some(HashSet::new());
    }
    if identities.len() > MAX_RANK_LOOKUP_IDENTITIES {
        return None;
    }
    let url = rank_lookup_url()?;
    let response: SignedOwnerRanks = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?
        .post(url)
        .json(&RankLookupRequest {
            identities: identities.to_vec(),
        })
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .ok()?;
    let public_key = STANDARD.decode(EMBEDDED_PUBLIC_KEY_B64.trim()).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    verify_owner_ranks(&response, &public_key, identities, now)
        .ok()
        .map(|owners| owners.into_iter().collect())
}

fn rank_lookup_url() -> Option<Url> {
    let base = std::env::var("UMBRA_RANK_API_URL")
        .or_else(|_| std::env::var("UMBRA_LICENSE_API_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:8788/".to_owned());
    Url::parse(&base).ok()?.join("v1/ranks/lookup").ok()
}
