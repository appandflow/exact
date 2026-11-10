// A stream's mapper that returns a promise (Charlie, 2026-11-10): the order
// its calls settle in, a rejection, the stream's end behind a pending call,
// a send from a later turn, and inflating Discord's zlib-stream frame by
// frame with DecompressionStream.
//
// A text frame is `<verb> <yields> <label>`: the mapper notes `start`, awaits
// `scheduler.yield()` that many times, then notes `end` (`yield`), throws
// (`reject`), or sends on its socket before its first `await` and after its
// last (`send`). Its value is the log.

type Socket = { send(data: string): void };
type Event = { type: string; data: string | ArrayBuffer; code?: number; reason?: string; kind?: string; message?: string };

const log: string[] = [];
const joined = (chunks: Uint8Array[]) => {
  const all = new Uint8Array(chunks.reduce((n, c) => n + c.byteLength, 0));
  let at = 0;
  for (const c of chunks) { all.set(c, at); at += c.byteLength; }
  return all;
};

// One zlib stream across frames, each ending in a sync flush: what one frame
// yields is what is queued once its write settles (pure-streams.ts says why).
function inflater() {
  const ds = new DecompressionStream("deflate");
  const writer = ds.writable.getWriter(), reader = ds.readable.getReader();
  let pending = reader.read();
  const NOTHING = {};
  return async (packet: Uint8Array) => {
    await writer.write(packet);
    const out: Uint8Array[] = [];
    for (;;) {
      const got = await Promise.race([pending, Promise.resolve(NOTHING)]);
      if (got === NOTHING) break;
      const { value, done } = got as ReadableStreamReadResult<Uint8Array>;
      if (done) break;
      out.push(value);
      pending = reader.read();
    }
    return new TextDecoder().decode(joined(out));
  };
}
let zlib: ((packet: Uint8Array) => Promise<string>) | null = null;

// A whole deflate payload's inflated length, written `slice` bytes at a
// time with a yield between writes: each write is a turn of its own.
async function inflated(bytes: Uint8Array, slice: number) {
  const ds = new DecompressionStream("deflate"), writer = ds.writable.getWriter(), reader = ds.readable.getReader();
  let total = 0;
  const reading = (async () => { for (let x = await reader.read(); !x.done; x = await reader.read()) total += x.value.byteLength; })();
  for (let at = 0; at < bytes.length; at += slice) {
    await writer.write(bytes.subarray(at, at + slice));
    if (at + slice < bytes.length) await scheduler.yield();
  }
  await writer.close();
  await reading;
  return total;
}

async function map(id: string, e: Event, socket?: Socket): Promise<string> {
  if (e.type === "open") return "open";
  if (e.type === "close") { await scheduler.yield(); log.push("close " + e.code); return log.join(" "); }
  if (e.type === "error") return "error " + e.message;
  if (typeof e.data !== "string") {
    const bytes = new Uint8Array(e.data);
    if (id === "zlib") { zlib ??= inflater(); return await zlib(bytes); }
    return "inflated " + await inflated(bytes, id === "split" ? 65536 : bytes.length);
  }
  const [verb, yields, label] = e.data.split(" ");
  log.push("start " + label);
  if (verb === "send") socket!.send("before " + label);
  for (let i = 0; i < Number(yields); i++) await scheduler.yield();
  if (verb === "reject") throw new Error("rejected " + label);
  if (verb === "send") socket!.send("after " + label);
  if (verb === "fetch") await fetch("https://mapper.test/").catch(e => log.push((e as Error).message));
  if (verb === "timer") try { setTimeout(() => {}, 1); } catch (e) { log.push((e as Error).message); }
  log.push("end " + label);
  return log.join(" ");
}

function feed(id: string): unknown {
  return fetch("wss://mapper.test/" + id, { exactStream: (e: Event, socket?: Socket) => map(id, e, socket) } as RequestInit);
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.mapper",
  grants: "net.websocket wss://mapper.test\n",
  answer(source: string, args: unknown[]) {
    if (source === "feed") return feed(String(args[0]));
    throw new Error("unknown source " + source);
  },
};
