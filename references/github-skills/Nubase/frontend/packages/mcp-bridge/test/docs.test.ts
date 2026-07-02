import test from 'node:test';
import assert from 'node:assert/strict';
import { fetchDocs } from '../src/docs.js';

test('fetchDocs returns bundled topics', () => {
  const all = fetchDocs('all');
  assert.equal('docs' in all, true);
  if (!('docs' in all)) throw new Error('expected all docs');
  assert.ok(all.topics.includes('memory'));
  assert.ok(all.docs.memory.includes('memory_context'));
});

test('fetchDocs reports unknown topics', () => {
  const result = fetchDocs('missing') as { error: string };
  assert.match(result.error, /Unknown topic/);
});
