// The dock areas of the editor window (spec 15.2): the tabs of each area, the splitters, the
// Views menu, and the saved layout of each project. Only these parts read the layout, so a drag
// does not render the graph again.

import { useLayoutEffect, useRef, useState, useSyncExternalStore, type DragEvent, type KeyboardEvent, type PointerEvent, type ReactNode } from 'react';
import { api, reportError } from './api';
import {
  closePanel,
  defaultLayout,
  floatPanel,
  isOpen,
  isShown,
  layoutText,
  MAX_SIZE,
  MIN_SIZE,
  movePanel,
  openPanel,
  openView,
  PANELS,
  parseLayout,
  railClick,
  railPanels,
  resetLayout,
  resizeArea,
  setViewport,
  TITLES,
  VIEW_SLOTS,
  viewTitle,
  type AreaId,
  type Layout,
  type PanelId,
  type Viewport,
  type ViewSlot,
} from './layout';

/** A change waits this long before it goes to the file, so that a drag saves one time. */
const SAVE_MS = 300;
const TAB_MIME = 'application/x-patina-tab';

let state = { id: '', serial: -1, layout: defaultLayout() };
let timer: ReturnType<typeof setTimeout> | undefined;
const listeners = new Set<() => void>();
const subscribe = (f: () => void) => {
  listeners.add(f);
  return () => {
    listeners.delete(f);
  };
};
const getLayout = () => state.layout;
const getSerial = () => state.serial;

let floatKey = '';

function set(next: typeof state) {
  state = next;
  for (const f of listeners) f();
  // The windows follow the float slots of the layout (15.8). A move of a window changes no window.
  const { layout } = state;
  const floats = VIEW_SLOTS.filter((s) => s in layout.floats).map((s) => ({ slot: s, name: layout.views[s] ?? '', title: viewTitle(layout, s), rect: layout.floats[s] ?? null }));
  const key = floats.map((f) => `${f.slot} ${f.name} ${f.title}`).join('\n');
  if (key === floatKey) return;
  floatKey = key;
  api.setFloats(floats).catch(reportError);
}

/** Closes the float windows and forgets the layout, when the project closes. */
export async function endLayout() {
  await flushLayout();
  set({ id: '', serial: -1, layout: defaultLayout() });
}

/** Changes the layout of the open project and saves it after a short time. */
export function changeLayout(f: (layout: Layout) => Layout) {
  const layout = f(state.layout);
  if (layout === state.layout) return;
  // A closed or replaced slot drops its view instance, so the next instance starts with `init`.
  for (const slot of VIEW_SLOTS) {
    if (state.layout.views[slot] && state.layout.views[slot] !== layout.views[slot]) api.closeView(slot).catch(reportError);
  }
  set({ ...state, layout });
  if (timer === undefined) api.layoutPending(true).catch(reportError);
  clearTimeout(timer);
  timer = setTimeout(flushLayout, SAVE_MS);
}

/** Saves a changed layout now, for example before the project closes. */
export async function flushLayout() {
  if (timer === undefined) return;
  clearTimeout(timer);
  timer = undefined;
  if (state.id) await api.saveLayout(state.id, layoutText(state.layout)).catch(reportError);
  // A change during the save started a new wait.
  if (timer === undefined) await api.layoutPending(false).catch(reportError);
}

/**
 * Reads the saved layout of the project that opened. A project without a layout file gets the
 * default layout. Gives the error text when Patina cannot keep a layout for the project.
 */
export async function loadLayout(id: string, serial: number): Promise<string> {
  await flushLayout();
  try {
    set({ id, serial, layout: parseLayout(await api.layout(id)) });
    return '';
  } catch (e) {
    set({ id: '', serial, layout: defaultLayout() });
    return String(e);
  }
}

/** The serial of the project of the layout. The dock shows after the layout of a project loads. */
export const useLayoutSerial = () => useSyncExternalStore(subscribe, getSerial);
export const savedViewport = (graph: string): Viewport | undefined => state.layout.viewports[graph];
export const saveViewport = (graph: string, v: Viewport) => changeLayout((l) => setViewport(l, graph, v));

export const Icon = ({ d }: { d: string }) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
    <path d={d} />
  </svg>
);

