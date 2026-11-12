// The bundle check of package code (js/bake/src/__exact_portable.mjs `packageRefusals`): what it refuses, and where it
// warns of code from a string.
import { expect, test } from 'bun:test';
import { parseSync } from 'rolldown/utils';
import { packageRefusals } from '../src/__exact_portable.mjs';

const check = (code, lang) => {
  const warnings = [];
  const refused = packageRefusals(code, (at) => `x${at}`, (c, o) => parseSync(`x.${lang}`, c, o).program, warnings, lang);
  return { refused, warnings };
};

// The refusals of package JavaScript, each as where and what: the ones the check made before uses of `Function` and
// `eval` became warnings (#128), less those. Their scopes are the refusals' own model (`declares` without `full`),
// so a change to the warnings' model moves none of these; a change here is a change to what a build refuses.
const REFUSALS = [
  ["process.cwd();", ["1:1 process"]],
  ["const Function = globalThis.process; Function.cwd();", ["1:38 process"]],
  ["var Function = globalThis.process; Function.cwd();", ["1:16 process"]],
  ["function f() { const eval = globalThis.document; return eval.body; }", ["1:57 document"]],
  ["function f(process) { return process.cwd(); }", []],
  ["function f() { function g() { var process = {}; } return process.cwd(); }", ["1:58 process"]],
  ["class C { static { var process = {}; process.cwd(); } }", ["1:24 process", "1:38 process"]],
  ["class C { static { let process = {}; process.cwd(); } }", []],
  ["class C { static { var process = {}; } m() { return process.cwd(); } }", ["1:24 process", "1:53 process"]],
  ["try { 0; } catch (document) { document.body; }", []],
  ["for (const window of []) window.x;", []],
  ["for (var window of []) window.x;", []],
  ["out: { var require = () => 0; } require(name);", []],
  ["switch (1) { case 1: const navigator = {}; navigator.x; }", []],
  ["const f = (a = 1, location = a) => location.href;", []],
  ["const f = (a = location, location = 1) => a;", []],
  ["class C { read = (document) => document.body; }", []],
  ["class C { read = document.body; }", ["1:18 document"]],
  ["{ function process() {} process.cwd(); }", []],
  ["globalThis.document.body;", ["1:1 document"]],
  ["typeof window !== 'undefined' && window.x;", []],
  ["import(name);", ["1:1 import()"]],
  ["import('./x.js');", []],
  ["require(name);", ["1:1 require"]],
  ["require('x');", []],
  ["process.env.NODE_ENV;", []],
  ["const g = globalThis.document; g.body;", ["1:32 document"]],
  ["const d = typeof document !== 'undefined' ? document : null;", []],
  ["new Function('return 1');", []],
  ["eval('1'); globalThis.eval('1'); Function.prototype.bind;", []],
  ["const F = Function; new F('');", []],
  ["x instanceof Function;", []],
  ["var Function = 1; function f() { return Function; }", []],
  ["function eval2() { var eval = 1; } eval('x');", []],
  ["const { process } = globalThis; process.cwd();", []],
  ["let [document] = []; document.body;", []],
  ["(function document() { return document.body; })();", []],
  ["(class window { m() { return window.x; } });", []],
  ["export { process } from './x.js'; process.cwd();", ["1:35 process"]],
  ["import { Buffer } from 'buffer'; Buffer.from('');", []],
  ["label: for (;;) { const __dirname = 1; __dirname; break label; } __filename;", ["1:66 __filename"]],
];

test('package refusals stay as they were decided, whatever the warnings\' scopes', () => {
  const said = REFUSALS.map(([code]) => [code, check(code, 'js').refused.map((why) => why.replace(/^x:(\d+:\d+) .*?(?:'([\w()]+)'.*|computed name.*)$/, (_, at, name) => `${at} ${name ?? 'require'}`))]);
  expect(said).toEqual(REFUSALS);
});

