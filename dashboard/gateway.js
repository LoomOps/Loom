'use strict';
const card=document.querySelector('.card');
// Superellipse (n=4) corners, with a constant pixel radius across card sizes.
// SVG clipping works in Firefox/Safari too, without relying on corner-shape support.
function shape() {
  const w=card.offsetWidth,h=card.offsetHeight;
  if (!w || !h) return;
  const r = Math.min(60,w/3,h/3), points=[];
  for (const [cx,cy,begin] of [[w-r,r,-90],[w-r,h-r,0],[r,h-r,90],[r,r,180]]) {
    for (let i=0;i<=20;i++) {
      const a=(begin+i*90/20)*Math.PI/180, c=Math.cos(a),s=Math.sin(a);
      points.push(`${((cx+r*Math.sign(c)*Math.sqrt(Math.abs(c)))/w).toFixed(6)} ${((cy+r*Math.sign(s)*Math.sqrt(Math.abs(s)))/h).toFixed(6)}`);
    }
  }
  const d='M'+points.join('L')+'Z';
  document.querySelector('#squircle-path').setAttribute('d',d);
  document.querySelector('#squircle-outline').setAttribute('d',d);
}
new ResizeObserver(shape).observe(card);
// One transition per target. Keep brand/host stable while the card resizes and
// content crossfades. Repeated polling does not replay the stage transition.
const reducedMotion=matchMedia('(prefers-reduced-motion: reduce)');
const motionTokens={enter:440,morph:420,content:260,ease:'cubic-bezier(.22,1,.36,1)'};
let motionKey=null,motionShown=false,motionAnimations=[],motionGhost=null;
function clearMotion(keepEntrance=false){
  const keep=[];
  for(const a of motionAnimations){if(keepEntrance&&a.effect?.target===document.querySelector('.card-shadow'))keep.push(a);else a.cancel();}
  motionAnimations=keep;motionGhost?.remove();motionGhost=null;card.style.removeProperty('overflow');
}
function snapshotContent(current){
  if(!current)return null;
  const ghost=current.cloneNode(true),originals=[current,...current.querySelectorAll('*')],copies=[ghost,...ghost.querySelectorAll('*')];
  // A clone loses ID-scoped styles when its IDs are removed. Freeze its rendered
  // layout before detaching it, so the outgoing icon/text never changes size.
  const properties=['display','box-sizing','width','height','min-width','min-height','max-width','max-height','margin','padding','gap','font-family','font-size','font-weight','font-style','line-height','letter-spacing','color','background','border','border-radius','align-items','justify-content','flex-direction','flex','white-space','text-align','text-decoration','overflow','visibility'];
  originals.forEach((el,i)=>{
    if(el instanceof HTMLElement){const computed=getComputedStyle(el);for(const property of properties)copies[i].style.setProperty(property,computed.getPropertyValue(property));}
    copies[i].removeAttribute('id');
  });
  ghost.setAttribute('aria-hidden','true');ghost.inert=true;ghost.classList.add('motion-ghost');
  return ghost;
}
function animateTarget(el,frames,options){
  const animation=el.animate(frames,{easing:motionTokens.ease,fill:'both',...options});motionAnimations.push(animation);
  animation.finished.then(()=>{animation.cancel();motionAnimations=motionAnimations.filter(a=>a!==animation);},()=>{});
  return animation;
}
function beginMotion(key){
  if(document.querySelector('.card-shadow').hidden)return null;
  const entrance=!motionShown;motionShown=true;
  if(!entrance&&key===motionKey)return null;
  motionKey=key;
  const before=card.getBoundingClientRect();
  const height=parseFloat(getComputedStyle(card).height),scale=before.width/card.offsetWidth||1;
  const current=card.querySelector('.screen:not(.motion-ghost):not([hidden])');
  const rect=current?.getBoundingClientRect();
  const opacity=current?Number(getComputedStyle(current).opacity):1;
  const animated=!matchMedia('(prefers-reduced-motion: reduce)').matches&&typeof card.animate==='function';
  const ghost=animated&&!entrance?snapshotContent(current):null;
  clearMotion(true);
  if(!animated)return null;
  if(ghost){
    ghost.style.top=(rect.top-before.top)/scale+'px';ghost.style.left=(rect.left-before.left)/scale+'px';ghost.style.width=rect.width/scale+'px';
  }
  return {entrance,height,ghost,opacity};
}

