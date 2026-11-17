// A small modal question with one button for each answer. The first button has the focus.

export function Confirm({ title, label, answers, onCancel }: { title: string; label: string; answers: [string, () => void][]; onCancel?: () => void }) {
  return (
    <div className="backdrop" onKeyDown={(e) => e.key === 'Escape' && onCancel?.()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
        <h2 id="confirm-title">{title}</h2>
        <div className="label">{label}</div>
        <div className="actions">
          {answers.map(([text, run], i) => (
            <button key={text} className={i === 0 ? 'btn primary' : 'btn'} autoFocus={i === 0} onClick={run}>
              {text}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
