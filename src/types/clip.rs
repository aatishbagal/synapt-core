// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Aatish Bagal

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Content type of a clipboard entry. Shared with synapt-clip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipType {
    Text,
    Image,
    File,
}

/// A clipboard entry. Shared type used by both Synapt and SynaptClip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: i64,
    pub content: String,
    pub content_type: ClipType,
    pub created_at: DateTime<Utc>,
    pub source_app: Option<String>,
}
