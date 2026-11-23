# Inb: security checklist

The threat surface of this project and how each part is held. Pair with
`SECURITY.md` (how to report) and the scanners below (what runs automatically).

## Scanners

- CodeQL (`.github/workflows/codeql.yml`): every push and PR to main, weekly.
- OpenSSF Scorecard (`.github/workflows/scorecard.yml`): weekly.
- Dependabot alerts and updates (`.github/dependabot.yml`).
- GitHub secret scanning and push protection: enable in the repo settings.

## S1. Inputs

- [ ] The screen-recording permission is asked for only when recording starts.
- [ ] Inb reads no settings or input files. It writes two places only: a new
      GIF per recording under the temp folder (`Inb/`), and "Inb N.gif" in
      the Desktop folder on Save, never overwriting an existing file.
- [ ] Every `unsafe` block states why it is sound (`// SAFETY:`).

## S2. Credentials and secrets

- [ ] No secrets in the tree; `.claude/settings.json` denies reading `.env*`.
- [ ] Tokens used by CI are repo secrets with least privilege.

## S3. Paths and processes

- [ ] File paths built from input are resolved and checked against a root.
- [ ] No shell interpolation of untrusted strings.

## S4. CI and supply chain

- [ ] Every third-party action is pinned to a commit SHA.
- [ ] Workflows declare minimal `permissions:`; `pull_request_target` jobs never
      check out PR code.

## Open backlog
