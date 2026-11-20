// The colors of a code field (spec 4.3): the comments, the words, the labels, the numbers and the other names.

/** The comment mark and the keywords that a node gives. */
export interface Syntax {
  comment: string;
  words: string[];
}

export type TokenKind = 'plain' | 'comment' | 'word' | 'label' | 'number' | 'name';

/** A part of the text and its color. The parts in order give the full text. */
export interface Token {
  text: string;
  kind: TokenKind;
}

/** Reads the syntax field of a code widget: the comment mark, then the words, with a space between. */
export function parseSyntax(field: string): Syntax {
  const [comment = '', ...words] = field.split(' ');
  return { comment, words: words.filter(Boolean) };
}

/** Splits `text` into colored parts. A word matches with no case. A label is a name and `:` at the start of a line. */
export function codeTokens(text: string, syntax: Syntax): Token[] {
  const words = new Set(syntax.words.map((w) => w.toUpperCase()));
  const tokens: Token[] = [];
  const push = (part: string, kind: TokenKind) => {
    if (part) tokens.push({ text: part, kind });
  };
  text.split('\n').forEach((line, i) => {
    if (i > 0) push('\n', 'plain');
    const at = syntax.comment ? line.indexOf(syntax.comment) : -1;
    let code = at >= 0 ? line.slice(0, at) : line;
    const label = /^(\s*)([A-Za-z_]\w*\s*:)/.exec(code);
    if (label) {
      push(label[1], 'plain');
      push(label[2], 'label');
      code = code.slice(label[0].length);
    }
    for (const [part, name, number] of code.matchAll(/([A-Za-z_]\w*)|(-?\d\w*)|[^A-Za-z_\d-]+|-/g)) {
      push(part, name ? (words.has(name.toUpperCase()) ? 'word' : 'name') : number ? 'number' : 'plain');
    }
    if (at >= 0) push(line.slice(at), 'comment');
  });
  return tokens;
}
