import test from 'node:test';
import assert from 'node:assert/strict';
import { resolve, join } from 'node:path';
import { readFileSync } from 'node:fs';
import { Script } from 'node:vm';
import { createHub,registrars } from '../scripts/registry-server.mjs';
test('hub discovers owned registrars and delegates opening without connection CRUD',async()=>{
 const packageRoot=resolve(import.meta.dirname,'..');const list=registrars(packageRoot);
 assert.equal(list.length,5);assert.ok(list.every(x=>x.available));assert.equal(list.find(x=>x.id==='db-operator').kind,'web');
 const opened=[];const server=createHub({packageRoot,open:async item=>{opened.push(item.id);return {opened:true}}});
 await new Promise(done=>server.listen(0,'127.0.0.1',done));const url='http://127.0.0.1:'+server.address().port;
 try{
 const {token}=await(await fetch(url+'/api/session')).json();const headers={'x-ae-sdd-ui-token':token};
 const items=await(await fetch(url+'/api/registrars',{headers})).json();assert.equal(items.registrars.length,5);
 assert.equal((await fetch(url+'/api/registrars/db-operator',{method:'POST',headers})).status,200);assert.deepEqual(opened,['db-operator']);
 assert.equal((await fetch(url+'/api/registrars/unknown',{method:'POST',headers})).status,404);
 assert.equal((await fetch(url+'/api/connections',{headers})).status,404);
 }finally{await new Promise(done=>server.close(done))}
 const html=readFileSync(join(packageRoot,'ui/index.html'),'utf8');new Script(html.match(/<script>([\s\S]*?)<\/script>/)[1]);assert.ok(html.includes('openRegistrar(item,button)'));
});
