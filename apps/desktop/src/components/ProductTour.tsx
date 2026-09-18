import {Icon} from './ui';

export function ProductTour({onClose}: {onClose(): void}) {
  return <div className="tour-backdrop" role="presentation" onMouseDown={e => {if (e.currentTarget === e.target) onClose();}}>
    <section className="tour-dialog" role="dialog" aria-modal="true" aria-labelledby="tour-title">
      <header className="tour-head">
        <div>
          <p className="eyebrow">Full 60-second demo</p>
          <h2 id="tour-title">See the whole ContextSleep loop</h2>
          <p>Watch a disposable Chrome + VS Code workspace get saved, slept, restored and verified.</p>
        </div>
        <button type="button" className="btn btn-icon" aria-label="Close walkthrough" onClick={onClose}><Icon name="close" /></button>
      </header>
      <video className="tour-video" controls autoPlay playsInline preload="metadata" src="/contextsleep-demo.mp4">
        Your browser cannot play the ContextSleep walkthrough video.
      </video>
      <div className="tour-legend" aria-label="Walkthrough steps">
        <span><b>1</b> Connect</span>
        <span><b>2</b> Save</span>
        <span><b>3</b> Review</span>
        <span><b>4</b> Sleep</span>
        <span><b>5</b> Restore</span>
        <span><b>6</b> Verify</span>
      </div>
    </section>
  </div>;
}
