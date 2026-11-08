const loomPayloadLabels = {
  available: '이관됨',
  intentionally_not_migrated: '의도적으로 미이전',
  unverified_or_partial: '미검증 또는 일부만 이관',
  missing: '파일 없음',
  corrupt: '파일 무결성 오류',
};
const loomLocationPages = new Map();
const loomEntityReads = new Map();
let loomDetailSequence = 0;

function loomText(value) {
  return esc(value === true ? '예' : value === false ? '아니요' : value ?? '미기록');
}

function loomReadNotice(entry) {
  if (entry?.error === 403) return '조회 권한이 없습니다.';
  if (entry?.error === 404) return '등록된 기록을 찾을 수 없습니다.';
  if (entry?.error) return '기록을 조회할 수 없습니다. 다시 시도해 주세요.';
  return '기록을 조회하는 중입니다.';
}

function loomHistoryPanel(entity, entry) {
  if (!entry?.data) {
    return panel('이관 위치', '<p>' + esc(loomReadNotice(entry)) + '</p>'
      + (entry?.error ? '<button class="pa-button" data-loom-detail-retry>다시 조회</button>' : ''));
  }
  const data = entry.data;
  const rows = data.body?.locations || [];
  const byStatus = new Map();
  for (const row of rows) byStatus.set(row.status, (byStatus.get(row.status) || 0) + 1);
  const status = data.body?.payload_status;
  const summary = status ? '<p data-loom-payload-status="' + esc(status) + '">'
    + esc(loomPayloadLabels[status] || status) + ' · ' + esc(data.snapshot) + '</p>' : '';
  const counts = [...byStatus].map(([key,n]) => esc(loomPayloadLabels[key] || key) + ' ' + n + '개').join(' · ');
  const current = loomLocationPages.get(entity.id) || 0;
  const page = rows.slice(current, current + 50);
  const pageControl = rows.length > 50 ? '<div class="pa-filter">'
    + '<button class="pa-button" data-loom-location-page="' + (current - 50) + '"' + (current === 0 ? ' disabled' : '') + '>이전</button>'
    + '<small>' + (current + 1) + '–' + (current + page.length) + ' / ' + rows.length + '</small>'
    + '<button class="pa-button" data-loom-location-page="' + (current + 50) + '"' + (current + page.length >= rows.length ? ' disabled' : '') + '>다음</button></div>' : '';
  return panel('이관 위치', '<div class="pa-loom-locations">' + summary + '<p>' + counts + '</p>'
    + (page.length ? catTable(['원본 위치','현재 위치','상태','Bytes'], page.map(row => [
      loomText(row.source_ref), loomText(row.target_ref), loomText(loomPayloadLabels[row.status] || row.status), loomText(row.size)
    ])) + pageControl : '<p>등록된 payload 위치가 없습니다.</p>') + '</div>');
}

function loomJobPanel(job, entry) {
  if (!entry?.data) {
    return panel('Loom 실행 기록', '<p>' + esc(loomReadNotice(entry)) + '</p>'
      + (entry?.error ? '<button class="pa-button" data-loom-detail-retry>다시 조회</button>' : ''));
  }
  const record = entry.data;
  const result = record.result || {};
  const execution = result.execution || {};
  const current = (record.attempts || []).find(attempt => attempt.id === record.attempt);
  const index = {complete:'indexed', processing:'pending', result_failed:'incomplete'}[record.state];
  const rows = [
    ['Job',record.id],['Owner',record.owner_profile?.display_name || record.owner],['Status',record.status || record.state],['Attempt',record.attempt],['Attempt 상태',current?.state],
    ['Exit code',execution.exit_code],['프로세스 그룹 반환',execution.process_group_returned],
    ['Artifact 검증',result.artifact_ready],['Typed outputs',Array.isArray(result.entities) ? result.entities.length : null],
    ['Scheduling',record.scheduling?.reason],['Recovery',record.recovery?.state],['Recovery Agent',record.recovery?.agent],
    ['Recovery Attempts',record.recovery ? record.recovery.rounds + ' / ' + record.recovery.max_rounds : null],
    ['Reservation Deadline',record.recovery?.deadline ? new Date(record.recovery.deadline*1000).toLocaleString() : null],
    ['Registry 색인',index],['Result 재시도',record.result_processing?.retries],
  ];
  const attempts = (record.attempts || []).map(attempt => [loomText(attempt.id), loomText(attempt.state),
    queueTime(queueMillis(attempt.started)), queueTime(queueMillis(attempt.ended))]);
  const linked = catalogData.entities.filter(entity => entity.loom_job_id === job.id)
    .map(entity => '<p>' + catLink(entity.id) + '</p>').join('');
  return panel('Loom 실행 기록', '<div class="pa-loom-job-summary">' + catTable(['필드','값'], rows.map(row => row.map(loomText))) + '</div>')
    + panel('Attempts', catTable(['Attempt','상태','시작','종료'], attempts))
    + (linked ? panel('Registry 기록', '<div class="pa-loom-job-links">' + linked + '</div>') : '');
}

