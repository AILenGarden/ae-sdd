import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { Script } from 'node:vm';
import { createRegistryServer, runAdmin, issueClientCapability, agentHomeDirectory, clientConfigFor, handOffClientConfig, elevation } from '../scripts/registry-server.mjs';
const root=resolve(import.meta.dirname,'..');
// Pin the target platform inside a test so the unix code paths are reachable from
// any host; the server reads the platform per call, not at import time.
async function onPlatform(platform,run){
 const original=process.platform;
 Object.defineProperty(process,'platform',{value:platform,configurable:true});
 try{return await run()}finally{Object.defineProperty(process,'platform',{value:original,configurable:true})}
}
test('registrar HTTP CRUD uses native admin with confirmation and preserves other connections',async()=>{
 const home=mkdtempSync(join(tmpdir(),'db-registrar-fixture-'));
 const previous=process.env.DB_OPERATOR_HOME;process.env.DB_OPERATOR_HOME=home;
 const server=createRegistryServer({manage: input=>runAdmin(input,join(root,'native/target/release/db-operator-admin.exe'))});
 await new Promise(done=>server.listen(0,'127.0.0.1',done));const url='http://127.0.0.1:'+server.address().port;
 try{
 const session=await(await fetch(url+'/api/session')).json();assert.equal(session.version,2);
 const headers={'content-type':'application/json','x-db-operator-ui-token':session.token};
 async function call(path,method='GET',value){const r=await fetch(url+path,{method,headers,body:value===undefined?undefined:JSON.stringify(value)});return {status:r.status,data:await r.json()}}
 assert.equal((await fetch(url+'/api/connections')).status,403);
 assert.equal((await fetch(url+'/api/session',{headers:{Origin:'https://untrusted.example'}})).status,403);
 const profile={engine:'mysql',host:'invalid.example',port:3306,database:'fixture',username:'fixture',ssl_mode:'require',environment:'test',tags:['fixture'],write_level:'none',connect_timeout:10,statement_timeout:30,max_rows:100};
 for(const name of ['fixture-a','fixture-b'])assert.equal((await call('/api/connections','POST',{name,profile,password:'temporary-fixture-only',test:false})).status,200);
 assert.equal((await call('/api/connections','POST',{name:'fixture-a',profile,password:'temporary-fixture-only',test:false})).status,400);
 assert.equal((await call('/api/connections')).data.connections.length,2);
 const shown=await call('/api/connections/fixture-a');assert.equal(shown.data.profile.database,'fixture');assert.ok(!JSON.stringify(shown).includes('temporary-fixture-only'));
 assert.equal((await call('/api/connections/fixture-a','PUT',{profile:{...profile,database:'updated'},password:'',test:false})).status,200);
 assert.equal((await call('/api/connections/fixture-a')).data.profile.database,'updated');
 assert.equal((await call('/api/connections/fixture-a','DELETE',{confirmed_name:'fixture-a'})).status,409);
 const pending=(await call('/api/connections/fixture-a/delete-confirmation','POST',{})).data;
 assert.equal((await call('/api/connections/fixture-a','DELETE',{...pending,confirmed_name:'wrong'})).status,409);
 assert.equal((await call('/api/connections/fixture-a','DELETE',{confirmation:pending.confirmation,confirmed_name:'fixture-a'})).status,200);
 assert.equal((await call('/api/connections/fixture-a','DELETE',{confirmation:pending.confirmation,confirmed_name:'fixture-a'})).status,409);
 const left=(await call('/api/connections')).data.connections;assert.deepEqual(left.map(x=>x.name),['fixture-b']);
 }finally{await new Promise(done=>server.close(done));if(previous===undefined)delete process.env.DB_OPERATOR_HOME;else process.env.DB_OPERATOR_HOME=previous;rmSync(home,{recursive:true,force:true})}
});
test('page has wired list edit confirmation issue and schema controls and valid script',()=>{
 const html=readFileSync(join(root,'ui/index.html'),'utf8');new Script(html.match(/<script>([\s\S]*?)<\/script>/)[1]);
 for(const id of ['connections','delete-dialog','confirm-name','confirm-delete','form-title','capability-status','issue-form','issue-connection','issue-endpoint','issue-output','issue-restart','issue-button','schema-refresh','issue-agent-user'])assert.ok(html.includes('id="'+id+'"'));
 assert.ok(html.includes('editConnection(c.name)'));assert.ok(html.includes('prepareDelete(c.name)'));assert.ok(html.includes("api('/api/capability/issue'"));assert.ok(html.includes("api('/api/schema-refresh'"));assert.ok(html.includes('updateCapability()'));
 assert.ok(html.includes('name="issue"'));assert.ok(html.includes('test,issue,...issueFields()'));assert.ok(html.includes('client.json 尚未生成'));
 assert.ok(html.includes('requires_agent_user'));assert.ok(html.includes("api('/api/defaults?agent='"));
});
test('capability issue and schema refresh routes validate input, sequence commands, and honor restart flag',async()=>{
 const calls=[];let restarts=0;const ep='\\\\.\\pipe\\db-operator-v2';
 const issued=async request=>{calls.push(request);if(request.connection==='fail-me')throw Error('boom');return request.output};
 const expectWriteLevel=async name=>name==='fixture-b'?'dml':'none';
 const server=createRegistryServer({manage:async input=>input.operation==='show'?{name:input.name,profile:{write_level:await expectWriteLevel(input.name)}}:{connections:[]},command:async argv=>{calls.push(argv);return {ok:true}},issue:issued,capability:()=>({active:true,connection:'fixture-a',client_path:'C:\\fixture\\client.json'}),restart:async()=>{restarts+=1}});
 await new Promise(done=>server.listen(0,'127.0.0.1',done));const url='http://127.0.0.1:'+server.address().port;
 try{
 const session=await(await fetch(url+'/api/session')).json();
 const headers={'content-type':'application/json','x-db-operator-ui-token':session.token};
 async function call(path,method='GET',value){const r=await fetch(url+path,{method,headers,body:value===undefined?undefined:JSON.stringify(value)});return {status:r.status,data:await r.json()}}
 const cap=await call('/api/capability');assert.equal(cap.data.connection,'fixture-a');assert.ok(cap.data.defaults.endpoint);assert.ok(cap.data.defaults.output);assert.ok(!JSON.stringify(cap.data).includes('digest'));
 assert.equal((await call('/api/schema-refresh','POST',{})).status,200);assert.deepEqual(calls.at(-1),['schema','refresh']);assert.equal(restarts,0);
 assert.equal((await call('/api/capability/issue','POST',{connection:'坏名字',endpoint:ep,output:'C:\\c.json'})).status,400);
 assert.equal((await call('/api/capability/issue','POST',{connection:'fixture-b',endpoint:'',output:'C:\\c.json'})).status,400);
 const issue=await call('/api/capability/issue','POST',{connection:'fixture-b',endpoint:ep,output:'C:\\Users\\a\\client.json',restart:true});
 assert.equal(issue.status,200);assert.equal(issue.data.replaced,'fixture-a');assert.equal(issue.data.service_restarted,true);assert.equal(issue.data.client_config,'C:\\Users\\a\\client.json');
 assert.deepEqual(calls.at(-1),{connection:'fixture-b',endpoint:ep,output:'C:\\Users\\a\\client.json',agentUser:'',writeLevel:'dml'});assert.equal(restarts,1);
 assert.equal(issue.data.write_level,'dml');
 const noRestart=await call('/api/capability/issue','POST',{connection:'fixture-c',endpoint:ep,output:'C:\\c.json',restart:false});
 assert.equal(noRestart.status,200);assert.equal(noRestart.data.service_restarted,false);assert.equal(restarts,1);
 const failed=await call('/api/capability/issue','POST',{connection:'fail-me',endpoint:ep,output:'C:\\c.json'});
 assert.equal(failed.status,400);assert.equal(failed.data.error,'boom');assert.equal(restarts,1);
 }finally{await new Promise(done=>server.close(done))}
});
test('connection creation can auto-issue the first capability and never rolls back registration',async()=>{
 const profile={engine:'mysql',host:'invalid.example',port:3306,database:'fixture',username:'fixture',ssl_mode:'require',environment:'test',tags:[],write_level:'none',connect_timeout:10,statement_timeout:30,max_rows:100};
 const ep='\\\\.\\pipe\\db-operator-v2';
 async function withServer({capability,issue},run){
  const manageCalls=[],issueCalls=[];let restarts=0;
  const server=createRegistryServer({manage:async input=>{manageCalls.push(input);if(input.operation==='show')return {name:input.name,profile:{write_level:'none'}};return {connection:input.name,saved:true,tested:false}},issue:async request=>{issueCalls.push(request);return issue(request)},capability,restart:async()=>{restarts+=1;return true}});
  await new Promise(done=>server.listen(0,'127.0.0.1',done));
  try{await run('http://127.0.0.1:'+server.address().port,{manageCalls,issueCalls,restarts:()=>restarts})}finally{await new Promise(done=>server.close(done))}
 }
 const headersFor=async url=>{const session=await(await fetch(url+'/api/session')).json();return {'content-type':'application/json','x-db-operator-ui-token':session.token}};
 const post=async(url,path,headers,value)=>{const r=await fetch(url+path,{method:'POST',headers,body:JSON.stringify(value)});return {status:r.status,data:await r.json()}};

 await withServer({capability:()=>({active:false,connection:null,client_path:null}),issue:async request=>request.output},async(url,spy)=>{
  const headers=await headersFor(url);
  const created=await post(url,'/api/connections',headers,{name:'first-conn',profile,password:'fixture-only',test:false,issue:true,output:'/tmp/agent/client.json'});
  assert.equal(created.status,200);assert.equal(created.data.issued,true);assert.equal(created.data.client_config,'/tmp/agent/client.json');assert.equal(created.data.service_restarted,true);
  assert.equal(spy.issueCalls.length,1);
  assert.deepEqual({connection:spy.issueCalls[0].connection,endpoint:spy.issueCalls[0].endpoint,output:spy.issueCalls[0].output,writeLevel:spy.issueCalls[0].writeLevel}, {connection:'first-conn',endpoint:ep,output:'/tmp/agent/client.json',writeLevel:'none'});
  assert.equal(created.data.write_level,'none');
  assert.equal(spy.restarts(),1);
  const updated=await fetch(url+'/api/connections/first-conn',{method:'PUT',headers,body:JSON.stringify({profile,password:'',test:false,issue:true})});
  assert.equal((await updated.json()).issued,undefined);
  assert.equal(spy.issueCalls.length,1);
 });

 await withServer({capability:()=>({active:true,connection:'other-conn',client_path:'C:\\other.json'}),issue:async request=>request.output},async(url,spy)=>{
  const headers=await headersFor(url);
  const created=await post(url,'/api/connections',headers,{name:'second-conn',profile,password:'fixture-only',test:false,issue:true});
  assert.equal(created.status,200);assert.equal(created.data.issued,undefined);assert.equal(created.data.issue_skipped,'capability-exists');assert.equal(created.data.existing_connection,'other-conn');
  assert.equal(spy.issueCalls.length,0);assert.equal(spy.restarts(),0);
 });

 await withServer({capability:()=>({active:false,connection:null,client_path:null}),issue:async()=>{throw Error('issue exploded')}},async(url,spy)=>{
  const headers=await headersFor(url);
  const created=await post(url,'/api/connections',headers,{name:'third-conn',profile,password:'fixture-only',test:false,issue:true});
  assert.equal(created.status,200);assert.equal(created.data.saved,true);assert.equal(created.data.issued,undefined);assert.equal(created.data.issue_error,'issue exploded');
  assert.equal(spy.manageCalls.filter(call=>call.operation==='create').length,1);
 });
});

