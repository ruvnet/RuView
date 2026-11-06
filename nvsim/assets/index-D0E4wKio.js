import{a as e,c as t,i as n,n as r,o as i,r as a,s as o,t as s}from"./lit-CxHpnxVN.js";import{n as c,r as l,t as ee}from"./signals-BHAHjpGc.js";(function(){let e=document.createElement(`link`).relList;if(e&&e.supports&&e.supports(`modulepreload`))return;for(let e of document.querySelectorAll(`link[rel="modulepreload"]`))n(e);new MutationObserver(e=>{for(let t of e)if(t.type===`childList`)for(let e of t.addedNodes)e.tagName===`LINK`&&e.rel===`modulepreload`&&n(e)}).observe(document,{childList:!0,subtree:!0});function t(e){let t={};return e.integrity&&(t.integrity=e.integrity),e.referrerPolicy&&(t.referrerPolicy=e.referrerPolicy),t.credentials=e.crossOrigin===`use-credentials`?`include`:e.crossOrigin===`anonymous`?`omit`:`same-origin`,t}function n(e){if(e.ep)return;e.ep=!0;let n=t(e);fetch(e.href,n)}})();function u(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a}var d=class extends e{constructor(...e){super(...e),this.view=`scene`}static{this.styles=t`
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
  `}navigate(e){this.dispatchEvent(new CustomEvent(`navigate`,{detail:e}))}render(){return i`
      <div class="logo" aria-hidden="true">NV</div>
      <nav role="navigation" aria-label="Primary"
        style="display:flex; flex-direction:column; align-items:center; gap:4px; flex:1;">
      <button class="btn ${this.view===`home`?`active`:``}"
        data-id="home-btn" title="Home" aria-label="Home"
        aria-current=${this.view===`home`?`page`:`false`}
        @click=${()=>this.navigate(`home`)}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 12L12 4l9 8M5 10v10h14V10"/></svg>
      </button>
      <button class="btn ${this.view===`scene`?`active`:``}"
        data-id="scene-btn" title="Scene" aria-label="Scene"
        aria-current=${this.view===`scene`?`page`:`false`}
        @click=${()=>this.navigate(`scene`)}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2L3 7l9 5 9-5-9-5zm0 13l-9-5v6l9 5 9-5v-6l-9 5z"/></svg>
      </button>
      <button class="btn ${this.view===`apps`?`active`:``}"
        data-id="apps-btn" title="App Store" aria-label="App Store"
        aria-current=${this.view===`apps`?`page`:`false`}
        @click=${()=>this.navigate(`apps`)}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
      </button>
      <button class="btn ${this.view===`inspector`?`active`:``}"
        data-id="inspector-btn" title="Inspector" aria-label="Inspector"
        aria-current=${this.view===`inspector`?`page`:`false`}
        @click=${()=>this.navigate(`inspector`)}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7"/><line x1="21" y1="21" x2="16.6" y2="16.6"/></svg>
      </button>
      <button class="btn ${this.view===`witness`?`active`:``}"
        data-id="witness-btn" title="Witness" aria-label="Witness"
        aria-current=${this.view===`witness`?`page`:`false`}
        @click=${()=>this.navigate(`witness`)}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 12l2 2 4-4M21 12c0 4.97-4.03 9-9 9s-9-4.03-9-9 4.03-9 9-9 9 4.03 9 9z"/></svg>
      </button>
      <button class="btn ghost ${this.view===`ghost-murmur`?`active`:``}"
        data-id="ghost-murmur-btn" title="Ghost Murmur — research spec"
        aria-label="Ghost Murmur research"
        aria-current=${this.view===`ghost-murmur`?`page`:`false`}
        @click=${()=>this.navigate(`ghost-murmur`)}>
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9 2C5.7 2 3 4.7 3 8v12l3-2 3 2 3-2 3 2 3-2 3 2V8c0-3.3-2.7-6-6-6H9z"/>
          <circle cx="9" cy="10" r="1.2" fill="currentColor"/>
          <circle cx="15" cy="10" r="1.2" fill="currentColor"/>
        </svg>
      </button>
      </nav>
      <div class="spacer"></div>
      <button class="btn" data-id="settings-btn" title="Settings" aria-label="Settings"
        @click=${()=>this.dispatchEvent(new CustomEvent(`open-settings`,{bubbles:!0,composed:!0}))}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06A1.65 1.65 0 0015 19.4a1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.6 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.6a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09A1.65 1.65 0 0015 4.6a1.65 1.65 0 001.82-.33l.06.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
      </button>
    `}};u([a()],d.prototype,`view`,void 0),d=u([n(`nv-rail`)],d);var f=l(`wasm`),p=l(``),m=l(!1),te=l(null),h=l(!1);l(!0);var g=l(1),ne=l(0),re=l(0n),_=l(3405691582n),v=l(1e4),y=l(1e3),b=l(1),x=l(!0),S=l(`dark`),C=l(`default`),w=l(!1),T=l(!0),E=l([0,0,0]),D=l(0),O=l(0),k=l(0),A=l(``),j=l(`idle`),M=l(``),N=l(null),P=l([]),ie=l([]),ae=l([]),oe=l([]),se=l(`rebar-walkby-01`),ce=l(``),le=l(!1),ue=l(`all`),F=l([]);function de(e){let t=F.value.slice();for(t.push(e);t.length>200;)t.shift();F.value=t}var I=l([]),fe=l([]),pe=l({});function me(e){let t=fe.value.slice();for(t.push(e);t.length>200;)t.shift();fe.value=t;let n={...pe.value};n[e.appId]=(n[e.appId]??0)+1,pe.value=n}var he=l(new Set),ge=ee(()=>f.value===`wasm`?`wasm`:`ws`),_e=null;function ve(e){_e=e}function L(){return _e}var R=l([]),ye=200;function z(e,t){if(le.value)return;let n=R.value.slice();for(n.push({ts:Date.now(),level:e,msg:t});n.length>ye;)n.shift();R.value=n}function be(e){let t=P.value.slice();t.push(e[0]),t.length>200&&t.shift();let n=ie.value.slice();n.push(e[1]),n.length>200&&n.shift();let r=ae.value.slice();r.push(e[2]),r.length>200&&r.shift(),P.value=t,ie.value=n,ae.value=r}function xe(e){let t=oe.value.slice();for(t.push(Math.max(0,Math.min(1,e)));t.length>48;)t.shift();oe.value=t}var B=class extends e{constructor(...e){super(...e),this.open=!1,this.mTitle=``,this.mBody=``,this.buttons=[],this.onModal=e=>{let t=e.detail;this.mTitle=t.title,this.mBody=t.body,this.buttons=t.buttons??[{label:`Close`,variant:`primary`}],this.open=!0,this.setAttribute(`open`,``),requestAnimationFrame(()=>{let e=this.shadowRoot;e&&e.querySelector(`input, select, textarea, button:not(.close)`)?.focus()})},this.onKey=e=>{e.key===`Escape`&&this.open&&this.close()}}static{this.styles=t`
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
  `}connectedCallback(){super.connectedCallback(),window.addEventListener(`nv-modal`,this.onModal),window.addEventListener(`keydown`,this.onKey)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener(`nv-modal`,this.onModal),window.removeEventListener(`keydown`,this.onKey)}updated(){if(!this.open)return;let e=this.shadowRoot;if(!e)return;let t=t=>{if(t.key!==`Tab`)return;let n=Array.from(e.querySelectorAll(`input, select, textarea, button, [href]`)).filter(e=>!e.hasAttribute(`disabled`));if(n.length===0)return;let r=n[0],i=n[n.length-1],a=e.activeElement??null;t.shiftKey&&a===r?(t.preventDefault(),i.focus()):!t.shiftKey&&a===i&&(t.preventDefault(),r.focus())};e.removeEventListener(`keydown`,t),e.addEventListener(`keydown`,t)}close(){this.open=!1,this.removeAttribute(`open`)}clickBtn(e){e.onClick?.(),this.close()}render(){return i`
      <div class="modal" role="dialog" aria-modal="true">
        <div class="h">
          <div class="ttl">${this.mTitle}</div>
          <button class="close" @click=${()=>this.close()}>×</button>
        </div>
        <div class="body" .innerHTML=${this.mBody}></div>
        <div class="f">
          ${this.buttons.map(e=>i`
            <button class=${e.variant??``} @click=${()=>this.clickBtn(e)}>${e.label}</button>
          `)}
        </div>
      </div>
    `}};u([r()],B.prototype,`open`,void 0),u([r()],B.prototype,`mTitle`,void 0),u([r()],B.prototype,`mBody`,void 0),u([r()],B.prototype,`buttons`,void 0),B=u([n(`nv-modal`)],B);function V(e){window.dispatchEvent(new CustomEvent(`nv-modal`,{detail:e}))}var H=class extends e{constructor(...e){super(...e),this.visible=!1,this.msg=``,this.icon=`✓`,this.timer=null,this.onToast=e=>{let t=e.detail;this.msg=t.msg??`Done`,this.icon=t.icon??`✓`,this.visible=!0,this.setAttribute(`visible`,``),this.timer!==null&&window.clearTimeout(this.timer),this.timer=window.setTimeout(()=>{this.visible=!1,this.removeAttribute(`visible`)},1800)}}static{this.styles=t`
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
  `}connectedCallback(){super.connectedCallback(),window.addEventListener(`nv-toast`,this.onToast)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener(`nv-toast`,this.onToast)}render(){return i`<span class="icon">${this.icon}</span><span>${this.msg}</span>`}};u([r()],H.prototype,`visible`,void 0),u([r()],H.prototype,`msg`,void 0),u([r()],H.prototype,`icon`,void 0),H=u([n(`nv-toast`)],H);function U(e,t=`✓`){window.dispatchEvent(new CustomEvent(`nv-toast`,{detail:{msg:e,icon:t}}))}var Se=class extends e{static{this.styles=t`
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
  `}connectedCallback(){super.connectedCallback(),c(()=>{k.value,ge.value,_.value,S.value,se.value,h.value,this.requestUpdate()})}async toggleRun(){let e=L();e&&(h.value?(await e.pause(),h.value=!1):(await e.run(),h.value=!0))}async reset(){let e=L();e&&await e.reset()}toggleTheme(){S.value=S.value===`dark`?`light`:`dark`}async openSeedModal(){V({title:`Set seed`,body:`<p>Set the 32-bit hex seed for the shot-noise PRNG. Same <code>(scene, config, seed)</code> → byte-identical witness.</p>
        <label>Hex seed</label>
        <input type="text" id="seed-input" value="${`0x${_.value.toString(16).toUpperCase().padStart(8,`0`)}`}" autofocus />`,buttons:[{label:`Cancel`,variant:`ghost`},{label:`Apply`,variant:`primary`,onClick:async()=>{let e=document.querySelector(`nv-modal`)?.shadowRoot?.querySelector(`#seed-input`);if(!e)return;let t=e.value.trim().replace(/^0x/i,``),n=BigInt(`0x`+t);_.value=n,await L()?.setSeed(n),z(`ok`,`seed → 0x${n.toString(16).toUpperCase()}`),U(`Seed → 0x${n.toString(16).toUpperCase().slice(0,8)}`,`⟳`)}}]})}openTransportSettings(){window.dispatchEvent(new CustomEvent(`open-settings`))}render(){let e=_.value.toString(16).toUpperCase().padStart(8,`0`);return i`
      <div class="crumbs">
        <span class="home">RuView</span><span class="sep">/</span>
        <span>nvsim</span><span class="sep">/</span>
        <span class="cur" id="scene-name">${se.value}</span>
      </div>
      <div class="spacer"></div>
      <span class="pill" id="fps-pill">
        <span class="dot"></span>
        <span id="fps-val">${k.value>0?(k.value/1e3).toFixed(2)+` kHz`:`idle`}</span>
      </span>
      <span class="pill wasm" id="transport-pill" title="Transport settings"
        @click=${this.openTransportSettings}>
        <span class="dot"></span>${ge.value}
      </span>
      <span class="pill seed" id="seed-pill" title="Set seed"
        @click=${this.openSeedModal}>
        seed: <b>0x${e}</b>
      </span>
      <button class="ghost" id="tour-btn" title="Replay the 10-step welcome tour"
        aria-label="Replay welcome tour"
        @click=${()=>window.dispatchEvent(new CustomEvent(`nv-show-tour`))}>
        ★ Tour
      </button>
      <button class="ghost" id="help-btn" title="Help (press ? any time)" aria-label="Open help"
        @click=${()=>window.dispatchEvent(new CustomEvent(`nv-show-help`))}>
        ?
      </button>
      <button class="ghost" id="theme-btn" title="Toggle theme" aria-label="Toggle theme"
        @click=${this.toggleTheme}>
        ${S.value===`dark`?`☼`:`☾`}
      </button>
      <button id="reset-btn" @click=${this.reset}>↺ Reset</button>
      <button class="primary" id="run-btn" @click=${this.toggleRun}>
        ${h.value?`❚❚ Pause`:`▶ Run`}
      </button>
    `}};Se=u([n(`nv-topbar`)],Se);var Ce=null;function we(){Ce!==null&&window.clearTimeout(Ce),Ce=window.setTimeout(async()=>{let e=L();if(e)try{await e.setConfig({digitiser:{f_s_hz:v.value,f_mod_hz:y.value},sensor:{gamma_fwhm_hz:1e6,t1_s:.005,t2_s:1e-6,t2_star_s:2e-7,contrast:.03,n_spins:0xe8d4a51000,shot_noise_disabled:!x.value},dt_s:b.value*.001}),z(`dbg`,`config pushed · fs=${v.value} f_mod=${y.value} dt=${b.value.toFixed(1)}ms noise=${x.value?`on`:`off`}`)}catch(e){z(`warn`,`config push failed: ${e.message}`)}},300)}var Te=class extends e{static{this.styles=t`
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
      margin-bottom: 6px;
    }
    .panel-help {
      font-size: 11.5px; color: var(--ink-3);
      margin: 0 0 10px;
      line-height: 1.5;
    }
    .help-link {
      color: var(--accent-2);
      cursor: pointer;
      text-decoration: underline dotted;
    }
    .help-link:hover { color: var(--accent); }
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
  `}connectedCallback(){super.connectedCallback(),c(()=>{v.value,y.value,b.value,x.value,h.value,this.requestUpdate()})}render(){return i`
      <div class="panel">
        <div class="panel-h">Scene <span class="count">4 sources</span></div>
        <div class="panel-help">
          Magnetic primitives in the simulated environment. Drag any in the
          canvas to reposition; positions persist across reloads.
        </div>
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
        <div class="panel-help">
          Element Six DNV-B1 reference: 1 mm³ diamond, ~10¹² NV centers.
          Floor δB ≈ 1.18 pT/√Hz per Barry 2020 §III.A.
          <span class="help-link" title="Open glossary"
            @click=${()=>window.dispatchEvent(new CustomEvent(`nv-show-help`,{detail:{section:`glossary`}}))}>What's NV?</span>
        </div>
        <div class="field-row" title="Sensing volume (cubic millimetres)"><span class="lbl">V</span><span class="val">1 mm³</span></div>
        <div class="field-row" title="Number of NV centers contributing to readout"><span class="lbl">N</span><span class="val">1e12 NV</span></div>
        <div class="field-row" title="ODMR contrast — fractional dip at resonance"><span class="lbl">C</span><span class="val">0.030</span></div>
        <div class="field-row" title="Inhomogeneous dephasing time T₂*"><span class="lbl">T₂*</span><span class="val">200 ns</span></div>
        <div class="field-row" title="Shot-noise-limited field sensitivity"><span class="lbl">δB</span><span class="val">1.18 pT/√Hz</span></div>
      </div>

      <div class="panel">
        <div class="panel-h">Tunables</div>
        <div class="panel-help">
          Live pipeline parameters. Edits debounce 300 ms then rebuild the
          WASM pipeline without restarting the frame stream.
        </div>
        <div class="slider-row" title="Digitiser sample rate — frames per second emitted by the pipeline">
          <div class="top"><span class="lbl">Sample rate</span><span class="val">${(v.value/1e3).toFixed(1)} kHz</span></div>
          <input type="range" min="1000" max="100000" .value=${String(v.value)}
            aria-label="Sample rate in Hz"
            @input=${e=>{v.value=+e.target.value,we()}} />
        </div>
        <div class="slider-row" title="Microwave modulation frequency for lock-in demodulation">
          <div class="top"><span class="lbl">Lockin f_mod</span><span class="val">${(y.value/1e3).toFixed(3)} kHz</span></div>
          <input type="range" min="100" max="5000" .value=${String(y.value)}
            aria-label="Lock-in modulation frequency in Hz"
            @input=${e=>{y.value=+e.target.value,we()}} />
        </div>
        <div class="slider-row" title="Per-sample integration time">
          <div class="top"><span class="lbl">Integration t</span><span class="val">${b.value.toFixed(1)} ms</span></div>
          <input type="range" min="0.1" max="10" step="0.1" .value=${String(b.value)}
            aria-label="Integration time in milliseconds"
            @input=${e=>{b.value=+e.target.value,we()}} />
        </div>
        <div class="slider-row" title="Toggle shot-noise sampling. OFF = analytic noise-free output (debug only)">
          <div class="top"><span class="lbl">Shot noise</span><span class="val">${x.value?`ON`:`OFF`}</span></div>
          <input type="range" min="0" max="1" .value=${x.value?`1`:`0`}
            aria-label="Shot-noise sampling enabled"
            @input=${e=>{x.value=e.target.value===`1`,we()}} />
        </div>
      </div>

      <div class="panel">
        <div class="panel-h">Pipeline</div>
        <div class="panel-help">
          Forward simulator stages, left to right. Stages glow cyan while
          the pipeline is running.
        </div>
        <div class="pipeline">
          <span class="stage ${h.value?`live`:``}">scene</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${h.value?`live`:``}">B-S</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${h.value?`live`:``}">prop</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${h.value?`live`:``}">NV</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${h.value?`live`:``}">ADC</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${h.value?`live`:``}">frame</span>
        </div>
      </div>
    `}};Te=u([n(`nv-sidebar`)],Te);var W=class extends e{constructor(...e){super(...e),this.zoom=1,this.layerVisible={source:!0,field:!0,label:!0},this.items=[{id:`rebar`,x:740,y:240,color:`oklch(0.72 0.18 330)`,name:`rebar.steel`},{id:`heart`,x:220,y:180,color:`oklch(0.78 0.14 195)`,name:`heart_proxy`},{id:`mains`,x:180,y:380,color:`oklch(0.72 0.18 330)`,name:`mains_60Hz`},{id:`door`,x:800,y:470,color:`oklch(0.78 0.14 145)`,name:`door.steel`}],this.dragging=null,this.selected=null,this.dragOffset={dx:0,dy:0},this.onKey=e=>{let t=e.target;if(!t||t.tagName!==`INPUT`&&t.tagName!==`TEXTAREA`){if(!this.selected)e.key===`Tab`&&document.activeElement===document.body&&(e.preventDefault(),this.selected=this.items[0]?.id??null);else if(e.key===`ArrowLeft`||e.key===`ArrowRight`||e.key===`ArrowUp`||e.key===`ArrowDown`){e.preventDefault();let t=e.shiftKey?32:8,n=e.key===`ArrowLeft`?-t:e.key===`ArrowRight`?t:0,r=e.key===`ArrowUp`?-t:e.key===`ArrowDown`?t:0;this.items=this.items.map(e=>e.id===this.selected?{...e,x:Math.max(20,Math.min(980,e.x+n)),y:Math.max(20,Math.min(580,e.y+r))}:e),I.value=this.items.map(({id:e,x:t,y:n})=>({id:e,x:t,y:n}))}else if(e.key===`Tab`){e.preventDefault();let t=(this.items.findIndex(e=>e.id===this.selected)+(e.shiftKey?-1:1)+this.items.length)%this.items.length;this.selected=this.items[t].id}else e.key===`Escape`&&(this.selected=null)}},this.onDown=(e,t)=>{t.preventDefault(),this.dragging=e,this.selected=e;let n=this.items.find(t=>t.id===e);if(!n)return;let r=this.renderRoot.querySelector(`svg`);if(!r)return;let i=this.toSvg(t,r);this.dragOffset={dx:i.x-n.x,dy:i.y-n.y}},this.onPointerMove=e=>{if(!this.dragging)return;let t=this.renderRoot.querySelector(`svg`);if(!t)return;let n=this.toSvg(e,t);this.items=this.items.map(e=>e.id===this.dragging?{...e,x:n.x-this.dragOffset.dx,y:n.y-this.dragOffset.dy}:e)},this.onPointerUp=()=>{this.dragging&&(I.value=this.items.map(({id:e,x:t,y:n})=>({id:e,x:t,y:n}))),this.dragging=null}}static{this.styles=t`
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
  `}connectedCallback(){super.connectedCallback(),I.value.length>0&&(this.items=this.items.map(e=>{let t=I.value.find(t=>t.id===e.id);return t?{...e,x:t.x,y:t.y}:e})),c(()=>{E.value,D.value,k.value,O.value,w.value,h.value,g.value,N.value,this.requestUpdate()}),c(()=>{let e=N.value;if(!e)return;let t=Math.sqrt(e.bPt[0]**2+e.bPt[1]**2+e.bPt[2]**2)/Math.max(Math.abs(e.sigmaPt[0]),Math.abs(e.sigmaPt[1]),Math.abs(e.sigmaPt[2]),.001);Number.isFinite(t)&&(O.value=t)}),window.addEventListener(`pointermove`,this.onPointerMove),window.addEventListener(`pointerup`,this.onPointerUp),window.addEventListener(`keydown`,this.onKey)}async toggleRun(){let e=L();e&&(h.value?(await e.pause(),h.value=!1):(await e.run(),h.value=!0))}async stepFwd(){let e=L();e&&(await e.step(`fwd`,10),z(`dbg`,`sim step → +1 frame`))}async stepBack(){let e=L();e&&(await e.step(`back`,10),z(`dbg`,`sim step ← -1 frame`))}cycleSpeed(){let e=[.25,.5,1,2,4];g.value=e[(e.indexOf(g.value)+1)%e.length]}zoomIn(){this.zoom=Math.min(2.5,this.zoom*1.2)}zoomOut(){this.zoom=Math.max(.5,this.zoom/1.2)}fitView(){this.zoom=1}toggleLayer(e){this.layerVisible={...this.layerVisible,[e]:!this.layerVisible[e]}}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener(`pointermove`,this.onPointerMove),window.removeEventListener(`pointerup`,this.onPointerUp),window.removeEventListener(`keydown`,this.onKey)}toSvg(e,t){let n=t.getBoundingClientRect();return{x:(e.clientX-n.left)/n.width*1e3,y:(e.clientY-n.top)/n.height*600}}render(){let e=E.value,t=[e[0]*1e9,e[1]*1e9,e[2]*1e9],n=D.value*1e9,r=w.value?``:`anim`,a=1e3/this.zoom,s=600/this.zoom,c=(1e3-a)/2,l=(600-s)/2;return i`
      <div class="grid"></div>
      <svg viewBox="${c.toFixed(1)} ${l.toFixed(1)} ${a.toFixed(1)} ${s.toFixed(1)}"
        preserveAspectRatio="xMidYMid meet" id="scene-svg">
        <defs>
          <radialGradient id="g-sensor" cx="50%" cy="50%" r="50%">
            <stop offset="0" stop-color="oklch(0.78 0.14 70)" stop-opacity="0.4"/>
            <stop offset="1" stop-color="oklch(0.78 0.14 70)" stop-opacity="0"/>
          </radialGradient>
          <filter id="glow"><feGaussianBlur stdDeviation="3" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
        </defs>

        <!-- Field lines from each source to sensor -->
        ${this.layerVisible.field?this.items.map(e=>o`
          <line class="field-line ${r}" x1=${e.x} y1=${e.y}
            x2="500" y2="320"
            stroke=${e.color} stroke-width="1" stroke-opacity="0.5"/>
        `):``}

        <!-- Source primitives -->
        ${this.layerVisible.source?this.items.map(e=>o`
          <g class=${`draggable ${this.dragging===e.id?`dragging`:``} ${this.selected===e.id?`selected`:``}`}
             data-id=${e.id} data-source-id=${e.id}
             transform=${`translate(${e.x.toFixed(0)},${e.y.toFixed(0)})`}
             @pointerdown=${t=>this.onDown(e.id,t)}>
            <ellipse cx="0" cy="0" rx="32" ry="22" fill=${e.color} fill-opacity="0.18"
              stroke=${e.color} stroke-width="1.2"/>
            <circle cx="0" cy="0" r="4" fill=${e.color}/>
            ${this.layerVisible.label?o`<text class="label" x="0" y="40" text-anchor="middle">${e.name}</text>`:``}
          </g>
        `):``}

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
        <button id="layer-source-btn" class=${this.layerVisible.source?`on`:``}
          title="Sources" @click=${()=>this.toggleLayer(`source`)}>●</button>
        <button id="layer-field-btn" class=${this.layerVisible.field?`on`:``}
          title="Field lines" @click=${()=>this.toggleLayer(`field`)}>≈</button>
        <button id="layer-label-btn" class=${this.layerVisible.label?`on`:``}
          title="Labels" @click=${()=>this.toggleLayer(`label`)}>T</button>
      </div>

      <div class="sim-controls" id="sim-controls">
        <button class="step" id="step-back-btn" title="Step back" @click=${this.stepBack}>⏮</button>
        <button class="play" id="play-btn" title="Play / pause" @click=${this.toggleRun}>
          ${h.value?`❚❚`:`▶`}
        </button>
        <button class="step" id="step-fwd-btn" title="Step forward" @click=${this.stepFwd}>⏭</button>
        <span class="speed" id="speed-val" title="Cycle speed" @click=${this.cycleSpeed}>${g.value}×</span>
      </div>

      <div class="scene-readout">
        <div class="stat-card">
          <div class="lbl">|B|</div>
          <div class="val amber" id="bmag-readout">${n.toFixed(3)} nT</div>
        </div>
        <div class="stat-card">
          <div class="lbl">FPS</div>
          <div class="val cyan" id="fps-readout">${k.value>0?Math.round(k.value):`—`}</div>
        </div>
        <div class="stat-card">
          <div class="lbl">SNR</div>
          <div class="val mint" id="snr-readout">${O.value>0?O.value.toFixed(1):`—`}</div>
        </div>
      </div>
    `}};u([r()],W.prototype,`zoom`,void 0),u([r()],W.prototype,`layerVisible`,void 0),u([r()],W.prototype,`items`,void 0),u([r()],W.prototype,`dragging`,void 0),u([r()],W.prototype,`selected`,void 0),W=u([n(`nv-scene`)],W);var Ee=class extends e{constructor(...e){super(...e),this.tab=`signal`,this.pinTab=null,this.expanded=!1}static{this.styles=t`
    :host {
      display: flex; flex-direction: column;
      background: var(--bg-1);
      border-left: 1px solid var(--line);
      overflow: hidden;
      height: 100%;
    }
    :host([expanded]) {
      border-left: 0;
      background: radial-gradient(ellipse at 50% 30%, var(--bg-2) 0%, var(--bg-0) 70%);
    }
    :host([expanded]) .tabs {
      padding: 0 24px;
      background: var(--bg-1);
    }
    :host([expanded]) .tab {
      padding: 16px 22px;
      font-size: 13.5px;
      flex: 0 0 auto;
    }
    :host([expanded]) .body {
      padding: 24px 28px;
      max-width: 1400px;
      width: 100%;
      margin: 0 auto;
    }
    :host([expanded]) .card { padding: 18px 20px; }
    :host([expanded]) .card-h .ttl { font-size: 14px; }
    :host([expanded]) svg { height: 220px; }
    :host([expanded]) .frame-strip { height: 48px; }
    :host([expanded]) table { font-size: 12.5px; }
    :host([expanded]) td { padding: 6px 0; }
    :host([expanded]) .hex { font-size: 12px; padding: 14px; line-height: 1.7; }
    :host([expanded]) .witness-box { font-size: 13px; padding: 14px 16px; line-height: 1.6; }
    :host([expanded]) .verify-btn { padding: 12px; font-size: 13px; }
    :host([expanded]) .grid-2 {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
    }
    :host([expanded]) .grid-2 > .card { margin-bottom: 0; }
    @media (max-width: 1024px) {
      :host([expanded]) .grid-2 { grid-template-columns: 1fr; }
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
  `}connectedCallback(){super.connectedCallback(),c(()=>{P.value,ie.value,ae.value,oe.value,N.value,A.value,j.value,E.value,D.value,this.requestUpdate()})}willUpdate(e){e.has(`pinTab`)&&this.pinTab&&this.tab!==this.pinTab&&(this.tab=this.pinTab)}async verify(){let e=L();if(e){j.value=`pending`,z(`info`,`verifying witness over 256 frames…`);try{let t=M.value,n=new Uint8Array(32);for(let e=0;e<32;e++)n[e]=parseInt(t.slice(e*2,e*2+2),16);let r=await e.verifyWitness(n);if(r.ok)j.value=`ok`,A.value=t,z(`ok`,`witness ${t.slice(0,16)}… matches · determinism gate ✓`);else{j.value=`fail`;let e=Array.from(r.actual).map(e=>e.toString(16).padStart(2,`0`)).join(``);A.value=e,z(`err`,`WITNESS MISMATCH actual=${e.slice(0,16)}…`)}}catch(e){j.value=`fail`,z(`err`,`verify failed: ${e.message}`)}}}renderHeader(){return this.expanded?i`
      <h1 style="margin: 8px 0 14px; font-size: 20px; letter-spacing: -0.01em;">
        ${{signal:`Signal inspector — live B-vector trace + frame stream`,frame:`Frame inspector — MagFrame v1 fields + raw bytes`,witness:`Witness panel — SHA-256 determinism gate`}[this.tab]}
      </h1>
      <p style="margin: 0 0 18px; font-size: 12.5px; color: var(--ink-3); line-height: 1.55; max-width: 780px;">
        ${this.tab===`signal`?`Real-time recovered field-vector and frame-stream sparkline. Both update at the running pipeline's frame rate. Use the Tunables panel in the sidebar to change f_s, f_mod, dt, and shot-noise behaviour.`:this.tab===`frame`?`Decoded view of the most recent MagFrame: typed fields plus the raw 60-byte little-endian binary record (magic 0xC51A_6E70).`:`Re-derive the SHA-256 witness for the canonical reference scene (seed=42, N=256) right now in your browser and compare against Proof::EXPECTED_WITNESS_HEX. Same inputs → same hash, byte-for-byte, across every machine and transport.`}
      </p>
    `:``}renderSignalTab(){let e=e=>{let t=``;return e.forEach((e,n)=>{let r=n/199*320,i=65-e*22;t+=(n===0?`M`:`L`)+` ${r.toFixed(1)} ${i.toFixed(1)} `}),t},t=E.value,n=[t[0]*1e9,t[1]*1e9,t[2]*1e9],r=P.value.length>0;return i`
      ${r?``:i`
        <div class="card" style="text-align:center; padding:18px;">
          <div style="font-size:13px; color:var(--ink-2); line-height:1.55;">
            No frames yet. Press <b>▶ Run</b> in the topbar (or hit <code style="font-family:var(--mono);background:var(--bg-3);padding:1px 5px;border-radius:4px;color:var(--accent);">Space</code>)
            to start the live B-vector trace.
          </div>
        </div>
      `}
      <div class=${this.expanded?`grid-2`:``}>
        <div class="card">
          <div class="card-h">
            <span class="ttl">B-vector trace</span>
            <span class="badge">3-axis · nT</span>
          </div>
          <svg viewBox="0 0 ${320} ${130}" preserveAspectRatio="none">
            <line x1="0" y1=${65} x2=${320} y2=${65} stroke="var(--line)" stroke-width="0.5"/>
            ${o`<path id="trace-x" d=${e(P.value)} stroke="oklch(0.78 0.14 70)" stroke-width="1.2" fill="none"/>`}
            ${o`<path id="trace-y" d=${e(ie.value)} stroke="oklch(0.78 0.12 195)" stroke-width="1.2" fill="none" opacity="0.8"/>`}
            ${o`<path id="trace-z" d=${e(ae.value)} stroke="oklch(0.72 0.18 330)" stroke-width="1.2" fill="none" opacity="0.7"/>`}
          </svg>
          ${this.expanded?i`<div style="display:flex;gap:14px;font-size:12px;font-family:var(--mono);margin-top:8px;">
            <span style="color:oklch(0.78 0.14 70);">x: ${n[0].toFixed(3)} nT</span>
            <span style="color:oklch(0.78 0.12 195);">y: ${n[1].toFixed(3)} nT</span>
            <span style="color:oklch(0.72 0.18 330);">z: ${n[2].toFixed(3)} nT</span>
            <span style="color:var(--accent);margin-left:auto;">|B| ${(D.value*1e9).toFixed(3)} nT</span>
          </div>`:``}
        </div>

        <div class="card">
          <div class="card-h">
            <span class="ttl">Frame stream</span>
            <span class="badge" id="strip-rate">live</span>
          </div>
          <div class="frame-strip" id="frame-strip">
            ${oe.value.map(e=>i`<div class="bar" style=${`height:${Math.max(4,e*100)}%`}></div>`)}
          </div>
          ${this.expanded?i`
            <div style="display:flex;gap:24px;font-family:var(--mono);font-size:12px;color:var(--ink-3);margin-top:12px;">
              <span>frames in window: <span style="color:var(--ink);">${oe.value.length}</span></span>
              <span>noise floor: <span style="color:var(--ink);">${N.value?N.value.noiseFloorPtSqrtHz.toFixed(2)+` pT/√Hz`:`—`}</span></span>
            </div>`:``}
        </div>
      </div>
    `}renderFrameTab(){let e=N.value,t=e?.raw,n=``;return t&&(n=Array.from(t).map(e=>e.toString(16).padStart(2,`0`)).slice(0,60).join(` `)),i`
      ${e?``:i`
        <div class="card" style="text-align:center; padding:18px;">
          <div style="font-size:13px; color:var(--ink-2); line-height:1.55;">
            No MagFrame to display yet. Start the pipeline (<b>▶ Run</b>) to populate.
          </div>
        </div>
      `}
      <div class=${this.expanded?`grid-2`:``}>
      <div class="card">
        <div class="card-h">
          <span class="ttl">MagFrame v1 fields</span>
          <span class="badge">60 B</span>
        </div>
        <table>
          <tr><td>magic</td><td id="frame-magic">${e?`0x`+e.magic.toString(16).toUpperCase():`—`}</td></tr>
          <tr><td>version</td><td>${e?.version??`—`}</td></tr>
          <tr><td>flags</td><td>0x${(e?.flags??0).toString(16).padStart(4,`0`)}</td></tr>
          <tr><td>sensor_id</td><td>${e?.sensorId??`—`}</td></tr>
          <tr><td>t_us</td><td>${e?e.tUs.toString():`—`}</td></tr>
          <tr><td>b_pT[0]</td><td id="frame-bx">${e?e.bPt[0].toFixed(1):`—`}</td></tr>
          <tr><td>b_pT[1]</td><td id="frame-by">${e?e.bPt[1].toFixed(1):`—`}</td></tr>
          <tr><td>b_pT[2]</td><td id="frame-bz">${e?e.bPt[2].toFixed(1):`—`}</td></tr>
          <tr><td>noise_floor</td><td>${e?e.noiseFloorPtSqrtHz.toFixed(2):`—`}</td></tr>
          <tr><td>temp_K</td><td>${e?e.temperatureK.toFixed(1):`—`}</td></tr>
        </table>
      </div>
      <div class="card">
        <div class="card-h">
          <span class="ttl">Hex dump</span>
          <span class="badge">LE</span>
        </div>
        <div class="hex" id="frame-hex">${n||`—`}</div>
        ${this.expanded?i`
          <div style="font-size: 11.5px; color: var(--ink-3); margin-top: 10px; line-height: 1.6;">
            Layout (little-endian): <code>magic(u32) version(u16) flags(u16) sensor_id(u16) _reserved(u16) t_us(u64) b_pt[3](f32) sigma_pt[3](f32) noise_floor(f32) temp_K(f32)</code>.
          </div>`:``}
      </div>
      </div>
    `}renderWitnessTab(){let e=j.value,t=e===`ok`?`ok`:e===`fail`?`fail`:``,n=e===`pending`?`Verifying…`:e===`ok`?`✓ Witness verified · determinism gate`:e===`fail`?`✗ Witness mismatch · audit required`:`Verify witness`,r=M.value&&A.value&&M.value===A.value;return i`
      ${this.expanded?i`
        <div style="display:grid;grid-template-columns:repeat(auto-fit, minmax(180px, 1fr));gap:12px;margin-bottom:18px;">
          <div class="card" style="margin:0;">
            <div style="font-size:10px;color:var(--ink-3);text-transform:uppercase;letter-spacing:0.06em;">Reference scene</div>
            <div style="font-family:var(--mono);font-size:14px;color:var(--ink);margin-top:4px;">Proof::REFERENCE</div>
            <div style="font-size:11.5px;color:var(--ink-3);margin-top:2px;">2 dipoles · 1 loop · 1 ferrous · 1 sensor</div>
          </div>
          <div class="card" style="margin:0;">
            <div style="font-size:10px;color:var(--ink-3);text-transform:uppercase;letter-spacing:0.06em;">Seed</div>
            <div style="font-family:var(--mono);font-size:14px;color:var(--accent);margin-top:4px;">0x0000002A</div>
            <div style="font-size:11.5px;color:var(--ink-3);margin-top:2px;">canonical Proof::SEED</div>
          </div>
          <div class="card" style="margin:0;">
            <div style="font-size:10px;color:var(--ink-3);text-transform:uppercase;letter-spacing:0.06em;">Sample count</div>
            <div style="font-family:var(--mono);font-size:14px;color:var(--ink);margin-top:4px;">256</div>
            <div style="font-size:11.5px;color:var(--ink-3);margin-top:2px;">Proof::N_SAMPLES</div>
          </div>
          <div class="card" style="margin:0;">
            <div style="font-size:10px;color:var(--ink-3);text-transform:uppercase;letter-spacing:0.06em;">Status</div>
            <div style="font-family:var(--mono);font-size:14px;margin-top:4px;color:${e===`ok`?`var(--ok)`:e===`fail`?`var(--bad)`:`var(--ink-3)`};">
              ${e===`ok`?`✓ matches`:e===`fail`?`✗ drift`:e===`pending`?`… running`:`— idle`}
            </div>
            <div style="font-size:11.5px;color:var(--ink-3);margin-top:2px;">${r?`byte-equivalent`:`not yet verified`}</div>
          </div>
        </div>
      `:``}
      <div class="card">
        <div class="card-h">
          <span class="ttl">Expected (Proof::EXPECTED_WITNESS_HEX)</span>
          <span class="badge">SHA-256</span>
        </div>
        <div class="witness-box" id="expected-witness">${M.value||`(loading…)`}</div>
      </div>
      <div class="card">
        <div class="card-h">
          <span class="ttl">Actual (last verify)</span>
          <span class="badge">SHA-256</span>
        </div>
        <div class="witness-box" id="actual-witness">${A.value||`(not verified yet)`}</div>
        <button class="verify-btn ${t}" id="verify-btn" @click=${this.verify}>${n}</button>
      </div>
      ${this.expanded?i`
        <div class="card">
          <div class="card-h">
            <span class="ttl">What this verifies</span>
            <span class="badge">ADR-089 §5</span>
          </div>
          <div style="font-size: 12.5px; color: var(--ink-2); line-height: 1.6;">
            <p style="margin: 0 0 10px;">Pressing <b>Verify</b> runs the canonical reference pipeline
              (<code>Proof::generate</code>) end-to-end inside this browser's WASM Worker:
              scene → Biot-Savart synthesis → material attenuation → NV ensemble → ADC + lock-in →
              concatenated <code>MagFrame</code> bytes → SHA-256.</p>
            <p style="margin: 0 0 10px;">If the resulting hash matches the constant pinned at build time
              (<code>cc8de9b01b0ff5bd…</code>), every constant — γ_e, D_GS, μ₀, T₂*, contrast, the PRNG
              stream, the frame layout, the pipeline ordering — is byte-identical to the published
              reference. If it doesn't match, <i>something</i> drifted; the dashboard names which.</p>
            <p style="margin: 0;">This is the same regression test that runs in
              <code>cargo test -p nvsim</code> — running in your browser, against your own WASM build.</p>
          </div>
        </div>
      `:``}
    `}render(){return i`
      <div class="tabs" role="tablist">
        <button class="tab ${this.tab===`signal`?`active`:``}" data-pane="signal"
          role="tab" aria-selected=${this.tab===`signal`}
          @click=${()=>this.tab=`signal`}>Signal</button>
        <button class="tab ${this.tab===`frame`?`active`:``}" data-pane="frame"
          role="tab" aria-selected=${this.tab===`frame`}
          @click=${()=>this.tab=`frame`}>Frame</button>
        <button class="tab ${this.tab===`witness`?`active`:``}" data-pane="witness"
          role="tab" aria-selected=${this.tab===`witness`}
          @click=${()=>this.tab=`witness`}>Witness</button>
      </div>
      <div class="body" role="tabpanel" tabindex="0">
        ${this.renderHeader()}
        ${this.tab===`signal`?this.renderSignalTab():this.tab===`frame`?this.renderFrameTab():this.renderWitnessTab()}
      </div>
    `}};u([r()],Ee.prototype,`tab`,void 0),u([a({attribute:!1})],Ee.prototype,`pinTab`,void 0),u([a({type:Boolean,reflect:!0})],Ee.prototype,`expanded`,void 0),Ee=u([n(`nv-inspector`)],Ee);var De=class extends e{constructor(...e){super(...e),this.hIdx=-1,this.onKey=e=>{if(e.key===`Enter`)this.exec(this.inputEl.value),this.inputEl.value=``;else if(e.key===`ArrowUp`){let t=F.value;t.length&&(this.hIdx=Math.max(0,this.hIdx-1),this.inputEl.value=t[this.hIdx]??``,e.preventDefault())}else if(e.key===`ArrowDown`){let t=F.value;t.length&&(this.hIdx=Math.min(t.length,this.hIdx+1),this.inputEl.value=t[this.hIdx]??``,e.preventDefault())}}}static{this.styles=t`
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
    .ts { color: var(--ink-3); font-size: 10.5px; padding-top: 1px; }
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
  `}connectedCallback(){super.connectedCallback(),c(()=>{R.value,ue.value,le.value,this.requestUpdate()})}updated(){let e=this.renderRoot.querySelector(`.body`);e&&(e.scrollTop=e.scrollHeight)}counts(){let e={info:0,warn:0,err:0,dbg:0,ok:0};for(let t of R.value)e[t.level]=(e[t.level]??0)+1;return e.all=R.value.length,e}async exec(e){if(e=e.trim(),!e)return;z(`info`,`<span style="color:var(--accent);">nvsim&gt;</span> ${e}`),de(e),this.hIdx=F.value.length;let[t,...n]=e.split(/\s+/),r=n.join(` `),i=L();switch(t){case`help`:z(`info`,`commands: help · scene.list · sensor.config · run · pause · reset · seed · proof.verify · proof.export · clear · theme · status`);break;case`scene.list`:z(`info`,`scene rebar-walkby-01:`),z(`info`,`  rebar.steel.coil   @ [+2.7, 0.0, +0.3] m χ=5000`),z(`info`,`  dipole.heart_proxy @ [-1.4, +0.2, +0.4] m m=1.0e-6 A·m²`),z(`info`,`  loop.mains_60Hz    @ [-1.6, -0.4, 0.0] m I=2 A`),z(`info`,`  eddy.door_steel    @ [+0.0, +1.8, +0.4] m σ=1e6 S/m`);break;case`sensor.config`:z(`info`,`NvSensor::cots_defaults() {`),z(`info`,`  pos=[0,0,0], V=1mm³, N=1e12, C=0.03, T2*=200ns`),z(`info`,`  D=2.870 GHz, γe=28 GHz/T, Γ=1.0 MHz, axes=4×〈111〉`),z(`info`,`  δB ≈ 1.18 pT/√Hz (Barry 2020 §III.A) }`);break;case`run`:i&&(await i.run(),h.value=!0,z(`ok`,`pipeline RUN`));break;case`pause`:i&&(await i.pause(),h.value=!1,z(`warn`,`pipeline PAUSED`));break;case`reset`:i&&(await i.reset(),z(`info`,`pipeline reset · t=0`));break;case`seed`:{if(!r){z(`info`,`current seed = 0x${_.value.toString(16).toUpperCase()}`);break}let e=BigInt(r.startsWith(`0x`)?r:`0x`+r);_.value=e,i&&await i.setSeed(e),z(`ok`,`seed → 0x${e.toString(16).toUpperCase()}`);break}case`proof.verify`:if(!i)break;z(`dbg`,`computing SHA-256 over 256 frames…`);try{let e=M.value,t=new Uint8Array(32);for(let n=0;n<32;n++)t[n]=parseInt(e.slice(n*2,n*2+2),16);(await i.verifyWitness(t)).ok?(j.value=`ok`,A.value=e,z(`ok`,`witness ${e.slice(0,16)}… matches · determinism gate ✓`)):(j.value=`fail`,z(`err`,`WITNESS MISMATCH`))}catch(e){z(`err`,`verify failed: ${e.message}`)}break;case`proof.export`:if(!i)break;z(`dbg`,`building proof bundle…`);try{let e=await i.exportProofBundle(),t=URL.createObjectURL(e),n=document.createElement(`a`);n.href=t,n.download=`nvsim-proof-${Date.now()}.json`,n.click(),URL.revokeObjectURL(t),z(`ok`,`proof bundle exported · ${e.size} bytes`)}catch(e){z(`err`,`export failed: ${e.message}`)}break;case`clear`:R.value=[];break;case`theme`:{let e=(r||``).toLowerCase();e===`light`||e===`dark`?(S.value=e,z(`ok`,`theme → ${e}`)):z(`info`,`theme [light|dark]`);break}case`status`:z(`info`,`running=${h.value} seed=0x${_.value.toString(16).toUpperCase()} verified=${j.value}`);break;default:z(`err`,`unknown command: ${t} · try help`)}}render(){let e=this.counts(),t=ue.value,n=R.value.filter(e=>t===`all`||e.level===t);return i`
      <div class="tabs">
        ${[`all`,`info`,`warn`,`err`,`dbg`].map(n=>i`
          <button class="tab ${t===n?`active`:``}" data-tab=${n}
            @click=${()=>ue.value=n}>
            ${n} <span class="cnt">${e[n]??0}</span>
          </button>
        `)}
        <span class="spacer"></span>
        <div class="tools">
          <button id="clear-log" title="Clear" @click=${()=>R.value=[]}>×</button>
          <button id="pause-log" title="Pause" @click=${()=>le.value=!le.value}>
            ${le.value?`▶`:`❚❚`}
          </button>
        </div>
      </div>
      <div class="body" role="log" aria-live="polite" aria-label="Console output">
        ${n.map(e=>{let t=new Date(e.ts),n=`${String(t.getSeconds()).padStart(2,`0`)}.${String(t.getMilliseconds()).padStart(3,`0`)}`;return i`<div class="line ${e.level}">
            <div class="ts">${n}</div>
            <div class="lvl">${e.level}</div>
            <div class="msg" .innerHTML=${e.msg}></div>
          </div>`})}
      </div>
      <div class="input">
        <span class="prompt">nvsim&gt;</span>
        <input id="console-input" type="text"
          placeholder="help · scene.list · sensor.config · run · proof.verify · clear"
          @keydown=${this.onKey}/>
      </div>
    `}};u([s(`#console-input`)],De.prototype,`inputEl`,void 0),De=u([n(`nv-console`)],De);var Oe=[{id:`nvsim`,name:`nvsim — NV-diamond magnetometer`,category:`sim`,crate:`nvsim`,summary:`Deterministic forward simulator: scene → Biot–Savart → NV ensemble → ADC → MagFrame stream + SHA-256 witness.`,budget:`L`,active:!0,status:`available`,tags:[`quantum`,`magnetometer`,`simulator`,`witness`,`wasm`],adr:`ADR-089`,runtime:`running`},{id:`gesture`,name:`Gesture (DTW)`,category:`sig`,crate:`wifi-densepose-wasm-edge`,summary:`Dynamic-Time-Warping gesture classifier from CSI motion templates.`,events:[1],budget:`M`,status:`available`,tags:[`hci`,`csi`,`classifier`,`dtw`],adr:`ADR-014`,runtime:`mesh-only`},{id:`coherence`,name:`Coherence gate`,category:`sig`,crate:`wifi-densepose-wasm-edge`,summary:`Z-score coherence scoring + Accept/PredictOnly/Reject/Recalibrate gate.`,events:[2],budget:`S`,status:`available`,tags:[`gate`,`csi`,`coherence`,`drift`],adr:`ADR-029`,runtime:`simulated`},{id:`adversarial`,name:`Adversarial-signal detector`,category:`ais`,crate:`wifi-densepose-wasm-edge`,summary:`Physically-impossible-signal detector — multi-link consistency, used to flag spoofed CSI.`,events:[3],budget:`M`,status:`available`,tags:[`security`,`csi`,`spoofing`,`mesh`],adr:`ADR-032`,runtime:`simulated`},{id:`rvf`,name:`RVF — Rust Verified Feature stream`,category:`sig`,crate:`wifi-densepose-wasm-edge`,summary:`Verified-frame builder with SHA-256 hash + version metadata for the feature stream.`,budget:`S`,status:`available`,tags:[`witness`,`csi`,`hash`],adr:`ADR-040`},{id:`occupancy`,name:`Occupancy estimator`,category:`bld`,crate:`wifi-densepose-wasm-edge`,summary:`Through-wall presence + person-count via CSI amplitude perturbation.`,events:[300,301,302],budget:`S`,status:`available`,tags:[`csi`,`building`,`presence`],runtime:`simulated`},{id:`vital_trend`,name:`Vital-trend monitor`,category:`med`,crate:`wifi-densepose-wasm-edge`,summary:`HR + BR trend tracking with bradycardia/tachycardia/apnea events.`,events:[100,101,102,103,104,105],budget:`S`,status:`available`,tags:[`medical`,`vitals`,`csi`],adr:`ADR-021`,runtime:`simulated`},{id:`intrusion`,name:`Intrusion detector`,category:`sec`,crate:`wifi-densepose-wasm-edge`,summary:`Zone-based intrusion alert from CSI motion patterns.`,events:[200,201],budget:`S`,status:`available`,tags:[`security`,`zone`,`csi`],runtime:`simulated`},{id:`med_sleep_apnea`,name:`Sleep-apnea detector`,category:`med`,crate:`wifi-densepose-wasm-edge`,summary:`Episodic respiratory pause detection during sleep cycles.`,events:[105],budget:`S`,status:`available`,tags:[`medical`,`sleep`,`breathing`]},{id:`med_cardiac_arrhythmia`,name:`Cardiac arrhythmia`,category:`med`,crate:`wifi-densepose-wasm-edge`,summary:`Beat-to-beat irregularity classifier from cardiac micro-Doppler.`,events:[103,104],budget:`M`,status:`available`,tags:[`medical`,`cardiac`,`arrhythmia`]},{id:`med_respiratory_distress`,name:`Respiratory distress`,category:`med`,crate:`wifi-densepose-wasm-edge`,summary:`Distress signature: rapid shallow breathing + accessory-muscle motion.`,events:[101,102],budget:`S`,status:`available`,tags:[`medical`,`breathing`,`icu`]},{id:`med_gait_analysis`,name:`Gait analysis`,category:`med`,crate:`wifi-densepose-wasm-edge`,summary:`Stride length, cadence, asymmetry from through-wall CSI pose tracking.`,budget:`M`,status:`available`,tags:[`medical`,`gait`,`pose`]},{id:`med_seizure_detect`,name:`Seizure detector`,category:`med`,crate:`wifi-densepose-wasm-edge`,summary:`Tonic-clonic seizure motion signature.`,budget:`M`,status:`beta`,tags:[`medical`,`neuro`]},{id:`sec_perimeter_breach`,name:`Perimeter breach`,category:`sec`,crate:`wifi-densepose-wasm-edge`,summary:`Approach/departure detection at user-defined boundary segments.`,events:[210,211,212,213],budget:`S`,status:`available`,tags:[`security`,`perimeter`]},{id:`sec_weapon_detect`,name:`Metal anomaly / weapon`,category:`sec`,crate:`wifi-densepose-wasm-edge`,summary:`Metal-perturbation flag in CSI; potential weapon presence (research).`,events:[220,221,222],budget:`M`,status:`research`,tags:[`security`,`metal`,`csi`]},{id:`sec_tailgating`,name:`Tailgating detector`,category:`sec`,crate:`wifi-densepose-wasm-edge`,summary:`Detect 2+ persons crossing a single-passage threshold.`,events:[230,231,232],budget:`S`,status:`available`,tags:[`security`,`access-control`]},{id:`sec_loitering`,name:`Loitering detector`,category:`sec`,crate:`wifi-densepose-wasm-edge`,summary:`Stationary occupancy past a configurable dwell threshold.`,events:[240,241,242],budget:`S`,status:`available`,tags:[`security`,`dwell`]},{id:`sec_panic_motion`,name:`Panic motion`,category:`sec`,crate:`wifi-densepose-wasm-edge`,summary:`High-energy distress motion: struggle / fleeing pattern.`,events:[250,251,252],budget:`S`,status:`beta`,tags:[`security`,`distress`]},{id:`bld_hvac_presence`,name:`HVAC presence`,category:`bld`,crate:`wifi-densepose-wasm-edge`,summary:`Occupied/activity-level/departure-countdown for HVAC zones.`,events:[310,311,312],budget:`S`,status:`available`,tags:[`hvac`,`building`,`energy`]},{id:`bld_lighting_zones`,name:`Lighting zones`,category:`bld`,crate:`wifi-densepose-wasm-edge`,summary:`Per-zone light on/dim/off cues from occupancy.`,events:[320,321,322],budget:`S`,status:`available`,tags:[`lighting`,`building`]},{id:`bld_elevator_count`,name:`Elevator count`,category:`bld`,crate:`wifi-densepose-wasm-edge`,summary:`Person count inside elevator car from CSI.`,events:[330],budget:`S`,status:`available`,tags:[`elevator`,`building`]},{id:`bld_meeting_room`,name:`Meeting-room utilization`,category:`bld`,crate:`wifi-densepose-wasm-edge`,summary:`Meeting size + duration analytics for booking systems.`,budget:`S`,status:`available`,tags:[`meeting`,`analytics`]},{id:`bld_energy_audit`,name:`Energy audit`,category:`bld`,crate:`wifi-densepose-wasm-edge`,summary:`Continuous occupancy-vs-HVAC-state audit for energy savings.`,budget:`M`,status:`available`,tags:[`energy`,`audit`]},{id:`ret_queue_length`,name:`Queue length`,category:`ret`,crate:`wifi-densepose-wasm-edge`,summary:`Live queue-length tracking for checkout / kiosks.`,budget:`S`,status:`available`,tags:[`retail`,`queue`]},{id:`ret_dwell_heatmap`,name:`Dwell heatmap`,category:`ret`,crate:`wifi-densepose-wasm-edge`,summary:`Per-zone dwell time accumulation; analytics-only export.`,budget:`M`,status:`available`,tags:[`retail`,`heatmap`]},{id:`ret_customer_flow`,name:`Customer flow`,category:`ret`,crate:`wifi-densepose-wasm-edge`,summary:`Origin-destination flow graph through a store layout.`,budget:`M`,status:`available`,tags:[`retail`,`flow`]},{id:`ret_table_turnover`,name:`Table turnover`,category:`ret`,crate:`wifi-densepose-wasm-edge`,summary:`Restaurant table seat / vacate transitions.`,budget:`S`,status:`available`,tags:[`retail`,`restaurant`]},{id:`ret_shelf_engagement`,name:`Shelf engagement`,category:`ret`,crate:`wifi-densepose-wasm-edge`,summary:`Reach-to-shelf gestures and dwell at product zones.`,budget:`M`,status:`available`,tags:[`retail`,`shelf`]},{id:`ind_forklift_proximity`,name:`Forklift proximity`,category:`ind`,crate:`wifi-densepose-wasm-edge`,summary:`Worker-near-forklift safety alert.`,budget:`S`,status:`available`,tags:[`industrial`,`safety`]},{id:`ind_confined_space`,name:`Confined-space monitor`,category:`ind`,crate:`wifi-densepose-wasm-edge`,summary:`Last-person-out detection + presence audit for OSHA confined-space entries.`,budget:`S`,status:`available`,tags:[`industrial`,`osha`]},{id:`ind_clean_room`,name:`Clean-room PPE / motion`,category:`ind`,crate:`wifi-densepose-wasm-edge`,summary:`Motion patterns consistent with proper PPE-clad movement.`,budget:`M`,status:`beta`,tags:[`industrial`,`cleanroom`]},{id:`ind_livestock_monitor`,name:`Livestock monitor`,category:`ind`,crate:`wifi-densepose-wasm-edge`,summary:`Vital-sign + activity tracking for stall-bound livestock.`,budget:`M`,status:`beta`,tags:[`agriculture`,`livestock`]},{id:`ind_structural_vibration`,name:`Structural vibration`,category:`ind`,crate:`wifi-densepose-wasm-edge`,summary:`Building/equipment micro-vibration via CSI phase derivative.`,budget:`M`,status:`research`,tags:[`industrial`,`vibration`]},{id:`sig_coherence_gate`,name:`Coherence gate (extended)`,category:`sig`,crate:`wifi-densepose-wasm-edge`,summary:`Hysteresis + multi-state coherence gate driving downstream apps.`,budget:`S`,status:`available`,tags:[`gate`,`csi`]},{id:`sig_flash_attention`,name:`Flash attention (CSI)`,category:`sig`,crate:`wifi-densepose-wasm-edge`,summary:`Edge-friendly attention block for CSI subcarrier weighting.`,budget:`M`,status:`beta`,tags:[`attention`,`csi`]},{id:`sig_temporal_compress`,name:`Temporal-tensor compress`,category:`sig`,crate:`wifi-densepose-wasm-edge`,summary:`RuVector temporal-tensor compression on the CSI buffer.`,budget:`M`,status:`available`,tags:[`compress`,`tensor`]},{id:`sig_sparse_recovery`,name:`Sparse recovery`,category:`sig`,crate:`wifi-densepose-wasm-edge`,summary:`114→56 subcarrier sparse interpolation via L1 solver.`,budget:`M`,status:`available`,tags:[`sparse`,`csi`]},{id:`sig_mincut_person_match`,name:`Mincut person-match`,category:`sig`,crate:`wifi-densepose-wasm-edge`,summary:`Min-cut person assignment across multistatic frames.`,budget:`M`,status:`available`,tags:[`mincut`,`matching`]},{id:`sig_optimal_transport`,name:`Optimal transport`,category:`sig`,crate:`wifi-densepose-wasm-edge`,summary:`OT-based feature alignment between mesh nodes.`,budget:`M`,status:`beta`,tags:[`ot`,`alignment`]},{id:`lrn_dtw_gesture_learn`,name:`DTW gesture learn`,category:`lrn`,crate:`wifi-densepose-wasm-edge`,summary:`On-device template learning for personalized gesture libraries.`,budget:`M`,status:`beta`,tags:[`lifelong`,`gesture`]},{id:`lrn_anomaly_attractor`,name:`Anomaly attractor`,category:`lrn`,crate:`wifi-densepose-wasm-edge`,summary:`Novelty detector with dynamic-attractor recall.`,budget:`M`,status:`research`,tags:[`novelty`,`lifelong`]},{id:`lrn_meta_adapt`,name:`Meta-adapt`,category:`lrn`,crate:`wifi-densepose-wasm-edge`,summary:`Meta-learning adapter for fast site-to-site transfer.`,budget:`L`,status:`research`,tags:[`meta-learning`]},{id:`lrn_ewc_lifelong`,name:`EWC++ lifelong`,category:`lrn`,crate:`wifi-densepose-wasm-edge`,summary:`Elastic-weight-consolidation gate to avoid catastrophic forgetting.`,budget:`M`,status:`beta`,tags:[`lifelong`,`ewc`]},{id:`spt_pagerank_influence`,name:`PageRank influence`,category:`spt`,crate:`wifi-densepose-wasm-edge`,summary:`Graph-influence ranking on the multistatic mesh.`,budget:`M`,status:`beta`,tags:[`graph`,`pagerank`]},{id:`spt_micro_hnsw`,name:`µHNSW vector index`,category:`spt`,crate:`wifi-densepose-wasm-edge`,summary:`Tiny HNSW index for AETHER re-ID embeddings on-device.`,budget:`M`,status:`available`,tags:[`hnsw`,`reid`]},{id:`spt_spiking_tracker`,name:`Spiking tracker`,category:`spt`,crate:`wifi-densepose-wasm-edge`,summary:`Spiking-network multi-target tracker.`,budget:`L`,status:`research`,tags:[`snn`,`tracker`]},{id:`tmp_pattern_sequence`,name:`Pattern sequence`,category:`tmp`,crate:`wifi-densepose-wasm-edge`,summary:`Sequence-of-events pattern matcher (e.g. ingress→linger→egress).`,budget:`M`,status:`available`,tags:[`temporal`,`pattern`]},{id:`tmp_temporal_logic_guard`,name:`Temporal logic guard`,category:`tmp`,crate:`wifi-densepose-wasm-edge`,summary:`LTL/MTL safety-property guard over event streams.`,budget:`M`,status:`beta`,tags:[`ltl`,`safety`]},{id:`tmp_goap_autonomy`,name:`GOAP autonomy`,category:`tmp`,crate:`wifi-densepose-wasm-edge`,summary:`Goal-oriented action planning for adaptive routines.`,budget:`L`,status:`research`,tags:[`planning`,`autonomy`]},{id:`ais_prompt_shield`,name:`Prompt shield`,category:`ais`,crate:`wifi-densepose-wasm-edge`,summary:`Edge-side LLM prompt-injection guard for on-device assistants.`,budget:`M`,status:`beta`,tags:[`security`,`llm`]},{id:`ais_behavioral_profiler`,name:`Behavioral profiler`,category:`ais`,crate:`wifi-densepose-wasm-edge`,summary:`Anomalous-behaviour profiler (drift in motion habits).`,budget:`M`,status:`beta`,tags:[`anomaly`,`behaviour`]},{id:`qnt_quantum_coherence`,name:`Quantum coherence`,category:`qnt`,crate:`wifi-densepose-wasm-edge`,summary:`Coherence diagnostics adapted for quantum-sensor signals.`,budget:`M`,status:`research`,tags:[`quantum`,`coherence`]},{id:`qnt_interference_search`,name:`Interference search`,category:`qnt`,crate:`wifi-densepose-wasm-edge`,summary:`Interferometric anomaly search across mesh viewpoints.`,budget:`L`,status:`research`,tags:[`quantum`,`interference`]},{id:`aut_psycho_symbolic`,name:`Psycho-symbolic agent`,category:`aut`,crate:`wifi-densepose-wasm-edge`,summary:`Symbolic-rule + neural-feature hybrid for low-power autonomy loops.`,budget:`L`,status:`research`,tags:[`autonomy`,`symbolic`]},{id:`aut_self_healing_mesh`,name:`Self-healing mesh`,category:`aut`,crate:`wifi-densepose-wasm-edge`,summary:`Mesh-topology repair with per-node health gossip.`,budget:`M`,status:`beta`,tags:[`mesh`,`health`]},{id:`exo_ghost_hunter`,name:`Ghost hunter (anomaly)`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Empty-room CSI anomaly detector — impulsive/periodic/drift/random + hidden-presence sub-detector.`,events:[650,651,652,653],budget:`S`,status:`available`,tags:[`anomaly`,`paranormal`,`csi`],adr:`ADR-041`,runtime:`simulated`},{id:`exo_breathing_sync`,name:`Breathing sync`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Multi-person breathing synchrony analytics.`,budget:`M`,status:`beta`,tags:[`breathing`,`sync`]},{id:`exo_dream_stage`,name:`Dream-stage classifier`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`NREM/REM stage classification from breathing + micro-motion.`,budget:`M`,status:`research`,tags:[`sleep`,`rem`]},{id:`exo_emotion_detect`,name:`Emotion detector`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Coarse arousal/valence from breathing + heart-rate variability.`,budget:`M`,status:`research`,tags:[`affect`]},{id:`exo_gesture_language`,name:`Gesture language`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Sign-language pattern recognition.`,budget:`L`,status:`research`,tags:[`hci`,`sign`]},{id:`exo_happiness_score`,name:`Happiness score`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Aggregate well-being score from co-occupancy + activity dynamics.`,budget:`M`,status:`research`,tags:[`affect`,`wellbeing`]},{id:`exo_hyperbolic_space`,name:`Hyperbolic space embed`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Hyperbolic embeddings for hierarchical scene structure.`,budget:`L`,status:`research`,tags:[`embedding`,`hyperbolic`]},{id:`exo_music_conductor`,name:`Music conductor`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Map gesture energy to MIDI tempo/dynamics.`,budget:`M`,status:`research`,tags:[`midi`,`art`]},{id:`exo_plant_growth`,name:`Plant-growth tracker`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Slow CSI drift tracking for greenhouse foliage growth.`,budget:`L`,status:`research`,tags:[`agriculture`]},{id:`exo_rain_detect`,name:`Rain detector`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Outdoor CSI signature of rainfall.`,budget:`M`,status:`research`,tags:[`weather`]},{id:`exo_time_crystal`,name:`Time-crystal periodicity`,category:`exo`,crate:`wifi-densepose-wasm-edge`,summary:`Periodicity diagnostics with anti-aliasing harmonics.`,budget:`M`,status:`research`,tags:[`periodicity`]}],ke={sim:{label:`Simulators`,color:`oklch(0.78 0.14 70)`,range:`—`},med:{label:`Medical & Health`,color:`oklch(0.65 0.22 25)`,range:`100–199`},sec:{label:`Security & Safety`,color:`oklch(0.7 0.18 35)`,range:`200–299`},bld:{label:`Smart Building`,color:`oklch(0.78 0.12 195)`,range:`300–399`},ret:{label:`Retail & Hospitality`,color:`oklch(0.78 0.14 145)`,range:`400–499`},ind:{label:`Industrial`,color:`oklch(0.72 0.18 330)`,range:`500–599`},sig:{label:`Signal Processing`,color:`oklch(0.78 0.14 70)`,range:`600–619`},lrn:{label:`Online Learning`,color:`oklch(0.78 0.12 260)`,range:`620–639`},spt:{label:`Spatial / Graph`,color:`oklch(0.7 0.18 100)`,range:`640–659`},tmp:{label:`Temporal / Planning`,color:`oklch(0.7 0.16 50)`,range:`660–679`},ais:{label:`AI Safety`,color:`oklch(0.65 0.22 25)`,range:`700–719`},qnt:{label:`Quantum`,color:`oklch(0.72 0.18 290)`,range:`720–739`},aut:{label:`Autonomy`,color:`oklch(0.78 0.14 145)`,range:`740–759`},exo:{label:`Exotic / Research`,color:`oklch(0.72 0.18 330)`,range:`650–699`}};function Ae(){return Oe.map(e=>({id:e.id,active:e.active===!0,eventCount:0}))}function je(e,t){if(!e)return 1;let n=e.toLowerCase(),r=0;return t.id.toLowerCase().includes(n)&&(r+=3),t.name.toLowerCase().includes(n)&&(r+=3),t.summary.toLowerCase().includes(n)&&(r+=1),t.tags?.some(e=>e.toLowerCase().includes(n))&&(r+=2),t.category===n&&(r+=5),r}var Me=`nvsim`,Ne=1,G=`kv`,Pe=null;function Fe(){return Pe||(Pe=new Promise((e,t)=>{let n=indexedDB.open(Me,Ne);n.onupgradeneeded=()=>{let e=n.result;e.objectStoreNames.contains(G)||e.createObjectStore(G)},n.onsuccess=()=>e(n.result),n.onerror=()=>t(n.error)}),Pe)}async function K(e){let t=await Fe();return await new Promise((n,r)=>{let i=t.transaction(G,`readonly`).objectStore(G).get(e);i.onsuccess=()=>n(i.result),i.onerror=()=>r(i.error)})}async function q(e,t){let n=await Fe();return await new Promise((r,i)=>{let a=n.transaction(G,`readwrite`);a.objectStore(G).put(t,e),a.oncomplete=()=>r(),a.onerror=()=>i(a.error)})}function Ie(e){if(e.length===0)return 0;let t=0;for(let n of e)t+=n;return t/e.length}function Le(e){if(e.length<2)return 0;let t=Ie(e),n=0;for(let r of e)n+=(r-t)*(r-t);return Math.sqrt(n/(e.length-1))}var Re={vital_trend:e=>{if(e.bHistory.length<64)return null;let t=e.state.lastEmitS??0;if(e.elapsedS-t<1)return null;e.state.lastEmitS=e.elapsedS;let n=e.bHistory.slice(-64),r=Ie(n),i=0;for(let e=1;e<n.length;e++)(n[e]-r)*(n[e-1]-r)<0&&i++;let a=i/2,o=Math.max(40,Math.min(180,Math.round(a/.65*60))),s=Math.max(8,Math.min(30,Math.round(o/4))),c=[{ts:Date.now(),appId:`vital_trend`,eventId:100,eventName:`VITAL_TREND`,value:o,detail:`HR≈${o} BPM, BR≈${s} br/min`}];return o<60?c.push({ts:Date.now(),appId:`vital_trend`,eventId:103,eventName:`BRADYCARDIA`,value:o,detail:`HR=${o} BPM`}):o>100&&c.push({ts:Date.now(),appId:`vital_trend`,eventId:104,eventName:`TACHYCARDIA`,value:o,detail:`HR=${o} BPM`}),s<12?c.push({ts:Date.now(),appId:`vital_trend`,eventId:101,eventName:`BRADYPNEA`,value:s,detail:`BR=${s} br/min`}):s>24&&c.push({ts:Date.now(),appId:`vital_trend`,eventId:102,eventName:`TACHYPNEA`,value:s,detail:`BR=${s} br/min`}),c},occupancy:e=>{if(e.bHistory.length<32)return null;let t=e.state.lastEmitS??0;if(e.elapsedS-t<2)return null;let n=Le(e.bHistory.slice(-128))*1e9,r=n>.01;return r===(e.state.occ??0)>.5?null:(e.state.occ=+!!r,e.state.lastEmitS=e.elapsedS,{ts:Date.now(),appId:`occupancy`,eventId:r?300:302,eventName:r?`ZONE_OCCUPIED`:`ZONE_TRANSITION`,value:n,detail:r?`σ(|B|)=${n.toFixed(3)} nT — entered`:`σ(|B|)=${n.toFixed(3)} nT — left`})},intrusion:e=>{let t=e.state.ambient??e.bMagT;e.state.ambient=.95*t+.05*e.bMagT;let n=e.bMagT>t*1.5&&e.bMagT>1e-12,r=e.state.dwellStart??0;return n&&r===0?e.state.dwellStart=e.elapsedS:n||(e.state.dwellStart=0),n&&r>0&&e.elapsedS-r>.5&&(e.state.lastEmitS??0)<r?(e.state.lastEmitS=e.elapsedS,{ts:Date.now(),appId:`intrusion`,eventId:200,eventName:`INTRUSION_ALERT`,value:e.bMagT*1e9,detail:`|B|=${(e.bMagT*1e9).toFixed(2)} nT > 1.5× ambient (${(t*1e9).toFixed(2)} nT) for ${(e.elapsedS-r).toFixed(1)} s`}):null},coherence:e=>{if(e.bHistory.length<64)return null;let t=e.state.lastEmitS??0;if(e.elapsedS-t<.5)return null;e.state.lastEmitS=e.elapsedS;let n=e.bHistory.slice(-32),r=e.bHistory.slice(-128,-32);if(r.length<32)return null;let i=Ie(r),a=Le(r);if(a===0)return null;let o=Ie(n),s=Math.abs(o-i)/a;return{ts:Date.now(),appId:`coherence`,eventId:2,eventName:`COHERENCE_SCORE`,value:s,detail:`z=${s.toFixed(2)} σ ${s>3?`· DRIFT`:s>1.5?`· marginal`:`· stable`}`}},adversarial:e=>{if(e.bHistory.length<32)return null;let t=e.state.lastEmitS??0;if(e.elapsedS-t<3)return null;let n=e.bHistory.slice(-32),r=0;for(let e=1;e<n.length;e++){let t=Math.abs(Math.log(Math.max(n[e],1e-15))-Math.log(Math.max(n[e-1],1e-15)));t>r&&(r=t)}return r>5?(e.state.lastEmitS=e.elapsedS,{ts:Date.now(),appId:`adversarial`,eventId:3,eventName:`ANOMALY_DETECTED`,value:r,detail:`log-jump ${r.toFixed(1)} — physically implausible step in |B|`}):null},exo_ghost_hunter:e=>{if(e.bHistory.length<128)return null;let t=e.state.lastEmitS??0;if(e.elapsedS-t<4)return null;e.state.lastEmitS=e.elapsedS;let n=e.bHistory.slice(-128),r=Le(n)*1e9,i=Ie(n),a=0;for(let e of n){let t=Math.abs(e-i);t>a&&(a=t)}let o=a>r*1e-9*4?1:e.elapsedS>10?3:4,s=o===1?`impulsive`:o===3?`drift`:`random`;return{ts:Date.now(),appId:`exo_ghost_hunter`,eventId:651,eventName:`ANOMALY_CLASS`,value:o,detail:`class=${s} · σ=${r.toFixed(3)} nT`}}};function ze(e){return e in Re}var J=l(Ae()),Be=l(``),Y=l(`all`),X=l(`all`);(async()=>{let e=await K(`app-activations`);e&&(J.value=e)})(),c(()=>{let e=J.value;e.length>0&&q(`app-activations`,e);let t=new Set;for(let n of e)n.active&&t.add(n.id);he.value=t});var Ve=class extends e{constructor(...e){super(...e),this.renderTick=0}static{this.styles=t`
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
    .chip .count { color: var(--ink-2); font-size: 10px; }
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
    .badge.rt-running { color: var(--ok); border-color: oklch(0.78 0.14 145 / 0.5); background: oklch(0.78 0.14 145 / 0.08); }
    .badge.rt-simulated { color: var(--accent); border-color: oklch(0.78 0.14 70 / 0.5); background: oklch(0.78 0.14 70 / 0.08); }
    .badge.rt-mesh-only { color: var(--ink-2); border-color: var(--line); }
    .events-feed {
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: var(--radius);
      padding: 14px;
      margin-bottom: 18px;
    }
    .events-feed h3 {
      margin: 0 0 8px;
      font-size: 13px; font-weight: 600;
      color: var(--ink);
    }
    .events-feed .lead {
      font-size: 12px; color: var(--ink-3);
      margin: 0 0 10px;
      line-height: 1.5;
    }
    .events-feed .lines {
      display: flex; flex-direction: column; gap: 4px;
      max-height: 160px; overflow-y: auto;
    }
    .ev-line {
      display: grid;
      grid-template-columns: 60px 90px 1fr;
      gap: 10px;
      padding: 4px 6px;
      border-radius: 4px;
      font-family: var(--mono);
      font-size: 11px;
      color: var(--ink-2);
    }
    .ev-line:hover { background: var(--bg-3); }
    .ev-line .ts { color: var(--ink-4); font-size: 10.5px; }
    .ev-line .id { color: var(--accent); font-size: 10.5px; }
    .ev-line .body { color: var(--ink); }
    .ev-empty {
      font-size: 12px; color: var(--ink-3);
      padding: 8px 0;
    }
    .card-events-count {
      font-size: 10.5px;
      color: var(--accent-4);
      font-family: var(--mono);
    }
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
  `}connectedCallback(){super.connectedCallback(),c(()=>{J.value,Be.value,Y.value,X.value,fe.value,pe.value,this.renderTick++})}isActive(e){return J.value.find(t=>t.id===e)?.active===!0}toggle(e){let t=this.isActive(e.id);if(J.value=J.value.map(t=>t.id===e.id?{...t,active:!t.active,lastActivatedAt:Date.now()}:t),t)z(`info`,`app <span class="k">${e.id}</span> deactivated`);else{let t=e.runtime??`mesh-only`,n=t===`simulated`?` · live runtime engaged`:t===`mesh-only`?` · queued (needs ESP32 mesh)`:``;z(`ok`,`app <span class="k">${e.id}</span> activated${n}`)}}filtered(){let e=Oe;return Y.value!==`all`&&(e=e.filter(e=>e.category===Y.value)),X.value!==`all`&&(e=e.filter(e=>e.status===X.value)),Be.value.trim()&&(e=e.map(e=>({a:e,s:je(Be.value,e)})).filter(e=>e.s>0).sort((e,t)=>t.s-e.s).map(e=>e.a)),e}categoryCounts(){let e={all:Oe.length};for(let t of Object.keys(ke))e[t]=0;for(let t of Oe)e[t.category]=(e[t.category]??0)+1;return e}render(){let e=this.filtered(),t=this.categoryCounts(),n=J.value.filter(e=>e.active).length;return i`
      <div class="head">
        <div class="ttl">
          App Store
          <small>${Oe.length} edge apps · ${n} active</small>
        </div>
        <input class="search" id="app-search" placeholder="Search by name, tag, or category…"
          .value=${Be.value}
          @input=${e=>{Be.value=e.target.value}} />
      </div>

      <div class="filters">
        <span class="chip ${Y.value===`all`?`on`:``}"
          @click=${()=>Y.value=`all`}>
          All<span class="count">${t.all}</span>
        </span>
        ${Object.keys(ke).map(e=>i`
          <span class="chip ${Y.value===e?`on`:``}"
            @click=${()=>Y.value=e}>
            <span class="swatch" style=${`background:${ke[e].color}`}></span>
            ${ke[e].label}
            <span class="count">${t[e]??0}</span>
          </span>
        `)}
        <span style="flex:1; min-width:8px"></span>
        <span class="chip ${X.value===`all`?`on`:``}" @click=${()=>X.value=`all`}>any</span>
        <span class="chip ${X.value===`available`?`on`:``}" @click=${()=>X.value=`available`}>available</span>
        <span class="chip ${X.value===`beta`?`on`:``}" @click=${()=>X.value=`beta`}>beta</span>
        <span class="chip ${X.value===`research`?`on`:``}" @click=${()=>X.value=`research`}>research</span>
      </div>

      ${this.renderEventsFeed()}

      ${e.length===0?i`<div class="empty">No apps match the current filters.</div>`:i`<div class="grid">${e.map(e=>this.card(e))}</div>`}
    `}renderEventsFeed(){let e=fe.value.slice(-12).reverse(),t=J.value.filter(e=>e.active&&ze(e.id)).length;return i`
      <div class="events-feed">
        <h3>Live runtime feed
          ${t>0?i`<span class="card-events-count" style="margin-left: 8px;">${t} simulated app${t===1?``:`s`} active</span>`:``}
        </h3>
        <p class="lead">
          Apps with the <span class="badge rt-simulated" style="font-size:9.5px; padding:0 4px;">simulated</span>
          runtime emit real i32 event IDs against nvsim's live frame stream below.
          Apps with <span class="badge rt-mesh-only" style="font-size:9.5px; padding:0 4px;">mesh-only</span>
          need an ESP32-S3 + WS transport (deferred to V2). The
          <span class="badge rt-running" style="font-size:9.5px; padding:0 4px;">running</span>
          badge marks <code>nvsim</code> itself, which is always running.
        </p>
        ${e.length===0?i`<div class="ev-empty">No events yet. Toggle a card with the <i>simulated</i> badge and press <b>▶ Run</b>.</div>`:i`<div class="lines">${e.map(e=>{let t=new Date(e.ts),n=`${String(t.getSeconds()).padStart(2,`0`)}.${String(t.getMilliseconds()).padStart(3,`0`)}`;return i`
                <div class="ev-line">
                  <span class="ts">${n}</span>
                  <span class="id">${e.appId}</span>
                  <span class="body"><b style="color:var(--accent-2);">${e.eventName}</b><span style="color:var(--ink-3);"> · ${e.eventId}</span> ${e.detail?`· ${e.detail}`:``}</span>
                </div>
              `})}</div>`}
      </div>
    `}card(e){let t=this.isActive(e.id),n=ke[e.category],r=e.runtime??`mesh-only`,a=pe.value[e.id]??0;return i`
      <div class="card ${t?`active`:``}" data-app-id=${e.id}>
        <div class="card-h">
          <span class="swatch" style=${`background:${n.color}`}></span>
          <span class="name">${e.name}</span>
        </div>
        <div class="summary">${e.summary}</div>
        <div class="meta">
          <span class="badge cat">${n.label}</span>
          <span class="badge status-${e.status}">${e.status}</span>
          <span class="badge rt-${r}" title=${{running:`This app is genuinely running in your browser right now.`,simulated:`A pared-down version of this algorithm runs against nvsim's magnetic frame stream as a proxy for its native CSI input. Toggle on, then press ▶ Run to see real event IDs in the feed.`,"mesh-only":`This algorithm needs CSI subcarrier data from an ESP32-S3 mesh. The toggle persists; activation is pushed via WS transport (V2).`}[r]}>${{running:`running`,simulated:`simulated`,"mesh-only":`needs mesh`}[r]}</span>
          ${e.budget?i`<span class="badge budget">budget ${e.budget}</span>`:``}
          ${e.adr?i`<span class="badge">${e.adr}</span>`:``}
          ${e.events?.length?i`<span class="badge">events ${e.events.join(`·`)}</span>`:``}
        </div>
        <div class="card-foot">
          <span class="events">${e.crate}</span>
          ${a>0?i`<span class="card-events-count">⚡ ${a} ev</span>`:``}
          <span class="toggle ${t?`on`:``}" role="switch" aria-label=${`Enable ${e.name}`}
            aria-checked=${t}
            data-app-toggle=${e.id}
            @click=${()=>this.toggle(e)}></span>
        </div>
      </div>
    `}};u([r()],Ve.prototype,`renderTick`,void 0),Ve=u([n(`nv-app-store`)],Ve);var Z=class extends e{constructor(...e){super(...e),this.open=!1,this.filter=``,this.idx=0,this.cmds=[{ico:`▶`,label:`Run pipeline`,kbd:`Space`,run:async()=>{await L()?.run(),h.value=!0,U(`Pipeline running`,`▶`)}},{ico:`❚`,label:`Pause pipeline`,run:async()=>{await L()?.pause(),h.value=!1,U(`Paused`,`❚❚`)}},{ico:`+`,label:`New scene…`,kbd:`⌘N`,run:()=>V({title:`New scene`,body:`<p>Build a fresh magnetic scene. The dashboard generates the JSON
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
        </select>`,buttons:[{label:`Cancel`,variant:`ghost`},{label:`Create`,variant:`primary`,onClick:async()=>{let e=document.querySelector(`nv-app`)?.shadowRoot?.querySelector(`nv-modal`)?.shadowRoot;if(!e)return;let t=(e.querySelector(`#ns-name`)?.value??`custom`).trim(),n=parseFloat(e.querySelector(`#ns-moment`)?.value??`1e-6`),r=parseFloat(e.querySelector(`#ns-distance`)?.value??`0.5`),i=e.querySelector(`#ns-ferrous`)?.value===`1`,a=e.querySelector(`#ns-mains`)?.value===`1`,o={dipoles:[{position:[0,0,r],moment:[0,0,n]}],loops:a?[{centre:[0,1,0],normal:[0,1,0],radius:.05,current:2,n_segments:64}]:[],ferrous:i?[{position:[1,0,0],volume:1e-4,susceptibility:5e3}]:[],eddy:[],sensors:[[0,0,0]],ambient_field:[1e-6,0,0]};await L()?.loadScene(o),z(`ok`,`scene <span class="s">${t}</span> loaded · 1 dipole · ${a?`1 loop · `:``}${i?`1 ferrous · `:``}1 sensor`),U(`Scene "${t}" loaded`,`+`)}}]})},{ico:`📦`,label:`Export proof bundle…`,kbd:`⌘E`,run:async()=>{let e=L();if(e){z(`dbg`,`building proof bundle…`);try{let t=await e.exportProofBundle(),n=URL.createObjectURL(t),r=document.createElement(`a`);r.href=n,r.download=`nvsim-proof-${Date.now()}.json`,r.click(),URL.revokeObjectURL(n),z(`ok`,`proof bundle exported · ${t.size} bytes`),U(`Proof bundle saved (${t.size} B)`,`📦`)}catch(e){z(`err`,`export failed: ${e.message}`)}}}},{ico:`⟳`,label:`Reset pipeline`,kbd:`⌘R`,run:()=>V({title:`Reset pipeline?`,body:`<p>Clears the frame stream and rewinds <code>t</code> to 0.</p>`,buttons:[{label:`Cancel`,variant:`ghost`},{label:`Reset`,variant:`danger`,onClick:async()=>{await L()?.reset(),z(`warn`,`pipeline reset · t=0`),U(`Pipeline reset`,`⟳`)}}]})},{ico:`✓`,label:`Verify witness`,run:async()=>{let e=L();if(!e)return;j.value=`pending`;let t=M.value,n=new Uint8Array(32);for(let e=0;e<32;e++)n[e]=parseInt(t.slice(e*2,e*2+2),16);(await e.verifyWitness(n)).ok?(j.value=`ok`,A.value=t,U(`Witness verified`,`✓`)):(j.value=`fail`,U(`Witness mismatch!`,`✗`))}},{ico:`☼`,label:`Toggle theme`,kbd:`⌘/`,run:()=>{S.value=S.value===`dark`?`light`:`dark`}},{ico:`⚙`,label:`Open settings`,kbd:`⌘,`,run:()=>window.dispatchEvent(new CustomEvent(`open-settings`))},{ico:`?`,label:`Keyboard shortcuts…`,run:()=>V({title:`Keyboard shortcuts`,body:`<div style="display:grid;grid-template-columns:auto 1fr;gap:6px 16px;font-size:13px;">
        <div><code>⌘K / Ctrl K</code></div><div>Command palette</div>
        <div><code>Space</code></div><div>Play / pause</div>
        <div><code>⌘R</code></div><div>Reset</div>
        <div><code>⌘,</code></div><div>Settings</div>
        <div><code>⌘/</code></div><div>Toggle theme</div>
        <div><code>\`</code></div><div>Debug HUD</div>
        <div><code>1 · 2 · 3</code></div><div>Inspector tabs</div>
        <div><code>Esc</code></div><div>Close modal/palette</div>
        <div><code>/</code></div><div>Focus REPL</div>
      </div>`,buttons:[{label:`Close`,variant:`primary`}]})},{ico:`i`,label:`About nvsim…`,run:()=>V({title:`About nvsim`,body:`<p><b>nvsim</b> is a deterministic, byte-reproducible forward simulator for nitrogen-vacancy diamond magnetometry.</p>
        <p>This dashboard runs nvsim as WASM in a Web Worker. Same <code>(scene, config, seed)</code> → byte-identical SHA-256 witness across runs and machines.</p>
        <p>License: MIT OR Apache-2.0 · See ADR-089, ADR-092.</p>`,buttons:[{label:`Close`,variant:`primary`}]})}],this.onKey=e=>{(e.metaKey||e.ctrlKey)&&e.key.toLowerCase()===`k`?(e.preventDefault(),this.openPal()):e.key===`Escape`&&this.open?this.closePal():this.open&&(e.key===`ArrowDown`?(this.idx=Math.min(this.cmds.length-1,this.idx+1),e.preventDefault()):e.key===`ArrowUp`?(this.idx=Math.max(0,this.idx-1),e.preventDefault()):e.key===`Enter`&&(this.runIdx(),e.preventDefault()))},this.onOpen=()=>this.openPal()}static{this.styles=t`
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
  `}connectedCallback(){super.connectedCallback(),window.addEventListener(`keydown`,this.onKey),window.addEventListener(`nv-palette`,this.onOpen)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener(`keydown`,this.onKey),window.removeEventListener(`nv-palette`,this.onOpen)}openPal(){this.open=!0,this.setAttribute(`open`,``),this.filter=``,this.idx=0,setTimeout(()=>this.inputEl?.focus(),0)}closePal(){this.open=!1,this.removeAttribute(`open`)}filtered(){if(!this.filter.trim())return this.cmds;let e=this.filter.toLowerCase();return this.cmds.filter(t=>t.label.toLowerCase().includes(e))}runIdx(){let e=this.filtered()[this.idx];e&&(e.run(),this.closePal())}render(){let e=this.filtered();return i`
      <div class="palette" data-id="palette">
        <div class="input">
          <input id="palette-input" type="text" placeholder="Type a command…"
            .value=${this.filter}
            @input=${e=>{this.filter=e.target.value,this.idx=0}} />
        </div>
        <div class="list">
          ${e.map((e,t)=>i`
            <div class="item ${t===this.idx?`active`:``}" @click=${()=>{this.idx=t,this.runIdx()}}>
              <span class="ico">${e.ico}</span>
              <span class="lbl">${e.label}</span>
              ${e.kbd?i`<span class="kbd">${e.kbd}</span>`:``}
            </div>
          `)}
        </div>
      </div>
    `}};u([r()],Z.prototype,`open`,void 0),u([r()],Z.prototype,`filter`,void 0),u([r()],Z.prototype,`idx`,void 0),u([s(`#palette-input`)],Z.prototype,`inputEl`,void 0),Z=u([n(`nv-palette`)],Z);var He=class extends e{constructor(...e){super(...e),this.open=!1,this.renderFps=0,this.lastTs=performance.now(),this.frameCount=0,this.rafId=0,this.onKey=e=>{e.key==="`"&&!e.target.matches(`input, textarea`)&&(this.open=!this.open,this.toggleAttribute(`open`,this.open))},this.tick=()=>{this.rafId=requestAnimationFrame(this.tick);let e=performance.now();this.frameCount++,e-this.lastTs>=500&&(this.renderFps=this.frameCount*1e3/(e-this.lastTs),this.frameCount=0,this.lastTs=e,this.requestUpdate())}}static{this.styles=t`
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
  `}connectedCallback(){super.connectedCallback(),window.addEventListener(`keydown`,this.onKey),c(()=>{k.value,re.value,D.value,O.value,ne.value,this.requestUpdate()}),this.tick()}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener(`keydown`,this.onKey),cancelAnimationFrame(this.rafId)}render(){return i`
      <div class="h"><span>nvsim · debug</span><span class="x" @click=${()=>{this.open=!1,this.removeAttribute(`open`)}}>✕</span></div>
      <div class="row"><span class="k">render fps</span><span class="v">${this.renderFps.toFixed(1)}</span></div>
      <div class="row"><span class="k">sim fps</span><span class="v">${k.value>0?Math.round(k.value):`—`}</span></div>
      <div class="row"><span class="k">frames</span><span class="v">${re.value.toString()}</span></div>
      <div class="row"><span class="k">|B|</span><span class="v">${(D.value*1e9).toFixed(3)} nT</span></div>
      <div class="row"><span class="k">SNR</span><span class="v">${O.value>0?O.value.toFixed(1):`—`}</span></div>
      <div class="row"><span class="k">DOM</span><span class="v">${document.querySelectorAll(`*`).length}</span></div>
    `}};u([r()],He.prototype,`open`,void 0),u([r()],He.prototype,`renderFps`,void 0),He=u([n(`nv-debug-hud`)],He);var Ue=class extends e{constructor(...e){super(...e),this.open=!1}static{this.styles=t`
    /* The host covers the viewport without transforming itself. Only the
     * inner .panel is transformed; otherwise the host's transform would
     * create a containing block for the fixed-position scrim, clipping
     * it to the panel's 420 px width and breaking outside-to-dismiss. */
    :host {
      position: fixed; inset: 0;
      z-index: 51;
      pointer-events: none;
      opacity: 0;
      transition: opacity 0.2s;
    }
    :host([open]) { pointer-events: auto; opacity: 1; }
    .scrim {
      position: absolute; inset: 0;
      background: rgba(0, 0, 0, 0.5);
      backdrop-filter: blur(2px);
    }
    .panel {
      position: absolute;
      top: 0; right: 0; bottom: 0;
      width: 420px; max-width: 100vw;
      background: var(--bg-1);
      border-left: 1px solid var(--line);
      transform: translateX(100%);
      transition: transform 0.25s cubic-bezier(0.4, 0, 0.2, 1);
      display: flex; flex-direction: column;
      box-shadow: -20px 0 60px -20px rgba(0, 0, 0, 0.5);
    }
    :host([open]) .panel { transform: translateX(0); }
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
  `}connectedCallback(){super.connectedCallback(),c(()=>{S.value,C.value,w.value,T.value,f.value,p.value,this.requestUpdate()}),window.addEventListener(`open-settings`,()=>{this.open=!0,this.setAttribute(`open`,``)})}close(){this.open=!1,this.removeAttribute(`open`)}async resetPrefs(){if(confirm(`Reset all preferences and IndexedDB state? Reloads the page.`)){try{let e=await indexedDB.databases?.();if(e)for(let t of e)t.name&&indexedDB.deleteDatabase(t.name)}catch{}location.reload()}}render(){return i`
      <div class="scrim" @click=${()=>this.close()}></div>
      <div class="panel" role="dialog" aria-modal="true" aria-label="Settings">
      <div class="h">
        <div class="ttl">Settings</div>
        <button class="close" @click=${()=>this.close()}>×</button>
      </div>
      <div class="body">
        <div class="group">
          <h4>Appearance</h4>
          <div class="row">
            <div>
              <div class="lbl">Theme</div>
              <div class="desc">Dark is the default; light has higher contrast for daylight work.</div>
            </div>
            <div class="seg">
              <button class=${S.value===`dark`?`on`:``}
                @click=${()=>S.value=`dark`}>dark</button>
              <button class=${S.value===`light`?`on`:``}
                @click=${()=>S.value=`light`}>light</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Density</div>
              <div class="desc">Affects panel padding and font scale (15 / 14 / 13 px). Choose what your eyes prefer.</div>
            </div>
            <div class="seg">
              <button class=${C.value===`comfy`?`on`:``}
                @click=${()=>C.value=`comfy`}>comfy</button>
              <button class=${C.value==="default"?`on`:``}
                @click=${()=>C.value=`default`}>default</button>
              <button class=${C.value===`compact`?`on`:``}
                @click=${()=>C.value=`compact`}>compact</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Reduce motion</div>
              <div class="desc">Stops the rotating diamond, animated field lines, and chart easing. Auto-on if your system has the prefers-reduced-motion preference set.</div>
            </div>
            <span class="toggle ${w.value?`on`:``}"
              role="switch" aria-checked=${w.value} aria-label="Reduce motion"
              @click=${()=>w.value=!w.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Pipeline</h4>
          <div class="row">
            <div>
              <div class="lbl">Auto-rerun on edit</div>
              <div class="desc">When you change a Tunables slider or load a new scene, push the change to the worker without a manual restart.</div>
            </div>
            <span class="toggle ${T.value?`on`:``}"
              role="switch" aria-checked=${T.value} aria-label="Auto-rerun on edit"
              @click=${()=>T.value=!T.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Transport</h4>
          <div class="row">
            <div>
              <div class="lbl">Mode</div>
              <div class="desc">WASM runs nvsim in your browser (default, no server). WS connects to a host-supplied nvsim-server (REST + binary WebSocket); see ADR-092 §6.2.</div>
            </div>
            <div class="seg">
              <button class=${f.value===`wasm`?`on`:``}
                @click=${()=>f.value=`wasm`}>WASM</button>
              <button class=${f.value===`ws`?`on`:``}
                @click=${()=>f.value=`ws`}>WS</button>
            </div>
          </div>
          ${f.value===`ws`?i`
            <div class="row">
              <div>
                <div class="lbl">WS URL</div>
                <div class="desc">Where your nvsim-server is listening. The server defaults to 127.0.0.1:7878.</div>
              </div>
              <input type="text" placeholder="ws://localhost:7878" .value=${p.value}
                @input=${e=>p.value=e.target.value} />
            </div>`:``}
        </div>

        <div class="group">
          <h4>Help</h4>
          <div class="row">
            <div>
              <div class="lbl">Open help center</div>
              <div class="desc">Quickstart, glossary, FAQ, and shortcuts. Press <kbd style="font-family:var(--mono);font-size:10.5px;padding:1px 4px;background:var(--bg-3);border:1px solid var(--line);border-radius:3px;">?</kbd> any time.</div>
            </div>
            <button class="seg"
              @click=${()=>{this.close(),window.dispatchEvent(new CustomEvent(`nv-show-help`))}}
              style="padding:6px 12px;cursor:pointer;background:var(--bg-3);border:1px solid var(--line);border-radius:6px;color:var(--ink);">
              Open
            </button>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Replay welcome tour</div>
              <div class="desc">Re-show the 6-step first-run walkthrough.</div>
            </div>
            <button class="seg"
              @click=${()=>{this.close(),window.dispatchEvent(new CustomEvent(`nv-show-tour`))}}
              style="padding:6px 12px;cursor:pointer;background:var(--bg-3);border:1px solid var(--line);border-radius:6px;color:var(--ink);">
              Replay
            </button>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Reset all preferences</div>
              <div class="desc">Wipe theme, density, motion, scene drag positions, REPL history, and the onboarding-seen flag.</div>
            </div>
            <button class="seg"
              @click=${()=>this.resetPrefs()}
              style="padding:6px 12px;cursor:pointer;background:var(--bg-3);border:1px solid oklch(0.65 0.22 25 / 0.4);border-radius:6px;color:var(--bad);">
              Reset
            </button>
          </div>
        </div>

        <div class="group">
          <h4>About</h4>
          <div class="row" style="border-bottom:0;">
            <div>
              <div class="lbl">nvsim · v0.3.0</div>
              <div class="desc">Open-source NV-diamond simulator. Apache-2.0 OR MIT.<br>
                <a style="color:var(--accent-2); text-decoration:underline dotted; cursor:pointer;"
                  @click=${()=>{this.close(),window.dispatchEvent(new CustomEvent(`nv-show-help`,{detail:{section:`about`}}))}}>
                  More info →
                </a></div>
            </div>
          </div>
        </div>
      </div>
      </div>
    `}};u([r()],Ue.prototype,`open`,void 0),Ue=u([n(`nv-settings-drawer`)],Ue);var Q=[{icon:`👋`,title:`Welcome to nvsim`,body:`<p style="font-size:14px; line-height:1.6;">
        <b>nvsim</b> is an open-source, deterministic forward simulator for
        <b>nitrogen-vacancy diamond magnetometry</b> — a real Rust crate compiled
        to WebAssembly and running in your browser, right now.</p>
      <p style="font-size:13px; color:var(--ink-2); line-height:1.55;">
        This 60-second tour walks you through the four panels, the App Store,
        the Ghost Murmur research view, and the determinism contract that
        makes nvsim distinctive.</p>
      <p style="font-size:11.5px; color:var(--ink-3); line-height:1.5; margin-top:14px;">
        Press <kbd>Esc</kbd> any time to skip. You can replay this tour from
        <b>Settings → Help</b>.</p>`,cta:{label:`Start the tour →`}},{icon:`🌐`,title:`The Scene canvas`,body:`<p>The middle panel shows your <b>magnetic scene</b> — a small simulated
        environment with four sources and one NV-diamond sensor at the centre.</p>
      <p>The four amber/cyan/magenta blobs are draggable: <b>rebar coil</b>
        (steel χ=5000), <b>heart proxy</b> dipole, <b>60 Hz mains</b> current loop,
        and a <b>steel door</b> (eddy current). Field lines connect each source
        to the sensor and animate while the pipeline runs.</p>
      <p style="font-size:12.5px; color:var(--ink-3);">
        Top-left toolbar: zoom in/out, fit-to-view, layer toggles. Bottom-right:
        sim controls (step / play / step / speed cycle). Drag positions persist
        across reloads.</p>`,hint:`Try dragging the heart_proxy after the tour ends.`},{icon:`▶`,title:`Run the pipeline`,body:`<p>Press <b>▶ Run</b> in the topbar (or hit <kbd>Space</kbd>) to start
        the live frame stream. nvsim runs at ~1.8 kHz on x86_64 WASM —
        well above the 1 kHz Cortex-A53 acceptance gate.</p>
      <p>The FPS pill in the topbar updates with the throughput. The B-vector
        trace and frame-stream sparkline in the right inspector update in real
        time.</p>
      <p style="font-size:12.5px; color:var(--ink-3);">
        <kbd>Space</kbd> toggles run/pause from anywhere. Reset (<kbd>⌘R</kbd>)
        rewinds <code>t</code> to 0 without changing the seed.</p>`},{icon:`🔍`,title:`Inspector — three tabs, three depths`,body:`<p>The right rail shows the live inspector: <b>Signal</b> (B-vector
        trace + frame-stream sparkline), <b>Frame</b> (decoded MagFrame fields +
        raw 60-byte hex dump), <b>Witness</b> (SHA-256 determinism gate).</p>
      <p>Click the <b>magnifier</b> icon in the left rail to expand the
        inspector to the full main area, with bigger charts and an explainer
        header. Click the <b>shield</b> icon to do the same focused on Witness.</p>
      <p style="font-size:12.5px; color:var(--ink-3);">
        Number keys <kbd>1</kbd> <kbd>2</kbd> <kbd>3</kbd> jump between the
        three inspector tabs from anywhere.</p>`},{icon:`✓`,title:`The witness — what makes nvsim distinctive`,body:`<p>nvsim's defining commitment: same <code>(scene, config, seed)</code> →
        byte-identical SHA-256 across runs, machines, and transports.</p>
      <p>Click the <b>Witness</b> tab and press <b>Verify witness</b>. The
        dashboard re-derives the hash for the canonical reference scene
        (<code>seed=42, N=256</code>) and asserts it matches the constant
        pinned at compile time
        (<code style="font-size:10.5px;">cc8de9b01b0ff5bd…</code>).</p>
      <p>A green check means every constant — γ_e, D_GS, μ₀, T₂*, contrast,
        the PRNG stream, the frame layout — is byte-identical to the published
        reference. A red ✗ means something drifted; the dashboard names which.</p>`},{icon:`🎚`,title:`Tunables — change the simulation live`,body:`<p>The left sidebar's <b>Tunables</b> panel has four sliders:</p>
      <ul style="margin:0 0 12px; padding-left:18px; font-size:13px; color:var(--ink-2); line-height:1.6;">
        <li><b>Sample rate</b> (1–100 kHz) — digitiser frame rate</li>
        <li><b>Lock-in f_mod</b> (0.1–5 kHz) — microwave modulation freq</li>
        <li><b>Integration t</b> (0.1–10 ms) — per-sample integration time</li>
        <li><b>Shot noise</b> (on/off) — toggle quantum noise</li>
      </ul>
      <p>Edits debounce 300 ms then rebuild the WASM pipeline without restarting
        the frame stream. Watch the noise floor and B-vector spread change
        in the Signal trace.</p>`},{icon:`👻`,title:`Ghost Murmur — research view`,body:`<p>Click the ghost icon in the left rail. This view audits the
        publicly-reported <b>April 2026 CIA Ghost Murmur</b> NV-diamond
        heartbeat-detection program against the open physics literature.</p>
      <p>Includes a <b>"Try it yourself"</b> sandbox: place a cardiac dipole at
        any distance from the sensor, hit Run, and see what the real nvsim
        pipeline recovers. Per-tier detectability bars compare the predicted
        signal vs each transport's noise floor (NV-ensemble lab, COTS DNV-B1,
        SQUID, 60 GHz mmWave, WiFi CSI).</p>
      <p style="font-size:12.5px; color:var(--ink-3);">
        Spoiler: at 1 km the cardiac MCG is ~10⁻¹² of its 10 cm value.
        Press claims of 40-mile detection sit far below any published instrument's
        floor.</p>`},{icon:`🛍`,title:`App Store — 65 edge apps`,body:`<p>Click the grid icon. The <b>App Store</b> catalogues every
        hot-loadable WASM edge module RuView ships, organised by category:
        medical, security, smart-building, retail, industrial, signal,
        learning, autonomy, exotic.</p>
      <p>Each card carries id / category / status / event IDs / compute budget /
        ADR back-reference. The toggle marks an app active in this session;
        the WS transport (when configured) pushes the activation set to a
        connected ESP32 mesh.</p>
      <p style="font-size:12.5px; color:var(--ink-3);">
        Try searching for "ghost", "heart", or "occupancy" to fuzzy-filter
        the catalogue.</p>`},{icon:`⌨`,title:`Console + REPL`,body:`<p>The bottom panel is a structured event log with five filter tabs
        (<b>all / info / warn / err / dbg</b>) plus a REPL prompt.</p>
      <p>REPL commands include
        <code>help</code>, <code>scene.list</code>, <code>sensor.config</code>,
        <code>run</code>, <code>pause</code>, <code>seed [hex]</code>,
        <code>proof.verify</code>, <code>proof.export</code>,
        <code>theme [light|dark]</code>, <code>status</code>, <code>clear</code>.</p>
      <p style="font-size:12.5px; color:var(--ink-3);">
        Press <kbd>/</kbd> to focus the REPL from anywhere. Arrow ↑/↓ recall
        history (persisted across reloads). <kbd>⌘K</kbd> opens the command
        palette with every action discoverable.</p>`},{icon:`🚀`,title:`You are ready`,body:`<p style="font-size:14px;">That's the whole tour. A few last pointers:</p>
      <ul style="margin:0 0 14px; padding-left:18px; font-size:13px; color:var(--ink-2); line-height:1.7;">
        <li>Press <kbd>?</kbd> any time to open the help center
          (Quickstart / Glossary / FAQ / Shortcuts / About).</li>
        <li>Press <kbd>⌘K</kbd> for the command palette.</li>
        <li>Press <kbd>\`</kbd> to toggle the debug HUD.</li>
        <li>Settings (<kbd>⌘,</kbd>) lets you switch theme, density, motion,
          transport, and replay this tour.</li>
      </ul>
      <p style="font-size:12.5px; color:var(--ink-3); line-height:1.55;">
        Source: <code>github.com/ruvnet/RuView</code> · Apache-2.0 OR MIT ·
        ADRs 089/090/091/092/093.</p>`,cta:{label:`Get started →`}}],We=class extends e{constructor(...e){super(...e),this.open=!1,this.step=0,this.show=()=>{this.step=0,this.open=!0,this.setAttribute(`open`,``)}}static{this.styles=t`
    :host {
      position: fixed; inset: 0;
      background: rgba(0, 0, 0, 0.55);
      backdrop-filter: blur(4px);
      z-index: 240;
      display: grid; place-items: center;
      opacity: 0; pointer-events: none;
      /* A dismissed modal must leave the rendering + accessibility tree, not
         just fade to opacity 0. While the 0.18s fade-out runs, automated
         scanners (and mid-animation users) still see the panel, and contrast
         tooling composites the fading text over the dimmed backdrop and
         reports false failures. Delaying visibility by the fade duration keeps
         the animation intact while making the closed state truly absent. */
      visibility: hidden;
      transition: opacity 0.18s, visibility 0s linear 0.18s;
    }
    :host([open]) {
      opacity: 1; pointer-events: auto;
      visibility: visible;
      transition: opacity 0.18s, visibility 0s;
    }
    .card {
      background: var(--bg-1);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      box-shadow: 0 30px 80px -20px rgba(0,0,0,0.7);
      width: min(640px, 94vw);
      max-height: 86vh;
      display: flex; flex-direction: column;
      transform: translateY(12px) scale(0.98);
      transition: transform 0.22s cubic-bezier(0.2,0.7,0.3,1);
      overflow: hidden;
    }
    :host([open]) .card { transform: translateY(0) scale(1); }
    .h {
      padding: 22px 26px 12px;
      display: flex; align-items: flex-start; gap: 14px;
    }
    .h .icon {
      width: 44px; height: 44px;
      border-radius: 12px;
      background: linear-gradient(135deg, oklch(0.78 0.14 70) 0%, oklch(0.55 0.16 30) 100%);
      display: grid; place-items: center;
      font-size: 22px;
      flex-shrink: 0;
      box-shadow: 0 4px 12px -2px oklch(0.55 0.16 30 / 0.35);
    }
    .h .title-wrap { flex: 1; min-width: 0; }
    .h h2 {
      margin: 0;
      font-size: 18px;
      letter-spacing: -0.01em;
      color: var(--ink);
    }
    .h .step-label {
      font-family: var(--mono);
      font-size: 10.5px;
      color: var(--ink-3);
      margin-top: 4px;
      text-transform: uppercase;
      letter-spacing: 0.06em;
    }
    .h .skip {
      width: 28px; height: 28px;
      background: transparent;
      border: 1px solid var(--line);
      border-radius: 6px;
      color: var(--ink-2);
      cursor: pointer;
      flex-shrink: 0;
    }
    .h .skip:hover { color: var(--ink); border-color: var(--line-2); }
    .body {
      padding: 0 26px 16px;
      font-size: 13px;
      color: var(--ink-2);
      line-height: 1.6;
      overflow-y: auto;
      flex: 1;
    }
    .body p { margin: 0 0 12px; }
    .body p:last-child { margin-bottom: 0; }
    .body code, .body kbd {
      font-family: var(--mono);
      font-size: 11.5px;
      padding: 1px 5px;
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 4px;
    }
    .body code { color: var(--accent); }
    .body kbd { color: var(--ink); }
    .hint {
      margin: 14px 0 0;
      padding: 10px 12px;
      background: oklch(0.78 0.12 195 / 0.06);
      border: 1px solid oklch(0.78 0.12 195 / 0.25);
      border-radius: 8px;
      font-size: 12px;
      color: var(--accent-2);
      display: flex; gap: 8px; align-items: flex-start;
    }
    .hint::before {
      content: '💡';
      flex-shrink: 0;
    }
    .footer {
      display: flex; align-items: center; gap: 14px;
      padding: 14px 22px;
      border-top: 1px solid var(--line);
      background: var(--bg-1);
    }
    .progress { flex: 1; }
    .dots { display: flex; gap: 5px; margin-bottom: 4px; }
    .dot {
      width: 6px; height: 6px; border-radius: 50%;
      background: var(--bg-3);
      border: 1px solid var(--line-2);
      transition: background 0.15s, border-color 0.15s, transform 0.15s;
    }
    .dot.active {
      background: var(--accent);
      border-color: var(--accent);
      transform: scale(1.2);
    }
    .dot.done {
      background: var(--accent-4);
      border-color: var(--accent-4);
    }
    .progress-label {
      font-family: var(--mono);
      font-size: 10px;
      color: var(--ink-3);
    }
    button.primary, button.ghost {
      padding: 9px 16px;
      border-radius: 8px;
      font-size: 13px;
      font-weight: 500;
      cursor: pointer;
      font-family: inherit;
      border: 1px solid var(--line);
      background: var(--bg-2);
      color: var(--ink);
    }
    button.ghost:hover { border-color: var(--line-2); }
    button.primary {
      background: var(--accent);
      border-color: var(--accent);
      color: #1a0f00;
    }
    button.primary:hover { filter: brightness(1.08); }
  `}async connectedCallback(){super.connectedCallback(),window.addEventListener(`nv-show-tour`,this.show),await K(`onboarding-seen`)||(this.open=!0,this.setAttribute(`open`,``)),this.setAttribute(`ready`,``)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener(`nv-show-tour`,this.show)}async dismiss(){this.open=!1,this.removeAttribute(`open`),await q(`onboarding-seen`,!0)}next(){Q[this.step].cta?.run?.(),this.step<Q.length-1?this.step++:this.dismiss()}prev(){this.step>0&&this.step--}render(){let e=Q[this.step],t=this.step===Q.length-1;return i`
      <div class="card" role="dialog" aria-modal="true" aria-label="Welcome tour">
        <div class="h">
          <div class="icon" aria-hidden="true">${e.icon}</div>
          <div class="title-wrap">
            <h2>${e.title}</h2>
            <div class="step-label">Step ${this.step+1} of ${Q.length}</div>
          </div>
          <button class="skip" @click=${()=>this.dismiss()} aria-label="Skip tour" title="Skip tour">×</button>
        </div>
        <div class="body">
          <div .innerHTML=${e.body}></div>
          ${e.hint?i`<div class="hint">${e.hint}</div>`:``}
        </div>
        <div class="footer">
          <div class="progress">
            <div class="dots">
              ${Q.map((e,t)=>i`
                <div class="dot ${t===this.step?`active`:t<this.step?`done`:``}"></div>
              `)}
            </div>
            <div class="progress-label">${this.step+1} / ${Q.length}</div>
          </div>
          ${this.step>0?i`<button class="ghost" @click=${()=>this.prev()}>← Back</button>`:i`<button class="ghost" @click=${()=>this.dismiss()}>Skip</button>`}
          <button class="primary" @click=${()=>this.next()}>
            ${e.cta?.label??(t?`Done`:`Next →`)}
          </button>
        </div>
      </div>
    `}};u([r()],We.prototype,`open`,void 0),u([r()],We.prototype,`step`,void 0),We=u([n(`nv-onboarding`)],We);var Ge=[{id:`nvBest`,label:`NV-ensemble (best lab)`,floorT:1e-12,color:`oklch(0.78 0.14 70)`},{id:`nvCots`,label:`NV-DNV-B1 (COTS)`,floorT:3e-10,color:`oklch(0.72 0.18 50)`},{id:`squid`,label:`SQUID (shielded room)`,floorT:1e-15,color:`oklch(0.78 0.12 195)`},{id:`mmw`,label:`60 GHz mmWave (μ-Doppler)`,floorT:0,color:`oklch(0.78 0.14 145)`},{id:`csi`,label:`WiFi CSI (presence)`,floorT:0,color:`oklch(0.72 0.18 330)`}],$=class extends e{constructor(...e){super(...e),this.distanceM=.1,this.momentLog10=-8.3,this.result=null,this.running=!1,this.err=null}static{this.styles=t`
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
  `}predictedDipoleFieldT(e,t){return 4*Math.PI*1e-7*t/(4*Math.PI*Math.max(e,1e-6)**3)}async runDemo(){let e=L();if(!e)this.err=`WASM client not ready`;else{this.err=null,this.running=!0,this.requestUpdate();try{let t=this.distanceM,n=10**this.momentLog10,r={dipoles:[{position:[0,0,t],moment:[0,0,n]}],loops:[],ferrous:[],eddy:[],sensors:[[0,0,0]],ambient_field:[0,0,0]},i={digitiser:{f_s_hz:1e4,f_mod_hz:1e3},sensor:{gamma_fwhm_hz:1e6,t1_s:.005,t2_s:1e-6,t2_star_s:2e-7,contrast:.03,n_spins:0xe8d4a51000,shot_noise_disabled:!1},dt_s:null};this.result=await e.runTransient(r,i,42n,64),z(`ok`,`ghost-demo · r=${t.toFixed(3)} m · |B| recovered = ${(this.result.bMagT*0xe8d4a51000).toExponential(2)} pT`)}catch(e){this.err=e.message,z(`err`,`ghost-demo failed: ${this.err}`)}finally{this.running=!1,this.requestUpdate()}}}formatField(e){if(e===0)return`0 T`;let t=Math.abs(e);return t>=.001?`${(e*1e3).toFixed(2)} mT`:t>=1e-6?`${(e*1e6).toFixed(2)} µT`:t>=1e-9?`${(e*1e9).toFixed(3)} nT`:t>=1e-12?`${(e*0xe8d4a51000).toFixed(2)} pT`:t>=1e-15?`${(e*0x38d7ea4c68000).toFixed(2)} fT`:t>=1e-18?`${(e*0xde0b6b3a7640000).toFixed(2)} aT`:`${e.toExponential(2)} T`}formatDistance(e){return e<1?`${(e*100).toFixed(1)} cm`:e<1e3?`${e.toFixed(2)} m`:e<1e5?`${(e/1e3).toFixed(2)} km`:`${(e/1609).toFixed(0)} mi`}renderDemo(){let e=10**this.momentLog10,t=this.predictedDipoleFieldT(this.distanceM,e),n=this.result?.bMagT??0,r=(this.result?.noiseFloorPtSqrtHz??0)*1e-12,a=Ge.map(e=>{let n=`bad`,r=`below floor`;if(e.id===`mmw`)this.distanceM<=5?(n=`ok`,r=`µ-Doppler @ chest`):this.distanceM<=15?(n=`warn`,r=`edge of range`):(n=`bad`,r=`out of range`);else if(e.id===`csi`)this.distanceM<=30?(n=this.distanceM<=10?`ok`:`warn`,r=`presence/breathing`):(n=`bad`,r=`out of range`);else if(e.floorT>0){let i=t/e.floorT;i>100?(n=`ok`,r=`${i.toExponential(1)}× floor`):i>1?(n=`warn`,r=`${i.toFixed(1)}× floor`):(n=`bad`,r=`${(1/i).toExponential(1)}× too weak`)}let a=e.floorT>0?Math.max(2,Math.min(100,100+12*Math.log10(t/e.floorT))):e.id===`mmw`?Math.max(2,100-this.distanceM*7):Math.max(2,100-this.distanceM*2);return i`
        <div class="tier-bar" data-tier=${e.id}>
          <div class="fill" style=${`width:${a}%; background:${e.color}; border-color:${e.color}`}></div>
          <div class="lbl">
            <span>${e.label}</span>
            <span class="verdict-${n}" style=${`color:${n===`ok`?`var(--ok)`:n===`warn`?`var(--warn)`:`var(--bad)`}`}>${r}</span>
          </div>
        </div>
      `}),o=t>1e-12?`ok`:t>1e-15?`warn`:`bad`,s=o===`ok`?`Above NV-ensemble lab floor — close-range MCG plausible at ${this.formatDistance(this.distanceM)}.`:o===`warn`?`Below NV ensemble best, above SQUID — research-grade only at ${this.formatDistance(this.distanceM)}.`:`Below every published instrument's noise floor at ${this.formatDistance(this.distanceM)}. Press-release physics.`;return i`
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
              <input type="range" id="demo-distance" aria-label="Distance from sensor"
                min="-2" max="5" step="0.05"
                .value=${String(Math.log10(this.distanceM))}
                @input=${e=>{this.distanceM=10**e.target.value}} />
              <div style="font-size: 10.5px; color: var(--ink-3); margin-top: 4px; font-family: var(--mono);">
                10 cm → 100 km log scale
              </div>
            </div>
            <div class="control">
              <div class="top">
                <span class="lbl">Heart dipole moment</span>
                <span class="val" id="demo-moment-val">${e.toExponential(2)} A·m²</span>
              </div>
              <input type="range" id="demo-moment" aria-label="Heart dipole moment"
                min="-10" max="-6" step="0.05"
                .value=${String(this.momentLog10)}
                @input=${e=>{this.momentLog10=+e.target.value}} />
              <div style="font-size: 10.5px; color: var(--ink-3); margin-top: 4px; font-family: var(--mono);">
                published cardiac MCG ≈ 5×10⁻⁹ A·m²
              </div>
            </div>
            <button class="demo-btn" id="demo-run-btn" ?disabled=${this.running}
              @click=${()=>this.runDemo()}>
              ${this.running?`Running nvsim…`:`▶ Run nvsim at this distance`}
            </button>
            ${this.err?i`<div class="verdict bad" style="margin-top: 10px;">Error: ${this.err}</div>`:``}
          </div>

          <div>
            <div class="readout">
              <div class="readout-row">
                <span class="l">Predicted |B| (1/r³)</span>
                <span class="v amber" id="demo-predicted">${this.formatField(t)}</span>
              </div>
              <div class="readout-row">
                <span class="l">Recovered |B| (nvsim)</span>
                <span class="v" id="demo-recovered">${this.result?this.formatField(n):`—`}</span>
              </div>
              <div class="readout-row">
                <span class="l">Sensor noise floor</span>
                <span class="v" id="demo-floor">${this.result?this.formatField(r)+`/√Hz`:`—`}</span>
              </div>
              <div class="readout-row">
                <span class="l">Frames run</span>
                <span class="v" id="demo-frames">${this.result?.nFrames??`—`}</span>
              </div>
              <div class="readout-row">
                <span class="l">Witness (this run)</span>
                <span class="v" style="font-size: 10px;" id="demo-witness">${this.result?.witnessHex.slice(0,16)??`—`}…</span>
              </div>
            </div>
            <div style="margin-top: 14px;">
              <div style="font-size: 11.5px; color: var(--ink-3); text-transform: uppercase; letter-spacing: 0.06em; margin-bottom: 8px;">
                Per-tier detectability
              </div>
              ${a}
            </div>
          </div>
        </div>
        <div class="verdict ${o}" id="demo-verdict">${s}</div>
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
    `}render(){return i`
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
      <div class="architecture" tabindex="0" role="group" aria-label="RuView three-tier mesh architecture">                      ┌──────────────────────────┐
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
    `}};u([r()],$.prototype,`distanceM`,void 0),u([r()],$.prototype,`momentLog10`,void 0),u([r()],$.prototype,`result`,void 0),u([r()],$.prototype,`running`,void 0),u([r()],$.prototype,`err`,void 0),$=u([n(`nv-ghost-murmur`)],$);var Ke=[{term:`NV-diamond`,category:`physics`,body:`Nitrogen-vacancy defect in synthetic diamond. The simulator models a 1 mm³ ensemble (~10¹² centers) addressed by 532 nm pump light + a 2.87 GHz microwave drive. Used as a room-temperature magnetometer with shot-noise floor ~1 pT/√Hz at the published lab record.`},{term:`CW-ODMR`,category:`physics`,body:`Continuously-driven optically-detected magnetic resonance. Sweep the microwave frequency around the NV zero-field splitting (D = 2.87 GHz) and watch the photoluminescence dip when the microwave matches the spin transition. The dip splits with applied magnetic field along each of the four ⟨111⟩ NV axes.`},{term:`MagFrame`,category:`rust`,body:`Fixed-layout 60-byte binary record nvsim emits per (sensor × sample). Magic 0xC51A_6E70, version 1, little-endian. Carries timestamp, recovered B vector (pT), per-axis sigma, noise floor, and flag bits for saturation / shot-noise-disabled / heavy-attenuation.`},{term:`Witness`,category:`rust`,body:`SHA-256 hash over the concatenated MagFrame bytes for a canonical reference run (Proof::REFERENCE_SCENE_JSON @ seed=42, N=256). Same inputs → same hash, byte-for-byte, across runs and machines. The dashboard re-derives it in WASM and compares against Proof::EXPECTED_WITNESS_HEX pinned at build time.`},{term:`Determinism gate`,category:`rust`,body:`A pass/fail check: did this build of nvsim produce the expected witness? If yes → every constant (γ_e, D_GS, μ₀, contrast, T₂*, the PRNG stream, the frame layout, the pipeline ordering) is byte-identical to the published reference. If no → something drifted; the dashboard names which.`},{term:`Lock-in demod`,category:`physics`,body:`Multiply the photoluminescence signal by cos(2π·f_mod·t) and low-pass to recover the slowly-varying B-field component. The simulator emulates a lock-in with output gain 2 and a single-pole IIR LP filter; settable via the Tunables panel (f_mod default 1 kHz).`},{term:`Shot-noise floor`,category:`physics`,body:`δB = 1 / (γ_e · C · √(N · t · T₂*)) — the irreducible quantum noise floor for an NV ensemble. With nvsim defaults (N=10¹², C=0.03, T₂*=200 ns): ≈1.18 pT/√Hz. Toggleable via the Tunables panel for "analytic" runs without noise.`},{term:`Biot-Savart`,category:`physics`,body:`Closed-form magnetic field at a point from a current loop or a magnetic dipole. The Scene panel's sources (heart proxy, mains loop, ferrous body, eddy current) all reduce to Biot-Savart-style superpositions over the sensor position.`},{term:`Multistatic fusion`,category:`physics`,body:`Combining evidence from multiple sensors at known geometric configurations. RuView's Cramer-Rao-weighted attention over WiFi CSI nodes + 60 GHz radar nodes + (hypothetically) NV nodes; documented in ADR-029 and the Ghost Murmur view.`},{term:`Scene`,category:`ui`,body:`The simulated magnetic environment: a list of sources (dipole, current loop, ferrous body, eddy current) plus one or more sensor positions and an ambient field. The dashboard ships a "rebar-walkby-01" reference scene; click "New scene…" in the command palette (⌘K) to build your own.`},{term:`Tunables`,category:`ui`,body:`Sliders that change the running pipeline's digitiser config. Each edit debounces 300 ms, then rebuilds the WASM pipeline with the new f_s / f_mod / dt / shot-noise setting. The frame stream picks up the change without a restart.`},{term:`Transport`,category:`ui`,body:`How the dashboard talks to nvsim. Default is WASM — the simulator runs in a Web Worker right here in your browser, no server. The optional WS transport is REST + binary WebSocket against a host-supplied nvsim-server (see ADR-092 §6.2). Toggle in Settings.`},{term:`App Store`,category:`ui`,body:`Catalog of all 65+ hot-loadable WASM edge modules from wifi-densepose-wasm-edge plus the simulators. Each card carries id / category / status / event IDs; the toggle marks an app active in this session and (in WS mode) pushes the activation to a connected ESP32 mesh.`},{term:`Ghost Murmur`,category:`ui`,body:`Research view that audits the publicly-reported April 2026 CIA NV-diamond heartbeat detector against the open physics literature. Includes a live "Try it yourself" sandbox where you can place a heart dipole at any distance from the sensor and ask: which transport tier would actually detect it?`}],qe=[{q:`Is this a real simulator or a mockup?`,a:"Real. The Rust crate at v2/crates/nvsim is the same code that runs in the browser via WASM. Press <b>Verify witness</b> on the Witness panel — the SHA-256 you see is byte-equivalent to what `cargo test -p nvsim` produces."},{q:`Why does my "Recovered |B|" sit much higher than "Predicted |B|" in the Ghost Murmur demo?`,a:`The recovered value reads the simulator's ADC quantization floor, not the actual magnetic signal. With COTS-default sensor noise (~300 pT/√Hz) and 16-bit ADC at ±10 µT FS, anything below ~1 pT vanishes into ~2 nT of digitization residual. That's the lesson — the press claim sits far below this floor at any meaningful range.`},{q:`Can I run my own scene?`,a:`Yes. Press ⌘K to open the command palette and pick "New scene…". You get five fields (name, dipole moment, distance, ferrous toggle, mains toggle); the dashboard builds the JSON and pushes it via <code>client.loadScene()</code>.`},{q:`Does any of my data leave the browser?`,a:`No. WASM mode is local-only — the worker, the WASM binary, and the IndexedDB persistence all live in your browser. The optional WS transport (off by default) talks to a host of your choosing.`},{q:`What does the witness mismatch (red ✗) mean?`,a:`The current build of nvsim produced a SHA-256 that doesn't match the constant pinned at compile time. Possible causes: a different Rust toolchain, a dependency version drift, a manual edit to a physics constant, or an honest bug. Audit the diff against ADR-089 §5.`},{q:`Why are the Inspector / Witness rail buttons there if there's already a right-side inspector?`,a:`The right-side inspector is the compact live view; the rail buttons open a full-width version with bigger charts, an explainer header, reference-scene metadata cards, and (on Witness) a "what this verifies" panel. Both stay in sync — the right rail is for glancing, the main area is for diving in.`},{q:`Why is there an "App Store" if this is a magnetometer simulator?`,a:`Because nvsim is one tile in a larger sensing platform. The catalog lists every hot-loadable WASM edge module RuView ships — medical, security, building, retail, industrial, signal, learning, autonomy. The simulators (nvsim today, more in future) are first-class entries in the same catalog.`}],Je=[{step:1,title:`Hit ▶ Run`,body:`The big amber button in the topbar starts the live frame stream. The pipeline runs ~1.8 kHz on x86_64 WASM, well above the 1 kHz Cortex-A53 acceptance gate.`},{step:2,title:`Watch the B-vector trace`,body:`The Inspector → Signal tab shows the recovered field per axis updating in real time. The frame strip below it is one bar per ~32-frame batch.`},{step:3,title:`Verify the witness`,body:`Click the rail Witness button (or REPL: <code>proof.verify</code>). The dashboard re-runs the canonical reference scene and asserts the SHA-256 byte-for-byte.`},{step:4,title:`Drag a source`,body:`Grab the rebar / heart proxy / mains loop / ferrous door in the scene canvas; positions persist via IndexedDB.`},{step:5,title:`Tweak the tunables`,body:`Sliders in the left sidebar update the running pipeline (f_s, f_mod, integration time, shot-noise). Changes debounce 300 ms then push to the worker.`},{step:6,title:`Open the Ghost Murmur view`,body:`The ghost icon in the rail. Move the distance + moment sliders, hit "Run nvsim at this distance" — the live demo runs the real Rust pipeline through WASM and shows which transport tier would actually detect.`},{step:7,title:`Browse the App Store`,body:`The grid icon. 65+ edge apps: medical, security, building, retail, industrial, signal, learning. Toggle to mark active in this session.`}],Ye=[{keys:`⌘K  /  Ctrl K`,label:`Command palette`},{keys:`Space`,label:`Play / pause pipeline`},{keys:`⌘R  /  Ctrl R`,label:`Reset pipeline (with confirm)`},{keys:`⌘,  /  Ctrl ,`,label:`Settings drawer`},{keys:`⌘N  /  Ctrl N`,label:`New scene`},{keys:`⌘E  /  Ctrl E`,label:`Export proof bundle`},{keys:`⌘/  /  Ctrl /`,label:`Toggle theme (dark / light)`},{keys:"`",label:`Toggle debug HUD`},{keys:`?`,label:`Open this help center`},{keys:`1 · 2 · 3`,label:`Switch inspector tab (Signal / Frame / Witness)`},{keys:`Esc`,label:`Close any modal / palette / drawer`},{keys:`/`,label:`Focus the REPL prompt`}],Xe=class extends e{constructor(...e){super(...e),this.open=!1,this.section=`quickstart`,this.query=``,this.closeListener=()=>this.close(),this.show=e=>{let t=e.detail;t?.section&&(this.section=t.section),this.open=!0,this.setAttribute(`open`,``)},this.onKey=e=>{let t=e.target,n=t?.tagName===`INPUT`||t?.tagName===`TEXTAREA`;e.key===`?`&&!n&&!e.ctrlKey&&!e.metaKey?(e.preventDefault(),this.show(new CustomEvent(`nv-show-help`))):e.key===`Escape`&&this.open&&this.close()}}static{this.styles=t`
    :host {
      position: fixed; inset: 0;
      background: rgba(0, 0, 0, 0.55);
      backdrop-filter: blur(4px);
      z-index: 230;
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
      width: min(880px, 94vw);
      max-height: 86vh;
      display: grid;
      grid-template-columns: 200px 1fr;
      grid-template-rows: auto 1fr auto;
      overflow: hidden;
      transform: translateY(12px) scale(0.98);
      transition: transform 0.22s cubic-bezier(0.2,0.7,0.3,1);
    }
    :host([open]) .modal { transform: translateY(0) scale(1); }
    @media (max-width: 700px) {
      .modal { grid-template-columns: 1fr; grid-template-rows: auto auto 1fr auto; max-height: 92vh; }
      .nav { border-right: 0; border-bottom: 1px solid var(--line); flex-direction: row; overflow-x: auto; }
      .nav button { white-space: nowrap; }
    }
    .h {
      grid-column: 1 / -1;
      padding: 14px 18px;
      border-bottom: 1px solid var(--line);
      display: flex; align-items: center; justify-content: space-between;
    }
    .h .ttl { font-size: 15px; font-weight: 600; }
    .nav {
      border-right: 1px solid var(--line);
      padding: 12px 8px;
      display: flex; flex-direction: column; gap: 2px;
      background: var(--bg-1);
    }
    .nav button {
      text-align: left;
      padding: 8px 12px;
      background: transparent;
      border: 1px solid transparent;
      border-radius: 6px;
      color: var(--ink-3);
      font-size: 12.5px;
      cursor: pointer;
      transition: color 0.15s, background 0.15s;
    }
    .nav button:hover { color: var(--ink); background: var(--bg-2); }
    .nav button.on {
      color: var(--ink); background: var(--bg-3);
      border-color: var(--line-2);
    }
    .body {
      padding: 18px 22px;
      overflow-y: auto;
      font-size: 13px;
      color: var(--ink-2);
      line-height: 1.6;
    }
    .body h2 {
      margin: 0 0 8px;
      font-size: 18px;
      color: var(--ink);
      letter-spacing: -0.01em;
    }
    .body .lead {
      color: var(--ink-3);
      font-size: 12.5px;
      margin: 0 0 14px;
    }
    .body p { margin: 0 0 12px; }
    .body code {
      font-family: var(--mono);
      background: var(--bg-3);
      padding: 1px 5px;
      border-radius: 4px;
      font-size: 11.5px;
      color: var(--accent);
    }
    .body kbd {
      font-family: var(--mono);
      padding: 2px 6px;
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 4px;
      font-size: 11.5px;
      color: var(--ink);
    }
    .step {
      display: grid;
      grid-template-columns: 32px 1fr;
      gap: 12px;
      padding: 10px 0;
      border-bottom: 1px solid var(--line);
    }
    .step:last-child { border-bottom: 0; }
    .step .num {
      width: 26px; height: 26px;
      border-radius: 50%;
      background: var(--accent);
      color: #1a0f00;
      font-family: var(--mono);
      font-size: 12.5px;
      font-weight: 700;
      display: grid; place-items: center;
    }
    .step .ttl { color: var(--ink); font-weight: 600; font-size: 13.5px; margin-bottom: 2px; }
    .step .body-text { font-size: 12.5px; color: var(--ink-2); line-height: 1.55; }
    .glossary-search {
      width: 100%;
      padding: 8px 12px;
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 6px;
      font-family: var(--mono);
      font-size: 12.5px;
      color: var(--ink);
      outline: none;
      margin-bottom: 14px;
    }
    .glossary-search:focus { border-color: var(--accent); }
    .term {
      padding: 10px 0;
      border-bottom: 1px solid var(--line);
    }
    .term:last-child { border-bottom: 0; }
    .term .head {
      display: flex; align-items: center; gap: 8px; margin-bottom: 4px;
    }
    .term .name {
      font-family: var(--mono);
      font-size: 13.5px;
      color: var(--accent);
      font-weight: 600;
    }
    .term .badge {
      font-family: var(--mono);
      font-size: 9.5px;
      padding: 1px 6px;
      border-radius: 4px;
      border: 1px solid var(--line);
      text-transform: uppercase;
      letter-spacing: 0.04em;
    }
    .term .badge.physics { color: var(--accent-2); border-color: oklch(0.78 0.12 195 / 0.4); }
    .term .badge.rust { color: var(--accent); border-color: oklch(0.78 0.14 70 / 0.4); }
    .term .badge.ui { color: var(--accent-4); border-color: oklch(0.78 0.14 145 / 0.4); }
    .term .body-text {
      font-size: 12.5px;
      color: var(--ink-2);
      line-height: 1.55;
    }
    .faq-item {
      padding: 10px 0;
      border-bottom: 1px solid var(--line);
    }
    .faq-item:last-child { border-bottom: 0; }
    .faq-item .q {
      color: var(--ink);
      font-weight: 600;
      font-size: 13.5px;
      margin-bottom: 4px;
    }
    .faq-item .a { font-size: 12.5px; color: var(--ink-2); line-height: 1.55; }
    .shortcuts {
      display: grid;
      grid-template-columns: auto 1fr;
      gap: 8px 16px;
      align-items: baseline;
    }
    .f {
      grid-column: 1 / -1;
      padding: 10px 18px;
      border-top: 1px solid var(--line);
      display: flex; align-items: center; justify-content: space-between;
      font-size: 11.5px; color: var(--ink-3);
    }
    .close {
      width: 28px; height: 28px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px;
      color: var(--ink-2);
      cursor: pointer;
    }
    .close:hover { color: var(--ink); border-color: var(--line-2); }
  `}connectedCallback(){super.connectedCallback(),window.addEventListener(`nv-show-help`,this.show),window.addEventListener(`nv-show-help-close`,this.closeListener),window.addEventListener(`keydown`,this.onKey)}disconnectedCallback(){super.disconnectedCallback(),window.removeEventListener(`nv-show-help`,this.show),window.removeEventListener(`nv-show-help-close`,this.closeListener),window.removeEventListener(`keydown`,this.onKey)}close(){this.open=!1,this.removeAttribute(`open`)}filteredGlossary(){if(!this.query.trim())return Ke;let e=this.query.toLowerCase();return Ke.filter(t=>t.term.toLowerCase().includes(e)||t.body.toLowerCase().includes(e))}renderQuickstart(){return i`
      <h2>Quickstart</h2>
      <p class="lead">Seven taps to get from "I just opened the dashboard" to "I'm running my own scene with verified determinism."</p>
      <button
        style="display:inline-flex; align-items:center; gap:8px; padding:10px 16px; margin-bottom:14px; background:var(--accent); color:#1a0f00; border:none; border-radius:8px; font-size:13px; font-weight:600; cursor:pointer; font-family:inherit;"
        @click=${()=>{window.dispatchEvent(new CustomEvent(`nv-show-help-close`)),window.dispatchEvent(new CustomEvent(`nv-show-tour`))}}>
        ★ Take the interactive 10-step tour
      </button>
      ${Je.map(e=>i`
        <div class="step">
          <div class="num">${e.step}</div>
          <div>
            <div class="ttl">${e.title}</div>
            <div class="body-text" .innerHTML=${e.body}></div>
          </div>
        </div>
      `)}
    `}renderGlossary(){let e=this.filteredGlossary();return i`
      <h2>Glossary</h2>
      <p class="lead">Every piece of jargon in the dashboard, defined in one paragraph each.</p>
      <input class="glossary-search" type="text" placeholder="Search 14 terms…"
        .value=${this.query}
        @input=${e=>this.query=e.target.value} />
      ${e.length===0?i`<p style="color: var(--ink-3);">No terms match.</p>`:e.map(e=>i`
            <div class="term">
              <div class="head">
                <span class="name">${e.term}</span>
                <span class="badge ${e.category}">${e.category}</span>
              </div>
              <div class="body-text">${e.body}</div>
            </div>
          `)}
    `}renderFaq(){return i`
      <h2>FAQ</h2>
      <p class="lead">The questions I was asked twice in the first week of demos.</p>
      ${qe.map(e=>i`
        <div class="faq-item">
          <div class="q">${e.q}</div>
          <div class="a" .innerHTML=${e.a}></div>
        </div>
      `)}
    `}renderShortcuts(){return i`
      <h2>Keyboard shortcuts</h2>
      <p class="lead">Everything is reachable without a mouse.</p>
      <div class="shortcuts">
        ${Ye.map(e=>i`
          <kbd>${e.keys}</kbd><span>${e.label}</span>
        `)}
      </div>
    `}renderAbout(){return i`
      <h2>About this dashboard</h2>
      <p class="lead">What you're looking at, in one screen.</p>
      <p><b>nvsim</b> is a deterministic forward simulator for nitrogen-vacancy diamond magnetometry.
        The Rust crate at <code>v2/crates/nvsim</code> is the source of truth; this dashboard is a
        Vite + Lit single-page app that ships the crate compiled to WebAssembly inside a Web Worker.</p>
      <p>The defining commitment is <b>determinism</b>: same <code>(scene, config, seed)</code> →
        byte-identical SHA-256 witness across browsers, OSes, and transports. Press the
        <kbd>Verify witness</kbd> button on the Witness tab to assert this live.</p>
      <p>The codebase is open source (Apache-2.0 OR MIT). Find it on GitHub:
        <code>github.com/ruvnet/RuView</code>. Decisions are documented in ADRs 089 (nvsim),
        090 (Lindblad extension, conditional), 091 (sub-THz radar research),
        092 (this dashboard), 093 (UX gap analysis).</p>
      <p>This dashboard is one of several RuView demos. Sibling demos at
        <code>github.io/RuView/</code> include the Observatory and Pose Fusion views.</p>
    `}render(){return i`
      <div class="modal" role="dialog" aria-modal="true" aria-label="Help center">
        <div class="h">
          <div class="ttl">Help</div>
          <button class="close" aria-label="Close help" @click=${()=>this.close()}>×</button>
        </div>
        <nav class="nav" role="tablist" aria-label="Help sections">
          ${[`quickstart`,`glossary`,`faq`,`shortcuts`,`about`].map(e=>i`
            <button class=${this.section===e?`on`:``} role="tab"
              aria-selected=${this.section===e}
              @click=${()=>this.section=e}>
              ${e===`quickstart`?`🚀 Quickstart`:e===`glossary`?`📖 Glossary`:e===`faq`?`? FAQ`:e===`shortcuts`?`⌨ Shortcuts`:`ℹ About`}
            </button>
          `)}
        </nav>
        <div class="body" role="tabpanel">
          ${this.section===`quickstart`?this.renderQuickstart():this.section===`glossary`?this.renderGlossary():this.section===`faq`?this.renderFaq():this.section===`shortcuts`?this.renderShortcuts():this.renderAbout()}
        </div>
        <div class="f">
          <span>Press <kbd style="font-family:var(--mono);font-size:10.5px;padding:1px 4px;background:var(--bg-3);border:1px solid var(--line);border-radius:3px;">?</kbd> any time to reopen</span>
          <span>nvsim · Apache-2.0 OR MIT</span>
        </div>
      </div>
    `}};u([r()],Xe.prototype,`open`,void 0),u([r()],Xe.prototype,`section`,void 0),u([r()],Xe.prototype,`query`,void 0),Xe=u([n(`nv-help`)],Xe);var Ze=class extends e{static{this.styles=t`
    :host {
      display: block;
      height: 100%;
      overflow-y: auto;
      background: radial-gradient(ellipse at 50% 30%, var(--bg-2) 0%, var(--bg-0) 70%);
      padding: 28px clamp(16px, 6vw, 56px) 60px;
    }
    .hero {
      max-width: 800px;
      margin: 16px auto 28px;
      text-align: center;
    }
    .hero .icon {
      width: 56px; height: 56px;
      margin: 0 auto 18px;
      border-radius: 14px;
      background: linear-gradient(135deg, oklch(0.78 0.14 70) 0%, oklch(0.55 0.16 30) 100%);
      display: grid; place-items: center;
      font-family: var(--mono);
      font-weight: 700;
      font-size: 18px;
      color: #1a0f00;
      box-shadow: 0 8px 24px -6px oklch(0.55 0.16 30 / 0.4);
    }
    .hero h1 {
      margin: 0 0 8px;
      font-size: clamp(24px, 4vw, 34px);
      letter-spacing: -0.02em;
      color: var(--ink);
      line-height: 1.15;
    }
    .hero .tag {
      font-size: clamp(13px, 1.6vw, 15px);
      color: var(--ink-2);
      margin: 0 0 22px;
      line-height: 1.55;
    }
    .hero .ctas {
      display: flex; flex-wrap: wrap; gap: 8px;
      justify-content: center;
    }
    .cta {
      padding: 11px 20px;
      border-radius: 10px;
      font-size: 14px;
      font-weight: 600;
      cursor: pointer;
      font-family: inherit;
      border: 1px solid var(--line);
      background: var(--bg-2);
      color: var(--ink);
      transition: transform 0.12s, border-color 0.12s, filter 0.12s;
    }
    .cta:hover { transform: translateY(-1px); border-color: var(--line-2); }
    .cta.primary {
      background: var(--accent);
      border-color: var(--accent);
      color: #1a0f00;
    }
    .cta.primary:hover { filter: brightness(1.08); }
    .status {
      display: inline-flex; align-items: center; gap: 8px;
      padding: 6px 12px;
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: 999px;
      font-size: 12px;
      font-family: var(--mono);
      color: var(--ink-2);
      margin-top: 18px;
    }
    .status .dot {
      width: 8px; height: 8px; border-radius: 50%;
      background: var(--ink-3);
    }
    .status.live .dot {
      background: var(--ok);
      box-shadow: 0 0 8px var(--ok);
      animation: pulse 2s infinite;
    }
    @keyframes pulse { 50% { opacity: 0.5; } }

    .grid {
      max-width: 980px;
      margin: 36px auto 0;
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: 14px;
    }
    .card {
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: var(--radius);
      padding: 18px 20px;
      cursor: pointer;
      transition: transform 0.12s, border-color 0.12s, background 0.12s;
      display: flex; flex-direction: column; gap: 6px;
      text-align: left;
      color: inherit;
    }
    .card:hover {
      transform: translateY(-2px);
      border-color: var(--accent);
      background: linear-gradient(180deg, var(--bg-2) 0%, oklch(0.78 0.14 70 / 0.04) 100%);
    }
    .card .ico {
      font-size: 22px;
      line-height: 1;
      margin-bottom: 4px;
    }
    .card h3 {
      margin: 0;
      font-size: 14.5px;
      font-weight: 600;
      color: var(--ink);
      letter-spacing: -0.01em;
    }
    .card p {
      margin: 0;
      font-size: 12.5px;
      color: var(--ink-2);
      line-height: 1.55;
    }
    .card .arrow {
      color: var(--accent);
      font-family: var(--mono);
      font-size: 11.5px;
      margin-top: 6px;
    }

    .footnote {
      max-width: 800px;
      margin: 36px auto 0;
      text-align: center;
      font-size: 12px;
      color: var(--ink-3);
      line-height: 1.55;
    }
    .footnote code {
      font-family: var(--mono);
      background: var(--bg-3);
      padding: 1px 5px;
      border-radius: 4px;
      color: var(--accent);
      font-size: 11px;
    }
    .footnote a {
      color: var(--accent-2);
      text-decoration: underline dotted;
      cursor: pointer;
    }
  `}connectedCallback(){super.connectedCallback(),c(()=>{h.value,j.value,k.value,this.requestUpdate()})}go(e){e===`tour`?window.dispatchEvent(new CustomEvent(`nv-show-tour`)):e===`help`?window.dispatchEvent(new CustomEvent(`nv-show-help`)):this.dispatchEvent(new CustomEvent(`navigate`,{detail:e,bubbles:!0,composed:!0}))}async runDemo(){let e=L();e&&(h.value||(await e.run(),h.value=!0,z(`ok`,`demo started · streaming MagFrames`)))}render(){let e=h.value,t=j.value===`ok`;return i`
      <div class="hero">
        <div class="icon" aria-hidden="true">NV</div>
        <h1>An open-source quantum-magnetometer simulator, in your browser.</h1>
        <p class="tag">
          nvsim runs a real Rust simulator (the same code that
          <code style="font-family:var(--mono); background:var(--bg-3); padding:1px 5px; border-radius:4px; color:var(--accent); font-size:12px;">cargo&nbsp;test</code>
          uses) entirely in WebAssembly. No server, no upload, no telemetry.
          Press the button to start the live magnetic-field simulation, or
          take the 60-second tour first.
        </p>
        <div class="ctas">
          <button class="cta primary" id="home-run-btn" @click=${()=>this.runDemo()}>
            ${e?`✓ Demo running`:`▶ Run the simulation`}
          </button>
          <button class="cta" id="home-tour-btn" @click=${()=>this.go(`tour`)}>
            ★ Take the 60-second tour
          </button>
          <button class="cta" id="home-help-btn" @click=${()=>this.go(`help`)}>
            ? Help center
          </button>
        </div>
        <div class="status ${e?`live`:``}">
          <span class="dot"></span>
          ${e?i`Live · ${k.value>0?(k.value/1e3).toFixed(2)+` kHz`:`starting…`}${t?` · witness verified ✓`:``}`:i`Idle${t?` · witness verified ✓`:``}`}
        </div>
      </div>

      <div class="grid">
        <div class="card" tabindex="0" role="button"
          @click=${()=>this.go(`scene`)}
          @keydown=${e=>{(e.key===`Enter`||e.key===` `)&&(e.preventDefault(),this.go(`scene`))}}>
          <div class="ico">🌐</div>
          <h3>Live scene</h3>
          <p>Drag magnetic sources, watch the recovered field update in real time, and tweak sample rate / noise / integration.</p>
          <div class="arrow">Open scene →</div>
        </div>

        <div class="card" tabindex="0" role="button"
          @click=${()=>this.go(`apps`)}
          @keydown=${e=>{(e.key===`Enter`||e.key===` `)&&(e.preventDefault(),this.go(`apps`))}}>
          <div class="ico">🛍</div>
          <h3>App Store · 66 edge apps</h3>
          <p>Browse 65 hot-loadable WASM sensing modules across medical, security, building, retail, industrial, learning. Six run live in the browser.</p>
          <div class="arrow">Browse the catalogue →</div>
        </div>

        <div class="card" tabindex="0" role="button"
          @click=${()=>this.go(`witness`)}
          @keydown=${e=>{(e.key===`Enter`||e.key===` `)&&(e.preventDefault(),this.go(`witness`))}}>
          <div class="ico">✓</div>
          <h3>Determinism gate</h3>
          <p>Re-derive the SHA-256 witness for the canonical reference scene right here in your browser. Same inputs → same hash, every time.</p>
          <div class="arrow">Verify the witness →</div>
        </div>

        <div class="card" tabindex="0" role="button"
          @click=${()=>this.go(`ghost-murmur`)}
          @keydown=${e=>{(e.key===`Enter`||e.key===` `)&&(e.preventDefault(),this.go(`ghost-murmur`))}}>
          <div class="ico">👻</div>
          <h3>Ghost Murmur reality check</h3>
          <p>Audit the publicly-reported April 2026 CIA NV-diamond program against published physics. Live distance/moment sliders.</p>
          <div class="arrow">Read the spec →</div>
        </div>
      </div>

      <p class="footnote">
        New here? <a @click=${()=>this.go(`tour`)}>Take the 60-second guided tour</a>
        — every panel is explained. Or press <code>?</code> for the help center
        (quickstart, glossary, FAQ, shortcuts) any time.<br>
        Open source · Apache-2.0 OR MIT · <code>github.com/ruvnet/RuView</code>
      </p>
    `}};Ze=u([n(`nv-home`)],Ze);var Qe=class extends e{constructor(...e){super(...e),this.view=`home`}static{this.styles=t`
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
    /* Home view simplifies: hides sidebar / inspector / console so the
       hero gets the full screen. Power-user panels stay one rail click away. */
    .app.simple {
      grid-template-columns: 56px 1fr;
      grid-template-rows: 48px 1fr;
      grid-template-areas:
        'rail topbar'
        'rail main';
    }
    .app.simple nv-sidebar,
    .app.simple nv-inspector,
    .app.simple nv-console { display: none; }
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
  `}render(){let e=this.view===`home`;return i`
      <a class="skip-link" href="#main-content"
        @click=${e=>{e.preventDefault(),this.shadowRoot?.querySelector(`.main`)?.focus()}}>
        Skip to main content
      </a>
      <div class="app ${e?`simple`:``}">
        <nv-rail .view=${this.view} @navigate=${e=>this.view=e.detail}></nv-rail>
        <nv-topbar></nv-topbar>
        <nv-sidebar></nv-sidebar>
        <main class="main" id="main-content" tabindex="-1" role="main" aria-label="Main view">
          ${this.view===`home`?i`<nv-home></nv-home>`:this.view===`apps`?i`<nv-app-store></nv-app-store>`:this.view===`ghost-murmur`?i`<nv-ghost-murmur></nv-ghost-murmur>`:this.view===`inspector`?i`<nv-inspector expanded .pinTab=${`signal`}></nv-inspector>`:this.view===`witness`?i`<nv-inspector expanded .pinTab=${`witness`}></nv-inspector>`:i`<nv-scene></nv-scene>`}
        </main>
        <nv-inspector
          .pinTab=${this.view===`inspector`?`signal`:this.view===`witness`?`witness`:null}>
        </nv-inspector>
        <nv-console></nv-console>
      </div>
      <nv-toast></nv-toast>
      <nv-modal></nv-modal>
      <nv-palette></nv-palette>
      <nv-debug-hud></nv-debug-hud>
      <nv-settings-drawer></nv-settings-drawer>
      <nv-onboarding></nv-onboarding>
      <nv-help></nv-help>
    `}};u([r()],Qe.prototype,`view`,void 0),Qe=u([n(`nv-app`)],Qe);function $e(e,t,n){let r=e.getUint32(t+0,!0),i=e.getUint16(t+4,!0),a=e.getUint16(t+6,!0),o=e.getUint16(t+8,!0),s=e.getBigUint64(t+12,!0),c=e.getFloat32(t+20,!0),l=e.getFloat32(t+24,!0),ee=e.getFloat32(t+28,!0),u=e.getFloat32(t+32,!0),d=e.getFloat32(t+36,!0),f=e.getFloat32(t+40,!0),p=e.getFloat32(t+44,!0),m=e.getFloat32(t+48,!0);return{magic:r,version:i,flags:a,sensorId:o,tUs:s,bPt:[c,l,ee],sigmaPt:[u,d,f],noiseFloorPtSqrtHz:p,temperatureK:m,raw:n.subarray(t,t+60)}}function et(e){let t=new DataView(e.buffer,e.byteOffset,e.byteLength),n=[];for(let r=0;r+60<=e.byteLength;r+=60)n.push($e(t,r,e));return n}var tt=class{constructor(){this.nextId=1,this.pending=new Map,this.frameSubs=new Set,this.eventSubs=new Set,this.bootInfo=null,this.worker=new Worker(new URL(`/RuView/nvsim/assets/worker-DggHM8f0.js`,``+import.meta.url),{type:`module`}),this.worker.addEventListener(`message`,e=>this.onMessage(e)),this.worker.addEventListener(`error`,e=>this.eventSubs.forEach(t=>t({type:`log`,level:`err`,msg:String(e.message)})))}onMessage(e){let t=e.data;if(t.type===`frames`){let e=t.batch,n=new Uint8Array(e),r={frames:et(n),bytes:n};this.frameSubs.forEach(e=>e(r));let i=t.fps;i>0&&this.eventSubs.forEach(e=>e({type:`fps`,value:i}))}else if(t.type===`state`)this.eventSubs.forEach(e=>e({type:`state`,running:!!t.running,t:0,framesEmitted:Number(t.framesEmitted??0)}));else if(t.type!==`ready`){if(t.type===`err`&&t.id==null)this.eventSubs.forEach(e=>e({type:`log`,level:`err`,msg:String(t.msg)}));else if(typeof t.id==`number`&&this.pending.has(t.id)){let e=this.pending.get(t.id);this.pending.delete(t.id),t.type===`err`?e.reject(Error(String(t.msg))):e.resolve(t)}}}rpc(e,t=[]){let n=this.nextId++;return new Promise((r,i)=>{this.pending.set(n,{resolve:r,reject:i}),this.worker.postMessage({...e,id:n},t)})}async boot(){if(this.bootInfo)return this.bootInfo;let e=await this.rpc({type:`boot`,base:`/RuView/nvsim/`});return this.bootInfo={buildVersion:e.buildVersion,frameMagic:e.frameMagic,frameBytes:e.frameBytes,expectedWitnessHex:e.expectedWitnessHex},this.bootInfo}async loadScene(e){await this.rpc({type:`setScene`,json:JSON.stringify(e)})}async setConfig(e){await this.rpc({type:`setConfig`,json:JSON.stringify(e)})}async setSeed(e){await this.rpc({type:`setSeed`,seed:Number(e&4294967295n)})}async reset(){await this.rpc({type:`reset`})}async run(e){await this.rpc({type:`run`})}async pause(){await this.rpc({type:`pause`})}async step(e,t){await this.rpc({type:`step`})}onFrames(e){this.frameSubs.add(e)}onEvent(e){this.eventSubs.add(e)}async generateWitness(e){let t=await this.rpc({type:`witnessGenerate`,samples:e});return new Uint8Array(t.witness)}async verifyWitness(e){let t=e.slice().buffer,n=await this.rpc({type:`witnessVerify`,samples:256,expected:t},[t]);return n.ok?{ok:!0}:{ok:!1,actual:new Uint8Array(n.actual)}}async runTransient(e,t,n,r){let i=await this.rpc({type:`runTransient`,scene:JSON.stringify(e),config:JSON.stringify(t),seed:Number(n&4294967295n),samples:r});return{bRecoveredT:[i.bRecoveredT[0],i.bRecoveredT[1],i.bRecoveredT[2]],bMagT:i.bMagT,noiseFloorPtSqrtHz:i.noiseFloorPtSqrtHz,sigmaPt:[i.sigmaPt[0],i.sigmaPt[1],i.sigmaPt[2]],nFrames:i.nFrames,witnessHex:i.witnessHex}}async exportProofBundle(){let e=await this.generateWitness(256),t=Array.from(e).map(e=>e.toString(16).padStart(2,`0`)).join(``),n=this.bootInfo??await this.boot(),r=JSON.stringify({kind:`nvsim-proof-bundle`,version:n.buildVersion,seed:`0x0000002A`,nSamples:256,witness:t,expected:n.expectedWitnessHex,ok:t===n.expectedWitnessHex,ts:new Date().toISOString()},null,2);return new Blob([r],{type:`application/json`})}async buildId(){return(await this.rpc({type:`buildId`})).buildId}async close(){this.worker.terminate()}};function nt(e){return e.startsWith(`ws://`)||e.startsWith(`wss://`)?e:e.replace(/^http/,`ws`)}var rt=class{constructor(e){this.ws=null,this.bootInfo=null,this.frameSubs=new Set,this.eventSubs=new Set,this.running=!1,this.framesEmitted=0,this.fpsLast=performance.now(),this.fpsCount=0,this.baseUrl=e.replace(/\/$/,``),this.wsUrl=`${nt(this.baseUrl)}/ws/stream`}async json(e,t){let n=await fetch(`${this.baseUrl}${e}`,{...t,headers:{"content-type":`application/json`,...t?.headers??{}}});if(!n.ok)throw Error(`${e}: ${n.status} ${n.statusText}`);return await n.json()}async boot(){if(this.bootInfo)return this.bootInfo;let e=await this.json(`/api/health`);return this.bootInfo={buildVersion:e.nvsim_version,frameMagic:e.magic,frameBytes:e.frame_bytes,expectedWitnessHex:e.expected_witness_hex},this.openWs(),this.bootInfo}openWs(){if(this.ws)return;let e=new WebSocket(this.wsUrl);e.binaryType=`arraybuffer`,e.onopen=()=>{this.eventSubs.forEach(e=>e({type:`log`,level:`ok`,msg:`ws/stream connected · ${this.wsUrl}`}))},e.onclose=()=>{this.ws=null,this.eventSubs.forEach(e=>e({type:`log`,level:`warn`,msg:`ws/stream closed`}))},e.onerror=()=>{this.eventSubs.forEach(e=>e({type:`log`,level:`err`,msg:`ws/stream error · ${this.wsUrl}`}))},e.onmessage=e=>{if(!(e.data instanceof ArrayBuffer))return;let t=new Uint8Array(e.data),n=et(t);if(n.length===0)return;let r={frames:n,bytes:t};this.frameSubs.forEach(e=>e(r)),this.framesEmitted+=n.length,this.fpsCount+=n.length;let i=performance.now();if(i-this.fpsLast>=1e3){let e=this.fpsCount*1e3/(i-this.fpsLast);this.eventSubs.forEach(t=>t({type:`fps`,value:e})),this.fpsLast=i,this.fpsCount=0}},this.ws=e}async loadScene(e){await this.json(`/api/scene`,{method:`PUT`,body:JSON.stringify(e)})}async setConfig(e){await this.json(`/api/config`,{method:`PUT`,body:JSON.stringify(e)})}async setSeed(e){await this.json(`/api/seed`,{method:`PUT`,body:JSON.stringify({seed_hex:`0x`+e.toString(16).toUpperCase().padStart(16,`0`)})})}async reset(){await this.json(`/api/reset`,{method:`POST`}),this.running=!1,this.framesEmitted=0,this.eventSubs.forEach(e=>e({type:`state`,running:!1,t:0,framesEmitted:0}))}async run(e){await this.json(`/api/run`,{method:`POST`}),this.running=!0,this.eventSubs.forEach(e=>e({type:`state`,running:!0,t:0,framesEmitted:this.framesEmitted}))}async pause(){await this.json(`/api/pause`,{method:`POST`}),this.running=!1,this.eventSubs.forEach(e=>e({type:`state`,running:!1,t:0,framesEmitted:this.framesEmitted}))}async step(e,t){await this.json(`/api/step`,{method:`POST`,body:JSON.stringify({direction:e,dt_ms:t})})}onFrames(e){this.frameSubs.add(e)}onEvent(e){this.eventSubs.add(e)}async generateWitness(e){let t=await this.json(`/api/witness/generate`,{method:`POST`,body:JSON.stringify({samples:e})}),n=new Uint8Array(32);for(let e=0;e<32;e++)n[e]=parseInt(t.witness_hex.slice(e*2,e*2+2),16);return n}async verifyWitness(e){let t=Array.from(e).map(e=>e.toString(16).padStart(2,`0`)).join(``),n=await this.json(`/api/witness/verify`,{method:`POST`,body:JSON.stringify({expected_hex:t,samples:256})});if(n.ok)return{ok:!0};let r=new Uint8Array(32);for(let e=0;e<32;e++)r[e]=parseInt(n.actual_hex.slice(e*2,e*2+2),16);return{ok:!1,actual:r}}async exportProofBundle(){let e=await fetch(`${this.baseUrl}/api/export-proof`,{method:`POST`}).then(e=>e.text());return new Blob([e],{type:`application/json`})}async runTransient(e,t,n,r){return{bRecoveredT:[0,0,0],bMagT:0,noiseFloorPtSqrtHz:0,sigmaPt:[0,0,0],nFrames:0,witnessHex:`(transient route not available in WS transport — V1 limitation)`}}async buildId(){return`nvsim@${(this.bootInfo??await this.boot()).buildVersion} (ws)`}async close(){this.ws?.close(),this.ws=null}};function it(e){document.documentElement.setAttribute(`data-theme`,e)}function at(e){document.body.classList.remove(`density-comfy`,`density-default`,`density-compact`),document.body.classList.add(`density-${e}`)}function ot(e){document.body.classList.toggle(`reduce-motion`,e)}(async()=>{let e=await K(`theme`)??`dark`,t=await K(`density`)??`default`,n=window.matchMedia?.(`(prefers-reduced-motion: reduce)`).matches??!1,r=await K(`motionReduced`)??n;S.value=e,it(e),C.value=t,at(t),w.value=r,ot(r),c(()=>{it(S.value),q(`theme`,S.value)}),c(()=>{at(C.value),q(`density`,C.value)}),c(()=>{ot(w.value),q(`motionReduced`,w.value)});let i=await K(`repl-history`);i&&Array.isArray(i)&&(F.value=i),c(()=>{q(`repl-history`,F.value)});let a=await K(`scene-positions`);a&&Array.isArray(a)&&(I.value=a),c(()=>{q(`scene-positions`,I.value)});let o=await K(`wsUrl`)??``;o&&(p.value=o),f.value=await K(`transport`)??`wasm`,c(()=>{q(`wsUrl`,p.value)}),c(()=>{q(`transport`,f.value)});let s={},l=[],ee=performance.now(),u=e=>{if(e.frames.length===0)return;let t=e.frames[e.frames.length-1];N.value=t;let n=t.bPt[0]*1e-12,r=t.bPt[1]*1e-12,i=t.bPt[2]*1e-12;E.value=[n,r,i];let a=Math.sqrt(n*n+r*r+i*i);for(D.value=a,be([n*1e9,r*1e9,i*1e9]),xe(Math.min(1,Math.abs(i*1e9)/5+.3)),l.push(a);l.length>256;)l.shift();let o=he.value;if(o.size===0)return;let c=(performance.now()-ee)/1e3;for(let e of o){let o=Re[e];if(!o)continue;s[e]||(s[e]={});let ee={frame:t,bMagT:a,bRecoveredT:[n,r,i],bHistory:l,elapsedS:c,state:s[e]};try{let e=o(ee);if(!e)continue;let t=Array.isArray(e)?e:[e];for(let e of t)me(e),z(`info`,`<span class="k">[${e.appId}]</span> <span class="s">${e.eventName}</span> <span class="n">(${e.eventId})</span>${e.detail?` · `+e.detail:``}`)}catch(t){z(`warn`,`[${e}] runtime error: ${t.message}`)}}},d=null;async function h(){try{d&&await d.close();let e=f.value;if(e===`ws`&&p.value.trim()){let e=new rt(p.value.trim()),t=await e.boot();d=e,m.value=!0,te.value=null,M.value=t.expectedWitnessHex,g(e),z(`ok`,`transport WS · ${p.value} · nvsim@${t.buildVersion}`)}else{e===`ws`&&z(`warn`,`WS transport selected but no URL set — falling back to WASM`);let t=new tt,n=await t.boot();d=t,m.value=!0,te.value=null,M.value=n.expectedWitnessHex,g(t),z(`ok`,`transport WASM · nvsim@${n.buildVersion} · magic=0x${n.frameMagic.toString(16).toUpperCase()}`)}ve(d)}catch(e){let t=e.message;te.value=t,m.value=!1,z(`err`,`transport boot failed: ${t}`)}}function g(e){e.onEvent(e=>{e.type===`log`&&z(e.level,e.msg),e.type===`fps`&&(k.value=e.value),e.type===`state`&&(re.value=BigInt(e.framesEmitted))}),e.onFrames(u)}let ne=!1;c(()=>{f.value,p.value,ne||(ne=!0,h().finally(()=>{ne=!1}))}),z(`info`,`nvsim — booting transport`);let _=null;c(()=>{let e=M.value,t=m.value;e&&t&&_!==e&&(_=e,(async()=>{let t=d;if(t)try{let n=new Uint8Array(32);for(let t=0;t<32;t++)n[t]=parseInt(e.slice(t*2,t*2+2),16);let r=await t.verifyWitness(n);if(r.ok)A.value=e,z(`ok`,`witness verified · determinism gate ✓ · transport=${f.value}`);else{let t=Array.from(r.actual).map(e=>e.toString(16).padStart(2,`0`)).join(``);A.value=t,z(`err`,`WITNESS MISMATCH · expected ${e.slice(0,16)}… got ${t.slice(0,16)}…`)}}catch(e){z(`warn`,`witness verify skipped: ${e.message}`)}})())}),ce.value=`(reference scene)`})();
//# sourceMappingURL=index-D0E4wKio.js.map