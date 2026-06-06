# Security Policy

BLACKOUT is a privacy tool, so security issues matter more than usual.

## Reporting a vulnerability

Please **do not** open a public issue for a security problem. Instead, use GitHub's
private **[Report a vulnerability](../../security/advisories/new)** feature, or email
the maintainer.

Good things to report:
- A file format where metadata **survives** cleaning (with a sample file).
- Any code path that makes a **network call** (there should be none).
- A case where an original file is modified instead of copied.
- A LOCKDOWN/PANIC/OPSEC action that **claims success but does nothing**.

## Scope & honest limits

By design, some things are **not** vulnerabilities — they're documented OS limits:
- Apple Lockdown Mode can't be toggled by any app (the app deep-links instead).
- iOS/macOS can't globally disable camera/mic/location from an app.
- macOS Bluetooth toggling needs the `blueutil` helper.

These are labeled in-app as "not available," not faked. Reports that these "don't
work" will be closed as by-design — but reports that the app *pretends* they work
are exactly what we want to hear about.
