import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { ReactFlowProvider } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import './theme.css';
import './dock.css';
import './canvas.css';
import { App } from './App';
import { reportError } from './api';

window.addEventListener('error', (e) => {
  // Monaco (automaticLayout) makes the browser defer a resize notice to the next frame. That is not a fault.
  if (!e.message.startsWith('ResizeObserver loop')) reportError(e.error ?? e.message);
});
window.addEventListener('unhandledrejection', (e) => reportError(e.reason));

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ReactFlowProvider>
      <App />
    </ReactFlowProvider>
  </StrictMode>,
);