// Where a use of `Function` is the global, and so warned: each row's code, then the lines of the uses warned. A scope
// that binds the name hides it there alone. TypeScript emits a namespace as a function, so its block is a `var` scope;
// its blocks see each other's exports, never their other names, and an enum, class or function merged with it adds
// nothing to its scope. What TypeScript erases (`declare`, a type-only namespace) hides nothing.
const LOCAL = '(_: string) => 0';
const SCOPES = [
  ['a function body\'s var', `function f() { var Function = ${LOCAL}; return Function(''); }\nFunction('');`, [2]],
  ['a nested function\'s var stays there', `function f() { function g() { var Function = ${LOCAL}; } return Function(''); }`, [1]],
  ['the module\'s var', `var Function = ${LOCAL};\nFunction('');`, []],
  ['a class static block\'s var', `class C { static { var Function = ${LOCAL}; Function(''); } }\nFunction('');`, [2]],
  ['a class static block\'s let', `class C { static { let Function = ${LOCAL}; Function(''); } }\nFunction('');`, [2]],
  ['a catch parameter', `try { 0; } catch (Function: any) { Function(''); }\nFunction('');`, [2]],
  ['a for head\'s const', `for (const Function of [${LOCAL}]) Function('');\nFunction('');`, [2]],
  ['a for head\'s var, the function\'s', `function f() { for (var Function of [${LOCAL}]) 0; return Function(''); }\nFunction('');`, [2]],
  ['a labelled block\'s const', `out: { const Function = ${LOCAL}; Function(''); }\nFunction('');`, [2]],
  ['a switch block\'s const', `switch (0) { case 0: const Function = ${LOCAL}; Function(''); }\nFunction('');`, [2]],
  ['an arrow\'s parameter, a default naming an earlier one', `const f = (a = 1, Function = (_: string) => a) => Function('');\nFunction('');`, [2]],
  ['a class field\'s arrow parameter', `class C { read = (Function: (s: string) => number) => Function(''); }\nFunction('');`, [2]],
  ['a class field\'s initializer', `class C { read = Function(''); }`, [1]],
  ...['var', 'let', 'const'].flatMap((kind) => ['', 'export '].map((exported) =>
    [`a namespace's ${exported}${kind}`, `namespace N { ${exported}${kind} Function = ${LOCAL}; Function(''); }\nFunction('');`, [2]])),
  ...['', 'export '].flatMap((exported) => [
    [`a namespace's ${exported}function`, `namespace N { ${exported}function Function(_: string) { return 0; } Function(''); }\nFunction('');`, [2]],
    [`a namespace's ${exported}class`, `namespace N { ${exported}class Function { constructor(_: string) {} } new Function(''); }\nFunction('');`, [2]],
  ]),
  ['a labelled var in a nested block of a namespace', `namespace N { out: { { var Function = ${LOCAL}; } } Function(''); }\nFunction('');`, [2]],
  ['a merged namespace\'s export', `namespace N { export const Function = ${LOCAL}; }\nnamespace N { Function(''); }\nFunction('');`, [3]],
  ['a merged namespace\'s own name stays in its block', `namespace N { const Function = ${LOCAL}; }\nnamespace N { Function(''); }`, [2]],
  ['a dotted namespace sees its parent\'s exports', `namespace A { export const Function = ${LOCAL}; }\nnamespace A.B { Function(''); }\nFunction('');`, [3]],
  ['a nested namespace it does not export is not the dotted one', `namespace A { namespace B { export const Function = ${LOCAL}; } }\nnamespace A.B { Function(''); }`, [2]],
  ['an enum\'s member, in a namespace merged with it', `enum N { Function = 1 }\nnamespace N { Function(''); }`, [2]],
  ['a namespace merged with an enum', `enum N { A = 1 }\nnamespace N { export const Function = ${LOCAL}; }\nnamespace N { Function(''); }`, []],
  ['a namespace merged with a class', `class N { static Function = 1; }\nnamespace N { Function(''); }`, [2]],
  ['a type-only namespace', `namespace Function { export type T = number; }\nFunction('');`, [2]],
  ['declare global', `declare global { var Function: (s: string) => number; }\nFunction('');\nexport {};`, [2]],
  ['import X = require()', `import Function = require('x');\nFunction('');`, []],
  ['import X = N.Y', `namespace N { export const Y = ${LOCAL}; }\nimport Function = N.Y;\nFunction('');`, []],
  ['a const enum', `const enum Function { A }\nFunction('');`, [2]],
];

test('a use of Function is warned where no scope binds the name at run time', () => {
  const said = SCOPES.map(([scope, code]) => [scope, check(code, 'ts').warnings.map((w) => Number(/^x:(\d+):/.exec(w)[1]))]);
  expect(said).toEqual(SCOPES.map(([scope, , lines]) => [scope, lines]));
});

