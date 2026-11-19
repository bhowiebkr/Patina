// The keys of the editor outside the text fields (15.10).
import { useEffect, type RefObject } from 'react';
import type { At, CanvasActions } from './canvasActions';

/** True when the key goes to a text field, so the keys of the editor do not apply. */
export function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
}

/** Ctrl+S saves all, Ctrl+Z undoes, and Ctrl+Y or Ctrl+Shift+Z redoes, while a project is open. */
export function useEditKeys(open: boolean, save: () => void, undo: () => void, redo: () => void) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (typing(e.target) || !e.ctrlKey || e.altKey || document.querySelector('.backdrop')) return;
      const key = e.key.toLowerCase();
      if (key === 's') save();
      else if (key === 'z' && !e.shiftKey) undo();
      else if (key === 'y' || (key === 'z' && e.shiftKey)) redo();
      else return;
      e.preventDefault();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [open, save, undo, redo]);
}

/** The keys of the canvas (15.2). They act on the selection. Shift+A opens the Add node palette, and Shift+G and Shift+N add a group and a note (15.12). */
export function useCanvasKeys(act: CanvasActions, trusted: boolean, pointer: RefObject<At | null>, addAt: (at: At) => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // A view in the center covers the graph, so the keys of the graph do not act (15.2).
      if (typing(e.target) || e.altKey || e.metaKey || document.querySelector('.backdrop, .menu-layer, .center-view')) return;
      const key = e.key.toLowerCase();
      // Ctrl+A, C, X and V act on the nodes only over the canvas, so a panel keeps its own keys.
      const onCanvas = !!pointer.current || (e.target instanceof Element && !!e.target.closest('.react-flow'));
      if (e.ctrlKey && !e.shiftKey && key === 'a' && onCanvas) act.selectAll();
      else if (e.ctrlKey && key === 'g' && trusted && (e.shiftKey ? act.uses : act.patina).length > 0) {
        if (e.shiftKey) act.expand();
        else act.collapse(act.middle());
      } else if (e.ctrlKey && !e.shiftKey && ['c', 'x', 'v'].includes(key) && onCanvas) {
        if (key === 'c') act.copy();
        else if (!trusted) return;
        else if (key === 'x') act.cut();
        else act.paste(act.middle());
      } else if (e.ctrlKey) return;
      else if (e.key === 'Delete' && trusted) act.remove();
      else if (key === 'f') act.fit();
      else if (e.shiftKey && key === 'l' && trusted) act.arrange();
      else if (e.shiftKey && key === 'a' && trusted) addAt(act.middle());
      else if (e.shiftKey && key === 'g' && trusted) act.addGroup(act.middle(), act.selected.length > 0);
      else if (e.shiftKey && key === 'n' && trusted) act.addNote(act.middle());
      else if (e.key === 'Escape') act.clear();
      else return;
      e.preventDefault();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [act, trusted, pointer, addAt]);
}
