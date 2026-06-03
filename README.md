# synapt-core

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/images/logo/png/SynaptV2_White_PNG_512sq.png">
    <img src="./assets/images/logo/png/SynaptV2_Black_PNG_512sq.png" alt="Synapt" width="120">
  </picture>
</p>

synapt-core is the shared type library for the Synapt ecosystem. It defines the data structures used across Synapt and SynaptClip with no logic, no networking, and no async code.

## Contents

- `types::peer` — `Peer`, `TrustedPeer`, `PeerStatus`: LAN device discovery and trust state
- `types::transfer` — `TransferId`, `TransferStatus`: file transfer tracking
- `types::clip` — `Clip`, `ClipType`: clipboard entry shared with SynaptClip
- `types::error` — `CoreError`: base error type for cross-crate boundaries

## What this crate does not contain

- No network code
- No async functions
- No database access
- No platform-specific code
- No business logic

For the application code see [synapt](https://github.com/aatishbagal/synapt) or [synapt-clip](https://github.com/aatishbagal/synapt-clip).

## Usage

This crate is not published to crates.io. Synapt and SynaptClip consume it via a path dependency set up by their install scripts.

```bash
git clone https://github.com/aatishbagal/synapt-core.git
```

```toml
# Cargo.toml
synapt-core = { path = "../synapt-core" }
```

## Types

```rust
pub struct Peer {
    pub device_id: Uuid,
    pub device_name: String,
    pub ip: IpAddr,
    pub pairing_port: u16,
    pub status: PeerStatus,
}

pub enum PeerStatus { Discovered, Pairing, Trusted }

pub struct TrustedPeer {
    pub device_id: Uuid,
    pub device_name: String,
    pub pubkey_b64: String,
    pub fingerprint: String,
    pub paired_at: i64,
    pub last_seen: Option<i64>,
}

pub struct TransferId(pub Uuid);

pub enum TransferStatus {
    Pending,
    InProgress { bytes_received: u64, total: u64 },
    Complete,
    Failed(String),
    Partial { bytes_received: u64, total: u64 },
}

pub struct Clip {
    pub id: i64,
    pub content: String,
    pub content_type: ClipType,
    pub created_at: DateTime<Utc>,
    pub source_app: Option<String>,
}
```

## Contributing

This crate has a narrow scope by design. Changes to shared types affect both Synapt and SynaptClip. Open an issue before adding new types or modifying existing ones.

## License

Apache License 2.0 — see [LICENSE](./LICENSE).