/** `keys` is the shortcut text at the right of the label. */
export type Item = { label: string; keys?: string; checked?: boolean; disabled?: boolean; run: () => void } | 'line';

/** A menu at a point. Esc or a click out of it closes it, and the focus goes back to its button. */
export function Menu({ x, y, label, items, onClose }: { x: number; y: number; label: string; items: Item[]; onClose: () => void }) {
  const [back] = useState(() => document.activeElement as HTMLElement | null);
  const box = useRef<HTMLDivElement>(null);
  // The menu moves into the window when the point is near its right or bottom edge.
  useLayoutEffect(() => {
    const el = box.current!;
    const r = el.getBoundingClientRect();
    el.style.left = `${Math.max(0, Math.min(x, innerWidth - r.width))}px`;
    el.style.top = `${Math.max(0, Math.min(y, innerHeight - r.height))}px`;
  }, [x, y]);
  const close = () => {
    onClose();
    back?.focus();
  };
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const list = [...e.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    const at = list.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'Escape' || e.key === 'Tab') close();
    else if (e.key === 'ArrowDown') list[(at + 1) % list.length]?.focus();
    else if (e.key === 'ArrowUp') list[(at - 1 + list.length) % list.length]?.focus();
    else return;
    // The keys of the menu do not go to the graph.
    e.preventDefault();
    e.stopPropagation();
  };
  const first = items.findIndex((item) => item !== 'line' && !item.disabled);
  return (
    <div className="menu-layer" onPointerDown={(e) => e.target === e.currentTarget && close()} onContextMenu={(e) => e.preventDefault()}>
      <div ref={box} className="menu" role="menu" aria-label={label} style={{ left: x, top: y }} onKeyDown={onKey}>
        {items.map((item, i) =>
          item === 'line' ? (
            <hr key={i} />
          ) : (
            <button
              key={i}
              role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
              aria-checked={item.checked}
              disabled={item.disabled}
              autoFocus={i === first}
              onClick={() => {
                close();
                item.run();
              }}
            >
              {item.label}
              {item.keys && <span className="keys">{item.keys}</span>}
            </button>
          ),
        )}
      </div>
    </div>
  );
}

/** The menu point under an element. */
export const below = (el: HTMLElement) => {
  const r = el.getBoundingClientRect();
  return { x: Math.min(r.left, window.innerWidth - 200), y: r.bottom };
};

/** The Views menu of the top bar: show or hide each panel, and Reset layout (15.2). */
/** The Views menu (15.2, 15.8): the panels, the views of the project and Reset layout. */
export function ViewsMenu({ views }: { views: string[] }) {
  const layout = useSyncExternalStore(subscribe, getLayout);
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const full = VIEW_SLOTS.every((s) => isOpen(layout, s));
  const items: Item[] = PANELS.filter((p) => !VIEW_SLOTS.includes(p as ViewSlot)).map((p) => ({
    label: TITLES[p],
    checked: isShown(layout, p),
    run: () => changeLayout((l) => (isShown(l, p) ? closePanel(l, p) : openPanel(l, p))),
  }));
  if (views.length > 0) items.push('line');
  for (const name of views) {
    // Each click opens a new instance (15.8).
    const open = VIEW_SLOTS.some((s) => layout.views[s] === name && isOpen(layout, s));
    items.push({ label: name, checked: open, disabled: full, run: () => changeLayout((l) => openView(l, name)) });
  }
  items.push('line', { label: 'Reset layout', run: () => changeLayout(resetLayout) });
  return (
    <>
      <button className="btn" aria-haspopup="menu" aria-expanded={at !== null} onClick={(e) => setAt(below(e.currentTarget))}>
        Views
        <Icon d="m4 6 4 4 4-4" />
      </button>
      {at && <Menu {...at} label="Views" items={items} onClose={() => setAt(null)} />}
    </>
  );
}

