//! TEE attestation policy structs.
//!
//! Holds runtime-configurable thresholds for TDX and SNP attestation.
//! Stored as JSON in the `settings` table under keys `tee_policy_tdx` and
//! `tee_policy_snp`, seeded by MIGRATION_V14 with values identical to the
//! prior compile-time constants.

use serde::{Deserialize, Serialize};

/// TDX attestation policy thresholds.
///
/// `minimum_tee_tcb_svn` is a 32-character lowercase hex string representing
/// 16 bytes (the `tee_tcb_svn` field of the TDX quote report body).
/// `accepted_mr_seams` is the list of accepted MRSEAM measurement hex strings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TdxPolicy {
    /// Minimum acceptable TEE TCB SVN, encoded as a 32-char lowercase hex string.
    pub minimum_tee_tcb_svn: String,
    /// Accepted MRSEAM measurement hex strings.
    pub accepted_mr_seams: Vec<String>,
}

impl Default for TdxPolicy {
    fn default() -> Self {
        Self {
            minimum_tee_tcb_svn: "03010200000000000000000000000000".to_string(),
            accepted_mr_seams: vec![
                "476a2997c62bccc78370913d0a80b956e3721b24272bc66c4d6307ced4be2865c40e26afac75f12df3425b03eb59ea7c".to_string(),
                "7bf063280e94fb051f5dd7b1fc59ce9aac42bb961df8d44b709c9b0ff87a7b4df648657ba6d1189589feab1d5a3c9a9d".to_string(),
                "685f891ea5c20e8fa27b151bf34bf3b50fbaf7143cc53662727cbdb167c0ad8385f1f6f3571539a91e104a1c96d75e04".to_string(),
                "49b66faa451d19ebbdbe89371b8daf2b65aa3984ec90110343e9e2eec116af08850fa20e3b1aa9a874d77a65380ee7e6".to_string(),
            ],
        }
    }
}

impl TdxPolicy {
    /// Decode `minimum_tee_tcb_svn` from hex into a fixed 16-byte array.
    pub fn minimum_tee_tcb_svn_bytes(&self) -> Result<[u8; 16], hex::FromHexError> {
        let bytes = hex::decode(&self.minimum_tee_tcb_svn)?;
        let mut arr = [0u8; 16];
        if bytes.len() != 16 {
            // Return an error by constructing an invalid hex decode scenario.
            // hex::FromHexError doesn't expose a constructor for length errors,
            // so we decode a deliberately bad string to produce the right type.
            return hex::decode("xy").map(|_| arr);
        }
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

/// SNP attestation policy thresholds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnpPolicy {
    /// Minimum acceptable bootloader TCB version.
    pub minimum_bootloader: u8,
    /// Minimum acceptable TEE TCB version.
    pub minimum_tee: u8,
    /// Minimum acceptable SNP TCB version.
    pub minimum_snp: u8,
    /// Minimum acceptable microcode TCB version.
    pub minimum_microcode: u8,
}

impl Default for SnpPolicy {
    fn default() -> Self {
        Self {
            minimum_bootloader: 7,
            minimum_tee: 0,
            minimum_snp: 14,
            minimum_microcode: 72,
        }
    }
}

/// Combined TEE attestation policy (TDX + SNP).
#[derive(Debug, Clone, Default)]
pub struct TeePolicy {
    pub tdx: TdxPolicy,
    pub snp: SnpPolicy,
}
