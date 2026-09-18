const statusEl = document.querySelector('#status')!;

async function retry() {
  try {
    const reply = await chrome.runtime.sendMessage({type: 'connect'});
    statusEl.textContent = reply.error ?? 'Connection requested. ContextSleep will report this profile automatically.';
  } catch (e) {
    statusEl.textContent = String(e);
  }
}

document.querySelector('#connect')!.addEventListener('click', () => { void retry(); });
void retry();