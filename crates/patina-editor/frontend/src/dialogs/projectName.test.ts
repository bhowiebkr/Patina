import assert from 'node:assert/strict';
import { test } from 'node:test';
import { projectFolder, projectNameProblem } from './projectName.ts';

test('a project name uses letters, digits, spaces and - _ . only', () => {
  for (const name of ['', 'synth', 'My Synth 2', 'a-b_c.d']) assert.equal(projectNameProblem(name), null, name);
  for (const name of ['.hidden', 'end.', 'end ', 'a/b', 'a:b', 'caf\u00e9', 'x'.repeat(65)]) assert.notEqual(projectNameProblem(name), null, name);
});

test('the new folder uses the separator of the location', () => {
  assert.equal(projectFolder('C:\\Projects', 'synth'), 'C:\\Projects\\synth');
  assert.equal(projectFolder('C:\\', 'synth'), 'C:\\synth');
  assert.equal(projectFolder('/home/me', 'synth'), '/home/me/synth');
});
