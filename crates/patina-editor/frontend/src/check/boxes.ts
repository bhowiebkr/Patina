// The box rules of the shot that do not need a page.

type Box = { left: number; top: number; right: number; bottom: number; height: number };

/** True when two text boxes share more than 2 px of width and half of the lower box height, so the glyphs touch. Two lines of text can share a few px of their boxes. */
export function textsMeet(a: Box, b: Box) {
  const width = Math.min(a.right, b.right) - Math.max(a.left, b.left);
  const height = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
  return width > 2 && height > Math.min(a.height, b.height) / 2;
}
