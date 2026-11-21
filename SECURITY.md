# Security policy

## Reporting a vulnerability

Please coordinate disclosure privately before publishing exploit details. On the AraBrief GitHub repository, open **Security → Advisories** and use **Report a vulnerability** if that option is available. Follow [GitHub's private reporting instructions](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately).

Private reporting must be enabled separately in GitHub; this file does not enable it. Its availability has not been confirmed for this repository. If the reporting option is unavailable and no verified private contact is published, open a public issue containing only a request for a private security reporting channel. Do not include the affected component, exploit, secret, or user data in that request. Wait for the maintainer to provide a private route before sending details. No private email address is designated here.

In a private report, include the affected version/commit, Windows environment, impact, reproduction steps, and a minimal synthetic proof of concept. Explain any required local access, user interaction, settings, or third-party service conditions. Share only redacted diagnostics; never send a working API key or another person's data.

## Scope and coordination

Reports about credential handling, IPC validation, network destinations, local-file access, untrusted article content, dependencies, and unsafe audio/lifecycle behavior are relevant. Test only on systems, accounts, and data you control or are authorized to assess. This policy does not authorize testing OpenAI, Google, publishers, GitHub, or Microsoft services.

Allow time to reproduce, assess, and prepare a fix before public disclosure. The maintainer and reporter should agree on disclosure timing privately. There is no guaranteed response time, bounty, security certification, or long-term support commitment.

## Version information and limitations

`v0.1.0` is the current published MVP. Please identify whether a report affects that release, the default branch, or both. This policy does not promise maintained security backports for older versions.

Known operational limitations are documented in [Troubleshooting](docs/troubleshooting.md), and data handling in [Privacy](docs/privacy.md). Current configuration has no explicit CSP (`csp: null`) and disables the WebView2 `msSmartScreenProtection` feature through browser arguments; it should not be presented as a hardened sandbox. These settings are documented here without changing application behavior. Local SQLite/audio files are not application-encrypted, and forced process termination can defer music-volume recovery until restart.

If an API key is exposed, revoke or rotate it in the OpenAI account. Deleting it locally or removing a public message does not invalidate the credential.
