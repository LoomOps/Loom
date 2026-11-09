let loomUpdateState = null;
function loomUpdateLabels(value) {
  return ({update_available:'Update available',up_to_date:'Up to date',checking:'Checking…',not_checked:'Not checked',no_release:'No release',unversioned:'Development build',disabled:'Checks disabled',error:'Check unavailable',not_running:'Check unavailable'})[value?.status] || 'Check for updates';
}
function loomUpdateBadge() {
  const footer=root.querySelector('.pa-footer');
  if (!footer) return;
  let button=footer.querySelector('[data-loom-updates]');
  if (!button) {
    button=document.createElement('button');button.type='button';button.dataset.loomUpdates='';
    button.addEventListener('click',()=>{
      const open=root.querySelector('.loom-update-panel');
      if (open) {open.remove();return;}
      const panel=document.createElement('section');panel.className='loom-update-panel';panel.setAttribute('aria-label','Loom releases');
      footer.append(panel);loomUpdatePanel();
    });
    footer.append(button);
  }
  button.textContent=loomUpdateLabels(loomUpdateState);
  button.classList.toggle('available',loomUpdateState?.status==='update_available');
  button.title='GitHub Releases';
  loomUpdatePanel();
}
function loomUpdatePanel() {
  const panel=root.querySelector('.loom-update-panel');
  if (!panel) return;
  const value=loomUpdateState || {};
  const errors={repository_unavailable:'저장소가 없거나 GitHub 읽기 권한이 없습니다.',credential_rejected:'GitHub credential을 갱신해 주세요.',access_denied:'GitHub 읽기 권한을 확인해 주세요.',rate_limited:'GitHub 요청 제한 후 다시 확인합니다.',network_unavailable:'GitHub에 연결할 수 없습니다.',repository_moved:'변경된 GitHub 저장소 주소를 설정해 주세요.',cache_unwritable:'릴리스 확인 기록을 저장할 수 없습니다.'};
  const latest=value.latest;
  const releaseURL=latest?.url && /^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/releases\/tag\//.test(latest.url) ? latest.url : null;
  panel.innerHTML='<strong>Loom releases</strong><button class="loom-update-close" aria-label="Close">×</button>'
    +'<p>'+esc(loomUpdateLabels(value))+'</p>'
    +'<dl><dt>Installed</dt><dd>'+esc(value.installed || loomBuildInfo?.image_build_id || '—')+'</dd>'
    +'<dt>Latest</dt><dd>'+(releaseURL?'<a href="'+esc(releaseURL)+'" target="_blank" rel="noopener noreferrer">'+esc(latest.tag)+'</a>':'—')+'</dd>'
    +'<dt>Last checked</dt><dd>'+esc(value.checked_at?new Date(value.checked_at*1000).toLocaleString():'—')+'</dd></dl>'
    +(value.error?'<p role="status">'+esc(errors[value.error]||'릴리스 확인에 실패했습니다. 다시 시도해 주세요.')+'</p>':'')
    +(value.stale?'<p>마지막 성공 시점의 결과입니다.</p>':'')
    +'<button data-update-now '+(value.checking || value.status==='disabled'?'disabled':'')+'>Check for updates</button>';
  panel.querySelector('.loom-update-close').onclick=()=>panel.remove();
  panel.querySelector('[data-update-now]').onclick=()=>{parent.postMessage({type:'loom-updates-check'},'*');};
}
window.addEventListener('message',event=>{
  if (event.source!==parent || event.data?.type!=='loom-updates') return;
  loomUpdateState=event.data.updates;loomUpdateBadge();
});

// Added Loom observations within the original workspace navigation.
let loomBuildInfo = null;
window.addEventListener('message', event => {
  if (event.source !== parent || event.data?.type !== 'loom-build') return;
  const build = event.data.build;
  loomBuildInfo = build;
  const label = root.querySelector('.pa-footer > span');
  if (!label || !build) return;
  label.textContent = 'Loom · ' + (build.image_build_id || build.build_id);
  label.title = '이미지 기준 빌드: ' + (build.image_build_id || build.build_id) + '\n라이브 소스 수정은 새 이미지 빌드와 별개입니다.\n' + (build.source_sha256 || '');
  catHostRefresh();
  loomUpdateBadge();
});
let loomDockerState = null;
let loomDockerError = null;
let loomDockerOpen = false;
let loomMcpState = null;
let loomMcpError = null;
let loomMcpOpen = false;

