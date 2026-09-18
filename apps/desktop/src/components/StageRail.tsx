import {Icon} from './ui';

export type StageKey = 'connect' | 'capture' | 'sleep' | 'restore' | 'ready';
export type StageState = 'done' | 'current' | 'todo';
export type Stage = {key: StageKey; label: string; hint: string; state: StageState};

/**
 * The whole product story, always visible: connect -> capture -> sleep -> restore -> ready.
 * State comes from observed data only; a step turns green when the engine recorded it.
 */
export function StageRail({stages, onSelect}: {stages: Stage[]; onSelect(key: StageKey): void}) {
  const doneCount = stages.filter(s => s.state === 'done').length;
  const progress = stages.length > 1 ? (doneCount / (stages.length - 1)) * 100 : 0;
  return <nav className="stage-rail" aria-label="Workspace lifecycle">
    <div className="stage-track" aria-hidden="true"><div className="stage-track-fill" style={{width: `${Math.min(100, progress)}%`}} /></div>
    <ol className="stage-list">
      {stages.map(stage => <li key={stage.key} className={`stage-step is-${stage.state}`}>
        <button type="button" onClick={() => onSelect(stage.key)} aria-current={stage.state === 'current' ? 'step' : undefined}>
          <span className="stage-node">{stage.state === 'done' ? <Icon name="check" /> : <i className="stage-pip" />}</span>
          <span className="stage-copy">
            <strong>{stage.label}</strong>
            <small>{stage.hint}</small>
          </span>
        </button>
      </li>)}
    </ol>
  </nav>;
}
