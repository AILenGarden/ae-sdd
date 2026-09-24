#!/usr/bin/env python3
"""Local browser UI for the ALCoding SKILL registry.

The server is intentionally small and local-only by default. It stores uploaded
SKILL bytes as opaque files and delegates all registry mutations to registry.py.
"""

from __future__ import annotations

import argparse
import json
import sys
from email import policy
from email.parser import BytesParser
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, unquote, urlparse

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

from registry import (  # noqa: E402
    RegistryError,
    build_index,
    register_uploaded_skill,
    replace_uploaded_skill,
    update_skill,
    remove_skill,
    load_registry,
    _find_entry,
    _metadata_from_json,
)


UI_HTML = r"""<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>ALCoding SKILL Registry</title>
  <style>
    :root { color-scheme: dark; --bg:#0b1020; --panel:#121a2d; --panel2:#17223a; --line:#293754; --text:#edf3ff; --muted:#94a4c4; --accent:#67d6c0; --danger:#ff7d8c; --warning:#f6c76f; }
    * { box-sizing:border-box; }
    body { margin:0; min-height:100vh; background:radial-gradient(circle at 20% -10%,#1d3554 0,transparent 42%),var(--bg); color:var(--text); font:14px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif; }
    header { border-bottom:1px solid var(--line); background:rgba(11,16,32,.82); backdrop-filter:blur(12px); position:sticky; top:0; z-index:2; }
    .header-inner { max-width:1180px; margin:auto; padding:20px 24px; display:flex; align-items:center; justify-content:space-between; gap:20px; }
    h1 { margin:0; font-size:22px; letter-spacing:.2px; } h1 span { color:var(--accent); }
    .subtitle { color:var(--muted); margin:4px 0 0; }
    main { max-width:1180px; margin:0 auto; padding:28px 24px 52px; display:grid; grid-template-columns:minmax(0,1fr) 390px; gap:24px; }
    .panel { background:rgba(18,26,45,.92); border:1px solid var(--line); border-radius:10px; box-shadow:0 18px 45px rgba(0,0,0,.18); }
    .panel-head { padding:18px 20px; border-bottom:1px solid var(--line); display:flex; align-items:center; justify-content:space-between; gap:12px; }
    .panel-head h2 { margin:0; font-size:16px; } .count { color:var(--muted); font-size:12px; }
    .registry { padding:16px; display:grid; gap:16px; }
    .category { border:1px solid var(--line); border-radius:8px; overflow:hidden; }
    .category-title { padding:10px 12px; background:var(--panel2); color:var(--accent); font-weight:650; text-transform:capitalize; }
    .skill { padding:14px 12px; border-top:1px solid var(--line); display:flex; align-items:flex-start; justify-content:space-between; gap:16px; }
    .skill:first-of-type { border-top:0; } .skill-main { min-width:0; } .skill-name { font-weight:650; } .skill-id { color:var(--muted); font:12px ui-monospace,SFMono-Regular,Consolas,monospace; margin-left:7px; }
    .skill-desc { color:var(--muted); margin:5px 0 0; overflow-wrap:anywhere; } .skill-path { color:#b6c5e1; font:12px ui-monospace,SFMono-Regular,Consolas,monospace; margin-top:7px; overflow-wrap:anywhere; }
    .status { display:inline-flex; align-items:center; gap:6px; color:var(--muted); font-size:12px; white-space:nowrap; } .status::before { content:""; width:7px; height:7px; border-radius:50%; background:var(--warning); } .status.ok::before { background:var(--accent); } .status.off::before { background:#65728f; }
    .actions { display:flex; gap:6px; flex-wrap:wrap; justify-content:flex-end; } button { border:1px solid var(--line); border-radius:6px; background:#1b2945; color:var(--text); padding:7px 10px; cursor:pointer; } button:hover { border-color:var(--accent); } button.primary { background:#236b67; border-color:#3ba99d; } button.danger { color:#ffd5da; border-color:#713b49; background:#3b202e; }
    form { padding:20px; display:grid; gap:14px; } label { display:grid; gap:6px; color:#c4d0e7; font-weight:600; } input, select, textarea { width:100%; border:1px solid var(--line); border-radius:6px; background:#0e1628; color:var(--text); padding:9px 10px; font:inherit; } textarea { min-height:90px; resize:vertical; } input[type=file] { padding:8px; } .hint { color:var(--muted); font-size:12px; font-weight:400; }
    .form-actions { display:flex; justify-content:flex-end; gap:8px; padding-top:4px; } .empty { color:var(--muted); text-align:center; padding:26px 12px; }
    #toast { position:fixed; right:22px; bottom:22px; max-width:360px; padding:12px 14px; border-radius:8px; background:#20314f; border:1px solid var(--line); box-shadow:0 12px 30px rgba(0,0,0,.25); opacity:0; transform:translateY(8px); transition:.2s; pointer-events:none; } #toast.show { opacity:1; transform:translateY(0); } #toast.error { border-color:#8b4353; color:#ffd5da; }
    @media (max-width:900px) { main { grid-template-columns:1fr; } }
    @media (max-width:560px) { .header-inner, main { padding-left:14px; padding-right:14px; } .skill { display:block; } .actions { justify-content:flex-start; margin-top:12px; } }
  </style>
</head>
<body>
  <header><div class="header-inner"><div><h1><span>AL</span>Coding Registry</h1><p class="subtitle">登记并发现团队的 SKILL，正文由作者自行维护</p></div><button id="refresh">刷新能力表</button></div></header>
  <main>
    <section class="panel"><div class="panel-head"><h2>能力表</h2><span class="count" id="count"></span></div><div class="registry" id="registry"><div class="empty">正在读取注册表...</div></div></section>
    <aside class="panel"><div class="panel-head"><h2 id="form-title">上传并注册 SKILL</h2></div>
      <form id="skill-form"><input type="hidden" id="editing-id" />
        <label>SKILL 文件 <span class="hint">上传内容按原字节保存，不会被解析</span><input id="skill-file" type="file" accept=".md,.markdown,.txt,*/*" /></label>
        <label>唯一 ID <span class="hint">小写字母、数字和连字符</span><input id="skill-id" required pattern="[a-z0-9]+(?:-[a-z0-9]+)*" placeholder="my-skill" /></label>
        <label>名称 <input id="skill-name" required placeholder="我的编码 SKILL" /></label>
        <label>类型 <select id="skill-type"><option value="global">global · 全局</option><option value="language">language · 语言</option><option value="architecture">architecture · 架构</option><option value="project">project · 项目约束</option></select></label>
        <label>适用范围 <select id="skill-scope"><option value="global">global · 全局</option><option value="project">project · 项目</option></select></label>
        <label id="project-id-field">项目 ID <span class="hint">项目范围必填，小写字母、数字和连字符</span><input id="skill-project-id" pattern="[a-z0-9]+(?:-[a-z0-9]+)*" placeholder="my-project" /></label>
        <label>描述 <textarea id="skill-description" placeholder="用于帮助 Agent 判断是否相关"></textarea></label>
        <label>标签 <span class="hint">用逗号分隔</span><input id="skill-tags" placeholder="java, backend" /></label>
        <label>元数据 JSON <span class="hint">可选，例如 {&quot;owner&quot;:&quot;team-a&quot;}</span><textarea id="skill-metadata" placeholder='{"owner":"team-a"}'></textarea></label>
        <div class="form-actions"><button type="button" id="cancel-edit" hidden>取消编辑</button><button class="primary" type="submit" id="submit">注册 SKILL</button></div>
      </form>
    </aside>
  </main>
  <div id="toast"></div>
  <script>
    const $ = (id) => document.getElementById(id);
    const labels = {global:'全局', language:'语言', architecture:'架构', project:'项目约束'};
    let index = null;
    function toast(message, error=false) { const t=$('toast'); t.textContent=message; t.className='show'+(error?' error':''); clearTimeout(window.__toast); window.__toast=setTimeout(()=>t.className='',3200); }
    async function api(url, options={}) { const res=await fetch(url, options); const data=await res.json().catch(()=>({})); if(!res.ok) throw new Error(data.error || `请求失败 (${res.status})`); return data; }
    function render(data) { index=data; const root=$('registry'); root.innerHTML=''; let total=0; for(const type of ['global','language','architecture','project']) { const entries=data.categories[type]||[]; total+=entries.length; const box=document.createElement('div'); box.className='category'; box.innerHTML=`<div class="category-title">${labels[type]} <span class="count">${entries.length}</span></div>`; if(!entries.length) { box.innerHTML += '<div class="empty">暂无登记</div>'; } for(const item of entries) { const row=document.createElement('div'); row.className='skill'; const status=item.enabled?(item.available===false?'可用性未知':'已启用'):'已禁用'; const cls=item.enabled?(item.available===false?'':'ok'):'off'; const scope=item.scope==='project'?`项目范围 · ${esc(item.project_id)}`:'全局范围'; row.innerHTML=`<div class="skill-main"><div class="skill-name">${esc(item.name)} <span class="skill-id">${esc(item.id)}</span></div><div class="skill-desc">${esc(item.description||'')} · ${scope}</div><div class="skill-path">${esc(item.path)}</div></div><div><div class="status ${cls}">${status}</div><div class="actions"><button data-action="edit" data-id="${esc(item.id)}">修改</button><button data-action="${item.enabled?'disable':'enable'}" data-id="${esc(item.id)}">${item.enabled?'禁用':'启用'}</button><button class="danger" data-action="remove" data-id="${esc(item.id)}">取消注册</button></div></div>`; box.appendChild(row); } root.appendChild(box); } $('count').textContent=`${total} 个已登记 SKILL`; }
    function esc(value) { return String(value??'').replace(/[&<>"']/g, c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c])); }
    async function refresh() { try { render(await api('/api/registry?include_disabled=1')); } catch(e) { toast(e.message,true); } }
    function updateScopeFields() { const project=$('skill-scope').value==='project'; $('project-id-field').hidden=!project; $('skill-project-id').required=project; if(!project) $('skill-project-id').value=''; }
    function resetForm() { $('skill-form').reset(); $('editing-id').value=''; $('skill-id').disabled=false; $('skill-file').required=true; $('form-title').textContent='上传并注册 SKILL'; $('submit').textContent='注册 SKILL'; $('cancel-edit').hidden=true; updateScopeFields(); }
    function edit(id) { const item=Object.values(index.categories).flat().find(x=>x.id===id); if(!item) return; $('editing-id').value=id; $('skill-id').value=item.id; $('skill-id').disabled=true; $('skill-name').value=item.name; $('skill-type').value=item.type; $('skill-scope').value=item.scope||'global'; $('skill-project-id').value=item.project_id||''; updateScopeFields(); $('skill-description').value=item.description||''; $('skill-tags').value=(item.tags||[]).join(', '); $('skill-metadata').value=item.metadata?JSON.stringify(item.metadata,null,2):''; $('skill-file').required=false; $('form-title').textContent='修改注册'; $('submit').textContent='保存修改'; $('cancel-edit').hidden=false; window.scrollTo({top:0,behavior:'smooth'}); }
    $('refresh').onclick=refresh; $('cancel-edit').onclick=resetForm; $('skill-scope').onchange=updateScopeFields; updateScopeFields();
    $('registry').onclick=async (event)=>{ const btn=event.target.closest('button[data-action]'); if(!btn)return; const id=btn.dataset.id, action=btn.dataset.action; try { if(action==='edit'){edit(id);return;} if(action==='remove'&&!confirm(`取消注册 ${id}？\n只移除登记，不删除 SKILL 文件。`))return; const method=action==='remove'?'DELETE':'POST'; const result=await api(`/api/skills/${encodeURIComponent(id)}/${action==='remove'?'': 'state'}`,{method,headers:{'Content-Type':'application/json'},body:action==='remove'?undefined:JSON.stringify({enabled:action==='enable'})}); toast(result.message||'操作完成'); await refresh(); } catch(e){toast(e.message,true);} };
    $('skill-form').onsubmit=async(event)=>{ event.preventDefault(); const file=$('skill-file').files[0]; const editing=$('editing-id').value; try { let metadata=null; if($('skill-metadata').value.trim()) metadata=JSON.parse($('skill-metadata').value); const tags=$('skill-tags').value.split(',').map(x=>x.trim()).filter(Boolean); const scope=$('skill-scope').value; const projectId=$('skill-project-id').value.trim(); if(editing) { const body={name:$('skill-name').value.trim(),type:$('skill-type').value,scope,project_id:scope==='project'?projectId:undefined,description:$('skill-description').value,tags,metadata}; delete body.path; if(scope!=='project') delete body.project_id; await api(`/api/skills/${encodeURIComponent(editing)}`,{method:'PATCH',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}); if(file) { const form=new FormData(); form.append('skill',file); await api(`/api/skills/${encodeURIComponent(editing)}/content`,{method:'POST',body:form}); } toast('注册信息已更新'); } else { if(!file) throw new Error('请选择要上传的 SKILL 文件'); const form=new FormData(); form.append('skill',file); form.append('id',$('skill-id').value.trim()); form.append('name',$('skill-name').value.trim()); form.append('type',$('skill-type').value); form.append('scope',scope); form.append('project_id',projectId); form.append('description',$('skill-description').value); form.append('tags',JSON.stringify(tags)); form.append('metadata',JSON.stringify(metadata||{})); await api('/api/skills',{method:'POST',body:form}); toast('SKILL 已注册'); } resetForm(); await refresh(); } catch(e){toast(e.message,true);} };
    refresh();
  </script>
</body>
</html>"""


