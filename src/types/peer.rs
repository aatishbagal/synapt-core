// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Aatish Bagal

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::net::IpAddr;

/// Status of a discovered peer relative to the local trust store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    Discovered,
    Pairing,
    Trusted,
}

/// A peer currently visible on the LAN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub device_id: Uuid,
    pub device_name: String,
    pub ip: IpAddr,
    pub pairing_port: u16,
    pub status: PeerStatus,
}

/// A peer that has been paired and stored in the trust store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedPeer {
    pub device_id: Uuid,
    pub device_name: String,
    pub pubkey_b64: String,
    pub fingerprint: String,
    pub paired_at: i64,
    pub last_seen: Option<i64>,
}
