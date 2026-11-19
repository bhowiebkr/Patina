// The groups and the notes of the canvas (spec 15.12).
import { createContext, memo, useContext, useRef, type CSSProperties, type KeyboardEvent } from 'react';
import { NodeResizeControl, NodeResizer, type NodeProps } from '@xyflow/react';
import type { Group, Note } from '../api';
import { Markdown } from './MarkdownView';
import { GROUP_MIN, GROUP_RGB, MAX_TEXT, MAX_TITLE, NOTE_MAX, NOTE_MIN, type GroupBoxNode, type NoteNode } from './groups';

export interface ItemTools {
  /** The canvas ID of the group or the note in an edit in place. */
  editing: string | null;
  setEditing: (id: string | null) => void;
  /** Sets a part of a group or a note. A place or a size is in the view. */
  change: (id: string, change: Partial<Group & Note>) => void;
}

export const ItemContext = createContext<ItemTools>({ editing: null, setEditing: () => {}, change: () => {} });

/** A text field in place. Enter (Ctrl+Enter in a text area) or a click out keeps the text, and Esc cancels. */
function InPlace({ text, multi, label, onDone }: { text: string; multi: boolean; label: string; onDone: (text: string | null) => void }) {
  const done = useRef(false);
  const finish = (value: string | null) => {
    if (done.current) return;
    done.current = true;
    onDone(value);
  };
  const props = {
    className: 'item-field nodrag nowheel nopan',
    defaultValue: text,
    maxLength: multi ? MAX_TEXT : MAX_TITLE,
    autoFocus: true,
    'aria-label': label,
    onBlur: (e: { currentTarget: { value: string } }) => finish(e.currentTarget.value),
    onKeyDown: (e: KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
      if (e.key === 'Escape') finish(null);
      else if (e.key === 'Enter' && (!multi || e.ctrlKey)) {
        e.preventDefault();
        finish(e.currentTarget.value);
      }
    },
  };
  return multi ? <textarea {...props} rows={Math.max(4, text.split('\n').length + 1)} /> : <input {...props} onFocus={(e) => e.currentTarget.select()} />;
}

function GroupBoxImpl({ id, data, selected }: NodeProps<GroupBoxNode>) {
  const items = useContext(ItemContext);
  const { group, editable } = data;
  const editing = editable && items.editing === id;
  const done = (title: string | null) => {
    items.setEditing(null);
    if (title !== null && title !== group.title) items.change(id, { title });
  };
  return (
    <div className="group-box" style={{ '--group': GROUP_RGB[group.color] } as CSSProperties}>
      <NodeResizer
        isVisible={!!selected && editable}
        minWidth={GROUP_MIN[0]}
        minHeight={GROUP_MIN[1]}
        handleClassName="group-handle"
        lineClassName="group-line"
        onResizeEnd={(_, p) => items.change(id, { pos: [p.x, p.y], size: [p.width, p.height] })}
      />
      <div className="group-title" title={editing ? undefined : group.title}>
        {editing ? <InPlace text={group.title} multi={false} label="Group title" onDone={done} /> : group.title}
      </div>
    </div>
  );
}

function NoteBoxImpl({ id, data, selected }: NodeProps<NoteNode>) {
  const items = useContext(ItemContext);
  const { note, editable } = data;
  const editing = editable && items.editing === id;
  const done = (value: string | null) => {
    items.setEditing(null);
    // The file keeps line feeds only, so a pasted carriage return goes.
    const text = value?.replace(/\r/g, '');
    if (text !== undefined && text !== note.text) items.change(id, { text });
  };
  return (
    <div className={editing ? 'note-box editing' : 'note-box'}>
      {editing ? (
        <InPlace text={note.text} multi label="Note text" onDone={done} />
      ) : note.text.trim() ? (
        <Markdown text={note.text} />
      ) : (
        <span className="note-empty">Empty note</span>
      )}
      {editable && selected && !editing && (
        <NodeResizeControl position="right" resizeDirection="horizontal" minWidth={NOTE_MIN} maxWidth={NOTE_MAX} className="note-handle" onResizeEnd={(_, p) => items.change(id, { width: p.width })} />
      )}
    </div>
  );
}

export const GroupBox = memo(GroupBoxImpl);
export const NoteBox = memo(NoteBoxImpl);
