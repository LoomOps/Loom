// The top-level document is the only transport and credential owner.
(() => {
  const frame = document.getElementById('codex-visualization');
  const access = document.getElementById('loom-entry');
  const status = document.getElementById('loom-access-status');
  let csrf = '';
  let ssoEnabled = false;
  let sessionUser = null;
  let tailnetInfo = null;
  let authenticated = false;
  const signedIn = () => authenticated;
  let lifetime = new AbortController();
  let catalogRequest;
  let resourceInterest = false;
  let resourceTimer;
  let resourceBusy = false;
  let catalogLoaded = false;
  let catalogEntities = new Map();
  let mediaRequest;
  let revisionTimer;
  let catalogRevision;
  let buildInfo;
  let entryURL = '';
  let updateTimer;
  let updatesBusy = false;
  const retry = document.getElementById('loom-entry-retry');
  retry.addEventListener('click',()=>location.reload());

  function showBuild(build) {
    buildInfo = build;
    if (build) emit('loom-build', {build});
    emit('loom-session', {user:sessionUser,tailnet:tailnetInfo});
    emit('loom-theme', {mode:window.LoomTheme.read()});
  }

  function emit(type, fields = {}) {
    frame.contentWindow?.postMessage({type, ...fields}, '*');
  }

  window.addEventListener('loom-theme-updated',event=>emit('loom-theme',{mode:event.detail}));

  function authError(error) {
    return error.status === 401 || error.invalidCredential;
  }

  function reset(message = '') {
    lifetime.abort();
    lifetime = new AbortController();
    clearTimeout(resourceTimer);
    clearTimeout(revisionTimer);
    clearTimeout(updateTimer);
    authenticated = false;
    sessionUser = null;
    emit('loom-session',{user:null});
    catalogEntities.clear();
    status.textContent = message;
    access.hidden = false;
    frame.hidden = true;
    if (entryURL) {location.replace(entryURL+'/');return;}
    document.getElementById('loom-sso-login').hidden = !ssoEnabled;
    retry.hidden = false;
  }

  async function get(path, signal = lifetime.signal) {
    const response = await fetch(path, {
      cache: 'no-store', credentials: 'same-origin', redirect: 'error',
      signal: AbortSignal.any([signal, AbortSignal.timeout(15000)]),
    });
    if (!response.ok) {
      const error = new Error('HTTP ' + response.status);
      error.status = response.status;
      if (response.status === 403) {
        const body = await response.json().catch(() => ({}));
        error.invalidCredential = ['Invalid token', 'Invalid or expired credential'].includes(body.error);
      }
      throw error;
    }
    return response;
  }

  async function read(path, signal) {
    return (await get(path, signal)).json();
  }

  async function refreshCatalog() {
    if (!signedIn() || catalogRequest) return;
    const signal = lifetime.signal;
    catalogRequest = read('/api/dashboard/catalog', signal);
    try {
      const incoming = await catalogRequest;
      if (signal.aborted) return;
      if (!Array.isArray(incoming.entities)) throw new Error('Invalid catalog response');
      catalogRevision = incoming.revision;
      const catalog = loomCatalogProjection(incoming);
      catalogEntities = new Map(catalog.entities.map(entity => [entity.id, entity]));
      emit('pai-catalog-data', {catalog, connected: true});
      window.dispatchEvent(new Event('pai-auth-restored'));
      catalogLoaded = true;
      frame.hidden = false;
      access.hidden = true;
    } catch (error) {
      if (signal.aborted) return;
      if (authError(error)) return reset('현재 credential로 다시 연결해 주세요.');
      if (error.status === 403) {
        emit('pai-catalog-data', {catalog: {entities: [], sources: []}, connected: false});
        emit('pai-catalog-status', {text: '카탈로그 접근 권한 없음'});
        return reset('카탈로그 조회 권한이 없습니다.');
      }
      const message = catalogLoaded ? '서버 연결 불가 · 마지막 조회 기록' : '카탈로그 연결 불가 · 다시 연결해 주세요.';
      emit('pai-catalog-status', {text: message});
      status.textContent = message;
      if (!catalogLoaded) {access.hidden=false;retry.hidden=false;}
    } finally {
      catalogRequest = null;
    }
  }

  function scheduleRevision() {
    clearTimeout(revisionTimer);
    if (!catalogLoaded || !signedIn() || document.hidden) return;
    revisionTimer = setTimeout(async () => {
      const signal = lifetime.signal;
      try {
        const {revision} = await read('/api/dashboard/revision', signal);
        if (!signal.aborted && revision !== catalogRevision) await refreshCatalog();
      } catch (error) {
        if (signal.aborted) return;
        if (authError(error)) return reset('현재 credential로 다시 연결해 주세요.');
        if (error.status !== 404) emit('pai-catalog-status', {text:'변경 확인 불가 · 마지막 조회 기록'});
      } finally {
        if (!signal.aborted) scheduleRevision();
      }
    }, 30000);
  }

  async function resources() {
    clearTimeout(resourceTimer);
    if (!signedIn() || !resourceInterest || document.hidden || resourceBusy) return;
    resourceBusy = true;
    const signal = lifetime.signal;
    let delay = 2000;
    try {
      const resources = await read('/api/dashboard/resources', signal);
      if (!signal.aborted && resourceInterest) emit('pai-resource-data', {resources: {...resources, host:loomHost(resources.host)}});
    } catch (error) {
      if (signal.aborted) return;
      if (authError(error)) return reset('현재 credential로 다시 연결해 주세요.');
      emit('pai-resource-error', {status: error.status});
      delay = 10000;
    } finally {
      resourceBusy = false;
      if (!signal.aborted && signedIn() && resourceInterest && !document.hidden) {
        resourceTimer = setTimeout(resources, delay);
      }
    }
  }

  async function updates(check = false) {
    if (!signedIn() || updatesBusy || document.hidden) return;
    updatesBusy = true;
    clearTimeout(updateTimer);
    const signal=lifetime.signal;
    try {
      let response;
      if (check) {
        response=await fetch('/api/updates/check',{method:'POST',credentials:'same-origin',redirect:'error',signal:AbortSignal.any([signal,AbortSignal.timeout(15000)]),
          headers:{'Content-Type':'application/json',...(csrf?{'X-Loom-CSRF':csrf}:{})},body:'{}'});
        if (!response.ok) throw new Error('check_failed');
        const receipt=await response.json();
        emit('loom-updates',{updates:receipt.updates});
      } else {
        const value=await read('/api/updates',signal);
        emit('loom-updates',{updates:value});
      }
    } catch(error) {
      if (!signal.aborted) emit('loom-updates',{updates:{status:'error',error:'connection_unavailable'}});
    } finally {
      updatesBusy=false;
      if (!signal.aborted) updateTimer=setTimeout(()=>updates(),check?1500:60000);
    }
  }

  async function docker() {
    if (!signedIn()) return;
    const signal = lifetime.signal;
    try {
      const docker = await read('/api/docker', signal);
      if (!signal.aborted) emit('loom-docker', {docker});
    } catch (error) {
      if (signal.aborted) return;
      if (authError(error)) return reset('현재 credential로 다시 연결해 주세요.');
      emit('loom-docker', {error: error.status || 503});
    }
  }

  async function mcp() {
    if (!signedIn()) return;
    const signal = lifetime.signal;
    try {
      const [init, listing, reference] = await Promise.all([
        read('/api/mcp/initialize', signal), read('/api/mcp/tools', signal), read('/api/mcp/docs', signal),
      ]);
      if (init.error || listing.error || !init.result?.serverInfo || !Array.isArray(listing.result?.tools)) {
        throw new Error('Invalid MCP response');
      }
      if (reference.result?.isError) throw new Error('Documentation unavailable');
      const docs=JSON.parse(reference.result.content.find(item=>item.type==='text').text);
      if (!signal.aborted) emit('loom-mcp', {mcp:{...init.result, tools:listing.result.tools, docs, observed_at:new Date().toISOString()}});
    } catch (error) {
      if (signal.aborted) return;
      if (authError(error)) return reset('현재 credential로 다시 연결해 주세요.');
      emit('loom-mcp', {error:error.status || 503});
    }
  }

  async function detail({identity, kind, requestId}) {
    if (!signedIn() || typeof identity !== 'string' || !Number.isSafeInteger(requestId)) return;
    const entity = catalogEntities.get(identity);
    const signal = lifetime.signal;
    let path;
    if (kind === 'history' && entity?.loom_history) {
      const {source, kind, id, revision} = entity.loom_history;
      path = '/api/history/get?' + new URLSearchParams({source, kind, id, revision});
    } else if (kind === 'registry' && /^[a-zA-Z0-9][a-zA-Z0-9_.:-]{0,159}$/.test(entity?.loom_registry?.id || '')) {
      path = '/api/registry/' + entity.loom_registry.id;
    } else if (kind === 'job' && /^[a-zA-Z0-9][a-zA-Z0-9_.:-]{0,159}$/.test(entity?.loom_job?.id || '')) {
      path = '/api/jobs/' + entity.loom_job.id;
    } else {
      return;
    }
    try {
      const data = await read(path, signal);
      if (!signal.aborted) emit('loom-detail-response', {identity, requestId, data});
    } catch (error) {
      if (signal.aborted) return;
      if (authError(error)) return reset('현재 credential로 다시 연결해 주세요.');
      emit('loom-detail-response', {identity, requestId, error: error.status || 503});
    }
  }

  async function queueAction(message) {
    if (!signedIn() || !['reorder','hold','resume'].includes(message.action)) return;
    const signal=lifetime.signal;
    try {
      const response=await fetch('/api/jobs/queue-actions', {method:'POST',cache:'no-store',credentials:'same-origin',redirect:'error',
        headers:{'Content-Type':'application/json',...(csrf?{'X-Loom-CSRF':csrf}:{})},
        body:JSON.stringify({action:message.action,arguments:message.arguments}),
        signal:AbortSignal.any([signal,AbortSignal.timeout(15000)])});
      if(!response.ok){const error=new Error('HTTP '+response.status);error.status=response.status;throw error;}
      const data=await response.json();
      await refreshCatalog();
      if(!signal.aborted)emit('loom-queue-result',{data});
    } catch(error) {
      if(signal.aborted)return;
      if(authError(error))return reset('현재 credential로 다시 연결해 주세요.');
      await refreshCatalog();
      emit('loom-queue-result',{error:error.status||503});
    }
  }

  async function media({entity, id}, download = false) {
    if (!signedIn() || typeof entity !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9_.:-]{0,159}$/.test(id || '')) return;
    const item = catalogEntities.get(entity)?.loom_media?.find(item => item.id === id);
    if (!item || item.status !== 'available') return;
    mediaRequest?.abort();
    mediaRequest = new AbortController();
    const signal = AbortSignal.any([mediaRequest.signal, lifetime.signal]);
    try {
      const response = await get('/api/dashboard/media/' + item.id, signal);
      const blob = await response.blob();
      if (blob.type !== item.mime || (!blob.size && !['text/markdown','text/plain','application/json','application/x-ndjson'].includes(item.mime))) throw new Error('Invalid media response');
      if (signal.aborted) return;
      if (!download) return emit('loom-media-response', {entity, id, label:item.label, blob});
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = (item.label || 'loom-media').replace(/[\\/\r\n]/g, '_');
      document.body.append(link);
      link.click();
      link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 30000);
    } catch (error) {
      if (signal.aborted) return;
      if (authError(error)) return reset('현재 credential로 다시 연결해 주세요.');
      emit('loom-media-response', {entity, id, error:error.status || 503});
    }
  }

  function showIssuedToken(result){
    const el=(tag,cls,text)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(text)node.textContent=text;return node;};
    const modal=el('dialog','loom-token-dialog');modal.setAttribute('aria-labelledby','loom-token-title');
    const header=el('header','loom-token-head'),heading=el('div');
    const title=el('h2','','Agent token');title.id='loom-token-title';
    heading.append(el('span','loom-token-eyebrow','CREDENTIAL'),title);
    const dismiss=el('button','loom-token-dismiss','×');dismiss.setAttribute('aria-label','Close');dismiss.onclick=()=>modal.close();header.append(heading,dismiss);
    const note=el('p','loom-token-note','이 토큰은 지금 한 번만 표시됩니다.');
    const value=el('textarea','loom-token-value');value.readOnly=true;value.rows=2;value.setAttribute('aria-label','Issued token');value.value=result.token;
    const copy=el('button','loom-token-button loom-token-primary','Copy token');
    copy.onclick=async()=>{try{await navigator.clipboard.writeText(value.value);copy.textContent='Copied';}catch{value.select();copy.textContent='선택한 토큰을 복사해 주세요';}};
    const secret=el('section','loom-token-section');secret.append(el('h3','','Token'),value,copy);
    const footer=el('footer','loom-token-footer');const close=el('button','loom-token-button','Done');close.onclick=()=>modal.close();
    footer.append(el('span','loom-token-hint','No expiration · Revoke anytime'),close);
    modal.addEventListener('close',()=>{value.value='';result.token='';modal.remove();});
    modal.append(header,note,secret,footer);document.body.append(modal);modal.showModal();copy.focus();
  }

  function confirmTokenRevoke(label){
    return new Promise(resolve=>{
      const modal=document.createElement('dialog');modal.className='loom-token-dialog loom-token-confirm';
      modal.setAttribute('aria-labelledby','loom-revoke-title');
      const title=document.createElement('h2');title.id='loom-revoke-title';title.textContent='Revoke token?';
      const name=document.createElement('p');name.className='loom-token-name';name.textContent=label||'Agent token';
      const note=document.createElement('p');note.className='loom-token-note';note.textContent='이 토큰을 사용하는 에이전트의 접근이 차단됩니다.';
      const actions=document.createElement('div');actions.className='loom-token-actions';
      const cancel=document.createElement('button');cancel.className='loom-token-button';cancel.textContent='Cancel';cancel.onclick=()=>modal.close('cancel');
      const revoke=document.createElement('button');revoke.className='loom-token-button loom-token-danger';revoke.textContent='Revoke';revoke.dataset.confirmRevoke='true';revoke.onclick=()=>modal.close('revoke');
      actions.append(cancel,revoke);modal.append(title,name,note,actions);
      modal.addEventListener('close',()=>{const confirmed=modal.returnValue==='revoke';modal.remove();resolve(confirmed);},{once:true});
      document.body.append(modal);modal.showModal();cancel.focus();
    });
  }

  let accessBusy=false;
  async function accessAdmin(message){
    if(!sessionUser?.access_admin)return;
    if(accessBusy){
      if(['loom-access-decision','loom-share-revoke','loom-token-issue','loom-token-revoke','loom-ssh-change'].includes(message.type))emit('loom-access-response',{error:'조회 중입니다. 잠시 후 다시 시도해 주세요.'});
      return;
    }
    if(message.type==='loom-access-decision' &&
       (!['approve','deny','retry','revoke','remove'].includes(message.action)||typeof message.id!=='string'||message.id.length>128))return;
    accessBusy=true;
    try {
      if(message.type==='loom-token-revoke'){
        if(typeof message.id!=='string'||!message.id||message.id.length>160)throw new Error('invalid_token');
        if(!await confirmTokenRevoke(typeof message.label==='string'?message.label.slice(0,80):'')){emit('loom-access-response',{});return;}
      }
      if(['loom-access-decision','loom-share-revoke'].includes(message.type)){
        if(typeof message.id!=='string'||message.id.length>128)return;
        const session=await read('/admission/session');
        const response=await fetch(message.type==='loom-share-revoke'?'/admission/share-revoke':'/admission/decide',{method:'POST',credentials:'same-origin',redirect:'error',
          headers:{'Content-Type':'application/json','X-Loom-CSRF':session.csrf},
          body:JSON.stringify({id:message.id,action:message.action}),signal:AbortSignal.timeout(15000)});
        if(!response.ok){const error=new Error('decision');error.status=response.status;throw error;}
      }
      if(['loom-token-issue','loom-token-revoke'].includes(message.type)){
        const session=await read('/admission/session');
        const response=await fetch(message.type==='loom-token-issue'?'/api/admin/tokens':'/api/admin/tokens/revoke',{
          method:'POST',credentials:'same-origin',redirect:'error',headers:{'Content-Type':'application/json','X-Loom-CSRF':session.csrf},
          body:JSON.stringify(message.type==='loom-token-issue'?message.body:{id:message.id}),signal:AbortSignal.timeout(15000)});
        if(!response.ok){const error=new Error('token');error.status=response.status;throw error;}
        const result=await response.json();if(result.token)showIssuedToken(result);
      }
      if(message.type==='loom-ssh-change'){
        const session=await read('/admission/session');
        const response=await fetch('/admission/ssh',{method:'POST',credentials:'same-origin',redirect:'error',headers:{'Content-Type':'application/json','X-Loom-CSRF':session.csrf},body:JSON.stringify(message.body),signal:AbortSignal.timeout(60000)});
        if(!response.ok){const error=new Error('ssh');error.status=response.status;throw error;}
      }
      const [requests,members,tokens,ssh]=await Promise.allSettled([read('/admission/requests'),read('/admission/members'),read('/api/admin/tokens'),read('/admission/ssh')]);
      if(requests.status==='rejected')throw requests.reason;
      emit('loom-access-response',{data:{...requests.value,ssh:ssh.status==='fulfilled'?ssh.value:null,ssh_error:ssh.status==='rejected',members:members.status==='fulfilled'?members.value.members:[],members_error:members.status==='rejected',tokens:tokens.status==='fulfilled'?tokens.value.tokens:[],owners:tokens.status==='fulfilled'?tokens.value.owners:[],owner_profiles:tokens.status==='fulfilled'?tokens.value.owner_profiles:[],tokens_error:tokens.status==='rejected'}});
    } catch(error){
      emit('loom-access-response',{error:error.status===401?'로그인이 만료되었습니다. 다시 로그인해 주세요.':error.status===403?'Admin 권한이 필요합니다.':'요청을 처리하지 못했습니다. 새로고침 후 상태를 확인해 주세요.'});
    } finally {accessBusy=false;}
  }

  window.addEventListener('message', event => {
    if (event.source !== frame.contentWindow || event.origin !== 'null' || !event.data) return;
    const message = event.data;
    if (message.type === 'loom-open-management' && tailnetInfo?.development && tailnetInfo.management_url) {
      location.assign(tailnetInfo.management_url+'/#/views/tailscale');
    } else if (message.type === 'loom-theme-choice') {
      window.LoomTheme.set(message.mode);
    } else if (message.type === 'loom-logout' && ssoEnabled && csrf) {
      fetch('/auth/logout', {method:'POST',credentials:'same-origin',headers:{'X-Loom-CSRF':csrf},redirect:'follow'})
        .then(response => {if(response.ok)location.reload();});
    } else if (['loom-access-request','loom-access-decision','loom-share-revoke','loom-token-issue','loom-token-revoke','loom-ssh-change'].includes(message.type)) {
      accessAdmin(message);
    } else if (message.type === 'pai-resource-interest') {
      resourceInterest = message.active === true;
      resources();
    } else if (message.type === 'pai-catalog-ready' || message.type === 'pai-catalog-refresh') {
      showBuild(buildInfo);
      refreshCatalog();
      updates();
    } else if (message.type === 'loom-queue-action') {
      queueAction(message);
    } else if (message.type === 'loom-detail-request') {
      detail(message);
    } else if (message.type === 'loom-media-request' || message.type === 'loom-media-download') {
      media(message, message.type === 'loom-media-download');
    } else if (message.type === 'loom-updates-check') {
      updates(true);
    } else if (message.type === 'loom-docker-request') {
      docker();
    } else if (message.type === 'loom-doc-copy') {
      if (typeof message.text !== 'string' || message.text.length>65536) return;
      Promise.resolve().then(()=>navigator.clipboard.writeText(message.text)).then(()=>emit('loom-doc-copied',{ok:true}),()=>emit('loom-doc-copied',{ok:false}));
    } else if (message.type === 'loom-mcp-request') {
      mcp();
    }
  });
  window.addEventListener('online', refreshCatalog);
  document.addEventListener('visibilitychange', () => {
    resources();
    if (!document.hidden && catalogLoaded) {refreshCatalog();updates();}
    scheduleRevision();
  });
  window.addEventListener('pagehide', () => {
    lifetime.abort();
    clearTimeout(resourceTimer);
    clearTimeout(revisionTimer);
    clearTimeout(updateTimer);
  });
  window.addEventListener('pageshow', event => {
    if (event.persisted) location.reload();
  });
  fetch('/session', {cache:'no-store', credentials:'same-origin', redirect:'error'})
    .then(response => { if(!response.ok) throw new Error('session'); return response.json(); })
    .then(async session => {
      showBuild(session.build);
      ssoEnabled = session.sso === true;
      csrf = session.csrf || '';
      entryURL = session.entry_url || '';
      document.getElementById('loom-sso-login').hidden = !ssoEnabled;
      sessionUser = session.user || null;
      tailnetInfo = session.tailnet || null;
      emit('loom-session',{user:sessionUser,tailnet:tailnetInfo});
      if (session.live_reload) setInterval(async () => {
        if (document.hidden) return;
        try {
          const latest = await fetch('/session', {cache:'no-store'}).then(r => r.json());
          sessionUser=latest.user || null;
          tailnetInfo=latest.tailnet || null;
          showBuild(latest.build);
          if (latest.revision !== session.revision) location.reload();
        } catch (_) {}
      }, 1000);
      authenticated = session.authentication_required === false;
      if (!authenticated) {
        if(session.entry_url){location.replace(session.entry_url+'/');return;}
        status.textContent = ssoEnabled ? '' : 'Tailscale 연결을 확인해 주세요. 계속 실패하면 관리자에게 문의해 주세요.';
        retry.hidden = false; access.hidden = false; return;
      }
      status.textContent = '연결 중';
      await refreshCatalog();
      resources();
      scheduleRevision();
      updates();
    }).catch(() => { access.hidden = false; status.textContent = '연결 설정을 읽을 수 없습니다.';retry.hidden=false; });
})();
