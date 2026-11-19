import { test } from 'node:test';
import assert from 'node:assert/strict';
import { MAX_CHARS, MAX_INLINE, parseMarkdown, type Block, type Inline } from './markdown.ts';

const t = (text: string): Inline => ({ type: 'text', text });
const p = (...content: Inline[]): Block => ({ type: 'paragraph', content });
const item = (...blocks: Block[]) => ({ checked: null, blocks });

// The inline content of a text that is one paragraph.
function inline(src: string): Inline[] {
  const blocks = parseMarkdown(src);
  assert.equal(blocks.length, 1);
  const b = blocks[0];
  if (b.type !== 'paragraph') throw new Error(`a ${b.type}, not a paragraph`);
  return b.content;
}

function inlineDepth(nodes: Inline[]): number {
  let max = 0;
  for (const n of nodes) if ('children' in n) max = Math.max(max, 1 + inlineDepth(n.children));
  return max;
}

test('headings have levels 1 to 6 and lose a closing run of #', () => {
  assert.deepEqual(parseMarkdown('# One\n## Two ##\n###### Six\n####### seven\n#tag\n### a#'), [
    { type: 'heading', level: 1, content: [t('One')] },
    { type: 'heading', level: 2, content: [t('Two')] },
    { type: 'heading', level: 6, content: [t('Six')] },
    p(t('####### seven #tag')),
    { type: 'heading', level: 3, content: [t('a#')] },
  ]);
});

test('a paragraph joins its lines, and two spaces or a backslash give a hard break', () => {
  assert.deepEqual(parseMarkdown('one\ntwo\n\nthree'), [p(t('one two')), p(t('three'))]);
  assert.deepEqual(inline('a  \nb'), [t('a'), { type: 'br' }, t('b')]);
  assert.deepEqual(inline('a\\\nb'), [t('a'), { type: 'br' }, t('b')]);
  assert.deepEqual(inline('a \nb  '), [t('a b')]);
});

test('lists keep their marker kind, the start number and the items', () => {
  assert.deepEqual(parseMarkdown('- a\n* b\n+ c\n\n3. x\n4) y'), [
    { type: 'list', ordered: false, start: 1, items: [item(p(t('a'))), item(p(t('b'))), item(p(t('c')))] },
    { type: 'list', ordered: true, start: 3, items: [item(p(t('x'))), item(p(t('y')))] },
  ]);
});

test('a list nests by indent and an item holds more lines', () => {
  assert.deepEqual(parseMarkdown('- a\n  more\n  - b\n    - c\n- d'), [
    {
      type: 'list',
      ordered: false,
      start: 1,
      items: [
        item(p(t('a more')), {
          type: 'list',
          ordered: false,
          start: 1,
          items: [item(p(t('b')), { type: 'list', ordered: false, start: 1, items: [item(p(t('c')))] })],
        }),
        item(p(t('d'))),
      ],
    },
  ]);
});

test('task items have a checked state', () => {
  assert.deepEqual(parseMarkdown('- [ ] todo\n- [x] done\n- [X] Done\n- [y] no'), [
    {
      type: 'list',
      ordered: false,
      start: 1,
      items: [
        { checked: false, blocks: [p(t('todo'))] },
        { checked: true, blocks: [p(t('done'))] },
        { checked: true, blocks: [p(t('Done'))] },
        item(p(t('[y] no'))),
      ],
    },
  ]);
});

test('a table has alignment, escaped pipes and padded rows', () => {
  assert.deepEqual(parseMarkdown('| a | b | c | d |\n| :--- | :---: | ---: | --- |\n| 1 | **2** | 3 \\| x |\nafter'), [
    {
      type: 'table',
      align: ['left', 'center', 'right', null],
      head: [[t('a')], [t('b')], [t('c')], [t('d')]],
      rows: [
        [[t('1')], [{ type: 'strong', children: [t('2')] }], [t('3 | x')], []],
        [[t('after')], [], [], []],
      ],
    },
  ]);
  assert.deepEqual(parseMarkdown('a | b\n--- | ---\n\nc'), [
    { type: 'table', align: [null, null], head: [[t('a')], [t('b')]], rows: [] },
    p(t('c')),
  ]);
  assert.deepEqual(parseMarkdown('| a | b |\n| --- |'), [p(t('| a | b | | --- |'))]);
});

test('a fenced code block keeps its text and the language word', () => {
  assert.deepEqual(parseMarkdown('```rust extra\nlet a = *b*;\n\n<b>\n```\nafter'), [
    { type: 'code', lang: 'rust', text: 'let a = *b*;\n\n<b>' },
    p(t('after')),
  ]);
  assert.deepEqual(parseMarkdown('~~~~\na\n~~~\n~~~~~'), [{ type: 'code', lang: '', text: 'a\n~~~' }]);
});

test('an unclosed fence runs to the end of the text', () => {
  assert.deepEqual(parseMarkdown('para\n```\n# not a heading\n\n- x'), [
    p(t('para')),
    { type: 'code', lang: '', text: '# not a heading\n\n- x' },
  ]);
});

