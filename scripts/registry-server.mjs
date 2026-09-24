import http from 'node:http';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { readFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const definitions=[
 {id:'al-coding',name:'编码规范',description:'在 ALCoding 自己的注册器中管理语言、架构和项目技能。',kind:'web',entry:'scripts/start_registry_ui.py',port:8765},
 {id:'al-spec',name:'规格规范',description:'打开 ALSpec 自己的注册文件进行调整；当前版本没有 Web 注册器。',kind:'file',entry:'framework/registry.yaml'},
 {id:'db-operator',name:'数据库连接',description:'打开 DB Operator 自己的连接管理器，注册、编辑、测试或删除连接。',kind:'web',entry:'scripts/open-registry.ps1',port:17842},
 {id:'al-ra',name:'需求分析',description:'该能力没有独立注册器，可打开使用说明。',kind:'document',entry:'SKILL.md'},
 {id:'al-knowledge',name:'项目知识',description:'按项目维护知识库，没有全局注册器，可打开使用说明。',kind:'document',entry:'SKILL.md'}
];
export function registrars(packageRoot=root){
 return definitions.map(d=>{
   const candidates=[join(packageRoot,'skills',d.id),join(packageRoot,'..',d.id)];
   const packagePath=candidates.find(p=>existsSync(join(p,d.entry)));
   return {...d,available:Boolean(packagePath),packagePath};
 });
}
function run(file,args,options={}){
 return new Promise((done,fail)=>{
  const child=spawn(file,args,{windowsHide:true,stdio:['ignore','pipe','pipe'],...options});
  let out='',err='';const timer=setTimeout(()=>{child.kill();fail(Error('启动超时，请确认授权窗口或管理器状态'));},60000);
  child.stdout?.on('data',x=>out+=x);child.stderr?.on('data',x=>err+=x);
  child.on('error',e=>{clearTimeout(timer);fail(e)});
  child.on('close',code=>{clearTimeout(timer);code===0?done(out):fail(Error(err||out||'启动失败'))});
 });
}
async function launch(item){
 const entry=join(item.packagePath,item.entry);
 if(item.kind!=='web'){
   const child=spawn('notepad.exe',[entry],{windowsHide:false,detached:true,stdio:'ignore'});
   await new Promise((done,fail)=>{child.once('spawn',done);child.once('error',fail)});child.unref();
   return {opened:true,message:item.kind==='file'?'已打开该能力的注册文件，请在编辑器中调整并保存。':'已打开该能力的使用说明。'};
 }
 const url='http://127.0.0.1:'+item.port+'/';
 try { const r=await fetch(url,{signal:AbortSignal.timeout(1000)});if(r.ok)return {url}; } catch {}
 if(item.id==='db-operator'){
   await run('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',entry,'-NoBrowser']);
 } else {
   let python=null;
   for(const candidate of [[process.env.AE_SDD_PYTHON||'python',[]],['py',['-3.13']],['py',['-3.14']]]){
     try{await run(candidate[0],[...candidate[1],'-c','import yaml']);python=candidate;break}catch{}
   }
   if(!python)throw Error('ALCoding 注册器需要可导入 PyYAML 的 Python；可通过 AE_SDD_PYTHON 指定现有环境。');
   const child=spawn(python[0],[...python[1],entry,'--root',item.packagePath,'--port',String(item.port),'--no-browser'],{cwd:item.packagePath,windowsHide:true,detached:true,stdio:'ignore'});
   await new Promise((done,fail)=>{child.once('spawn',done);child.once('error',fail)});child.unref();
 }
 for(let i=0;i<40;i++){
   try{const r=await fetch(url,{signal:AbortSignal.timeout(500)});if(r.ok)return {url};}catch{}
   await new Promise(done=>setTimeout(done,250));
 }
 throw Error('管理器未就绪，请检查依赖或 UAC 授权；未打开替代页面。');
}
export function createHub({packageRoot=root,open=launch}={}){
 const token=randomBytes(32).toString('hex');
 return http.createServer(async(req,res)=>{
  const json=(code,data)=>{res.writeHead(code,{'content-type':'application/json; charset=utf-8','cache-control':'no-store'});res.end(JSON.stringify(data))};
  const origin='http://127.0.0.1:'+req.socket.localPort;
  if(req.headers.host!==new URL(origin).host||(req.headers.origin&&req.headers.origin!==origin))return json(403,{error:'只允许本机页面访问'});
  try{
   if(req.method==='GET'&&req.url==='/'){res.writeHead(200,{'content-type':'text/html; charset=utf-8','cache-control':'no-store','x-frame-options':'DENY'});return res.end(readFileSync(join(packageRoot,'ui/index.html')))}
   if(req.method==='GET'&&req.url==='/api/session')return json(200,{token,service:'ae-sdd',version:2});
   if(req.headers['x-ae-sdd-ui-token']!==token)return json(403,{error:'请刷新管理页面'});
   const items=registrars(packageRoot);
   if(req.method==='GET'&&req.url==='/api/registrars')return json(200,{registrars:items.map(({packagePath,...item})=>item)});
   if(req.method==='POST'&&req.url.startsWith('/api/registrars/')){
     const id=req.url.slice('/api/registrars/'.length);const item=items.find(i=>i.id===id);
     if(!item?.available)return json(404,{error:'该能力没有可用的注册管理入口'});
     return json(200,await open(item));
   }
   if(req.method==='GET'&&req.url==='/api/status'){
     const result=JSON.parse(await run('codex',['plugin','list','--json']));
     const plugin=(result.installed||[]).find(x=>x.pluginId==='ae-sdd@personal');
     return json(200,{installed:Boolean(plugin),version:plugin?.version||null});
   }
   if(req.method==='POST'&&(req.url==='/api/install'||req.url==='/api/uninstall')){
     return json(200,{output:await run('codex',['plugin',req.url==='/api/install'?'add':'remove','ae-sdd@personal','--json'])});
   }
   json(404,{error:'接口不存在'});
  }catch(e){json(400,{error:e.message})}
 });
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const port=Number(process.env.AE_SDD_UI_PORT||17843);
 createHub().listen(port,'127.0.0.1',()=>console.log('ae-sdd UI: http://127.0.0.1:'+port+'/'));
}
