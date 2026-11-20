// SPDX-License-Identifier: MIT
// RuView console widget (ADR-375): one self-contained HTML resource rendered by
// MCP Apps hosts (`_meta.ui.resourceUri`, mimeType text/html;profile=mcp-app)
// and ChatGPT (`openai/outputTemplate`). No external requests, no innerHTML:
// every value from a tool result reaches the DOM through textContent only.

export const CONSOLE_URI = 'ui://ruview/console-v1.html';
export const UI_MIME = 'text/html;profile=mcp-app';

export const CONSOLE_HTML = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>RuView console</title>
<style>
:root{color-scheme:dark;--bg:#090b0d;--panel:#0f1312;--text:#eef0e9;--muted:#9ca49e;--line:#29302d;--acid:#d0ff72;--warn:#ffcf5c;--bad:#ff7a6b;--mono:ui-monospace,Consolas,Menlo,monospace}
:root[data-theme=light]{color-scheme:light;--bg:#f7f8f4;--panel:#fff;--text:#121512;--muted:#5b625d;--line:#dde2da;--acid:#4a7a00;--warn:#8a5d00;--bad:#b42318}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px/1.5 system-ui,Arial,sans-serif}
main{padding:16px;max-width:880px;margin:0 auto}
.top{display:flex;align-items:center;gap:10px;flex-wrap:wrap;margin-bottom:12px}
.eyebrow,.mono,.badge,.chip,th{font-family:var(--mono);font-size:11px;letter-spacing:1.2px;text-transform:uppercase}
.cmd{font-family:var(--mono);font-size:12px;overflow-wrap:anywhere}.id{font-family:var(--mono);font-size:12px;white-space:nowrap}
.eyebrow{color:var(--muted);flex:1;min-width:0}
.badge{border:1px solid var(--line);border-radius:3px;padding:2px 8px;color:var(--muted)}
.badge[data-s=ok]{color:var(--acid);border-color:var(--acid)}.badge[data-s=warn]{color:var(--warn);border-color:var(--warn)}.badge[data-s=bad]{color:var(--bad);border-color:var(--bad)}
button{font:inherit;font-size:12px;background:none;color:var(--text);border:1px solid var(--line);border-radius:3px;padding:5px 11px;cursor:pointer}
button:focus-visible{outline:2px solid var(--acid);outline-offset:2px}button:disabled{opacity:.5;cursor:wait}
.alert{border:1px solid var(--bad);border-left-width:4px;border-radius:4px;padding:10px 12px;margin:0 0 12px}
.alert.warn{border-color:var(--warn)}.alert b{display:block}.alert p{margin:4px 0 0;color:var(--muted)}
.stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:8px;margin-bottom:12px}
.stat,.card{background:var(--panel);border:1px solid var(--line);border-radius:6px;padding:10px 12px}
.stat .v{font-size:20px;font-weight:600}.stat .k,.k{color:var(--muted);font-size:12px}
.cards{display:grid;gap:10px}.card h3{margin:0 0 8px;font-size:15px;display:flex;gap:8px;align-items:center;flex-wrap:wrap}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(110px,1fr));gap:8px}
.meter{height:4px;background:var(--line);border-radius:2px;margin-top:4px;overflow:hidden}.meter i{display:block;height:100%;background:var(--acid)}
.chip{display:inline-block;border:1px solid var(--line);border-radius:3px;padding:1px 6px;margin:2px 4px 0 0;color:var(--muted)}
.note{color:var(--muted);font-size:12px;margin:8px 0 0}
table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:6px 8px;border-bottom:1px solid var(--line);vertical-align:top}th{color:var(--muted);font-weight:400}
pre{background:var(--panel);border:1px solid var(--line);border-radius:6px;padding:10px;overflow:auto;font:12px/1.45 var(--mono);max-height:340px}
.muted{color:var(--muted)}
</style></head>
<body><main id="app" aria-live="polite"><p class="muted">Waiting for a RuView result…</p></main>
<script>
(function(){
'use strict';
var app=document.getElementById('app');
var state={tool:null,args:null,result:null};
var pending={},rpcId=0;

function h(tag,props){
  var el=document.createElement(tag);
  if(props)Object.keys(props).forEach(function(k){
    var v=props[k];if(v==null||v===false)return;
    if(k==='class')el.className=v;else if(k.slice(0,2)==='on')el.addEventListener(k.slice(2),v);else el.setAttribute(k,v===true?'':String(v));
  });
  for(var i=2;i<arguments.length;i++)add(el,arguments[i]);
  return el;
}
function add(el,c){if(c==null||c===false)return;if(Array.isArray(c)){c.forEach(function(x){add(el,x)});return}el.append(c instanceof Node?c:document.createTextNode(String(c)))}
function num(v,d){return typeof v==='number'&&isFinite(v)?v.toFixed(d==null?0:d):'—'}
function pct(v){return typeof v==='number'?(v*100).toFixed(v<0.01&&v>0?2:1)+'%':'—'}
function stat(k,v){return h('div',{class:'stat'},h('div',{class:'v'},v),h('div',{class:'k'},k))}
function meter(frac){var w=Math.max(0,Math.min(1,frac||0));return h('div',{class:'meter'},h('i',{style:'width:'+(w*100).toFixed(1)+'%'}))}
function metric(k,v,frac){return h('div',null,h('div',{class:'k'},k),h('div',null,v),frac==null?null:meter(frac))}
function alertBox(title,body,warn){return h('div',{class:'alert'+(warn?' warn':''),role:'alert'},h('b',null,title),body?h('p',null,body):null)}

function post(msg){if(window.parent&&window.parent!==window)window.parent.postMessage(msg,'*')}
function request(method,params){
  var id=++rpcId;post({jsonrpc:'2.0',id:id,method:method,params:params||{}});
  return new Promise(function(res,rej){pending[id]={res:res,rej:rej};setTimeout(function(){if(pending[id]){delete pending[id];rej(new Error('host did not answer '+method))}},20000)});
}
function applyTheme(t){if(t==='light'||t==='dark')document.documentElement.setAttribute('data-theme',t)}
function toolFrom(meta){return meta&&typeof meta['ruview/tool']==='string'?meta['ruview/tool']:null}
function setResult(r,tool){if(!r||typeof r!=='object')return;state.result=r;if(tool)state.tool=tool;render()}

function canRerun(){return !!(state.tool&&(window.openai&&window.openai.callTool||window.parent!==window))}
function rerun(btn){
  btn.disabled=true;btn.textContent='Running…';
  var call=window.openai&&window.openai.callTool?window.openai.callTool(state.tool,state.args||{}):request('tools/call',{name:state.tool,arguments:state.args||{}});
  Promise.resolve(call).then(function(r){setResult(r&&(r.structuredContent||r),state.tool)}).catch(function(e){
    btn.disabled=false;btn.textContent='Retry';app.prepend(alertBox('Refresh failed',e&&e.message||String(e),true));
  });
}

function header(view,r){
  var s=r.ok===false?'bad':(r.evidence&&/^SYNTHETIC/.test(r.evidence))?'warn':'ok';
  var label=r.ok===false?(r.reason||'failed').replace(/_/g,' '):(r.evidence?r.evidence.split(':')[0]:'OK');
  return h('div',{class:'top'},
    h('div',{class:'eyebrow'},'RUVIEW / '+view),
    h('span',{class:'badge','data-s':s},label),
    canRerun()?h('button',{type:'button',onclick:function(e){rerun(e.currentTarget)}},'Refresh'):null);
}
function failure(r){return r.ok===false?alertBox((r.reason||'failed').replace(/_/g,' '),r.remedy||r.detail||r.hint||null):null}

function nodeCard(n,maxHz){
  var src=n.source==='realtek'?'Realtek RTL8721Dx':'ESP32';
  var csi=n.csi||{};
  var shapes=(n.csiShapes||[]).map(function(s){return h('span',{class:'chip'},s.shape+' · '+s.frames)});
  var seq=[];if(n.seqReordered)seq.push(n.seqReordered+' reordered');if(n.seqStrays)seq.push(n.seqStrays+' stray');if(n.seqResyncs)seq.push(n.seqResyncs+' resync');
  var v=n.lastVitals;
  return h('div',{class:'card'},
    h('h3',null,src+' · node '+n.nodeId,h('span',{class:'chip'},Object.keys(n.packets||{}).map(function(k){return k+' '+n.packets[k]}).join(' · '))),
    h('div',{class:'grid'},
      metric('CSI rate',num(n.csiRateHz,1)+' Hz',maxHz?n.csiRateHz/maxHz:0),
      h('div',null,h('div',{class:'k'},'Loss'),h('div',{style:n.csiLossFraction>0.05?'color:var(--warn)':null},pct(n.csiLossFraction))),
      metric('RSSI',num(n.rssiMean,1)+' dBm',n.rssiMean==null?null:(n.rssiMean+95)/65),
      metric('CSI shape',csi.shape?csi.shape+(csi.channel?' · ch '+csi.channel:csi.freqMhz?' · '+csi.freqMhz+' MHz':''):'—')),
    shapes.length>1?h('div',{class:'note'},'Shapes: ',shapes):null,
    seq.length?h('div',{class:'note'},'Sequence: '+seq.join(' · ')):null,
    csi.synthetic?alertBox('SYNTHETIC frames','This node set the simulator flag; these are not live measurements.',true):null,
    v?h('div',{class:'note'},'Device-reported (not validated): presence '+(v.presence?'yes':'no')+' · breathing '+num(v.breathingBpm,1)+' bpm · heart '+num(v.heartBpm,1)+' bpm'):null);
}
function analysis(a){
  if(!a)return null;
  if(a.ok===false)return alertBox('Kernel analysis: '+String(a.reason||'failed').replace(/_/g,' '),a.detail||a.remedy||null,true);
  var s=a.summary||{},last=s.last||{},hr=last.heart||{},rr=last.respiratory||{},inp=a.input||{};
  return h('div',{class:'card'},
    h('h3',null,'Kernel analysis',h('span',{class:'chip'},(a.backend||'?')+' · '+(a.integrity||'?'))),
    h('div',{class:'grid'},
      metric('Input',(inp.frames||0)+' frames · '+num(inp.sampleRateHz,1)+' Hz'),
      metric('Heart (est.)',num(hr.bpm,1)+' bpm · '+(hr.status||'—')),
      metric('Breathing (est.)',rr.status==='unavailable'?'unavailable':num(rr.bpm,1)+' bpm · '+(rr.status||'—')),
      metric('Signal quality',num(last.signal_quality,2))),
    h('p',{class:'note'},a.note||'Estimates without a reference measurement.'));
}
function viewCapture(r){
  var nodes=r.nodes||[],maxHz=nodes.reduce(function(m,n){return Math.max(m,n.csiRateHz||0)},0);
  return [
    failure(r),
    (r.heartbeatOnlySenders||[]).length?alertBox('Heartbeat-only senders','Alive but no CSI: '+r.heartbeatOnlySenders.map(function(s){return s.address+' ('+s.heartbeats+')'}).join(', '),true):null,
    h('div',{class:'stats'},stat('Packets',r.packets!=null?r.packets:'—'),stat('Decoded',r.decodedPackets!=null?r.decodedPackets:'—'),stat('CSI nodes',r.csiNodes!=null?r.csiNodes:'—'),stat('Window',(r.seconds||'—')+' s')),
    h('div',{class:'cards'},nodes.map(function(n){return nodeCard(n,maxHz)}),analysis(r.analysis)),
    r.unknownPackets?h('p',{class:'note'},'Unknown packets: '+r.unknownPackets+(r.unknownMagics?' ('+Object.keys(r.unknownMagics).join(', ')+')':'')):null,
    r.listen?h('p',{class:'note cmd'},'listen '+r.listen):null];
}
function viewDevices(r){
  return [failure(r),h('table',null,h('thead',null,h('tr',null,h('th',null,'Port'),h('th',null,'Bridge'),h('th',null,'Likely'),h('th',null,'Confirm with'))),
    h('tbody',null,(r.devices||[]).filter(function(d){return !d.builtin}).map(function(d){
      return h('tr',null,h('td',{class:'id'},d.port),h('td',null,d.bridge||d.usb||'unknown'),h('td',null,(d.likelyRoles||[]).map(function(x){return h('span',{class:'chip'},x)})),h('td',{class:'cmd'},(d.confirmWith||[])[0]||'—'))}))),
    h('p',{class:'note'},r.note||'')];
}
function viewDoctor(r){
  var s=r.summary||{};var rows=(r.checks||[]).map(function(c){
    var st=c.status==='pass'?'ok':c.status==='fail'?'bad':c.status==='warn'?'warn':'';
    return h('tr',null,h('td',null,h('span',{class:'badge','data-s':st},c.status)),h('td',{class:'id'},c.group+'/'+c.id),h('td',null,c.detail,c.remedy&&c.status!=='pass'?h('div',{class:'note'},'fix: '+c.remedy):null))});
  return [h('div',{class:'stats'},stat('Pass',s.pass||0),stat('Warn',s.warn||0),stat('Fail',s.fail||0),stat('Skip',s.skip||0)),h('table',null,h('tbody',null,rows))];
}
function viewRadar(r){
  if(r.ok===false)return [failure(r)];
  var d=r.device||{},present=r.presentNow!=null?r.presentNow:(r.presentFraction==null?null:r.presentFraction>=0.5);
  var src=r.source==='esphome'?'ESPHome '+(d.esphomeVersion||'')+' · '+(d.name||r.host||'')+(d.projectVersion?' · '+d.projectVersion:''):(r.model||'radar')+' · '+(r.frames||0)+' frames · '+(r.checksumErrors||0)+' checksum errors';
  return [
    h('div',{class:'stats'},
      stat('Presence',present==null?'—':present?'Detected':'None'),
      stat('Distance',r.distanceCmMean==null?'—':num(r.distanceCmMean,1)+' cm'),
      stat('Heart (device)',r.heartBpmMean==null?'—':num(r.heartBpmMean,1)+' bpm'),
      stat('Breathing (device)',r.breathingBpmMean==null?'—':num(r.breathingBpmMean,1)+' bpm')),
    r.entities?h('table',null,h('thead',null,h('tr',null,h('th',null,'Entity'),h('th',null,'Last'),h('th',null,'Updates'))),
      h('tbody',null,r.entities.map(function(e){return h('tr',null,h('td',null,e.name),h('td',{class:'id'},e.last==null?'—':(typeof e.last==='number'?num(e.last,2):String(e.last))+(e.unit?' '+e.unit:'')),h('td',null,e.updates))}))):null,
    h('p',{class:'note'},src),
    h('p',{class:'note'},'Device-reported values computed by the radar firmware; not validated against a reference.')];
}
function viewGeneric(r){return [failure(r),h('pre',null,JSON.stringify(r,null,2))]}

function render(){
  var r=state.result;if(!r)return;
  var t=state.tool||'';
  var view=r.nodes||t==='ruview_esp32_capture'?['NODE STREAM',viewCapture]:t==='ruview_mmwave_read'||r.heartBpmMean!==undefined?['60 GHZ RADAR',viewRadar]:r.devices?['DEVICES',viewDevices]:r.checks&&r.summary?['DOCTOR',viewDoctor]:[(t.replace(/^ruview_/,'').replace(/_/g,' ')||'RESULT').toUpperCase(),viewGeneric];
  app.replaceChildren(header(view[0],r));add(app,view[1](r));
  notifySize();
}
function notifySize(){var hgt=Math.ceil(document.documentElement.scrollHeight);if(window.openai&&typeof window.openai.notifyIntrinsicHeight==='function')window.openai.notifyIntrinsicHeight(hgt);else post({jsonrpc:'2.0',method:'ui/notifications/size-changed',params:{height:hgt}})}

window.addEventListener('message',function(e){
  if(e.source!==window.parent)return;var m=e.data;if(!m||m.jsonrpc!=='2.0')return;
  if(m.id!=null&&!m.method&&pending[m.id]){var p=pending[m.id];delete pending[m.id];if(m.error)p.rej(new Error(m.error.message||'host error'));else p.res(m.result);return}
  var params=m.params||{};
  if(m.method==='ui/notifications/tool-input')state.args=params.arguments||null;
  else if(m.method==='ui/notifications/tool-result')setResult(params.structuredContent,toolFrom(params._meta));
  else if(m.method==='ui/notifications/host-context-changed')applyTheme(params.theme);
});
function fromOpenAI(){var o=window.openai;if(!o)return;applyTheme(o.theme);if(o.toolInput)state.args=o.toolInput;if(o.toolOutput)setResult(o.toolOutput,toolFrom(o.toolResponseMetadata))}
window.addEventListener('openai:set_globals',fromOpenAI);
if(window.openai)fromOpenAI();
else request('ui/initialize',{protocolVersion:'2025-06-18',appInfo:{name:'ruview-console',version:'1'},appCapabilities:{}}).then(function(res){
  applyTheme(res&&res.hostContext&&res.hostContext.theme);post({jsonrpc:'2.0',method:'ui/notifications/initialized',params:{}});
}).catch(function(){});
})();
</script></body></html>`;

/** The resource descriptor advertised by resources/list. */
export const CONSOLE_RESOURCE = Object.freeze({
  uri: CONSOLE_URI,
  name: 'ruview-console',
  title: 'RuView console',
  description: 'Interactive view of RuView node captures, device scans and diagnostics. Self-contained; makes no network requests.',
  mimeType: UI_MIME,
});

/** Resource-level metadata: an empty CSP allowlist (no external origins) for both host families. */
export const CONSOLE_RESOURCE_META = Object.freeze({
  ui: { csp: { connectDomains: [], resourceDomains: [] }, prefersBorder: true },
  'openai/widgetDescription': 'Shows a RuView tool result: per-node CSI rate, loss and RSSI, radar presence and vitals, device scans, or doctor checks, with a refresh button.',
  'openai/widgetPrefersBorder': true,
  'openai/widgetCSP': { connect_domains: [], resource_domains: [] },
});

/** Tools whose results render in the console widget. */
export const UI_TOOLS = Object.freeze(['ruview_esp32_capture', 'ruview_devices_scan', 'ruview_doctor', 'ruview_mmwave_read']);

/** Tool-level metadata pointing a UI tool at the widget (MCP Apps + ChatGPT keys). */
export function uiToolMeta() {
  return {
    ui: { resourceUri: CONSOLE_URI },
    'ui/resourceUri': CONSOLE_URI,
    'openai/outputTemplate': CONSOLE_URI,
    'openai/widgetAccessible': true,
    'openai/toolInvocation/invoking': 'Reading RuView…',
    'openai/toolInvocation/invoked': 'RuView result ready',
  };
}
