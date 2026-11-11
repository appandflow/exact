// A stand-in for a Convex deployment, for an app's tests (LLP 1157 §6): the
// server half of the sync protocol over functions given as plain JavaScript
// on in-memory state, so `bun exact.mjs test` runs with no Convex backend,
// from empty data, on this machine. It answers what `convex@1.49.0`'s
// `src/browser/sync/protocol.ts` declares and a backend was recorded sending
// (test/sync.test.ts): a `Transition` per query-set change, a
// `MutationResponse` and then a `Transition` to every session whose results
// the mutation changed. It checks no token and runs no Convex code: the
// app's real functions are proven against Convex itself.
//
// An app's server.ts:
//   import { serve } from '@exact/convex/test/server.ts';
//   serve({ port: Number(process.env.PORT), queries: { 'board:get': (args) => … }, mutations: { … } });

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
type Handler = (args: any) => Json | void;
export interface Functions {
  /** The port to listen on, on 127.0.0.1. */
  port: number;
  /** By path (`module:function`): a query's value for its arguments, read from the caller's own state. */
  queries: Record<string, Handler>;
  /** A mutation changes that state and returns its value; one that throws is refused with the error's message. */
  mutations?: Record<string, Handler>;
  actions?: Record<string, Handler>;
}

type Version = { querySet: number; identity: number; ts: string };
type Session = { version: Version; queries: Map<number, { udfPath: string; args: Json; last: string }>; send: (message: object) => void; idle: ReturnType<typeof setInterval> | undefined };

/** A timestamp as the wire has it: a u64, little-endian, in base64. */
export const wireTs = (n: number) => { const b = new Uint8Array(8); new DataView(b.buffer).setBigUint64(0, BigInt(n), true); return btoa(String.fromCharCode(...b)); };

export function serve({ port, queries, mutations = {}, actions = {} }: Functions) {
  const sessions = new Set<Session>();
  let clock = 1;
  const run = (udfPath: string, args: Json): { type: 'QueryUpdated'; value: Json } | { type: 'QueryFailed'; errorMessage: string } => {
    const query = queries[udfPath];
    if (!query) return { type: 'QueryFailed', errorMessage: `Could not find public function for '${udfPath}'.` };
    try { return { type: 'QueryUpdated', value: (query(args) ?? null) as Json }; }
    catch (e) { return { type: 'QueryFailed', errorMessage: String((e as Error).message ?? e) }; }
  };
  /** A session's `Transition` to `end`: each of `ids`' results, and every other query's that changed. */
  const transition = (s: Session, end: Partial<Version>, ids: number[] = [], removed: number[] = [], always = true) => {
    const modifications: object[] = removed.map(queryId => ({ type: 'QueryRemoved', queryId }));
    for (const [queryId, q] of s.queries) {
      const result = run(q.udfPath, q.args), text = JSON.stringify(result);
      if (text === q.last && !ids.includes(queryId)) continue;
      q.last = text;
      modifications.push({ ...result, queryId, logLines: [], journal: null });
    }
    if (!modifications.length && !always) return;
    const startVersion = s.version;
    s.version = { ...s.version, ...end };
    s.send({ type: 'Transition', startVersion, endVersion: s.version, modifications });
  };
  const server = Bun.serve<Session, never>({
    hostname: '127.0.0.1', port,
    fetch(request, bun) {
      if (new URL(request.url).pathname.endsWith('/sync') && bun.upgrade(request, { data: { version: { querySet: 0, identity: 0, ts: wireTs(0) }, queries: new Map(), send: () => {}, idle: undefined } })) return undefined;
      return new Response('A Convex stand-in for tests (@exact/convex/test/server.ts).');
    },
    websocket: {
      open(ws) {
        const s = ws.data;
        s.send = message => { ws.send(JSON.stringify(message)); };
        // Convex pings a connection that has been quiet for 15 seconds.
        s.idle = setInterval(() => s.send({ type: 'Ping' }), 15000);
        sessions.add(s);
      },
      close(ws) { clearInterval(ws.data.idle); sessions.delete(ws.data); },
      message(ws, text) {
        const s = ws.data, m = JSON.parse(String(text));
        if (m.type === 'ModifyQuerySet') {
          const added: number[] = [], removed: number[] = [];
          for (const mod of m.modifications) {
            if (mod.type === 'Add') { s.queries.set(mod.queryId, { udfPath: mod.udfPath, args: mod.args[0] ?? {}, last: '' }); added.push(mod.queryId); }
            else if (s.queries.delete(mod.queryId)) removed.push(mod.queryId);
          }
          transition(s, { querySet: m.newVersion, ts: wireTs(clock) }, added, removed);
        } else if (m.type === 'Mutation' || m.type === 'Action') {
          const kind = m.type === 'Mutation' ? 'MutationResponse' : 'ActionResponse';
          const handler = (m.type === 'Mutation' ? mutations : actions)[m.udfPath];
          let result: Json = null;
          try {
            if (!handler) throw new Error(`Could not find public function for '${m.udfPath}'.`);
            result = (handler(m.args[0] ?? {}) ?? null) as Json;
          } catch (e) {
            s.send({ type: kind, requestId: m.requestId, success: false, result: String((e as Error).message ?? e), logLines: [] });
            return;
          }
          const ts = wireTs(++clock);
          s.send({ type: kind, requestId: m.requestId, success: true, result, logLines: [], ...(m.type === 'Mutation' ? { ts } : {}) });
          for (const other of sessions) transition(other, { ts }, [], [], other === s);
        } else if (m.type === 'Authenticate') {
          transition(s, { identity: m.baseVersion + 1, ts: wireTs(clock) });
        }
      },
    },
  });
  return { stop: () => server.stop(true), port: server.port };
}
