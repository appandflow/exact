// @exact/convex's public API (LLP 1157), for the type checker and for reading:
// `index.ts` implements it, and `test/types.ts`, given to the type checker by a
// case in `test/sync.test.ts`, confirms the two agree. The recipe is
// docs/recipes/convex.md.

/** A Convex function's failure: `message` is Convex's, `data` a `ConvexError`'s payload. */
export declare class ConvexFailure extends Error {
  data: unknown;
  constructor(message: string, data?: unknown);
}

/** What `live()` answers; `ConvexLive` in Contract. */
export interface ConvexLive {
  /** "connecting" until the socket opens, "open" until the first results arrive, "ready" from then; "closed" once it
   *  has ended (refresh it to connect again), or "failed" when it ended on something a new connection would meet too. */
  status: string;
  /** Rises when a query's answer may have changed. Every query resource takes it as an argument. */
  rev: number;
  /** Rises with every frame Convex sends, its 15-second `Ping` included. */
  beat: number;
  /** Why the socket ended; "" otherwise. */
  error: string;
  /** Convex's words when it refused the token; "" otherwise. */
  authError: string;
}

/** Convex's own pure functions, which the app imports from its `convex` and hands over: `getFunctionName` from
 *  `convex/server`, `convexToJson` and `jsonToConvex` from `convex/values`. The module imports no package itself,
 *  so the app has one copy of `convex` and the wire's encoding is Convex's. */
export interface ConvexFunctions { getFunctionName(reference: any): string; convexToJson(value: any): any; jsonToConvex(json: any): any }

// A generated function reference (`api.messages.list`), as `convex/server` types one.
type Reference<Kind extends string> = { _type: Kind; _args: any; _returnType: any };
type ArgsOf<R extends Reference<string>> = R['_args'];
type ReturnOf<R extends Reference<string>> = R['_returnType'];

export interface ConvexOptions {
  /** The largest frame Convex may send, in bytes; the host's stream ceiling (1 MiB) when absent. */
  maxMessageBytes?: number;
}

export interface ConvexClient {
  /** The `live` source's answer: the socket, as a stream. One root resource asks it; refreshing that resource connects again. */
  live(): Promise<ConvexLive>;
  /** A query's answer at `rev` (`live.rev`): now, when the socket has delivered one, so a changed `rev` shows every
   *  query's new result in one commit; else once it arrives. `shape` maps Convex's value to the resource's shape, in
   *  the same turn. A failed query throws `ConvexFailure`, and so does one asked while the socket is closed. */
  query<Q extends Reference<'query'>, T = ReturnOf<Q>>(rev: number, ref: Q, args: ArgsOf<Q>, shape?: (value: ReturnOf<Q>) => T): T | Promise<T>;
  /** A mutation: its result once the queries show its write. It rejects with `ConvexFailure` when Convex refuses it,
   *  when the socket is not open (nothing is sent), and when the socket closes first (it may have run). */
  mutation<M extends Reference<'mutation'>>(ref: M, args: ArgsOf<M>): Promise<ReturnOf<M>>;
  /** An action: its result. It rejects as a mutation does. */
  action<A extends Reference<'action'>>(ref: A, args: ArgsOf<A>): Promise<ReturnOf<A>>;
  /** The person's token (a JWT the deployment is configured to accept), or `undefined` signed out. Call it from a mutation's source. */
  setAuth(token: string | undefined): void;
  /** The queries Convex is asked for and the requests in flight, for a test or a log. */
  inspect(): { queries: { id: number; udfPath: string; sent: boolean; answered: boolean }[]; requests: number };
}

/** A client for the deployment at `address` (its `CONVEX_URL`), over `functions`, Convex's own. */
export declare function convex(address: string, functions: ConvexFunctions, options?: ConvexOptions): ConvexClient;
