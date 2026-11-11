// index.d.ts declares what index.ts implements (LLP 1157), read as an app's
// checker reads it, through the package's `types` export. Each pair is one
// type, as `Same` compares them: a member either side lacks, an optional one
// too, fails the check, which one-way assignability would let by.
import type * as declared from '@exact/convex';
import * as implemented from '../index.ts';

type Same<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
const same = <A, B>(proof: Same<A, B>): Same<A, B> => proof;

export const checks = [
  same<declared.ConvexLive, implemented.ConvexLive>(true),
  same<declared.ConvexFunctions, implemented.ConvexFunctions>(true),
  same<declared.ConvexOptions, implemented.ConvexOptions>(true),
  same<declared.ConvexClient, implemented.ConvexClient>(true),
  same<declared.ConvexFailure, implemented.ConvexFailure>(true),
  same<typeof declared.convex, typeof implemented.convex>(true),
];
