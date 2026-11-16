// The dock layout of the editor window (spec 15.2): the tabs of each dock area, the size of each
// area and the pan and zoom of each graph. These are pure functions, so `npm test` runs them.

/** The panels of the view instances (15.8). Each slot shows one instance of a view. */
export const VIEW_SLOTS = ['view', 'view2', 'view3', 'view4'] as const;
export type ViewSlot = (typeof VIEW_SLOTS)[number];
export const PANELS = ['layers', 'project', 'inspector', 'logs', 'output', ...VIEW_SLOTS] as const;
export type PanelId = (typeof PANELS)[number];
export const AREAS = ['left', 'right', 'bottom'] as const;
export type AreaId = (typeof AREAS)[number];

export const TITLES: Record<PanelId, string> = { layers: 'Layers', project: 'Project', inspector: 'Inspector', logs: 'Logs', output: 'Output', view: 'View', view2: 'View 2', view3: 'View 3', view4: 'View 4' };
/** The area of each panel in the default layout. A closed panel opens there again. */
const HOME: Record<PanelId, AreaId> = { layers: 'left', project: 'left', inspector: 'right', logs: 'bottom', output: 'bottom', view: 'right', view2: 'right', view3: 'right', view4: 'right' };

export const MIN_SIZE = 120;
export const MAX_SIZE = 1600;
/** The zoom limits of the graph view. */
const MIN_ZOOM = 0.1;
const MAX_ZOOM = 4;
const VERSION = 1;

export interface Area {
  tabs: PanelId[];
  active: PanelId | null;
  /** The width of a side area or the height of the bottom area, in pixels. */
  size: number;
}

export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

/** The place and the size of a float window on the screen, in logical pixels. */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Layout {
  areas: Record<AreaId, Area>;
  /** The view slots in float windows (15.8), with the last place of each window. */
  floats: Partial<Record<ViewSlot, Rect | null>>;
  /** The pan and zoom of each graph, by the graph name. */
  viewports: Record<string, Viewport>;
  /** The view folder of each open view slot, for example `views/scene`. */
  views: Partial<Record<ViewSlot, string>>;
  /** True when the left area folds to the icons of the navigation rail. */
  folded: boolean;
}

/** The layout of the table of spec 15.2. */
export function defaultLayout(): Layout {
  return {
    areas: {
      left: { tabs: ['layers', 'project'], active: 'project', size: 260 },
      right: { tabs: ['inspector'], active: 'inspector', size: 360 },
      bottom: { tabs: ['logs', 'output'], active: 'output', size: 200 },
    },
    floats: {},
    viewports: {},
    views: {},
    folded: false,
  };
}

export function areaOf(layout: Layout, panel: PanelId): AreaId | null {
  return AREAS.find((id) => layout.areas[id].tabs.includes(panel)) ?? null;
}

const withArea = (layout: Layout, id: AreaId, area: Area): Layout => ({ ...layout, areas: { ...layout.areas, [id]: area } });
const unfold = (layout: Layout): Layout => (layout.folded ? { ...layout, folded: false } : layout);

const isSlot = (v: unknown): v is ViewSlot => VIEW_SLOTS.includes(v as ViewSlot);

/** True when the panel shows: in a dock area or in a float window. */
export const isOpen = (layout: Layout, panel: PanelId) => areaOf(layout, panel) !== null || (isSlot(panel) && panel in layout.floats);

/** The layout without the float window of `slot`. */
function unfloat(layout: Layout, slot: ViewSlot): Layout {
  if (!(slot in layout.floats)) return layout;
  const floats = { ...layout.floats };
  delete floats[slot];
  return { ...layout, floats };
}

/** Closes the panel. A closed view slot drops its view instance. */
export function closePanel(layout: Layout, panel: PanelId): Layout {
  const out = removeTab(layout, panel);
  if (!isSlot(panel)) return out;
  const views = { ...out.views };
  delete views[panel];
  return { ...unfloat(out, panel), views };
}

/** Moves a view slot that shows a view out of the dock, into its own window (15.8). */
export function floatPanel(layout: Layout, slot: ViewSlot, rect: Rect | null = null): Layout {
  if (!layout.views[slot]) return layout;
  return { ...removeTab(layout, slot), floats: { ...layout.floats, [slot]: rect } };
}

/** Moves a float view slot back into its default dock area. */
export function dockPanel(layout: Layout, slot: ViewSlot): Layout {
  return slot in layout.floats ? movePanel(unfloat(layout, slot), slot, HOME[slot]) : layout;
}

/** Keeps the last place of a float window. */
export function moveFloat(layout: Layout, slot: ViewSlot, rect: Rect): Layout {
  return slot in layout.floats ? { ...layout, floats: { ...layout.floats, [slot]: rect } } : layout;
}

/** Opens a new instance of view `name` in the first closed view slot. All slots open: no change. */
export function openView(layout: Layout, name: string): Layout {
  const slot = VIEW_SLOTS.find((s) => !isOpen(layout, s));
  return slot ? openPanel({ ...layout, views: { ...layout.views, [slot]: name } }, slot) : layout;
}

