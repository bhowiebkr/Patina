# GUI image handoff

Claude Code directs the GUI image work. Codex generates and reviews the images.
Both agents use this folder in the same checkout. No service or install is needed.

Owner direction (2026-11-14): generate one image per request. Do not generate extra
revisions or variants unless the owner asks. The owner also highlighted
category-colored node headers as a positive reference. Use
[the GUI visual guide](../../docs/gui-visual-guide.md) for the current palette,
content-driven node sizes, shared controls and image corrections.

## Send a request

1. Read [the specification](../../docs/patina-spec.md), especially sections 0 and 15,
   and [the GUI visual guide](../../docs/gui-visual-guide.md).
2. Copy [request-template.md](request-template.md) into [requests/](requests/).
3. Give it a unique name, such as `0001-main-editor.draft`.
4. Fill in the brief. Use paths relative to the project root.
5. Rename the finished file to `0001-main-editor.md`.
6. Read `responses/0001-main-editor.md` for progress and the result.

Only `.md` files in `requests/` are submitted requests. Process them in filename
order unless Claude marks a different priority in the brief. Claude writes the
requests. Codex writes the responses. Neither agent edits the other's files.

Keep a submitted brief unchanged while Codex works on it. If Codex asks for more
information, append an **Answers** section to that request. For a revision after
delivery, submit a new request ID and name the previous request and image.

## Return an image

1. Read the submitted brief and its reference images.
2. Write a response with **Status: in-progress** before generating.
3. Use the built-in image generation tool for raster mockups.
4. Save the image in `docs/mockups/` at the requested path.
5. Inspect the saved image against the brief and the specification.
6. Update the response with the result.

Each response includes:

- **Status:** `in-progress`, `needs-input`, `done`, or `blocked`.
- **Updated:** date and time with a timezone offset.
- **Request:** the request ID.
- **Files:** project-relative paths to the saved images.
- **Generation:** the tool used and the final prompt, including any edit prompts.
- **Checked:** what was actually inspected, including actual image dimensions.
- **Limits:** text errors, missed details, or requirements that remain unmet.
- **Next:** questions or a suggested next step, when needed.

Use `done` only after the requested files exist and have been inspected. A mockup
is a design reference. It does not prove that the application works. If the tool
fails, report the failure. Do not silently switch to a paid API or another service.

## Visual direction and ownership

- Match [the GUI visual guide](../../docs/gui-visual-guide.md). Use mockup 9 for
  theme direction, neutral charcoal palette tokens, and the original blue logo.
- Also use [the error reference](../../docs/mockups/mockup-3-error-view.png) when relevant.
- Follow the spec when a reference differs. Section 0 lists known differences.
- Show **Base** and **Slot** as compile layers. Use separate build and run times.
- Use Patina node-kind icons. Keep state colors separate from selection color.
- Leave section 18 decisions to the owner.
- Preserve existing images unless the owner requests replacement or removal.
  Use new names such as `docs/mockups/0001-main-editor-v1.png` for new deliverables.
- Codex owns only this handoff and the requested images. Claude owns application
  changes. Coordinate before editing a shared file.
- Keep each agent's unrelated changes. Follow `AGENTS.md` for checks and commits.
- Fresh Codex image workers read the playbook in `G:/codex-usage-pacing.md` and
  receive the request ID as their task brief.

## When messages are read

This is a file handoff, not a live connection to either agent. A file change does
not wake an idle chat. An already-running Claude session may need to be told to
read this file once; `CLAUDE.md` links here for future sessions.

When active on image work, Codex checks `requests/` before starting and after each
delivery. Claude checks `responses/` before work that needs an image. If an agent
has ended its turn, prompt that agent to check the handoff. Do not assume that a
request has been seen until its response exists.

The [initial response](responses/0000-ready.md) records setup and verification.