function loomRegistryPanel(entry) {
  if (!entry?.data) {
    return panel('Loom Registry', '<p>' + esc(loomReadNotice(entry)) + '</p>'
      + (entry?.error ? '<button class="pa-button" data-loom-detail-retry>다시 조회</button>' : ''));
  }
  const item = entry.data;
  const manifest = (item.components || []).map(row => [row.ref, row.size, row.sha256].map(loomText));
  const locations = (item.locations || []).map(row => [
    row.component, row.machine_id, row.storage_id, row.ref, loomPayloadLabels[row.status] || row.status
  ].map(loomText));
  const lineage = (item.relations || []).map(row => [
    loomText(row.kind), loomText(row.input), catLink(row.id), loomText(row.revision)
  ]);
  return panel('Loom Registry', catTable(['기록','Revision','Job','Attempt'], [[
    loomText(item.id), loomText(item.revision), catLink(item.job), loomText(item.attempt)
  ]]))
    + panel('Lineage', catTable(['관계','입력','기록','Revision'], lineage))
    + panel('Payload 위치', catTable(['Component','Machine','Storage','Ref','상태'], locations))
    + panel('Manifest', catTable(['Reference','Bytes','SHA-256'], manifest));
}

function loomVerificationPanel(entry) {
  const verification = entry?.data?.body?.verification;
  if (!Array.isArray(verification) || !verification.length) return '';
  return panel('원본 verification', catTable(['수준','정책 revision','사유','기록 시각'],
    verification.map(row => [loomText(row.level),loomText(row.revision),loomText(row.reason),
      queueTime(queueMillis(row.created))])));
}

function loomJobContract() {
  return catalogData.job_contract || {categories:{other:'Other'},execution_modes:{}};
}
function loomJobClass(entity) {
  const p = entity.properties || {}, c = loomJobContract();
  return {category:c.categories[p.category] || 'Other', mode:c.execution_modes[p.execution_mode] || '미기록'};
}
function loomFilteredWorkItems() {
  return workItems().filter(e => (!state.jobCategory || (e.properties?.category || 'other') === state.jobCategory)
    && (!state.jobExecutionMode || e.properties?.execution_mode === state.jobExecutionMode));
}
function loomJobFilters() {
  const c = loomJobContract();
  return '<div class="pa-job-filters">' + [['jobCategory','Job Type',c.categories],['jobExecutionMode','Execution Mode',c.execution_modes]]
    .map(([key,label,choices]) => '<label>'+label+' <select data-job-filter="'+key+'" aria-label="'+label+'"><option value="">전체</option>'
      + Object.entries(choices).map(([value,name]) => '<option value="'+esc(value)+'"'+(state[key]===value?' selected':'')+'>'+esc(name)+'</option>').join('')+'</select></label>').join('')+'</div>';
}
root.addEventListener('change', event => {
  const field = event.target.closest('[data-job-filter]');
  if (!field) return;
  const pane = ws.panes.find(p => p.id === field.closest('[data-pane]')?.dataset.pane);
  if (!pane) return;
  const view = activeTab(pane).view;
  view[field.dataset.jobFilter] = field.value;
  view.queueHistoryLimits = {};
  render();persist();
});

// The existing queue and terminal-history views consume both record families.
workItems = function() {
  return catKind('job').filter(entity => entity.properties?.record_role === 'work'
    || (entity.properties?.record_role === 'loom_job' && !entity.loom_registry));
};