function loomContainerDetails(row) {
  const fields = [
    ['Image 상태',row.image_status === 'removed' ? '이미지 삭제됨 · 종료 컨테이너의 참조' : row.image_status === 'available' ? '로컬 이미지 존재' : '미확인'],['Container ID',row.id],['Image ID',row.image_id],['Image digests',row.image_digests?.join(', ')],
    ['시작',row.started_at ? resTime(row.started_at) : null],['종료',row.finished_at ? resTime(row.finished_at) : null],
    ['Exit code',row.exit_code],['Network I/O',row.stats?.network_io],['Block I/O',row.stats?.block_io],
    ['메모리 %',row.stats?.memory_percent],['재시작',row.restart_count],
  ];
  return catProperties(Object.fromEntries(fields))
    + (row.job_id ? '<p>Job · ' + (catEntity(row.job_id) ? catLink(row.job_id) : esc(row.job_id)) + '</p>' : '') ;
}

function loomDockerView() {
  const docker = loomDockerState;
  const labels = {ok:'정상', partial:'일부 계측', stale:'이전 계측', unavailable:'계측 불가'};
  let body = '<p>Docker 관측값을 읽는 중입니다.</p>';
  if (loomDockerError) {
    body = loomDockerError === 403 ? '<p>Docker 조회 권한이 없습니다.</p>' : '<p>Docker 상태를 조회할 수 없습니다.</p>';
  } else if (docker) {
    const summary = '<p>Docker Engine ' + esc(docker.engine?.version || '—') + ' · '
      + esc(labels[docker.status] || docker.status || '미확인') + ' · ' + esc(resTime(docker.observed_at)) + '</p>'
      + (docker.errors?.length ? '<p class="pa-subtle">' + docker.errors.map(esc).join(' · ') + '</p>' : '');
    const rows = docker.containers || [];
    const groups = [
      ['Loom · 배포 버전', rows.filter(row => row.compose_project === 'loom')],
      ['Loom · dev-build', rows.filter(row => row.compose_project === 'loom-dev-build')],
      ['Loom · 기타', rows.filter(row => row.compose_project?.startsWith('loom-') && row.compose_project !== 'loom-dev-build')],
      ['기타 컨테이너', rows.filter(row => row.compose_project !== 'loom' && !row.compose_project?.startsWith('loom-'))],
    ];
    body = summary + '<p class="pa-subtle">전체 ' + rows.length + '개 · 실행 중 '
      + rows.filter(row => row.state === 'running').length + '개</p>'
      + groups.filter(([, items]) => items.length).map(([label, items]) => {
        const head = ['컨테이너','Compose / 서비스','상태','Health','이미지','CPU','메모리'];
        const body = items.map(row => {
          const values = [esc(row.name), esc(row.compose_project || '—') + '<small>' + esc(row.compose_service || '—') + '</small>',
            '<span class="pa-container-state" data-state="'+esc(row.state)+'">'+esc(row.state)+'</span>',
            esc(row.health === 'not_configured' ? '미설정' : row.health),
            esc(row.image), esc(row.stats?.cpu_percent == null ? '—' : String(row.stats.cpu_percent).replace(/%+$/, '') + '%'),
            esc(row.stats?.memory_usage || '—')];
          return '<tr data-container-row="'+esc(row.name)+'" tabindex="0" aria-expanded="false" aria-label="'+esc(row.name)+' 상세">'+values.map(v=>'<td>'+v+'</td>').join('')+'</tr>'
            + '<tr hidden data-container-extra="'+esc(row.id)+'"><td colspan="7">'+loomContainerDetails(row)+'</td></tr>';
        }).join('');
        return panel(esc(label) + ' <small>' + items.length + '</small>', '<div class="pa-table-wrap" tabindex="0" aria-label="'+esc(label)+' 컨테이너 목록"><table class="pa-container-table"><colgroup>'
          + [23,14,9,10,23,7,14].map(w=>'<col style="width:'+w+'%">').join('')
          + '</colgroup><thead><tr>'+head.map(h=>'<th>'+esc(h)+'</th>').join('')+'</tr></thead><tbody>'+body+'</tbody></table></div>');
      }).join('');
    if (!(docker.containers || []).length) body += '<p>표시할 컨테이너 관측값이 없습니다.</p>';
  }
  return heading('RESOURCES','Docker','컨테이너 상태와 자원 사용량')
    + '<button class="pa-button" type="button" data-loom-docker-refresh>새로고침</button>'
    + '<section class="pa-loom-docker" aria-label="Docker 상태">' + body + '</section>';
}