test('unix issuance keeps the private copy and hands the client config to the agent account',async()=>{
 const calls=[];
 const command=async argv=>{calls.push(['admin',...argv]);return {ok:true}};
 const handOff=async({staged,output,agentUser})=>{calls.push(['handoff',staged,output,agentUser])};
 await onPlatform('darwin',async()=>{
  process.env.DB_OPERATOR_HOME='/var/lib/db-operator';
  try{
   const written=await issueClientCapability({connection:'fixture-a',endpoint:'/var/run/db-operator/db-operator.sock',output:'/Users/agent/Library/Application Support/db-operator/client.json',agentUser:'agent',writeLevel:'dml'},command,handOff);
   assert.equal(written,'/Users/agent/Library/Application Support/db-operator/client.json');
   assert.deepEqual(calls[0],['admin','client','issue','--connection','fixture-a','--endpoint','/var/run/db-operator/db-operator.sock','--output','/var/lib/db-operator/client.json','--write-level','dml']);
   assert.deepEqual(calls[1],['handoff','/var/lib/db-operator/client.json','/Users/agent/Library/Application Support/db-operator/client.json','agent']);
  }finally{delete process.env.DB_OPERATOR_HOME}
 });
});

test('unix handoff installs the client config for the agent account with restrictive modes',async()=>{
 const calls=[];
 const run=async(cmd,argv)=>{calls.push([cmd,...argv]);return cmd==='id'?'staff\n':''};
 await onPlatform('darwin',()=>handOffClientConfig({staged:'/var/lib/db-operator/client.json',output:'/Users/agent/Library/Application Support/db-operator/client.json',agentUser:'agent'},run));
 assert.deepEqual(calls[0],['id','-gn','agent']);
 assert.deepEqual(calls[1],['install','-d','-o','agent','-g','staff','-m','0700','/Users/agent/Library/Application Support/db-operator']);
 assert.deepEqual(calls[2],['install','-o','agent','-g','staff','-m','0600','/var/lib/db-operator/client.json','/Users/agent/Library/Application Support/db-operator/client.json']);
});

