import{f as Ge,u as Ye,i as m,a as b,b as c,w as U}from"./lit-BS7WqYd5.js";import{y as o,g as Xe,j as x}from"./signals-SG45zFCj.js";(function(){const t=document.createElement("link").relList;if(t&&t.supports&&t.supports("modulepreload"))return;for(const a of document.querySelectorAll('link[rel="modulepreload"]'))r(a);new MutationObserver(a=>{for(const i of a)if(i.type==="childList")for(const n of i.addedNodes)n.tagName==="LINK"&&n.rel==="modulepreload"&&r(n)}).observe(document,{childList:!0,subtree:!0});function s(a){const i={};return a.integrity&&(i.integrity=a.integrity),a.referrerPolicy&&(i.referrerPolicy=a.referrerPolicy),a.crossOrigin==="use-credentials"?i.credentials="include":a.crossOrigin==="anonymous"?i.credentials="omit":i.credentials="same-origin",i}function r(a){if(a.ep)return;a.ep=!0;const i=s(a);fetch(a.href,i)}})();/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const h=e=>(t,s)=>{s!==void 0?s.addInitializer(()=>{customElements.define(e,t)}):customElements.define(e,t)};/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const Je={attribute:!0,type:String,converter:Ye,reflect:!1,hasChanged:Ge},Qe=(e=Je,t,s)=>{const{kind:r,metadata:a}=s;let i=globalThis.litPropertyMetadata.get(a);if(i===void 0&&globalThis.litPropertyMetadata.set(a,i=new Map),r==="setter"&&((e=Object.create(e)).wrapped=!0),i.set(s.name,e),r==="accessor"){const{name:n}=s;return{set(d){const p=t.get.call(this);t.set.call(this,d),this.requestUpdate(n,p,e,!0,d)},init(d){return d!==void 0&&this.C(n,void 0,e,d),d}}}if(r==="setter"){const{name:n}=s;return function(d){const p=this[n];t.call(this,d),this.requestUpdate(n,p,e,!0,d)}}throw Error("Unsupported decorator location: "+r)};function De(e){return(t,s)=>typeof s=="object"?Qe(e,t,s):((r,a,i)=>{const n=a.hasOwnProperty(i);return a.constructor.createProperty(i,r),n?Object.getOwnPropertyDescriptor(a,i):void 0})(e,t,s)}/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */function u(e){return De({...e,state:!0,attribute:!1})}/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const Ze=(e,t,s)=>(s.configurable=!0,s.enumerable=!0,Reflect.decorate&&typeof t!="object"&&Object.defineProperty(e,t,s),s);/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */function Ie(e,t){return(s,r,a)=>{const i=n=>n.renderRoot?.querySelector(e)??null;return Ze(s,r,{get(){return i(this)}})}}var et=Object.defineProperty,tt=Object.getOwnPropertyDescriptor,Le=(e,t,s,r)=>{for(var a=r>1?void 0:r?tt(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&et(t,s,a),a};let ie=class extends b{constructor(){super(...arguments),this.view="scene"}navigate(e){this.dispatchEvent(new CustomEvent("navigate",{detail:e}))}render(){return c`
      <div class="logo">NV</div>
      <button class="btn ${this.view==="scene"?"active":""}" data-id="scene-btn" title="Scene"
        @click=${()=>this.navigate("scene")}>
        <svg viewBox="0 0 24 24"><path d="M12 2L3 7l9 5 9-5-9-5zm0 13l-9-5v6l9 5 9-5v-6l-9 5z"/></svg>
      </button>
      <button class="btn ${this.view==="apps"?"active":""}" data-id="apps-btn" title="App Store"
        @click=${()=>this.navigate("apps")}>
        <svg viewBox="0 0 24 24"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
      </button>
      <button class="btn" title="Inspector" @click=${()=>this.navigate("scene")}>
        <svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="7"/><line x1="21" y1="21" x2="16.6" y2="16.6"/></svg>
      </button>
      <button class="btn" title="Witness">
        <svg viewBox="0 0 24 24"><path d="M9 12l2 2 4-4M21 12c0 4.97-4.03 9-9 9s-9-4.03-9-9 4.03-9 9-9 9 4.03 9 9z"/></svg>
      </button>
      <div class="spacer"></div>
      <button class="btn" data-id="settings-btn" title="Settings"
        @click=${()=>this.dispatchEvent(new CustomEvent("open-settings",{bubbles:!0,composed:!0}))}>
        <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06A1.65 1.65 0 0015 19.4a1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.6 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.6a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09A1.65 1.65 0 0015 4.6a1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
      </button>
    `}};ie.styles=m`
    :host {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 10px 0;
      gap: 4px;
      background: var(--bg-1);
      border-right: 1px solid var(--line);
    }
    .logo {
      width: 36px; height: 36px;
      border-radius: 10px;
      background: linear-gradient(135deg, oklch(0.78 0.14 70) 0%, oklch(0.55 0.16 30) 100%);
      display: grid; place-items: center;
      color: #1a0f00;
      font-weight: 700;
      font-family: var(--mono);
      font-size: 11px;
      margin-bottom: 14px;
      box-shadow: 0 4px 12px -2px oklch(0.55 0.16 30 / 0.35);
    }
    .btn {
      width: 36px; height: 36px;
      border-radius: 8px;
      background: transparent;
      border: 1px solid transparent;
      color: var(--ink-3);
      display: grid; place-items: center;
      transition: all 0.15s;
      position: relative;
      cursor: pointer;
    }
    .btn:hover { color: var(--ink); background: var(--bg-2); }
    .btn.active {
      color: var(--ink);
      background: var(--bg-3);
      border-color: var(--line-2);
    }
    .btn.active::before {
      content: ''; position: absolute; left: -10px; top: 8px; bottom: 8px;
      width: 2px; background: var(--accent); border-radius: 2px;
    }
    .spacer { flex: 1; }
    svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 1.8; }
  `;Le([De()],ie.prototype,"view",2);ie=Le([h("nv-rail")],ie);const _=o("wasm"),ye=o("");o(!1);o(null);const v=o(!1);o(!0);o(1);const at=o(0),$e=o(0n),q=o(0xCAFEBABEn),Q=o(1e4),Z=o(1e3),ee=o(1),te=o(!0),g=o("dark"),w=o("default"),$=o(!1),ae=o(!0),ne=o([0,0,0]),D=o(0),T=o(0),k=o(0),C=o(""),f=o("idle"),I=o(""),_e=o(null),oe=o([]),le=o([]),ce=o([]),de=o([]),ze=o("rebar-walkby-01"),st=o(""),N=o(!1),we=o("all"),Ae=Xe(()=>_.value==="wasm"?"wasm":"ws");let Fe=null;function rt(e){Fe=e}function P(){return Fe}const S=o([]),it=200;function l(e,t){if(N.value)return;const s=S.value.slice();for(s.push({ts:Date.now(),level:e,msg:t});s.length>it;)s.shift();S.value=s}function nt(e){const s=oe.value.slice();s.push(e[0]),s.length>200&&s.shift();const r=le.value.slice();r.push(e[1]),r.length>200&&r.shift();const a=ce.value.slice();a.push(e[2]),a.length>200&&a.shift(),oe.value=s,le.value=r,ce.value=a}function ot(e){const s=de.value.slice();for(s.push(Math.max(0,Math.min(1,e)));s.length>48;)s.shift();de.value=s}var lt=Object.getOwnPropertyDescriptor,ct=(e,t,s,r)=>{for(var a=r>1?void 0:r?lt(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=n(a)||a);return a};let Se=class extends b{connectedCallback(){super.connectedCallback(),x(()=>{k.value,Ae.value,q.value,g.value,ze.value,v.value,this.requestUpdate()})}async toggleRun(){const e=P();e&&(v.value?(await e.pause(),v.value=!1):(await e.run(),v.value=!0))}async reset(){const e=P();e&&await e.reset()}toggleTheme(){g.value=g.value==="dark"?"light":"dark"}render(){const e=q.value.toString(16).toUpperCase().padStart(8,"0");return c`
      <div class="crumbs">
        <span class="home">RuView</span><span class="sep">/</span>
        <span>nvsim</span><span class="sep">/</span>
        <span class="cur" id="scene-name">${ze.value}</span>
      </div>
      <div class="spacer"></div>
      <span class="pill" id="fps-pill">
        <span class="dot"></span>
        <span id="fps-val">${k.value>0?(k.value/1e3).toFixed(2)+" kHz":"idle"}</span>
      </span>
      <span class="pill wasm" id="transport-pill"><span class="dot"></span>${Ae.value}</span>
      <span class="pill seed" id="seed-pill">seed: <b>0x${e}</b></span>
      <button class="ghost" id="theme-btn" title="Toggle theme" @click=${this.toggleTheme}>
        ${g.value==="dark"?"☼":"☾"}
      </button>
      <button id="reset-btn" @click=${this.reset}>↺ Reset</button>
      <button class="primary" id="run-btn" @click=${this.toggleRun}>
        ${v.value?"❚❚ Pause":"▶ Run"}
      </button>
    `}};Se.styles=m`
    :host {
      display: flex; align-items: center;
      padding: 0 16px; gap: 12px;
      background: var(--bg-1);
      border-bottom: 1px solid var(--line);
      z-index: 10;
    }
    .crumbs { display: flex; align-items: center; gap: 8px; font-size: 12.5px; color: var(--ink-3); }
    .crumbs .sep { color: var(--ink-4); }
    .crumbs .cur { color: var(--ink); font-weight: 500; }
    .spacer { flex: 1; }
    .pill {
      display: inline-flex; align-items: center; gap: 6px;
      padding: 5px 10px;
      background: var(--bg-2); border: 1px solid var(--line);
      border-radius: 999px;
      font-size: 12px; color: var(--ink-2);
      font-family: var(--mono); font-weight: 500;
    }
    .pill .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--ok); box-shadow: 0 0 6px var(--ok); animation: pulse 2s infinite; }
    .pill.wasm .dot { background: var(--accent-2); box-shadow: 0 0 6px var(--accent-2); }
    .pill.seed { color: var(--ink-3); }
    .pill.seed b { color: var(--accent); font-weight: 600; }
    button {
      display: inline-flex; align-items: center; gap: 6px;
      padding: 6px 12px;
      background: var(--bg-2); border: 1px solid var(--line);
      border-radius: 8px;
      font-size: 12.5px; font-weight: 500; color: var(--ink);
      cursor: pointer;
      transition: all 0.15s;
    }
    button:hover { border-color: var(--line-2); background: var(--bg-3); }
    button.primary { background: var(--accent); border-color: var(--accent); color: #1a0f00; }
    button.primary:hover { filter: brightness(1.08); }
    button.ghost { background: transparent; }
  `;Se=ct([h("nv-topbar")],Se);var dt=Object.getOwnPropertyDescriptor,pt=(e,t,s,r)=>{for(var a=r>1?void 0:r?dt(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=n(a)||a);return a};let Ce=class extends b{connectedCallback(){super.connectedCallback(),x(()=>{Q.value,Z.value,ee.value,te.value,v.value,this.requestUpdate()})}render(){return c`
      <div class="panel">
        <div class="panel-h">Scene <span class="count">4 sources</span></div>
        <div class="scene-item">
          <span class="swatch" style="background:oklch(0.72 0.18 330)"></span>
          <span class="name">rebar.steel.coil</span>
          <span class="meta">χ=5000</span>
        </div>
        <div class="scene-item">
          <span class="swatch" style="background:oklch(0.78 0.14 195)"></span>
          <span class="name">heart_proxy</span>
          <span class="meta">1e-6 A·m²</span>
        </div>
        <div class="scene-item">
          <span class="swatch" style="background:oklch(0.72 0.18 330)"></span>
          <span class="name">mains_60Hz</span>
          <span class="meta">2 A · 60 Hz</span>
        </div>
        <div class="scene-item">
          <span class="swatch" style="background:oklch(0.78 0.14 145)"></span>
          <span class="name">door.steel</span>
          <span class="meta">eddy</span>
        </div>
      </div>

      <div class="panel">
        <div class="panel-h">NV sensor <span class="count">COTS</span></div>
        <div class="field-row"><span class="lbl">V</span><span class="val">1 mm³</span></div>
        <div class="field-row"><span class="lbl">N</span><span class="val">1e12 NV</span></div>
        <div class="field-row"><span class="lbl">C</span><span class="val">0.030</span></div>
        <div class="field-row"><span class="lbl">T₂*</span><span class="val">200 ns</span></div>
        <div class="field-row"><span class="lbl">δB</span><span class="val">1.18 pT/√Hz</span></div>
      </div>

      <div class="panel">
        <div class="panel-h">Tunables</div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Sample rate</span><span class="val">${(Q.value/1e3).toFixed(1)} kHz</span></div>
          <input type="range" min="1000" max="100000" .value=${String(Q.value)}
            @input=${e=>Q.value=+e.target.value} />
        </div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Lockin f_mod</span><span class="val">${(Z.value/1e3).toFixed(3)} kHz</span></div>
          <input type="range" min="100" max="5000" .value=${String(Z.value)}
            @input=${e=>Z.value=+e.target.value} />
        </div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Integration t</span><span class="val">${ee.value.toFixed(1)} ms</span></div>
          <input type="range" min="0.1" max="10" step="0.1" .value=${String(ee.value)}
            @input=${e=>ee.value=+e.target.value} />
        </div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Shot noise</span><span class="val">${te.value?"ON":"OFF"}</span></div>
          <input type="range" min="0" max="1" .value=${te.value?"1":"0"}
            @input=${e=>te.value=e.target.value==="1"} />
        </div>
      </div>

      <div class="panel">
        <div class="panel-h">Pipeline</div>
        <div class="pipeline">
          <span class="stage ${v.value?"live":""}">scene</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${v.value?"live":""}">B-S</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${v.value?"live":""}">prop</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${v.value?"live":""}">NV</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${v.value?"live":""}">ADC</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${v.value?"live":""}">frame</span>
        </div>
      </div>
    `}};Ce.styles=m`
    :host {
      display: flex; flex-direction: column; gap: 14px;
      padding: 14px; overflow-y: auto;
      background: var(--bg-1); border-right: 1px solid var(--line);
    }
    .panel {
      background: var(--bg-2); border: 1px solid var(--line);
      border-radius: var(--radius); padding: 12px;
    }
    .panel-h {
      display: flex; align-items: center; justify-content: space-between;
      font-size: 11px; font-weight: 600; color: var(--ink-3);
      text-transform: uppercase; letter-spacing: 0.08em;
      margin-bottom: 10px;
    }
    .count {
      background: var(--bg-3); color: var(--ink-2);
      padding: 1px 6px; border-radius: 999px;
      font-family: var(--mono); font-size: 10px;
      text-transform: none; letter-spacing: 0;
    }
    .scene-item {
      display: flex; align-items: center; gap: 10px;
      padding: 8px 10px;
      border-radius: var(--radius-sm);
      cursor: pointer;
      transition: background 0.15s;
      border: 1px solid transparent;
    }
    .scene-item:hover { background: var(--bg-3); }
    .scene-item .swatch { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
    .scene-item .name { font-size: 13px; flex: 1; }
    .scene-item .meta { font-family: var(--mono); font-size: 10.5px; color: var(--ink-3); }
    .field-row {
      display: flex; align-items: center; justify-content: space-between;
      padding: 6px 0; font-size: 12.5px;
      border-bottom: 1px solid var(--line);
    }
    .field-row:last-child { border-bottom: 0; }
    .field-row .lbl { color: var(--ink-3); }
    .field-row .val { font-family: var(--mono); color: var(--ink); font-size: 12px; }
    .slider-row { padding: 8px 0; border-bottom: 1px solid var(--line); }
    .slider-row:last-child { border-bottom: 0; padding-bottom: 0; }
    .slider-row .top { display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 12px; }
    .slider-row .top .lbl { color: var(--ink-3); }
    .slider-row .top .val { font-family: var(--mono); color: var(--ink); }
    input[type="range"] {
      -webkit-appearance: none; appearance: none;
      width: 100%; height: 4px;
      background: var(--bg-3); border-radius: 2px; outline: none;
    }
    input[type="range"]::-webkit-slider-thumb {
      -webkit-appearance: none; appearance: none;
      width: 14px; height: 14px; border-radius: 50%;
      background: var(--accent); cursor: pointer;
      border: 2px solid var(--bg-2);
      box-shadow: 0 0 0 1px var(--line-2);
    }
    .pipeline { display: flex; gap: 4px; align-items: center; flex-wrap: wrap; margin-top: 6px; }
    .stage {
      flex: 1; min-width: 50px;
      padding: 4px 6px;
      background: var(--bg-3); border: 1px solid var(--line);
      border-radius: 6px; font-size: 9.5px; text-align: center;
      color: var(--ink-2); font-family: var(--mono);
    }
    .stage.live { border-color: var(--accent-2); color: var(--accent-2); }
    .stage-arrow { color: var(--ink-4); font-size: 10px; }
  `;Ce=pt([h("nv-sidebar")],Ce);var ut=Object.defineProperty,vt=Object.getOwnPropertyDescriptor,be=(e,t,s,r)=>{for(var a=r>1?void 0:r?vt(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&ut(t,s,a),a};let L=class extends b{constructor(){super(...arguments),this.items=[{id:"rebar",x:740,y:240,color:"oklch(0.72 0.18 330)",name:"rebar.steel"},{id:"heart",x:220,y:180,color:"oklch(0.78 0.14 195)",name:"heart_proxy"},{id:"mains",x:180,y:380,color:"oklch(0.72 0.18 330)",name:"mains_60Hz"},{id:"door",x:800,y:470,color:"oklch(0.78 0.14 145)",name:"door.steel"}],this.dragging=null,this.selected=null,this.dragOffset={dx:0,dy:0},this.onDown=(e,t)=>{t.preventDefault(),this.dragging=e,this.selected=e;const s=this.items.find(i=>i.id===e);if(!s)return;const r=this.renderRoot.querySelector("svg");if(!r)return;const a=this.toSvg(t,r);this.dragOffset={dx:a.x-s.x,dy:a.y-s.y}},this.onPointerMove=e=>{if(!this.dragging)return;const t=this.renderRoot.querySelector("svg");if(!t)return;const s=this.toSvg(e,t);this.items=this.items.map(r=>r.id===this.dragging?{...r,x:s.x-this.dragOffset.dx,y:s.y-this.dragOffset.dy}:r)},this.onPointerUp=()=>{this.dragging=null}}connectedCallback(){super.connectedCallback(),x(()=>{ne.value,D.value,k.value,T.value,$.value,this.requestUpdate()}),window.addEventListener("pointermove",this.onPointerMove),window.addEventListener("pointerup",this.onPointerUp)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("pointermove",this.onPointerMove),window.removeEventListener("pointerup",this.onPointerUp)}toSvg(e,t){const s=t.getBoundingClientRect(),r=(e.clientX-s.left)/s.width*1e3,a=(e.clientY-s.top)/s.height*600;return{x:r,y:a}}render(){const e=ne.value,t=[e[0]*1e9,e[1]*1e9,e[2]*1e9],s=D.value*1e9,r=$.value?"":"anim";return c`
      <div class="grid"></div>
      <svg viewBox="0 0 1000 600" preserveAspectRatio="xMidYMid meet" id="scene-svg">
        <defs>
          <radialGradient id="g-sensor" cx="50%" cy="50%" r="50%">
            <stop offset="0" stop-color="oklch(0.78 0.14 70)" stop-opacity="0.4"/>
            <stop offset="1" stop-color="oklch(0.78 0.14 70)" stop-opacity="0"/>
          </radialGradient>
          <filter id="glow"><feGaussianBlur stdDeviation="3" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
        </defs>

        <!-- Field lines from each source to sensor -->
        ${this.items.map(a=>U`
          <line class="field-line ${r}" x1=${a.x} y1=${a.y}
            x2="500" y2="320"
            stroke=${a.color} stroke-width="1" stroke-opacity="0.5"/>
        `)}

        <!-- Source primitives -->
        ${this.items.map(a=>U`
          <g class=${`draggable ${this.dragging===a.id?"dragging":""} ${this.selected===a.id?"selected":""}`}
             data-id=${a.id} data-source-id=${a.id}
             transform=${`translate(${a.x.toFixed(0)},${a.y.toFixed(0)})`}
             @pointerdown=${i=>this.onDown(a.id,i)}>
            <ellipse cx="0" cy="0" rx="32" ry="22" fill=${a.color} fill-opacity="0.18"
              stroke=${a.color} stroke-width="1.2"/>
            <circle cx="0" cy="0" r="4" fill=${a.color}/>
            <text class="label" x="0" y="40" text-anchor="middle">${a.name}</text>
          </g>
        `)}

        <!-- Sensor (NV diamond) at center -->
        <g id="sensor-g" class="draggable" data-id="sensor" transform="translate(500, 320)">
          <circle cx="0" cy="0" r="46" fill="url(#g-sensor)"/>
          <g class=${`crystal ${r}`} stroke="oklch(0.78 0.14 70)" stroke-width="2"
             fill="oklch(0.78 0.14 70 / 0.08)" filter="url(#glow)">
            <polygon points="0,-22 19,-7 12,18 -12,18 -19,-7"/>
          </g>
          <circle cx="0" cy="0" r="3" fill="var(--accent)"/>
          <text class="label" x="0" y="56" text-anchor="middle">
            sensor · 〈111〉 NV
          </text>
          <text class="label" x="0" y="72" text-anchor="middle">
            B_in: <tspan fill="var(--accent)" id="b-in-svg">[${t[0].toFixed(2)}, ${t[1].toFixed(2)}, ${t[2].toFixed(2)}] nT</tspan>
          </text>
        </g>
      </svg>

      <div class="scene-readout">
        <div class="stat-card">
          <div class="lbl">|B|</div>
          <div class="val amber" id="bmag-readout">${s.toFixed(3)} nT</div>
        </div>
        <div class="stat-card">
          <div class="lbl">FPS</div>
          <div class="val cyan" id="fps-readout">${k.value>0?Math.round(k.value):"—"}</div>
        </div>
        <div class="stat-card">
          <div class="lbl">SNR</div>
          <div class="val mint" id="snr-readout">${T.value>0?T.value.toFixed(1):"—"}</div>
        </div>
      </div>
    `}};L.styles=m`
    :host {
      display: block; height: 100%; width: 100%;
      background: radial-gradient(ellipse at 50% 30%, var(--bg-2) 0%, var(--bg-0) 70%);
      position: relative; overflow: hidden;
      border-bottom: 1px solid var(--line);
    }
    .grid {
      position: absolute; inset: 0;
      background-image:
        linear-gradient(var(--grid) 1px, transparent 1px),
        linear-gradient(90deg, var(--grid) 1px, transparent 1px);
      background-size: 32px 32px;
      pointer-events: none;
      mask-image: radial-gradient(ellipse at center, black 40%, transparent 100%);
    }
    svg { position: absolute; inset: 0; width: 100%; height: 100%; }
    .stat-card {
      background: rgba(13,17,23,0.7);
      backdrop-filter: blur(8px);
      border: 1px solid var(--line);
      border-radius: var(--radius-sm);
      padding: 8px 12px;
      font-size: 11px;
      min-width: 96px;
    }
    [data-theme="light"] .stat-card { background: rgba(255,255,255,0.85); }
    .stat-card .lbl {
      color: var(--ink-3);
      text-transform: uppercase; font-weight: 600; letter-spacing: 0.06em; font-size: 9.5px;
    }
    .stat-card .val { font-family: var(--mono); font-size: 16px; font-weight: 600; margin-top: 2px; }
    .stat-card .val.amber { color: var(--accent); }
    .stat-card .val.cyan { color: var(--accent-2); }
    .stat-card .val.mint { color: var(--accent-4); }
    .scene-readout {
      position: absolute; top: 14px; right: 14px;
      display: flex; gap: 8px; z-index: 5;
    }
    .draggable { cursor: grab; transition: filter 0.15s; }
    .draggable:hover { filter: brightness(1.15) drop-shadow(0 0 6px currentColor); }
    .draggable.dragging { cursor: grabbing; filter: brightness(1.25) drop-shadow(0 0 10px currentColor); }
    .field-line { stroke-dasharray: 4 6; }
    @keyframes dash { to { stroke-dashoffset: -200; } }
    .field-line.anim { animation: dash 4s linear infinite; }
    @keyframes spin {
      0% { transform: rotateY(0) rotateX(8deg); }
      100% { transform: rotateY(360deg) rotateX(8deg); }
    }
    .crystal { transform-origin: center; transform-box: fill-box; }
    .crystal.anim { animation: spin 12s linear infinite; }
    .label {
      font-family: var(--mono); font-size: 11px; fill: var(--ink-2);
      pointer-events: none;
    }
  `;be([u()],L.prototype,"items",2);be([u()],L.prototype,"dragging",2);be([u()],L.prototype,"selected",2);L=be([h("nv-scene")],L);var gt=Object.defineProperty,mt=Object.getOwnPropertyDescriptor,Re=(e,t,s,r)=>{for(var a=r>1?void 0:r?mt(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&gt(t,s,a),a};let pe=class extends b{constructor(){super(...arguments),this.tab="signal"}connectedCallback(){super.connectedCallback(),x(()=>{oe.value,le.value,ce.value,de.value,_e.value,C.value,f.value,ne.value,D.value,this.requestUpdate()})}async verify(){const e=P();if(e){f.value="pending",l("info","verifying witness over 256 frames…");try{const t=I.value,s=new Uint8Array(32);for(let a=0;a<32;a++)s[a]=parseInt(t.slice(a*2,a*2+2),16);const r=await e.verifyWitness(s);if(r.ok)f.value="ok",C.value=t,l("ok",`witness ${t.slice(0,16)}… matches · determinism gate ✓`);else{f.value="fail";const a=Array.from(r.actual).map(i=>i.toString(16).padStart(2,"0")).join("");C.value=a,l("err",`WITNESS MISMATCH actual=${a.slice(0,16)}…`)}}catch(t){f.value="fail",l("err",`verify failed: ${t.message}`)}}}renderSignalTab(){const i=n=>{let d="";return n.forEach((p,O)=>{const fe=O/Math.max(1,199)*320,xe=65-p*22;d+=(O===0?"M":"L")+` ${fe.toFixed(1)} ${xe.toFixed(1)} `}),d};return c`
      <div class="card">
        <div class="card-h">
          <span class="ttl">B-vector trace</span>
          <span class="badge">3-axis · nT</span>
        </div>
        <svg viewBox="0 0 ${320} ${130}" preserveAspectRatio="none">
          <line x1="0" y1=${65} x2=${320} y2=${65} stroke="var(--line)" stroke-width="0.5"/>
          ${U`<path id="trace-x" d=${i(oe.value)} stroke="oklch(0.78 0.14 70)" stroke-width="1.2" fill="none"/>`}
          ${U`<path id="trace-y" d=${i(le.value)} stroke="oklch(0.78 0.12 195)" stroke-width="1.2" fill="none" opacity="0.8"/>`}
          ${U`<path id="trace-z" d=${i(ce.value)} stroke="oklch(0.72 0.18 330)" stroke-width="1.2" fill="none" opacity="0.7"/>`}
        </svg>
      </div>

      <div class="card">
        <div class="card-h">
          <span class="ttl">Frame stream</span>
          <span class="badge" id="strip-rate">live</span>
        </div>
        <div class="frame-strip" id="frame-strip">
          ${de.value.map(n=>c`<div class="bar" style=${`height:${Math.max(4,n*100)}%`}></div>`)}
        </div>
      </div>
    `}renderFrameTab(){const e=_e.value,t=e?.raw;let s="";return t&&(s=Array.from(t).map(a=>a.toString(16).padStart(2,"0")).slice(0,60).join(" ")),c`
      <div class="card">
        <div class="card-h">
          <span class="ttl">MagFrame v1 fields</span>
          <span class="badge">60 B</span>
        </div>
        <table>
          <tr><td>magic</td><td id="frame-magic">${e?"0x"+e.magic.toString(16).toUpperCase():"—"}</td></tr>
          <tr><td>version</td><td>${e?.version??"—"}</td></tr>
          <tr><td>flags</td><td>0x${(e?.flags??0).toString(16).padStart(4,"0")}</td></tr>
          <tr><td>sensor_id</td><td>${e?.sensorId??"—"}</td></tr>
          <tr><td>t_us</td><td>${e?e.tUs.toString():"—"}</td></tr>
          <tr><td>b_pT[0]</td><td id="frame-bx">${e?e.bPt[0].toFixed(1):"—"}</td></tr>
          <tr><td>b_pT[1]</td><td id="frame-by">${e?e.bPt[1].toFixed(1):"—"}</td></tr>
          <tr><td>b_pT[2]</td><td id="frame-bz">${e?e.bPt[2].toFixed(1):"—"}</td></tr>
          <tr><td>noise_floor</td><td>${e?e.noiseFloorPtSqrtHz.toFixed(2):"—"}</td></tr>
          <tr><td>temp_K</td><td>${e?e.temperatureK.toFixed(1):"—"}</td></tr>
        </table>
      </div>
      <div class="card">
        <div class="card-h">
          <span class="ttl">Hex dump</span>
          <span class="badge">LE</span>
        </div>
        <div class="hex" id="frame-hex">${s||"—"}</div>
      </div>
    `}renderWitnessTab(){const e=f.value,t=e==="ok"?"ok":e==="fail"?"fail":"",s=e==="pending"?"Verifying…":e==="ok"?"✓ Witness verified · determinism gate":e==="fail"?"✗ Witness mismatch · audit required":"Verify witness";return c`
      <div class="card">
        <div class="card-h">
          <span class="ttl">Expected (Proof::EXPECTED_WITNESS_HEX)</span>
          <span class="badge">SHA-256</span>
        </div>
        <div class="witness-box" id="expected-witness">${I.value||"(loading…)"}</div>
      </div>
      <div class="card">
        <div class="card-h">
          <span class="ttl">Actual (last verify)</span>
          <span class="badge">SHA-256</span>
        </div>
        <div class="witness-box" id="actual-witness">${C.value||"(not verified yet)"}</div>
        <button class="verify-btn ${t}" id="verify-btn" @click=${this.verify}>${s}</button>
      </div>
    `}render(){return c`
      <div class="tabs">
        <button class="tab ${this.tab==="signal"?"active":""}" data-pane="signal" @click=${()=>this.tab="signal"}>Signal</button>
        <button class="tab ${this.tab==="frame"?"active":""}" data-pane="frame" @click=${()=>this.tab="frame"}>Frame</button>
        <button class="tab ${this.tab==="witness"?"active":""}" data-pane="witness" @click=${()=>this.tab="witness"}>Witness</button>
      </div>
      <div class="body">
        ${this.tab==="signal"?this.renderSignalTab():this.tab==="frame"?this.renderFrameTab():this.renderWitnessTab()}
      </div>
    `}};pe.styles=m`
    :host {
      display: flex; flex-direction: column;
      background: var(--bg-1);
      border-left: 1px solid var(--line);
      overflow: hidden;
      height: 100%;
    }
    .tabs {
      display: flex; border-bottom: 1px solid var(--line);
    }
    .tab {
      flex: 1;
      padding: 11px 8px;
      background: transparent; border: none;
      font-size: 11.5px; font-weight: 500;
      color: var(--ink-3);
      border-bottom: 2px solid transparent;
      cursor: pointer; transition: color 0.15s, border-color 0.15s;
    }
    .tab.active { color: var(--ink); border-bottom-color: var(--accent); }
    .tab:hover { color: var(--ink-2); }
    .body { padding: 14px; flex: 1; overflow-y: auto; }

    .card {
      background: var(--bg-2); border: 1px solid var(--line);
      border-radius: var(--radius); padding: 12px;
      margin-bottom: 12px;
    }
    .card-h {
      display: flex; justify-content: space-between; align-items: center;
      margin-bottom: 8px;
    }
    .card-h .ttl { font-size: 12px; font-weight: 600; }
    .badge {
      font-family: var(--mono); font-size: 10px;
      padding: 2px 6px;
      background: oklch(0.78 0.14 195 / 0.12);
      color: var(--accent-2);
      border-radius: 4px;
      border: 1px solid oklch(0.78 0.14 195 / 0.3);
    }
    svg { width: 100%; height: 130px; }
    .frame-strip {
      height: 28px;
      display: flex; align-items: flex-end; gap: 1px;
      padding: 4px 0;
    }
    .bar {
      flex: 1;
      background: linear-gradient(to top, var(--accent-2), var(--accent));
      border-radius: 1px;
      min-height: 2px;
    }
    table { width: 100%; border-collapse: collapse; font-family: var(--mono); font-size: 10.5px; }
    td { padding: 4px 0; border-bottom: 1px solid var(--line); }
    td:first-child { color: var(--ink-3); }
    td:last-child { text-align: right; color: var(--ink); }
    .hex {
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: var(--radius-sm);
      padding: 10px;
      font-family: var(--mono);
      font-size: 10.5px;
      color: var(--ink-2);
      line-height: 1.6;
      overflow-x: auto;
      white-space: nowrap;
    }
    .hex .magic { color: var(--accent); font-weight: 600; }
    .witness-box {
      font-family: var(--mono);
      font-size: 11px;
      color: var(--ink-2);
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 8px 10px;
      word-break: break-all;
      line-height: 1.5;
    }
    .verify-btn {
      margin-top: 10px;
      width: 100%;
      padding: 8px;
      border: 1px solid var(--line);
      background: var(--bg-3);
      color: var(--ink);
      border-radius: 8px;
      cursor: pointer;
      font-family: var(--mono);
      font-size: 12px;
    }
    .verify-btn:hover { border-color: var(--accent); }
    .verify-btn.ok { border-color: var(--ok); color: var(--ok); }
    .verify-btn.fail { border-color: var(--bad); color: var(--bad); }
  `;Re([u()],pe.prototype,"tab",2);pe=Re([h("nv-inspector")],pe);var bt=Object.defineProperty,ht=Object.getOwnPropertyDescriptor,je=(e,t,s,r)=>{for(var a=r>1?void 0:r?ht(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&bt(t,s,a),a};let ue=class extends b{constructor(){super(...arguments),this.history=[],this.hIdx=-1,this.onKey=e=>{e.key==="Enter"?(this.exec(this.inputEl.value),this.inputEl.value=""):e.key==="ArrowUp"?this.history.length&&(this.hIdx=Math.max(0,this.hIdx-1),this.inputEl.value=this.history[this.hIdx]??"",e.preventDefault()):e.key==="ArrowDown"&&this.history.length&&(this.hIdx=Math.min(this.history.length,this.hIdx+1),this.inputEl.value=this.history[this.hIdx]??"",e.preventDefault())}}connectedCallback(){super.connectedCallback(),x(()=>{S.value,we.value,N.value,this.requestUpdate()})}updated(){const e=this.renderRoot.querySelector(".body");e&&(e.scrollTop=e.scrollHeight)}counts(){const e={info:0,warn:0,err:0,dbg:0,ok:0};for(const t of S.value)e[t.level]=(e[t.level]??0)+1;return e.all=S.value.length,e}async exec(e){if(e=e.trim(),!e)return;l("info",`<span style="color:var(--accent);">nvsim&gt;</span> ${e}`),this.history.push(e),this.hIdx=this.history.length;const[t,...s]=e.split(/\s+/),r=s.join(" "),a=P();switch(t){case"help":l("info","commands: help · scene.list · sensor.config · run · pause · seed · proof.verify · clear · theme · status");break;case"scene.list":l("info","scene rebar-walkby-01:"),l("info","  rebar.steel.coil   @ [+2.7, 0.0, +0.3] m χ=5000"),l("info","  dipole.heart_proxy @ [-1.4, +0.2, +0.4] m m=1.0e-6 A·m²"),l("info","  loop.mains_60Hz    @ [-1.6, -0.4, 0.0] m I=2 A"),l("info","  eddy.door_steel    @ [+0.0, +1.8, +0.4] m σ=1e6 S/m");break;case"sensor.config":l("info","NvSensor::cots_defaults() {"),l("info","  pos=[0,0,0], V=1mm³, N=1e12, C=0.03, T2*=200ns"),l("info","  D=2.870 GHz, γe=28 GHz/T, Γ=1.0 MHz, axes=4×〈111〉"),l("info","  δB ≈ 1.18 pT/√Hz (Barry 2020 §III.A) }");break;case"run":a&&(await a.run(),v.value=!0,l("ok","pipeline RUN"));break;case"pause":a&&(await a.pause(),v.value=!1,l("warn","pipeline PAUSED"));break;case"reset":a&&(await a.reset(),l("info","pipeline reset · t=0"));break;case"seed":{if(!r){l("info",`current seed = 0x${q.value.toString(16).toUpperCase()}`);break}const i=BigInt(r.startsWith("0x")?r:"0x"+r);q.value=i,a&&await a.setSeed(i),l("ok",`seed → 0x${i.toString(16).toUpperCase()}`);break}case"proof.verify":{if(!a)break;l("dbg","computing SHA-256 over 256 frames…");try{const i=I.value,n=new Uint8Array(32);for(let p=0;p<32;p++)n[p]=parseInt(i.slice(p*2,p*2+2),16);(await a.verifyWitness(n)).ok?(f.value="ok",C.value=i,l("ok",`witness ${i.slice(0,16)}… matches · determinism gate ✓`)):(f.value="fail",l("err","WITNESS MISMATCH"))}catch(i){l("err",`verify failed: ${i.message}`)}break}case"clear":S.value=[];break;case"theme":{const i=(r||"").toLowerCase();i==="light"||i==="dark"?(g.value=i,l("ok",`theme → ${i}`)):l("info","theme [light|dark]");break}case"status":l("info",`running=${v.value} seed=0x${q.value.toString(16).toUpperCase()} verified=${f.value}`);break;default:l("err",`unknown command: ${t} · try help`)}}render(){const e=this.counts(),t=we.value,s=S.value.filter(r=>t==="all"||r.level===t);return c`
      <div class="tabs">
        ${["all","info","warn","err","dbg"].map(r=>c`
          <button class="tab ${t===r?"active":""}" data-tab=${r}
            @click=${()=>we.value=r}>
            ${r} <span class="cnt">${e[r]??0}</span>
          </button>
        `)}
        <span class="spacer"></span>
        <div class="tools">
          <button id="clear-log" title="Clear" @click=${()=>S.value=[]}>×</button>
          <button id="pause-log" title="Pause" @click=${()=>N.value=!N.value}>
            ${N.value?"▶":"❚❚"}
          </button>
        </div>
      </div>
      <div class="body">
        ${s.map(r=>{const a=new Date(r.ts),i=`${String(a.getSeconds()).padStart(2,"0")}.${String(a.getMilliseconds()).padStart(3,"0")}`;return c`<div class="line ${r.level}">
            <div class="ts">${i}</div>
            <div class="lvl">${r.level}</div>
            <div class="msg" .innerHTML=${r.msg}></div>
          </div>`})}
      </div>
      <div class="input">
        <span class="prompt">nvsim&gt;</span>
        <input id="console-input" type="text"
          placeholder="help · scene.list · sensor.config · run · proof.verify · clear"
          @keydown=${this.onKey}/>
      </div>
    `}};ue.styles=m`
    :host {
      display: flex; flex-direction: column;
      background: var(--bg-1);
      overflow: hidden;
    }
    .tabs {
      display: flex; align-items: center;
      border-bottom: 1px solid var(--line);
      padding: 0 10px;
      gap: 2px;
    }
    .tab {
      padding: 8px 12px;
      background: transparent; border: none;
      font-size: 11.5px; color: var(--ink-3);
      font-family: var(--mono);
      border-bottom: 2px solid transparent;
      cursor: pointer;
      margin-bottom: -1px;
    }
    .tab.active { color: var(--ink); border-bottom-color: var(--accent); }
    .tab .cnt {
      background: var(--bg-3); padding: 1px 5px; border-radius: 999px;
      font-size: 9.5px; color: var(--ink-2); margin-left: 4px;
    }
    .spacer { flex: 1; }
    .tools { display: flex; gap: 4px; padding: 4px 0; }
    .tools button {
      width: 24px; height: 24px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px;
      color: var(--ink-3);
      font-size: 11px; cursor: pointer;
    }
    .tools button:hover { color: var(--ink); border-color: var(--line-2); }

    .body {
      flex: 1; overflow-y: auto;
      font-family: var(--mono);
      font-size: 11.5px;
      padding: 6px 0;
      background: var(--bg-0);
    }
    .line {
      display: grid;
      grid-template-columns: 70px 60px 1fr;
      gap: 12px;
      padding: 2px 12px;
      color: var(--ink-2);
      border-left: 2px solid transparent;
    }
    .line:hover { background: var(--bg-1); }
    .ts { color: var(--ink-4); font-size: 10.5px; padding-top: 1px; }
    .lvl {
      font-size: 10px; font-weight: 600;
      text-transform: uppercase; letter-spacing: 0.04em; padding-top: 1px;
    }
    .line.info .lvl { color: var(--accent-2); }
    .line.warn .lvl { color: var(--warn); }
    .line.warn { border-left-color: var(--warn); background: oklch(0.7 0.18 35 / 0.04); }
    .line.err .lvl { color: var(--bad); }
    .line.err { border-left-color: var(--bad); background: oklch(0.65 0.22 25 / 0.05); }
    .line.dbg .lvl { color: var(--ink-3); }
    .line.ok .lvl { color: var(--ok); }
    .msg { color: var(--ink); white-space: pre-wrap; word-break: break-word; }

    .input {
      display: flex; align-items: center;
      border-top: 1px solid var(--line);
      background: var(--bg-0);
      padding: 0 10px;
      height: 32px; gap: 8px;
    }
    .prompt { color: var(--accent); font-family: var(--mono); font-size: 12px; }
    input[type="text"] {
      flex: 1; background: transparent; border: none; outline: none;
      color: var(--ink); font-family: var(--mono); font-size: 12px;
      height: 100%;
    }
    input::placeholder { color: var(--ink-4); }
  `;je([Ie("#console-input")],ue.prototype,"inputEl",2);ue=je([h("nv-console")],ue);const B=[{id:"nvsim",name:"nvsim — NV-diamond magnetometer",category:"sim",crate:"nvsim",summary:"Deterministic forward simulator: scene → Biot–Savart → NV ensemble → ADC → MagFrame stream + SHA-256 witness.",budget:"L",active:!0,status:"available",tags:["quantum","magnetometer","simulator","witness","wasm"],adr:"ADR-089"},{id:"gesture",name:"Gesture (DTW)",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Dynamic-Time-Warping gesture classifier from CSI motion templates.",events:[1],budget:"M",status:"available",tags:["hci","csi","classifier","dtw"],adr:"ADR-014"},{id:"coherence",name:"Coherence gate",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Z-score coherence scoring + Accept/PredictOnly/Reject/Recalibrate gate.",events:[2],budget:"S",status:"available",tags:["gate","csi","coherence","drift"],adr:"ADR-029"},{id:"adversarial",name:"Adversarial-signal detector",category:"ais",crate:"wifi-densepose-wasm-edge",summary:"Physically-impossible-signal detector — multi-link consistency, used to flag spoofed CSI.",events:[3],budget:"M",status:"available",tags:["security","csi","spoofing","mesh"],adr:"ADR-032"},{id:"rvf",name:"RVF — Rust Verified Feature stream",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Verified-frame builder with SHA-256 hash + version metadata for the feature stream.",budget:"S",status:"available",tags:["witness","csi","hash"],adr:"ADR-040"},{id:"occupancy",name:"Occupancy estimator",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Through-wall presence + person-count via CSI amplitude perturbation.",events:[300,301,302],budget:"S",status:"available",tags:["csi","building","presence"]},{id:"vital_trend",name:"Vital-trend monitor",category:"med",crate:"wifi-densepose-wasm-edge",summary:"HR + BR trend tracking with bradycardia/tachycardia/apnea events.",events:[100,101,102,103,104,105],budget:"S",status:"available",tags:["medical","vitals","csi"],adr:"ADR-021"},{id:"intrusion",name:"Intrusion detector",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Zone-based intrusion alert from CSI motion patterns.",events:[200,201],budget:"S",status:"available",tags:["security","zone","csi"]},{id:"med_sleep_apnea",name:"Sleep-apnea detector",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Episodic respiratory pause detection during sleep cycles.",events:[105],budget:"S",status:"available",tags:["medical","sleep","breathing"]},{id:"med_cardiac_arrhythmia",name:"Cardiac arrhythmia",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Beat-to-beat irregularity classifier from cardiac micro-Doppler.",events:[103,104],budget:"M",status:"available",tags:["medical","cardiac","arrhythmia"]},{id:"med_respiratory_distress",name:"Respiratory distress",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Distress signature: rapid shallow breathing + accessory-muscle motion.",events:[101,102],budget:"S",status:"available",tags:["medical","breathing","icu"]},{id:"med_gait_analysis",name:"Gait analysis",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Stride length, cadence, asymmetry from through-wall CSI pose tracking.",budget:"M",status:"available",tags:["medical","gait","pose"]},{id:"med_seizure_detect",name:"Seizure detector",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Tonic-clonic seizure motion signature.",budget:"M",status:"beta",tags:["medical","neuro"]},{id:"sec_perimeter_breach",name:"Perimeter breach",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Approach/departure detection at user-defined boundary segments.",events:[210,211,212,213],budget:"S",status:"available",tags:["security","perimeter"]},{id:"sec_weapon_detect",name:"Metal anomaly / weapon",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Metal-perturbation flag in CSI; potential weapon presence (research).",events:[220,221,222],budget:"M",status:"research",tags:["security","metal","csi"]},{id:"sec_tailgating",name:"Tailgating detector",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Detect 2+ persons crossing a single-passage threshold.",events:[230,231,232],budget:"S",status:"available",tags:["security","access-control"]},{id:"sec_loitering",name:"Loitering detector",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Stationary occupancy past a configurable dwell threshold.",events:[240,241,242],budget:"S",status:"available",tags:["security","dwell"]},{id:"sec_panic_motion",name:"Panic motion",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"High-energy distress motion: struggle / fleeing pattern.",events:[250,251,252],budget:"S",status:"beta",tags:["security","distress"]},{id:"bld_hvac_presence",name:"HVAC presence",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Occupied/activity-level/departure-countdown for HVAC zones.",events:[310,311,312],budget:"S",status:"available",tags:["hvac","building","energy"]},{id:"bld_lighting_zones",name:"Lighting zones",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Per-zone light on/dim/off cues from occupancy.",events:[320,321,322],budget:"S",status:"available",tags:["lighting","building"]},{id:"bld_elevator_count",name:"Elevator count",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Person count inside elevator car from CSI.",events:[330],budget:"S",status:"available",tags:["elevator","building"]},{id:"bld_meeting_room",name:"Meeting-room utilization",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Meeting size + duration analytics for booking systems.",budget:"S",status:"available",tags:["meeting","analytics"]},{id:"bld_energy_audit",name:"Energy audit",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Continuous occupancy-vs-HVAC-state audit for energy savings.",budget:"M",status:"available",tags:["energy","audit"]},{id:"ret_queue_length",name:"Queue length",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Live queue-length tracking for checkout / kiosks.",budget:"S",status:"available",tags:["retail","queue"]},{id:"ret_dwell_heatmap",name:"Dwell heatmap",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Per-zone dwell time accumulation; analytics-only export.",budget:"M",status:"available",tags:["retail","heatmap"]},{id:"ret_customer_flow",name:"Customer flow",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Origin-destination flow graph through a store layout.",budget:"M",status:"available",tags:["retail","flow"]},{id:"ret_table_turnover",name:"Table turnover",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Restaurant table seat / vacate transitions.",budget:"S",status:"available",tags:["retail","restaurant"]},{id:"ret_shelf_engagement",name:"Shelf engagement",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Reach-to-shelf gestures and dwell at product zones.",budget:"M",status:"available",tags:["retail","shelf"]},{id:"ind_forklift_proximity",name:"Forklift proximity",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Worker-near-forklift safety alert.",budget:"S",status:"available",tags:["industrial","safety"]},{id:"ind_confined_space",name:"Confined-space monitor",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Last-person-out detection + presence audit for OSHA confined-space entries.",budget:"S",status:"available",tags:["industrial","osha"]},{id:"ind_clean_room",name:"Clean-room PPE / motion",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Motion patterns consistent with proper PPE-clad movement.",budget:"M",status:"beta",tags:["industrial","cleanroom"]},{id:"ind_livestock_monitor",name:"Livestock monitor",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Vital-sign + activity tracking for stall-bound livestock.",budget:"M",status:"beta",tags:["agriculture","livestock"]},{id:"ind_structural_vibration",name:"Structural vibration",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Building/equipment micro-vibration via CSI phase derivative.",budget:"M",status:"research",tags:["industrial","vibration"]},{id:"sig_coherence_gate",name:"Coherence gate (extended)",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Hysteresis + multi-state coherence gate driving downstream apps.",budget:"S",status:"available",tags:["gate","csi"]},{id:"sig_flash_attention",name:"Flash attention (CSI)",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Edge-friendly attention block for CSI subcarrier weighting.",budget:"M",status:"beta",tags:["attention","csi"]},{id:"sig_temporal_compress",name:"Temporal-tensor compress",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"RuVector temporal-tensor compression on the CSI buffer.",budget:"M",status:"available",tags:["compress","tensor"]},{id:"sig_sparse_recovery",name:"Sparse recovery",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"114→56 subcarrier sparse interpolation via L1 solver.",budget:"M",status:"available",tags:["sparse","csi"]},{id:"sig_mincut_person_match",name:"Mincut person-match",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Min-cut person assignment across multistatic frames.",budget:"M",status:"available",tags:["mincut","matching"]},{id:"sig_optimal_transport",name:"Optimal transport",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"OT-based feature alignment between mesh nodes.",budget:"M",status:"beta",tags:["ot","alignment"]},{id:"lrn_dtw_gesture_learn",name:"DTW gesture learn",category:"lrn",crate:"wifi-densepose-wasm-edge",summary:"On-device template learning for personalized gesture libraries.",budget:"M",status:"beta",tags:["lifelong","gesture"]},{id:"lrn_anomaly_attractor",name:"Anomaly attractor",category:"lrn",crate:"wifi-densepose-wasm-edge",summary:"Novelty detector with dynamic-attractor recall.",budget:"M",status:"research",tags:["novelty","lifelong"]},{id:"lrn_meta_adapt",name:"Meta-adapt",category:"lrn",crate:"wifi-densepose-wasm-edge",summary:"Meta-learning adapter for fast site-to-site transfer.",budget:"L",status:"research",tags:["meta-learning"]},{id:"lrn_ewc_lifelong",name:"EWC++ lifelong",category:"lrn",crate:"wifi-densepose-wasm-edge",summary:"Elastic-weight-consolidation gate to avoid catastrophic forgetting.",budget:"M",status:"beta",tags:["lifelong","ewc"]},{id:"spt_pagerank_influence",name:"PageRank influence",category:"spt",crate:"wifi-densepose-wasm-edge",summary:"Graph-influence ranking on the multistatic mesh.",budget:"M",status:"beta",tags:["graph","pagerank"]},{id:"spt_micro_hnsw",name:"µHNSW vector index",category:"spt",crate:"wifi-densepose-wasm-edge",summary:"Tiny HNSW index for AETHER re-ID embeddings on-device.",budget:"M",status:"available",tags:["hnsw","reid"]},{id:"spt_spiking_tracker",name:"Spiking tracker",category:"spt",crate:"wifi-densepose-wasm-edge",summary:"Spiking-network multi-target tracker.",budget:"L",status:"research",tags:["snn","tracker"]},{id:"tmp_pattern_sequence",name:"Pattern sequence",category:"tmp",crate:"wifi-densepose-wasm-edge",summary:"Sequence-of-events pattern matcher (e.g. ingress→linger→egress).",budget:"M",status:"available",tags:["temporal","pattern"]},{id:"tmp_temporal_logic_guard",name:"Temporal logic guard",category:"tmp",crate:"wifi-densepose-wasm-edge",summary:"LTL/MTL safety-property guard over event streams.",budget:"M",status:"beta",tags:["ltl","safety"]},{id:"tmp_goap_autonomy",name:"GOAP autonomy",category:"tmp",crate:"wifi-densepose-wasm-edge",summary:"Goal-oriented action planning for adaptive routines.",budget:"L",status:"research",tags:["planning","autonomy"]},{id:"ais_prompt_shield",name:"Prompt shield",category:"ais",crate:"wifi-densepose-wasm-edge",summary:"Edge-side LLM prompt-injection guard for on-device assistants.",budget:"M",status:"beta",tags:["security","llm"]},{id:"ais_behavioral_profiler",name:"Behavioral profiler",category:"ais",crate:"wifi-densepose-wasm-edge",summary:"Anomalous-behaviour profiler (drift in motion habits).",budget:"M",status:"beta",tags:["anomaly","behaviour"]},{id:"qnt_quantum_coherence",name:"Quantum coherence",category:"qnt",crate:"wifi-densepose-wasm-edge",summary:"Coherence diagnostics adapted for quantum-sensor signals.",budget:"M",status:"research",tags:["quantum","coherence"]},{id:"qnt_interference_search",name:"Interference search",category:"qnt",crate:"wifi-densepose-wasm-edge",summary:"Interferometric anomaly search across mesh viewpoints.",budget:"L",status:"research",tags:["quantum","interference"]},{id:"aut_psycho_symbolic",name:"Psycho-symbolic agent",category:"aut",crate:"wifi-densepose-wasm-edge",summary:"Symbolic-rule + neural-feature hybrid for low-power autonomy loops.",budget:"L",status:"research",tags:["autonomy","symbolic"]},{id:"aut_self_healing_mesh",name:"Self-healing mesh",category:"aut",crate:"wifi-densepose-wasm-edge",summary:"Mesh-topology repair with per-node health gossip.",budget:"M",status:"beta",tags:["mesh","health"]},{id:"exo_ghost_hunter",name:"Ghost hunter (anomaly)",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Empty-room CSI anomaly detector — impulsive/periodic/drift/random + hidden-presence sub-detector.",events:[650,651,652,653],budget:"S",status:"available",tags:["anomaly","paranormal","csi"],adr:"ADR-041"},{id:"exo_breathing_sync",name:"Breathing sync",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Multi-person breathing synchrony analytics.",budget:"M",status:"beta",tags:["breathing","sync"]},{id:"exo_dream_stage",name:"Dream-stage classifier",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"NREM/REM stage classification from breathing + micro-motion.",budget:"M",status:"research",tags:["sleep","rem"]},{id:"exo_emotion_detect",name:"Emotion detector",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Coarse arousal/valence from breathing + heart-rate variability.",budget:"M",status:"research",tags:["affect"]},{id:"exo_gesture_language",name:"Gesture language",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Sign-language pattern recognition.",budget:"L",status:"research",tags:["hci","sign"]},{id:"exo_happiness_score",name:"Happiness score",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Aggregate well-being score from co-occupancy + activity dynamics.",budget:"M",status:"research",tags:["affect","wellbeing"]},{id:"exo_hyperbolic_space",name:"Hyperbolic space embed",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Hyperbolic embeddings for hierarchical scene structure.",budget:"L",status:"research",tags:["embedding","hyperbolic"]},{id:"exo_music_conductor",name:"Music conductor",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Map gesture energy to MIDI tempo/dynamics.",budget:"M",status:"research",tags:["midi","art"]},{id:"exo_plant_growth",name:"Plant-growth tracker",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Slow CSI drift tracking for greenhouse foliage growth.",budget:"L",status:"research",tags:["agriculture"]},{id:"exo_rain_detect",name:"Rain detector",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Outdoor CSI signature of rainfall.",budget:"M",status:"research",tags:["weather"]},{id:"exo_time_crystal",name:"Time-crystal periodicity",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Periodicity diagnostics with anti-aliasing harmonics.",budget:"M",status:"research",tags:["periodicity"]}],j={sim:{label:"Simulators",color:"oklch(0.78 0.14 70)",range:"—"},med:{label:"Medical & Health",color:"oklch(0.65 0.22 25)",range:"100–199"},sec:{label:"Security & Safety",color:"oklch(0.7 0.18 35)",range:"200–299"},bld:{label:"Smart Building",color:"oklch(0.78 0.12 195)",range:"300–399"},ret:{label:"Retail & Hospitality",color:"oklch(0.78 0.14 145)",range:"400–499"},ind:{label:"Industrial",color:"oklch(0.72 0.18 330)",range:"500–599"},sig:{label:"Signal Processing",color:"oklch(0.78 0.14 70)",range:"600–619"},lrn:{label:"Online Learning",color:"oklch(0.78 0.12 260)",range:"620–639"},spt:{label:"Spatial / Graph",color:"oklch(0.7 0.18 100)",range:"640–659"},tmp:{label:"Temporal / Planning",color:"oklch(0.7 0.16 50)",range:"660–679"},ais:{label:"AI Safety",color:"oklch(0.65 0.22 25)",range:"700–719"},qnt:{label:"Quantum",color:"oklch(0.72 0.18 290)",range:"720–739"},aut:{label:"Autonomy",color:"oklch(0.78 0.14 145)",range:"740–759"},exo:{label:"Exotic / Research",color:"oklch(0.72 0.18 330)",range:"650–699"}};function ft(){return B.map(e=>({id:e.id,active:e.active===!0,eventCount:0}))}function xt(e,t){if(!e)return 1;const s=e.toLowerCase();let r=0;return t.id.toLowerCase().includes(s)&&(r+=3),t.name.toLowerCase().includes(s)&&(r+=3),t.summary.toLowerCase().includes(s)&&(r+=1),t.tags?.some(a=>a.toLowerCase().includes(s))&&(r+=2),t.category===s&&(r+=5),r}const yt="nvsim",wt=1,F="kv";let se=null;function He(){return se||(se=new Promise((e,t)=>{const s=indexedDB.open(yt,wt);s.onupgradeneeded=()=>{const r=s.result;r.objectStoreNames.contains(F)||r.createObjectStore(F)},s.onsuccess=()=>e(s.result),s.onerror=()=>t(s.error)}),se)}async function V(e){const t=await He();return await new Promise((s,r)=>{const i=t.transaction(F,"readonly").objectStore(F).get(e);i.onsuccess=()=>s(i.result),i.onerror=()=>r(i.error)})}async function K(e,t){const s=await He();return await new Promise((r,a)=>{const i=s.transaction(F,"readwrite");i.objectStore(F).put(t,e),i.oncomplete=()=>r(),i.onerror=()=>a(i.error)})}var kt=Object.defineProperty,$t=Object.getOwnPropertyDescriptor,We=(e,t,s,r)=>{for(var a=r>1?void 0:r?$t(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&kt(t,s,a),a};const z=o(ft()),H=o(""),M=o("all"),y=o("all");(async()=>{const e=await V("app-activations");e&&(z.value=e)})();x(()=>{const e=z.value;e.length>0&&K("app-activations",e)});let ve=class extends b{constructor(){super(...arguments),this.renderTick=0}connectedCallback(){super.connectedCallback(),x(()=>{z.value,H.value,M.value,y.value,this.renderTick++})}isActive(e){return z.value.find(t=>t.id===e)?.active===!0}toggle(e){const t=z.value.map(s=>s.id===e.id?{...s,active:!s.active,lastActivatedAt:Date.now()}:s);z.value=t,l(this.isActive(e.id)?"ok":"info",`app <span class="k">${e.id}</span> deactivated`)}filtered(){let e=B;return M.value!=="all"&&(e=e.filter(t=>t.category===M.value)),y.value!=="all"&&(e=e.filter(t=>t.status===y.value)),H.value.trim()&&(e=e.map(t=>({a:t,s:xt(H.value,t)})).filter(t=>t.s>0).sort((t,s)=>s.s-t.s).map(t=>t.a)),e}categoryCounts(){const e={all:B.length};for(const t of Object.keys(j))e[t]=0;for(const t of B)e[t.category]=(e[t.category]??0)+1;return e}render(){const e=this.filtered(),t=this.categoryCounts(),s=z.value.filter(r=>r.active).length;return c`
      <div class="head">
        <div class="ttl">
          App Store
          <small>${B.length} edge apps · ${s} active</small>
        </div>
        <input class="search" id="app-search" placeholder="Search by name, tag, or category…"
          .value=${H.value}
          @input=${r=>{H.value=r.target.value}} />
      </div>

      <div class="filters">
        <span class="chip ${M.value==="all"?"on":""}"
          @click=${()=>M.value="all"}>
          All<span class="count">${t.all}</span>
        </span>
        ${Object.keys(j).map(r=>c`
          <span class="chip ${M.value===r?"on":""}"
            @click=${()=>M.value=r}>
            <span class="swatch" style=${`background:${j[r].color}`}></span>
            ${j[r].label}
            <span class="count">${t[r]??0}</span>
          </span>
        `)}
        <span style="flex:1; min-width:8px"></span>
        <span class="chip ${y.value==="all"?"on":""}" @click=${()=>y.value="all"}>any</span>
        <span class="chip ${y.value==="available"?"on":""}" @click=${()=>y.value="available"}>available</span>
        <span class="chip ${y.value==="beta"?"on":""}" @click=${()=>y.value="beta"}>beta</span>
        <span class="chip ${y.value==="research"?"on":""}" @click=${()=>y.value="research"}>research</span>
      </div>

      ${e.length===0?c`<div class="empty">No apps match the current filters.</div>`:c`<div class="grid">${e.map(r=>this.card(r))}</div>`}
    `}card(e){const t=this.isActive(e.id),s=j[e.category];return c`
      <div class="card ${t?"active":""}" data-app-id=${e.id}>
        <div class="card-h">
          <span class="swatch" style=${`background:${s.color}`}></span>
          <span class="name">${e.name}</span>
        </div>
        <div class="summary">${e.summary}</div>
        <div class="meta">
          <span class="badge cat">${s.label}</span>
          <span class="badge status-${e.status}">${e.status}</span>
          ${e.budget?c`<span class="badge budget">budget ${e.budget}</span>`:""}
          ${e.adr?c`<span class="badge">${e.adr}</span>`:""}
          ${e.events?.length?c`<span class="badge">events ${e.events.join("·")}</span>`:""}
        </div>
        <div class="card-foot">
          <span class="events">${e.crate}</span>
          <span class="toggle ${t?"on":""}" role="switch"
            aria-checked=${t}
            data-app-toggle=${e.id}
            @click=${()=>this.toggle(e)}></span>
        </div>
      </div>
    `}};ve.styles=m`
    :host {
      display: block;
      height: 100%;
      overflow-y: auto;
      background: radial-gradient(ellipse at 50% 30%, var(--bg-2) 0%, var(--bg-0) 70%);
      padding: 24px;
    }
    .head {
      display: flex; align-items: center; gap: 16px;
      margin-bottom: 18px;
      flex-wrap: wrap;
    }
    .ttl {
      font-size: 22px; font-weight: 700; letter-spacing: -0.02em;
      color: var(--ink);
      flex: 1; min-width: 200px;
    }
    .ttl small {
      font-size: 12.5px; font-weight: 400;
      color: var(--ink-3); margin-left: 8px;
    }
    .search {
      width: 320px; max-width: 100%;
      padding: 8px 12px;
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: 8px;
      font-family: var(--mono);
      font-size: 12.5px;
      color: var(--ink); outline: none;
    }
    .search:focus { border-color: var(--accent); }
    .filters {
      display: flex; flex-wrap: wrap; gap: 6px;
      margin-bottom: 18px;
    }
    .chip {
      padding: 4px 10px;
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: 999px;
      font-size: 11.5px; color: var(--ink-3);
      cursor: pointer;
      font-family: var(--mono);
      display: inline-flex; align-items: center; gap: 4px;
    }
    .chip:hover { color: var(--ink); border-color: var(--line-2); }
    .chip.on { background: var(--bg-3); border-color: var(--accent); color: var(--ink); }
    .chip .swatch {
      width: 7px; height: 7px; border-radius: 50%;
    }
    .chip .count { color: var(--ink-3); font-size: 10px; }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
      gap: 12px;
    }
    .card {
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: var(--radius);
      padding: 12px 14px;
      display: flex; flex-direction: column; gap: 6px;
      transition: border-color 0.15s, transform 0.15s;
      position: relative;
    }
    .card:hover { border-color: var(--line-2); transform: translateY(-1px); }
    .card.active {
      border-color: oklch(0.78 0.14 145 / 0.7);
      background: linear-gradient(180deg, var(--bg-2) 0%, oklch(0.78 0.14 145 / 0.04) 100%);
    }
    .card-h {
      display: flex; align-items: flex-start; gap: 8px;
      margin-bottom: 2px;
    }
    .card-h .name {
      font-size: 13.5px; font-weight: 600; color: var(--ink);
      flex: 1; line-height: 1.3;
    }
    .card-h .swatch {
      width: 10px; height: 10px; border-radius: 50%;
      flex-shrink: 0; margin-top: 4px;
    }
    .summary {
      font-size: 12px; color: var(--ink-2); line-height: 1.45;
      flex: 1;
    }
    .meta {
      display: flex; flex-wrap: wrap; gap: 4px; margin-top: 6px;
      font-family: var(--mono); font-size: 10px;
    }
    .badge {
      padding: 1px 6px; border-radius: 4px;
      background: var(--bg-3); color: var(--ink-3);
      border: 1px solid var(--line);
    }
    .badge.cat { color: var(--accent); border-color: oklch(0.78 0.14 70 / 0.3); }
    .badge.status-available { color: var(--ok); border-color: oklch(0.78 0.14 145 / 0.4); }
    .badge.status-beta { color: var(--warn); border-color: oklch(0.7 0.18 35 / 0.4); }
    .badge.status-research { color: var(--accent-3); border-color: oklch(0.72 0.18 330 / 0.4); }
    .badge.budget { color: var(--accent-2); border-color: oklch(0.78 0.12 195 / 0.3); }
    .card-foot {
      display: flex; align-items: center; gap: 8px;
      padding-top: 8px; margin-top: 4px;
      border-top: 1px solid var(--line);
      font-size: 11px; color: var(--ink-3);
    }
    .toggle {
      position: relative;
      width: 32px; height: 18px;
      background: var(--bg-3); border: 1px solid var(--line-2);
      border-radius: 999px; cursor: pointer;
      transition: background 0.15s;
      flex-shrink: 0;
    }
    .toggle::after {
      content: ''; position: absolute;
      top: 1px; left: 1px;
      width: 12px; height: 12px;
      background: var(--ink-3); border-radius: 50%;
      transition: transform 0.15s, background 0.15s;
    }
    .toggle.on { background: var(--accent); border-color: var(--accent); }
    .toggle.on::after { background: #1a0f00; transform: translateX(14px); }
    .events {
      font-family: var(--mono); font-size: 10px; color: var(--ink-3);
      flex: 1;
    }
    .empty {
      padding: 40px;
      text-align: center; color: var(--ink-3);
      font-size: 13px;
    }
  `;We([u()],ve.prototype,"renderTick",2);ve=We([h("nv-app-store")],ve);var _t=Object.defineProperty,St=Object.getOwnPropertyDescriptor,he=(e,t,s,r)=>{for(var a=r>1?void 0:r?St(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&_t(t,s,a),a};let R=class extends b{constructor(){super(...arguments),this.visible=!1,this.msg="",this.icon="✓",this.timer=null,this.onToast=e=>{const t=e.detail;this.msg=t.msg??"Done",this.icon=t.icon??"✓",this.visible=!0,this.setAttribute("visible",""),this.timer!==null&&window.clearTimeout(this.timer),this.timer=window.setTimeout(()=>{this.visible=!1,this.removeAttribute("visible")},1800)}}connectedCallback(){super.connectedCallback(),window.addEventListener("nv-toast",this.onToast)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("nv-toast",this.onToast)}render(){return c`<span class="icon">${this.icon}</span><span>${this.msg}</span>`}};R.styles=m`
    :host {
      position: fixed; bottom: 24px; left: 50%;
      transform: translateX(-50%) translateY(80px);
      background: var(--bg-2);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      padding: 10px 14px;
      font-size: 12.5px;
      box-shadow: var(--shadow);
      z-index: 100;
      opacity: 0; pointer-events: none;
      transition: opacity 0.2s, transform 0.2s;
      display: flex; align-items: center; gap: 8px;
    }
    :host([visible]) {
      opacity: 1;
      transform: translateX(-50%) translateY(0);
      pointer-events: auto;
    }
    .icon { color: var(--accent); }
  `;he([u()],R.prototype,"visible",2);he([u()],R.prototype,"msg",2);he([u()],R.prototype,"icon",2);R=he([h("nv-toast")],R);function W(e,t="✓"){window.dispatchEvent(new CustomEvent("nv-toast",{detail:{msg:e,icon:t}}))}var Ct=Object.defineProperty,Pt=Object.getOwnPropertyDescriptor,X=(e,t,s,r)=>{for(var a=r>1?void 0:r?Pt(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&Ct(t,s,a),a};let A=class extends b{constructor(){super(...arguments),this.open=!1,this.mTitle="",this.mBody="",this.buttons=[],this.onModal=e=>{const t=e.detail;this.mTitle=t.title,this.mBody=t.body,this.buttons=t.buttons??[{label:"Close",variant:"primary"}],this.open=!0,this.setAttribute("open","")},this.onKey=e=>{e.key==="Escape"&&this.open&&this.close()}}connectedCallback(){super.connectedCallback(),window.addEventListener("nv-modal",this.onModal),window.addEventListener("keydown",this.onKey)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("nv-modal",this.onModal),window.removeEventListener("keydown",this.onKey)}close(){this.open=!1,this.removeAttribute("open")}clickBtn(e){e.onClick?.(),this.close()}render(){return c`
      <div class="modal" role="dialog" aria-modal="true">
        <div class="h">
          <div class="ttl">${this.mTitle}</div>
          <button class="close" @click=${()=>this.close()}>×</button>
        </div>
        <div class="body" .innerHTML=${this.mBody}></div>
        <div class="f">
          ${this.buttons.map(e=>c`
            <button class=${e.variant??""} @click=${()=>this.clickBtn(e)}>${e.label}</button>
          `)}
        </div>
      </div>
    `}};A.styles=m`
    :host {
      position: fixed; inset: 0;
      background: rgba(0,0,0,0.55);
      backdrop-filter: blur(4px);
      z-index: 200;
      display: grid; place-items: center;
      opacity: 0; pointer-events: none;
      transition: opacity 0.18s;
    }
    :host([open]) { opacity: 1; pointer-events: auto; }
    .modal {
      background: var(--bg-1);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      box-shadow: 0 30px 80px -20px rgba(0,0,0,0.7);
      width: min(520px, 92vw);
      max-height: 86vh;
      display: flex; flex-direction: column;
      transform: translateY(12px) scale(0.98);
      transition: transform 0.22s cubic-bezier(0.2,0.7,0.3,1);
    }
    :host([open]) .modal { transform: translateY(0) scale(1); }
    .h {
      padding: 14px 16px;
      border-bottom: 1px solid var(--line);
      display: flex; align-items: center; justify-content: space-between;
    }
    .h .ttl { font-size: 14px; font-weight: 600; }
    .body { padding: 16px; overflow-y: auto; font-size: 13px; color: var(--ink-2); line-height: 1.55; }
    .f {
      padding: 12px 16px;
      border-top: 1px solid var(--line);
      display: flex; gap: 8px; justify-content: flex-end;
    }
    button {
      padding: 6px 12px;
      border-radius: 8px;
      font-size: 12.5px;
      cursor: pointer;
      font-family: inherit;
      border: 1px solid var(--line);
      background: var(--bg-2); color: var(--ink);
    }
    button.ghost { background: transparent; }
    button.primary { background: var(--accent); border-color: var(--accent); color: #1a0f00; }
    button.danger { background: var(--bad); border-color: var(--bad); color: #fff; }
    .close {
      width: 28px; height: 28px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px;
      color: var(--ink-2);
    }
  `;X([u()],A.prototype,"open",2);X([u()],A.prototype,"mTitle",2);X([u()],A.prototype,"mBody",2);X([u()],A.prototype,"buttons",2);A=X([h("nv-modal")],A);function ke(e){window.dispatchEvent(new CustomEvent("nv-modal",{detail:e}))}var Mt=Object.defineProperty,zt=Object.getOwnPropertyDescriptor,J=(e,t,s,r)=>{for(var a=r>1?void 0:r?zt(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&Mt(t,s,a),a};let E=class extends b{constructor(){super(...arguments),this.open=!1,this.filter="",this.idx=0,this.cmds=[{ico:"▶",label:"Run pipeline",kbd:"Space",run:async()=>{await P()?.run(),v.value=!0,W("Pipeline running","▶")}},{ico:"❚",label:"Pause pipeline",run:async()=>{await P()?.pause(),v.value=!1,W("Paused","❚❚")}},{ico:"⟳",label:"Reset pipeline",kbd:"⌘R",run:()=>ke({title:"Reset pipeline?",body:"<p>Clears the frame stream and rewinds <code>t</code> to 0.</p>",buttons:[{label:"Cancel",variant:"ghost"},{label:"Reset",variant:"danger",onClick:async()=>{await P()?.reset(),l("warn","pipeline reset · t=0"),W("Pipeline reset","⟳")}}]})},{ico:"✓",label:"Verify witness",run:async()=>{const e=P();if(!e)return;f.value="pending";const t=I.value,s=new Uint8Array(32);for(let a=0;a<32;a++)s[a]=parseInt(t.slice(a*2,a*2+2),16);(await e.verifyWitness(s)).ok?(f.value="ok",C.value=t,W("Witness verified","✓")):(f.value="fail",W("Witness mismatch!","✗"))}},{ico:"☼",label:"Toggle theme",kbd:"⌘/",run:()=>{g.value=g.value==="dark"?"light":"dark"}},{ico:"⚙",label:"Open settings",kbd:"⌘,",run:()=>window.dispatchEvent(new CustomEvent("open-settings"))},{ico:"?",label:"Keyboard shortcuts…",run:()=>ke({title:"Keyboard shortcuts",body:`<div style="display:grid;grid-template-columns:auto 1fr;gap:6px 16px;font-size:13px;">
        <div><code>⌘K / Ctrl K</code></div><div>Command palette</div>
        <div><code>Space</code></div><div>Play / pause</div>
        <div><code>⌘R</code></div><div>Reset</div>
        <div><code>⌘,</code></div><div>Settings</div>
        <div><code>⌘/</code></div><div>Toggle theme</div>
        <div><code>\`</code></div><div>Debug HUD</div>
        <div><code>1 · 2 · 3</code></div><div>Inspector tabs</div>
        <div><code>Esc</code></div><div>Close modal/palette</div>
        <div><code>/</code></div><div>Focus REPL</div>
      </div>`,buttons:[{label:"Close",variant:"primary"}]})},{ico:"i",label:"About nvsim…",run:()=>ke({title:"About nvsim",body:`<p><b>nvsim</b> is a deterministic, byte-reproducible forward simulator for nitrogen-vacancy diamond magnetometry.</p>
        <p>This dashboard runs nvsim as WASM in a Web Worker. Same <code>(scene, config, seed)</code> → byte-identical SHA-256 witness across runs and machines.</p>
        <p>License: MIT OR Apache-2.0 · See ADR-089, ADR-092.</p>`,buttons:[{label:"Close",variant:"primary"}]})}],this.onKey=e=>{(e.metaKey||e.ctrlKey)&&e.key.toLowerCase()==="k"?(e.preventDefault(),this.openPal()):e.key==="Escape"&&this.open?this.closePal():this.open&&(e.key==="ArrowDown"?(this.idx=Math.min(this.cmds.length-1,this.idx+1),e.preventDefault()):e.key==="ArrowUp"?(this.idx=Math.max(0,this.idx-1),e.preventDefault()):e.key==="Enter"&&(this.runIdx(),e.preventDefault()))},this.onOpen=()=>this.openPal()}connectedCallback(){super.connectedCallback(),window.addEventListener("keydown",this.onKey),window.addEventListener("nv-palette",this.onOpen)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("keydown",this.onKey),window.removeEventListener("nv-palette",this.onOpen)}openPal(){this.open=!0,this.setAttribute("open",""),this.filter="",this.idx=0,setTimeout(()=>this.inputEl?.focus(),0)}closePal(){this.open=!1,this.removeAttribute("open")}filtered(){if(!this.filter.trim())return this.cmds;const e=this.filter.toLowerCase();return this.cmds.filter(t=>t.label.toLowerCase().includes(e))}runIdx(){const t=this.filtered()[this.idx];t&&(t.run(),this.closePal())}render(){const e=this.filtered();return c`
      <div class="palette" data-id="palette">
        <div class="input">
          <input id="palette-input" type="text" placeholder="Type a command…"
            .value=${this.filter}
            @input=${t=>{this.filter=t.target.value,this.idx=0}} />
        </div>
        <div class="list">
          ${e.map((t,s)=>c`
            <div class="item ${s===this.idx?"active":""}" @click=${()=>{this.idx=s,this.runIdx()}}>
              <span class="ico">${t.ico}</span>
              <span class="lbl">${t.label}</span>
              ${t.kbd?c`<span class="kbd">${t.kbd}</span>`:""}
            </div>
          `)}
        </div>
      </div>
    `}};E.styles=m`
    :host {
      position: fixed; inset: 0; z-index: 220;
      background: rgba(0,0,0,0.5);
      opacity: 0; pointer-events: none;
      transition: opacity 0.15s;
      display: flex; justify-content: center; padding-top: 12vh;
      backdrop-filter: blur(4px);
    }
    :host([open]) { opacity: 1; pointer-events: auto; }
    .palette {
      width: min(560px, 92vw);
      background: var(--bg-1);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      box-shadow: 0 30px 80px -20px rgba(0,0,0,0.7);
      overflow: hidden;
      display: flex; flex-direction: column;
      max-height: 60vh;
    }
    .input {
      padding: 14px 16px;
      border-bottom: 1px solid var(--line);
    }
    input {
      width: 100%;
      background: transparent; border: none; outline: none;
      color: var(--ink); font-size: 14px;
      font-family: inherit;
    }
    .list { flex: 1; overflow-y: auto; padding: 4px; }
    .item {
      display: flex; align-items: center; gap: 10px;
      padding: 8px 12px;
      border-radius: 6px;
      cursor: pointer;
      font-size: 12.5px;
    }
    .item.active { background: var(--bg-3); }
    .item .ico { width: 20px; text-align: center; color: var(--accent); }
    .item .lbl { flex: 1; }
    .item .kbd {
      font-family: var(--mono); font-size: 10.5px;
      color: var(--ink-3);
      padding: 1px 5px; background: var(--bg-3); border-radius: 4px;
    }
  `;J([u()],E.prototype,"open",2);J([u()],E.prototype,"filter",2);J([u()],E.prototype,"idx",2);J([Ie("#palette-input")],E.prototype,"inputEl",2);E=J([h("nv-palette")],E);var At=Object.defineProperty,Et=Object.getOwnPropertyDescriptor,Pe=(e,t,s,r)=>{for(var a=r>1?void 0:r?Et(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&At(t,s,a),a};let G=class extends b{constructor(){super(...arguments),this.open=!1,this.renderFps=0,this.lastTs=performance.now(),this.frameCount=0,this.rafId=0,this.onKey=e=>{e.key==="`"&&!e.target.matches("input, textarea")&&(this.open=!this.open,this.toggleAttribute("open",this.open))},this.tick=()=>{this.rafId=requestAnimationFrame(this.tick);const e=performance.now();this.frameCount++,e-this.lastTs>=500&&(this.renderFps=this.frameCount*1e3/(e-this.lastTs),this.frameCount=0,this.lastTs=e,this.requestUpdate())}}connectedCallback(){super.connectedCallback(),window.addEventListener("keydown",this.onKey),x(()=>{k.value,$e.value,D.value,T.value,at.value,this.requestUpdate()}),this.tick()}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("keydown",this.onKey),cancelAnimationFrame(this.rafId)}render(){return c`
      <div class="h"><span>nvsim · debug</span><span class="x" @click=${()=>{this.open=!1,this.removeAttribute("open")}}>✕</span></div>
      <div class="row"><span class="k">render fps</span><span class="v">${this.renderFps.toFixed(1)}</span></div>
      <div class="row"><span class="k">sim fps</span><span class="v">${k.value>0?Math.round(k.value):"—"}</span></div>
      <div class="row"><span class="k">frames</span><span class="v">${$e.value.toString()}</span></div>
      <div class="row"><span class="k">|B|</span><span class="v">${(D.value*1e9).toFixed(3)} nT</span></div>
      <div class="row"><span class="k">SNR</span><span class="v">${T.value>0?T.value.toFixed(1):"—"}</span></div>
      <div class="row"><span class="k">DOM</span><span class="v">${document.querySelectorAll("*").length}</span></div>
    `}};G.styles=m`
    :host {
      position: fixed; bottom: 8px; right: 8px;
      width: 220px;
      background: rgba(13,17,23,0.85);
      backdrop-filter: blur(8px);
      border: 1px solid var(--line-2);
      border-radius: 8px;
      padding: 8px 10px;
      font-family: var(--mono); font-size: 11px;
      color: var(--ink-2);
      z-index: 99;
      display: none;
      box-shadow: var(--shadow);
    }
    :host([open]) { display: block; }
    .h {
      display: flex; justify-content: space-between;
      font-weight: 600; color: var(--ink);
      margin-bottom: 6px; padding-bottom: 4px;
      border-bottom: 1px solid var(--line);
    }
    .x { cursor: pointer; color: var(--ink-3); }
    .row {
      display: flex; justify-content: space-between;
      padding: 1px 0;
    }
    .k { color: var(--ink-3); }
    .v { color: var(--ink); }
  `;Pe([u()],G.prototype,"open",2);Pe([u()],G.prototype,"renderFps",2);G=Pe([h("nv-debug-hud")],G);var Ot=Object.defineProperty,Tt=Object.getOwnPropertyDescriptor,Ne=(e,t,s,r)=>{for(var a=r>1?void 0:r?Tt(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&Ot(t,s,a),a};let ge=class extends b{constructor(){super(...arguments),this.open=!1}connectedCallback(){super.connectedCallback(),x(()=>{g.value,w.value,$.value,ae.value,_.value,ye.value,this.requestUpdate()}),window.addEventListener("open-settings",()=>{this.open=!0,this.setAttribute("open","")})}close(){this.open=!1,this.removeAttribute("open")}render(){return c`
      <div class="scrim" @click=${()=>this.close()}></div>
      <div class="h">
        <div class="ttl">Settings</div>
        <button class="close" @click=${()=>this.close()}>×</button>
      </div>
      <div class="body">
        <div class="group">
          <h4>Appearance</h4>
          <div class="row">
            <div><div class="lbl">Theme</div></div>
            <div class="seg">
              <button class=${g.value==="dark"?"on":""} @click=${()=>g.value="dark"}>dark</button>
              <button class=${g.value==="light"?"on":""} @click=${()=>g.value="light"}>light</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Density</div>
              <div class="desc">Affects panel padding and font scale.</div>
            </div>
            <div class="seg">
              <button class=${w.value==="comfy"?"on":""} @click=${()=>w.value="comfy"}>comfy</button>
              <button class=${w.value==="default"?"on":""} @click=${()=>w.value="default"}>default</button>
              <button class=${w.value==="compact"?"on":""} @click=${()=>w.value="compact"}>compact</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Reduce motion</div>
              <div class="desc">Disable rotating crystal & field-line animation.</div>
            </div>
            <span class="toggle ${$.value?"on":""}"
              @click=${()=>$.value=!$.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Pipeline</h4>
          <div class="row">
            <div><div class="lbl">Auto-rerun on edit</div>
            <div class="desc">Restart pipeline when scene/config changes.</div></div>
            <span class="toggle ${ae.value?"on":""}"
              @click=${()=>ae.value=!ae.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Transport</h4>
          <div class="row">
            <div><div class="lbl">Mode</div></div>
            <div class="seg">
              <button class=${_.value==="wasm"?"on":""} @click=${()=>_.value="wasm"}>WASM</button>
              <button class=${_.value==="ws"?"on":""} @click=${()=>_.value="ws"}>WS</button>
            </div>
          </div>
          ${_.value==="ws"?c`
            <div class="row">
              <div><div class="lbl">WS URL</div></div>
              <input type="text" placeholder="ws://localhost:7878" .value=${ye.value}
                @input=${e=>ye.value=e.target.value} />
            </div>`:""}
        </div>
      </div>
    `}};ge.styles=m`
    :host {
      position: fixed; top: 0; right: 0; bottom: 0;
      width: 420px; max-width: 100vw;
      background: var(--bg-1);
      border-left: 1px solid var(--line);
      z-index: 51;
      transform: translateX(100%);
      transition: transform 0.25s cubic-bezier(0.4, 0, 0.2, 1);
      display: flex; flex-direction: column;
      box-shadow: -20px 0 60px -20px rgba(0,0,0,0.5);
    }
    :host([open]) { transform: translateX(0); }
    .scrim {
      position: fixed; inset: 0;
      background: rgba(0,0,0,0.5);
      z-index: 50;
      opacity: 0; pointer-events: none;
      transition: opacity 0.2s;
    }
    :host([open]) .scrim { opacity: 1; pointer-events: auto; }
    .h {
      padding: 14px 16px;
      border-bottom: 1px solid var(--line);
      display: flex; align-items: center; justify-content: space-between;
    }
    .h .ttl { font-size: 14px; font-weight: 600; }
    .body { flex: 1; overflow-y: auto; padding: 16px; }
    .group { margin-bottom: 22px; }
    .group h4 {
      margin: 0 0 10px;
      font-size: 11px; font-weight: 600;
      text-transform: uppercase; letter-spacing: 0.08em;
      color: var(--ink-3);
    }
    .row {
      display: flex; justify-content: space-between; align-items: center;
      padding: 10px 0;
      border-bottom: 1px solid var(--line);
    }
    .row:last-child { border-bottom: 0; }
    .row .lbl { font-size: 13px; }
    .row .desc { font-size: 11.5px; color: var(--ink-3); margin-top: 2px; }
    .row > div:first-child { flex: 1; padding-right: 12px; }
    .seg {
      display: inline-flex;
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: var(--radius-sm);
      padding: 2px;
    }
    .seg button {
      padding: 4px 10px;
      background: transparent; border: none;
      border-radius: 6px;
      font-size: 11.5px; color: var(--ink-3);
      font-family: var(--mono);
      cursor: pointer;
    }
    .seg button.on { background: var(--bg-1); color: var(--ink); }
    .toggle {
      position: relative;
      width: 36px; height: 20px;
      background: var(--bg-3);
      border: 1px solid var(--line-2);
      border-radius: 999px;
      cursor: pointer;
      flex-shrink: 0;
    }
    .toggle::after {
      content: ''; position: absolute;
      top: 2px; left: 2px;
      width: 14px; height: 14px;
      background: var(--ink-3);
      border-radius: 50%;
      transition: transform 0.15s, background 0.15s;
    }
    .toggle.on { background: var(--accent); border-color: var(--accent); }
    .toggle.on::after { background: #1a0f00; transform: translateX(16px); }
    .close {
      width: 28px; height: 28px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px;
      color: var(--ink-2);
    }
    input[type="text"] {
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 6px 10px;
      color: var(--ink); font-family: var(--mono); font-size: 12px;
      outline: none;
    }
  `;Ne([u()],ge.prototype,"open",2);ge=Ne([h("nv-settings-drawer")],ge);var Dt=Object.defineProperty,It=Object.getOwnPropertyDescriptor,Me=(e,t,s,r)=>{for(var a=r>1?void 0:r?It(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&Dt(t,s,a),a};const re=[{title:"Welcome to nvsim",body:`<p><b>nvsim</b> is an open-source, deterministic forward simulator for
      nitrogen-vacancy diamond magnetometry — a real Rust crate compiled to
      WASM and running in your browser, right now.</p>
      <p>This 30-second tour highlights the four panels you'll use most.</p>`,cta:"Start tour"},{title:"1. Scene canvas",body:`<p>The middle panel shows your <b>magnetic scene</b> — sources you can
      drag (rebar, heart proxy, mains hum, ferrous door) and a single NV-diamond
      sensor in the centre. Field lines from each source connect to the sensor
      and animate while the pipeline runs.</p>
      <p>Click <code>2</code> on your keyboard any time to jump to the Frame inspector.</p>`},{title:"2. Run the pipeline",body:`<p>Click the <b>▶ Run</b> button (top-right) to start streaming
      <code>MagFrame</code> records at the digitiser's sample rate. The B-vector
      trace and Frame stream sparkline update live, and the FPS pill in the
      topbar shows the simulator's throughput in kHz.</p>
      <p><kbd>Space</kbd> toggles run/pause from anywhere.</p>`},{title:"3. Witness panel",body:`<p>The <b>Witness</b> tab is the heart of nvsim's determinism contract.
      Click <b>Verify</b> and the pipeline re-derives the SHA-256 over a 256-frame
      reference run and asserts it matches the constant pinned in the Rust crate.</p>
      <p>Same input → same hash → byte-for-byte across browsers, OSes, transports.
      If the hash drifts, your build is non-canonical.</p>`},{title:"4. App Store",body:`<p>The grid icon on the left rail opens the <b>App Store</b> — every
      hot-loadable WASM edge module RuView ships, plus the simulators. 66 apps
      across 13 categories: medical, security, building, retail, industrial,
      signal, learning, autonomy, and more.</p>
      <p>Toggle any card to mark it active in this session; the WS transport
      will push the activation set to a connected ESP32 mesh.</p>`},{title:"You are ready",body:`<p>Press <kbd>⌘K</kbd> (or <kbd>Ctrl K</kbd>) any time for the command
      palette, <kbd>?</kbd> for the full shortcuts list, or just start clicking.</p>
      <p>Source on GitHub:
      <code>github.com/ruvnet/RuView</code> · ADR-089, ADR-092 · MIT/Apache-2.0.</p>`,cta:"Get started"}];let Y=class extends b{constructor(){super(...arguments),this.open=!1,this.step=0,this.show=()=>{this.step=0,this.open=!0,this.setAttribute("open","")}}async connectedCallback(){super.connectedCallback(),window.addEventListener("nv-show-tour",this.show),await V("onboarding-seen")||(this.open=!0,this.setAttribute("open",""))}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("nv-show-tour",this.show)}async dismiss(){this.open=!1,this.removeAttribute("open"),await K("onboarding-seen",!0)}next(){this.step<re.length-1?this.step++:this.dismiss()}prev(){this.step>0&&this.step--}render(){const e=re[this.step];return c`
      <div class="card" role="dialog" aria-modal="true" aria-label="Welcome tour">
        <div class="h">
          <h2>${e.title}</h2>
          <button class="skip" @click=${()=>this.dismiss()} aria-label="Skip tour">×</button>
        </div>
        <div class="body" .innerHTML=${e.body}></div>
        <div class="footer">
          <div class="dots">
            ${re.map((t,s)=>c`<div class="dot ${s===this.step?"active":""}"></div>`)}
          </div>
          ${this.step>0?c`<button class="ghost" @click=${()=>this.prev()}>Back</button>`:""}
          <button class="primary" @click=${()=>this.next()}>
            ${this.step===re.length-1?e.cta??"Done":e.cta??"Next"}
          </button>
        </div>
      </div>
    `}};Y.styles=m`
    :host {
      position: fixed; inset: 0;
      background: rgba(0, 0, 0, 0.55);
      backdrop-filter: blur(4px);
      z-index: 240;
      display: grid; place-items: center;
      opacity: 0; pointer-events: none;
      transition: opacity 0.18s;
    }
    :host([open]) { opacity: 1; pointer-events: auto; }
    .card {
      background: var(--bg-1);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      box-shadow: 0 30px 80px -20px rgba(0,0,0,0.7);
      width: min(560px, 92vw);
      max-height: 86vh;
      display: flex; flex-direction: column;
      transform: translateY(12px) scale(0.98);
      transition: transform 0.22s cubic-bezier(0.2,0.7,0.3,1);
    }
    :host([open]) .card { transform: translateY(0) scale(1); }
    .h {
      padding: 20px 22px 8px;
      display: flex; justify-content: space-between; align-items: flex-start;
    }
    .h h2 { margin: 0; font-size: 18px; letter-spacing: -0.01em; }
    .body {
      padding: 8px 22px 16px;
      font-size: 13px; color: var(--ink-2); line-height: 1.55;
      overflow-y: auto;
    }
    .body p { margin: 0 0 12px; }
    .body code, .body kbd {
      font-family: var(--mono); font-size: 11.5px;
      padding: 1px 5px; background: var(--bg-3);
      border: 1px solid var(--line); border-radius: 4px;
      color: var(--accent);
    }
    .footer {
      display: flex; align-items: center; gap: 12px;
      padding: 12px 22px;
      border-top: 1px solid var(--line);
    }
    .dots { display: flex; gap: 6px; flex: 1; }
    .dot {
      width: 6px; height: 6px; border-radius: 50%;
      background: var(--bg-3); border: 1px solid var(--line-2);
    }
    .dot.active { background: var(--accent); border-color: var(--accent); }
    button {
      padding: 8px 14px;
      border-radius: 8px;
      font-size: 12.5px; font-weight: 500;
      border: 1px solid var(--line);
      background: var(--bg-2); color: var(--ink);
      cursor: pointer;
      font-family: inherit;
    }
    button.primary {
      background: var(--accent); border-color: var(--accent);
      color: #1a0f00;
    }
    button.ghost { background: transparent; }
    .skip {
      width: 28px; height: 28px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px; color: var(--ink-2);
    }
  `;Me([u()],Y.prototype,"open",2);Me([u()],Y.prototype,"step",2);Y=Me([h("nv-onboarding")],Y);var Lt=Object.defineProperty,Ft=Object.getOwnPropertyDescriptor,Be=(e,t,s,r)=>{for(var a=r>1?void 0:r?Ft(t,s):t,i=e.length-1,n;i>=0;i--)(n=e[i])&&(a=(r?n(t,s,a):n(a))||a);return r&&a&&Lt(t,s,a),a};let me=class extends b{constructor(){super(...arguments),this.view="scene"}render(){return c`
      <div class="app">
        <nv-rail .view=${this.view} @navigate=${e=>this.view=e.detail}></nv-rail>
        <nv-topbar></nv-topbar>
        <nv-sidebar></nv-sidebar>
        <div class="main">
          ${this.view==="apps"?c`<nv-app-store></nv-app-store>`:c`<nv-scene></nv-scene>`}
        </div>
        <nv-inspector></nv-inspector>
        <nv-console></nv-console>
      </div>
      <nv-toast></nv-toast>
      <nv-modal></nv-modal>
      <nv-palette></nv-palette>
      <nv-debug-hud></nv-debug-hud>
      <nv-settings-drawer></nv-settings-drawer>
      <nv-onboarding></nv-onboarding>
    `}};me.styles=m`
    :host {
      display: block;
      height: 100vh;
      width: 100vw;
      background: var(--bg-0);
    }
    .app {
      display: grid;
      grid-template-columns: 56px 280px 1fr 340px;
      grid-template-rows: 48px 1fr 220px;
      grid-template-areas:
        'rail topbar topbar topbar'
        'rail sidebar main inspector'
        'rail sidebar console inspector';
      height: 100vh;
      width: 100vw;
    }
    nv-rail { grid-area: rail; }
    nv-topbar { grid-area: topbar; }
    nv-sidebar { grid-area: sidebar; }
    .main { grid-area: main; min-width: 0; min-height: 0; position: relative; overflow: hidden; }
    nv-inspector { grid-area: inspector; }
    nv-console { grid-area: console; min-height: 0; }
    @media (max-width: 1180px) {
      .app {
        grid-template-columns: 56px 1fr 320px;
        grid-template-areas:
          'rail topbar topbar'
          'rail main inspector'
          'rail console console';
      }
      nv-sidebar { display: none; }
    }
    @media (max-width: 860px) {
      .app {
        grid-template-columns: 1fr;
        grid-template-rows: 52px 1fr 200px;
        grid-template-areas:
          'topbar'
          'main'
          'console';
      }
      nv-rail, nv-sidebar, nv-inspector { display: none; }
    }
  `;Be([u()],me.prototype,"view",2);me=Be([h("nv-app")],me);function Rt(e,t,s){const r=e.getUint32(t+0,!0),a=e.getUint16(t+4,!0),i=e.getUint16(t+6,!0),n=e.getUint16(t+8,!0),d=e.getBigUint64(t+12,!0),p=e.getFloat32(t+20,!0),O=e.getFloat32(t+24,!0),fe=e.getFloat32(t+28,!0),xe=e.getFloat32(t+32,!0),Ue=e.getFloat32(t+36,!0),qe=e.getFloat32(t+40,!0),Ve=e.getFloat32(t+44,!0),Ke=e.getFloat32(t+48,!0);return{magic:r,version:a,flags:i,sensorId:n,tUs:d,bPt:[p,O,fe],sigmaPt:[xe,Ue,qe],noiseFloorPtSqrtHz:Ve,temperatureK:Ke,raw:s.subarray(t,t+60)}}function jt(e){const s=new DataView(e.buffer,e.byteOffset,e.byteLength),r=[];for(let a=0;a+60<=e.byteLength;a+=60)r.push(Rt(s,a,e));return r}class Ht{constructor(){this.nextId=1,this.pending=new Map,this.frameSubs=new Set,this.eventSubs=new Set,this.bootInfo=null,this.worker=new Worker(new URL("/RuView/nvsim/assets/worker-BW0kLvYS.js",import.meta.url),{type:"module"}),this.worker.addEventListener("message",t=>this.onMessage(t)),this.worker.addEventListener("error",t=>this.eventSubs.forEach(s=>s({type:"log",level:"err",msg:String(t.message)})))}onMessage(t){const s=t.data;if(s.type==="frames"){const r=s.batch,a=new Uint8Array(r),n={frames:jt(a),bytes:a};this.frameSubs.forEach(p=>p(n));const d=s.fps;d>0&&this.eventSubs.forEach(p=>p({type:"fps",value:d}));return}if(s.type==="state"){this.eventSubs.forEach(r=>r({type:"state",running:!!s.running,t:0,framesEmitted:Number(s.framesEmitted??0)}));return}if(s.type!=="ready"){if(s.type==="err"&&s.id==null){this.eventSubs.forEach(r=>r({type:"log",level:"err",msg:String(s.msg)}));return}if(typeof s.id=="number"&&this.pending.has(s.id)){const r=this.pending.get(s.id);this.pending.delete(s.id),s.type==="err"?r.reject(new Error(String(s.msg))):r.resolve(s)}}}rpc(t,s=[]){const r=this.nextId++;return new Promise((a,i)=>{this.pending.set(r,{resolve:a,reject:i}),this.worker.postMessage({...t,id:r},s)})}async boot(){if(this.bootInfo)return this.bootInfo;const s=await this.rpc({type:"boot",base:"/RuView/nvsim/"});return this.bootInfo={buildVersion:s.buildVersion,frameMagic:s.frameMagic,frameBytes:s.frameBytes,expectedWitnessHex:s.expectedWitnessHex},this.bootInfo}async loadScene(t){await this.rpc({type:"setScene",json:JSON.stringify(t)})}async setConfig(t){await this.rpc({type:"setConfig",json:JSON.stringify(t)})}async setSeed(t){await this.rpc({type:"setSeed",seed:Number(t&0xFFFFFFFFn)})}async reset(){await this.rpc({type:"reset"})}async run(t){await this.rpc({type:"run"})}async pause(){await this.rpc({type:"pause"})}async step(t,s){await this.rpc({type:"step"})}onFrames(t){this.frameSubs.add(t)}onEvent(t){this.eventSubs.add(t)}async generateWitness(t){const s=await this.rpc({type:"witnessGenerate",samples:t});return new Uint8Array(s.witness)}async verifyWitness(t){const s=t.slice().buffer,r=await this.rpc({type:"witnessVerify",samples:256,expected:s},[s]);return r.ok?{ok:!0}:{ok:!1,actual:new Uint8Array(r.actual)}}async exportProofBundle(){const t=await this.generateWitness(256),s=Array.from(t).map(i=>i.toString(16).padStart(2,"0")).join(""),r=this.bootInfo??await this.boot(),a=JSON.stringify({kind:"nvsim-proof-bundle",version:r.buildVersion,seed:"0x0000002A",nSamples:256,witness:s,expected:r.expectedWitnessHex,ok:s===r.expectedWitnessHex,ts:new Date().toISOString()},null,2);return new Blob([a],{type:"application/json"})}async buildId(){return(await this.rpc({type:"buildId"})).buildId}async close(){this.worker.terminate()}}function Ee(e){document.documentElement.setAttribute("data-theme",e)}function Oe(e){document.body.classList.remove("density-comfy","density-default","density-compact"),document.body.classList.add(`density-${e}`)}function Te(e){document.body.classList.toggle("reduce-motion",e)}(async()=>{const e=await V("theme")??"dark",t=await V("density")??"default",s=await V("motionReduced")??!1;g.value=e,Ee(e),w.value=t,Oe(t),$.value=s,Te(s),x(()=>{Ee(g.value),K("theme",g.value)}),x(()=>{Oe(w.value),K("density",w.value)}),x(()=>{Te($.value),K("motionReduced",$.value)});const r=new Ht;rt(r),l("info","nvsim — booting WASM runtime"),r.onEvent(a=>{a.type==="log"&&l(a.level,a.msg),a.type==="fps"&&(k.value=a.value),a.type==="state"&&($e.value=BigInt(a.framesEmitted))}),r.onFrames(a=>{if(a.frames.length===0)return;const i=a.frames[a.frames.length-1];_e.value=i;const n=i.bPt[0]*1e-12,d=i.bPt[1]*1e-12,p=i.bPt[2]*1e-12;ne.value=[n,d,p],D.value=Math.sqrt(n*n+d*d+p*p),nt([n*1e9,d*1e9,p*1e9]);const O=Math.min(1,Math.abs(p*1e9)/5+.3);ot(O)});try{const a=await r.boot();I.value=a.expectedWitnessHex,l("ok",`WASM module ready · nvsim@${a.buildVersion} · magic=0x${a.frameMagic.toString(16).toUpperCase()}`),l("info",`expected witness · ${a.expectedWitnessHex.slice(0,16)}…`),st.value="(reference scene)",_.value="wasm"}catch(a){l("err",`boot failed: ${a.message}`)}try{const a=I.value;if(a){const i=new Uint8Array(32);for(let d=0;d<32;d++)i[d]=parseInt(a.slice(d*2,d*2+2),16);const n=await r.verifyWitness(i);if(n.ok)C.value=a,l("ok","witness verified · determinism gate ✓");else{const d=Array.from(n.actual).map(p=>p.toString(16).padStart(2,"0")).join("");C.value=d,l("err",`WITNESS MISMATCH · expected ${a.slice(0,16)}… got ${d.slice(0,16)}…`)}}}catch(a){l("warn",`witness verify skipped: ${a.message}`)}})();
//# sourceMappingURL=index-C2Z9YIJa.js.map
