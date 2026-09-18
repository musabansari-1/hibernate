import type {Memory} from '../../../../packages/core/model';
import {gb, gbExact, signedGb} from '../lib/format';

const RADIUS = 52;
const CIRCUMFERENCE = Math.PI * RADIUS; // half circle

/**
 * Available system memory as Windows reports it. ContextSleep never attributes
 * a share of this to a single workspace, so the gauge is labelled as a system reading.
 */
export function MemoryGauge({memory, delta}: {memory?: Memory; delta?: number | null}) {
  // The arc shows what is still available, so the green length matches the headline number.
  const availableFraction = memory ? memory.availablePhysical / memory.totalPhysical : 0;
  const offset = CIRCUMFERENCE * (1 - availableFraction);
  return <div className="memory-gauge">
    <div className="gauge-visual">
      <svg viewBox="0 0 128 74" role="img" aria-label={memory ? `${Math.round(availableFraction * 100)} percent of system memory available` : 'System memory reading unavailable'}>
        <path d="M12 66a52 52 0 0 1 104 0" className="gauge-base" />
        <path d="M12 66a52 52 0 0 1 104 0" className="gauge-value"
          strokeDasharray={CIRCUMFERENCE} strokeDashoffset={offset} />
      </svg>
      <div className="gauge-readout">
        <strong>{memory ? gb(memory.availablePhysical) : '--'}</strong>
        <small>available</small>
      </div>
    </div>
    <dl className="gauge-facts">
      <div><dt>In use</dt><dd>{memory ? gb(memory.totalPhysical - memory.availablePhysical) : '--'}</dd></div>
      <div><dt>Installed</dt><dd>{memory ? gb(memory.totalPhysical) : '--'}</dd></div>
    </dl>
    {delta != null && <p className={`gauge-delta ${delta >= 0 ? 'is-up' : 'is-down'}`}>
      <strong>{signedGb(delta)}</strong> across the last sleep
    </p>}
    <small className="gauge-note">Measured by Windows across the whole machine, not claimed as exclusive workspace savings.</small>
  </div>;
}

/** Before/after reading for one operation, phrased as a system observation. */
export function MemoryDelta({before, after, children}: {before: Memory; after: Memory; children?: React.ReactNode}) {
  const change = after.availablePhysical - before.availablePhysical;
  return <div className="memory-delta">
    <div className="delta-head">
      <span className="delta-label">System available memory</span>
      <strong className={change >= 0 ? 'is-up' : 'is-down'}>{signedGb(change)}</strong>
    </div>
    <div className="delta-scale">
      <span><small>before</small><strong>{gbExact(before.availablePhysical)}</strong></span>
      <i aria-hidden="true" />
      <span><small>after</small><strong>{gbExact(after.availablePhysical)}</strong></span>
    </div>
    <p className="delta-note">System available memory changed by {signedGb(change)} during this operation. Other applications and shared Windows memory also move this number, so it is not an exclusive workspace figure.</p>
    {children}
  </div>;
}
