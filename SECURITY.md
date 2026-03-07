# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a Vulnerability

If you discover a security vulnerability in pick, please report it responsibly.

**Do not open a public issue.** Instead, use [GitHub's private vulnerability reporting](https://github.com/aryanbhosale/pick/security/advisories/new) with:

- A description of the vulnerability
- Steps to reproduce
- Affected versions
- Any suggested fix (optional)

You can expect an initial response within 72 hours. Once confirmed, a fix will be prioritized and released as soon as possible.

## Scope

pick processes untrusted input (piped data, files). Relevant security concerns include:

- Memory exhaustion from large inputs (mitigated: 100 MB input limit)
- Parsing vulnerabilities in format parsers
- Path traversal via the `--file` flag (mitigated: standard filesystem permissions)
