/** Keeps separate clicks out of the same undo group, even between two display frames. */
export class ViewInput {
  private pending: { line: string; gesture: number }[] = [];
  private drag: { widget: string; gesture: number } | null = null;
  /** The newest place of each viewport. A place is a state, so an old place never goes out (15.8). */
  private places = new Map<string, string>();

  push(line: string, next: () => number) {
    const [widget, action] = line.split(' ');
    if (action === 'place') {
      this.places.set(widget, line);
      return;
    }
    const continuing = (action === 'move' || action === 'up') && this.drag?.widget === widget;
    const gesture = continuing ? this.drag!.gesture : next();
    if (action === 'down') this.drag = { widget, gesture };
    if (action === 'up') this.drag = null;
    this.pending.push({ line, gesture });
  }

  take() {
    const gesture = this.pending[0]?.gesture;
    let count = 0;
    while (count < this.pending.length && this.pending[count].gesture === gesture) count++;
    const lines = [...this.places.values(), ...this.pending.splice(0, count).map((e) => e.line)];
    this.places.clear();
    return { events: lines.join('\n'), gesture };
  }
}
