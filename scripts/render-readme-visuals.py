#!/usr/bin/env python3
"""Generate the RuView README visual system. Deterministic, no network."""
from pathlib import Path
from html import escape
import math,json,re
from readme_visuals_xml import parse_svg
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'assets/readme';OUT.mkdir(parents=True,exist_ok=True)
C='#78cfff';BLUE='#438fff';INK='#071124';WHITE='#f3f7ff';MUTED='#b0c2db'
LIME='#b8f36c';AMBER='#ffb45e'
def text(x,y,value,size=16,color=MUTED,anchor='start',extra=''):
 weight='font-weight="650" letter-spacing="-0.6"' if size>=25 else 'font-weight="500"'
 return f'<text x="{x}" y="{y}" font-size="{size}" fill="{color}" text-anchor="{anchor}" {weight} {extra}>{escape(value)}</text>'
def path(d,color=C,width=1.5,extra=''):
 return f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round" {extra}/>'
def circle(x,y,r,color=C,extra=''):
 fill = '' if 'fill=' in extra else 'fill="none"'
 return f'<circle cx="{x}" cy="{y}" r="{r}" stroke="{color}" {fill} {extra}/>'

def icon(kind,x,y,scale=1):
 s=''
 if kind in ['signal','start','presence']:
  for r in [18,32,46]:s+=f'<ellipse rx="{r}" ry="{r*.48}" fill="none" stroke="{C}" opacity=".4"/>'
  s+=path('M-46 0H46M0 -23V23',BLUE,.7)
  s+='<g class="sweep">'+path('M0 0L40 -18',LIME,2)+circle(34,-15,3,LIME,extra=f'fill="{LIME}"')+'</g>'
  for px,py in [(-20,8),(12,13),(23,-8)]:s+=circle(px,py,3,AMBER,extra=f'fill="{AMBER}" class="spark"')
 elif kind=='pose':
  # Authored anatomical mesh silhouette; a conceptual figure, not captured pose data.
  s+='<defs><linearGradient id="humanSurface" x1="0" y1="0" x2="1" y2=".3"><stop stop-color="#153d48"/><stop offset=".42" stop-color="#338469"/><stop offset=".7" stop-color="#83c766"/><stop offset="1" stop-color="#163c4a"/></linearGradient><linearGradient id="humanRear"><stop stop-color="#0f2d40"/><stop offset="1" stop-color="#397a68"/></linearGradient></defs>'
  s+='<g class="humanTurn"><g stroke="#88d783" stroke-width=".65" stroke-linejoin="round">'
  # Far arm and leg provide occlusion and depth.
  s+='<g class="armFar"><path d="M-9 -25C-15 -25 -18 -19 -21 -13L-29 -5C-32 -3 -33 1 -30 3L-25 2L-14 -7C-9 -12 -6 -19 -6 -23Z" fill="url(#humanRear)"/></g>'
  s+='<g class="legFar"><path d="M-6 5C-13 10 -15 18 -17 26L-25 43L-29 47C-30 49 -28 51 -24 51L-17 50L-16 46L-8 30C-4 23 2 15 3 10Z" fill="url(#humanRear)"/><circle cx="-15" cy="26" r="1.8" fill="#ffb45e"/></g>'
  s+='<g class="legNear"><path d="M3 5C10 7 14 17 19 23C24 28 23 35 24 41L29 45C32 47 32 49 28 50L20 49C17 48 17 46 17 42L14 32C8 28 2 23 -3 16L-4 10Z" fill="url(#humanSurface)"/><circle cx="19" cy="28" r="1.8" fill="#ffb45e"/></g>'
  # Near arm: shoulder, upper arm, elbow, forearm and closed hand.
  s+='<g class="armNear"><path d="M7 -26C14 -25 17 -20 20 -13L24 -17L29 -24C30 -27 33 -29 35 -26L35 -22L30 -16L24 -8C22 -5 18 -5 16 -8L8 -17Z" fill="url(#humanSurface)"/></g>'
  # Shoulder and chest taper into abdomen and pelvis.
  s+='<path d="M-4 -32L4 -33L6 -28C11 -27 14 -24 13 -19L9 -5L10 7C10 13 2 15 -5 12L-10 7L-8 -5L-12 -20C-13 -25 -8 -28 -5 -28Z" fill="url(#humanSurface)"/>'
  # Three-quarter head with jaw, nose and ear contour.
  s+='<path d="M-6 -42C-6 -48 2 -51 7 -47C10 -45 10 -41 9 -39L11 -36L8 -35L7 -31L3 -29L-3 -32L-5 -37Z" fill="url(#humanSurface)"/>'
  s+='</g>'
  # Surface contours make volume legible at README scale.
  s+=path('M-9 -23Q-2 -18 10 -23M-7 -15Q0 -11 9 -16M-6 -8Q1 -5 8 -9M-7 1Q1 5 9 1M0 -26L2 9M-4 -40Q0 -42 7 -40M2 -32L6 -34',LIME,.55,extra='opacity=".65"')
  s+=path('M-5 -45Q1 -49 6 -46M-10 -22L-6 -8',C,.7,extra='opacity=".8"')
  s+='<path d="M-8 -25Q-2 -21 1 -10L3 9Q-5 8 -6 3L-5 -8Z" fill="#a7ebff" class="humanLight" opacity=".2"/>'
  # Joint markers remain attached to the anatomical surface.
  for px,py in [(-9,-24),(10,-23),(1,-12),(2,9)]:
   s+=circle(px,py,3.5,AMBER,extra='stroke-opacity=".25"')+circle(px,py,1.5,AMBER,extra=f'fill="{AMBER}" class="spark"')
  s+='</g>'
  s+=path('M-42 -42V-53H-30M30 -53H42V-42M42 45V55H30M-30 55H-42V45',BLUE,.8)
 elif kind in ['vitals','models']:
  for j in range(3):
   s+=path(f'M-49 {j*12-14}C-34 {j*12-14} -30 {j*12-35} -17 {j*12-35}S0 {j*12+7} 14 {j*12+7}S34 {j*12-14} 49 {j*12-14}',C if j!=1 else LIME,1.5,extra=f'class="trace" style="animation-delay:-{j}s" opacity="{1-j*.2}"')
  s+='<g class="cursor">'+path('M0 -42V40',AMBER,1)+circle(0,-8,3,AMBER,extra=f'fill="{AMBER}"')+'</g>'
 elif kind in ['edge','hardware','seed','evidence']:
  for j in range(3):
   yy=j*13
   s+=f'<g class="layer" style="animation-delay:-{j*.7}s"><path d="M-38 {yy-23}L0 {yy-42}L38 {yy-23}L0 {yy-4}Z" fill="#102749" stroke="{C if j==0 else BLUE}"/><path d="M-38 {yy-23}V{yy-16}L0 {yy+3}L38 {yy-16}V{yy-23}M0 {yy-4}V{yy+3}" fill="#09172c" stroke="#315685"/></g>'
  s+=path('M-12 -31L0 -37L12 -31L0 -25Z',LIME,2)
  for px in [-26,0,26]:s+=path(f'M{px} 22V39',AMBER,1,extra='class="flow"')
 elif kind in ['home','automation']:
  s+='<path d="M-43 12L0 -10L43 12L0 35Z" fill="#102749" stroke="#438fff"/>'
  s+=path('M-32 17V-16L0 -35L32 -16V17M-32 -16L0 3L32 -16M0 3V34',C,1.5)
  s+=path('M-22 20L-10 13L12 25M-10 13V-5',LIME,1.5,extra='class="flow"')
  for px,py in [(-22,20),(-10,-5),(12,25)]:s+=circle(px,py,4,AMBER,extra=f'fill="#102749" class="breathe"')
 elif kind in ['agent','agents']:
  for px,py in [(0,-36),(-38,-10),(-23,31),(30,28),(39,-17)]:
   s+=path(f'M0 0L{px} {py}',BLUE,1,extra='class="flow"')
   s+=f'<path d="M{px} {py-7}l7 4v8l-7 4l-7 -4v-8Z" fill="#102749" stroke="{C}" class="breathe"/>'
  s+=circle(0,0,15,LIME)+path('M-6 0L-1 5L8 -6',LIME,2)
 else:s+=path('M-35 0L0 -25L35 0L0 25Z',C,2)
 base='<ellipse cy="53" rx="48" ry="8" fill="#020816" opacity=".65"/>'
 if kind=='pose':
  base='<ellipse cy="53" rx="37" ry="7" fill="#020816" class="gaitShadow"/>'
  return f'<g transform="translate({x} {y}) scale({scale})">{base}<g class="humanBody">{s}</g></g>'
 return f'<g transform="translate({x} {y}) scale({scale})">{base}<g class="float">{s}</g></g>'

