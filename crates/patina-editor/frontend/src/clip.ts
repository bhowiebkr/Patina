// The clipboard of copy and paste (spec 15.11). The clip is text, so two editors share it.

let memory: string | null = null;

/** The self-check keeps the clip in memory, so it never changes the clipboard of the user. */
export function keepClipInMemory() {
  memory ??= '';
}

export const clipboard = {
  write: async (text: string) => {
    if (memory === null) await navigator.clipboard.writeText(text);
    else memory = text;
  },
  read: async () => memory ?? (await navigator.clipboard.readText()),
};