def _json(handler: BaseHTTPRequestHandler, status: int, payload: dict[str, Any]) -> None:
    data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json; charset=utf-8")
    handler.send_header("Content-Length", str(len(data)))
    handler.end_headers()
    handler.wfile.write(data)


class RegistryHandler(BaseHTTPRequestHandler):
    server_version = "ALCodingRegistry/0.1"

    @property
    def root(self) -> Path:
        return self.server.registry_root  # type: ignore[attr-defined]

    def log_message(self, format: str, *args: Any) -> None:
        print(f"[registry-ui] {self.address_string()} - {format % args}")

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        try:
            if parsed.path in {"/", "/index.html"}:
                data = UI_HTML.encode("utf-8")
                self.send_response(HTTPStatus.OK)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
            if parsed.path == "/api/registry":
                query = parse_qs(parsed.query)
                include_disabled = query.get("include_disabled", ["0"])[0] == "1"
                project_id = query.get("project_id", [None])[0]
                _json(self, HTTPStatus.OK, build_index(self.root, include_disabled, project_id))
                return
            if parsed.path.startswith("/api/skills/"):
                skill_id = unquote(parsed.path.rsplit("/", 1)[-1])
                registry = load_registry(self.root)
                entry = _find_entry(registry["skills"], skill_id)
                _json(self, HTTPStatus.OK, {"entry": entry})
                return
            _json(self, HTTPStatus.NOT_FOUND, {"error": "not found"})
        except (RegistryError, OSError, ValueError) as exc:
            _json(self, HTTPStatus.BAD_REQUEST, {"error": str(exc)})

    def do_POST(self) -> None:
        parsed = urlparse(self.path)
        try:
            if parsed.path == "/api/skills":
                fields, files = self._read_multipart()
                content = files.get("skill")
                if content is None:
                    raise RegistryError("skill file is required")
                tags = json.loads(fields.get("tags", "[]"))
                metadata = json.loads(fields.get("metadata", "{}"))
                if not isinstance(tags, list) or not all(isinstance(tag, str) for tag in tags):
                    raise RegistryError("tags must be a JSON string array")
                if not isinstance(metadata, dict):
                    raise RegistryError("metadata must be a JSON object")
                entry = register_uploaded_skill(
                    self.root, skill_id=fields.get("id", ""), name=fields.get("name", ""),
                    skill_type=fields.get("type", ""), content=content,
                    description=fields.get("description") or None, tags=tags, metadata=metadata,
                    scope=fields.get("scope", "global"), project_id=fields.get("project_id") or None,
                )
                _json(self, HTTPStatus.CREATED, {"message": "registered", "entry": entry})
                return
            if parsed.path.startswith("/api/skills/") and parsed.path.endswith("/content"):
                skill_id = unquote(parsed.path.split("/")[3])
                _fields, files = self._read_multipart()
                content = files.get("skill")
                if content is None:
                    raise RegistryError("skill file is required")
                replace_uploaded_skill(self.root, skill_id, content)
                _json(self, HTTPStatus.OK, {"message": "content updated"})
                return
            if parsed.path.startswith("/api/skills/") and parsed.path.endswith("/state"):
                skill_id = unquote(parsed.path.split("/")[3])
                payload = self._read_json()
                enabled = payload.get("enabled")
                if not isinstance(enabled, bool):
                    raise RegistryError("enabled must be boolean")
                entry = update_skill(self.root, skill_id, {"enabled": enabled})
                _json(self, HTTPStatus.OK, {"message": "state updated", "entry": entry})
                return
            _json(self, HTTPStatus.NOT_FOUND, {"error": "not found"})
        except (RegistryError, OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
            _json(self, HTTPStatus.BAD_REQUEST, {"error": str(exc)})

    def do_PATCH(self) -> None:
        parsed = urlparse(self.path)
        try:
            if not parsed.path.startswith("/api/skills/"):
                _json(self, HTTPStatus.NOT_FOUND, {"error": "not found"})
                return
            skill_id = unquote(parsed.path.split("/")[3])
            payload = self._read_json()
            changes = {key: payload[key] for key in ("id", "name", "type", "scope", "project_id", "path", "description", "tags", "metadata") if key in payload}
            entry = update_skill(self.root, skill_id, changes)
            _json(self, HTTPStatus.OK, {"message": "updated", "entry": entry})
        except (RegistryError, OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
            _json(self, HTTPStatus.BAD_REQUEST, {"error": str(exc)})

    def do_DELETE(self) -> None:
        parsed = urlparse(self.path)
        try:
            if not parsed.path.startswith("/api/skills/"):
                _json(self, HTTPStatus.NOT_FOUND, {"error": "not found"})
                return
            skill_id = unquote(parsed.path.split("/")[3])
            entry = remove_skill(self.root, skill_id)
            _json(self, HTTPStatus.OK, {"message": "unregistered", "entry": entry})
        except (RegistryError, OSError, ValueError) as exc:
            _json(self, HTTPStatus.BAD_REQUEST, {"error": str(exc)})

    def _read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        if length > 1024 * 1024:
            raise RegistryError("JSON request is too large")
        payload = json.loads(self.rfile.read(length).decode("utf-8"))
        if not isinstance(payload, dict):
            raise RegistryError("JSON body must be an object")
        return payload

    def _read_multipart(self) -> tuple[dict[str, str], dict[str, bytes]]:
        content_type = self.headers.get("Content-Type", "")
        if not content_type.lower().startswith("multipart/form-data") or "boundary=" not in content_type:
            raise RegistryError("multipart/form-data with a boundary is required")
        length = int(self.headers.get("Content-Length", "0"))
        if length <= 0 or length > 12 * 1024 * 1024:
            raise RegistryError("multipart request is empty or too large")
        raw_body = self.rfile.read(length)
        message = BytesParser(policy=policy.default).parsebytes(
            f"Content-Type: {content_type}\r\nMIME-Version: 1.0\r\n\r\n".encode("utf-8") + raw_body
        )
        if not message.is_multipart():
            raise RegistryError("invalid multipart request")
        fields: dict[str, str] = {}
        files: dict[str, bytes] = {}
        for part in message.iter_parts():
            disposition = part.get("Content-Disposition", "")
            name = part.get_param("name", header="content-disposition")
            if not name:
                continue
            filename = part.get_filename()
            payload = part.get_payload(decode=True) or b""
            if filename is not None:
                files[str(name)] = payload
            else:
                charset = part.get_content_charset() or "utf-8"
                fields[str(name)] = payload.decode(charset, errors="strict")
        return fields, files


class RegistryServer(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, address: tuple[str, int], root: Path):
        super().__init__(address, RegistryHandler)
        self.registry_root = root.resolve()


def serve_server(server: RegistryServer) -> None:
    print(f"ALCoding Registry UI: http://{server.server_address[0]}:{server.server_port}/")
    print(f"Registry root: {server.registry_root}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopping ALCoding Registry UI")
    finally:
        server.server_close()


def serve(root: str | Path, host: str = "127.0.0.1", port: int = 8765) -> None:
    root_path = Path(root).resolve()
    server = RegistryServer((host, port), root_path)
    serve_server(server)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Start the local ALCoding registry UI")
    parser.add_argument("--root", default=".", help="ALCoding root")
    parser.add_argument("--host", default="127.0.0.1", help="bind host")
    parser.add_argument("--port", type=int, default=8765, help="bind port (0 chooses a free port)")
    args = parser.parse_args(argv)
    if not (0 <= args.port <= 65535):
        parser.error("port must be between 0 and 65535")
    serve(args.root, args.host, args.port)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
