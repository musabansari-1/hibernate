/* ContextSleep lifecycle illustration.
   Purely presentational: it shows the shape of the product loop with example numbers.
   Every claim it renders matches what the product actually does. */
(function () {
  var screen = document.querySelector('.demo-screen');
  if (!screen) return;

  var badge = document.querySelector('[data-demo-badge]');
  var memory = document.querySelector('[data-demo-memory]');
  var bar = document.querySelector('[data-demo-bar]');
  var note = document.querySelector('[data-demo-note]');
  var caption = document.querySelector('[data-demo-caption]');
  var buttons = Array.prototype.slice.call(document.querySelectorAll('.demo-steps button'));
  var states = {
    vscode: document.querySelector('[data-demo-state="vscode"]'),
    chrome: document.querySelector('[data-demo-state="chrome"]'),
    folder: document.querySelector('[data-demo-state="folder"]')
  };

  var FRAMES = [
    {
      badge: 'Active workspace', badgeTone: 'neutral',
      rows: {vscode: ['Open', 'neutral'], chrome: ['Open', 'neutral'], folder: ['Open', 'neutral']},
      memory: '5.4 GB', bar: 34,
      note: 'Windows reports 5.4 GB of 15.8 GB available.',
      caption: 'A workspace you are done with for now: an editor, a browser window and a folder.'
    },
    {
      badge: 'Capturing', badgeTone: 'info',
      rows: {vscode: ['Saved', 'good'], chrome: ['Saved', 'good'], folder: ['Saved', 'good']},
      memory: '5.4 GB', bar: 34,
      note: 'Context saved locally. Nothing has closed.',
      caption: 'Capture writes a durable local checkpoint first. Nothing closes at this step.'
    },
    {
      badge: 'Review', badgeTone: 'warn',
      rows: {vscode: ['Safe to sleep', 'good'], chrome: ['Safe to sleep', 'good'], folder: ['Kept open', 'warn']},
      memory: '5.4 GB', bar: 34,
      note: 'Explorer is context restore only, so it is left open.',
      caption: 'You approve each window. Anything ContextSleep cannot safely identify stays open.'
    },
    {
      badge: 'Asleep', badgeTone: 'info',
      rows: {vscode: ['Closed', 'sleep'], chrome: ['Closed', 'sleep'], folder: ['Kept open', 'warn']},
      memory: '7.3 GB', bar: 46,
      note: 'System available memory changed by +1.90 GB during this sleep.',
      caption: 'Approved windows are rechecked, closed, and confirmed to have disappeared.'
    },
    {
      badge: 'Restoring', badgeTone: 'info',
      rows: {vscode: ['Reopening', 'info'], chrome: ['Reopening', 'info'], folder: ['Open', 'neutral']},
      memory: '6.9 GB', bar: 44,
      note: 'Chrome relaunches into the enrolled profile; VS Code reopens the saved workspace.',
      caption: 'Restore rebuilds the workspace from the checkpoint you captured.'
    },
    {
      badge: 'Ready', badgeTone: 'ready',
      rows: {vscode: ['Verified', 'good'], chrome: ['21 of 21 tabs', 'good'], folder: ['Open', 'neutral']},
      memory: '6.8 GB', bar: 43,
      note: '4 of 4 capabilities verified by observation.',
      caption: 'READY is a conclusion from observed evidence, not a finished progress bar.'
    }
  ];

  var index = 0;
  var timer = null;
  var reduced = window.matchMedia('(prefers-reduced-motion: reduce)');

  function render(i) {
    var f = FRAMES[i];
    index = i;
    screen.setAttribute('data-frame', String(i));
    badge.textContent = f.badge;
    badge.setAttribute('data-tone', f.badgeTone);
    memory.textContent = f.memory;
    bar.style.width = f.bar + '%';
    note.textContent = f.note;
    caption.textContent = f.caption;
    Object.keys(states).forEach(function (key) {
      var el = states[key];
      if (!el) return;
      el.textContent = f.rows[key][0];
      el.setAttribute('data-tone', f.rows[key][1]);
      var row = el.closest('li');
      if (row) row.setAttribute('data-tone', f.rows[key][1]);
    });
    buttons.forEach(function (b, n) {
      b.setAttribute('aria-selected', n === i ? 'true' : 'false');
      b.parentElement.classList.toggle('is-active', n === i);
      b.parentElement.classList.toggle('is-past', n < i);
    });
  }

  function stop() {
    if (timer) {window.clearInterval(timer); timer = null;}
  }

  function start() {
    stop();
    if (reduced.matches) return;
    timer = window.setInterval(function () {render((index + 1) % FRAMES.length);}, 2800);
  }

  buttons.forEach(function (b, n) {
    b.addEventListener('click', function () {stop(); render(n);});
    b.addEventListener('keydown', function (event) {
      if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
      event.preventDefault();
      stop();
      var next = (n + (event.key === 'ArrowRight' ? 1 : FRAMES.length - 1)) % FRAMES.length;
      render(next);
      buttons[next].focus();
    });
  });

  var demo = document.getElementById('demo');
  demo.addEventListener('mouseenter', stop);
  demo.addEventListener('focusin', stop);

  render(0);

  if ('IntersectionObserver' in window) {
    new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {entry.isIntersecting ? start() : stop();});
    }, {threshold: 0.35}).observe(demo);
  } else {
    start();
  }

  reduced.addEventListener('change', function () {reduced.matches ? stop() : start();});
})();
