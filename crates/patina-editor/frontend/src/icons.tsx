import type { ReactNode } from 'react';
import type { Domain, NodeKind } from './api';

/** The names of the node kinds, for the icon tooltips (15.3). */
const KIND_NAMES: Record<NodeKind, string> = {
  function: 'Function node',
  stateful: 'Stateful node',
  generic: 'Generic node',
  async: 'Async node',
  trigger: 'Trigger node',
  subgraph: 'Subgraph',
};

/** Patina's own drawings of the node kinds, in a 14 px box (15.3). */
const KIND_PATHS: Record<NodeKind, ReactNode> = {
  function: (
    <>
      <rect x="4" y="3.5" width="6" height="7" rx="1" />
      <path d="M1 7h3M10 7h3" />
    </>
  ),
  stateful: (
    <>
      <ellipse cx="7" cy="3.5" rx="4.5" ry="1.8" />
      <path d="M2.5 3.5v7a4.5 1.8 0 0 0 9 0v-7M2.5 7a4.5 1.8 0 0 0 9 0" />
    </>
  ),
  generic: <path d="M5 3 1.5 7 5 11M9 3l3.5 4L9 11M7 6.2v1.6" />,
  async: <path d="M3.5 1.5h7M3.5 12.5h7M4.5 1.5 7 7l-2.5 5.5M9.5 1.5 7 7l2.5 5.5" />,
  trigger: <path d="M8 1.5 3.5 8H7l-1 4.5L10.5 6H7z" />,
  subgraph: (
    <>
      <rect x="1.5" y="1.5" width="11" height="11" rx="1.5" />
      <rect x="4.5" y="4.5" width="5" height="5" rx="0.5" />
    </>
  ),
};

/** The names of the clock domains, for the icon tooltips (9.1). */
const DOMAIN_NAMES: Record<Domain, string> = {
  batch: 'Batch domain: runs once',
  frame: 'Frame domain',
  audio: 'Audio domain',
  event: 'Event domain',
};

const DOMAIN_PATHS: Record<Domain, ReactNode> = {
  batch: <path d="M3 3v8l5.5-4zM10.5 3v8" />,
  frame: (
    <>
      <rect x="1.5" y="2.5" width="11" height="9" rx="1" />
      <path d="M4.5 2.5v9M9.5 2.5v9" />
    </>
  ),
  audio: <path d="M1 7Q4 1 7 7t6 0" />,
  event: (
    <>
      <circle cx="4" cy="10" r="1" />
      <path d="M4 6.5A3.5 3.5 0 0 1 7.5 10M4 3a7 7 0 0 1 7 7" />
    </>
  ),
};

function Icon({ className, name, children }: { className: string; name: string; children: ReactNode }) {
  return (
    <span className={`node-icon ${className}`} role="img" aria-label={name} title={name}>
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" strokeLinecap="round" aria-hidden="true">
        {children}
      </svg>
    </span>
  );
}

/** The icon of the node kind, before the node name (15.3). */
export function KindIcon({ kind }: { kind: NodeKind }) {
  return (
    <Icon className={`kind-icon ${kind}`} name={KIND_NAMES[kind]}>
      {KIND_PATHS[kind]}
    </Icon>
  );
}

/** The icon of the clock domain of a node use (9.1, 15.3). */
export function DomainIcon({ domain }: { domain: Domain }) {
  return (
    <Icon className={`domain-icon ${domain}`} name={DOMAIN_NAMES[domain]}>
      {DOMAIN_PATHS[domain]}
    </Icon>
  );
}
