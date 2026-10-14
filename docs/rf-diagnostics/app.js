'use strict';
const M=RFDiagnostics,$=id=>document.getElementById(id),ns='http://www.w3.org/2000/svg';
let generation=0;
function render(rows){
 const last=rows.at(-1),kinds=[...new Set(rows.map(r=>M.modes[r.source_kind]))];
 $('mode').textContent=kinds.join(' + ')||'NO DATA';
 $('count').textContent=rows.length.toLocaleString();
 $('rms').textContent=last?.rms_dbfs==null?'—':last.rms_dbfs.toFixed(2);
 $('rails').textContent=last?.rail_fraction==null?'—':(last.rail_fraction*100).toFixed(3)+'%';
 $('gaps').textContent=rows.length?rows.reduce((n,r)=>n+BigInt(r.missing_samples),0n).toLocaleString():'—';
 const reasons=new Map();
 for(const r of rows)for(const reason of new Set(r.invalid_reasons))reasons.set(reason,(reasons.get(reason)||0)+1);
 $('reasons').replaceChildren();
 for(const [reason,count] of reasons){const li=document.createElement('li');li.textContent=reason+' · '+count.toLocaleString();$('reasons').append(li);}
 $('meta').textContent=last?[
 'Source: '+last.source_id,'Origin: '+last.source_kind+' (file/demo replay)',
 'Rate: '+(last.sample_rate_hz||'unknown')+' samples/s',
 'Centre: '+(last.center_frequency_hz??'unknown')+' Hz (unverified)',
 'Run / slot: '+last.run+' / '+last.slot_index,
 'Host monotonic ns: '+last.observed_monotonic_ns,
 'Absolute sample start: '+(last.sample_start??'unknown'),
 'Configuration verified: false','Valid for inference: false',
 'TTL: '+last.ttl_ms+' ms; replay freshness unknown'
 ].join('\n'):'No observations';
 const tail=rows.slice(-500),values=tail.map(r=>r.rms_dbfs).filter(v=>v!==null);
 const low=Math.min(...values),high=Math.max(...values),span=high-low||1;
 $('chart').replaceChildren();
 let path='';
 tail.forEach((r,i)=>{
  if(r.rms_dbfs===null){path+=' ';return;}
  const x=20+i*960/Math.max(1,tail.length-1),y=220-200*(r.rms_dbfs-low)/span;
  path+=(i===0||tail[i-1].rms_dbfs===null?'M':'L')+x+','+y+' ';
  if(r.missing_samples){const dot=document.createElementNS(ns,'circle');dot.setAttribute('cx',x);dot.setAttribute('cy',y);dot.setAttribute('r',4);dot.setAttribute('fill','#ffb66c');$('chart').append(dot);}
 });
 const line=document.createElementNS(ns,'path');line.setAttribute('d',path);line.setAttribute('fill','none');line.setAttribute('stroke','#71e7db');line.setAttribute('stroke-width',2);$('chart').prepend(line);
 $('range').textContent=values.length?'Range '+low.toFixed(2)+' to '+high.toFixed(2)+' dBFS · '+tail.length+' latest records':'No RMS observations';
 $('chart').setAttribute('aria-label',$('range').textContent);
}
$('demo').onclick=()=>{generation++;render(M.demo());$('status').textContent='Synthetic demonstration loaded. No hardware evidence.';};
$('clear').onclick=()=>{generation++;render([]);$('file').value='';$('status').textContent='Cleared. No observations retained.';};
$('file').onchange=async()=>{
 const ticket=++generation,file=$('file').files[0];if(!file)return;
 try{
  if(file.size>M.MAX_BYTES)throw Error('File exceeds 10 MiB');
  const rows=M.parse(await file.text());if(ticket!==generation)return;
  render(rows);$('status').textContent='Imported '+rows.length+' records. Source labels are unverified claims; this is file replay.';
 }catch(e){if(ticket!==generation)return;render([]);$('status').textContent='Import rejected: '+e.message;}
};
