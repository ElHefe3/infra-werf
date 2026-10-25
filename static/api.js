export const $=s=>document.querySelector(s), $$=s=>[...document.querySelectorAll(s)];
export const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
export async function api(path,method='GET',data){
  const response=await fetch('/api/'+path,{method,headers:{'Content-Type':'application/json','X-Atlas-Client':'browser'},...(data===undefined?{}:{body:JSON.stringify(data)})});
  const result=await response.json();if(!response.ok)throw new Error(result.error||'Request failed');return result;
}
export const date=s=>s?new Date(s).toLocaleString():'Not collected';
export function observation(s){
  if(!s.inventory)return {label:'Not observed',stale:true};
  const stale=Date.now()-Date.parse(s.inventory.collected_at)>15*60*1000;
  return {label:stale?'Saved · stale':'Saved observation',stale};
}
export function workloads(s){const i=s.inventory;return i?[...i.containers.map(c=>({...c,kind:c.engine})),...i.services.map(c=>({...c,kind:'systemd'}))]:[];}
export const active=s=>workloads(s).filter(w=>['active','running'].includes(w.state));
export const matches=(s,q)=>[s.name,s.host,...workloads(s).map(v=>v.name)].join(' ').toLowerCase().includes(q);
export const giB=s=>s.inventory?(s.inventory.memory_total_kib/1024/1024).toFixed(1)+' GiB':'—';
export function wireForm(formId,submit){
  $(formId).onsubmit=async e=>{e.preventDefault();const b=e.target.querySelector('button[type="submit"]');b.disabled=true;
    try{await submit(Object.fromEntries(new FormData(e.target)));}
    catch(error){$('#formError').hidden=false;$('#formError').textContent=error.message;}
    finally{b.disabled=false;}
  };
}
export function addServer(open,refresh,notify){
  open(`<div class="eyebrow">GROW YOUR MAP</div><h2 id="sheetTitle">Add a server</h2><p class="detail-note">Register a host in Atlas. Adding it does not connect or install software.</p><form id="addForm"><label class="field">Display name<input name="name" required maxlength="80" placeholder="Media server"></label><label class="field">Hostname or IP<input name="host" required maxlength="253" placeholder="media.home" autocapitalize="none" spellcheck="false"></label><button class="primary" type="submit">Add server →</button><p id="formError" role="alert" class="notice" hidden></p></form>`);
  wireForm('#addForm',async data=>{await api('servers','POST',data);await refresh();document.querySelector('dialog[open]').close();notify('Server saved to Atlas');});
}
export function connection(s,servers,edges,open,refresh,notify){
  const targets=servers.filter(v=>v.id!==s.id);if(!targets.length)return notify('Add another server first');
  open(`<div class="eyebrow">DOCUMENT A RELATIONSHIP</div><h2 id="sheetTitle">Connect ${esc(s.name)}</h2><p class="detail-note">Records a dependency; network configuration stays unchanged.</p><form id="edgeForm"><label class="field">Target<select name="target">${targets.map(v=>`<option value="${v.id}">${esc(v.name)}</option>`).join('')}</select></label><label class="field">Relationship<input name="label" placeholder="Backs up to" required maxlength="80"></label><button class="primary" type="submit">Save connection</button><p id="formError" role="alert" class="notice" hidden></p></form><div class="detail-label">EXISTING RELATIONSHIPS</div>${edges.filter(e=>e.source===s.id||e.target===s.id).map(e=>`<div class="file"><span>${esc(e.label)}</span><small>${esc(servers.find(v=>v.id===(e.source===s.id?e.target:e.source))?.name)}</small><button class="secondary small-button" data-remove-edge="${e.id}">Remove</button></div>`).join('')||'<p class="detail-note">No documented connections.</p>'}`);
  wireForm('#edgeForm',async data=>{await api('edges','POST',{source:s.id,...data});await refresh();document.querySelector('dialog[open]').close();notify('Relationship saved');});
  $$('[data-remove-edge]').forEach(b=>b.onclick=async()=>{try{await api('edges/'+b.dataset.removeEdge,'DELETE',{});await refresh();document.querySelector('dialog[open]').close();notify('Relationship removed');}catch(e){notify(e.message);}});
}
export function editServer(s,open,refresh,notify){
  open(`<div class="eyebrow">SERVER SETTINGS</div><h2 id="sheetTitle">Edit server</h2><form id="editForm"><label class="field">Display name<input name="name" value="${esc(s.name)}" required maxlength="80"></label><label class="field">Connection hostname<input name="host" value="${esc(s.host)}" required maxlength="253"></label><p class="detail-note">Saved inventory remains associated with this server and retains its observed hostname and time.</p><button class="primary" type="submit">Save details</button><p id="formError" class="notice" role="alert" hidden></p></form>`);
  wireForm('#editForm',async data=>{await api('servers/'+s.id,'PATCH',data);await refresh();document.querySelector('dialog[open]').close();notify('Server details saved');});
}
export function workspace(s,open,refresh,notify,onBack){
  function show(app){
    open(`<button id="backDetails" class="back secondary">‹ Server overview</button><h2 id="sheetTitle">${esc(s.name)}</h2><p class="detail-note">${esc(s.host)}</p><nav class="apps" aria-label="Server apps">${['services','system','files','terminal','settings','transfers'].map(a=>`<button data-app="${a}" class="secondary ${a===app?'active':''}" ${a===app?'aria-current="page"':''}>${a[0].toUpperCase()+a.slice(1)}</button>`).join('')}</nav><div id="appBody"></div>`);
    $('#backDetails').onclick=onBack;$$('[data-app]').forEach(b=>b.onclick=()=>show(b.dataset.app));
    if(app==='services'){
      $('#appBody').innerHTML=`<p class="detail-note">Observed ${date(s.inventory?.collected_at)}. State below belongs to this saved observation.</p><input id="workloadSearch" class="workload-search" placeholder="Filter services and containers" aria-label="Filter services and containers"><div id="workloads"></div>`;
      const render=()=>{const q=$('#workloadSearch').value.toLowerCase(),rows=workloads(s).filter(v=>[v.name,v.scope,v.kind,v.state].join(' ').toLowerCase().includes(q));$('#workloads').innerHTML=rows.map(w=>`<div class="file"><div><strong>${esc(w.name)}</strong><small>${esc(w.scope)} · ${esc(w.kind)}${w.boot?' · '+esc(w.boot):''}</small></div><span class="state-tag">${esc(w.state||'unknown')}</span></div>`).join('')||'<p class="empty">No matching inventory. Import a saved report to see workloads.</p>';};$('#workloadSearch').oninput=render;render();
    }else if(app==='system'){
      const i=s.inventory;$('#appBody').innerHTML=`<div class="file">Observed hostname<small>${esc(i?.hostname||'—')}</small></div><div class="file">CPU threads<small>${i?.cpus??'—'}</small></div><div class="file">Total memory<small>${giB(s)}</small></div><div class="file">Observed at<small>${date(i?.collected_at)}</small></div><div class="lock"><strong>Live utilisation is not collected yet</strong>CPU, RAM and disk usage will appear when a live inventory source supplies them.</div>${(i?.warnings||[]).map(w=>`<p class="notice">${esc(w)}</p>`).join('')}`;
    }else if(app==='settings'){
      $('#appBody').innerHTML='<p class="detail-note">Connection details are stored in the coordinator database. Passwords and private keys are not stored by this increment.</p><button id="editServer" class="secondary">Edit server details</button>';$('#editServer').onclick=()=>editServer(s,open,refresh,notify);
    }else{
      const messages={files:['Files need an authenticated SSH session','Remote file browsing is the next implementation increment. No sample files or simulated authentication are shown here.'],terminal:['Terminal needs an authenticated SSH session','Interactive SSH is not connected in this increment.'],transfers:['No transfers are running','Uploads and downloads will be enabled alongside authenticated file access.']};
      $('#appBody').innerHTML=`<div class="lock"><strong>${messages[app][0]}</strong>${messages[app][1]}</div>`;
    }
  }show('services');
}
export function gestureMap(canvas,world,zoomLabel,positions,drawEdges,onSelect,onMove){
  let scale=1,tx=0,ty=0,gesture=null,drag=null,moved=false,suppress=false;const pointers=new Map();
  const transform=()=>{world.style.transform=`translate(${tx}px,${ty}px) scale(${scale})`;zoomLabel.textContent=Math.round(scale*100)+'%';};
  const fit=()=>{const ps=positions();if(!ps.length)return;const minx=Math.min(...ps.map(p=>p.x))-25,miny=Math.min(...ps.map(p=>p.y))-25,maxx=Math.max(...ps.map(p=>p.x+p.w))+25,maxy=Math.max(...ps.map(p=>p.y+p.h))+50;scale=Math.min(1.15,(canvas.clientWidth-12)/(maxx-minx),(canvas.clientHeight-35)/(maxy-miny));tx=(canvas.clientWidth-(maxx-minx)*scale)/2-minx*scale;ty=(canvas.clientHeight-35-(maxy-miny)*scale)/2-miny*scale;transform();};
  const zoom=(f,x=canvas.clientWidth/2,y=canvas.clientHeight/2)=>{const next=Math.max(.2,Math.min(2,scale*f));tx=x-(x-tx)*next/scale;ty=y-(y-ty)*next/scale;scale=next;transform();};
  const base=()=>{const p=[...pointers.values()];gesture=p.length?{x:p[0].x,y:p[0].y,tx,ty,scale}:null;if(p.length>=2){gesture.cx=(p[0].x+p[1].x)/2;gesture.cy=(p[0].y+p[1].y)/2;gesture.distance=Math.hypot(p[0].x-p[1].x,p[0].y-p[1].y)||1;}};
  canvas.onpointerdown=e=>{if(e.button!==0||e.target.closest('.controls,.zoom-controls'))return;const n=e.target.closest('[data-node]');if(n&&e.pointerType==='mouse'&&onMove){const p=positions().find(v=>v.id===n.dataset.node);drag={id:p.id,x:p.x,y:p.y,cx:e.clientX,cy:e.clientY,node:n};}pointers.set(e.pointerId,{x:e.clientX,y:e.clientY});moved=false;suppress=false;base();canvas.setPointerCapture(e.pointerId);};
  canvas.onpointermove=e=>{if(!pointers.has(e.pointerId)||!gesture)return;pointers.set(e.pointerId,{x:e.clientX,y:e.clientY});const ps=[...pointers.values()];if(ps.length>=2){drag=null;if(!gesture.distance)base();const cx=(ps[0].x+ps[1].x)/2,cy=(ps[0].y+ps[1].y)/2,b=canvas.getBoundingClientRect();scale=Math.max(.2,Math.min(2,gesture.scale*Math.hypot(ps[0].x-ps[1].x,ps[0].y-ps[1].y)/gesture.distance));tx=cx-b.left-(gesture.cx-b.left-gesture.tx)*scale/gesture.scale;ty=cy-b.top-(gesture.cy-b.top-gesture.ty)*scale/gesture.scale;moved=true;transform();}else{const dx=ps[0].x-gesture.x,dy=ps[0].y-gesture.y;if(Math.abs(dx)+Math.abs(dy)>6)moved=true;if(moved&&drag){const p=positions().find(v=>v.id===drag.id);p.x=Math.max(-10000,Math.min(10000,drag.x+dx/scale));p.y=Math.max(-10000,Math.min(10000,drag.y+dy/scale));drag.node.style.left=p.x+'px';drag.node.style.top=p.y+'px';drawEdges();}else if(moved){tx=gesture.tx+dx;ty=gesture.ty+dy;transform();}}if(moved)suppress=true;};
  canvas.onpointerup=e=>{if(!pointers.has(e.pointerId))return;const n=drag?.id;if(moved&&drag)onMove(n);if(!moved){const hit=document.elementFromPoint(e.clientX,e.clientY)?.closest('[data-node]');if(hit)onSelect(hit.dataset.node);}drag=null;pointers.delete(e.pointerId);if(pointers.size)base();else gesture=null;setTimeout(()=>suppress=false,150);};
  canvas.onpointercancel=e=>{pointers.delete(e.pointerId);drag=null;base();};
  canvas.onclick=e=>{const n=e.target.closest('[data-node]');if(n&&e.detail===0&&!suppress)onSelect(n.dataset.node);};
  canvas.addEventListener('wheel',e=>{e.preventDefault();const b=canvas.getBoundingClientRect();zoom(e.deltaY<0?1.08:1/1.08,e.clientX-b.left,e.clientY-b.top);},{passive:false});
  return {fit,zoom,busy:()=>pointers.size>0};
}
