import test from 'node:test';
import assert from 'node:assert/strict';
import { classifySql, countStatements } from '../src/sql-risk.js';

test('classifies SQL risk', () => {
  assert.equal(classifySql('select * from todos'), 'READ');
  assert.equal(classifySql("insert into todos(text) values ('ship')"), 'DATA_WRITE');
  assert.equal(classifySql('create table todos(id bigint)'), 'SCHEMA_WRITE');
  assert.equal(classifySql('drop table todos'), 'DANGEROUS');
  assert.equal(classifySql('select 1; drop table todos;'), 'DANGEROUS');
});

test('counts statements', () => {
  assert.equal(countStatements('select 1; ; select 2;'), 2);
});
