# Mango vNext — Managed PPQ accounts

## What's new
- **Automatic PPQ setup**: create a PPQ account inside Mango without first making one on ppq.ai.
- **Lightning top-up**: generate a Bitcoin Lightning invoice and pay it from any external Lightning wallet by scanning a QR code or opening a wallet intent.
- **Encrypted `.mppq` backup and restore**: export your PPQ credentials to a password-protected `.mppq` file and restore them on a new or reset device.
- **USD credit balance display**: see your PPQ balance as prepaid USD credit.
- **Insufficient-balance handling**: if a chat fails because of low PPQ balance, Mango prompts you to top up before retrying.
- **Duress behavior**: a duress wipe erases local Mango data but preserves PPQ credentials, so the PPQ account can be recovered after the threat has passed.

## Requirements
- A managed PPQ account is a user-owned PPQ account. **PPQ holds the balance as prepaid USD credit; Mango never touches funds.**
- Lightning top-ups are denominated in sats and must be within the range PPQ currently advertises (100–1,000,000 sats per PPQ's live limits).
- Creating or restoring an `.mppq` backup requires the backup password you chose; Mango does not store it.

## Security notes
- PPQ `credit_id` and API keys are stored in the platform-protected keychain and never enter app state, logs, crash reports, or analytics.
- `.mppq` backups are encrypted with Argon2id and AES-GCM using your backup password; Mango never stores or transmits the password.
- Mango collects no telemetry or analytics.

## Known limitations
- Restore requires both the `.mppq` file and its password. If either is lost, Mango cannot recover the PPQ account.
- Lightning invoices expire after 15 minutes if unpaid.
- Managed PPQ UI and copy are English-only in this release.
- Android is supported first; other platforms will follow.