def spatial(x,y,scale=1):
 """Projected geometry gives depth without scripts or CSS 3D dependencies."""
 s='<path d="M-82 36L0 -6L82 36L0 78Z" fill="#0a2733" stroke="#286171"/>'
 s+='<path d="M-82 36V-52L0 -94L82 -52V36M-82 -52L0 -10L82 -52M0 -10V78" fill="none" stroke="#366d80"/>'
 s+='<g class="scan"><path d="M-81 32L0 -10L81 32L0 74Z" fill="#5fe3d8" fill-opacity=".12" stroke="#72eee0" stroke-opacity=".7"/></g>'
 for px,py in [(-40,6),(0,-15),(40,6),(-20,30),(20,30),(0,49)]:
  s+=circle(px,py,2,BLUE,extra='fill="#70b6ff" class="spark"')
 s+='<g class="orbit"><ellipse cx="0" cy="4" rx="102" ry="30" fill="none" stroke="#70b6ff" stroke-opacity=".4" stroke-dasharray="2 8"/><circle cx="102" cy="4" r="4" fill="#91f6df"/></g>'
 s+='<ellipse cx="0" cy="36" rx="85" ry="28" fill="none" stroke="#70b6ff" class="wave"/>'
 return f'<g transform="translate({x} {y}) scale({scale})">{s}</g>'