/** The tab title of a view slot: the view name, with the number of the instance from the second. */
export function viewTitle(layout: Layout, slot: ViewSlot): string {
  const name = layout.views[slot];
  if (!name) return TITLES[slot];
  const n = VIEW_SLOTS.slice(0, VIEW_SLOTS.indexOf(slot) + 1).filter((s) => layout.views[s] === name).length;
  const short = name.replace(/^views\//, '');
  return n > 1 ? `${short} ${n}` : short;
}

/** Removes the tab of the panel. The next tab of the area becomes active. */
function removeTab(layout: Layout, panel: PanelId): Layout {
  const id = areaOf(layout, panel);
  if (!id) return layout;
  const area = layout.areas[id];
  const at = area.tabs.indexOf(panel);
  const tabs = area.tabs.filter((t) => t !== panel);
  const active = area.active === panel ? (tabs[Math.min(at, tabs.length - 1)] ?? null) : area.active;
  return withArea(layout, id, { ...area, tabs, active });
}

/** Moves the tab of the panel before the tab `before`, or to the end of the area. */
export function movePanel(layout: Layout, panel: PanelId, to: AreaId, before?: PanelId): Layout {
  if (before === panel) return layout;
  const out = removeTab(layout, panel);
  const area = out.areas[to];
  const tabs = [...area.tabs];
  const at = before ? tabs.indexOf(before) : -1;
  tabs.splice(at < 0 ? tabs.length : at, 0, panel);
  const next = withArea(out, to, { ...area, tabs, active: panel });
  return to === 'left' ? unfold(next) : next;
}

/** Makes the panel the active tab of its area. A closed panel opens in its default area. */
export function openPanel(layout: Layout, panel: PanelId): Layout {
  const id = areaOf(layout, panel);
  if (!id) return movePanel(layout, panel, HOME[panel]);
  const next = layout.areas[id].active === panel ? layout : withArea(layout, id, { ...layout.areas[id], active: panel });
  return id === 'left' ? unfold(next) : next;
}

/** True when the panel has a tab in a dock area that shows: a folded left area does not show. */
export const isShown = (layout: Layout, panel: PanelId) => {
  const id = areaOf(layout, panel);
  return id !== null && !(id === 'left' && layout.folded);
};

/** The panels of the navigation rail: the tabs of the left area, and Project when it is closed. */
export function railPanels(layout: Layout): PanelId[] {
  const tabs = layout.areas.left.tabs;
  return isOpen(layout, 'project') ? tabs : [...tabs, 'project'];
}

/** A click on a rail icon. The icon of the active panel of the left area folds or unfolds the area. */
export function railClick(layout: Layout, panel: PanelId): Layout {
  const left = layout.areas.left;
  if (left.active === panel) return { ...layout, folded: !layout.folded };
  return openPanel(layout, panel);
}

const clampSize = (size: number) => Math.round(Math.min(MAX_SIZE, Math.max(MIN_SIZE, size)));

export function resizeArea(layout: Layout, id: AreaId, size: number): Layout {
  const next = clampSize(size);
  return next === layout.areas[id].size ? layout : withArea(layout, id, { ...layout.areas[id], size: next });
}

export function setViewport(layout: Layout, graph: string, v: Viewport): Layout {
  return { ...layout, viewports: { ...layout.viewports, [graph]: v } };
}

/** Reset layout (15.2): the default areas. The pan and zoom of the graphs stay. */
export function resetLayout(layout: Layout): Layout {
  return { ...defaultLayout(), viewports: layout.viewports };
}

export function layoutText(layout: Layout): string {
  return JSON.stringify({ version: VERSION, ...layout });
}

type Raw = Record<string, unknown>;
const isRaw = (v: unknown): v is Raw => typeof v === 'object' && v !== null && !Array.isArray(v);
const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const isPanel = (v: unknown): v is PanelId => PANELS.includes(v as PanelId);

/**
 * Reads a layout file. A file that is not a layout gives the default layout. Unknown and repeated
 * panels and bad values are dropped, and the sizes are clamped.
 */
export function parseLayout(text: string | null): Layout {
  const layout = defaultLayout();
  let raw: unknown;
  try {
    raw = text === null ? null : JSON.parse(text);
  } catch {
    return layout;
  }
  if (!isRaw(raw) || raw.version !== VERSION || !isRaw(raw.areas)) return layout;
  const seen = new Set<PanelId>();
  for (const id of AREAS) {
    const area = isRaw(raw.areas[id]) ? raw.areas[id] : {};
    const tabs = (Array.isArray(area.tabs) ? area.tabs : []).filter((t): t is PanelId => isPanel(t) && !seen.has(t) && !!seen.add(t));
    const active = isPanel(area.active) && tabs.includes(area.active) ? area.active : (tabs[0] ?? null);
    layout.areas[id] = { tabs, active, size: finite(area.size) ? clampSize(area.size) : layout.areas[id].size };
  }
  for (const [graph, v] of Object.entries(isRaw(raw.viewports) ? raw.viewports : {})) {
    if (isRaw(v) && finite(v.x) && finite(v.y) && finite(v.zoom) && v.zoom >= MIN_ZOOM && v.zoom <= MAX_ZOOM) {
      layout.viewports[graph] = { x: v.x, y: v.y, zoom: v.zoom };
    }
  }
  for (const [slot, r] of Object.entries(isRaw(raw.floats) ? raw.floats : {})) {
    if (!isSlot(slot) || seen.has(slot)) continue;
    if (r === null) layout.floats[slot] = null;
    else if (isRaw(r) && finite(r.x) && finite(r.y) && finite(r.w) && finite(r.h)) layout.floats[slot] = { x: r.x, y: r.y, w: clampSize(r.w), h: clampSize(r.h) };
    else continue;
    seen.add(slot);
  }
  for (const [slot, name] of Object.entries(isRaw(raw.views) ? raw.views : {})) {
    if (isSlot(slot) && typeof name === 'string' && seen.has(slot)) layout.views[slot] = name;
  }
  layout.folded = raw.folded === true;
  // A float window without a view has nothing to show.
  for (const slot of VIEW_SLOTS) if (!layout.views[slot]) delete layout.floats[slot];
  return layout;
}
