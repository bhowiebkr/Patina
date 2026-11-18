// The highlight of Cargo.toml and the other TOML files in the Code tab (15.10). Monaco has no TOML.

import * as monaco from 'monaco-editor/editor/editor.api';

monaco.languages.register({ id: 'toml', extensions: ['.toml'], aliases: ['TOML'] });

monaco.languages.setLanguageConfiguration('toml', {
  comments: { lineComment: '#' },
  brackets: [
    ['[', ']'],
    ['{', '}'],
  ],
  autoClosingPairs: [
    { open: '[', close: ']' },
    { open: '{', close: '}' },
    { open: '"', close: '"', notIn: ['string'] },
    { open: "'", close: "'", notIn: ['string'] },
  ],
});

monaco.languages.setMonarchTokensProvider('toml', {
  tokenizer: {
    root: [
      [/#.*$/, 'comment'],
      // A table header, for example [dependencies] or [[bin]].
      [/^\s*\[\[?[^\]]*\]\]?/, 'type'],
      // A key before "=", with dots and quotes.
      [/(?:[A-Za-z0-9_-]+|"[^"]*"|'[^']*')(?:\s*\.\s*(?:[A-Za-z0-9_-]+|"[^"]*"|'[^']*'))*(?=\s*=)/, 'variable'],
      { include: '@value' },
    ],
    value: [
      [/"""/, 'string', '@basicMulti'],
      [/'''/, 'string', '@literalMulti'],
      [/"/, 'string', '@basic'],
      [/'[^']*'/, 'string'],
      [/\b(?:true|false)\b/, 'keyword'],
      [/\d{4}-\d{2}-\d{2}(?:[Tt ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[Zz]|[+-]\d{2}:\d{2})?)?/, 'number'],
      [/[+-]?(?:0x[0-9A-Fa-f_]+|0o[0-7_]+|0b[01_]+|inf|nan|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?)/, 'number'],
      [/[[\]{},=.]/, 'delimiter'],
    ],
    basic: [
      [/[^"\\]+/, 'string'],
      [/\\./, 'string.escape'],
      [/"/, 'string', '@pop'],
    ],
    basicMulti: [
      [/"""/, 'string', '@pop'],
      [/[^"\\]+/, 'string'],
      [/\\./, 'string.escape'],
      [/"/, 'string'],
    ],
    literalMulti: [
      [/'''/, 'string', '@pop'],
      [/[^']+/, 'string'],
      [/'/, 'string'],
    ],
  },
});