def _frame(w,h,title,desc,body):
 return f'''<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" aria-labelledby="title desc"><title id="title">{escape(title)}</title><desc id="desc">{escape(desc)}</desc><defs><linearGradient id="wash" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#0b303a"/><stop offset=".55" stop-color="#06171e"/><stop offset="1" stop-color="#030c13"/></linearGradient><linearGradient id="accent"><stop stop-color="#91f6df"/><stop offset="1" stop-color="#26bad7"/></linearGradient><radialGradient id="halo"><stop stop-color="#35d4db" stop-opacity=".17"/><stop offset="1" stop-color="#35d4db" stop-opacity="0"/></radialGradient></defs><style>text{{font-family:Arial,Helvetica,sans-serif}}.mono{{font-family:ui-monospace,Consolas,monospace;letter-spacing:2px}}.flow{{stroke-dasharray:5 12;animation:flow 12s linear infinite}}.breathe{{animation:breathe 6s ease-in-out infinite}}.trace{{stroke-dasharray:400;stroke-dashoffset:0;animation:trace 10s ease-in-out infinite}}.float{{animation:float 9s ease-in-out infinite}}@keyframes flow{{to{{stroke-dashoffset:-136}}}}@keyframes breathe{{50%{{opacity:.4}}}}@keyframes trace{{0%,100%{{stroke-dashoffset:0}}50%{{stroke-dashoffset:90}}}}@keyframes float{{50%{{transform:translateY(-6px)}}}}@media(prefers-reduced-motion:reduce){{*{{animation:none!important}}}}</style><rect x=".5" y=".5" width="{w-1}" height="{h-1}" rx="18" fill="url(#wash)" stroke="#245360"/>{body}</svg>'''
