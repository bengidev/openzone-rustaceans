# Security Policy

## Supported Versions

OpenZone Rustaceans is in early development. Security updates apply to the latest `main` branch until stable releases are published.

## Reporting a Vulnerability

If you discover a security issue, please report it privately before public disclosure.

Include:

- A clear description of the vulnerability
- Steps to reproduce
- Affected platform/version or commit
- Potential impact
- Suggested fix, if available

## Secret Handling

- Never commit API keys, tokens, credentials, or private config.
- Keep local environment/config files out of version control.
- Do not hard-code AI provider credentials in Rust, UI, or config source.
- Redact secrets from logs, crash reports, screenshots, and issue reports.
- Prefer secure runtime configuration and OS-native secret storage for sensitive values.

## Desktop Permissions

Future desktop integrations should request only the minimum permissions needed for a workflow. Any feature that reads files, observes app state, automates input, or sends context to an AI provider should be explicit, reviewable, and user-controlled.

## AI Data Handling

Future AI integrations should clearly define:

- What user data is sent to AI providers
- Which provider processes each request
- Whether local models or remote APIs are used
- How conversation data is stored locally
- How users can delete stored data
- How sensitive context is filtered before model calls
