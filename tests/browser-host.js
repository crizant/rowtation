// Test-only NSH web host. Real File/FileList/DragEvent objects exercise the
// registered Rust browser adapter; this module is never shipped in the app.
export function createHost() {
  const pending = [];
  let pickStructured = false;
  const fixture = (name, text) => new File([text], name, { type: 'text/csv' });
  // Each slice the app reads waits for F3, so tests control when reads finish.
  const delayed = (name, text) => {
    const file = fixture(name, text);
    Object.defineProperty(file, 'slice', { value: (start, end) => {
      const blob = Blob.prototype.slice.call(file, start, end);
      Object.defineProperty(blob, 'arrayBuffer', { value: () => new Promise(resolve => {
        pending.push(() => resolve(Blob.prototype.arrayBuffer.call(blob)));
      }) });
      return blob;
    } });
    return file;
  };
  document.addEventListener('click', event => {
    const input = event.target;
    if (!(input instanceof HTMLInputElement) || input.id !== 'rowtation-file-input') return;
    event.preventDefault(); // Supply a synthetic choice instead of an OS dialog.
    const files = new DataTransfer();
    files.items.add(pickStructured ? fixture('picked.json', '[{"name":"Mia","n":900719925474099312345}]') : fixture('picked.csv', 'id,city,a,b,c,d,e\n00123,Brisbane,1,2,3,4,5\n00234,Sydney,6,7,8,9,10'));
    pickStructured = false;
    input.files = files.files;
    queueMicrotask(() => input.dispatchEvent(new Event('change', { bubbles: true })));
  }, true);
  document.addEventListener('keydown', event => {
    if (event.key === 'F11') { event.preventDefault(); pickStructured = true; return; }
    if (event.key === 'F3') {
      event.preventDefault();
      const read = pending.shift();
      if (!read) throw new Error('No pending file read to release');
      read();
      return;
    }
    if (!['F2', 'F4', 'F6', 'F7', 'F8', 'F9', 'F10', 'F12', 'F5'].includes(event.key)) return;
    event.preventDefault();
    const dataTransfer = new DataTransfer();
    if (event.key === 'F12') {
      dataTransfer.items.add(fixture('dropped.xml', '<rows><row id="001"><name>Hana</name></row></rows>'));
    } else if (event.key === 'F5') {
      dataTransfer.items.add(fixture('dropped.ndjson', '{"name":"Mia"}\n\n{"city":"Brisbane"}'));
    } else if (event.key === 'F2') {
      dataTransfer.items.add(delayed('delayed.csv', 'name\nDelayed'));
    } else if (event.key === 'F6') {
      // 9,000,003 bytes: past the 8 MiB confirmation threshold, read in one slice.
      dataTransfer.items.add(delayed('large.csv', 'id\n' + '7\n'.repeat(4_500_000)));
    } else if (event.key === 'F4') {
      dataTransfer.items.add(fixture('invalid-utf8.csv', new Uint8Array([255])));
    } else if (event.key === 'F7') {
      dataTransfer.items.add(fixture('unsupported.xlsx', 'unsupported'));
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
