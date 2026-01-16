# Security Policy

## Supported Versions

The following versions of outlier are currently supported with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| < 0.2.0 | :x:                |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly.

### How to Report

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please use [GitHub's private vulnerability reporting](https://github.com/wingnut128/outlier/security/advisories/new) to submit your report.

Include the following information in your report:

- Type of vulnerability (e.g., buffer overflow, SQL injection, cross-site scripting)
- Full paths of source file(s) related to the vulnerability
- Location of the affected source code (tag/branch/commit or direct URL)
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if possible)
- Impact of the issue, including how an attacker might exploit it

### What to Expect

- **Initial Response**: We will acknowledge receipt of your vulnerability report within 48 hours.
- **Status Updates**: We will provide updates on the progress of addressing the vulnerability at least every 5 business days.
- **Resolution**: We aim to resolve critical vulnerabilities within 30 days of the initial report.

### Disclosure Policy

- We will work with you to understand and resolve the issue quickly.
- We will keep you informed of our progress.
- We will credit you in our release notes (unless you prefer to remain anonymous).
- We request that you give us reasonable time to address the issue before public disclosure.

## Security Best Practices

When using outlier:

- Keep your installation up to date with the latest version
- Review the CHANGELOG.md for security-related updates
- If using the API server mode, ensure it's properly secured behind a firewall or reverse proxy in production environments
- Never expose the API server directly to the public internet without proper authentication

## Security Features

- **No network access by default**: CLI mode operates entirely locally
- **No data persistence**: outlier does not store or cache any data
- **Minimal dependencies**: We keep dependencies minimal and regularly audited
