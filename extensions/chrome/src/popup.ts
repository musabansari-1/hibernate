document.querySelector('#connect')!.addEventListener('click', async () => {
    const status = document.querySelector('#status')!;
    try {
        const reply = await chrome.runtime.sendMessage({ type: 'connect' });
        status.textContent = reply.error ?? 'Connection requested. Check the desktop integration status.';
    }
    catch (e) {
        status.textContent = String(e);
    }
});
