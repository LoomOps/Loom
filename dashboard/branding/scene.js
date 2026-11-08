(()=>{
const canvas=document.querySelector('#loom-brand-scene'),ctx=canvas.getContext('2d');
const paths=Object.fromEntries(['base','gate','token'].map(name=>[name,new Path2D(document.querySelector('.loom-logo .loom-logo-'+name).getAttribute('d'))]));
let time=0,raf=0,last=0,paint=0;
const reduced=matchMedia('(prefers-reduced-motion: reduce)'),systemTheme=matchMedia('(prefers-color-scheme: dark)');
let dark=false;
function readTheme(){const mode=document.documentElement.dataset.loomTheme||'system';dark=mode==='dark'||(mode==='system'&&systemTheme.matches);}
readTheme();
const clamp=x=>Math.max(0,Math.min(1,x));const ease=x=>{x=clamp(x);return x*x*(3-2*x)};
function state(t){const p=((t%8)+8)%8;let open=0;
if(p>=1.6&&p<2.1)open=ease((p-1.6)/.5);else if(p>=2.1&&p<2.9)open=1;
else if(p>=2.9&&p<3.4)open=1-ease((p-2.9)/.5);else if(p>=5.3&&p<5.8)open=ease((p-5.3)/.5);
else if(p>=5.8&&p<6.5)open=1;else if(p>=6.5&&p<7)open=1-ease((p-6.5)/.5);
let x=0,y=0,alpha=0;if(p>=2.1&&p<2.9){y=-20*(1-ease((p-2.1)/.8));alpha=ease((p-2.2)/.3)}
else if(p>=2.9&&p<5.8)alpha=1;else if(p>=5.8&&p<6.5){x=25*ease((p-5.8)/.7);alpha=1-ease((p-6)/.35)}
return {open,x,y,alpha,phase:p};}
// Small orthographic camera orbit; the mark's center stays anchored.
function camera(t){const rad=Math.PI/180,base=-16*rad,swing=4*rad*Math.sin(t*Math.PI*2/16);
const yaw=base+swing,pitch=base-swing;
return {a:.94*Math.cos(yaw)/Math.cos(base),b:-.1+.94*(Math.sin(pitch)*Math.sin(yaw)-Math.sin(base)**2),d:.94*Math.cos(pitch)/Math.cos(base),zx:.58*Math.sin(yaw),zy:-.68*Math.sin(pitch)*Math.cos(yaw)};}
let view=camera(0);
function solid(path,blue,dx=0,dy=0){ctx.save();ctx.transform(view.a,view.b,0,view.d,dx*view.a,dy*view.d+dx*view.b);
// Identical front-plane geometry for SVG and Canvas: depth does not offset gaps.
for(let i=18;i>=1;i--){ctx.save();ctx.translate(i*view.zx,i*view.zy);ctx.fillStyle=blue?'#3d5e9a':(dark?'#999da3':'#a6abb2');ctx.fill(path);ctx.restore()}
let g=ctx.createLinearGradient(10,8,90,88);
if(blue){g.addColorStop(0,'#a7c3ff');g.addColorStop(.55,'#5688ef');g.addColorStop(1,'#345fca')}
else{g.addColorStop(0,dark?'#e7e8eb':'#f1f2f4');g.addColorStop(.55,dark?'#d9dce0':'#e5e8ec');g.addColorStop(1,dark?'#c2c7ce':'#ccd2d9')}
ctx.fillStyle=g;ctx.fill(path);ctx.lineWidth=.35;ctx.strokeStyle=blue?'#d8e6ff44':'#ffffff22';ctx.stroke(path);ctx.restore();}
function draw(t){const s=state(t),w=canvas.width,h=canvas.height;view=camera(t);ctx.clearRect(0,0,w,h);ctx.save();ctx.translate(w/2,h/2);ctx.scale(w/125,h/125);ctx.translate(-52*view.a,-49*view.d-52*view.b);
solid(paths.base,false);solid(paths.gate,true,s.open*9,-s.open*6);ctx.globalAlpha=s.alpha;solid(paths.token,true,s.x,s.y);ctx.restore();
}
function stop(){cancelAnimationFrame(raf);raf=0;last=0;paint=0}
function start(){if(!raf&&!reduced.matches&&!document.hidden)raf=requestAnimationFrame(loop)}
function loop(ts){raf=0;if(reduced.matches||document.hidden)return;if(last)time+=(ts-last)/1000;last=ts;if(ts-paint>=33){draw(time);paint=ts}start()}
function themeChanged(){readTheme();draw(time)}
window.addEventListener('loom-theme-updated',themeChanged);systemTheme.addEventListener('change',themeChanged);
reduced.addEventListener('change',()=>{stop();if(reduced.matches){time=0;draw(0)}else start()});
document.addEventListener('visibilitychange',()=>document.hidden?stop():start());draw(time);document.documentElement.classList.add('loom-scene-ready');start();
})();