MOTION='''<style>
text{font-family:"Avenir Next","Segoe UI Variable Display","Segoe UI",Helvetica,Arial,sans-serif}
.mono{font-family:"SFMono-Regular",Consolas,"Liberation Mono",monospace;letter-spacing:1.6px}
.float{animation:float 5s ease-in-out infinite}
.flow{stroke-dasharray:5 12;animation:flow 5s linear infinite}
.breathe{animation:breathe 4s ease-in-out infinite}
.trace{stroke-dasharray:400;animation:trace 6s ease-in-out infinite}
.scan{animation:scan 7s ease-in-out infinite}
.orbit{stroke-dasharray:3 14;animation:flow 8s linear infinite}
.spark{animation:spark 3.5s ease-in-out infinite}
.wave{transform-box:fill-box;transform-origin:center;animation:wave 4s ease-out infinite}
@keyframes float{0%,100%{transform:translate(0,0)}50%{transform:translate(3px,-10px)}}
@keyframes scan{0%,100%{transform:translateY(0)}50%{transform:translateY(-76px)}}

.sweep{animation:sweep 5s ease-in-out infinite;transform-origin:0 0}
.limb{animation:limb 3s ease-in-out infinite;transform-origin:0 -18px}
.stride{animation:limb 3s ease-in-out infinite reverse;transform-origin:5px 16px}
.layer{animation:layer 6s ease-in-out infinite}
.cursor{animation:cursor 5s ease-in-out infinite}
@keyframes sweep{0%,100%{transform:rotate(-20deg)}50%{transform:rotate(155deg)}}
@keyframes limb{0%,100%{transform:rotate(-3deg)}50%{transform:rotate(3deg)}}
@keyframes layer{50%{transform:translateY(-5px)}}
@keyframes cursor{0%,100%{transform:translateX(-35px);opacity:.3}50%{transform:translateX(35px);opacity:1}}


.humanTurn{transform-box:fill-box;transform-origin:center 85%;animation:humanTurn 8s ease-in-out infinite}
.humanBody{animation:humanBody 1.6s ease-in-out infinite}
.armFar{transform-origin:-9px -24px;animation:armSwing 1.6s ease-in-out infinite}
.armNear{transform-origin:10px -23px;animation:armSwing 1.6s ease-in-out infinite;animation-delay:-.8s}
.legFar{transform-origin:-3px 9px;animation:legSwing 1.6s ease-in-out infinite;animation-delay:-.8s}
.legNear{transform-origin:2px 9px;animation:legSwing 1.6s ease-in-out infinite}
.humanLight{animation:humanLight 8s ease-in-out infinite}
.gaitShadow{transform-box:fill-box;transform-origin:center;animation:gaitShadow 1.6s ease-in-out infinite}
@keyframes humanTurn{0%,100%{transform:perspective(450px) rotateY(-24deg) rotateZ(-2deg)}50%{transform:perspective(450px) rotateY(24deg) rotateZ(2deg)}}
@keyframes humanBody{0%,50%,100%{transform:translateY(0)}25%,75%{transform:translateY(-2px)}}
@keyframes armSwing{0%,100%{transform:rotate(-17deg)}50%{transform:rotate(17deg)}}
@keyframes legSwing{0%,100%{transform:rotate(-12deg)}50%{transform:rotate(12deg)}}
@keyframes humanLight{0%,100%{opacity:.1}50%{opacity:.45}}
@keyframes gaitShadow{0%,50%,100%{transform:scaleX(1);opacity:.7}25%,75%{transform:scaleX(.85);opacity:.45}}

@keyframes spark{50%{opacity:.25}}
@keyframes flow{to{stroke-dashoffset:-136}}
@keyframes breathe{50%{opacity:.4}}
@keyframes trace{0%,100%{stroke-dashoffset:0}50%{stroke-dashoffset:90}}
@keyframes wave{0%{transform:scale(.5);opacity:.7}100%{transform:scale(1.3);opacity:0}}
@media(prefers-reduced-motion:reduce){*{animation:none!important}}
</style>'''

