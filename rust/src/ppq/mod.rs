//! PPQ account orchestration module (plan Wave 1).
//!
//! Boundary rules (see `.planning/PPQ_ORCHESTRATED_ACCOUNT_ANDROID_PLAN.md`):
//! - This module owns PPQ *account* management: provisioning, funding,
//!   backup/restore custody. It is separate from `llm::ppq_private`, which
//!   owns the private-inference E2EE transport. Different failure modes,
//!   different tests.
//! - `credit_id` and API keys are `Zeroizing` everywhere and never enter
//!   `AppState`, logs, or error strings.
//! - Wire types in [`contracts`] are frozen against the sanitized fixtures in
//!   `src/tests/ppq-fixtures/` (Wave 0 evidence). Unknown invoice
//!   status values are handled conservatively, never guessed.

pub mod account;
pub mod client;
pub mod contracts;
pub mod recovery;
pub mod secret_store;

pub use client::{PpqClient, PRODUCTION_BASE_URL};
pub use contracts::{
    AccountCredentials, CurrencyLimits, InvoiceStatusValue, LightningInvoice, PaymentMethod,
    PaymentMethods, SatsAmount,
};
pub use secret_store::{
    PpqSecretStore, API_KEY_KEY, API_KEY_SERVICE, CREDIT_ID_KEY, CREDIT_ID_SERVICE,
};
