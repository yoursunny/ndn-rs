//! Core types for the NDN File Transfer Protocol.

use serde::{Deserialize, Serialize};

/// Unique file identifier: hex-encoded SHA-256 of the file content.
pub type FileId = String;

/// How each file segment's Data packet is signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SigningMode {
    /// No signature (content-hash verification only).
    #[default]
    None,
    /// Ed25519 signature per segment.
    Ed25519,
    /// HMAC-SHA256 per segment (faster, pre-shared key only).
    Hmac,
}

impl std::fmt::Display for SigningMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None    => write!(f, "none"),
            Self::Ed25519 => write!(f, "ed25519"),
            Self::Hmac    => write!(f, "hmac"),
        }
    }
}

/// How file content is encrypted before chunking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EncryptionMode {
    /// No encryption.
    #[default]
    None,
    /// AES-256-GCM (key negotiated out-of-band or pre-shared).
    AesGcm,
}

/// Metadata describing a hosted file.
///
/// Served as JSON at `/<node>/ndn-ft/v0/file/<file-id>/meta`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File identifier (SHA-256 hex of raw content).
    pub id: FileId,

    /// Original file name (basename, no path).
    pub name: String,

    /// Total file size in bytes.
    pub size: u64,

    /// Number of segments.
    pub segments: u32,

    /// Bytes per segment (last segment may be smaller).
    pub segment_size: u32,

    /// Hex-encoded SHA-256 of the full reassembled content.
    ///
    /// Receivers MUST verify this after reassembly before accepting the file.
    pub sha256: String,

    /// MIME type (optional; helps receivers choose an application).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,

    /// NDN prefix of the node hosting this file.
    pub sender_prefix: String,

    /// Unix timestamp when this file was added to the store.
    pub ts: u64,

    /// Signing mode used for segment Data packets.
    pub signing: SigningMode,

    /// Encryption mode applied before chunking.
    pub encryption: EncryptionMode,
}

/// A transfer offer sent by a sender to a receiver's `/notify` endpoint.
///
/// Carried as JSON in the **application parameters** of the notification Interest.
/// Receivers respond with [`OfferResponse`] as the Data content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOffer {
    /// Protocol version (current: 1).
    pub version: u8,

    /// File metadata summary (same as stored in `/meta`).
    pub meta: FileMetadata,
}

/// Receiver's response to a [`FileOffer`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferResponse {
    /// Whether the receiver accepted the file.
    pub accept: bool,

    /// Human-readable reason for rejection (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl OfferResponse {
    /// Create an acceptance response.
    pub fn accept() -> Self {
        Self { accept: true, reason: None }
    }

    /// Create a rejection response with an optional reason.
    pub fn reject(reason: impl Into<String>) -> Self {
        Self { accept: false, reason: Some(reason.into()) }
    }
}

/// Options for hosting a file with [`FileServer`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostOpts {
    /// Signing mode for segment Data packets.
    pub signing: SigningMode,

    /// Encryption mode applied to content before chunking.
    pub encryption: EncryptionMode,

    /// Segment size in bytes (0 = use default: 8192).
    pub segment_size: usize,

    /// Data freshness period in milliseconds (0 = omit).
    pub freshness_ms: u64,

    /// If true, pre-chunk and sign all segments at host time.
    ///
    /// Increases startup latency but eliminates per-Interest signing cost.
    /// If false, segments are built on demand (lower memory, sign per request).
    pub pre_chunk: bool,
}

/// Global configuration for file transfer behaviour.
#[derive(Debug, Clone)]
pub struct TransferConfig {
    /// Default download directory.
    pub download_dir: std::path::PathBuf,

    /// Whether to auto-accept all incoming offers without user confirmation.
    pub auto_accept: bool,

    /// Default signing mode for files sent from this node.
    pub default_signing: SigningMode,

    /// Default encryption mode for files sent from this node.
    pub default_encryption: EncryptionMode,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            download_dir: dirs_next::download_dir()
                .unwrap_or_else(|| std::path::PathBuf::from(".")),
            auto_accept: false,
            default_signing: SigningMode::None,
            default_encryption: EncryptionMode::None,
        }
    }
}