function finishMotion(previous){
  if(!previous)return;
  const {entrance,height,ghost,opacity}=previous;
  const next=card.offsetHeight,content=card.querySelector('.screen:not(.motion-ghost):not([hidden])');
  if(entrance){
    animateTarget(document.querySelector('.card-shadow'),[{opacity:0,transform:'translateY(14px) scale(.98)'},{opacity:1,transform:'translateY(0) scale(1)'}],{duration:motionTokens.enter});return;
  }
  card.style.overflow='hidden';
  if(ghost){card.append(ghost);motionGhost=ghost;const fade=animateTarget(ghost,[{opacity,transform:'translateY(0)'},{opacity:0,transform:'translateY(-4px)'}],{duration:220});fade.finished.then(()=>ghost.remove(),()=>{});}
  if(content)animateTarget(content,[{opacity:0,transform:'translateY(10px)'},{opacity:1,transform:'translateY(0)'}],{duration:motionTokens.content});
  const resize=animateTarget(card,[{height:height+'px'},{height:next+'px'}],{duration:motionTokens.morph});
  resize.finished.then(()=>{card.style.removeProperty('overflow');shape();},()=>{});
}
reducedMotion.addEventListener('change',()=>{if(reducedMotion.matches)clearMotion();});
let context=null,ready=false,screen='login',requestRow=null,identity=null,progress='';
let probeFrame=null,probeTimer=null,redirectTimer=null,checking=false,refreshing=false,resetting=false,requestVersion=0;
let requestError='',loginError='';
// Input convenience only; this value never authorizes or submits a request.
const emailPreferenceKey='loom:gateway:last-request-email:v1';
function savedEmail(){
  try{const value=localStorage.getItem(emailPreferenceKey)||'';return value.length<=254&&/^[^\s@]+@[^\s@]+$/.test(value)?value:'';}catch{return '';}
}
function rememberEmail(email){try{localStorage.setItem(emailPreferenceKey,email);}catch{}}
document.querySelector('#email').value=savedEmail();
const labels={requested:'Tailscale 공유를 준비하고 있습니다.',invited:'Tailscale 공유 초대 메일을 확인해 주세요.',awaiting_approval:'관리자 승인을 기다리고 있습니다.',approved:'연결 중…',ready:'연결 확인 중…',revoking:'Removing access',revoked:'Access removed',denied:'Access denied',expired:'Request expired'};
function requestFeedback(row){
  if(row.error){
    if(row.error==='access_check_required')return {status:'접속 상태를 확인하지 못했습니다.',note:'공유 초대는 발송하지 않았습니다.\nTailscale 연결을 확인하고, 계속되면 관리자에게 문의해 주세요.'};
    if(row.error==='invite_delivery_uncertain')return {status:'초대 발송을 확인하고 있습니다.',note:'중복 발송을 피하기 위해 확인 중입니다.\n계속 지연되면 관리자에게 문의해 주세요.'};
    if(row.error==='share_identity_changed'||row.error==='share_acceptance_changed')return {status:'공유를 수락한 계정을 확인할 수 없습니다.',note:'관리자에게 문의해 주세요.'};
    return {status:row.state==='requested'?'공유 초대를 준비하지 못했습니다.':'공유 상태를 확인하지 못했습니다.',
      note:row.state==='requested'?'이메일을 확인해 주세요. 올바른 주소라면 관리자에게 문의해 주세요.':'잠시 후 자동으로 다시 확인합니다. 계속되면 관리자에게 문의해 주세요.'};
  }
  if(row.state==='requested'){
    const delayed=Number(row.created)>0 && Date.now()/1000-Number(row.created)>60;
    return {status:delayed?'공유 준비가 지연되고 있습니다.':labels.requested,note:delayed?'자동으로 다시 확인하고 있습니다. 계속 지연되면 관리자에게 문의해 주세요.':'잠시 이 페이지에서 기다려 주세요.'};
  }
  if(row.state==='awaiting_approval')return {status:labels[row.state],note:'이 페이지를 닫아도 됩니다.\n나중에 다시 접속해 확인해 주세요.'};
  return {status:labels[row.state]||'상태 확인 중…',note:row.state==='invited'&&row.sender_email?[row.sender_name,row.sender_email].filter(Boolean).join(' · '):''};
}
function render(){
  const feedbackKey=screen==='sent'&&requestRow?requestFeedback(requestRow):null;
  const transition=beginMotion(JSON.stringify([screen,feedbackKey,requestError,loginError,!!identity,requestRow?.can_cancel]));
  document.querySelectorAll('.screen:not(.motion-ghost)').forEach(el=>el.hidden=el.id!==screen);
  document.querySelector('#entry-progress').hidden=ready&&!progress;
  document.querySelector('#entry-progress-text').textContent=progress||'연결 확인 중…';
  document.querySelector('#recognized').hidden=!identity;
  if(identity){document.querySelector('#recognized-name').textContent=identity.name||identity.login;document.querySelector('#recognized-email').textContent=identity.login;}
  const error=document.querySelector('#login-error');error.textContent=loginError;error.hidden=!loginError;
  document.querySelector('#retry-connection').disabled=checking||resetting;
  if(requestRow){
    document.querySelector('#receipt-email').textContent=requestRow.email;
    const feedback=requestFeedback(requestRow);
    document.querySelector('.pending').textContent=requestError||feedback.status;
    const note=document.querySelector('#request-note');note.textContent=requestError?'':feedback.note;note.hidden=!note.textContent;
    document.querySelector('#edit-email').hidden=!(requestRow.can_cancel&&(requestRow.state==='invited'||requestRow.state==='requested'&&requestRow.error));
    const stage=requestError||requestRow.error?'issue':requestRow.state==='invited'?'mail':requestRow.state==='awaiting_approval'?'approval':'prepare';
    document.querySelectorAll('#request-visual [data-stage]').forEach(el=>el.toggleAttribute('hidden',el.dataset.stage!==stage));
    document.querySelector('#request-spinner').hidden=!!requestError||!!requestRow.error||!['requested','approved','ready','revoking'].includes(requestRow.state);
    document.querySelector('#request-again').hidden=!['expired','revoked'].includes(requestRow.state);
  }
  card.setAttribute('aria-busy',String(!ready||!!progress));shape();finishMotion(transition);
}
async function call(path,body){
  const r=await fetch(path,{credentials:'same-origin',cache:'no-store',signal:AbortSignal.timeout(10000),...(body?{method:'POST',headers:{'Content-Type':'application/json','X-Loom-CSRF':context.csrf},body:JSON.stringify(body)}:{})});
  if(!r.ok){
    const error=await r.json().catch(()=>({}));
    throw new Error(error.code==='access_check_required'?'공유 초대는 발송하지 않았습니다. Tailscale 연결을 확인하고, 계속되면 관리자에게 문의해 주세요.':'요청을 처리하지 못했습니다.');
  }
  return r.json();
}
function stopProbe(){clearTimeout(probeTimer);probeFrame?.remove();probeFrame=null;checking=false;}
function connectionFailed(){
  stopProbe();progress='';
  // Unknown visitors see the single request action; approved users keep their email.
  if(requestRow?.state==='ready')requestError='Tailscale 연결을 확인해 주세요.';
  if(!requestRow&&!identity)screen='request';
  render();
}
function checkConnection(){
  if(!context||checking||redirectTimer)return;
  stopProbe();checking=true;progress='연결 확인 중…';identity=null;loginError='';render();
  const nonce=crypto.randomUUID().replaceAll('-','');
  const frame=document.createElement('iframe');frame.dataset.gatewayProbe=nonce;frame.title='Tailscale connection check';frame.hidden=true;
  frame.src=context.dashboard+'/gateway/probe?nonce='+encodeURIComponent(nonce);probeFrame=frame;
  document.body.append(frame);probeTimer=setTimeout(connectionFailed,5000);
}
addEventListener('message',event=>{
  const value=event.data;
  if(!probeFrame||event.source!==probeFrame.contentWindow||event.origin!==context.dashboard||value?.type!=='loom-gateway-probe'||value.nonce!==probeFrame.dataset.gatewayProbe)return;
  stopProbe();
  if(value.authorized===true&&value.identity&&typeof value.identity.login==='string'&&['/','/auth/login'].includes(value.destination)){
    identity=value.identity;screen='login';progress='연결 중…';render();
    redirectTimer=setTimeout(()=>location.replace(context.dashboard+value.destination),1500);
  }else if(value.reason==='identity_unavailable'){
    progress='';screen='login';loginError='Tailscale 사용자 정보를 확인하지 못했습니다. 연결을 확인하고, 계속되면 관리자에게 문의해 주세요.';render();
  }else connectionFailed();
});
async function refreshRequest(){
  if(refreshing||resetting)return;
  refreshing=true;const version=requestVersion;
  try{
    const data=await call('/api/request');if(version!==requestVersion)return;
    requestRow=data.requests[0]||null;requestError='';
    if(requestRow&&!identity&&screen!=='request')screen='sent';
    if(!requestRow&&screen==='sent')screen='login';
    render();
    if(requestRow?.state==='ready'&&!checking&&!identity)checkConnection();
  }catch(error){
    if(version===requestVersion){requestError='최신 상태를 확인할 수 없습니다.';render();}
    throw error;
  }finally{refreshing=false;}
}
async function init(){
  try{
    context=await call('/api/session');
    const destination=new URL(context.dashboard);if(!['http:','https:'].includes(destination.protocol)||destination.origin!==context.dashboard)throw new Error('Invalid destination');
    document.querySelectorAll('[data-destination]').forEach(el=>el.textContent=context.name);document.title='Loom Ops · '+context.name;
    if(typeof context.name!=='string'||!context.name.trim())throw new Error('Missing host');
    document.querySelector('.card-shadow').hidden=false;document.querySelector('#gateway-loading').hidden=true;
    ready=true;progress='연결 확인 중…';render();await refreshRequest();render();checkConnection();
  }catch{ready=true;loginError='게이트웨이에 연결하지 못했습니다.';document.querySelector('#gateway-loading span:last-child').textContent=loginError;progress='';render();}
}
async function openRequest(event){
  const editing=event?.currentTarget?.id==='edit-email';
  if(resetting)return;
  resetting=true;requestVersion++;stopProbe();clearTimeout(redirectTimer);redirectTimer=null;
  try{
    if(context.user?.login){await call('/api/request/reset',{});context=await call('/api/session');}
    requestRow=null;identity=null;requestError='';loginError='';progress='';
    document.querySelector('#email').value=editing?'':savedEmail();document.querySelector('#request-error').hidden=true;
    screen='request';render();document.querySelector('#email').focus();
  }catch(e){if(screen==='sent')requestError='신청 상태를 다시 확인해 주세요.';else loginError=e.message;render();}
  finally{resetting=false;render();}
}
document.querySelector('#edit-email').onclick=openRequest;
document.querySelector('#request-again').onclick=openRequest;
document.querySelector('#retry-connection').onclick=async()=>{
  if(!context||document.querySelector('.card-shadow').hidden){await init();return;}
  await refreshRequest().catch(()=>{});checkConnection();
};
document.querySelector('#request-form').onsubmit=async event=>{
  event.preventDefault();const button=event.submitter;button.disabled=true;
  try{
    const email=document.querySelector('#email').value.trim();
    context=await call('/api/session');
    if(context.user?.login && context.user.login.toLowerCase()!==email.toLowerCase()){
      await call('/api/request/reset',{});context=await call('/api/session');
    }
    await call('/api/request',{email});
    rememberEmail(email);
    context=await call('/api/session');screen='sent';await refreshRequest();render();
  }catch(e){const error=document.querySelector('#request-error');error.textContent=e.message;error.hidden=false;screen='request';render();}
  finally{button.disabled=false;}
};
setInterval(()=>{if(context&&!resetting&&screen==='sent'&&!document.hidden)refreshRequest().catch(()=>{});},15000);
function refreshOnReturn(){if(context&&!resetting&&screen==='sent'&&!document.hidden)refreshRequest().catch(()=>{});}
addEventListener('focus',refreshOnReturn);document.addEventListener('visibilitychange',refreshOnReturn);
addEventListener('pagehide',()=>{stopProbe();clearTimeout(redirectTimer);clearMotion();});
render();init();
