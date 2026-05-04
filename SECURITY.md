# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in HonkHonk, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please email: **djfreaq@gmail.com**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

## Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 7 days
- **Fix or mitigation**: Depends on severity, targeting 30 days for critical issues

## Scope

This policy covers:
- The HonkHonk application binary
- Build and packaging scripts in this repository
- GitHub Actions workflows

Out of scope:
- Third-party dependencies (report upstream, but let us know so we can track)
- Issues requiring physical access to the machine

## Security Considerations

HonkHonk interacts with:
- **PipeWire** (audio server) — via pipewire-rs bindings
- **D-Bus** (desktop portals) — via ashpd for shortcuts and file dialogs
- **Filesystem** — reads audio files from user-specified directories

The application:
- Does not make network connections
- Does not process untrusted remote input
- Runs entirely in userspace with no elevated privileges
- Stores configuration in XDG-compliant directories only
