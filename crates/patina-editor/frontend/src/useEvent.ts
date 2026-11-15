// A React hook for the events of the editor process.

import { useEffect, useRef } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';

/** Listens to a Tauri event while the component is mounted. */
export function useEvent<T>(subscribe: (handler: (value: T) => void) => Promise<UnlistenFn>, handler: (value: T) => void) {
  const latest = useRef(handler);
  useEffect(() => {
    latest.current = handler;
  });
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let gone = false;
    subscribe((value) => latest.current(value)).then((f) => (gone ? f() : (unlisten = f)));
    return () => {
      gone = true;
      unlisten?.();
    };
  }, [subscribe]);
}