test('a blockquote holds other blocks', () => {
  assert.deepEqual(parseMarkdown('> # T\n> - a\n>\n> text\n\nout'), [
    {
      type: 'quote',
      blocks: [
        { type: 'heading', level: 1, content: [t('T')] },
        { type: 'list', ordered: false, start: 1, items: [item(p(t('a')))] },
        p(t('text')),
      ],
    },
    p(t('out')),
  ]);
});

test('horizontal rules have three or more marks of one kind', () => {
  assert.deepEqual(parseMarkdown('---\n***\n___\n- - -\n\n--\n\n-*-'), [
    { type: 'rule' },
    { type: 'rule' },
    { type: 'rule' },
    { type: 'rule' },
    p(t('--')),
    p(t('-*-')),
  ]);
});

test('inline marks nest', () => {
  assert.deepEqual(inline('***a*** **b _c_ d** __e__ ~~f~~ ~g~ snake_case_word'), [
    { type: 'em', children: [{ type: 'strong', children: [t('a')] }] },
    t(' '),
    { type: 'strong', children: [t('b '), { type: 'em', children: [t('c')] }, t(' d')] },
    t(' '),
    { type: 'strong', children: [t('e')] },
    t(' '),
    { type: 'del', children: [t('f')] },
    t(' '),
    { type: 'del', children: [t('g')] },
    t(' snake_case_word'),
  ]);
  assert.deepEqual(inline('**a *b* c** * no * ~~~x~~~'), [
    { type: 'strong', children: [t('a '), { type: 'em', children: [t('b')] }, t(' c')] },
    t(' * no * ~~~x~~~'),
  ]);
});

test('code spans keep their text literal', () => {
  assert.deepEqual(inline('`a*b*` `` c`d `` `open'), [
    { type: 'code', text: 'a*b*' },
    t(' '),
    { type: 'code', text: 'c`d' },
    t(' `open'),
  ]);
});

test('links and images keep the target as data', () => {
  assert.deepEqual(inline('[a *b*](http://x.y/(1) "title") ![alt *t*](<i m.png>)'), [
    { type: 'link', url: 'http://x.y/(1)', children: [t('a '), { type: 'em', children: [t('b')] }] },
    t(' '),
    { type: 'image', src: 'i m.png', alt: 'alt t' },
  ]);
  assert.deepEqual(inline('[a [b](u)](v) [no] [x](y'), [
    t('[a '),
    { type: 'link', url: 'u', children: [t('b')] },
    t('](v) [no] [x](y'),
  ]);
  assert.deepEqual(inline('*[a*](u)'), [t('*'), { type: 'link', url: 'u', children: [t('a*')] }]);
});

test('backslash escapes make marks literal', () => {
  assert.deepEqual(inline('\\*a\\* \\[b\\](c) \\\\ \\q \\`d`'), [t('*a* [b](c) \\ \\q `d`')]);
});

test('raw HTML stays text', () => {
  assert.deepEqual(parseMarkdown('<b>x</b> <script>alert(1)</script>\n<img src=x onerror=y>'), [
    p(t('<b>x</b> <script>alert(1)</script> <img src=x onerror=y>')),
  ]);
});

test('lists and quotes nest 4 deep, and deeper markers stay text', () => {
  let quote = parseMarkdown('> > > > > deep');
  for (let k = 0; k < 4; k++) {
    assert.equal(quote.length, 1);
    const q = quote[0];
    if (q.type !== 'quote') throw new Error(`a ${q.type} at depth ${k}`);
    quote = q.blocks;
  }
  assert.deepEqual(quote, [p(t('> deep'))]);

  let list = parseMarkdown('- a\n  - b\n    - c\n      - d\n        - e');
  for (let k = 0; k < 4; k++) {
    const l = list[list.length - 1];
    if (l.type !== 'list') throw new Error(`a ${l.type} at depth ${k}`);
    list = l.items[0].blocks;
  }
  assert.deepEqual(list, [p(t('d - e'))]);
});

test('the parser cuts a long text and limits the inline depth', () => {
  assert.deepEqual(inline('a'.repeat(MAX_CHARS + 50)), [t('a'.repeat(MAX_CHARS))]);
  const deep = inline('*'.repeat(3000) + 'x' + '*'.repeat(3000));
  assert.ok(inlineDepth(deep) <= MAX_INLINE);
  assert.match(JSON.stringify(deep), /"x"/);
});

test('hostile texts parse fast', () => {
  const half = MAX_CHARS / 2;
  const texts = [
    '*a_'.repeat(half),
    '**a *'.repeat(half),
    '[]('.repeat(half),
    '[](a ('.repeat(half),
    '[](<'.repeat(half),
    '[a'.repeat(half) + ']'.repeat(half),
    '`'.repeat(half) + 'x' + '``'.repeat(half),
    '> '.repeat(half),
    '- '.repeat(half),
    '| a '.repeat(half) + '\n' + '|-'.repeat(half),
    'a  \n'.repeat(half),
  ];
  for (const text of texts) {
    const start = performance.now();
    parseMarkdown(text);
    const ms = performance.now() - start;
    assert.ok(ms < 250, `${JSON.stringify(text.slice(0, 12))} took ${ms.toFixed(0)} ms`);
  }
});