const loomRender = render;
render = function() {
  loomRender();
  const open = Boolean(root.querySelector('.pa-loom-docker'));
  if (open && !loomDockerOpen) parent.postMessage({type:'loom-docker-request'}, '*');
  loomDockerOpen = open;
  const mcpOpen = Boolean(root.querySelector('.pa-loom-mcp'));
  if (mcpOpen && !loomMcpOpen) parent.postMessage({type:'loom-mcp-request'}, '*');
  loomMcpOpen = mcpOpen;
};
function loomToggleContainer(row) {
  const detail = row.nextElementSibling;
  detail.hidden = !detail.hidden;
  row.setAttribute('aria-expanded', String(!detail.hidden));
}
root.addEventListener('keydown', event => {
  if (!['Enter',' '].includes(event.key) || !event.target.matches('[data-container-row]')) return;
  event.preventDefault();
  loomToggleContainer(event.target);
});
root.addEventListener('click', event => {
  const row = event.target.closest('[data-container-row]');
  if (row && !event.target.closest('a,button,input') && !window.getSelection()?.toString()) loomToggleContainer(row);
  if (event.target.closest('[data-loom-docker-refresh]')) parent.postMessage({type:'loom-docker-request'}, '*');
  if (event.target.closest('[data-loom-mcp-refresh]')) parent.postMessage({type:'loom-mcp-request'}, '*');
});
window.addEventListener('message', event => {
  if (event.source !== parent || event.data?.type !== 'loom-mcp') return;
  loomMcpState = event.data.mcp || null;
  loomMcpError = event.data.error || null;
  for (const panel of root.querySelectorAll('.pa-loom-mcp')) {
    const template = document.createElement('template');
    template.innerHTML = loomMcpView();
    patchResourceNode(panel,template.content.querySelector('.pa-loom-mcp'),true);
  }
});
window.addEventListener('message', event => {
  if (event.source !== parent || event.data?.type !== 'loom-docker') return;
  loomDockerState = event.data.docker || null;
  loomDockerError = event.data.error || null;
  for (const panel of root.querySelectorAll('.pa-loom-docker')) {
    const template = document.createElement('template');
    template.innerHTML = loomDockerView();
    patchResourceNode(panel,template.content.querySelector('.pa-loom-docker'),true);
  }
});

const loomResourceContents = resourceContents;
resourceContents = function() {
  if (resourceError === 'denied') return '<p>리소스 조회 권한이 없습니다.</p>';
  return loomResourceContents();
};
window.addEventListener('message', event => {
  if (event.source !== parent || event.data?.type !== 'pai-resource-error' || event.data.status !== 403) return;
  resourceData = null;
  resourceError = 'denied';
  for (const panel of root.querySelectorAll('[data-resource-panel]')) refreshResourcePanel(panel);
});

// The same host sample supplies both the Inspector and the persistent sidebar.
const loomHostRefresh = catHostRefresh;
catHostRefresh = function(host = catalogData.host) {
  loomHostRefresh(host);
  const bottom = root.querySelector('.pa-sidebar-bottom');
  if (bottom && host) {
    const inventory = host.inventory || {};
    const next=bottom.cloneNode(false);
    next.innerHTML = '<div class="pa-node-summary"><small>' + esc(host.os || inventory.os || 'OS 미확인')
      + '</small><small>Kernel · ' + esc(host.kernel || inventory.kernel || '미확인')
      + '</small>' + (inventory.gpu ? '<small>NVIDIA Driver · '+esc(inventory.gpu.split(',').at(-1).trim())+'</small>' : '')
      + (inventory.docker_engine ? '<small>Docker · '+esc(inventory.docker_engine)+'</small>' : '')
      + (host.status !== 'ok' ? '<small>호스트 정보 '+(host.status === 'stale' ? '갱신 지연' : '수집 대기')+'</small>' : '') + '</div>';
    patchResourceNode(bottom,next,true);
  }
  const footer = root.querySelector('.pa-footer > span:last-child');
  if (footer && host) {
    footer.textContent = (host.hostname || host.name) + (host.observed_at ? ' · 머신 정보 갱신 ' + resTime(host.observed_at).replace(/^\d{4}-\d{2}-\d{2} /,'') : ' · 머신 정보 수집 대기');
    footer.title = 'OS·커널·IP 등 호스트 정보를 읽은 시각입니다. Job 진행 시각이나 빌드 시각이 아닙니다.';
  }
};
catHostLabel = function(host, showStatus=false) {
  if (!host) return '';
  const online = catalogConnection === '최근 조회 성공' && host.status === 'ok';
  return '<span>' + esc(host.hostname || host.name || '머신 미확인')
    + (showStatus ? '<i class="pa-dot pa-host-online '+(online ? 'is-online' : 'is-offline')+'" role="img" aria-label="'+(online ? '연결됨' : '연결 확인 필요')+'"></i>' : '')
    + '</span><small>' + esc(host.ip_kind || 'IP') + ' · ' + esc(host.ip || '수집 대기') + '</small>'
    + (showStatus && loomBuildInfo?.live_source === true ? '<small class="pa-node-environment">DEV BUILD</small>' : '');
};
catHostRefresh();