/** A splitter changes the size of an area with the mouse or the arrow keys. */
function Splitter({ area, size }: { area: AreaId; size: number }) {
  const start = useRef<{ at: number; size: number } | null>(null);
  const vertical = area !== 'bottom';
  // The left area grows to the right. The right and bottom areas grow to the left and up.
  const sign = area === 'left' ? 1 : -1;
  const pos = (e: PointerEvent) => (vertical ? e.clientX : e.clientY);
  const resize = (n: number) => changeLayout((l) => resizeArea(l, area, n));
  const onKey = (e: KeyboardEvent) => {
    const keys: Record<string, number> = vertical ? { ArrowLeft: -1, ArrowRight: 1 } : { ArrowUp: -1, ArrowDown: 1 };
    if (!keys[e.key]) return;
    e.preventDefault();
    resize(size + keys[e.key] * sign * (e.shiftKey ? 64 : 16));
  };
  return (
    <div
      className={vertical ? 'splitter vertical' : 'splitter horizontal'}
      role="separator"
      tabIndex={0}
      aria-orientation={vertical ? 'vertical' : 'horizontal'}
      aria-label={`Size of the ${area} area`}
      aria-valuenow={size}
      aria-valuemin={MIN_SIZE}
      aria-valuemax={MAX_SIZE}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        start.current = { at: pos(e), size };
      }}
      onPointerMove={(e) => start.current && resize(start.current.size + sign * (pos(e) - start.current.at))}
      onPointerUp={() => (start.current = null)}
      onPointerCancel={() => (start.current = null)}
      onKeyDown={onKey}
    />
  );
}

export interface Panel {
  body: ReactNode;
  /** The tab title. The default is the title of the panel. */
  title?: string;
  /** The buttons of the panel in the tab row, for example Clear. */
  actions?: ReactNode;
}

interface AreaProps {
  id: AreaId;
  layout: Layout;
  panels: Record<PanelId, Panel>;
  dragging: boolean;
  onDrag: (dragging: boolean) => void;
}

/** One dock area: a tab for each panel and the body of the active panel. */
function DockArea({ id, layout, panels, dragging, onDrag }: AreaProps) {
  const [menu, setMenu] = useState<{ panel: PanelId; x: number; y: number } | null>(null);
  const { tabs, active, size } = layout.areas[id];
  const over = (e: DragEvent) => {
    if (!e.dataTransfer.types.includes(TAB_MIME)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
  };
  const drop = (e: DragEvent, before?: PanelId) => {
    const panel = e.dataTransfer.getData(TAB_MIME);
    if (!PANELS.includes(panel as PanelId)) return;
    e.preventDefault();
    e.stopPropagation();
    onDrag(false);
    changeLayout((l) => movePanel(l, panel as PanelId, id, before));
  };
  if (tabs.length === 0 || (id === 'left' && layout.folded)) {
    // An empty or folded area takes no space. It shows only as a place to drop a tab.
    return dragging ? (
      <div className={`drop-zone ${id}`} aria-label={`Drop a tab in the ${id} area`} onDragOver={over} onDrop={(e) => drop(e)} />
    ) : null;
  }
  const moveItems = (panel: PanelId): Item[] => [
    ...(['left', 'right', 'bottom'] as const).map((to) => ({
      label: `Move to the ${to} area`,
      disabled: to === id,
      run: () => changeLayout((l) => movePanel(l, panel, to)),
    })),
    ...(layout.views[panel as ViewSlot] ? [{ label: 'Float in a window', run: () => changeLayout((l) => floatPanel(l, panel as ViewSlot)) }] : []),
    'line',
    { label: 'Close', run: () => changeLayout((l) => closePanel(l, panel)) },
  ];
  return (
    <section className={`area ${id}`} style={id === 'bottom' ? { height: size } : { width: size }} onDragOver={over} onDrop={(e) => drop(e)}>
      <div className="tabs" role="tablist" aria-label={`The ${id} area`}>
        {tabs.map((t) => (
          <button
            key={t}
            role="tab"
            aria-selected={t === active}
            className={t === active ? 'tab active' : 'tab'}
            title="Drag to another area, or right-click to move or close"
            draggable
            onClick={() => changeLayout((l) => openPanel(l, t))}
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu({ panel: t, ...below(e.currentTarget) });
            }}
            onDragStart={(e) => {
              e.dataTransfer.setData(TAB_MIME, t);
              e.dataTransfer.effectAllowed = 'move';
              // A change of the page in dragstart can stop the drag, so the drop zones show after it.
              setTimeout(() => onDrag(true));
            }}
            onDragEnd={(e) => {
              onDrag(false);
              // A view tab that drops out of the window floats there (15.2).
              const out = e.clientX < 0 || e.clientY < 0 || e.clientX > innerWidth || e.clientY > innerHeight;
              if (out && e.dataTransfer.dropEffect === 'none' && layout.views[t as ViewSlot]) {
                changeLayout((l) => floatPanel(l, t as ViewSlot, { x: e.screenX - 40, y: e.screenY - 10, w: 480, h: 640 }));
              }
            }}
            onDrop={(e) => drop(e, t)}
          >
            {panels[t].title ?? TITLES[t]}
          </button>
        ))}
        <span className="spacer" />
        {active && panels[active].actions}
        {active && (
          <button className="icon-btn" aria-label={`Close ${TITLES[active]}`} title="Close" onClick={() => changeLayout((l) => closePanel(l, active))}>
            <Icon d="m4 4 8 8m0-8-8 8" />
          </button>
        )}
      </div>
      <div className="area-body" role="tabpanel" aria-label={active ? TITLES[active] : undefined}>
        {active && panels[active].body}
      </div>
      {menu && <Menu x={menu.x} y={menu.y} label={TITLES[menu.panel]} items={moveItems(menu.panel)} onClose={() => setMenu(null)} />}
    </section>
  );
}

