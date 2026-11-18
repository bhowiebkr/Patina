// The list of common errors and their fixes (15.8). The Logs show a "Fix:" line after each error
// line that a rule matches. Add a rule here for each error that users often meet.

interface Hint {
  match: RegExp;
  hint: string;
}

const HINTS: Hint[] = [
  { match: /has no input `?([\w-]+)`? for the parameter|lost parameter/, hint: 'Click the x of the crossed-out parameter on the node to delete it, or add the input to the node again. Undo restores it.' },
  { match: /has no pin|no such pin/, hint: 'Delete the wire to the crossed-out pin, or give the node the pin again in its code.' },
  { match: /error\[E0425\]|cannot find value|cannot find function/, hint: 'Check the spelling of the name. Add a `use` line for an item of another module or crate.' },
  { match: /error\[E043[23]\]|unresolved import|failed to resolve|use of unresolved module|unlinked crate/, hint: 'Add the crate with Add crate on the Code tab, or correct the path of the `use` line.' },
  { match: /error\[E0308\]|mismatched types/, hint: 'The value has another type than the code expects. Convert it, for example with `as f32`, `.into()` or `.to_string()`.' },
  { match: /error\[E0599\]|no method named/, hint: 'Check the method name. A trait method needs a `use` line for its trait.' },
  { match: /error\[E0382\]|use of moved value|borrow of moved value/, hint: 'Take a reference (`&value`), or call `.clone()` before the value moves.' },
  { match: /error\[E0502\]|error\[E0499\]|cannot borrow .* as mutable/, hint: 'End the first borrow before the second one, for example with a smaller block or a copy of the value.' },
  { match: /error\[E0277\]|the trait bound .* is not satisfied|doesn't implement/, hint: 'The type needs this trait. Derive it, for example `#[derive(Clone, Default)]`, or use another type.' },
  { match: /error\[E0061\]|takes \d+ arguments? but \d+ arguments? (?:was|were) supplied/, hint: 'Give the function the number of arguments that its definition has.' },
  { match: /expected one of|expected `;`|unexpected closing delimiter|this file contains an unclosed delimiter/, hint: 'Look for a missing `;`, `,`, `)` or `}` at the line of the error or the line before it.' },
  { match: /gives the type of T|the wires give two types|A cycle of generic nodes/, hint: 'Connect a wire that gives the type of the generic input, or give all wires to the node the same type.' },
  { match: /domain to the .* domain|declares the \w+ domain|must name its clock domain/, hint: 'Add `domain = frame` (or batch, audio, event) to `#[node(...)]`, or set the domain of the node use in the Inspector.' },
  { match: /linker .* not found|could not find .*rustc|rustup|toolchain is not installed|can't find crate for `std`/, hint: 'Open Settings and click Install toolchain.' },
  { match: /cannot read Cargo\.toml|failed to parse manifest/, hint: 'Open the Cargo.toml of the folder in the Code tab and correct the line of the error.' },
];

/** The fix for an error text, or null when no rule matches. */
export function hintOf(text: string): string | null {
  return HINTS.find((h) => h.match.test(text))?.hint ?? null;
}
