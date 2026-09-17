const token = location.hash.slice(1);
void chrome.storage.local.get(`lazy:${token}`).then(data => { const item = data[`lazy:${token}`] as {
    title: string;
    url: string;
} | undefined; if (item) {
    document.title = item.title || 'Sleeping tab';
    document.querySelector('#title')!.textContent = item.title;
    document.querySelector('#url')!.textContent = item.url;
} });
document.querySelector('#wake')!.addEventListener('click', async () => { try {
    const r = await chrome.runtime.sendMessage({ type: 'wake', token });
    if (r?.error)
        throw new Error(r.error);
}
catch (e) {
    document.querySelector('#status')!.textContent = String(e);
} });