interface DockProps {
  panels: Record<Exclude<PanelId, ViewSlot>, Panel>;
  /** The body of a view slot that shows view `name`, or no view. */
  view: (slot: ViewSlot, name: string | null) => ReactNode;
  /** The center: the graph view. */
  children: ReactNode;
}

const WINDOW_ICON = 'M1.5 2.5h13v11h-13zM1.5 5.5h13';
const RAIL_ICONS: Partial<Record<PanelId, string>> = {
  layers: 'M8 1.5 14.5 5 8 8.5 1.5 5zM1.5 8 8 11.5 14.5 8M1.5 11 8 14.5 14.5 11',
  project: 'M1.5 3.5h5l1.5 2h6.5v8h-13z',
  inspector: 'M3 2v12M8 2v12M13 2v12M1.5 5h3M6.5 10h3M11.5 6.5h3',
  logs: 'M2 4h12M2 8h12M2 12h8',
  output: 'M1.5 2.5h13v11h-13zM4 6l2 2-2 2M8 10.5h4',
};

/** The navigation rail, the dock areas, the splitters and the center. */
export function Dock({ panels: fixed, view, children }: DockProps) {
  const layout = useSyncExternalStore(subscribe, getLayout);
  const [dragging, setDragging] = useState(false);
  const slots = Object.fromEntries(VIEW_SLOTS.map((s) => [s, { title: viewTitle(layout, s), body: view(s, layout.views[s] ?? null) }]));
  const panels = { ...fixed, ...slots } as Record<PanelId, Panel>;
  const area = (id: AreaId) => <DockArea id={id} layout={layout} panels={panels} dragging={dragging} onDrag={setDragging} />;
  const split = (id: AreaId) => layout.areas[id].tabs.length > 0 && !(id === 'left' && layout.folded) && <Splitter area={id} size={layout.areas[id].size} />;
  const left = layout.areas.left;
  return (
    <div className="middle">
      <nav className="strip" aria-label="Panels">
        {railPanels(layout).map((p) => {
          const title = panels[p].title ?? TITLES[p];
          const on = left.active === p && !layout.folded;
          return (
            <button
              key={p}
              className={on ? 'icon-btn active' : 'icon-btn'}
              aria-label={title}
              aria-pressed={on}
              title={on ? `${title} - click to fold the left area` : title}
              onClick={() => changeLayout((l) => railClick(l, p))}
            >
              <Icon d={RAIL_ICONS[p] ?? WINDOW_ICON} />
            </button>
          );
        })}
      </nav>
      {area('left')}
      {split('left')}
      <div className="work">
        {children}
        {split('bottom')}
        {area('bottom')}
      </div>
      {split('right')}
      {area('right')}
    </div>
  );
}
