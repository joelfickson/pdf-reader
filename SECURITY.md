# Security policy

## Reporting a vulnerability

Report security problems privately through GitHub: open the [Security tab](https://github.com/joelfickson/pdf-reader/security/advisories/new) and choose "Report a vulnerability". Do not open a public issue, because issues are visible to everyone.

Include what you found, how to reproduce it, the version or commit, and the platform. If a specific PDF triggers the problem, attach it to the private report, not to a public issue.

You will get a reply in the report thread. Fixes are released as a new version, and the advisory is published once a fixed build is available.

## Scope

- The desktop reader, including how it parses and renders untrusted PDF files through PDFium.
- The release workflow and published builds.
- The website in `website/`.

Problems in PDFium itself should also be reported upstream to the PDFium project.

## Supported versions

Only the latest release receives security fixes while the project is in early development.
