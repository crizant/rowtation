// Test-only NSH web host. Real File/FileList/DragEvent objects exercise the
// registered Rust browser adapter; this module is never shipped in the app.
export function createHost() {
  const fixture = (name, text) => new File([text], name, { type: 'text/csv' });
  document.addEventListener('click', event => {
    const input = event.target;
    if (!(input instanceof HTMLInputElement) || input.id !== 'rowtation-file-input') return;
    event.preventDefault(); // Supply a synthetic choice instead of an OS dialog.
    const files = new DataTransfer();
    files.items.add(fixture('picked.csv', 'id,city,a,b,c,d,e\n00123,Brisbane,1,2,3,4,5\n00234,Sydney,6,7,8,9,10'));
    input.files = files.files;
    queueMicrotask(() => input.dispatchEvent(new Event('change', { bubbles: true })));
  }, true);
  document.addEventListener('keydown', event => {
    if (!['F7', 'F8', 'F9', 'F10'].includes(event.key)) return;
    event.preventDefault();
    const dataTransfer = new DataTransfer();
    if (event.key === 'F7') {
      dataTransfer.items.add(fixture('unsupported.json', '{"name":"Mia"}'));
    } else if (event.key === 'F10') {
      dataTransfer.items.add(fixture('broken.csv', 'name,city\n"unclosed'));
    } else {
      dataTransfer.items.add(fixture('dropped.tsv', 'name\tcity\nHana\tMelbourne'));
      if (event.key === 'F9') dataTransfer.items.add(fixture('second.csv', 'a\nb'));
    }
    document.dispatchEvent(new DragEvent('drop', { dataTransfer, bubbles: true, cancelable: true }));
  }, true);
  return { call() { throw new Error('No application host calls expected'); } };
}