const loomLegacyWorkRow = workRow;
workRow = function(entity, current) {
  const row = loomLegacyWorkRow(entity, current);
  if (entity.properties?.record_role === 'loom_job') {
    row[0] = '<div class="pa-queue-name">' + catLink(entity.id) + '<small>'
      + (entity.attempts || []).length + ' Attempts</small></div>';
  }
  const kind = loomJobClass(entity);
  row[0] += '<small class="pa-job-classification">'+esc(kind.category)+(kind.mode==='미기록'?'':' ('+esc(kind.mode)+')')+'</small>';
  return row;
};

function loomDescriptor(entity) {
  if (entity?.loom_registry) return {kind:'registry', identity:entity.id, key:entity.loom_registry};
  if (entity?.loom_history) return {kind:'history', identity:entity.id, key:entity.loom_history};
  if (entity?.loom_job) return {kind:'job', identity:entity.id, key:{id:entity.loom_job.id}};
  return null;
}

function loomRequestDetail(descriptor) {
  const key = JSON.stringify(descriptor.key);
  if (loomEntityReads.get(descriptor.identity)?.key === key) return;
  if (loomEntityReads.size >= 12) {
    const first = loomEntityReads.keys().next().value;
    loomEntityReads.delete(first);
    loomLocationPages.delete(first);
  }
  const requestId = ++loomDetailSequence;
  loomEntityReads.set(descriptor.identity, {key, requestId, kind:descriptor.kind});
  parent.postMessage({type:'loom-detail-request', kind:descriptor.kind, identity:descriptor.identity, requestId}, '*');
}

const loomCatalogDetail = catalogDetail;
// Keep the existing share transport and route handling behind one compact menu.
paiEntityActions = function(id) {
  return `<details class="pa-entity-menu"><summary aria-label="항목 메뉴" title="항목 메뉴"><svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true"><circle cx="4" cy="10" r="1.5"/><circle cx="10" cy="10" r="1.5"/><circle cx="16" cy="10" r="1.5"/></svg></summary><div class="pa-entity-menu-items"><button data-share-route="${esc(id)}" aria-haspopup="dialog">${icon('share')}<span>공유</span></button><a href="${esc(paiEntityURL(id))}" target="_blank" rel="noopener noreferrer" data-open-route="${esc(id)}"><span aria-hidden="true">↗</span><span>새 창에서 열기</span></a></div></details>`;
};

function loomDetailHeading(html, id) {
  const template = document.createElement('template');
  template.innerHTML = html;
  // Older Job pages prepend an unstyled actions row above the heading.
  for (const action of template.content.querySelectorAll('[data-share-route],[data-open-route]')) {
    if (action.closest('.pa-entity-menu')) continue;
    const row = action.closest('.pa-catalog-status,.pa-entity-actions');
    action.remove();
    if (row && !row.textContent.trim()) row.remove();
  }
  for (const child of [...template.content.children]) {
    if (child.matches('.pa-job-status')) child.remove();
  }
  const title = template.content.querySelector('.pa-heading');
  if (title) {
    title.classList.add('pa-entity-heading');
    title.querySelector('.pa-job-header-meta')?.remove();
    if (!title.querySelector('.pa-entity-menu')) title.insertAdjacentHTML('beforeend', paiEntityActions(id));
    const h1 = title.querySelector('h1');
    const menu = title.querySelector('.pa-entity-menu');
    if (h1 && menu) {
      let row = h1.closest('.pa-title-line');
      if (!row) { row = document.createElement('div'); h1.before(row); row.append(h1); }
      row.classList.add('pa-entity-title-row');
      row.append(menu);
    }
    template.content.prepend(title);
    const entity = catEntity(id);
    const owner = entity?.owner_profile || {};
    const ownerRow = '<div><dt>Owner</dt><dd title="'+esc(owner.login || '')+'">'+esc(owner.display_name || entity?.properties?.owner_name || entity?.owner || 'Unassigned')+'</dd></div>';
    if (entity?.kind === 'job') {
      const properties = document.createElement('div');
      properties.innerHTML = panel('Job Properties', '<dl class="pa-job-properties">'
        + ownerRow + '<div><dt>Status</dt><dd>' + queueBadge(entity) + '</dd></div>'
        + '<div><dt>Task level</dt><dd>' + workVerificationCell(entity) + '</dd></div>'
        + (entity?.properties?.output_storage ? '<div><dt>Output Storage</dt><dd>'+esc(entity.properties.output_storage)+'</dd></div>' : '')
        + '<div><dt>Job Type</dt><dd>' + esc(loomJobClass(entity).category) + '</dd></div>'
        + '<div><dt title="Full: main run; Pilot: reduced trial; Smoke: startup check">Execution Mode</dt><dd>' + esc(loomJobClass(entity).mode) + '</dd></div></dl>');
      title.after(...properties.children);
    } else if (entity) {
      const properties = document.createElement('div');
      properties.innerHTML = panel('Properties', '<dl class="pa-job-properties">'+ownerRow+'</dl>');
      title.after(...properties.children);
    }
  }
  return template.innerHTML;
}