function loomHistoryMore(total, filter) {
  const limit = state.queueHistoryLimits?.[filter] || 10;
  if (total <= 10) return '';
  return '<div class="pa-history-more">'
    + (limit < total ? '<button class="pa-button" data-history-more="'+esc(filter)+'">더보기 · '+Math.min(limit,total)+' / '+total+'</button>' : '')
    + (limit > 10 ? '<button class="pa-button" data-history-collapse="'+esc(filter)+'">접기</button>' : '') + '</div>';
}
root.addEventListener('click', event => {
  const button = event.target.closest('[data-history-more],[data-history-collapse]');
  if (!button) return;
  const pane = ws.panes.find(p => p.id === button.closest('[data-pane]')?.dataset.pane);
  if (!pane) return;
  const view = activeTab(pane).view;
  const key = button.dataset.historyMore ?? button.dataset.historyCollapse;
  view.queueHistoryLimits = {...view.queueHistoryLimits,
    [key]: button.hasAttribute('data-history-collapse') ? 10 : (view.queueHistoryLimits?.[key] || 10) + 10};
  const panel = button.closest('.pa-panel');
  const previous = state;
  state = view;
  try {
    const rows = loomFilteredWorkItems().filter(item => queueState(item).info[3])
      .sort((a,b)=>(queueTimes(b).ended??queueTimes(b).exited??0)-(queueTimes(a).ended??queueTimes(a).exited??0))
      .filter(item=>key==='all'||queueState(item).info[0]===key);
    const tbody = panel.querySelector('tbody');
    tbody.innerHTML = rows.slice(0,view.queueHistoryLimits[key])
      .map(item=>'<tr>'+workRow(item,false).map(cell=>'<td>'+cell+'</td>').join('')+'</tr>').join('');
    for (const link of tbody.querySelectorAll('button[data-entity]')) {
      const more = document.createElement('button');
      more.className='pa-entity-more';more.dataset.entityOptions='true';more._target=link;
      more.setAttribute('aria-label',link.textContent.trim()+' 열기 옵션');more.textContent='⋯';
      link.insertAdjacentElement('afterend',more);
    }
    panel.querySelector('.pa-history-more').outerHTML = loomHistoryMore(rows.length,key);
    panel.querySelector('[data-history-more],[data-history-collapse]')?.focus({preventScroll:true});
    if(globalThis.lucide)loomCreateIcons({attrs:{width:16,height:16}});
  } finally { state = previous; }
  persist();
});

// Agent orchestration belongs to the caller. Restore old bookmarks to Dashboard.
for(const pane of ws.panes){
  for(const tab of pane.tabs)if(tab.view.page==='agent'){tab.view.page='queue';tab.id=tabKey(tab.view);}
  pane.tabs=pane.tabs.filter((tab,i,all)=>all.findIndex(t=>t.id===tab.id)===i);
  if(!pane.tabs.some(t=>t.id===pane.active))pane.active=pane.tabs[0]?.id;
}

