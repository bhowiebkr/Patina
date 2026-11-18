// The timeline widget of a node GUI (spec 4.3): a transport, a ruler with the loop and the
// playhead, and clip lanes and automation lanes on one time axis (mockup 5). A drag sends one
// edit at its end, as the other value widgets do.

import { useRef, useState, type CSSProperties, type KeyboardEvent, type MouseEvent, type PointerEvent } from 'react';
import { guiIcon } from './icons';
import { clock, curvePath, dragClip, dragPoint, dragRange, quantity, ruler, type Clip, type ClipPart, type Lane } from './timelineData';
import { own, round, Titled, type Input } from './ValueWidgets';

const SNAPS = [0, 0.01, 0.05, 0.1, 0.25, 0.5, 1];
const ARROWS: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1 };

type Drag = { moved: boolean } & (
  | { kind: 'seek'; at: number }
  | { kind: 'loop'; end: 0 | 1; at: number }
  | { kind: 'clip'; lane: number; i: number; part: ClipPart; from: number; at: number }
  | { kind: 'point'; lane: number; i: number; at: [number, number] }
);

type Auto = Extract<Lane, { kind: 'auto' }>;
const top = (l: Auto, v: number) => (l.high > l.low ? 1 - (v - l.low) / (l.high - l.low) : 0.5);
const clipEdit = (lane: number, i: number, c: Clip) => `clip ${lane} ${i} ${c.start} ${c.length} ${c.offset}`;

