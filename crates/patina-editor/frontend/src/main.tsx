import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { ReactFlowProvider } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import './theme.css';
import './code/code.css';
import './dialogs/start.css';
import './panels/dock.css';
import './canvas/canvas.css';
import './gui.css';
import './controls/controls.css';
import './canvas/bus.css';
import { App } from './App';
import { reportError } from './api';
import { FloatView } from './panels/Floats';

window.addEventListener('error', (e) => {
  // Monaco (automaticLayout) makes the browser defer a resize notice to the next frame. That is not a fault.
  if (!e.message.startsWith('ResizeObserver loop')) reportError(e.error ?? e.message);
});
window.addEventListener('unhandledrejection', (e) => reportError(e.reason));
// The menu of the WebView (Back, Refresh, Print) has no use in the editor. Text fields keep it for Cut, Copy and Paste (15.2).
window.addEventListener('contextmenu', (e) => {
  if (!(e.target instanceof Element && e.target.closest('input, textarea, [contenteditable="true"]'))) e.preventDefault();
});

// A float window (15.8) opens this page with the view slot and the view name in the query.
const params = new URLSearchParams(location.search);
const float = params.get('float');

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {float ? (
      <FloatView slot={float} name={params.get('name') ?? ''} />
    ) : (
      <ReactFlowProvider>
        <App />
      </ReactFlowProvider>
    )}
  </StrictMode>,
);
