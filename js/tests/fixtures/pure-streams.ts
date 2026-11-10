// Streams and Compression Streams, run in Hermes (js/src/streams.js) and in
// Chrome by the same bundle (tests/it/pure.rs): the order a write, a read and
// a transform settle in, piping, teeing, iteration, and the codecs' bytes and
// errors. Output chunking is the implementation's, so only joined bytes are
// compared.

const bytes = (b64: string) => Uint8Array.from(atob(b64), c => c.charCodeAt(0));
const joined = (chunks: Uint8Array[]) => {
  const all = new Uint8Array(chunks.reduce((n, c) => n + c.byteLength, 0));
  let at = 0;
  for (const c of chunks) { all.set(c, at); at += c.byteLength; }
  return all;
};
const text = (chunks: Uint8Array[]) => new TextDecoder().decode(joined(chunks));
const errorName = (e: unknown) => (e instanceof TypeError ? 'TypeError' : e instanceof RangeError ? 'RangeError' : String((e as Error)?.name ?? e));
const settles = (p: Promise<unknown>) => p.then(v => 'ok ' + JSON.stringify(v ?? null), errorName);

// Discord's zlib-stream: one zlib stream across messages, each ending in a sync flush.
const PACKETS = [
  'eJyqVsovULIyNNBRSlGyqlbKSE0sKklKTSyJz8wrSS0qS8xRsjIxNDI1qK0FAAAA//8=',
  'ZNM9akMxFEThrQTVKTwz/u8M8QbSBeMiEGMMSZo4lXl7tzAGDXc6oXNV6EO6PSb74LVt2/t+9/bRnofO/5fvr7+2PdzapW+0WQ+/nz+nvnykl1mbXp8NtWE01sbRVJtGm9c2H21R22K0ZW3L0Va1rUZb17YebVPbxu4eMHCZpDEbBA5MB8ED80EAwYQQRDAjBBJMCcEEc0JAwaQQVDArhhXNimFFf0f5kMyKYUWzYljRrBhWNCuGFc2KYUWzYljRrBhWNCuFlcxKYSWzUljJf11+O7NSWMmsFFYyK4WVzEphJbNSWMmsFFZ9ZzpO0x0AAP//',
  'ghRghoa1AAAAAP//',
];
const DEFLATE = 'eJzLSM3JyVdISU3LSSxJBQAjDAUK', RAW = 'y0jNyclXKEosBwA=', GZIP = 'H4sIAAAAAAAAE8tIzcnJV0ivyiwAABlq0t8KAAAA';

// What one packet yields: write it, then read what is queued, keeping one
// read pending for the next packet. After a write settles, its transform has
// run and its output is queued (or fulfilled the pending read), so a read
// that does not settle at once means there is no more.
function inflater() {
  const ds = new DecompressionStream('deflate');
  const writer = ds.writable.getWriter(), reader = ds.readable.getReader();
  let pending = reader.read();
  const NOTHING = {};
  return async (packet: Uint8Array) => {
    await writer.write(packet);
    const out: Uint8Array[] = [];
    for (;;) {
      const got = await Promise.race([pending, Promise.resolve(NOTHING)]);
      if (got === NOTHING) return text(out);
      const { value, done } = got as ReadableStreamReadResult<Uint8Array>;
      if (done) return text(out);
      out.push(value);
      pending = reader.read();
    }
  };
}

async function decompress(format: CompressionFormat, input: Uint8Array) {
  const stream = new ReadableStream({ start(c) { c.enqueue(input); c.close(); } }).pipeThrough(new DecompressionStream(format));
  const out: Uint8Array[] = [];
  for await (const chunk of stream) out.push(chunk);
  return text(out);
}

async function roundTrip(format: CompressionFormat, value: string) {
  const packed: Uint8Array[] = [];
  const cs = new CompressionStream(format), w = cs.writable.getWriter();
  w.write(new TextEncoder().encode(value)); w.close();
  const r = cs.readable.getReader();
  for (let x = await r.read(); !x.done; x = await r.read()) packed.push(x.value);
  return decompress(format, joined(packed));
}

