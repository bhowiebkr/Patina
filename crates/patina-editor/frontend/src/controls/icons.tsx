import type { CSSProperties, ReactNode } from 'react';
import type { Domain, NodeKind } from '../api';

/** The names of the node kinds, for the icon tooltips (15.3). */
const KIND_NAMES: Record<NodeKind, string> = {
  function: 'Function node',
  stateful: 'Stateful node',
  generic: 'Generic node',
  async: 'Async node',
  trigger: 'Trigger node',
  subgraph: 'Subgraph',
};

/** Patina's own drawings of the node kinds, as SVG markup in a 14 px box (15.3). */
const KIND_PATHS: Record<NodeKind, string> = {
  function: '<rect x="4" y="3.5" width="6" height="7" rx="1"/><path d="M1 7h3M10 7h3"/>',
  stateful: '<ellipse cx="7" cy="3.5" rx="4.5" ry="1.8"/><path d="M2.5 3.5v7a4.5 1.8 0 0 0 9 0v-7M2.5 7a4.5 1.8 0 0 0 9 0"/>',
  generic: '<path d="M5 3 1.5 7 5 11M9 3l3.5 4L9 11M7 6.2v1.6"/>',
  async: '<path d="M3.5 1.5h7M3.5 12.5h7M4.5 1.5 7 7l-2.5 5.5M9.5 1.5 7 7l2.5 5.5"/>',
  trigger: '<path d="M8 1.5 3.5 8H7l-1 4.5L10.5 6H7z"/>',
  subgraph: '<rect x="1.5" y="1.5" width="11" height="11" rx="1.5"/><rect x="4.5" y="4.5" width="5" height="5" rx="0.5"/>',
};

/** The names of the clock domains, for the icon tooltips (9.1). */
const DOMAIN_NAMES: Record<Domain, string> = {
  batch: 'Batch domain: runs once',
  frame: 'Frame domain',
  audio: 'Audio domain',
  event: 'Event domain',
};

const DOMAIN_PATHS: Record<Domain, string> = {
  batch: '<path d="M3 3v8l5.5-4zM10.5 3v8"/>',
  frame: '<rect x="1.5" y="2.5" width="11" height="9" rx="1"/><path d="M4.5 2.5v9M9.5 2.5v9"/>',
  audio: '<path d="M1 7Q4 1 7 7t6 0"/>',
  event: '<circle cx="4" cy="10" r="1"/><path d="M4 6.5A3.5 3.5 0 0 1 7.5 10M4 3a7 7 0 0 1 7 7"/>',
};

/** The icons that a node GUI icon button can show (4.3), as paths in a 16 px box. */
const GUI_ICONS: Record<string, string> = {
  'rotate-left': 'M2 8a6 6 0 1 0 6-6 6.5 6.5 0 0 0-4.5 1.8L2 5.3M2 2v3.3h3.3',
  'rotate-right': 'M14 8a6 6 0 1 1-6-6 6.5 6.5 0 0 1 4.5 1.8L14 5.3M14 2v3.3h-3.3',
  'flip-h': 'M2 8h12M5 5 2 8l3 3M11 5l3 3-3 3',
  'flip-v': 'M8 2v12M5 5l3-3 3 3M5 11l3 3 3-3',
  play: 'M4.5 2.5v11l9-5.5z',
  pause: 'M5 3v10M11 3v10',
  stop: 'M3.5 3.5h9v9h-9z',
  rewind: 'M3.5 3v10M12.5 3 5.5 8l7 5z',
  record: 'M8 3a5 5 0 1 1 0 10A5 5 0 1 1 8 3z',
  add: 'M8 3v10M3 8h10',
  remove: 'M3 8h10',
  folder: 'M1.5 3.5h4.5l1.5 1.5h7v7.5h-13z',
  eye: 'M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8zM8 6a2 2 0 1 1 0 4 2 2 0 1 1 0-4z',
  'eye-off': 'M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8zM2.5 2.5l11 11',
  grip: 'M5.5 3h1v1h-1zM9.5 3h1v1h-1zM5.5 7.5h1v1h-1zM9.5 7.5h1v1h-1zM5.5 12h1v1h-1zM9.5 12h1v1h-1z',
};

/** The drawing of a node GUI icon, or null for an unknown name. */
export function guiIcon(name: string): ReactNode {
  const d = Object.hasOwn(GUI_ICONS, name) ? GUI_ICONS[name] : null;
  return d === null ? null : (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" strokeLinecap="round" aria-hidden="true">
      <path d={d} />
    </svg>
  );
}

const iconStyles = new Map<string, CSSProperties>();

/**
 * The drawing as background images in the muted and the text colors of the theme. In 2000 nodes, a
 * background image paints much faster than an inline SVG (docs/quality-plan.md, phase 4).
 */
function iconStyle(paths: string): CSSProperties {
  let style = iconStyles.get(paths);
  if (!style) {
    const theme = getComputedStyle(document.documentElement);
    const url = (token: string, opacity: number) => {
      const stroke = theme.getPropertyValue(token).trim();
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="${stroke}" stroke-opacity="${opacity}" stroke-width="1.3" stroke-linejoin="round" stroke-linecap="round">${paths}</svg>`;
      return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
    };
    style = { '--icon': url('--muted', 1), '--icon-colored': url('--text', 0.8) } as CSSProperties;
    iconStyles.set(paths, style);
  }
  return style;
}

function Icon({ className, name, paths }: { className: string; name: string; paths: string }) {
  return <span className={`node-icon ${className}`} role="img" aria-label={name} title={name} style={iconStyle(paths)} />;
}

/** The icon of the node kind, before the node name (15.3). */
export function KindIcon({ kind }: { kind: NodeKind }) {
  return (
    <Icon className={`kind-icon ${kind}`} name={KIND_NAMES[kind]} paths={KIND_PATHS[kind]} />
  );
}

/** The icon of the clock domain of a node use (9.1, 15.3). */
export function DomainIcon({ domain }: { domain: Domain }) {
  return (
    <Icon className={`domain-icon ${domain}`} name={DOMAIN_NAMES[domain]} paths={DOMAIN_PATHS[domain]} />
  );
}
