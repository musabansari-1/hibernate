const index = new URL(location.href).searchParams.get('tab') ?? '1';
document.title = `Validation ${index} · Hibernate`;
document.querySelector('#title')!.textContent = `Workspace validation page ${index}`;
const buffer = new Uint8Array(8 * 1024 * 1024);
buffer.fill(37);
// Retain the touched buffer for an explicit, reproducible resident-memory workload.
(globalThis as unknown as {
    hibernateFixture: Uint8Array;
}).hibernateFixture = buffer;
document.querySelector('#status')!.textContent = '8 MiB test buffer allocated. Safe to close.';
