# Security Policy

## Supported Versions

Security fixes are prepared against the current `main` branch before each public release.

## Reporting a Vulnerability

Please report security issues privately to the maintainers instead of opening a public issue. Include:

- affected platform or build
- steps to reproduce
- expected and observed impact
- logs or screenshots with secrets removed

We will acknowledge valid reports, investigate the affected storage or inference boundary, and publish a fix once users have an update path.

## Sensitive Data Expectations

Mango is designed for confidential local storage. Reports involving backup behavior, plaintext temporary files, lock-screen bypasses, duress wipe, API-key storage, model tool access, and attestation claims are security relevant.
