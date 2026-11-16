// The float windows of the view panels (spec 15.8). A float window shows one view slot. The editor
// window sends it the state of the project. It sends the writes and the selections of its view
// back. The Rust side opens and closes the windows.

import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { reportError, type View } from './api';
import { changeLayout } from './Dock';
import { dockPanel, closePanel, moveFloat, VIEW_SLOTS, type Rect, type ViewSlot } from './layout';
import { useEvent } from './useEvent';
import { ViewPanel } from './ViewPanel';

/** The state of the project that a float window needs. */
interface FloatState {
  trusted: boolean;
  revision: number;
  views: string[];
  selected: string[];
}

/** A write or a selection of the view in a float window. */
interface FloatFrame {
  view?: View;
  select?: string[];
}

const on =
  <T,>(name: string) =>
  (handler: (value: T) => void): Promise<UnlistenFn> =>
    listen<T>(name, (e) => handler(e.payload));
const onState = on<FloatState>('float-state');
const onReady = on<string>('float-ready');
const onFrame = on<FloatFrame>('float-frame');
const onDock = on<string>('float-dock');
const onClosed = on<string>('float-closed');
const onMoved = on<{ slot: string; rect: Rect }>('float-moved');

const send = (name: string, payload: unknown) => emit(name, payload).catch(reportError);
const isSlot = (v: string): v is ViewSlot => VIEW_SLOTS.includes(v as ViewSlot);

/**
 * The editor window side. Sends the state of the project to the float windows, shows their writes
 * and selections, and does their layout changes: Dock, a close and a move. Returns a function that
 * sends the state again, for a new selection.
 */
export function useFloatHost(view: View | null, selected: RefObject<string[]>, show: (v: View) => void, select: (ids: string[]) => void) {
  const state = useRef(view);
  state.current = view;
  const sendState = useCallback(() => {
    const v = state.current;
    if (v) send('float-state', { trusted: v.trusted, revision: v.codeRevision, views: v.views, selected: selected.current });
  }, [selected]);
  const key = view && `${view.trusted} ${view.codeRevision} ${view.views.join('\n')}`;
  useEffect(() => {
    sendState();
  }, [key, sendState]);
  useEvent(onReady, sendState);
  useEvent(onFrame, (f) => {
    if (f.view) show(f.view);
    if (f.select) select(f.select);
  });
  useEvent(onDock, (slot) => isSlot(slot) && changeLayout((l) => dockPanel(l, slot)));
  useEvent(onClosed, (slot) => isSlot(slot) && changeLayout((l) => closePanel(l, slot)));
  useEvent(onMoved, ({ slot, rect }) => isSlot(slot) && changeLayout((l) => moveFloat(l, slot, rect)));
  return sendState;
}

/** The page of a float window: the view of one slot, and Dock. */
export function FloatView({ slot, name }: { slot: string; name: string }) {
  const [state, setState] = useState<FloatState | null>(null);
  const selected = useRef<string[]>([]);
  useEvent(onState, (s) => {
    selected.current = s.selected;
    setState(s);
  });
  useEffect(() => {
    send('float-ready', slot);
  }, [slot]);
  const onView = useCallback((view: View) => send('float-frame', { view }), []);
  const onSelect = useCallback((ids: string[]) => send('float-frame', { select: ids }), []);
  return (
    <div className="float-shell">
      <header className="topbar">
        <b>{name.replace(/^views\//, '')}</b>
        <span className="spacer" />
        <button className="btn" title="Move the view back into the editor window" onClick={() => send('float-dock', slot)}>
          Dock
        </button>
      </header>
      {state ? (
        <ViewPanel name={state.views.includes(name) ? name : null} instance={slot} revision={state.revision} trusted={state.trusted} onView={onView} selected={selected} onSelect={onSelect} />
      ) : (
        <div className="item empty">Waiting for the editor window</div>
      )}
    </div>
  );
}
