// SPDX-License-Identifier: Apache-2.0
// Copyright 2024 [Your Name]

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque transfer identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransferId(pub Uuid);

impl TransferId {
    /// Create a new random transfer identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TransferId {
    fn default() -> Self {
        Self::new()
    }
}

/// Current state of a file transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferStatus {
    Pending,
    InProgress { bytes_received: u64, total: u64 },
    Complete,
    Failed(String),
    Partial { bytes_received: u64, total: u64 },
}