test('unix issuance refuses an empty agent account and never writes the client config directly',async()=>{
 const command=async()=>{throw Error('must not issue without an agent account')};
 await onPlatform('linux',async()=>{
  await assert.rejects(()=>issueClientCapability({connection:'fixture-a',endpoint:'/run/db-operator/db-operator.sock',output:'/home/agent/.config/db-operator/client.json',agentUser:''},command),/Agent 账号/);
 });
});

test('agent home discovery derives the platform client config path',async()=>{
 const seen=[];
 const run=async(cmd,argv)=>{seen.push([cmd,...argv]);return cmd==='dscl'?'NFSHomeDirectory: /Users/agent\n':'agent:x:501:20::/home/agent:/bin/sh\n'};
 await onPlatform('darwin',async()=>{
  const macHome=await agentHomeDirectory('agent',run);
  assert.equal(macHome,'/Users/agent');
  assert.equal(clientConfigFor(macHome),'/Users/agent/Library/Application Support/db-operator/client.json');
 });
 await onPlatform('linux',async()=>{
  const linuxHome=await agentHomeDirectory('agent',run);
  assert.equal(linuxHome,'/home/agent');
  assert.equal(clientConfigFor(linuxHome),'/home/agent/.config/db-operator/client.json');
 });
 assert.deepEqual(seen[0],['dscl','.','-read','/Users/agent','NFSHomeDirectory']);
 assert.deepEqual(seen[1],['getent','passwd','agent']);
});

