import{f as tt,u as at,i as y,a as w,b as p,w as G}from"./lit-BS7WqYd5.js";import{y as c,g as st,j as f}from"./signals-SG45zFCj.js";(function(){const t=document.createElement("link").relList;if(t&&t.supports&&t.supports("modulepreload"))return;for(const s of document.querySelectorAll('link[rel="modulepreload"]'))r(s);new MutationObserver(s=>{for(const i of s)if(i.type==="childList")for(const o of i.addedNodes)o.tagName==="LINK"&&o.rel==="modulepreload"&&r(o)}).observe(document,{childList:!0,subtree:!0});function a(s){const i={};return s.integrity&&(i.integrity=s.integrity),s.referrerPolicy&&(i.referrerPolicy=s.referrerPolicy),s.crossOrigin==="use-credentials"?i.credentials="include":s.crossOrigin==="anonymous"?i.credentials="omit":i.credentials="same-origin",i}function r(s){if(s.ep)return;s.ep=!0;const i=a(s);fetch(s.href,i)}})();/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const k=e=>(t,a)=>{a!==void 0?a.addInitializer(()=>{customElements.define(e,t)}):customElements.define(e,t)};/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const rt={attribute:!0,type:String,converter:at,reflect:!1,hasChanged:tt},it=(e=rt,t,a)=>{const{kind:r,metadata:s}=a;let i=globalThis.litPropertyMetadata.get(s);if(i===void 0&&globalThis.litPropertyMetadata.set(s,i=new Map),r==="setter"&&((e=Object.create(e)).wrapped=!0),i.set(a.name,e),r==="accessor"){const{name:o}=a;return{set(n){const l=t.get.call(this);t.set.call(this,n),this.requestUpdate(o,l,e,!0,n)},init(n){return n!==void 0&&this.C(o,void 0,e,n),n}}}if(r==="setter"){const{name:o}=a;return function(n){const l=this[o];t.call(this,n),this.requestUpdate(o,l,e,!0,n)}}throw Error("Unsupported decorator location: "+r)};function Fe(e){return(t,a)=>typeof a=="object"?it(e,t,a):((r,s,i)=>{const o=s.hasOwnProperty(i);return s.constructor.createProperty(i,r),o?Object.getOwnPropertyDescriptor(s,i):void 0})(e,t,a)}/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */function u(e){return Fe({...e,state:!0,attribute:!1})}/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const ot=(e,t,a)=>(a.configurable=!0,a.enumerable=!0,Reflect.decorate&&typeof t!="object"&&Object.defineProperty(e,t,a),a);/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */function We(e,t){return(a,r,s)=>{const i=o=>o.renderRoot?.querySelector(e)??null;return ot(a,r,{get(){return i(this)}})}}var nt=Object.defineProperty,lt=Object.getOwnPropertyDescriptor,Ue=(e,t,a,r)=>{for(var s=r>1?void 0:r?lt(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&nt(t,a,s),s};let he=class extends w{constructor(){super(...arguments),this.view="scene"}navigate(e){this.dispatchEvent(new CustomEvent("navigate",{detail:e}))}render(){return p`
      <div class="logo" aria-hidden="true">NV</div>
      <nav role="navigation" aria-label="Primary"
        style="display:flex; flex-direction:column; align-items:center; gap:4px; flex:1;">
      <button class="btn ${this.view==="scene"?"active":""}"
        data-id="scene-btn" title="Scene" aria-label="Scene"
        aria-current=${this.view==="scene"?"page":"false"}
        @click=${()=>this.navigate("scene")}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2L3 7l9 5 9-5-9-5zm0 13l-9-5v6l9 5 9-5v-6l-9 5z"/></svg>
      </button>
      <button class="btn ${this.view==="apps"?"active":""}"
        data-id="apps-btn" title="App Store" aria-label="App Store"
        aria-current=${this.view==="apps"?"page":"false"}
        @click=${()=>this.navigate("apps")}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
      </button>
      <button class="btn ${this.view==="inspector"?"active":""}"
        data-id="inspector-btn" title="Inspector" aria-label="Inspector"
        aria-current=${this.view==="inspector"?"page":"false"}
        @click=${()=>this.navigate("inspector")}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7"/><line x1="21" y1="21" x2="16.6" y2="16.6"/></svg>
      </button>
      <button class="btn ${this.view==="witness"?"active":""}"
        data-id="witness-btn" title="Witness" aria-label="Witness"
        aria-current=${this.view==="witness"?"page":"false"}
        @click=${()=>this.navigate("witness")}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 12l2 2 4-4M21 12c0 4.97-4.03 9-9 9s-9-4.03-9-9 4.03-9 9-9 9 4.03 9 9z"/></svg>
      </button>
      <button class="btn ghost ${this.view==="ghost-murmur"?"active":""}"
        data-id="ghost-murmur-btn" title="Ghost Murmur — research spec"
        aria-label="Ghost Murmur research"
        aria-current=${this.view==="ghost-murmur"?"page":"false"}
        @click=${()=>this.navigate("ghost-murmur")}>
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9 2C5.7 2 3 4.7 3 8v12l3-2 3 2 3-2 3 2 3-2 3 2V8c0-3.3-2.7-6-6-6H9z"/>
          <circle cx="9" cy="10" r="1.2" fill="currentColor"/>
          <circle cx="15" cy="10" r="1.2" fill="currentColor"/>
        </svg>
      </button>
      </nav>
      <div class="spacer"></div>
      <button class="btn" data-id="settings-btn" title="Settings" aria-label="Settings"
        @click=${()=>this.dispatchEvent(new CustomEvent("open-settings",{bubbles:!0,composed:!0}))}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06A1.65 1.65 0 0015 19.4a1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.6 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.6a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09A1.65 1.65 0 0015 4.6a1.65 1.65 0 001.82-.33l.06.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
      </button>
    `}};he.styles=y`
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
    .btn.ghost.active::before { background: var(--accent-3); }
    .spacer { flex: 1; }
    svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 1.8; }
  `;Ue([Fe()],he.prototype,"view",2);he=Ue([k("nv-rail")],he);const P=c("wasm"),Pe=c("");c(!1);c(null);const g=c(!1);c(!0);const ue=c(1),dt=c(0),Re=c(0n),L=c(0xCAFEBABEn),j=c(1e4),W=c(1e3),U=c(1),q=c(!0),x=c("dark"),_=c("default"),z=c(!1),ve=c(!0),fe=c([0,0,0]),K=c(0),H=c(0),M=c(0),A=c(""),$=c("idle"),Y=c(""),oe=c(null),xe=c([]),ye=c([]),we=c([]),ke=c([]),He=c("rebar-walkby-01"),ct=c(""),ae=c(!1),Te=c("all"),N=c([]);function pt(e){const t=N.value.slice();for(t.push(e);t.length>200;)t.shift();N.value=t}const ie=c([]),Ne=st(()=>P.value==="wasm"?"wasm":"ws");let qe=null;function ut(e){qe=e}function h(){return qe}const T=c([]),vt=200;function d(e,t){if(ae.value)return;const a=T.value.slice();for(a.push({ts:Date.now(),level:e,msg:t});a.length>vt;)a.shift();T.value=a}function gt(e){const a=xe.value.slice();a.push(e[0]),a.length>200&&a.shift();const r=ye.value.slice();r.push(e[1]),r.length>200&&r.shift();const s=we.value.slice();s.push(e[2]),s.length>200&&s.shift(),xe.value=a,ye.value=r,we.value=s}function mt(e){const a=ke.value.slice();for(a.push(Math.max(0,Math.min(1,e)));a.length>48;)a.shift();ke.value=a}var bt=Object.defineProperty,ht=Object.getOwnPropertyDescriptor,ce=(e,t,a,r)=>{for(var s=r>1?void 0:r?ht(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&bt(t,a,s),s};let B=class extends w{constructor(){super(...arguments),this.open=!1,this.mTitle="",this.mBody="",this.buttons=[],this.onModal=e=>{const t=e.detail;this.mTitle=t.title,this.mBody=t.body,this.buttons=t.buttons??[{label:"Close",variant:"primary"}],this.open=!0,this.setAttribute("open",""),requestAnimationFrame(()=>{const a=this.shadowRoot;if(!a)return;a.querySelector("input, select, textarea, button:not(.close)")?.focus()})},this.onKey=e=>{e.key==="Escape"&&this.open&&this.close()}}connectedCallback(){super.connectedCallback(),window.addEventListener("nv-modal",this.onModal),window.addEventListener("keydown",this.onKey)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("nv-modal",this.onModal),window.removeEventListener("keydown",this.onKey)}updated(){if(!this.open)return;const e=this.shadowRoot;if(!e)return;const t=a=>{if(a.key!=="Tab")return;const r=Array.from(e.querySelectorAll("input, select, textarea, button, [href]")).filter(n=>!n.hasAttribute("disabled"));if(r.length===0)return;const s=r[0],i=r[r.length-1],o=e.activeElement??null;a.shiftKey&&o===s?(a.preventDefault(),i.focus()):!a.shiftKey&&o===i&&(a.preventDefault(),s.focus())};e.removeEventListener("keydown",t),e.addEventListener("keydown",t)}close(){this.open=!1,this.removeAttribute("open")}clickBtn(e){e.onClick?.(),this.close()}render(){return p`
      <div class="modal" role="dialog" aria-modal="true">
        <div class="h">
          <div class="ttl">${this.mTitle}</div>
          <button class="close" @click=${()=>this.close()}>×</button>
        </div>
        <div class="body" .innerHTML=${this.mBody}></div>
        <div class="f">
          ${this.buttons.map(e=>p`
            <button class=${e.variant??""} @click=${()=>this.clickBtn(e)}>${e.label}</button>
          `)}
        </div>
      </div>
    `}};B.styles=y`
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
  `;ce([u()],B.prototype,"open",2);ce([u()],B.prototype,"mTitle",2);ce([u()],B.prototype,"mBody",2);ce([u()],B.prototype,"buttons",2);B=ce([k("nv-modal")],B);function se(e){window.dispatchEvent(new CustomEvent("nv-modal",{detail:e}))}var ft=Object.defineProperty,xt=Object.getOwnPropertyDescriptor,ze=(e,t,a,r)=>{for(var s=r>1?void 0:r?xt(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&ft(t,a,s),s};let X=class extends w{constructor(){super(...arguments),this.visible=!1,this.msg="",this.icon="✓",this.timer=null,this.onToast=e=>{const t=e.detail;this.msg=t.msg??"Done",this.icon=t.icon??"✓",this.visible=!0,this.setAttribute("visible",""),this.timer!==null&&window.clearTimeout(this.timer),this.timer=window.setTimeout(()=>{this.visible=!1,this.removeAttribute("visible")},1800)}}connectedCallback(){super.connectedCallback(),window.addEventListener("nv-toast",this.onToast)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("nv-toast",this.onToast)}render(){return p`<span class="icon">${this.icon}</span><span>${this.msg}</span>`}};X.styles=y`
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
  `;ze([u()],X.prototype,"visible",2);ze([u()],X.prototype,"msg",2);ze([u()],X.prototype,"icon",2);X=ze([k("nv-toast")],X);function C(e,t="✓"){window.dispatchEvent(new CustomEvent("nv-toast",{detail:{msg:e,icon:t}}))}var yt=Object.getOwnPropertyDescriptor,wt=(e,t,a,r)=>{for(var s=r>1?void 0:r?yt(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=o(s)||s);return s};let De=class extends w{connectedCallback(){super.connectedCallback(),f(()=>{M.value,Ne.value,L.value,x.value,He.value,g.value,this.requestUpdate()})}async toggleRun(){const e=h();e&&(g.value?(await e.pause(),g.value=!1):(await e.run(),g.value=!0))}async reset(){const e=h();e&&await e.reset()}toggleTheme(){x.value=x.value==="dark"?"light":"dark"}async openSeedModal(){const e=`0x${L.value.toString(16).toUpperCase().padStart(8,"0")}`;se({title:"Set seed",body:`<p>Set the 32-bit hex seed for the shot-noise PRNG. Same <code>(scene, config, seed)</code> → byte-identical witness.</p>
        <label>Hex seed</label>
        <input type="text" id="seed-input" value="${e}" autofocus />`,buttons:[{label:"Cancel",variant:"ghost"},{label:"Apply",variant:"primary",onClick:async()=>{const t=document.querySelector("nv-modal")?.shadowRoot?.querySelector("#seed-input");if(!t)return;const a=t.value.trim().replace(/^0x/i,""),r=BigInt("0x"+a);L.value=r,await h()?.setSeed(r),d("ok",`seed → 0x${r.toString(16).toUpperCase()}`),C(`Seed → 0x${r.toString(16).toUpperCase().slice(0,8)}`,"⟳")}}]})}openTransportSettings(){window.dispatchEvent(new CustomEvent("open-settings"))}render(){const e=L.value.toString(16).toUpperCase().padStart(8,"0");return p`
      <div class="crumbs">
        <span class="home">RuView</span><span class="sep">/</span>
        <span>nvsim</span><span class="sep">/</span>
        <span class="cur" id="scene-name">${He.value}</span>
      </div>
      <div class="spacer"></div>
      <span class="pill" id="fps-pill">
        <span class="dot"></span>
        <span id="fps-val">${M.value>0?(M.value/1e3).toFixed(2)+" kHz":"idle"}</span>
      </span>
      <span class="pill wasm" id="transport-pill" title="Transport settings"
        @click=${this.openTransportSettings}>
        <span class="dot"></span>${Ne.value}
      </span>
      <span class="pill seed" id="seed-pill" title="Set seed"
        @click=${this.openSeedModal}>
        seed: <b>0x${e}</b>
      </span>
      <button class="ghost" id="theme-btn" title="Toggle theme" @click=${this.toggleTheme}>
        ${x.value==="dark"?"☼":"☾"}
      </button>
      <button id="reset-btn" @click=${this.reset}>↺ Reset</button>
      <button class="primary" id="run-btn" @click=${this.toggleRun}>
        ${g.value?"❚❚ Pause":"▶ Run"}
      </button>
    `}};De.styles=y`
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
    .pill.seed { color: var(--ink-3); cursor: pointer; }
    .pill.seed:hover { border-color: var(--line-2); }
    .pill.seed b { color: var(--accent); font-weight: 600; }
    .pill.wasm { cursor: pointer; }
    .pill.wasm:hover { border-color: var(--line-2); }
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
  `;De=wt([k("nv-topbar")],De);var kt=Object.getOwnPropertyDescriptor,$t=(e,t,a,r)=>{for(var s=r>1?void 0:r?kt(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=o(s)||s);return s};let Ae=null;function ge(){Ae!==null&&window.clearTimeout(Ae),Ae=window.setTimeout(async()=>{const e=h();if(e)try{await e.setConfig({digitiser:{f_s_hz:j.value,f_mod_hz:W.value},sensor:{gamma_fwhm_hz:1e6,t1_s:.005,t2_s:1e-6,t2_star_s:2e-7,contrast:.03,n_spins:1e12,shot_noise_disabled:!q.value},dt_s:U.value*.001}),d("dbg",`config pushed · fs=${j.value} f_mod=${W.value} dt=${U.value.toFixed(1)}ms noise=${q.value?"on":"off"}`)}catch(t){d("warn",`config push failed: ${t.message}`)}},300)}let Ee=class extends w{connectedCallback(){super.connectedCallback(),f(()=>{j.value,W.value,U.value,q.value,g.value,this.requestUpdate()})}render(){return p`
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
          <div class="top"><span class="lbl">Sample rate</span><span class="val">${(j.value/1e3).toFixed(1)} kHz</span></div>
          <input type="range" min="1000" max="100000" .value=${String(j.value)}
            @input=${e=>{j.value=+e.target.value,ge()}} />
        </div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Lockin f_mod</span><span class="val">${(W.value/1e3).toFixed(3)} kHz</span></div>
          <input type="range" min="100" max="5000" .value=${String(W.value)}
            @input=${e=>{W.value=+e.target.value,ge()}} />
        </div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Integration t</span><span class="val">${U.value.toFixed(1)} ms</span></div>
          <input type="range" min="0.1" max="10" step="0.1" .value=${String(U.value)}
            @input=${e=>{U.value=+e.target.value,ge()}} />
        </div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Shot noise</span><span class="val">${q.value?"ON":"OFF"}</span></div>
          <input type="range" min="0" max="1" .value=${q.value?"1":"0"}
            @input=${e=>{q.value=e.target.value==="1",ge()}} />
        </div>
      </div>

      <div class="panel">
        <div class="panel-h">Pipeline</div>
        <div class="pipeline">
          <span class="stage ${g.value?"live":""}">scene</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${g.value?"live":""}">B-S</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${g.value?"live":""}">prop</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${g.value?"live":""}">NV</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${g.value?"live":""}">ADC</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${g.value?"live":""}">frame</span>
        </div>
      </div>
    `}};Ee.styles=y`
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
  `;Ee=$t([k("nv-sidebar")],Ee);var St=Object.defineProperty,_t=Object.getOwnPropertyDescriptor,Q=(e,t,a,r)=>{for(var s=r>1?void 0:r?_t(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&St(t,a,s),s};let R=class extends w{constructor(){super(...arguments),this.zoom=1,this.layerVisible={source:!0,field:!0,label:!0},this.items=[{id:"rebar",x:740,y:240,color:"oklch(0.72 0.18 330)",name:"rebar.steel"},{id:"heart",x:220,y:180,color:"oklch(0.78 0.14 195)",name:"heart_proxy"},{id:"mains",x:180,y:380,color:"oklch(0.72 0.18 330)",name:"mains_60Hz"},{id:"door",x:800,y:470,color:"oklch(0.78 0.14 145)",name:"door.steel"}],this.dragging=null,this.selected=null,this.dragOffset={dx:0,dy:0},this.onDown=(e,t)=>{t.preventDefault(),this.dragging=e,this.selected=e;const a=this.items.find(i=>i.id===e);if(!a)return;const r=this.renderRoot.querySelector("svg");if(!r)return;const s=this.toSvg(t,r);this.dragOffset={dx:s.x-a.x,dy:s.y-a.y}},this.onPointerMove=e=>{if(!this.dragging)return;const t=this.renderRoot.querySelector("svg");if(!t)return;const a=this.toSvg(e,t);this.items=this.items.map(r=>r.id===this.dragging?{...r,x:a.x-this.dragOffset.dx,y:a.y-this.dragOffset.dy}:r)},this.onPointerUp=()=>{this.dragging&&(ie.value=this.items.map(({id:e,x:t,y:a})=>({id:e,x:t,y:a}))),this.dragging=null}}connectedCallback(){super.connectedCallback(),ie.value.length>0&&(this.items=this.items.map(e=>{const t=ie.value.find(a=>a.id===e.id);return t?{...e,x:t.x,y:t.y}:e})),f(()=>{fe.value,K.value,M.value,H.value,z.value,g.value,ue.value,oe.value,this.requestUpdate()}),f(()=>{const e=oe.value;if(!e)return;const t=Math.sqrt(e.bPt[0]**2+e.bPt[1]**2+e.bPt[2]**2),a=Math.max(Math.abs(e.sigmaPt[0]),Math.abs(e.sigmaPt[1]),Math.abs(e.sigmaPt[2]),.001),r=t/a;Number.isFinite(r)&&(H.value=r)}),window.addEventListener("pointermove",this.onPointerMove),window.addEventListener("pointerup",this.onPointerUp)}async toggleRun(){const e=h();e&&(g.value?(await e.pause(),g.value=!1):(await e.run(),g.value=!0))}async stepFwd(){const e=h();e&&(await e.step("fwd",10),d("dbg","sim step → +1 frame"))}async stepBack(){const e=h();e&&(await e.step("back",10),d("dbg","sim step ← -1 frame"))}cycleSpeed(){const e=[.25,.5,1,2,4],t=e.indexOf(ue.value);ue.value=e[(t+1)%e.length]}zoomIn(){this.zoom=Math.min(2.5,this.zoom*1.2)}zoomOut(){this.zoom=Math.max(.5,this.zoom/1.2)}fitView(){this.zoom=1}toggleLayer(e){this.layerVisible={...this.layerVisible,[e]:!this.layerVisible[e]}}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("pointermove",this.onPointerMove),window.removeEventListener("pointerup",this.onPointerUp)}toSvg(e,t){const a=t.getBoundingClientRect(),r=(e.clientX-a.left)/a.width*1e3,s=(e.clientY-a.top)/a.height*600;return{x:r,y:s}}render(){const e=fe.value,t=[e[0]*1e9,e[1]*1e9,e[2]*1e9],a=K.value*1e9,r=z.value?"":"anim",s=1e3/this.zoom,i=600/this.zoom,o=(1e3-s)/2,n=(600-i)/2;return p`
      <div class="grid"></div>
      <svg viewBox="${o.toFixed(1)} ${n.toFixed(1)} ${s.toFixed(1)} ${i.toFixed(1)}"
        preserveAspectRatio="xMidYMid meet" id="scene-svg">
        <defs>
          <radialGradient id="g-sensor" cx="50%" cy="50%" r="50%">
            <stop offset="0" stop-color="oklch(0.78 0.14 70)" stop-opacity="0.4"/>
            <stop offset="1" stop-color="oklch(0.78 0.14 70)" stop-opacity="0"/>
          </radialGradient>
          <filter id="glow"><feGaussianBlur stdDeviation="3" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
        </defs>

        <!-- Field lines from each source to sensor -->
        ${this.layerVisible.field?this.items.map(l=>G`
          <line class="field-line ${r}" x1=${l.x} y1=${l.y}
            x2="500" y2="320"
            stroke=${l.color} stroke-width="1" stroke-opacity="0.5"/>
        `):""}

        <!-- Source primitives -->
        ${this.layerVisible.source?this.items.map(l=>G`
          <g class=${`draggable ${this.dragging===l.id?"dragging":""} ${this.selected===l.id?"selected":""}`}
             data-id=${l.id} data-source-id=${l.id}
             transform=${`translate(${l.x.toFixed(0)},${l.y.toFixed(0)})`}
             @pointerdown=${v=>this.onDown(l.id,v)}>
            <ellipse cx="0" cy="0" rx="32" ry="22" fill=${l.color} fill-opacity="0.18"
              stroke=${l.color} stroke-width="1.2"/>
            <circle cx="0" cy="0" r="4" fill=${l.color}/>
            ${this.layerVisible.label?G`<text class="label" x="0" y="40" text-anchor="middle">${l.name}</text>`:""}
          </g>
        `):""}

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

      <div class="scene-toolbar" id="scene-toolbar">
        <button id="zoom-in-btn" title="Zoom in" @click=${this.zoomIn}>+</button>
        <button id="zoom-out-btn" title="Zoom out" @click=${this.zoomOut}>−</button>
        <button id="fit-btn" title="Fit to view" @click=${this.fitView}>⊡</button>
        <button id="layer-source-btn" class=${this.layerVisible.source?"on":""}
          title="Sources" @click=${()=>this.toggleLayer("source")}>●</button>
        <button id="layer-field-btn" class=${this.layerVisible.field?"on":""}
          title="Field lines" @click=${()=>this.toggleLayer("field")}>≈</button>
        <button id="layer-label-btn" class=${this.layerVisible.label?"on":""}
          title="Labels" @click=${()=>this.toggleLayer("label")}>T</button>
      </div>

      <div class="sim-controls" id="sim-controls">
        <button class="step" id="step-back-btn" title="Step back" @click=${this.stepBack}>⏮</button>
        <button class="play" id="play-btn" title="Play / pause" @click=${this.toggleRun}>
          ${g.value?"❚❚":"▶"}
        </button>
        <button class="step" id="step-fwd-btn" title="Step forward" @click=${this.stepFwd}>⏭</button>
        <span class="speed" id="speed-val" title="Cycle speed" @click=${this.cycleSpeed}>${ue.value}×</span>
      </div>

      <div class="scene-readout">
        <div class="stat-card">
          <div class="lbl">|B|</div>
          <div class="val amber" id="bmag-readout">${a.toFixed(3)} nT</div>
        </div>
        <div class="stat-card">
          <div class="lbl">FPS</div>
          <div class="val cyan" id="fps-readout">${M.value>0?Math.round(M.value):"—"}</div>
        </div>
        <div class="stat-card">
          <div class="lbl">SNR</div>
          <div class="val mint" id="snr-readout">${H.value>0?H.value.toFixed(1):"—"}</div>
        </div>
      </div>
    `}};R.styles=y`
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
    .scene-toolbar {
      position: absolute; top: 14px; left: 14px;
      display: flex; gap: 6px; z-index: 5;
      background: rgba(13,17,23,0.85);
      backdrop-filter: blur(8px);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 4px;
    }
    [data-theme="light"] .scene-toolbar { background: rgba(255,255,255,0.85); }
    .scene-toolbar button {
      width: 28px; height: 28px;
      background: transparent;
      border: 1px solid transparent;
      border-radius: 6px;
      color: var(--ink-2);
      cursor: pointer;
      display: grid; place-items: center;
      font-size: 13px;
    }
    .scene-toolbar button:hover { color: var(--ink); background: var(--bg-2); }
    .scene-toolbar button.on { background: var(--bg-3); color: var(--accent); border-color: var(--line-2); }

    .sim-controls {
      position: absolute; bottom: 14px; right: 14px;
      display: flex; gap: 6px; align-items: center;
      background: rgba(13,17,23,0.85);
      backdrop-filter: blur(12px);
      border: 1px solid var(--line-2);
      border-radius: 999px;
      padding: 6px 10px;
      z-index: 5;
    }
    [data-theme="light"] .sim-controls { background: rgba(255,255,255,0.92); }
    .sim-controls .play {
      width: 32px; height: 32px;
      background: var(--accent);
      border: none;
      border-radius: 50%;
      color: #1a0f00;
      cursor: pointer;
      display: grid; place-items: center;
      font-size: 13px;
    }
    .sim-controls .play:hover { filter: brightness(1.08); }
    .sim-controls .step {
      width: 26px; height: 26px;
      border-radius: 6px;
      background: transparent;
      color: var(--ink-2);
      border: 1px solid var(--line);
      cursor: pointer;
      font-size: 11px;
    }
    .sim-controls .step:hover { color: var(--ink); border-color: var(--line-2); }
    .sim-controls .speed {
      font-family: var(--mono); font-size: 11px;
      color: var(--ink-2);
      padding: 0 6px;
      min-width: 36px;
      text-align: center;
      cursor: pointer;
    }
  `;Q([u()],R.prototype,"zoom",2);Q([u()],R.prototype,"layerVisible",2);Q([u()],R.prototype,"items",2);Q([u()],R.prototype,"dragging",2);Q([u()],R.prototype,"selected",2);R=Q([k("nv-scene")],R);var Mt=Object.defineProperty,zt=Object.getOwnPropertyDescriptor,Oe=(e,t,a,r)=>{for(var s=r>1?void 0:r?zt(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&Mt(t,a,s),s};let ne=class extends w{constructor(){super(...arguments),this.tab="signal",this.pinTab=null}connectedCallback(){super.connectedCallback(),f(()=>{xe.value,ye.value,we.value,ke.value,oe.value,A.value,$.value,fe.value,K.value,this.requestUpdate()})}willUpdate(e){e.has("pinTab")&&this.pinTab&&this.tab!==this.pinTab&&(this.tab=this.pinTab)}async verify(){const e=h();if(e){$.value="pending",d("info","verifying witness over 256 frames…");try{const t=Y.value,a=new Uint8Array(32);for(let s=0;s<32;s++)a[s]=parseInt(t.slice(s*2,s*2+2),16);const r=await e.verifyWitness(a);if(r.ok)$.value="ok",A.value=t,d("ok",`witness ${t.slice(0,16)}… matches · determinism gate ✓`);else{$.value="fail";const s=Array.from(r.actual).map(i=>i.toString(16).padStart(2,"0")).join("");A.value=s,d("err",`WITNESS MISMATCH actual=${s.slice(0,16)}…`)}}catch(t){$.value="fail",d("err",`verify failed: ${t.message}`)}}}renderSignalTab(){const i=o=>{let n="";return o.forEach((l,v)=>{const m=v/Math.max(1,199)*320,b=65-l*22;n+=(v===0?"M":"L")+` ${m.toFixed(1)} ${b.toFixed(1)} `}),n};return p`
      <div class="card">
        <div class="card-h">
          <span class="ttl">B-vector trace</span>
          <span class="badge">3-axis · nT</span>
        </div>
        <svg viewBox="0 0 ${320} ${130}" preserveAspectRatio="none">
          <line x1="0" y1=${65} x2=${320} y2=${65} stroke="var(--line)" stroke-width="0.5"/>
          ${G`<path id="trace-x" d=${i(xe.value)} stroke="oklch(0.78 0.14 70)" stroke-width="1.2" fill="none"/>`}
          ${G`<path id="trace-y" d=${i(ye.value)} stroke="oklch(0.78 0.12 195)" stroke-width="1.2" fill="none" opacity="0.8"/>`}
          ${G`<path id="trace-z" d=${i(we.value)} stroke="oklch(0.72 0.18 330)" stroke-width="1.2" fill="none" opacity="0.7"/>`}
        </svg>
      </div>

      <div class="card">
        <div class="card-h">
          <span class="ttl">Frame stream</span>
          <span class="badge" id="strip-rate">live</span>
        </div>
        <div class="frame-strip" id="frame-strip">
          ${ke.value.map(o=>p`<div class="bar" style=${`height:${Math.max(4,o*100)}%`}></div>`)}
        </div>
      </div>
    `}renderFrameTab(){const e=oe.value,t=e?.raw;let a="";return t&&(a=Array.from(t).map(s=>s.toString(16).padStart(2,"0")).slice(0,60).join(" ")),p`
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
        <div class="hex" id="frame-hex">${a||"—"}</div>
      </div>
    `}renderWitnessTab(){const e=$.value,t=e==="ok"?"ok":e==="fail"?"fail":"",a=e==="pending"?"Verifying…":e==="ok"?"✓ Witness verified · determinism gate":e==="fail"?"✗ Witness mismatch · audit required":"Verify witness";return p`
      <div class="card">
        <div class="card-h">
          <span class="ttl">Expected (Proof::EXPECTED_WITNESS_HEX)</span>
          <span class="badge">SHA-256</span>
        </div>
        <div class="witness-box" id="expected-witness">${Y.value||"(loading…)"}</div>
      </div>
      <div class="card">
        <div class="card-h">
          <span class="ttl">Actual (last verify)</span>
          <span class="badge">SHA-256</span>
        </div>
        <div class="witness-box" id="actual-witness">${A.value||"(not verified yet)"}</div>
        <button class="verify-btn ${t}" id="verify-btn" @click=${this.verify}>${a}</button>
      </div>
    `}render(){return p`
      <div class="tabs">
        <button class="tab ${this.tab==="signal"?"active":""}" data-pane="signal" @click=${()=>this.tab="signal"}>Signal</button>
        <button class="tab ${this.tab==="frame"?"active":""}" data-pane="frame" @click=${()=>this.tab="frame"}>Frame</button>
        <button class="tab ${this.tab==="witness"?"active":""}" data-pane="witness" @click=${()=>this.tab="witness"}>Witness</button>
      </div>
      <div class="body">
        ${this.tab==="signal"?this.renderSignalTab():this.tab==="frame"?this.renderFrameTab():this.renderWitnessTab()}
      </div>
    `}};ne.styles=y`
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
  `;Oe([u()],ne.prototype,"tab",2);Oe([Fe({attribute:!1})],ne.prototype,"pinTab",2);ne=Oe([k("nv-inspector")],ne);var Ct=Object.defineProperty,Pt=Object.getOwnPropertyDescriptor,Ge=(e,t,a,r)=>{for(var s=r>1?void 0:r?Pt(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&Ct(t,a,s),s};let $e=class extends w{constructor(){super(...arguments),this.hIdx=-1,this.onKey=e=>{if(e.key==="Enter")this.exec(this.inputEl.value),this.inputEl.value="";else if(e.key==="ArrowUp"){const t=N.value;t.length&&(this.hIdx=Math.max(0,this.hIdx-1),this.inputEl.value=t[this.hIdx]??"",e.preventDefault())}else if(e.key==="ArrowDown"){const t=N.value;t.length&&(this.hIdx=Math.min(t.length,this.hIdx+1),this.inputEl.value=t[this.hIdx]??"",e.preventDefault())}}}connectedCallback(){super.connectedCallback(),f(()=>{T.value,Te.value,ae.value,this.requestUpdate()})}updated(){const e=this.renderRoot.querySelector(".body");e&&(e.scrollTop=e.scrollHeight)}counts(){const e={info:0,warn:0,err:0,dbg:0,ok:0};for(const t of T.value)e[t.level]=(e[t.level]??0)+1;return e.all=T.value.length,e}async exec(e){if(e=e.trim(),!e)return;d("info",`<span style="color:var(--accent);">nvsim&gt;</span> ${e}`),pt(e),this.hIdx=N.value.length;const[t,...a]=e.split(/\s+/),r=a.join(" "),s=h();switch(t){case"help":d("info","commands: help · scene.list · sensor.config · run · pause · reset · seed · proof.verify · proof.export · clear · theme · status");break;case"scene.list":d("info","scene rebar-walkby-01:"),d("info","  rebar.steel.coil   @ [+2.7, 0.0, +0.3] m χ=5000"),d("info","  dipole.heart_proxy @ [-1.4, +0.2, +0.4] m m=1.0e-6 A·m²"),d("info","  loop.mains_60Hz    @ [-1.6, -0.4, 0.0] m I=2 A"),d("info","  eddy.door_steel    @ [+0.0, +1.8, +0.4] m σ=1e6 S/m");break;case"sensor.config":d("info","NvSensor::cots_defaults() {"),d("info","  pos=[0,0,0], V=1mm³, N=1e12, C=0.03, T2*=200ns"),d("info","  D=2.870 GHz, γe=28 GHz/T, Γ=1.0 MHz, axes=4×〈111〉"),d("info","  δB ≈ 1.18 pT/√Hz (Barry 2020 §III.A) }");break;case"run":s&&(await s.run(),g.value=!0,d("ok","pipeline RUN"));break;case"pause":s&&(await s.pause(),g.value=!1,d("warn","pipeline PAUSED"));break;case"reset":s&&(await s.reset(),d("info","pipeline reset · t=0"));break;case"seed":{if(!r){d("info",`current seed = 0x${L.value.toString(16).toUpperCase()}`);break}const i=BigInt(r.startsWith("0x")?r:"0x"+r);L.value=i,s&&await s.setSeed(i),d("ok",`seed → 0x${i.toString(16).toUpperCase()}`);break}case"proof.verify":{if(!s)break;d("dbg","computing SHA-256 over 256 frames…");try{const i=Y.value,o=new Uint8Array(32);for(let l=0;l<32;l++)o[l]=parseInt(i.slice(l*2,l*2+2),16);(await s.verifyWitness(o)).ok?($.value="ok",A.value=i,d("ok",`witness ${i.slice(0,16)}… matches · determinism gate ✓`)):($.value="fail",d("err","WITNESS MISMATCH"))}catch(i){d("err",`verify failed: ${i.message}`)}break}case"proof.export":{if(!s)break;d("dbg","building proof bundle…");try{const i=await s.exportProofBundle(),o=URL.createObjectURL(i),n=document.createElement("a");n.href=o,n.download=`nvsim-proof-${Date.now()}.json`,n.click(),URL.revokeObjectURL(o),d("ok",`proof bundle exported · ${i.size} bytes`)}catch(i){d("err",`export failed: ${i.message}`)}break}case"clear":T.value=[];break;case"theme":{const i=(r||"").toLowerCase();i==="light"||i==="dark"?(x.value=i,d("ok",`theme → ${i}`)):d("info","theme [light|dark]");break}case"status":d("info",`running=${g.value} seed=0x${L.value.toString(16).toUpperCase()} verified=${$.value}`);break;default:d("err",`unknown command: ${t} · try help`)}}render(){const e=this.counts(),t=Te.value,a=T.value.filter(r=>t==="all"||r.level===t);return p`
      <div class="tabs">
        ${["all","info","warn","err","dbg"].map(r=>p`
          <button class="tab ${t===r?"active":""}" data-tab=${r}
            @click=${()=>Te.value=r}>
            ${r} <span class="cnt">${e[r]??0}</span>
          </button>
        `)}
        <span class="spacer"></span>
        <div class="tools">
          <button id="clear-log" title="Clear" @click=${()=>T.value=[]}>×</button>
          <button id="pause-log" title="Pause" @click=${()=>ae.value=!ae.value}>
            ${ae.value?"▶":"❚❚"}
          </button>
        </div>
      </div>
      <div class="body" role="log" aria-live="polite" aria-label="Console output">
        ${a.map(r=>{const s=new Date(r.ts),i=`${String(s.getSeconds()).padStart(2,"0")}.${String(s.getMilliseconds()).padStart(3,"0")}`;return p`<div class="line ${r.level}">
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
    `}};$e.styles=y`
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
  `;Ge([We("#console-input")],$e.prototype,"inputEl",2);$e=Ge([k("nv-console")],$e);const re=[{id:"nvsim",name:"nvsim — NV-diamond magnetometer",category:"sim",crate:"nvsim",summary:"Deterministic forward simulator: scene → Biot–Savart → NV ensemble → ADC → MagFrame stream + SHA-256 witness.",budget:"L",active:!0,status:"available",tags:["quantum","magnetometer","simulator","witness","wasm"],adr:"ADR-089"},{id:"gesture",name:"Gesture (DTW)",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Dynamic-Time-Warping gesture classifier from CSI motion templates.",events:[1],budget:"M",status:"available",tags:["hci","csi","classifier","dtw"],adr:"ADR-014"},{id:"coherence",name:"Coherence gate",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Z-score coherence scoring + Accept/PredictOnly/Reject/Recalibrate gate.",events:[2],budget:"S",status:"available",tags:["gate","csi","coherence","drift"],adr:"ADR-029"},{id:"adversarial",name:"Adversarial-signal detector",category:"ais",crate:"wifi-densepose-wasm-edge",summary:"Physically-impossible-signal detector — multi-link consistency, used to flag spoofed CSI.",events:[3],budget:"M",status:"available",tags:["security","csi","spoofing","mesh"],adr:"ADR-032"},{id:"rvf",name:"RVF — Rust Verified Feature stream",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Verified-frame builder with SHA-256 hash + version metadata for the feature stream.",budget:"S",status:"available",tags:["witness","csi","hash"],adr:"ADR-040"},{id:"occupancy",name:"Occupancy estimator",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Through-wall presence + person-count via CSI amplitude perturbation.",events:[300,301,302],budget:"S",status:"available",tags:["csi","building","presence"]},{id:"vital_trend",name:"Vital-trend monitor",category:"med",crate:"wifi-densepose-wasm-edge",summary:"HR + BR trend tracking with bradycardia/tachycardia/apnea events.",events:[100,101,102,103,104,105],budget:"S",status:"available",tags:["medical","vitals","csi"],adr:"ADR-021"},{id:"intrusion",name:"Intrusion detector",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Zone-based intrusion alert from CSI motion patterns.",events:[200,201],budget:"S",status:"available",tags:["security","zone","csi"]},{id:"med_sleep_apnea",name:"Sleep-apnea detector",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Episodic respiratory pause detection during sleep cycles.",events:[105],budget:"S",status:"available",tags:["medical","sleep","breathing"]},{id:"med_cardiac_arrhythmia",name:"Cardiac arrhythmia",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Beat-to-beat irregularity classifier from cardiac micro-Doppler.",events:[103,104],budget:"M",status:"available",tags:["medical","cardiac","arrhythmia"]},{id:"med_respiratory_distress",name:"Respiratory distress",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Distress signature: rapid shallow breathing + accessory-muscle motion.",events:[101,102],budget:"S",status:"available",tags:["medical","breathing","icu"]},{id:"med_gait_analysis",name:"Gait analysis",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Stride length, cadence, asymmetry from through-wall CSI pose tracking.",budget:"M",status:"available",tags:["medical","gait","pose"]},{id:"med_seizure_detect",name:"Seizure detector",category:"med",crate:"wifi-densepose-wasm-edge",summary:"Tonic-clonic seizure motion signature.",budget:"M",status:"beta",tags:["medical","neuro"]},{id:"sec_perimeter_breach",name:"Perimeter breach",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Approach/departure detection at user-defined boundary segments.",events:[210,211,212,213],budget:"S",status:"available",tags:["security","perimeter"]},{id:"sec_weapon_detect",name:"Metal anomaly / weapon",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Metal-perturbation flag in CSI; potential weapon presence (research).",events:[220,221,222],budget:"M",status:"research",tags:["security","metal","csi"]},{id:"sec_tailgating",name:"Tailgating detector",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Detect 2+ persons crossing a single-passage threshold.",events:[230,231,232],budget:"S",status:"available",tags:["security","access-control"]},{id:"sec_loitering",name:"Loitering detector",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"Stationary occupancy past a configurable dwell threshold.",events:[240,241,242],budget:"S",status:"available",tags:["security","dwell"]},{id:"sec_panic_motion",name:"Panic motion",category:"sec",crate:"wifi-densepose-wasm-edge",summary:"High-energy distress motion: struggle / fleeing pattern.",events:[250,251,252],budget:"S",status:"beta",tags:["security","distress"]},{id:"bld_hvac_presence",name:"HVAC presence",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Occupied/activity-level/departure-countdown for HVAC zones.",events:[310,311,312],budget:"S",status:"available",tags:["hvac","building","energy"]},{id:"bld_lighting_zones",name:"Lighting zones",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Per-zone light on/dim/off cues from occupancy.",events:[320,321,322],budget:"S",status:"available",tags:["lighting","building"]},{id:"bld_elevator_count",name:"Elevator count",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Person count inside elevator car from CSI.",events:[330],budget:"S",status:"available",tags:["elevator","building"]},{id:"bld_meeting_room",name:"Meeting-room utilization",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Meeting size + duration analytics for booking systems.",budget:"S",status:"available",tags:["meeting","analytics"]},{id:"bld_energy_audit",name:"Energy audit",category:"bld",crate:"wifi-densepose-wasm-edge",summary:"Continuous occupancy-vs-HVAC-state audit for energy savings.",budget:"M",status:"available",tags:["energy","audit"]},{id:"ret_queue_length",name:"Queue length",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Live queue-length tracking for checkout / kiosks.",budget:"S",status:"available",tags:["retail","queue"]},{id:"ret_dwell_heatmap",name:"Dwell heatmap",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Per-zone dwell time accumulation; analytics-only export.",budget:"M",status:"available",tags:["retail","heatmap"]},{id:"ret_customer_flow",name:"Customer flow",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Origin-destination flow graph through a store layout.",budget:"M",status:"available",tags:["retail","flow"]},{id:"ret_table_turnover",name:"Table turnover",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Restaurant table seat / vacate transitions.",budget:"S",status:"available",tags:["retail","restaurant"]},{id:"ret_shelf_engagement",name:"Shelf engagement",category:"ret",crate:"wifi-densepose-wasm-edge",summary:"Reach-to-shelf gestures and dwell at product zones.",budget:"M",status:"available",tags:["retail","shelf"]},{id:"ind_forklift_proximity",name:"Forklift proximity",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Worker-near-forklift safety alert.",budget:"S",status:"available",tags:["industrial","safety"]},{id:"ind_confined_space",name:"Confined-space monitor",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Last-person-out detection + presence audit for OSHA confined-space entries.",budget:"S",status:"available",tags:["industrial","osha"]},{id:"ind_clean_room",name:"Clean-room PPE / motion",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Motion patterns consistent with proper PPE-clad movement.",budget:"M",status:"beta",tags:["industrial","cleanroom"]},{id:"ind_livestock_monitor",name:"Livestock monitor",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Vital-sign + activity tracking for stall-bound livestock.",budget:"M",status:"beta",tags:["agriculture","livestock"]},{id:"ind_structural_vibration",name:"Structural vibration",category:"ind",crate:"wifi-densepose-wasm-edge",summary:"Building/equipment micro-vibration via CSI phase derivative.",budget:"M",status:"research",tags:["industrial","vibration"]},{id:"sig_coherence_gate",name:"Coherence gate (extended)",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Hysteresis + multi-state coherence gate driving downstream apps.",budget:"S",status:"available",tags:["gate","csi"]},{id:"sig_flash_attention",name:"Flash attention (CSI)",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Edge-friendly attention block for CSI subcarrier weighting.",budget:"M",status:"beta",tags:["attention","csi"]},{id:"sig_temporal_compress",name:"Temporal-tensor compress",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"RuVector temporal-tensor compression on the CSI buffer.",budget:"M",status:"available",tags:["compress","tensor"]},{id:"sig_sparse_recovery",name:"Sparse recovery",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"114→56 subcarrier sparse interpolation via L1 solver.",budget:"M",status:"available",tags:["sparse","csi"]},{id:"sig_mincut_person_match",name:"Mincut person-match",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"Min-cut person assignment across multistatic frames.",budget:"M",status:"available",tags:["mincut","matching"]},{id:"sig_optimal_transport",name:"Optimal transport",category:"sig",crate:"wifi-densepose-wasm-edge",summary:"OT-based feature alignment between mesh nodes.",budget:"M",status:"beta",tags:["ot","alignment"]},{id:"lrn_dtw_gesture_learn",name:"DTW gesture learn",category:"lrn",crate:"wifi-densepose-wasm-edge",summary:"On-device template learning for personalized gesture libraries.",budget:"M",status:"beta",tags:["lifelong","gesture"]},{id:"lrn_anomaly_attractor",name:"Anomaly attractor",category:"lrn",crate:"wifi-densepose-wasm-edge",summary:"Novelty detector with dynamic-attractor recall.",budget:"M",status:"research",tags:["novelty","lifelong"]},{id:"lrn_meta_adapt",name:"Meta-adapt",category:"lrn",crate:"wifi-densepose-wasm-edge",summary:"Meta-learning adapter for fast site-to-site transfer.",budget:"L",status:"research",tags:["meta-learning"]},{id:"lrn_ewc_lifelong",name:"EWC++ lifelong",category:"lrn",crate:"wifi-densepose-wasm-edge",summary:"Elastic-weight-consolidation gate to avoid catastrophic forgetting.",budget:"M",status:"beta",tags:["lifelong","ewc"]},{id:"spt_pagerank_influence",name:"PageRank influence",category:"spt",crate:"wifi-densepose-wasm-edge",summary:"Graph-influence ranking on the multistatic mesh.",budget:"M",status:"beta",tags:["graph","pagerank"]},{id:"spt_micro_hnsw",name:"µHNSW vector index",category:"spt",crate:"wifi-densepose-wasm-edge",summary:"Tiny HNSW index for AETHER re-ID embeddings on-device.",budget:"M",status:"available",tags:["hnsw","reid"]},{id:"spt_spiking_tracker",name:"Spiking tracker",category:"spt",crate:"wifi-densepose-wasm-edge",summary:"Spiking-network multi-target tracker.",budget:"L",status:"research",tags:["snn","tracker"]},{id:"tmp_pattern_sequence",name:"Pattern sequence",category:"tmp",crate:"wifi-densepose-wasm-edge",summary:"Sequence-of-events pattern matcher (e.g. ingress→linger→egress).",budget:"M",status:"available",tags:["temporal","pattern"]},{id:"tmp_temporal_logic_guard",name:"Temporal logic guard",category:"tmp",crate:"wifi-densepose-wasm-edge",summary:"LTL/MTL safety-property guard over event streams.",budget:"M",status:"beta",tags:["ltl","safety"]},{id:"tmp_goap_autonomy",name:"GOAP autonomy",category:"tmp",crate:"wifi-densepose-wasm-edge",summary:"Goal-oriented action planning for adaptive routines.",budget:"L",status:"research",tags:["planning","autonomy"]},{id:"ais_prompt_shield",name:"Prompt shield",category:"ais",crate:"wifi-densepose-wasm-edge",summary:"Edge-side LLM prompt-injection guard for on-device assistants.",budget:"M",status:"beta",tags:["security","llm"]},{id:"ais_behavioral_profiler",name:"Behavioral profiler",category:"ais",crate:"wifi-densepose-wasm-edge",summary:"Anomalous-behaviour profiler (drift in motion habits).",budget:"M",status:"beta",tags:["anomaly","behaviour"]},{id:"qnt_quantum_coherence",name:"Quantum coherence",category:"qnt",crate:"wifi-densepose-wasm-edge",summary:"Coherence diagnostics adapted for quantum-sensor signals.",budget:"M",status:"research",tags:["quantum","coherence"]},{id:"qnt_interference_search",name:"Interference search",category:"qnt",crate:"wifi-densepose-wasm-edge",summary:"Interferometric anomaly search across mesh viewpoints.",budget:"L",status:"research",tags:["quantum","interference"]},{id:"aut_psycho_symbolic",name:"Psycho-symbolic agent",category:"aut",crate:"wifi-densepose-wasm-edge",summary:"Symbolic-rule + neural-feature hybrid for low-power autonomy loops.",budget:"L",status:"research",tags:["autonomy","symbolic"]},{id:"aut_self_healing_mesh",name:"Self-healing mesh",category:"aut",crate:"wifi-densepose-wasm-edge",summary:"Mesh-topology repair with per-node health gossip.",budget:"M",status:"beta",tags:["mesh","health"]},{id:"exo_ghost_hunter",name:"Ghost hunter (anomaly)",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Empty-room CSI anomaly detector — impulsive/periodic/drift/random + hidden-presence sub-detector.",events:[650,651,652,653],budget:"S",status:"available",tags:["anomaly","paranormal","csi"],adr:"ADR-041"},{id:"exo_breathing_sync",name:"Breathing sync",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Multi-person breathing synchrony analytics.",budget:"M",status:"beta",tags:["breathing","sync"]},{id:"exo_dream_stage",name:"Dream-stage classifier",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"NREM/REM stage classification from breathing + micro-motion.",budget:"M",status:"research",tags:["sleep","rem"]},{id:"exo_emotion_detect",name:"Emotion detector",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Coarse arousal/valence from breathing + heart-rate variability.",budget:"M",status:"research",tags:["affect"]},{id:"exo_gesture_language",name:"Gesture language",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Sign-language pattern recognition.",budget:"L",status:"research",tags:["hci","sign"]},{id:"exo_happiness_score",name:"Happiness score",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Aggregate well-being score from co-occupancy + activity dynamics.",budget:"M",status:"research",tags:["affect","wellbeing"]},{id:"exo_hyperbolic_space",name:"Hyperbolic space embed",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Hyperbolic embeddings for hierarchical scene structure.",budget:"L",status:"research",tags:["embedding","hyperbolic"]},{id:"exo_music_conductor",name:"Music conductor",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Map gesture energy to MIDI tempo/dynamics.",budget:"M",status:"research",tags:["midi","art"]},{id:"exo_plant_growth",name:"Plant-growth tracker",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Slow CSI drift tracking for greenhouse foliage growth.",budget:"L",status:"research",tags:["agriculture"]},{id:"exo_rain_detect",name:"Rain detector",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Outdoor CSI signature of rainfall.",budget:"M",status:"research",tags:["weather"]},{id:"exo_time_crystal",name:"Time-crystal periodicity",category:"exo",crate:"wifi-densepose-wasm-edge",summary:"Periodicity diagnostics with anti-aliasing harmonics.",budget:"M",status:"research",tags:["periodicity"]}],ee={sim:{label:"Simulators",color:"oklch(0.78 0.14 70)",range:"—"},med:{label:"Medical & Health",color:"oklch(0.65 0.22 25)",range:"100–199"},sec:{label:"Security & Safety",color:"oklch(0.7 0.18 35)",range:"200–299"},bld:{label:"Smart Building",color:"oklch(0.78 0.12 195)",range:"300–399"},ret:{label:"Retail & Hospitality",color:"oklch(0.78 0.14 145)",range:"400–499"},ind:{label:"Industrial",color:"oklch(0.72 0.18 330)",range:"500–599"},sig:{label:"Signal Processing",color:"oklch(0.78 0.14 70)",range:"600–619"},lrn:{label:"Online Learning",color:"oklch(0.78 0.12 260)",range:"620–639"},spt:{label:"Spatial / Graph",color:"oklch(0.7 0.18 100)",range:"640–659"},tmp:{label:"Temporal / Planning",color:"oklch(0.7 0.16 50)",range:"660–679"},ais:{label:"AI Safety",color:"oklch(0.65 0.22 25)",range:"700–719"},qnt:{label:"Quantum",color:"oklch(0.72 0.18 290)",range:"720–739"},aut:{label:"Autonomy",color:"oklch(0.78 0.14 145)",range:"740–759"},exo:{label:"Exotic / Research",color:"oklch(0.72 0.18 330)",range:"650–699"}};function Tt(){return re.map(e=>({id:e.id,active:e.active===!0,eventCount:0}))}function At(e,t){if(!e)return 1;const a=e.toLowerCase();let r=0;return t.id.toLowerCase().includes(a)&&(r+=3),t.name.toLowerCase().includes(a)&&(r+=3),t.summary.toLowerCase().includes(a)&&(r+=1),t.tags?.some(s=>s.toLowerCase().includes(a))&&(r+=2),t.category===a&&(r+=5),r}const Rt="nvsim",Dt=1,J="kv";let me=null;function Ke(){return me||(me=new Promise((e,t)=>{const a=indexedDB.open(Rt,Dt);a.onupgradeneeded=()=>{const r=a.result;r.objectStoreNames.contains(J)||r.createObjectStore(J)},a.onsuccess=()=>e(a.result),a.onerror=()=>t(a.error)}),me)}async function F(e){const t=await Ke();return await new Promise((a,r)=>{const i=t.transaction(J,"readonly").objectStore(J).get(e);i.onsuccess=()=>a(i.result),i.onerror=()=>r(i.error)})}async function O(e,t){const a=await Ke();return await new Promise((r,s)=>{const i=a.transaction(J,"readwrite");i.objectStore(J).put(t,e),i.oncomplete=()=>r(),i.onerror=()=>s(i.error)})}var Et=Object.defineProperty,Ft=Object.getOwnPropertyDescriptor,Ye=(e,t,a,r)=>{for(var s=r>1?void 0:r?Ft(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&Et(t,a,s),s};const I=c(Tt()),te=c(""),E=c("all"),S=c("all");(async()=>{const e=await F("app-activations");e&&(I.value=e)})();f(()=>{const e=I.value;e.length>0&&O("app-activations",e)});let Se=class extends w{constructor(){super(...arguments),this.renderTick=0}connectedCallback(){super.connectedCallback(),f(()=>{I.value,te.value,E.value,S.value,this.renderTick++})}isActive(e){return I.value.find(t=>t.id===e)?.active===!0}toggle(e){const t=I.value.map(a=>a.id===e.id?{...a,active:!a.active,lastActivatedAt:Date.now()}:a);I.value=t,d(this.isActive(e.id)?"ok":"info",`app <span class="k">${e.id}</span> deactivated`)}filtered(){let e=re;return E.value!=="all"&&(e=e.filter(t=>t.category===E.value)),S.value!=="all"&&(e=e.filter(t=>t.status===S.value)),te.value.trim()&&(e=e.map(t=>({a:t,s:At(te.value,t)})).filter(t=>t.s>0).sort((t,a)=>a.s-t.s).map(t=>t.a)),e}categoryCounts(){const e={all:re.length};for(const t of Object.keys(ee))e[t]=0;for(const t of re)e[t.category]=(e[t.category]??0)+1;return e}render(){const e=this.filtered(),t=this.categoryCounts(),a=I.value.filter(r=>r.active).length;return p`
      <div class="head">
        <div class="ttl">
          App Store
          <small>${re.length} edge apps · ${a} active</small>
        </div>
        <input class="search" id="app-search" placeholder="Search by name, tag, or category…"
          .value=${te.value}
          @input=${r=>{te.value=r.target.value}} />
      </div>

      <div class="filters">
        <span class="chip ${E.value==="all"?"on":""}"
          @click=${()=>E.value="all"}>
          All<span class="count">${t.all}</span>
        </span>
        ${Object.keys(ee).map(r=>p`
          <span class="chip ${E.value===r?"on":""}"
            @click=${()=>E.value=r}>
            <span class="swatch" style=${`background:${ee[r].color}`}></span>
            ${ee[r].label}
            <span class="count">${t[r]??0}</span>
          </span>
        `)}
        <span style="flex:1; min-width:8px"></span>
        <span class="chip ${S.value==="all"?"on":""}" @click=${()=>S.value="all"}>any</span>
        <span class="chip ${S.value==="available"?"on":""}" @click=${()=>S.value="available"}>available</span>
        <span class="chip ${S.value==="beta"?"on":""}" @click=${()=>S.value="beta"}>beta</span>
        <span class="chip ${S.value==="research"?"on":""}" @click=${()=>S.value="research"}>research</span>
      </div>

      ${e.length===0?p`<div class="empty">No apps match the current filters.</div>`:p`<div class="grid">${e.map(r=>this.card(r))}</div>`}
    `}card(e){const t=this.isActive(e.id),a=ee[e.category];return p`
      <div class="card ${t?"active":""}" data-app-id=${e.id}>
        <div class="card-h">
          <span class="swatch" style=${`background:${a.color}`}></span>
          <span class="name">${e.name}</span>
        </div>
        <div class="summary">${e.summary}</div>
        <div class="meta">
          <span class="badge cat">${a.label}</span>
          <span class="badge status-${e.status}">${e.status}</span>
          ${e.budget?p`<span class="badge budget">budget ${e.budget}</span>`:""}
          ${e.adr?p`<span class="badge">${e.adr}</span>`:""}
          ${e.events?.length?p`<span class="badge">events ${e.events.join("·")}</span>`:""}
        </div>
        <div class="card-foot">
          <span class="events">${e.crate}</span>
          <span class="toggle ${t?"on":""}" role="switch"
            aria-checked=${t}
            data-app-toggle=${e.id}
            @click=${()=>this.toggle(e)}></span>
        </div>
      </div>
    `}};Se.styles=y`
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
  `;Ye([u()],Se.prototype,"renderTick",2);Se=Ye([k("nv-app-store")],Se);var Ot=Object.defineProperty,It=Object.getOwnPropertyDescriptor,pe=(e,t,a,r)=>{for(var s=r>1?void 0:r?It(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&Ot(t,a,s),s};let V=class extends w{constructor(){super(...arguments),this.open=!1,this.filter="",this.idx=0,this.cmds=[{ico:"▶",label:"Run pipeline",kbd:"Space",run:async()=>{await h()?.run(),g.value=!0,C("Pipeline running","▶")}},{ico:"❚",label:"Pause pipeline",run:async()=>{await h()?.pause(),g.value=!1,C("Paused","❚❚")}},{ico:"+",label:"New scene…",kbd:"⌘N",run:()=>se({title:"New scene",body:`<p>Build a fresh magnetic scene. The dashboard generates the JSON
        and pushes it to the running pipeline (or you can copy the JSON
        for offline use).</p>
        <label>Name</label>
        <input type="text" id="ns-name" value="custom-scene-${Date.now().toString(36)}" />
        <label>Heart-proxy dipole moment (A·m²)</label>
        <input type="text" id="ns-moment" value="1.0e-6" />
        <label>Distance heart → sensor (m)</label>
        <input type="text" id="ns-distance" value="0.5" />
        <label>Add ferrous distractor at +x = 1 m?</label>
        <select id="ns-ferrous">
          <option value="0">No</option>
          <option value="1" selected>Yes (steel coil, χ=5000)</option>
        </select>
        <label>Add 60 Hz mains-current loop?</label>
        <select id="ns-mains">
          <option value="0">No</option>
          <option value="1" selected>Yes (2 A loop, 5 cm radius, +y = 1 m)</option>
        </select>`,buttons:[{label:"Cancel",variant:"ghost"},{label:"Create",variant:"primary",onClick:async()=>{const e=document.querySelector("nv-app")?.shadowRoot?.querySelector("nv-modal")?.shadowRoot;if(!e)return;const t=(e.querySelector("#ns-name")?.value??"custom").trim(),a=parseFloat(e.querySelector("#ns-moment")?.value??"1e-6"),r=parseFloat(e.querySelector("#ns-distance")?.value??"0.5"),s=e.querySelector("#ns-ferrous")?.value==="1",i=e.querySelector("#ns-mains")?.value==="1",o={dipoles:[{position:[0,0,r],moment:[0,0,a]}],loops:i?[{centre:[0,1,0],normal:[0,1,0],radius:.05,current:2,n_segments:64}]:[],ferrous:s?[{position:[1,0,0],volume:1e-4,susceptibility:5e3}]:[],eddy:[],sensors:[[0,0,0]],ambient_field:[1e-6,0,0]};await h()?.loadScene(o),d("ok",`scene <span class="s">${t}</span> loaded · 1 dipole · ${i?"1 loop · ":""}${s?"1 ferrous · ":""}1 sensor`),C(`Scene "${t}" loaded`,"+")}}]})},{ico:"📦",label:"Export proof bundle…",kbd:"⌘E",run:async()=>{const e=h();if(e){d("dbg","building proof bundle…");try{const t=await e.exportProofBundle(),a=URL.createObjectURL(t),r=document.createElement("a");r.href=a,r.download=`nvsim-proof-${Date.now()}.json`,r.click(),URL.revokeObjectURL(a),d("ok",`proof bundle exported · ${t.size} bytes`),C(`Proof bundle saved (${t.size} B)`,"📦")}catch(t){d("err",`export failed: ${t.message}`)}}}},{ico:"⟳",label:"Reset pipeline",kbd:"⌘R",run:()=>se({title:"Reset pipeline?",body:"<p>Clears the frame stream and rewinds <code>t</code> to 0.</p>",buttons:[{label:"Cancel",variant:"ghost"},{label:"Reset",variant:"danger",onClick:async()=>{await h()?.reset(),d("warn","pipeline reset · t=0"),C("Pipeline reset","⟳")}}]})},{ico:"✓",label:"Verify witness",run:async()=>{const e=h();if(!e)return;$.value="pending";const t=Y.value,a=new Uint8Array(32);for(let s=0;s<32;s++)a[s]=parseInt(t.slice(s*2,s*2+2),16);(await e.verifyWitness(a)).ok?($.value="ok",A.value=t,C("Witness verified","✓")):($.value="fail",C("Witness mismatch!","✗"))}},{ico:"☼",label:"Toggle theme",kbd:"⌘/",run:()=>{x.value=x.value==="dark"?"light":"dark"}},{ico:"⚙",label:"Open settings",kbd:"⌘,",run:()=>window.dispatchEvent(new CustomEvent("open-settings"))},{ico:"?",label:"Keyboard shortcuts…",run:()=>se({title:"Keyboard shortcuts",body:`<div style="display:grid;grid-template-columns:auto 1fr;gap:6px 16px;font-size:13px;">
        <div><code>⌘K / Ctrl K</code></div><div>Command palette</div>
        <div><code>Space</code></div><div>Play / pause</div>
        <div><code>⌘R</code></div><div>Reset</div>
        <div><code>⌘,</code></div><div>Settings</div>
        <div><code>⌘/</code></div><div>Toggle theme</div>
        <div><code>\`</code></div><div>Debug HUD</div>
        <div><code>1 · 2 · 3</code></div><div>Inspector tabs</div>
        <div><code>Esc</code></div><div>Close modal/palette</div>
        <div><code>/</code></div><div>Focus REPL</div>
      </div>`,buttons:[{label:"Close",variant:"primary"}]})},{ico:"i",label:"About nvsim…",run:()=>se({title:"About nvsim",body:`<p><b>nvsim</b> is a deterministic, byte-reproducible forward simulator for nitrogen-vacancy diamond magnetometry.</p>
        <p>This dashboard runs nvsim as WASM in a Web Worker. Same <code>(scene, config, seed)</code> → byte-identical SHA-256 witness across runs and machines.</p>
        <p>License: MIT OR Apache-2.0 · See ADR-089, ADR-092.</p>`,buttons:[{label:"Close",variant:"primary"}]})}],this.onKey=e=>{(e.metaKey||e.ctrlKey)&&e.key.toLowerCase()==="k"?(e.preventDefault(),this.openPal()):e.key==="Escape"&&this.open?this.closePal():this.open&&(e.key==="ArrowDown"?(this.idx=Math.min(this.cmds.length-1,this.idx+1),e.preventDefault()):e.key==="ArrowUp"?(this.idx=Math.max(0,this.idx-1),e.preventDefault()):e.key==="Enter"&&(this.runIdx(),e.preventDefault()))},this.onOpen=()=>this.openPal()}connectedCallback(){super.connectedCallback(),window.addEventListener("keydown",this.onKey),window.addEventListener("nv-palette",this.onOpen)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("keydown",this.onKey),window.removeEventListener("nv-palette",this.onOpen)}openPal(){this.open=!0,this.setAttribute("open",""),this.filter="",this.idx=0,setTimeout(()=>this.inputEl?.focus(),0)}closePal(){this.open=!1,this.removeAttribute("open")}filtered(){if(!this.filter.trim())return this.cmds;const e=this.filter.toLowerCase();return this.cmds.filter(t=>t.label.toLowerCase().includes(e))}runIdx(){const t=this.filtered()[this.idx];t&&(t.run(),this.closePal())}render(){const e=this.filtered();return p`
      <div class="palette" data-id="palette">
        <div class="input">
          <input id="palette-input" type="text" placeholder="Type a command…"
            .value=${this.filter}
            @input=${t=>{this.filter=t.target.value,this.idx=0}} />
        </div>
        <div class="list">
          ${e.map((t,a)=>p`
            <div class="item ${a===this.idx?"active":""}" @click=${()=>{this.idx=a,this.runIdx()}}>
              <span class="ico">${t.ico}</span>
              <span class="lbl">${t.label}</span>
              ${t.kbd?p`<span class="kbd">${t.kbd}</span>`:""}
            </div>
          `)}
        </div>
      </div>
    `}};V.styles=y`
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
  `;pe([u()],V.prototype,"open",2);pe([u()],V.prototype,"filter",2);pe([u()],V.prototype,"idx",2);pe([We("#palette-input")],V.prototype,"inputEl",2);V=pe([k("nv-palette")],V);var Lt=Object.defineProperty,Ht=Object.getOwnPropertyDescriptor,Ie=(e,t,a,r)=>{for(var s=r>1?void 0:r?Ht(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&Lt(t,a,s),s};let le=class extends w{constructor(){super(...arguments),this.open=!1,this.renderFps=0,this.lastTs=performance.now(),this.frameCount=0,this.rafId=0,this.onKey=e=>{e.key==="`"&&!e.target.matches("input, textarea")&&(this.open=!this.open,this.toggleAttribute("open",this.open))},this.tick=()=>{this.rafId=requestAnimationFrame(this.tick);const e=performance.now();this.frameCount++,e-this.lastTs>=500&&(this.renderFps=this.frameCount*1e3/(e-this.lastTs),this.frameCount=0,this.lastTs=e,this.requestUpdate())}}connectedCallback(){super.connectedCallback(),window.addEventListener("keydown",this.onKey),f(()=>{M.value,Re.value,K.value,H.value,dt.value,this.requestUpdate()}),this.tick()}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("keydown",this.onKey),cancelAnimationFrame(this.rafId)}render(){return p`
      <div class="h"><span>nvsim · debug</span><span class="x" @click=${()=>{this.open=!1,this.removeAttribute("open")}}>✕</span></div>
      <div class="row"><span class="k">render fps</span><span class="v">${this.renderFps.toFixed(1)}</span></div>
      <div class="row"><span class="k">sim fps</span><span class="v">${M.value>0?Math.round(M.value):"—"}</span></div>
      <div class="row"><span class="k">frames</span><span class="v">${Re.value.toString()}</span></div>
      <div class="row"><span class="k">|B|</span><span class="v">${(K.value*1e9).toFixed(3)} nT</span></div>
      <div class="row"><span class="k">SNR</span><span class="v">${H.value>0?H.value.toFixed(1):"—"}</span></div>
      <div class="row"><span class="k">DOM</span><span class="v">${document.querySelectorAll("*").length}</span></div>
    `}};le.styles=y`
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
  `;Ie([u()],le.prototype,"open",2);Ie([u()],le.prototype,"renderFps",2);le=Ie([k("nv-debug-hud")],le);var Nt=Object.defineProperty,Bt=Object.getOwnPropertyDescriptor,Xe=(e,t,a,r)=>{for(var s=r>1?void 0:r?Bt(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&Nt(t,a,s),s};let _e=class extends w{constructor(){super(...arguments),this.open=!1}connectedCallback(){super.connectedCallback(),f(()=>{x.value,_.value,z.value,ve.value,P.value,Pe.value,this.requestUpdate()}),window.addEventListener("open-settings",()=>{this.open=!0,this.setAttribute("open","")})}close(){this.open=!1,this.removeAttribute("open")}render(){return p`
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
              <button class=${x.value==="dark"?"on":""} @click=${()=>x.value="dark"}>dark</button>
              <button class=${x.value==="light"?"on":""} @click=${()=>x.value="light"}>light</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Density</div>
              <div class="desc">Affects panel padding and font scale.</div>
            </div>
            <div class="seg">
              <button class=${_.value==="comfy"?"on":""} @click=${()=>_.value="comfy"}>comfy</button>
              <button class=${_.value==="default"?"on":""} @click=${()=>_.value="default"}>default</button>
              <button class=${_.value==="compact"?"on":""} @click=${()=>_.value="compact"}>compact</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Reduce motion</div>
              <div class="desc">Disable rotating crystal & field-line animation.</div>
            </div>
            <span class="toggle ${z.value?"on":""}"
              @click=${()=>z.value=!z.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Pipeline</h4>
          <div class="row">
            <div><div class="lbl">Auto-rerun on edit</div>
            <div class="desc">Restart pipeline when scene/config changes.</div></div>
            <span class="toggle ${ve.value?"on":""}"
              @click=${()=>ve.value=!ve.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Transport</h4>
          <div class="row">
            <div><div class="lbl">Mode</div></div>
            <div class="seg">
              <button class=${P.value==="wasm"?"on":""} @click=${()=>P.value="wasm"}>WASM</button>
              <button class=${P.value==="ws"?"on":""} @click=${()=>P.value="ws"}>WS</button>
            </div>
          </div>
          ${P.value==="ws"?p`
            <div class="row">
              <div><div class="lbl">WS URL</div></div>
              <input type="text" placeholder="ws://localhost:7878" .value=${Pe.value}
                @input=${e=>Pe.value=e.target.value} />
            </div>`:""}
        </div>
      </div>
    `}};_e.styles=y`
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
  `;Xe([u()],_e.prototype,"open",2);_e=Xe([k("nv-settings-drawer")],_e);var Vt=Object.defineProperty,jt=Object.getOwnPropertyDescriptor,Le=(e,t,a,r)=>{for(var s=r>1?void 0:r?jt(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&Vt(t,a,s),s};const be=[{title:"Welcome to nvsim",body:`<p><b>nvsim</b> is an open-source, deterministic forward simulator for
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
      <code>github.com/ruvnet/RuView</code> · ADR-089, ADR-092 · MIT/Apache-2.0.</p>`,cta:"Get started"}];let de=class extends w{constructor(){super(...arguments),this.open=!1,this.step=0,this.show=()=>{this.step=0,this.open=!0,this.setAttribute("open","")}}async connectedCallback(){super.connectedCallback(),window.addEventListener("nv-show-tour",this.show),await F("onboarding-seen")||(this.open=!0,this.setAttribute("open",""))}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener("nv-show-tour",this.show)}async dismiss(){this.open=!1,this.removeAttribute("open"),await O("onboarding-seen",!0)}next(){this.step<be.length-1?this.step++:this.dismiss()}prev(){this.step>0&&this.step--}render(){const e=be[this.step];return p`
      <div class="card" role="dialog" aria-modal="true" aria-label="Welcome tour">
        <div class="h">
          <h2>${e.title}</h2>
          <button class="skip" @click=${()=>this.dismiss()} aria-label="Skip tour">×</button>
        </div>
        <div class="body" .innerHTML=${e.body}></div>
        <div class="footer">
          <div class="dots">
            ${be.map((t,a)=>p`<div class="dot ${a===this.step?"active":""}"></div>`)}
          </div>
          ${this.step>0?p`<button class="ghost" @click=${()=>this.prev()}>Back</button>`:""}
          <button class="primary" @click=${()=>this.next()}>
            ${this.step===be.length-1?e.cta??"Done":e.cta??"Next"}
          </button>
        </div>
      </div>
    `}};de.styles=y`
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
  `;Le([u()],de.prototype,"open",2);Le([u()],de.prototype,"step",2);de=Le([k("nv-onboarding")],de);var Wt=Object.defineProperty,Ut=Object.getOwnPropertyDescriptor,Z=(e,t,a,r)=>{for(var s=r>1?void 0:r?Ut(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&Wt(t,a,s),s};const qt=[{id:"nvBest",label:"NV-ensemble (best lab)",floorT:1e-12,color:"oklch(0.78 0.14 70)"},{id:"nvCots",label:"NV-DNV-B1 (COTS)",floorT:3e-10,color:"oklch(0.72 0.18 50)"},{id:"squid",label:"SQUID (shielded room)",floorT:1e-15,color:"oklch(0.78 0.12 195)"},{id:"mmw",label:"60 GHz mmWave (μ-Doppler)",floorT:0,color:"oklch(0.78 0.14 145)"},{id:"csi",label:"WiFi CSI (presence)",floorT:0,color:"oklch(0.72 0.18 330)"}];let D=class extends w{constructor(){super(...arguments),this.distanceM=.1,this.momentLog10=-8.3,this.result=null,this.running=!1,this.err=null}predictedDipoleFieldT(e,t){return 4*Math.PI*1e-7*t/(4*Math.PI*Math.pow(Math.max(e,1e-6),3))}async runDemo(){const e=h();if(!e){this.err="WASM client not ready";return}this.err=null,this.running=!0,this.requestUpdate();try{const t=this.distanceM,a=Math.pow(10,this.momentLog10),r={dipoles:[{position:[0,0,t],moment:[0,0,a]}],loops:[],ferrous:[],eddy:[],sensors:[[0,0,0]],ambient_field:[0,0,0]},s={digitiser:{f_s_hz:1e4,f_mod_hz:1e3},sensor:{gamma_fwhm_hz:1e6,t1_s:.005,t2_s:1e-6,t2_star_s:2e-7,contrast:.03,n_spins:1e12,shot_noise_disabled:!1},dt_s:null};this.result=await e.runTransient(r,s,42n,64),d("ok",`ghost-demo · r=${t.toFixed(3)} m · |B| recovered = ${(this.result.bMagT*1e12).toExponential(2)} pT`)}catch(t){this.err=t.message,d("err",`ghost-demo failed: ${this.err}`)}finally{this.running=!1,this.requestUpdate()}}formatField(e){if(e===0)return"0 T";const t=Math.abs(e);return t>=.001?`${(e*1e3).toFixed(2)} mT`:t>=1e-6?`${(e*1e6).toFixed(2)} µT`:t>=1e-9?`${(e*1e9).toFixed(3)} nT`:t>=1e-12?`${(e*1e12).toFixed(2)} pT`:t>=1e-15?`${(e*1e15).toFixed(2)} fT`:t>=1e-18?`${(e*1e18).toFixed(2)} aT`:`${e.toExponential(2)} T`}formatDistance(e){return e<1?`${(e*100).toFixed(1)} cm`:e<1e3?`${e.toFixed(2)} m`:e<1e5?`${(e/1e3).toFixed(2)} km`:`${(e/1609).toFixed(0)} mi`}renderDemo(){const e=Math.pow(10,this.momentLog10),t=this.predictedDipoleFieldT(this.distanceM,e),a=this.result?.bMagT??0,r=(this.result?.noiseFloorPtSqrtHz??0)*1e-12,s=qt.map(n=>{let l="bad",v="below floor";if(n.id==="mmw")this.distanceM<=5?(l="ok",v="µ-Doppler @ chest"):this.distanceM<=15?(l="warn",v="edge of range"):(l="bad",v="out of range");else if(n.id==="csi")this.distanceM<=30?(l=this.distanceM<=10?"ok":"warn",v="presence/breathing"):(l="bad",v="out of range");else if(n.floorT>0){const b=t/n.floorT;b>100?(l="ok",v=`${b.toExponential(1)}× floor`):b>1?(l="warn",v=`${b.toFixed(1)}× floor`):(l="bad",v=`${(1/b).toExponential(1)}× too weak`)}const m=n.floorT>0?Math.max(2,Math.min(100,100+12*Math.log10(t/n.floorT))):n.id==="mmw"?Math.max(2,100-this.distanceM*7):Math.max(2,100-this.distanceM*2);return p`
        <div class="tier-bar" data-tier=${n.id}>
          <div class="fill" style=${`width:${m}%; background:${n.color}; border-color:${n.color}`}></div>
          <div class="lbl">
            <span>${n.label}</span>
            <span class="verdict-${l}" style=${`color:${l==="ok"?"var(--ok)":l==="warn"?"var(--warn)":"var(--bad)"}`}>${v}</span>
          </div>
        </div>
      `}),i=t>1e-12?"ok":t>1e-15?"warn":"bad",o=i==="ok"?`Above NV-ensemble lab floor — close-range MCG plausible at ${this.formatDistance(this.distanceM)}.`:i==="warn"?`Below NV ensemble best, above SQUID — research-grade only at ${this.formatDistance(this.distanceM)}.`:`Below every published instrument's noise floor at ${this.formatDistance(this.distanceM)}. Press-release physics.`;return p`
      <div class="demo">
        <h3 style="margin: 0 0 6px;">Try it yourself</h3>
        <div style="font-size: 12.5px; color: var(--ink-2); margin-bottom: 4px; line-height: 1.5;">
          Place a cardiac dipole at variable distance from the NV sensor. The
          dashboard runs the <i>real</i> nvsim Rust pipeline (compiled to WASM)
          end-to-end and reports what each tier would actually detect. Same
          determinism contract as the rest of the dashboard.
        </div>
        <div class="demo-grid">
          <div>
            <div class="control">
              <div class="top">
                <span class="lbl">Distance from sensor</span>
                <span class="val" id="demo-dist-val">${this.formatDistance(this.distanceM)}</span>
              </div>
              <input type="range" id="demo-distance"
                min="-2" max="5" step="0.05"
                .value=${String(Math.log10(this.distanceM))}
                @input=${n=>{this.distanceM=Math.pow(10,+n.target.value)}} />
              <div style="font-size: 10.5px; color: var(--ink-3); margin-top: 4px; font-family: var(--mono);">
                10 cm → 100 km log scale
              </div>
            </div>
            <div class="control">
              <div class="top">
                <span class="lbl">Heart dipole moment</span>
                <span class="val" id="demo-moment-val">${e.toExponential(2)} A·m²</span>
              </div>
              <input type="range" id="demo-moment"
                min="-10" max="-6" step="0.05"
                .value=${String(this.momentLog10)}
                @input=${n=>{this.momentLog10=+n.target.value}} />
              <div style="font-size: 10.5px; color: var(--ink-3); margin-top: 4px; font-family: var(--mono);">
                published cardiac MCG ≈ 5×10⁻⁹ A·m²
              </div>
            </div>
            <button class="demo-btn" id="demo-run-btn" ?disabled=${this.running}
              @click=${()=>this.runDemo()}>
              ${this.running?"Running nvsim…":"▶ Run nvsim at this distance"}
            </button>
            ${this.err?p`<div class="verdict bad" style="margin-top: 10px;">Error: ${this.err}</div>`:""}
          </div>

          <div>
            <div class="readout">
              <div class="readout-row">
                <span class="l">Predicted |B| (1/r³)</span>
                <span class="v amber" id="demo-predicted">${this.formatField(t)}</span>
              </div>
              <div class="readout-row">
                <span class="l">Recovered |B| (nvsim)</span>
                <span class="v" id="demo-recovered">${this.result?this.formatField(a):"—"}</span>
              </div>
              <div class="readout-row">
                <span class="l">Sensor noise floor</span>
                <span class="v" id="demo-floor">${this.result?this.formatField(r)+"/√Hz":"—"}</span>
              </div>
              <div class="readout-row">
                <span class="l">Frames run</span>
                <span class="v" id="demo-frames">${this.result?.nFrames??"—"}</span>
              </div>
              <div class="readout-row">
                <span class="l">Witness (this run)</span>
                <span class="v" style="font-size: 10px;" id="demo-witness">${this.result?.witnessHex.slice(0,16)??"—"}…</span>
              </div>
            </div>
            <div style="margin-top: 14px;">
              <div style="font-size: 11.5px; color: var(--ink-3); text-transform: uppercase; letter-spacing: 0.06em; margin-bottom: 8px;">
                Per-tier detectability
              </div>
              ${s}
            </div>
          </div>
        </div>
        <div class="verdict ${i}" id="demo-verdict">${o}</div>
        <div class="demo-notes">
          The <code>predicted</code> value uses the closed-form magnetic-dipole
          far field <code>|B| = μ₀·m / (4π·r³)</code>. The <code>recovered</code>
          value comes from the same Rust pipeline that drives the Witness panel —
          scene → Biot-Savart → NV ensemble → ADC → MagFrame. Use the moment
          slider to ask "what if the heart were stronger?". Use the distance
          slider to walk through 10 cm (clinical MCG), 1 m (close approach),
          10 m (room-scale), 1 km (skeptic's range), and 65 km (the press claim).
        </div>
      </div>
    `}render(){return p`
      <h1>Ghost Murmur — open-source reality check</h1>
      <div class="subtitle">
        The physics-vs-press audit for the publicly-reported April 2026
        CIA NV-diamond heartbeat detector, and how RuView's existing
        stack maps onto an honest, civilian version of the same idea.
      </div>

      <div class="links">
        <a href="https://github.com/ruvnet/RuView/blob/feat/nvsim-pipeline-simulator/docs/research/quantum-sensing/16-ghost-murmur-ruview-spec.md" target="_blank" rel="noopener">
          📄 Full spec (583 lines)
        </a>
        <a href="https://gist.github.com/ruvnet/e44d0c3f0ad10d9c4933a196a16d405c" target="_blank" rel="noopener">
          ✦ Public gist
        </a>
        <a href="https://github.com/ruvnet/RuView/issues/437" target="_blank" rel="noopener">
          # Issue #437
        </a>
        <a href="https://www.scientificamerican.com/article/what-is-the-quantum-ghost-murmur-purportedly-used-in-iran-scientists/" target="_blank" rel="noopener">
          ↗ Scientific American
        </a>
      </div>

      <h2>What the press reported</h2>
      <div class="grid">
        <div class="card">
          <h3>The story</h3>
          <p>3 Apr 2026: USAF F-15E pilot "Dude 44 Bravo" goes down in southern Iran during the regional exchange and evades for ~2 days.</p>
          <p>President Trump publicly suggests detection from <b>40 miles away</b> on a mountainside at night; CIA Director Ratcliffe says "invisible to the enemy, but not to the CIA."</p>
        </div>
        <div class="card">
          <h3>The named tech</h3>
          <p><b>"Ghost Murmur"</b> — Lockheed Skunk Works system using NV defects in synthetic diamond + AI to extract a heartbeat from environmental noise.</p>
          <p>Outlets: <i>Newsweek, Scientific American, Military.com, WION, Open The Magazine, Yahoo, Calcalist</i> + HN thread #47679241.</p>
        </div>
        <div class="card">
          <h3>What physicists said</h3>
          <p>Wikswo (Vanderbilt), Orzel (Union College), Roth (Oakland) — all pushing back hard.</p>
          <p>"At 1 km, the heartbeat field drops to ~10⁻¹² of its 10 cm value." MCG-only at multi-mile range is <span class="pill skeptical">not consistent with published physics</span>.</p>
        </div>
      </div>

      <h2>Live demo — nvsim WASM</h2>
      ${this.renderDemo()}

      <h2>Physics reality check</h2>
      <div class="card" style="padding: 6px 14px;">
        <table>
          <thead>
            <tr><th>Distance</th><th>Cardiac MCG (peak QRS)</th><th>vs Earth field (~50 µT)</th></tr>
          </thead>
          <tbody>
            <tr><td>10 cm</td><td class="amber">50 pT</td><td>10⁹× weaker</td></tr>
            <tr><td>1 m</td><td class="amber">50 fT</td><td>10¹²× weaker</td></tr>
            <tr><td>10 m</td><td class="cyan">50 aT</td><td>10¹⁵× weaker</td></tr>
            <tr><td>1 km</td><td class="bad">5 × 10⁻²³ T</td><td>10²⁷× weaker</td></tr>
            <tr><td>40 mi (65 km)</td><td class="bad">~10⁻²⁸ T</td><td>10³³× weaker</td></tr>
          </tbody>
        </table>
        <p style="font-size: 12px; color: var(--ink-3); margin: 10px 0 0; line-height: 1.5;">
          Best published NV-ensemble lab record: <b>0.9 pT/√Hz</b> [Wolf 2015].
          Best SQUID in a shielded room: <b>~1 fT/√Hz</b>. To detect a single heartbeat at 10 m
          you'd need ~2 billion× more sensitivity than any published ensemble has ever shown,
          in a magnetically silent environment. <i>40 miles is press-release physics.</i>
        </p>
      </div>

      <h2>RuView's three-tier mesh — what is actually buildable</h2>
      <div class="architecture">                      ┌──────────────────────────┐
                      │   Tier 3 — NV-diamond    │  Range: 0.1–2 m (lab)
                      │     magnetometer ring    │  Status: nvsim simulator only
                      │     (close-confirm)      │  Hardware: $$$ (≥$8k DNV-B1)
                      └──────────┬───────────────┘
                                 │
                      ┌──────────┴───────────────┐
                      │   Tier 2 — 60 GHz FMCW   │  Range: 1–10 m HR/BR
                      │     mmWave radar mesh    │  Status: shipping (ADR-021)
                      │   (vital signs, posture) │  Hardware: $15 (MR60BHA2 + ESP32-C6)
                      └──────────┬───────────────┘
                                 │
                      ┌──────────┴───────────────┐
                      │  Tier 1 — WiFi CSI mesh  │  Range: 10–30 m through-wall
                      │   (presence, breathing,  │  Status: shipping (ADR-014, ADR-029)
                      │    pose, intention)      │  Hardware: $9 (ESP32-S3 8MB)
                      └──────────┬───────────────┘
                                 │
                                 ▼
                  ┌────────────────────────────────┐
                  │  RuvSense multistatic fusion   │
                  │   + cross-viewpoint attention  │
                  │   + AETHER re-ID embeddings    │
                  │   + Cramer-Rao gating          │
                  └────────────────────────────────┘</div>

      <h2>Press claim → RuView equivalent</h2>
      <div class="card" style="padding: 6px 14px;">
        <table>
          <thead>
            <tr><th>Press claim</th><th>RuView equivalent today</th><th>Crate / ADR</th><th>Honest range</th></tr>
          </thead>
          <tbody>
            <tr>
              <td>NV-diamond magnetometry</td>
              <td>Deterministic NV pipeline simulator</td>
              <td><code>nvsim</code> · ADR-089</td>
              <td>Simulator only</td>
            </tr>
            <tr>
              <td>"AI strips environmental noise"</td>
              <td>RuvSense multistatic fusion + AETHER</td>
              <td>signal/ruvsense/ · ADR-029</td>
              <td>Mature</td>
            </tr>
            <tr>
              <td>Heartbeat at distance</td>
              <td>60 GHz FMCW HR/BR + WiFi CSI breathing</td>
              <td>vitals · ADR-021</td>
              <td><span class="pill ok">1–5 m HR · 10–30 m presence</span></td>
            </tr>
            <tr>
              <td>Long-range localisation</td>
              <td>Multistatic time-of-flight + CRLB</td>
              <td>ruvector/viewpoint/</td>
              <td>Limited by node spacing</td>
            </tr>
            <tr>
              <td><i>40-mile single-heartbeat detection</i></td>
              <td><i>Not feasible at any tier</i></td>
              <td>—</td>
              <td><span class="pill skeptical">Press-release physics</span></td>
            </tr>
          </tbody>
        </table>
      </div>

      <h2>Build today on $165</h2>
      <div class="grid">
        <div class="card">
          <h3>Bill of materials</h3>
          <p style="font-family: var(--mono); font-size: 11.5px; line-height: 1.7; color: var(--ink-2);">
            3 × ESP32-S3 8 MB ($9 ea)<br>
            3 × PoE injector + cat6 ($6 ea)<br>
            1 × ESP32-C6 + Seeed MR60BHA2 ($15)<br>
            1 × Raspberry Pi 5 8 GB ($80)<br>
            1 × unmanaged GbE switch ($25)
          </p>
          <p><b>Total: $165</b></p>
        </div>
        <div class="card">
          <h3>Honest performance</h3>
          <span class="stat"><span class="v">95%</span><span class="l">TPR (LOS, 0–15 m)</span></span><br><br>
          <span class="stat"><span class="v">±2 bpm</span><span class="l">HR (LOS 0–3 m)</span></span><br><br>
          <span class="stat"><span class="v">±1 br/min</span><span class="l">BR (any mode)</span></span><br><br>
          <span class="stat"><span class="v">~10 cm</span><span class="l">pose error</span></span><br><br>
          <span class="stat"><span class="v">80–150 ms</span><span class="l">end-to-end latency</span></span>
        </div>
        <div class="card">
          <h3>Determinism</h3>
          <p>Same <code style="font-family: var(--mono); color: var(--accent);">(scene, config, seed)</code> → byte-identical SHA-256 witness across browsers, OSes, transports.</p>
          <p>Reference: <span style="font-family: var(--mono); font-size: 10.5px; color: var(--accent-3);">cc8de9b01b0ff5bd…</span></p>
          <p>Try the Witness tab on the right — it re-derives the hash live in this browser and compares against the published reference.</p>
        </div>
      </div>

      <h2>Privacy, ethics, legal</h2>
      <div class="ethics">
        <h3>This is the open-source version. Same physics, opposite governance.</h3>
        <ul>
          <li><b>Civilian opt-in only</b> — search-and-rescue, elder-care, occupancy, ICU vitals. Not surveillance.</li>
          <li><b>No directional pursuit</b> — no beam-steering, target-following, or remote person-of-interest tracking.</li>
          <li><b>Data minimisation</b> — fused output is <code>(presence, HR, BR, pose, p_alive)</code>; raw streams discarded at the edge.</li>
          <li><b>PII gates</b> (ADR-040) block identifying biometric streams from leaving the local mesh without consent.</li>
          <li><b>Adversarial-signal detection</b> flags physically-impossible signal patterns from compromised mesh nodes.</li>
          <li><b>No export-controlled hardware</b> — RuView targets &lt; $50 COTS. ITAR/EAR sub-THz coherent radars and shielded NV ensembles are out of scope.</li>
        </ul>
        <p style="font-size: 11.5px; color: var(--ink-3); margin: 10px 0 0;">
          RuView is not affiliated with the United States government, the CIA, Lockheed Martin,
          or any classified program. References to "Ghost Murmur" in this view refer
          exclusively to the publicly-reported program of that name as covered in the open
          press in April 2026.
        </p>
      </div>

      <h2>Cross-references</h2>
      <div class="card">
        <p style="font-size: 12px; color: var(--ink-2); line-height: 1.7; margin: 0;">
          <b>ADRs:</b> 014 (signal) · 021 (vitals) · 024 (AETHER) · 027 (MERIDIAN) ·
          028 (witness audit) · 029 (RuvSense) · 040 (PII gates) · 086 (ESP32 RaBitQ) ·
          <b>089 (nvsim, Accepted)</b> · 090 (Lindblad, Proposed-conditional) ·
          091 (sub-THz radar research) · <b>092 (this dashboard)</b>.<br><br>
          <b>Primary physics:</b> Cohen 1970 · Bison 2009 · Wolf 2015 · Barry RMP 2020 · Doherty 2013 · Jackson 3e §5.6/§5.8.
        </p>
      </div>
    `}};D.styles=y`
    :host {
      display: block;
      height: 100%;
      overflow-y: auto;
      background: radial-gradient(ellipse at 50% 30%, var(--bg-2) 0%, var(--bg-0) 70%);
      padding: 24px 28px 60px;
    }
    h1 {
      margin: 0 0 4px;
      font-size: 22px;
      letter-spacing: -0.02em;
      color: var(--ink);
    }
    .subtitle {
      color: var(--ink-3);
      font-size: 13px;
      margin-bottom: 22px;
    }
    .links {
      display: flex; flex-wrap: wrap; gap: 6px;
      margin-bottom: 22px;
    }
    .links a {
      padding: 5px 10px;
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: 999px;
      font-size: 11.5px;
      font-family: var(--mono);
      color: var(--accent-2);
      text-decoration: none;
    }
    .links a:hover { border-color: var(--accent-2); }
    h2 {
      font-size: 14px;
      font-weight: 600;
      letter-spacing: 0.06em;
      text-transform: uppercase;
      color: var(--ink-3);
      margin: 28px 0 10px;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      gap: 12px;
    }
    .card {
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: var(--radius);
      padding: 14px;
    }
    .card h3 {
      margin: 0 0 8px;
      font-size: 13.5px; font-weight: 600;
      color: var(--ink);
    }
    .card p {
      font-size: 12.5px; color: var(--ink-2);
      margin: 0 0 8px;
      line-height: 1.5;
    }
    .card p:last-child { margin-bottom: 0; }
    .stat {
      display: inline-flex; align-items: baseline; gap: 6px;
      margin-right: 10px;
    }
    .stat .v {
      font-family: var(--mono); font-size: 16px; font-weight: 600;
      color: var(--accent);
    }
    .stat .l {
      font-size: 10px; color: var(--ink-3);
      text-transform: uppercase; letter-spacing: 0.04em;
    }
    table {
      width: 100%; border-collapse: collapse;
      font-size: 12.5px;
    }
    th, td {
      padding: 8px 10px;
      text-align: left;
      border-bottom: 1px solid var(--line);
    }
    th {
      color: var(--ink-3);
      font-weight: 600;
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.06em;
    }
    td.amber { color: var(--accent); font-family: var(--mono); }
    td.cyan { color: var(--accent-2); font-family: var(--mono); }
    td.bad { color: var(--bad); font-family: var(--mono); }
    .pill {
      display: inline-block;
      padding: 1px 6px;
      border-radius: 4px;
      font-family: var(--mono);
      font-size: 10px;
      border: 1px solid var(--line);
    }
    .pill.ok { color: var(--ok); border-color: oklch(0.78 0.14 145 / 0.4); }
    .pill.skeptical { color: var(--bad); border-color: oklch(0.65 0.22 25 / 0.4); }
    .pill.partial { color: var(--warn); border-color: oklch(0.7 0.18 35 / 0.4); }
    .architecture {
      font-family: var(--mono);
      font-size: 11px;
      color: var(--ink-2);
      background: var(--bg-3);
      padding: 16px;
      border-radius: var(--radius-sm);
      border: 1px solid var(--line);
      white-space: pre;
      overflow-x: auto;
      line-height: 1.4;
    }
    .ethics {
      background: linear-gradient(180deg, var(--bg-2) 0%, oklch(0.65 0.22 25 / 0.04) 100%);
      border: 1px solid oklch(0.65 0.22 25 / 0.25);
      border-radius: var(--radius);
      padding: 16px;
    }
    .ethics h3 { color: var(--bad); margin-top: 0; }
    .ethics ul { padding-left: 18px; margin: 8px 0; }
    .ethics li { font-size: 12.5px; color: var(--ink-2); margin-bottom: 4px; }

    /* Demo */
    .demo {
      background: linear-gradient(180deg, var(--bg-2) 0%, oklch(0.78 0.14 70 / 0.04) 100%);
      border: 1px solid oklch(0.78 0.14 70 / 0.3);
      border-radius: var(--radius);
      padding: 18px;
    }
    .demo-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 18px;
      margin-top: 12px;
    }
    @media (max-width: 720px) { .demo-grid { grid-template-columns: 1fr; } }
    .control { margin-bottom: 14px; }
    .control .top {
      display: flex; justify-content: space-between;
      font-size: 12px; margin-bottom: 6px;
    }
    .control .top .lbl { color: var(--ink-3); }
    .control .top .val {
      font-family: var(--mono); color: var(--ink);
    }
    .control input[type="range"] {
      -webkit-appearance: none; appearance: none;
      width: 100%; height: 4px;
      background: var(--bg-3); border-radius: 2px; outline: none;
    }
    .control input[type="range"]::-webkit-slider-thumb {
      -webkit-appearance: none; appearance: none;
      width: 14px; height: 14px; border-radius: 50%;
      background: var(--accent); cursor: pointer;
      border: 2px solid var(--bg-2);
    }
    .demo-btn {
      width: 100%;
      padding: 10px;
      border: 1px solid var(--accent);
      background: var(--accent);
      color: #1a0f00;
      border-radius: 8px;
      font-size: 13px; font-weight: 600;
      cursor: pointer;
    }
    .demo-btn:hover { filter: brightness(1.08); }
    .demo-btn:disabled { opacity: 0.6; cursor: progress; }
    .readout {
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 12px;
    }
    .readout-row {
      display: flex; justify-content: space-between;
      padding: 4px 0;
      font-family: var(--mono); font-size: 12px;
    }
    .readout-row .l { color: var(--ink-3); }
    .readout-row .v { color: var(--ink); }
    .readout-row .v.amber { color: var(--accent); }
    .tier-bar {
      position: relative;
      margin: 6px 0;
      height: 22px;
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 4px;
      overflow: hidden;
    }
    .tier-bar .fill {
      position: absolute; top: 0; bottom: 0; left: 0;
      transition: width 0.2s ease-out;
      border-right: 2px solid;
    }
    .tier-bar .lbl {
      position: relative; z-index: 1;
      font-family: var(--mono); font-size: 11px;
      padding: 3px 8px;
      color: var(--ink);
      display: flex; justify-content: space-between;
      pointer-events: none;
    }
    .verdict {
      margin-top: 10px;
      padding: 10px 12px;
      border-radius: 8px;
      font-size: 12.5px; font-weight: 500;
      border: 1px solid;
    }
    .verdict.ok { background: oklch(0.78 0.14 145 / 0.08); border-color: oklch(0.78 0.14 145 / 0.4); color: var(--ok); }
    .verdict.warn { background: oklch(0.7 0.18 35 / 0.08); border-color: oklch(0.7 0.18 35 / 0.4); color: var(--warn); }
    .verdict.bad { background: oklch(0.65 0.22 25 / 0.08); border-color: oklch(0.65 0.22 25 / 0.4); color: var(--bad); }
    .demo-notes {
      font-size: 11.5px; color: var(--ink-3);
      margin-top: 10px; line-height: 1.5;
    }
  `;Z([u()],D.prototype,"distanceM",2);Z([u()],D.prototype,"momentLog10",2);Z([u()],D.prototype,"result",2);Z([u()],D.prototype,"running",2);Z([u()],D.prototype,"err",2);D=Z([k("nv-ghost-murmur")],D);var Gt=Object.defineProperty,Kt=Object.getOwnPropertyDescriptor,Je=(e,t,a,r)=>{for(var s=r>1?void 0:r?Kt(t,a):t,i=e.length-1,o;i>=0;i--)(o=e[i])&&(s=(r?o(t,a,s):o(s))||s);return r&&s&&Gt(t,a,s),s};let Me=class extends w{constructor(){super(...arguments),this.view="scene"}render(){return p`
      <a class="skip-link" href="#main-content"
        @click=${e=>{e.preventDefault(),this.shadowRoot?.querySelector(".main")?.focus()}}>
        Skip to main content
      </a>
      <div class="app">
        <nv-rail .view=${this.view} @navigate=${e=>this.view=e.detail}></nv-rail>
        <nv-topbar></nv-topbar>
        <nv-sidebar></nv-sidebar>
        <main class="main" id="main-content" tabindex="-1" role="main" aria-label="Main view">
          ${this.view==="apps"?p`<nv-app-store></nv-app-store>`:this.view==="ghost-murmur"?p`<nv-ghost-murmur></nv-ghost-murmur>`:p`<nv-scene></nv-scene>`}
        </main>
        <nv-inspector
          .pinTab=${this.view==="inspector"?"signal":this.view==="witness"?"witness":null}>
        </nv-inspector>
        <nv-console></nv-console>
      </div>
      <nv-toast></nv-toast>
      <nv-modal></nv-modal>
      <nv-palette></nv-palette>
      <nv-debug-hud></nv-debug-hud>
      <nv-settings-drawer></nv-settings-drawer>
      <nv-onboarding></nv-onboarding>
    `}};Me.styles=y`
    :host {
      display: block;
      height: 100vh;
      width: 100vw;
      background: var(--bg-0);
    }
    .skip-link {
      position: absolute;
      top: -40px;
      left: 8px;
      padding: 6px 12px;
      background: var(--accent);
      color: #1a0f00;
      border-radius: 6px;
      font-size: 12.5px;
      font-weight: 600;
      text-decoration: none;
      z-index: 1000;
      transition: top 0.15s;
    }
    .skip-link:focus { top: 8px; }
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
  `;Je([u()],Me.prototype,"view",2);Me=Je([k("nv-app")],Me);function Yt(e,t,a){const r=e.getUint32(t+0,!0),s=e.getUint16(t+4,!0),i=e.getUint16(t+6,!0),o=e.getUint16(t+8,!0),n=e.getBigUint64(t+12,!0),l=e.getFloat32(t+20,!0),v=e.getFloat32(t+24,!0),m=e.getFloat32(t+28,!0),b=e.getFloat32(t+32,!0),Ce=e.getFloat32(t+36,!0),Qe=e.getFloat32(t+40,!0),Ze=e.getFloat32(t+44,!0),et=e.getFloat32(t+48,!0);return{magic:r,version:s,flags:i,sensorId:o,tUs:n,bPt:[l,v,m],sigmaPt:[b,Ce,Qe],noiseFloorPtSqrtHz:Ze,temperatureK:et,raw:a.subarray(t,t+60)}}function Xt(e){const a=new DataView(e.buffer,e.byteOffset,e.byteLength),r=[];for(let s=0;s+60<=e.byteLength;s+=60)r.push(Yt(a,s,e));return r}class Jt{constructor(){this.nextId=1,this.pending=new Map,this.frameSubs=new Set,this.eventSubs=new Set,this.bootInfo=null,this.worker=new Worker(new URL("/RuView/nvsim/assets/worker-C19MRcXs.js",import.meta.url),{type:"module"}),this.worker.addEventListener("message",t=>this.onMessage(t)),this.worker.addEventListener("error",t=>this.eventSubs.forEach(a=>a({type:"log",level:"err",msg:String(t.message)})))}onMessage(t){const a=t.data;if(a.type==="frames"){const r=a.batch,s=new Uint8Array(r),o={frames:Xt(s),bytes:s};this.frameSubs.forEach(l=>l(o));const n=a.fps;n>0&&this.eventSubs.forEach(l=>l({type:"fps",value:n}));return}if(a.type==="state"){this.eventSubs.forEach(r=>r({type:"state",running:!!a.running,t:0,framesEmitted:Number(a.framesEmitted??0)}));return}if(a.type!=="ready"){if(a.type==="err"&&a.id==null){this.eventSubs.forEach(r=>r({type:"log",level:"err",msg:String(a.msg)}));return}if(typeof a.id=="number"&&this.pending.has(a.id)){const r=this.pending.get(a.id);this.pending.delete(a.id),a.type==="err"?r.reject(new Error(String(a.msg))):r.resolve(a)}}}rpc(t,a=[]){const r=this.nextId++;return new Promise((s,i)=>{this.pending.set(r,{resolve:s,reject:i}),this.worker.postMessage({...t,id:r},a)})}async boot(){if(this.bootInfo)return this.bootInfo;const a=await this.rpc({type:"boot",base:"/RuView/nvsim/"});return this.bootInfo={buildVersion:a.buildVersion,frameMagic:a.frameMagic,frameBytes:a.frameBytes,expectedWitnessHex:a.expectedWitnessHex},this.bootInfo}async loadScene(t){await this.rpc({type:"setScene",json:JSON.stringify(t)})}async setConfig(t){await this.rpc({type:"setConfig",json:JSON.stringify(t)})}async setSeed(t){await this.rpc({type:"setSeed",seed:Number(t&0xFFFFFFFFn)})}async reset(){await this.rpc({type:"reset"})}async run(t){await this.rpc({type:"run"})}async pause(){await this.rpc({type:"pause"})}async step(t,a){await this.rpc({type:"step"})}onFrames(t){this.frameSubs.add(t)}onEvent(t){this.eventSubs.add(t)}async generateWitness(t){const a=await this.rpc({type:"witnessGenerate",samples:t});return new Uint8Array(a.witness)}async verifyWitness(t){const a=t.slice().buffer,r=await this.rpc({type:"witnessVerify",samples:256,expected:a},[a]);return r.ok?{ok:!0}:{ok:!1,actual:new Uint8Array(r.actual)}}async runTransient(t,a,r,s){const i=await this.rpc({type:"runTransient",scene:JSON.stringify(t),config:JSON.stringify(a),seed:Number(r&0xFFFFFFFFn),samples:s});return{bRecoveredT:[i.bRecoveredT[0],i.bRecoveredT[1],i.bRecoveredT[2]],bMagT:i.bMagT,noiseFloorPtSqrtHz:i.noiseFloorPtSqrtHz,sigmaPt:[i.sigmaPt[0],i.sigmaPt[1],i.sigmaPt[2]],nFrames:i.nFrames,witnessHex:i.witnessHex}}async exportProofBundle(){const t=await this.generateWitness(256),a=Array.from(t).map(i=>i.toString(16).padStart(2,"0")).join(""),r=this.bootInfo??await this.boot(),s=JSON.stringify({kind:"nvsim-proof-bundle",version:r.buildVersion,seed:"0x0000002A",nSamples:256,witness:a,expected:r.expectedWitnessHex,ok:a===r.expectedWitnessHex,ts:new Date().toISOString()},null,2);return new Blob([s],{type:"application/json"})}async buildId(){return(await this.rpc({type:"buildId"})).buildId}async close(){this.worker.terminate()}}function Be(e){document.documentElement.setAttribute("data-theme",e)}function Ve(e){document.body.classList.remove("density-comfy","density-default","density-compact"),document.body.classList.add(`density-${e}`)}function je(e){document.body.classList.toggle("reduce-motion",e)}(async()=>{const e=await F("theme")??"dark",t=await F("density")??"default",a=window.matchMedia?.("(prefers-reduced-motion: reduce)").matches??!1,r=await F("motionReduced")??a;x.value=e,Be(e),_.value=t,Ve(t),z.value=r,je(r),f(()=>{Be(x.value),O("theme",x.value)}),f(()=>{Ve(_.value),O("density",_.value)}),f(()=>{je(z.value),O("motionReduced",z.value)});const s=await F("repl-history");s&&Array.isArray(s)&&(N.value=s),f(()=>{O("repl-history",N.value)});const i=await F("scene-positions");i&&Array.isArray(i)&&(ie.value=i),f(()=>{O("scene-positions",ie.value)});const o=new Jt;ut(o),d("info","nvsim — booting WASM runtime"),o.onEvent(n=>{n.type==="log"&&d(n.level,n.msg),n.type==="fps"&&(M.value=n.value),n.type==="state"&&(Re.value=BigInt(n.framesEmitted))}),o.onFrames(n=>{if(n.frames.length===0)return;const l=n.frames[n.frames.length-1];oe.value=l;const v=l.bPt[0]*1e-12,m=l.bPt[1]*1e-12,b=l.bPt[2]*1e-12;fe.value=[v,m,b],K.value=Math.sqrt(v*v+m*m+b*b),gt([v*1e9,m*1e9,b*1e9]);const Ce=Math.min(1,Math.abs(b*1e9)/5+.3);mt(Ce)});try{const n=await o.boot();Y.value=n.expectedWitnessHex,d("ok",`WASM module ready · nvsim@${n.buildVersion} · magic=0x${n.frameMagic.toString(16).toUpperCase()}`),d("info",`expected witness · ${n.expectedWitnessHex.slice(0,16)}…`),ct.value="(reference scene)",P.value="wasm"}catch(n){d("err",`boot failed: ${n.message}`)}try{const n=Y.value;if(n){const l=new Uint8Array(32);for(let m=0;m<32;m++)l[m]=parseInt(n.slice(m*2,m*2+2),16);const v=await o.verifyWitness(l);if(v.ok)A.value=n,d("ok","witness verified · determinism gate ✓");else{const m=Array.from(v.actual).map(b=>b.toString(16).padStart(2,"0")).join("");A.value=m,d("err",`WITNESS MISMATCH · expected ${n.slice(0,16)}… got ${m.slice(0,16)}…`)}}}catch(n){d("warn",`witness verify skipped: ${n.message}`)}})();
//# sourceMappingURL=index-CjC_xGBQ.js.map