async function order() {
  const log: string[] = [];
  const ts = new TransformStream<number, number>({ transform(c, ctl) { log.push('transform ' + c); ctl.enqueue(c * 2); } });
  const w = ts.writable.getWriter(), r = ts.readable.getReader();
  log.push('desired ' + w.desiredSize);
  const w1 = w.write(1).then(() => log.push('wrote 1'));
  const w2 = w.write(2).then(() => log.push('wrote 2'));
  log.push('desired ' + w.desiredSize);
  w.ready.then(() => log.push('ready'));
  const r1 = r.read().then(x => log.push('read ' + x.value));
  await Promise.all([w1, r1]);
  const r2 = r.read().then(x => log.push('read ' + x.value));
  await Promise.all([w2, r2]);
  const closing = w.close().then(() => log.push('closed'));
  const end = r.read().then(x => log.push('done ' + x.done));
  await Promise.all([closing, end]);
  return log;
}

async function pipes() {
  const seen: string[] = [];
  const upper = new TransformStream<string, string>({ transform(c, ctl) { ctl.enqueue(c.toUpperCase()); }, flush(ctl) { ctl.enqueue('!'); } });
  const source = new ReadableStream<string>({ start(c) { ['a', 'b', 'c'].forEach(x => c.enqueue(x)); c.close(); } });
  await source.pipeThrough(upper).pipeTo(new WritableStream({ write(c) { seen.push(c); }, close() { seen.push('closed'); } }));
  const [one, two] = new ReadableStream<number>({ start(c) { c.enqueue(1); c.enqueue(2); c.close(); } }).tee();
  const all = async (s: ReadableStream<number>) => { const out: number[] = []; for await (const x of s) out.push(x); return out; };
  const teed = await Promise.all([all(one), all(two)]);
  // A sink that fails: the pipe rejects, and the source is cancelled with that reason.
  let cancelled = '';
  const failing = new ReadableStream({ pull(c) { c.enqueue(1); }, cancel(r) { cancelled = String(r); } })
    .pipeTo(new WritableStream({ write() { throw 'sink broke'; } }));
  const failed = await failing.then(() => 'resolved', e => String(e));
  // Pulled with backpressure: a high-water mark of 2 pulls twice before any read.
  let pulls = 0;
  const pulled = new ReadableStream({ pull(c) { pulls++; c.enqueue(pulls); } }, new CountQueuingStrategy({ highWaterMark: 2 }));
  await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  const before = pulls;
  const reader = pulled.getReader();
  const first = (await reader.read()).value;
  await reader.cancel();
  return { seen, teed, failed, cancelled, before, first, locked: pulled.locked, size: new ByteLengthQueuingStrategy({ highWaterMark: 8 }).size(new Uint8Array(3)) };
}

async function codecs() {
  const inflate = inflater();
  const packets: string[] = [];
  for (const p of PACKETS) packets.push(await inflate(bytes(p)));
  const formats = [await decompress('deflate', bytes(DEFLATE)), await decompress('deflate-raw', bytes(RAW)), await decompress('gzip', bytes(GZIP))];
  const trips = [];
  for (const f of ['deflate', 'deflate-raw', 'gzip'] as CompressionFormat[]) trips.push(await roundTrip(f, 'round ' + f + ' '.repeat(40) + 'trip'));
  // The standard's errors: corrupt input, data after the end, input that ends early, a chunk that is not bytes.
  const corrupt = await settles(decompress('deflate', new Uint8Array([1, 2, 3, 4, 5])));
  const trailing = await settles(decompress('deflate', new Uint8Array([...bytes(DEFLATE), 0])));
  const short = bytes(DEFLATE).subarray(0, 8);
  const ds = new DecompressionStream('deflate'), w = ds.writable.getWriter(), r = ds.readable.getReader();
  const reading = (async () => { const out: Uint8Array[] = []; for (let x = await r.read(); !x.done; x = await r.read()) out.push(x.value); return text(out); })();
  const wrote = await settles(w.write(short));
  const closed = await settles(w.close());
  const truncated = [wrote, closed, await settles(reading)];
  // (a write waits for a read: the transform runs only once the readable side wants a chunk)
  const gz = new DecompressionStream('gzip'), gzRead = settles(gz.readable.getReader().read());
  const notBytes = [await settles(gz.writable.getWriter().write('text' as never)), await gzRead];
  let format = '';
  try { new DecompressionStream('lzma' as CompressionFormat); } catch (e) { format = errorName(e); }
  return { packets, formats, trips, corrupt, trailing, truncated, notBytes, format };
}

export async function streams(): Promise<string> {
  return JSON.stringify({ order: await order(), pipes: await pipes(), codecs: await codecs() });
}