def frame(w,h,title,desc,body):
 value=_frame(w,h,title,desc,body)
 motion=MOTION
 for name in ['float','flow','breathe','trace','scan','orbit','spark','wave','sweep','limb','stride','layer','cursor','humanTurn','humanBody','armFar','armNear','legFar','legNear','humanLight','gaitShadow']:
  if f'class="{name}"' not in body:
   motion=re.sub(r'\.'+name+r'\{[^}]*\}\n?', '', motion)
 for name in ['float','flow','breathe','trace','scan','spark','wave','sweep','limb','layer','cursor','humanTurn','humanBody','armSwing','legSwing','humanLight','gaitShadow']:
  if f'animation:{name} ' not in motion:
   marker='@keyframes '+name+'{'; start=motion.find(marker)
   if start>=0:
    pos=start+len(marker); depth=1
    while depth:
     depth+=(motion[pos]=='{')-(motion[pos]=='}');pos+=1
    motion=motion[:start]+motion[pos:].removeprefix('\n')
 value=value[:value.index('<style>')]+motion+value[value.index('</style>')+8:]
 palette={'#0b303a':'#102749','#06171e':'#071429','#030c13':'#03091a',
          '#245360':'#2d4e77','#91f6df':'#9ee7ff','#26bad7':'#348bff',
          '#35d4db':'#348bff','#72eee0':C,'#70b6ff':BLUE,'#041117':INK,
          '#133a45':'#142c50','#285c68':'#2e527d','#0a2733':'#0b1b35',
          '#286171':'#2e5b8e','#366d80':'#4473a5','#5fe3d8':'#438fff',
          '#0a2630':'#0c2242','#12343d':'#162f50','#4b9ba4':'#5490c5',
          '#071b23':'#091b35','#285360':'#315685','#214451':'#233f63','#3c8897':'#437cb2'}
 for old,new in palette.items():value=value.replace(old,new)
 rim=f'<path d="M24 1H{w-24}" stroke="url(#accent)" opacity=".55"/><path d="M24 {h-1}H{w-24}" stroke="#438fff" opacity=".15"/>'
 return value.replace('</svg>',rim+'</svg>')

manifest=[]
def save(name,w,h,title,desc,body,target):
 name+='-spatial-v6' if name in ['hero','feature-pose'] else '-spatial-v4'
 s=frame(w,h,title,desc,body);raw=(s+'\n').encode('utf-8');parse_svg(raw);(OUT/(name+'.svg')).write_bytes(raw);manifest.append({'file':'assets/readme/'+name+'.svg','title':title,'target':target,'width':w,'height':h})
