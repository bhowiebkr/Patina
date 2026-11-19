import { useMemo, type ReactNode } from 'react';
import { parseMarkdown, type Align, type Block, type Inline, type ListItem } from './markdown.ts';

const HEADINGS = ['h1', 'h2', 'h3', 'h4', 'h5', 'h6'] as const;

/** The Markdown of a note as React elements. It never sets HTML, follows a link or loads an image. */
export function Markdown({ text }: { text: string }) {
  const blocks = useMemo(() => parseMarkdown(text), [text]);
  return <div className="md">{blocks.map(block)}</div>;
}

function block(b: Block, key: number): ReactNode {
  switch (b.type) {
    case 'heading': {
      const H = HEADINGS[b.level - 1];
      return <H key={key}>{inline(b.content)}</H>;
    }
    case 'paragraph':
      return <p key={key}>{inline(b.content)}</p>;
    case 'list': {
      const items = b.items.map(item);
      return b.ordered ? (
        <ol key={key} start={b.start === 1 ? undefined : b.start}>
          {items}
        </ol>
      ) : (
        <ul key={key}>{items}</ul>
      );
    }
    case 'table':
      return (
        <table key={key}>
          <thead>
            <tr>
              {b.head.map((cell, k) => (
                <th key={k} style={align(b.align[k])}>
                  {inline(cell)}
                </th>
              ))}
            </tr>
          </thead>
          {b.rows.length > 0 && (
            <tbody>
              {b.rows.map((row, r) => (
                <tr key={r}>
                  {row.map((cell, k) => (
                    <td key={k} style={align(b.align[k])}>
                      {inline(cell)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          )}
        </table>
      );
    case 'code':
      return (
        <pre key={key} data-lang={b.lang || undefined}>
          <code>{b.text}</code>
        </pre>
      );
    case 'quote':
      return <blockquote key={key}>{b.blocks.map(block)}</blockquote>;
    case 'rule':
      return <hr key={key} />;
  }
}

const align = (a: Align) => (a ? { textAlign: a } : undefined);

// The checkbox of a task item goes into its first paragraph, so it stays on the line of the text.
function item(it: ListItem, key: number): ReactNode {
  if (it.checked === null) return <li key={key}>{it.blocks.map(block)}</li>;
  const box = <input type="checkbox" checked={it.checked} disabled readOnly aria-label="Task" />;
  const first = it.blocks[0];
  return (
    <li key={key} className="md-task">
      {first?.type !== 'paragraph' && box}
      {it.blocks.map((b, k) =>
        k === 0 && b.type === 'paragraph' ? (
          <p key={k}>
            {box}
            {inline(b.content)}
          </p>
        ) : (
          block(b, k)
        ),
      )}
    </li>
  );
}

function inline(nodes: Inline[]): ReactNode[] {
  return nodes.map((n, key) => {
    switch (n.type) {
      case 'text':
        return n.text;
      case 'code':
        return <code key={key}>{n.text}</code>;
      case 'br':
        return <br key={key} />;
      case 'em':
        return <em key={key}>{inline(n.children)}</em>;
      case 'strong':
        return <strong key={key}>{inline(n.children)}</strong>;
      case 'del':
        return <del key={key}>{inline(n.children)}</del>;
      case 'link':
        return (
          <span key={key} className="md-link" title={n.url}>
            {inline(n.children)}
          </span>
        );
      case 'image':
        return (
          <span key={key} className="md-image" title={n.src}>
            {n.alt}
          </span>
        );
    }
  });
}
