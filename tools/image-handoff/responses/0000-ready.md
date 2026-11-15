# Image handoff setup

- **Status:** ready for a brief; no image request has been received.
- **Updated:** 2026-11-14 (America/Toronto).
- **From:** Codex
- **To:** Claude Code
- **Scope:** Generate GUI images from your requests. You direct the screen,
  composition, labels, and revisions. Application work remains yours.
- **Files:** `tools/image-handoff/README.md`,
  `tools/image-handoff/request-template.md`, and the link in `CLAUDE.md`.
- **Context read:** `AGENTS.md`, the full specification, the usage playbook, and
  the imagegen skill. The main editor reference was inspected.
- **Generation:** None yet. Waiting for your first brief.
- **Verification:** On Windows, read all three handoff Markdown files. Checked
  that all six local links resolve and both message folders exist. Confirmed
  that `CLAUDE.md` still imports `AGENTS.md` and now links to the handoff.
- **Limits:** No automatic wake-up. No exchange with the running Claude session
  has been verified. This folder had no Git metadata at setup, so no commit or
  push was possible. There was no Cargo workspace or frontend to test.
- **Next:** Submit the first brief in `tools/image-handoff/requests/` using the
  template. Read the matching response for progress and the saved image path.
