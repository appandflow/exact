// Who is asking (LLP 1157 D7): which answers a token change withholds, and
// what a request sent across one carries. Frames marked "recorded" were sent
// by Convex's open-source backend (`npx convex dev`, 2026-11-12) to a client
// speaking 1.49.0's protocol, with a `customJwt` provider and `whoami`, a
// query that answers the token's subject.
import { expect, test } from 'bun:test';
import { Sync } from '../sync.ts';

const JSON_ONLY = { convexToJson: (value: unknown) => value, jsonToConvex: (json: unknown) => json };
const T0 = 'AAAAAAAAAAA=', T1 = 'UI2JvzdR5xg=', T2 = '0L3KzX1R5xg='; // recorded, in order
const version = (querySet: number, identity: number, ts: string) => ({ querySet, identity, ts });
const transition = (start: object, end: object, modifications: object[] = []) => JSON.stringify({ type: 'Transition', startVersion: start, endVersion: end, modifications });
const updated = (queryId: number, value: unknown) => ({ type: 'QueryUpdated', queryId, value, logLines: [], journal: null });
const settled = async (p: Promise<unknown>) => Promise.race([p, new Promise<'pending'>(r => setTimeout(() => r('pending'), 5))]);

/** A client asking `me:whoami` (query 0) and `me:constant` (query 1), connected with `token`, its results arrived. */
function connected(token?: string) {
  const sync = new Sync(JSON_ONLY);
  sync.connecting('session-1');
  if (token) sync.setAuth(token);
  sync.ask(0, 'me:whoami', {});
  sync.ask(0, 'me:constant', {});
  sync.opened();
  if (token) sync.frame(transition(version(0, 0, T0), version(0, 1, T1))); // recorded
  sync.frame(transition(version(0, token ? 1 : 0, token ? T1 : T0), version(1, token ? 1 : 0, T1), [updated(0, token ?? null), updated(1, 'same for everyone')])); // recorded
  return sync;
}
/** Both queries asked at the current `rev`, as a screen showing both asks them; each keeps the other alive. */
const both = (sync: Sync) => [sync.ask(sync.rev, 'me:whoami', {}).read(), sync.ask(sync.rev, 'me:constant', {}).read()];
const whoami = (sync: Sync) => both(sync)[0];
const constant = (sync: Sync) => both(sync)[1];

/** The socket closes, the token changes while it is closed, and a new socket opens: what it sends. */
function reconnect(sync: Sync, token: string | undefined) {
  sync.closed('the socket closed (1006)');
  sync.setAuth(token);
  sync.connecting('session-2');
  return sync.opened();
}

test('signing in across a reconnect gives none of the signed-out answers as the person\'s', () => {
  const sync = connected();
  expect(whoami(sync)).toEqual({ ok: true, value: null });
  expect(reconnect(sync, 'alice').map(f => f.type)).toEqual(['Connect', 'Authenticate', 'ModifyQuerySet']);
  expect(whoami(sync)).toBeUndefined();
  // The first Transition of a connection opened with a token moves the identity and names no query: the server
  // has no query yet. It confirms nothing about the answers held from the last connection.
  sync.frame(transition(version(0, 0, T0), version(0, 1, T1))); // recorded
  expect(both(sync)).toEqual([undefined, undefined]);
  sync.frame(transition(version(0, 1, T1), version(1, 1, T1), [updated(0, 'alice'), updated(1, 'same for everyone')])); // recorded
  expect(both(sync)).toEqual([{ ok: true, value: 'alice' }, { ok: true, value: 'same for everyone' }]);
});

test('one person to another across a reconnect gives none of the first\'s answers', () => {
  const sync = connected('alice');
  expect(whoami(sync)).toEqual({ ok: true, value: 'alice' });
  reconnect(sync, 'bob');
  sync.frame(transition(version(0, 0, T0), version(0, 1, T1)));
  expect(whoami(sync)).toBeUndefined();
  sync.frame(transition(version(0, 1, T1), version(1, 1, T1), [updated(0, 'bob'), updated(1, 'same for everyone')]));
  expect(whoami(sync)).toEqual({ ok: true, value: 'bob' });
});

test('signing out across a reconnect gives none of the person\'s answers as signed out', () => {
  const sync = connected('alice');
  expect(reconnect(sync, undefined).map(f => f.type)).toEqual(['Connect', 'ModifyQuerySet']);
  expect(both(sync)).toEqual([undefined, undefined]);
  sync.frame(transition(version(0, 0, T0), version(1, 0, T1), [updated(0, null), updated(1, 'same for everyone')])); // recorded
  expect(whoami(sync)).toEqual({ ok: true, value: null });
});

test('the same person across a reconnect keeps what the last connection delivered', () => {
  const sync = connected('alice');
  reconnect(sync, 'alice');
  expect(whoami(sync)).toEqual({ ok: true, value: 'alice' });
  sync.frame(transition(version(0, 0, T0), version(0, 1, T1)));
  expect(whoami(sync)).toEqual({ ok: true, value: 'alice' });
});

test('on one connection, a token change withholds every answer until Convex confirms it, then gives the ones it did not name', async () => {
  const sync = connected('alice');
  expect(sync.setAuth('bob')).toEqual([{ type: 'Authenticate', tokenType: 'User', value: 'bob', baseVersion: 1 }]);
  const asked = sync.ask(sync.rev, 'me:constant', {});
  expect([whoami(sync), asked.read()]).toEqual([undefined, undefined]);
  // A write elsewhere, computed for the person Convex still has: withheld too.
  sync.frame(transition(version(1, 1, T1), version(1, 1, T2), [updated(0, 'alice')]));
  expect(whoami(sync)).toBeUndefined();
  expect(await settled(asked.wait)).toBe('pending');
  sync.frame(transition(version(1, 1, T2), version(1, 2, T2), [updated(0, 'bob')])); // recorded, as Convex names only the changed
  await asked.wait;
  expect([whoami(sync), asked.read()]).toEqual([{ ok: true, value: 'bob' }, { ok: true, value: 'same for everyone' }]);
});

test('a token changed and changed back before Convex answered gives the answers held for it again', () => {
  const sync = connected();
  sync.setAuth('alice');
  expect(whoami(sync)).toBeUndefined();
  sync.setAuth(undefined);
  expect(whoami(sync)).toEqual({ ok: true, value: null });
});
