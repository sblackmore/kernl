# Security Policy

## Reporting a vulnerability

If you discover a security vulnerability in the kernl compiler or toolchain, please report it responsibly.

**Email:** kernl-lang@users.noreply.github.com

Please include:
1. Description of the vulnerability
2. Steps to reproduce
3. Potential impact
4. Suggested fix (if any)

We will acknowledge your report within 48 hours and aim to provide a fix or mitigation plan within 7 days.

## Scope

The following are in scope for security reports:

- **Compiler bugs** that could cause the compiler to emit unsafe or incorrect code
- **Memory safety issues** in the compiler itself
- **Verification bypasses** where the type checker or verifier fails to catch an invalid program
- **Denial of service** via crafted `.knl` input that causes the compiler to hang or consume excessive resources

## Out of scope

- Issues in third-party dependencies (report these upstream)
- Feature requests or general bugs (use GitHub Issues)
- Social engineering attacks

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Disclosure policy

We follow coordinated disclosure. Once a fix is available, we will publish a security advisory on the repository.
