// The keys of the editor outside the text fields (15.10).
import { useEffect } from 'react';

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
