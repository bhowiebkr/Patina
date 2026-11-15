// The dock layout of the editor window (spec 15.2): the tabs of each dock area, the size of each
// area and the pan and zoom of each graph. These are pure functions, so `npm test` runs them.

export const PANELS = ['layers', 'project', 'inspector', 'logs', 'output'] as const;
export type PanelId = (typeof PANELS)[number];
export const AREAS = ['left', 'right', 'bottom'] as const;
export type AreaId = (typeof AREAS)[number];

export const TITLES: Record<PanelId, string> = { layers: 'Layers', project: 'Project', inspector: 'Inspector', logs: 'Logs', output: 'Output' };
/** The area of each panel in the default layout. A closed panel opens there again. */
const HOME: Record<PanelId, AreaId> = { layers: 'left', project: 'left', inspector: 'right', logs: 'bottom', output: 'bottom' };

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

export interface Layout {
  areas: Record<AreaId, Area>;
  /** The pan and zoom of each graph, by the graph name. */
  viewports: Record<string, Viewport>;
}

/** The layout of the table of spec 15.2. */
export function defaultLayout(): Layout {
  return {
    areas: {
      left: { tabs: ['layers', 'project'], active: 'project', size: 260 },
      right: { tabs: ['inspector'], active: 'inspector', size: 360 },
      bottom: { tabs: ['logs', 'output'], active: 'output', size: 200 },
    },
    viewports: {},
  };
}

export function areaOf(layout: Layout, panel: PanelId): AreaId | null {
  return AREAS.find((id) => layout.areas[id].tabs.includes(panel)) ?? null;
}

const withArea = (layout: Layout, id: AreaId, area: Area): Layout => ({ ...layout, areas: { ...layout.areas, [id]: area } });

/** Removes the tab of the panel. The next tab of the area becomes active. */
export function closePanel(layout: Layout, panel: PanelId): Layout {
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
  const out = closePanel(layout, panel);
  const area = out.areas[to];
  const tabs = [...area.tabs];
  const at = before ? tabs.indexOf(before) : -1;
  tabs.splice(at < 0 ? tabs.length : at, 0, panel);
  return withArea(out, to, { ...area, tabs, active: panel });
}

/** Makes the panel the active tab of its area. A closed panel opens in its default area. */
export function openPanel(layout: Layout, panel: PanelId): Layout {
  const id = areaOf(layout, panel);
  if (!id) return movePanel(layout, panel, HOME[panel]);
  return layout.areas[id].active === panel ? layout : withArea(layout, id, { ...layout.areas[id], active: panel });
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
  return layout;
}
