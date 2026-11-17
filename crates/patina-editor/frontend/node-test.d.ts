// The parts of `node:test` and `node:assert/strict` that the tests use, so `tsc` checks the
// tests without a download of `@types/node`.

declare module 'node:test' {
  export function test(name: string, fn: () => void | Promise<void>): Promise<void>;
}

declare module 'node:assert/strict' {
  function ok(value: unknown, message?: string): asserts value;
  function equal<T>(actual: unknown, expected: T, message?: string): asserts actual is T;
  function notEqual(actual: unknown, expected: unknown, message?: string): void;
  function deepEqual<T>(actual: unknown, expected: T, message?: string): asserts actual is T;
  function match(value: string, regExp: RegExp, message?: string): void;
  const assert: {
    ok: typeof ok;
    equal: typeof equal;
    notEqual: typeof notEqual;
    deepEqual: typeof deepEqual;
    match: typeof match;
  };
  export default assert;
}