catalogDetail = function(id) {
  const entity = catEntity(id);
  const descriptor = loomDescriptor(entity);
  // Live Jobs have their own attempts endpoint; legacy history's empty cards
  // incorrectly suggested that these completed Jobs had no execution record.
  const liveJob = descriptor?.kind === 'job' || !!entity?.loom_imported_job;
  const base = loomDetailHeading(liveJob
    ? heading('JOB', esc(entity.label), '', paiEntityActions(id))
      + panel('실행', catTable(['등록','실행 시작','실행 종료','실행 시간'], [queueRow(entity).slice(2)]))
    : loomCatalogDetail(id), id);
  if (!descriptor) return base;
  loomRequestDetail(descriptor);
  const entry = loomEntityReads.get(id);
  const detail = entity?.loom_imported_job ? loomJobPanel({id:entity.id}, {...entry, data:entry?.data?.body?.record})
    : descriptor.kind === 'history' ? (entity.kind === 'job' ? '' : loomHistoryPanel(entity, entry))
    : descriptor.kind === 'registry' ? loomRegistryPanel(entry)
    : loomJobPanel(descriptor.key, entry);
  return base + loomVerificationPanel(entry) + detail;
};

document.addEventListener('click', event => {
  for (const menu of root.querySelectorAll('.pa-entity-menu[open]')) {
    if (!menu.contains(event.target) || event.target.closest('[data-share-route],[data-open-route]')) menu.open = false;
  }
}, true);
root.addEventListener('keydown', event => {
  if (event.key !== 'Escape') return;
  const menu = event.target.closest('.pa-entity-menu[open]');
  if (menu) { menu.open = false; menu.querySelector('summary').focus(); event.stopPropagation(); }
});

root.addEventListener('click', event => {
  const trigger = event.target.closest('[data-loom-detail-retry],[data-loom-location-page]');
  if (!trigger) return;
  const pane = ws.panes.find(item => item.id === trigger.closest('[data-pane]')?.dataset.pane);
  const id = pane && activeTab(pane).view.entity;
  if (!id) return;
  if (trigger.hasAttribute('data-loom-detail-retry')) loomEntityReads.delete(id);
  else loomLocationPages.set(id, Number(trigger.dataset.loomLocationPage));
  render();
});

window.addEventListener('message', event => {
  if (event.source !== parent) return;
  const message = event.data;
  if (message?.type === 'pai-catalog-data') {
    for (const [identity, entry] of loomEntityReads) {
      if (entry.kind === 'job') loomEntityReads.delete(identity);
    }
    return;
  }
  if (message?.type !== 'loom-detail-response') return;
  const entry = loomEntityReads.get(message.identity);
  if (entry?.requestId !== message.requestId) return;
  loomEntityReads.set(message.identity, {...entry, data:message.data, error:message.error});
  scheduleCatalogRender();
});

// Canonical phase labels; compatibility state values remain readable in old records.
const loomPriorQueueState=queueState;
queueState=function(entity){
  const base=loomPriorQueueState(entity), p=entity.properties||{}, r=p.recovery;
  if(r && ['waiting','claimed','pending','executing'].includes(r.state)) return {raw:base.raw,info:['recovery',r.state==='claimed'?'Recovering':'Recovery · '+r.state,'!',false]};
  if(r && ['deferred','exhausted'].includes(r.state)) return {raw:base.raw,info:['failed',r.state==='deferred'?'Deferred':'Recovery exhausted','!',true]};
  const labels={queued:'Queued',blocked:'Waiting for inputs',preparing:'Preparing',ready:'Ready',running:'Running',processing:'Finalizing',complete:'Completed',result_failed:'Finalizing failed'};
  return {raw:base.raw,info:[base.info[0],labels[base.raw]||base.info[1],base.info[2],base.info[3]]};
};