# Cinematic RF stage: authored projected planes, not a live measurement.
s=text(48,45,'RUVIEW  /  SPATIAL INTELLIGENCE',12,C,extra='class="mono"')
s+=text(44,140,'Invisible signals.',59,WHITE)+text(44,204,'Visible possibility.',55,LIME)
s+=text(48,256,'Explore the space between sensing and understanding.',18)
s+=text(48,284,'WiFi sensing. Edge models. Local automation.',18)
s+='<rect x="48" y="327" width="240" height="49" rx="24" fill="url(#accent)"/>'+text(72,358,'Enter the field',18,INK)+path('M246 351H266M259 344L266 351L259 358',INK,2)
s+=text(49,418,'RF SENSING / OPEN RESEARCH / EDGE COMPUTE',10,MUTED,extra='class="mono"')
s+='<circle cx="955" cy="230" r="234" fill="url(#halo)"/>'
# Ground perspective and an exploded room volume.
for i in range(7):s+=path(f'M{785+i*24} 270L{648+i*91} 432','#1b3558',.8)
for j in range(5):s+=path(f'M{768-j*27} {286+j*31}H{1110+j*16}','#1b3558',.8)
s+='<path d="M780 295L963 211L1148 303L960 400Z" fill="#0d2342" fill-opacity=".65" stroke="#315685"/>'
s+='<path d="M780 295V153L963 70L1148 162V303M780 153L960 244L1148 162M960 244V400" fill="none" stroke="#438fff" opacity=".55"/>'
s+='<path d="M963 70V211L1148 303V162Z" fill="#438fff" fill-opacity=".05"/>'
s+='<g class="scan"><path d="M780 286L963 202L1148 294L960 390Z" fill="#78cfff" fill-opacity=".08" stroke="#78cfff" stroke-opacity=".6"/></g>'
for i in range(3):s+=f'<g transform="translate(957 320)"><ellipse rx="{90+i*26}" ry="{30+i*8}" fill="none" stroke="{C}" class="wave" style="animation-delay:-{i*1.3}s"/></g>'
s+=icon('pose',959,236,1.8)+icon('hardware',771,324,.72)
for j in range(3):s+=path(f'M795 {309+j*8}Q854 {245+j*14} 939 {223+j*17}',C,1.2,extra=f'class="flow" style="animation-delay:-{j}s"')
for px,py in [(780,153),(963,70),(1148,162),(1148,303),(960,400)]:s+=circle(px,py,3,AMBER,extra=f'fill="{AMBER}" class="spark"')
s+=text(1148,419,'CONCEPTUAL RF FIELD',10,MUTED,'end',extra='class="mono"')
save('hero',1200,450,'RuView: invisible signals, visible possibility','Animated conceptual RF room with an anatomical human silhouette, moving scan plane and propagating waves. Illustration only, not a sensor measurement.',s,'docs/user-guide.md')
# Visual navigation icons, each receives a real outer link in Markdown.
nav=[('start','Start here','docs/user-guide.md'),('hardware','Hardware','firmware/esp32-csi-node/README.md'),('models','Models','https://huggingface.co/ruvnet/wifi-densepose-pretrained'),('agents','AI toolkit','harness/ruview/README.md'),('home','Smart home','docs/integrations/home-assistant.md'),('seed','Cognitum','https://cognitum.one/seed')]
for k,label,target in nav:save('nav-'+k,180,116,label,'Open '+label+'. Animated navigation icon.',icon(k,90,45,.65)+text(90,98,label,16,WHITE,'middle'),target)
# Capability cards. No unverified performance claims.
cards=[('presence','01','Presence & motion','Explore changes in room occupancy.','Inspect motion features from CSI.','SENSING GUIDE','docs/user-guide.md'),('vitals','02','Breathing & vital signals','Inspect phase-derived signal trends.','Validate estimates on your hardware.','SIGNAL PIPELINE','v2/crates/wifi-densepose-signal/README.md'),('pose','03','Pose research','Train and compare body keypoints.','Live accuracy differs from benchmarks.','READ THE EVIDENCE','docs/benchmarks/pose-estimation-cog.md'),('edge','04','Edge sensor mesh','Provision, capture and calibrate.','Explore ESP32 firmware and runbooks.','HARDWARE GUIDE','firmware/esp32-csi-node/README.md'),('automation','05','Local automation','Connect rooms to your smart home.','Home Assistant, MQTT and Matter.','INTEGRATION GUIDE','docs/integrations/home-assistant.md'),('agents','06','A guided AI operator','Source-cited setup and verification.','CLI, MCP and agent workflows.','TOOLKIT GUIDE','harness/ruview/README.md')]
for k,num,title,l1,l2,cta,target in cards:
 s=text(30,39,'RUVIEW / '+num,11,C,extra='class="mono"')+text(30,89,title,25,WHITE)+text(30,126,l1,14)+text(30,151,l2,14)
 s+='<circle cx="465" cy="170" r="110" fill="url(#halo)"/>'+icon(k,480,144,1.12)+path('M30 204H550','#245360',1)+path('M30 204H550',C,1,extra='class="flow" opacity=".5"')
 s+=text(30,245,cta,12,C,extra='class="mono"')+path('M510 239H540M532 231L540 239L532 247',C,1.5)
 save('feature-'+k,580,276,title,l1+' '+l2+' Conceptual animation; follow the linked source for capability limits.',s,target)