test('destructuring a global reads its code constructor at the property', () => {
  const cases = [
    ["const { Function: compile } = globalThis; compile(source);", ['1:9 Function']],
    ["const { eval: execute } = self; execute(source);", ['1:9 eval']],
    ["const { Function } = globalThis; Function(source);", ['1:9 Function']],
    ["let compile; ({ Function: compile } = globalThis);", ['1:17 Function']],
    ["const { ['Function']: compile, [`eval`]: execute } = globalThis;", ['1:10 Function', '1:33 eval']],
    ["function run({ Function: compile } = globalThis) { return compile(source); }", ['1:16 Function']],
    ["const { Function: compile = fallback } = (globalThis);", ['1:9 Function']],
    ["const { 'Function': compile } = globalThis;", ['1:9 Function']],
    ["const { nested: { Function: compile } } = globalThis;", []],
    ["const { [key]: compile } = globalThis;", []],
    ["const { Function: compile } = local;", []],
    ["function run(globalThis) { const { Function: compile } = globalThis; }", []],
    ["function run(self) { const { eval: execute } = self; }", []],
    ["type T = { Function: typeof Function };", []],
  ];
  for (const [code, sites] of cases) {
    const { refused, warnings } = check(code, 'ts');
    expect([code, refused, warnings.map(w => w.replace(/^x:(\d+:\d+) warning \[code-from-string\] uses '([^']+)'.*$/, '$1 $2'))])
      .toEqual([code, [], sites]);
  }
});

test('parameter initializers see parameters and outer bindings before the body scope', () => {
  const cases = [
    ['function run(make = Function) { const Function = 1; return make(source); }', 1],
    ['function run(make = Function) { var Function = 1; return make(source); }', 1],
    ['function run(make = Function) { function Function() {} return make(source); }', 1],
    ['function run(make = Function) { class Function {} return make(source); }', 1],
    ['const run = (make = Function) => { let Function = 1; return make(source); };', 1],
    ['function run(make = eval) { const eval = 1; return make(source); }', 1],
    ['function run(make = globalThis.Function) { var globalThis = {}; }', 1],
    ['function run({ Function: make } = globalThis) { var globalThis = {}; }', 1],
    ['function run(make = () => Function(source)) { var Function = 1; }', 1],
    ['function run(Function, make = Function) { return make(source); }', 0],
    ['function run(make = Function, Function = local) { return make(source); }', 0],
    ['function run({ Function }, make = Function) { return make(source); }', 0],
    ['const run = function Function(make = Function) { return make(source); };', 0],
    ['const Function = local; function run(make = Function) { var Function = 1; }', 0],
    ['function outer() { var Function = local; function run(make = Function) { var Function = 1; } }', 0],
    ['function run(make = local) { { var Function = local; } return Function(source); }', 0],
    ['function run(make = local) { return () => Function(source); var Function; }', 0],
    ['function run(make: typeof Function = local) { const Function = 1; }', 0],
    ['function run(make = typeof Function) { var Function; }', 0],
    ['function run(make = typeof globalThis.Function) { var globalThis; }', 0],
    ['function run(make = typeof (globalThis as any).eval) { var globalThis; }', 0],
  ];
  for (const [code, count] of cases) expect([code, check(code, 'ts').warnings.length]).toEqual([code, count]);
});

test('JSX component names refer to values once, while tags and attributes do not', () => {
  const cases = [
    ['<Function />;', 1],
    ['<Function></Function>;', 1],
    ['<Function.Component></Function.Component>;', 1],
    ['<globalThis.Function />;', 1],
    ['<self.eval />;', 1],
    ['Function.prototype.bind;', 0],
    ['self.eval.prototype.bind;', 0],
    ['<Function.prototype.bind />;', 0],
    ['<self.eval.prototype.bind />;', 0],
    ['<globalThis.Function.prototype.call></globalThis.Function.prototype.call>;', 0],
    ['<eval.prototype.apply />;', 0],
    ['Function.prototype.constructor;', 1],
    ['self.eval.prototype.constructor;', 1],
    ['<Function.prototype.constructor />;', 1],
    ['<self.eval.prototype.constructor />;', 1],
    ['<eval />;', 0],
    ['<Function:tag />;', 0],
    ['<div Function="eval" />;', 0],
    ['<Function Function={Function} />;', 2],
    ['<components.Function />;', 0],
    ['const Function = local; <Function />;', 0],
    ['function run(Function) { return <Function />; }', 0],
    ['function run(globalThis) { return <globalThis.Function />; }', 0],
    ['import type { Function } from "x"; <Function />;', 1],
    ['type Function = () => void; <Function />;', 1],
  ];
  for (const lang of ['jsx', 'tsx']) {
    for (const [code, count] of cases) {
      if (lang === 'jsx' && /^(import type|type )/.test(code)) continue;
      expect([lang, code, check(code, lang).warnings.length]).toEqual([lang, code, count]);
    }
  }
});