export function Timeline({ w, send, disabled }: Input<'timeline'>) {
  const t = w.t;
  const total = t.length;
  const [drag, setDrag] = useState<Drag | null>(null);
  const rect = useRef<DOMRect | null>(null);
  const pct = (time: number) => `${(time / total) * 100}%`;
  const timeAt = (x: number) => (rect.current ? Math.min(Math.max(((x - rect.current.left) / rect.current.width) * total, 0), total) : 0);
  const valueAt = (y: number, l: Auto) => (rect.current ? l.high - ((y - rect.current.top) / rect.current.height) * (l.high - l.low) : l.low);
  const step = (e: KeyboardEvent) => (e.shiftKey ? 0.01 : t.snap || 0.1);

  // The values that a drag shows before its edit comes back.
  const playhead = drag?.kind === 'seek' ? round(drag.at) : t.playhead;
  const range = drag?.kind === 'loop' ? dragRange(t.range, drag.end, drag.at, total, t.snap) : t.range;
  const clipAt = (lane: number, i: number, c: Clip) =>
    drag?.kind === 'clip' && drag.lane === lane && drag.i === i ? dragClip(c, drag.part, drag.at - drag.from, total, t.snap) : c;
  const pointAt = (lane: number, i: number, l: Auto) =>
    drag?.kind === 'point' && drag.lane === lane && drag.i === i ? dragPoint(l.points, i, drag.at[0], drag.at[1], l.low, l.high, total, t.snap) : l.points[i];
  const held = drag?.kind === 'clip' || drag?.kind === 'point' ? [drag.lane, drag.i] : t.selected;
  const isSelected = (lane: number, i: number) => held?.[0] === lane && held[1] === i;
  const select = (lane: number, i: number) => !isSelected(lane, i) && send(`select ${lane} ${i}`);

  const down = (e: PointerEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    const area = target.closest<HTMLElement>('.tl-area');
    if (e.button !== 0 || e.detail > 1 || !area) return;
    rect.current = area.getBoundingClientRect();
    const at = timeAt(e.clientX);
    const lane = Number(area.dataset.lane);
    const i = Number(target.closest<HTMLElement>('[data-i]')?.dataset.i);
    const l = t.lanes[lane];
    let next: Drag | null = null;
    if (area.dataset.ruler !== undefined) next = target.dataset.end ? { kind: 'loop', end: target.dataset.end === '1' ? 1 : 0, at, moved: false } : { kind: 'seek', at, moved: false };
    else if (l?.kind === 'clips' && Number.isInteger(i)) next = { kind: 'clip', lane, i, part: (target.dataset.part as ClipPart) ?? 'move', from: at, at, moved: false };
    else if (l?.kind === 'auto' && Number.isInteger(i)) next = { kind: 'point', lane, i, at: l.points[i], moved: false };
    if (!next) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag(next);
  };
  const move = (e: PointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    const l = drag.kind === 'point' ? t.lanes[drag.lane] : null;
    if (drag.kind === 'point' && l?.kind === 'auto') setDrag({ ...drag, at: [timeAt(e.clientX), valueAt(e.clientY, l)], moved: true });
    else if (drag.kind !== 'point') setDrag({ ...drag, at: timeAt(e.clientX), moved: true });
  };
  const up = () => {
    const d = drag;
    setDrag(null);
    if (!d) return;
    if (d.kind === 'seek' && playhead !== t.playhead) send(`seek ${playhead}`);
    if (d.kind === 'loop' && d.moved && (range[0] !== t.range[0] || range[1] !== t.range[1])) send(`range ${range[0]} ${range[1]}`);
    const l = t.lanes[d.kind === 'clip' || d.kind === 'point' ? d.lane : -1];
    if (d.kind === 'clip' && l?.kind === 'clips') {
      const [c, moved] = [l.clips[d.i], clipAt(d.lane, d.i, l.clips[d.i])];
      if (d.moved && (moved.start !== c.start || moved.length !== c.length || moved.offset !== c.offset)) send(clipEdit(d.lane, d.i, moved));
      else select(d.lane, d.i);
    }
    if (d.kind === 'point' && l?.kind === 'auto') {
      const [p, moved] = [l.points[d.i], pointAt(d.lane, d.i, l)];
      if (d.moved && (moved[0] !== p[0] || moved[1] !== p[1])) send(`point ${d.lane} ${d.i} ${moved.join(' ')}`);
      else select(d.lane, d.i);
    }
  };
  // A double click on an automation lane adds a point, and on a point removes it.
  const double = (e: MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    const area = target.closest<HTMLElement>('.tl-area');
    const lane = Number(area?.dataset.lane);
    const l = t.lanes[lane];
    if (!area || l?.kind !== 'auto') return;
    const i = Number(target.dataset.i);
    rect.current = area.getBoundingClientRect();
    if (Number.isInteger(i)) send(`remove ${lane} ${i}`);
    else send(`add ${lane} ${dragPoint([], 0, timeAt(e.clientX), valueAt(e.clientY, l), l.low, l.high, total, t.snap).join(' ')}`);
  };

  const keySeek = (e: KeyboardEvent) => {
    const to = e.key === 'Home' ? 0 : e.key === 'End' ? total : ARROWS[e.key] ? Math.min(Math.max(t.playhead + ARROWS[e.key] * step(e), 0), total) : null;
    if (to === null) return;
    own(e);
    if (round(to) !== t.playhead) send(`seek ${round(to)}`);
  };
  const keyLoop = (e: KeyboardEvent, end: 0 | 1) => {
    if (!ARROWS[e.key]) return;
    own(e);
    const r = dragRange(t.range, end, t.range[end] + ARROWS[e.key] * step(e), total, 0);
    if (r[0] !== t.range[0] || r[1] !== t.range[1]) send(`range ${r[0]} ${r[1]}`);
  };
  const keyClip = (e: KeyboardEvent, lane: number, i: number, c: Clip, part: ClipPart) => {
    if (e.key === 'Enter' || e.key === ' ') {
      own(e);
      select(lane, i);
    } else if (ARROWS[e.key]) {
      own(e);
      const next = dragClip(c, part, ARROWS[e.key] * step(e), total, 0);
      if (next.start !== c.start || next.length !== c.length || next.offset !== c.offset) send(clipEdit(lane, i, next));
    }
  };
  const keyPoint = (e: KeyboardEvent, lane: number, i: number, l: Auto) => {
    const [pt, pv] = l.points[i];
    const dv = (l.high - l.low) / (e.shiftKey ? 100 : 20);
    const d = { ArrowLeft: [-step(e), 0], ArrowRight: [step(e), 0], ArrowUp: [0, dv], ArrowDown: [0, -dv] }[e.key];
    if (e.key === 'Delete' || e.key === 'Backspace') {
      own(e);
      send(`remove ${lane} ${i}`);
    } else if (e.key === 'Enter' || e.key === ' ') {
      own(e);
      select(lane, i);
    } else if (d) {
      own(e);
      const p = dragPoint(l.points, i, pt + d[0], pv + d[1], l.low, l.high, total, 0);
      if (p[0] !== pt || p[1] !== pv) send(`point ${lane} ${i} ${p.join(' ')}`);
    }
  };

  const { step: major, decimals } = ruler(total);
  const majors = Array.from({ length: Math.floor(total / major + 1e-6) + 1 }, (_, k) => round(k * major));
  const minors = Array.from({ length: Math.floor((total / major) * 5 + 1e-6) + 1 }, (_, k) => (k * major) / 5).filter((_, k) => k % 5 !== 0);
  const solo = t.lanes.some((l) => l.solo);
  const focus = disabled ? -1 : 0;
  const rows = t.lanes.length;
  const lines = (
    <svg viewBox={`0 0 ${total} 1`} preserveAspectRatio="none" aria-hidden="true">
      <path d={majors.map((m) => `M${m} 0V1`).join('')} />
    </svg>
  );
  return (
    <Titled text={w.text}>
      <div
        className="gui-timeline"
        onPointerDown={disabled ? undefined : down}
        onPointerMove={disabled ? undefined : move}
        onPointerUp={disabled ? undefined : up}
        onPointerCancel={() => setDrag(null)}
        onLostPointerCapture={() => setDrag(null)}
        onDoubleClick={disabled ? undefined : double}
      >
        <div className="tl-transport">
          <button className="btn gui-icon" aria-label="Play" title="Play" aria-pressed={t.playing} disabled={disabled} onClick={() => send('play 1')}>
            {guiIcon('play')}
          </button>
          <button className="btn gui-icon" aria-label="Pause" title="Pause" disabled={disabled} onClick={() => send('play 0')}>
            {guiIcon('pause')}
          </button>
          <button className="btn gui-icon" aria-label="Rewind" title="Rewind" disabled={disabled} onClick={() => send('seek 0')}>
            {guiIcon('rewind')}
          </button>
          <span className="tl-clock" aria-label="Time">
            {clock(playhead)}
          </span>
          <label className="tl-option">
            <input type="checkbox" checked={t.looping} disabled={disabled} onChange={(e) => send(`loop ${e.currentTarget.checked ? 1 : 0}`)} />
            Loop
          </label>
          <label className="tl-option">
            Snap
            <select className="field gui-field" value={String(t.snap)} disabled={disabled} onChange={(e) => send(`snap ${e.currentTarget.value}`)}>
              {[...new Set([...SNAPS, t.snap])]
                .sort((a, b) => a - b)
                .map((s) => (
                  <option key={s} value={String(s)}>
                    {s ? `${s} s` : 'Off'}
                  </option>
                ))}
            </select>
          </label>
        </div>
        <div className="tl-grid">
          <div className="tl-ruler">
            <div className="tl-area" data-ruler="">
              {majors.map((m, k) => (
                <span key={k} className="tl-label" style={{ left: pct(m) }}>
                  {m.toFixed(decimals) + (k === majors.length - 1 ? ' s' : '')}
                </span>
              ))}
              <svg className="tl-ticks" viewBox={`0 0 ${total} 2`} preserveAspectRatio="none" aria-hidden="true">
                <path d={majors.map((m) => `M${m} 0V2`).join('') + minors.map((m) => `M${m} 1V2`).join('')} />
              </svg>
              <div className={t.looping ? 'tl-loop on' : 'tl-loop'} style={{ left: pct(range[0]), width: pct(range[1] - range[0]) }}>
                <span className="tl-loop-label">{`Loop (${range[0].toFixed(1)} - ${range[1].toFixed(1)} s)`}</span>
              </div>
              {range.map((v, end) => (
                <div
                  key={end}
                  data-end={end}
                  className="tl-loop-end"
                  style={{ left: pct(v) }}
                  role="slider"
                  tabIndex={focus}
                  aria-label={`Loop ${end ? 'end' : 'start'}`}
                  aria-valuemin={0}
                  aria-valuemax={total}
                  aria-valuenow={v}
                  onKeyDown={disabled ? undefined : (e) => keyLoop(e, end ? 1 : 0)}
                />
              ))}
              <div
                className="tl-mark"
                style={{ left: pct(playhead) }}
                role="slider"
                tabIndex={focus}
                aria-label="Playhead"
                aria-valuemin={0}
                aria-valuemax={total}
                aria-valuenow={playhead}
                aria-valuetext={clock(playhead)}
                onKeyDown={disabled ? undefined : keySeek}
              />
            </div>
          </div>
          {rows > 0 && (
            <div className="tl-lines" style={{ gridRow: `2 / span ${rows}` }}>
              <div className="tl-inner">{lines}</div>
            </div>
          )}
          {t.lanes.map((l, li) => {
            const name = l.name || `Lane ${li + 1}`;
            const quiet = l.mute || (solo && !l.solo);
            return [
              <div key={`h${li}`} className={quiet ? 'tl-head quiet' : 'tl-head'} style={{ gridRow: li + 2 }}>
                <span className="tl-name" title={name}>
                  {name}
                </span>
                <button className="btn tl-ms" aria-pressed={l.mute} aria-label={`Mute ${name}`} title="Mute" disabled={disabled} onClick={() => send(`mute ${li} ${l.mute ? 0 : 1}`)}>
                  M
                </button>
                <button className="btn tl-ms" aria-pressed={l.solo} aria-label={`Solo ${name}`} title="Solo" disabled={disabled} onClick={() => send(`solo ${li} ${l.solo ? 0 : 1}`)}>
                  S
                </button>
              </div>,
              <div key={`t${li}`} className={`tl-track ${l.kind}${quiet ? ' quiet' : ''}`} style={{ gridRow: li + 2 }}>
                {l.kind === 'clips' ? (
                  <div className="tl-area" data-lane={li}>
                    {l.clips.map((source, i) => {
                      const c = clipAt(li, i, source);
                      const label = c.name || `Clip ${i + 1}`;
                      return [
                        <div
                          key={`c${i}`}
                          data-i={i}
                          className={isSelected(li, i) ? 'tl-clip selected' : 'tl-clip'}
                          style={{ left: pct(c.start), width: pct(c.length), '--clip': c.color } as CSSProperties}
                          role="button"
                          tabIndex={focus}
                          aria-pressed={isSelected(li, i)}
                          aria-label={`${label}: start ${c.start} s, length ${c.length} s`}
                          title="Drag to move, or press the left and right arrows"
                          onKeyDown={disabled ? undefined : (e) => keyClip(e, li, i, source, 'move')}
                        >
                          <span className="tl-clip-name">{c.name}</span>
                          <ClipWave columns={c.columns} />
                        </div>,
                        ...(['start', 'end'] as const).map((part) => (
                          <div
                            key={`${part}${i}`}
                            data-i={i}
                            data-part={part}
                            className="tl-trim"
                            style={{ left: pct(part === 'start' ? c.start : c.start + c.length) }}
                            role="slider"
                            tabIndex={focus}
                            aria-label={`${label} trim ${part}`}
                            aria-valuemin={0}
                            aria-valuemax={total}
                            aria-valuenow={part === 'start' ? c.start : round(c.start + c.length)}
                            onKeyDown={disabled ? undefined : (e) => keyClip(e, li, i, source, part)}
                          />
                        )),
                      ];
                    })}
                  </div>
                ) : (
                  <div className="tl-area" data-lane={li} title="Double click to add a point">
                    <svg className="tl-curve" viewBox={`0 0 ${total} 1`} preserveAspectRatio="none" aria-hidden="true">
                      <path className="fill" d={`${curvePath(l.points.map((_, i) => pointAt(li, i, l)), total, l.low, l.high)}V1H0Z`} />
                      <path className="trace" d={curvePath(l.points.map((_, i) => pointAt(li, i, l)), total, l.low, l.high)} />
                    </svg>
                    <span className="tl-axis high">{quantity(l.high, l.unit)}</span>
                    <span className="tl-axis low">{quantity(l.low, l.unit)}</span>
                    {l.points.map((_, i) => {
                      const [pt, pv] = pointAt(li, i, l);
                      const on = isSelected(li, i);
                      const at = { left: pct(pt), top: `${top(l, pv) * 100}%` };
                      return [
                        <div
                          key={`p${i}`}
                          data-i={i}
                          className={on ? 'tl-point selected' : 'tl-point'}
                          style={at}
                          role="button"
                          tabIndex={focus}
                          aria-pressed={on}
                          aria-label={`${name} point ${i + 1}: ${pt} s, ${quantity(pv, l.unit)}`}
                          title="Drag, or press the arrows. Double click or Delete removes it"
                          onKeyDown={disabled ? undefined : (e) => keyPoint(e, li, i, l)}
                        />,
                        on && (
                          <span key={`v${i}`} className="tl-tip" style={at}>
                            {quantity(pv, l.unit)}
                          </span>
                        ),
                      ];
                    })}
                  </div>
                )}
              </div>,
            ];
          })}
          <div className="tl-over" style={{ gridRow: `1 / span ${rows + 1}` }}>
            <div className="tl-inner">
              <div className="tl-playhead" style={{ left: pct(playhead) }} />
            </div>
          </div>
        </div>
      </div>
    </Titled>
  );
}

/** The waveform of a clip: the lowest and the highest sample of each column, from -1 to 1. */
function ClipWave({ columns }: { columns: [number, number][] }) {
  const n = columns.length;
  if (n === 0) return null;
  const y = (v: number) => (Number.isFinite(v) ? 1 - Math.min(Math.max(v, -1), 1) : 1);
  const points = [...columns.map(([, high], i) => `${i + 0.5} ${y(high)}`), ...columns.map(([low], i) => `${i + 0.5} ${y(low)}`).reverse()];
  return (
    <svg viewBox={`0 0 ${n} 2`} preserveAspectRatio="none" aria-hidden="true">
      <polygon className="wave" points={points.join(' ')} />
    </svg>
  );
}
