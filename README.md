# Mango

Mango is a confidential AI client for chatting with confidential inference providers from a local app. It focuses on security, privacy, and local-first capabilities. Its based on BYOK (Bring Your Own Key), no subscriptions or cloud storage. 

## Integrated providers

Mango currently includes built-in support for:

- Tinfoil
- PPQ.AI
- Custom providers using confidential/self-hosted endpoints

It also integrates:

- Brave Search for web search tools

### PPQ managed accounts

Mango can set up a PPQ account automatically inside the app, so you do not need a PPQ account first. You fund it from any external Lightning wallet by scanning a QR code. Your PPQ balance is prepaid USD credit held by PPQ; Mango never touches the funds. Your credentials are stored on the device and can be exported to an encrypted `.mppq` backup with a password. A duress wipe erases local data while preserving PPQ credentials, and existing BYOK PPQ keys and custom providers still work. Back up, restore, or top up from **Settings → Providers**.

- Automatic PPQ setup
- Lightning top-up via external wallet or QR
- Encrypted `.mppq` backup and restore
- Duress wipe preserves PPQ credentials
- BYOK and custom providers still supported

Top-ups use Bitcoin Lightning in the range PPQ currently advertises (100–1,000,000 sats per PPQ's live limits).

## Features
- Chat with selectable models and streaming responses
- RAG by attaching documents to conversations
- Local semantic search with on-device embeddings
- Conversation memory extraction and memory management
- Per-conversation instructions and tool-use controls
- Local document library with PDF/text ingestion

## Design Plans
- [Inference routing improvement plan](INFERENCE_ROUTING_PLAN.md)
- [Android remediation implementation plan](ANDROID_REMEDIATION_IMPLEMENTATION_PLAN.md)

## Security & Privacy
- App lock with PIN and biometric unlock
- Duress PIN support to erase local data
- Local encrypted persistence
- Provider health and attestation status in the UI
- Re-attestation interval controls
- Onboarding flow for provider setup and attestation demo


## Roadmap 
- More integrated tools 
- Agentic workflows
- Multi-provider routing
- Multi modal support
- Local models support
