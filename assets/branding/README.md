# Patina branding: Aged Dovetail

The owner selected **Aged Dovetail**, concept 03 of logo contact sheet 04, on 2026-11-15.
The pieces in this folder come from branding sheet 01. The sheets were removed on 2026-11-20.
The git history keeps them.

## The pieces

The pieces are crops of the 1536 x 1024 sheet. [crops.json](crops.json) has the crop of each
piece. The extraction cleared alpha values of 2 and less, because the sheet has alpha 1 and 2
noise in the gutters. It removed islands smaller than 30 pixels.

| File | Size (px) | Use |
|---|---|---|
| `emblem.png` | 402 x 381 | The large detailed emblem. The 256 px icon. |
| `icon-medium.png` | 229 x 210 | The icons of 64 to 128 px. |
| `icon-compact.png` | 161 x 153 | The icons of 16 to 48 px. The installer banner. |
| `icon-tiny.png` | 86 x 84 | Not used. Its dark steel has low contrast on charcoal. |
| `mark-light.png`, `mark-dark.png` | 146 x 132, 141 x 132 | Flat marks for one-color use. Not used yet. |
| `project-file.png` | 207 x 239 | The project-file symbol. |
| `banner.png` | 807 x 188 | The **About** dialog. |
| `splash.png` | 699 x 270 | The start screen. |
| `installer-side.png` | 125 x 316 | The installer dialog bitmap. |
| `title-accent.png` | 430 x 86 | The top bar. |

The artwork has no text. The application draws each name, version and label.
The wide pieces (title accent, banner and splash) fade to the right. The page draws the name
on the fade, with a dark text shadow, so the artwork and the name read as one mark.

## Exports

- `icons/app-<n>.png` and `icons/project-file-<n>.png` for n = 16, 20, 24, 32, 40, 48, 64, 96,
  128 and 256. Each is square, with the piece centered and a margin of 3%.
- `crates/patina-editor/icons/icon.ico` holds the ten app sizes. Windows uses 16, 20, 24 and
  32 px for the title bar and the taskbar at 100%, 125%, 150% and 200% scale.
- `crates/patina-editor/icons/icon.png` is the 256 px app icon.
- `crates/patina-editor/icons/project-file.ico` holds the ten project-file sizes. No file type
  uses it yet.
- `crates/patina-editor/icons/wix-banner.bmp` (493 x 58) and `wix-dialog.bmp` (493 x 312) are
  the bitmaps of the `.msi`. WiX writes black text on them, so the text areas stay white.
- `crates/patina-editor/frontend/public/brand/` has copies of `title-accent.png`, `splash.png`
  and `banner.png` for the page.

## Resolution limits

- The emblem is 402 px wide, so 256 px is the largest sharp icon.
- The splash shows at a height of 160 px. It stays sharp up to about 170% display scale.
- The banner shows at a height of 90 px in the **About** dialog. It stays sharp up to about 210%.
- The title accent shows at a height of 36 px. It stays sharp up to about 240%.
- The project-file symbol has no simplified small treatment. Below 32 px it loses its detail.
- The installer side art is 125 x 316 px. It is made smaller to fit the 164 x 312 px area.