test('unix session reports the platform and agent account requirement',async()=>{
 await onPlatform('darwin',async()=>{
  const server=createRegistryServer({manage:async()=>({connections:[]})});
  await new Promise(done=>server.listen(0,'127.0.0.1',done));
  try{
   const session=await(await fetch('http://127.0.0.1:'+server.address().port+'/api/session')).json();
   assert.equal(session.platform,'macos');assert.equal(session.requires_agent_user,true);
  }finally{await new Promise(done=>server.close(done))}
 });
});

test('unix admin commands run as the service identity through the server account runner',async()=>{
 const seen=[];
 const admin='/usr/local/libexec/db-operator/db-operator-admin';
 await onPlatform('darwin',async()=>{
  process.env.DB_OPERATOR_HOME='/var/lib/db-operator';
  try{
   const job=elevation(['list'],admin);
   assert.equal(job.command,'sudo');
   assert.deepEqual(job.argv,['-n','-u','_dboperator','/usr/bin/env','DB_OPERATOR_HOME=/var/lib/db-operator',admin,'list']);
  }finally{delete process.env.DB_OPERATOR_HOME}
 });
 await onPlatform('linux',async()=>{
  const job=elevation(['list'],admin);
  assert.equal(job.command,'runuser');
  assert.deepEqual(job.argv,['-u','db-operator','--','/usr/bin/env','DB_OPERATOR_HOME=/var/lib/db-operator',admin,'list']);
 });
 if(typeof process.getuid==='function')assert.ok(seen.length>=0);
});

test('non-root unix sessions refuse to drive the admin tool',async()=>{
 if(typeof process.getuid!=='function'||process.getuid()===0)return;
 await onPlatform('linux',()=>{
  assert.throws(()=>elevation(['list'],'/usr/local/libexec/db-operator/db-operator-admin'),/管理员权限/);
 });
});