// Access administration uses the same document tabs; transport stays in parent.
let loomAccessState=null,loomAccessError=null,loomAccessBusy=false,loomAccessOpen=false,loomTailnetInfo=null;
const loomAccessLabels={requested:'Preparing invitation',awaiting_approval:'Awaiting approval',revoking:'Revoking',revoked:'Revoked',expired:'Expired',external_share:'Not granted',denied:'Denied',approved:'Granting access',invited:'Awaiting acceptance',ready:'Granted'};
function loomTailnetDate(value,seconds=false){
  if(value==null||value==='')return '—';
  const date=new Date(seconds?Number(value)*1000:value);
  return Number.isFinite(date.getTime())?date.toLocaleString(undefined,{year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit'}):'—';
}
function loomAccessView(){
  const intro=heading('NETWORK','Tailscale',loomTailnetInfo?.machine || '이 머신의 연결 및 접근 정보');
  const user=loomSessionUser;
  const account=panel('Account',user?'<div class="pa-tailnet-account"><span class="pa-tailnet-avatar" aria-hidden="true">'+esc(Array.from((user.name||user.login||'U').trim())[0])+'</span><div class="pa-tailnet-identity"><strong>'+esc(user.name||user.login)+'</strong><span>'+esc(user.login||'')+'</span></div><span class="pa-tailnet-badge">'+(loomTailnetInfo?.development?'Development':user.access_admin?'Admin':'Member')+'</span></div>':'<p class="pa-tailnet-empty">'+(loomTailnetInfo?.development?'Development · 인증 생략':'현재 연결에는 Tailscale 사용자 정보가 없습니다.')+'</p>');
  if(!user?.access_admin){
    const management=loomTailnetInfo?.development?panel('Access management','<p class="pa-tailnet-empty">개발 환경입니다. 멤버와 토큰은 배포판에서 관리합니다.</p>'+(loomTailnetInfo.management_url?'<button class="pa-button" type="button" data-open-management>Open dashboard ↗</button>':'')):'';
    return intro+'<section class="pa-loom-tailnet">'+account+management+'</section>';
  }
  const members=loomAccessState?.members||[];
  const memberPanel=panel('Member <small>'+members.length+'</small>',loomAccessState?.members_error?'<p class="pa-tailnet-empty" role="status">멤버 목록을 불러오지 못했습니다. 새로고침해 주세요.</p>':members.length?'<div class="pa-tailnet-table-wrap"><table class="pa-tailnet-table" aria-label="Member"><thead><tr><th>Account</th><th>Role</th><th>Shared</th><th>Actions</th></tr></thead><tbody>'+members.map(m=>'<tr data-tailnet-member="'+esc(m.id)+'"><td data-label="Account"><div class="pa-tailnet-identity"><strong>'+esc(m.displayName||m.loginName||'—')+'</strong><span>'+esc(m.loginName||'—')+'</span></div></td><td data-label="Role"><span class="pa-tailnet-badge">'+(['admin','owner'].includes(String(m.role).toLowerCase())?'Admin':'Member')+'</span></td><td data-label="Shared">'+esc(loomTailnetDate(m.accepted_at||m.created,!!m.accepted_at))+'</td><td data-label="Actions">'+(m.role==='Member'?'<button class="pa-button" type="button" data-share-revoke="'+esc(m.id)+'" '+(loomAccessBusy?'disabled':'')+'>Revoke</button>':'')+'</td></tr>').join('')+'</tbody></table></div>':'<p class="pa-tailnet-empty">'+(loomAccessState?'등록된 멤버가 없습니다.':'불러오는 중…')+'</p>','<button type="button" class="pa-button" data-access-refresh '+(loomAccessBusy?'disabled':'')+'>Refresh</button>');
  const rows=loomAccessState?.requests || [];
  const content=rows.map(r=>{
    const actions=['denied','revoked'].includes(r.state)?[['remove','Remove']]:['pending','awaiting_approval'].includes(r.state)?[['approve','Approve'],['deny','Deny']]:r.state==='ready'?[['revoke','Revoke']]:['requested','invited','approved'].includes(r.state)?[['deny','Deny'],...(r.error?[['retry','Retry']]:[])]:r.error?[['retry','Retry']]:[];
    return '<tr><td data-label="Email">'+esc(r.email)+(r.accepted_email?'<small class="pa-subtle">Accepted: '+esc(r.accepted_email)+'</small>':'')+'</td><td data-label="Grant status"><span class="pa-tailnet-badge" data-status="'+esc(r.state)+'">'+esc(r.user_cancelled?(r.state==='revoking'?'Cancelling':'Cancelled by user'):(loomAccessLabels[r.state]||r.state))+'</span>'+(r.error?'<p class="pa-tailnet-error">'+esc(r.error)+'</p>':'')+'</td><td data-label="Requested">'+esc(loomTailnetDate(r.created,true))+'</td><td data-label="Actions"><div class="pa-tailnet-actions">'+actions.map(([action,label])=>'<button type="button" class="pa-button" data-access-id="'+esc(r.id)+'" data-access-action="'+action+'" '+(loomAccessBusy?'disabled':'')+'>'+label+'</button>').join('')+'</div></td></tr>';
  }).join('');
  const requests=panel('Access Request <small>'+rows.length+'</small>',rows.length?'<div class="pa-tailnet-table-wrap"><table class="pa-tailnet-table" aria-label="Access Request"><thead><tr><th>Email</th><th>Grant status</th><th>Requested</th><th>Actions</th></tr></thead><tbody>'+content+'</tbody></table></div>':'<p class="pa-tailnet-empty">'+(loomAccessState?'아직 신청 내역이 없습니다.':'불러오는 중…')+'</p>');
  const sshRows=loomAccessState?.ssh?.accounts||[];
  const sshPanel=panel('SSH',loomAccessState?.ssh_error?'<p role="status">SSH 상태를 확인하지 못했습니다.</p>':!loomAccessState?.ssh?.configured?'<p class="pa-tailnet-empty">SSH management is not configured on this machine.</p>':'<div class="pa-tailnet-table-wrap"><table class="pa-tailnet-table" aria-label="SSH accounts"><thead><tr><th>Account</th><th>SSH account</th><th>Status</th><th>Actions</th></tr></thead><tbody>'+members.map(m=>{
    const login=String(m.loginName||'').toLowerCase(),r=sshRows.find(r=>r.login===login);
    const suggested='loomu_'+login.split('@')[0].replace(/[^a-z0-9_]/g,'_').slice(0,24);
    return '<tr data-ssh-login="'+esc(login)+'"><td>'+esc(m.displayName||login)+'<small class="pa-subtle">'+esc(login)+'</small></td><td><input class="pa-input" aria-label="SSH account for '+esc(login)+'" data-ssh-username value="'+esc(r?.username||suggested)+'" pattern="loomu_[a-z0-9][a-z0-9_]{0,23}" maxlength="30" '+(r?'readonly':'')+'></td><td>'+esc(r?.state||'Not enabled')+(r?.error?'<small class="pa-tailnet-error">'+esc(r.error)+'</small>':'')+'</td><td><button type="button" class="pa-button" data-ssh-action="'+(r?.state==='enabled'?'disable':'enable')+'" '+(loomAccessBusy?'disabled':'')+'>'+(r?.state==='enabled'?'Disable SSH':'Enable SSH')+'</button>'+(r&&r.state!=='enabled'&&r.state!=='disabled'?'<button type="button" class="pa-button" data-ssh-action="disable">Disable SSH</button>':'')+'</td></tr>';
  }).join('')+'</tbody></table></div>');
  const tokens=loomAccessState?.tokens||[],owners=loomAccessState?.owners||[];
  const active=tokens.filter(t=>!t.revoked),revoked=tokens.filter(t=>t.revoked);
  const permissions=t=>t.scopes.length===4&&['read','submit','cancel','events'].every(s=>t.scopes.includes(s))?'Operator':t.scopes.includes('read')&&t.scopes.every(s=>['read','events'].includes(s))?'Read only':t.scopes.join(', ');
  const tokenTable=(items,label)=>'<div class="pa-tailnet-table-wrap"><table class="pa-tailnet-table" aria-label="'+label+'"><thead><tr><th>Name</th><th>Owner</th><th>Permissions</th><th>Status</th><th></th></tr></thead><tbody>'+items.map(t=>'<tr><td data-label="Name"><strong>'+esc(t.label)+'</strong></td><td data-label="Owner">'+esc(owners.find(o=>o.id===t.owner)?.login||t.owner)+'</td><td data-label="Permissions"><span class="pa-tailnet-badge">'+esc(permissions(t))+'</span></td><td data-label="Status">'+(t.revoked?'Revoked':'<span class="pa-token-active">Active</span>')+'</td><td data-label="Actions">'+(!t.revoked?'<button class="pa-button" type="button" data-token-revoke="'+esc(t.id)+'">Revoke</button>':'—')+'</td></tr>').join('')+'</tbody></table></div>';
  const tokenPanel=panel('Agent Tokens <small>'+active.length+'</small>',
    '<form data-token-form class="pa-token-form"><label><span>Name</span><input aria-label="Token name" name="label" placeholder="e.g. Research agent" maxlength="80" required></label><label><span>Owner</span><select aria-label="Token owner" name="owner" required>'+owners.map(o=>'<option value="'+esc(o.id)+'">'+esc(o.display_name||o.login)+'</option>').join('')+'</select></label><label><span>Permissions</span><select aria-label="Token permissions" name="permissions"><option value="operator">Operator</option><option value="read">Read only</option></select></label><button class="pa-button pa-token-create" type="button" data-token-create '+(loomAccessBusy||!owners.length?'disabled':'')+'>Create token</button></form><p class="pa-token-caption">No expiration · Revoke anytime</p>'+
    (loomAccessState?.tokens_error?'<p role="status">토큰 목록을 불러오지 못했습니다.</p>':(active.length?tokenTable(active,'Agent Tokens'):'<div class="pa-token-empty">No active tokens</div>')+(revoked.length?'<details class="pa-token-history"><summary>Revoked <span>'+revoked.length+'</span></summary>'+tokenTable(revoked,'Revoked tokens')+'</details>':'')));
  return intro+'<section class="pa-loom-tailnet">'+account+'<div class="pa-loom-access" aria-label="머신 접근 관리"><p class="pa-tailnet-feedback" role="status">'+esc(loomAccessError||(loomAccessBusy?'처리 중…':''))+'</p>'+memberPanel+sshPanel+requests+tokenPanel+'</div></section>';
}
function loomAccessPatch(){
  for(const current of root.querySelectorAll('.pa-loom-tailnet')){
    const next=document.createElement('template');next.innerHTML=loomAccessView();
    const panel=next.content.querySelector('.pa-loom-tailnet');
    if(panel)patchResourceNode(current,panel,true);
  }
}
const renderBeforeAccess=render;
render=function(){
  renderBeforeAccess();
  const open=!!root.querySelector('.pa-loom-access');
  if(open&&!loomAccessOpen)parent.postMessage({type:'loom-access-request'},'*');
  loomAccessOpen=open;
};
root.addEventListener('click',event=>{
  if(event.target.closest('[data-open-management]')){parent.postMessage({type:'loom-open-management'},'*');return;}
  const tokenRevoke=event.target.closest('[data-token-revoke]');
  if(tokenRevoke&&loomSessionUser?.access_admin&&!loomAccessBusy){
    loomAccessBusy=true;loomAccessPatch();parent.postMessage({type:'loom-token-revoke',id:tokenRevoke.dataset.tokenRevoke,label:loomAccessState?.tokens?.find(t=>t.id===tokenRevoke.dataset.tokenRevoke)?.label||''},'*');return;
  }
  const sshButton=event.target.closest('[data-ssh-action]');
  if(sshButton&&loomSessionUser?.access_admin&&!loomAccessBusy){
    const row=sshButton.closest('[data-ssh-login]'),input=row.querySelector('[data-ssh-username]');
    if(!input.reportValidity())return;
    const body={login:row.dataset.sshLogin,username:input.value,action:sshButton.dataset.sshAction};
    loomAccessBusy=true;loomAccessPatch();parent.postMessage({type:'loom-ssh-change',body},'*');return;
  }
  const revoke=event.target.closest('[data-share-revoke]');
  const action=event.target.closest('[data-access-action]');
  if(!loomSessionUser?.access_admin || loomAccessBusy)return;
  if(action || revoke || event.target.closest('[data-access-refresh]')){
    loomAccessBusy=true;loomAccessError=null;loomAccessPatch();
    parent.postMessage(revoke?{type:'loom-share-revoke',id:revoke.dataset.shareRevoke}:action?{type:'loom-access-decision',id:action.dataset.accessId,action:action.dataset.accessAction}:{type:'loom-access-request'},'*');
  }
});
window.addEventListener('message',event=>{
  if(event.source!==parent || event.data?.type!=='loom-access-response' || !loomSessionUser?.access_admin)return;
  loomAccessBusy=false;loomAccessError=event.data.error||null;
  if(event.data.data)loomAccessState=event.data.data;
  loomAccessPatch();
});
setInterval(()=>{if(!document.hidden&&loomSessionUser?.access_admin&&root.querySelector('.pa-loom-access')&&!loomAccessBusy)parent.postMessage({type:'loom-access-request'},'*');},15000);

// Only display identity crosses into the workspace; cookies/CSRF stay in parent.
let loomSessionUser = null;
function loomSessionControl() {
  const sidebar = root.querySelector('.pa-sidebar');
  if (!sidebar) return;
  const existing = root.querySelector('[data-loom-session]');
  const identity = JSON.stringify(loomSessionUser);
  if (existing?.dataset.identity === identity) return;
  existing?.remove();
  if (!loomSessionUser) return;
  const account=document.createElement('div');account.dataset.loomSession='true';account.dataset.identity=identity;
  const trigger=document.createElement('button');trigger.type='button';trigger.className='pa-profile-trigger';
  trigger.setAttribute('aria-label','내 계정 · Tailscale 열기');
  const avatar=document.createElement('span');avatar.className='pa-profile-avatar';avatar.setAttribute('aria-hidden','true');
  avatar.textContent=Array.from((loomSessionUser.name || loomSessionUser.login || 'U').trim())[0].toUpperCase();
  const name=document.createElement('span');name.dataset.loomSessionName='true';
  name.textContent=loomSessionUser.name || loomSessionUser.login || 'Signed in';name.title=name.textContent;
  trigger.append(avatar,name);account.append(trigger);sidebar.append(account);
  trigger.onclick=()=>openView({page:'access'});
}
window.addEventListener('message',event=>{
  if(event.source!==parent || event.data?.type!=='loom-session')return;
  const changed=JSON.stringify(loomSessionUser)!==JSON.stringify(event.data.user||null);
  loomSessionUser=event.data.user || null;loomTailnetInfo=event.data.tailnet||null;
  if(changed){
    loomAccessState=null;loomAccessError=null;loomAccessBusy=false;loomAccessOpen=false;
    if(root.querySelector('.pa-loom-tailnet'))render();
  }
  loomSessionControl();
});
const renderBeforeSession = render;
render = function(){renderBeforeSession();loomSessionControl();};
window.addEventListener('resize',()=>{
  const menu=root.querySelector('#loom-profile-menu');
  if(menu?.matches(':popover-open'))menu.hidePopover();
});

window.addEventListener('message',event=>{
  if(event.source!==parent || event.data?.type!=='loom-theme' || !['light','dark','system'].includes(event.data.mode))return;
  window.loomThemePreference=event.data.mode;ws.theme=event.data.mode;applyTheme();
});

// A top-layer tooltip stays visible outside the clipped narrow sidebar.
(() => {
  const tip=document.createElement('div');
  tip.id='loom-nav-tooltip';tip.className='pa-nav-tooltip';
  tip.setAttribute('role','tooltip');tip.setAttribute('popover','manual');root.append(tip);
  let active=null;
  function hide(){
    if(tip.matches(':popover-open'))tip.hidePopover();
    if(active)active.removeAttribute('aria-describedby');active=null;
  }
  function show(button){
    if(!matchMedia('(max-width:760px)').matches || !button)return;
    hide();active=button;
    const label=button.textContent.trim();
    button.setAttribute('aria-label',label);button.setAttribute('aria-describedby',tip.id);
    tip.textContent=label;tip.showPopover();
    const r=button.getBoundingClientRect();
    tip.style.left=Math.max(8,Math.min(r.right+10,innerWidth-tip.offsetWidth-8))+'px';
    tip.style.top=Math.max(8,Math.min(r.top+(r.height-tip.offsetHeight)/2,innerHeight-tip.offsetHeight-8))+'px';
  }
  const nav=root.querySelector('#pa-nav');
  nav.addEventListener('pointerover',e=>{if(e.pointerType!=='touch')show(e.target.closest('.pa-nav-item'));});
  nav.addEventListener('pointerout',e=>{if(active && !active.contains(e.relatedTarget))hide();});
  nav.addEventListener('focusin',e=>show(e.target.closest('.pa-nav-item')));
  nav.addEventListener('focusout',hide);
  nav.addEventListener('pointerdown',hide);
  window.addEventListener('resize',hide);
  document.addEventListener('keydown',e=>{if(e.key==='Escape')hide();});
  const previousRender=render;render=function(){hide();return previousRender();};
})();

function loomCreateAgentToken(form){
  if(!form||!loomSessionUser?.access_admin||loomAccessBusy||!form.reportValidity())return;
  const f=new FormData(form);const body={label:f.get('label'),owner:f.get('owner'),scopes:f.get('permissions')==='read'?['read']:['read','submit','cancel','events']};
  loomAccessBusy=true;loomAccessPatch();parent.postMessage({type:'loom-token-issue',body},'*');
}
root.addEventListener('click',event=>{
  const trigger=event.target.closest('[data-token-create]');
  if(trigger){event.preventDefault();loomCreateAgentToken(trigger.closest('[data-token-form]'));}
});
root.addEventListener('keydown',event=>{
  if(event.key==='Enter'&&event.target.matches('[data-token-form] input')){
    event.preventDefault();loomCreateAgentToken(event.target.closest('[data-token-form]'));
  }
});