# Overview uses three clearly labelled stages rather than fabricated telemetry.
s=text(36,44,'SIGNAL TO CONTEXT',12,C,extra='class="mono"')+text(36,89,'A sensing loop you can inspect.',34,WHITE)
for i,(label,sub,kind,tail) in enumerate([('Capture RF','CSI from supported hardware','signal','Provision and calibrate'),('Extract context','Signal processing and models','models','Compare against real data'),('Act locally','Events, APIs and integrations','automation','Apply rules and review outputs')]):
 x=36+i*396;s+=f'<rect x="{x}" y="120" width="336" height="220" rx="14" fill="#071b23" stroke="#285360"/>'
 s+=text(x+24,153,f'0{i+1}',12,C)+text(x+24,188,label,26,WHITE)+text(x+24,216,sub,15)+spatial(x+269,278,.35)+icon(kind,x+269,270,.5)+text(x+24,311,tail,13)
 if i<2:s+=path(f'M{x+344} 229H{x+384}M{x+376} 221L{x+384} 229L{x+376} 237',C,2,extra='class="flow"')
s+=text(36,373,'CONCEPTUAL PIPELINE / HARDWARE AND ENVIRONMENT VALIDATION MATTER',11,MUTED,extra='class="mono"')
save('overview',1200,396,'RuView sensing overview','Conceptual sequence: capture RF, extract context, act locally. Links to architecture and user documentation.',s,'docs/user-guide.md')
# Chapter headings act as visual landmarks, all remain links.
headers=[('features','Explore the capabilities','SENSING / MODELS / AUTOMATION','presence','docs/user-guide.md'),('edge','Intelligence at the edge','SENSORS / FIRMWARE / MODULES','edge','firmware/esp32-csi-node/README.md'),('learning','From recordings to models','CAPTURE / TRAIN / EVALUATE','models','docs/user-guide.md'),('evidence','Evidence before accuracy claims','BENCHMARKS / REPRODUCERS / LIMITS','evidence','docs/benchmarks/pose-estimation-cog.md'),('docs','Choose your next step','GUIDES / HARDWARE / INTEGRATIONS','agents','docs/user-guide.md')]
for k,title,sub,kind,target in headers:
 s=icon(kind,76,75,.75)+text(140,63,title,34,WHITE)+text(142,99,sub,12,C,extra='class="mono"')
 s+=spatial(1012,84,.55)
 s+=path('M1140 70H1160M1152 62L1160 70L1152 78',C,2)
 save('header-'+k,1200,148,title,'Animated section heading. '+sub,s,target)
# Brand destination; original ruOS promotion remains in the README.
s=text(44,42,'COGNITUM ONE',12,C,extra='class="mono"')+text(44,99,'Ambient Intelligence.',42,WHITE)+text(44,147,'At the edge of the physical world.',25,C)+text(44,190,'Explore the platform behind the ecosystem.',16)
s+='<rect x="44" y="224" width="256" height="46" rx="11" fill="url(#accent)"/>'+text(65,254,'Explore Cognitum',18,INK)+path('M263 247H284M277 240L284 247L277 254',INK,2)
s+='<circle cx="947" cy="151" r="173" fill="url(#halo)"/>'+spatial(945,173,1.55)+icon('seed',945,145,1.65)
for j in range(3):s+=f'<ellipse cx="945" cy="150" rx="{140+j*20}" ry="{30+j*13}" transform="rotate(-25 945 150)" fill="none" stroke="#3c8897" stroke-dasharray="5 18" class="flow"/>'
save('cognitum',1200,304,'Explore Cognitum One','Ambient Intelligence at the edge of the physical world. Open cognitum.one.',s,'https://cognitum.one')
(OUT/'manifest.json').write_bytes((json.dumps({'schema_version':1,'style':'RuView spatial v4: navy glass, electric blue, lime and amber','motion':'Decorative CSS; prefers-reduced-motion supported; no script or remote dependencies','assets':manifest},indent=2)+'\n').encode('utf-8'))
print(f'Generated {len(manifest)} SVGs, {sum((ROOT/x["file"]).stat().st_size for x in manifest):,} bytes')
