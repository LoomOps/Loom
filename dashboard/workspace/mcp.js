// Reference browser. It never invokes the operations shown in its examples.
let loomDocGroup = 'jobs';
let loomDocAction = 'list';
function loomDocJson(value) { return JSON.stringify(value, null, 2); }
function loomDocCode(label, value) {
  return '<div class="pa-doc-code"><div><b>'+esc(label)+'</b><button type="button" class="pa-button" data-doc-copy>복사</button></div><pre tabindex="0"><code>'+esc(value)+'</code></pre></div>';
}
function loomDocFields(schema) {
  const required=schema.required || [], fields=Object.entries(schema.properties || {}).sort(([a],[b])=>Number(required.includes(b))-Number(required.includes(a)));
  if (!fields.length) return '<p class="pa-subtle">입력 필드가 없습니다. <code>{}</code>를 사용합니다.</p>';
  return '<div class="pa-table-wrap"><table class="pa-doc-fields"><thead><tr><th>Field</th><th>Type / Required</th><th>Description / Default</th></tr></thead><tbody>' + fields.map(([key,field])=>{
    const type=Array.isArray(field.type)?field.type.join(' | '):field.type || 'any';
    const values=field.enum ? '<div class="pa-doc-values">'+esc(field.enum.map(v=>JSON.stringify(v)).join(' · '))+'</div>' : '';
    const defaults=Object.hasOwn(field,'default')?'<div>Default: <code>'+esc(JSON.stringify(field.default))+'</code></div>':'';
    return '<tr><td><code>'+esc(key)+'</code></td><td>'+esc(type)+'<small>'+(required.includes(key)?'Required':'Optional')+'</small></td><td>'+esc(field.description || '')+defaults+values+'</td></tr>';
  }).join('')+'</tbody></table></div>';
}
function loomMcpView() {
  const mcp=loomMcpState, docs=mcp?.docs;
  let body='<p role="status">MCP 사용 명세를 읽는 중입니다.</p>';
  if (loomMcpError) body='<p role="alert">'+(loomMcpError===403?'MCP 조회 권한이 없습니다.':'MCP 응답을 받을 수 없습니다.')+'</p>';
  else if (mcp) {
    body='<div class="pa-doc-status"><span><b>'+esc(mcp.serverInfo.title || mcp.serverInfo.name)+'</b> · '+esc(mcp.serverInfo.version)+'</span><span>연결 정상 · '+esc(docs?.revision || mcp.protocolVersion)+'</span></div>';
    if (!docs?.groups?.length) body+='<p role="alert">사용 명세를 가져오지 못했습니다. 서버의 문서 API와 권한을 확인해 주세요.</p>';
    else {
      const groups=docs.groups, group=groups.find(g=>g.name===loomDocGroup) || groups[0];
      const action=group.actions.find(a=>a.action===loomDocAction) || group.actions[0];
      const args=action.example || {}, request={jsonrpc:'2.0',id:1,method:'tools/call',params:{name:group.name,arguments:{action:action.action,arguments:args}}};
      const http={group:group.name,action:action.action,arguments:args};
      const curl="curl --request POST \"${LOOM_URL}/api/actions\" \\\n  --header \"Authorization: Bearer ${LOOM_TOKEN}\" \\\n  --header 'Content-Type: application/json' \\\n  --data-binary @- <<'JSON'\n"+loomDocJson(http)+'\nJSON';
      body+='<p class="pa-subtle">현재 권한으로 조회 가능한 '+groups.length+'개 그룹 · '+groups.reduce((n,g)=>n+g.actions.length,0)+'개 Action. 실제 입력 계약에서 생성한 명세입니다.</p>';
      body+='<div class="pa-doc-groups" aria-label="MCP 그룹">'+groups.map(g=>'<button type="button" data-doc-group="'+esc(g.name)+'" aria-pressed="'+(g.name===group.name)+'">'+esc(g.name)+'<small>'+g.actions.length+'</small></button>').join('')+'</div>';
      body+='<div class="pa-doc-layout"><nav class="pa-doc-actions" aria-label="Action 목록"><p>'+esc(group.description)+'</p>'+group.actions.map(a=>'<button type="button" data-doc-action="'+esc(a.action)+'" data-doc-parent="'+esc(group.name)+'" aria-current="'+(a.action===action.action?'true':'false')+'"><code>'+esc(a.action)+'</code><small>'+esc(a.title)+'</small></button>').join('')+'</nav>';
      body+='<article class="pa-doc-detail"><div class="pa-doc-title"><h2>'+esc(group.name+'.'+action.action)+'</h2><span class="pa-doc-scope">Scope · '+esc(action.scope)+'</span></div><p>'+esc(action.description)+'</p><p class="pa-subtle">'+esc(action.access)+'</p>';
      body+='<h3>Input</h3>'+loomDocFields(action.schema)+'<details class="pa-doc-schema"><summary>전체 JSON Schema · 중첩 필드 포함</summary>'+loomDocCode('Input schema',loomDocJson(action.schema))+'</details>';
      body+='<h3>Response</h3><p>'+esc(action.returns)+'</p><p class="pa-subtle">'+esc(docs.transport.result)+'</p>';
      body+='<h3>Errors</h3><p class="pa-subtle">'+esc(docs.transport.errors)+'</p>'+(action.errors.length?'<ul>'+action.errors.map(e=>'<li>'+esc(e)+'</li>').join('')+'</ul>':'<p>인증·권한·입력 형식 오류를 확인합니다.</p>');
      body+='<h3>Examples</h3><p class="pa-subtle">&lt;…&gt; 값을 실제 ID로 바꾸세요. params는 선택한 profile에 맞춰 작성합니다. 예제는 자동 실행되지 않으며, 현재 토큰은 포함하지 않습니다.</p>';
      body+=loomDocCode('MCP · POST /mcp',loomDocJson(request))+loomDocCode('HTTP · POST /api/actions',curl);
      body+='<p class="pa-subtle">LOOM_URL은 Ops API가 연결된 서비스 주소, LOOM_TOKEN은 별도로 설정한 토큰입니다. 토큰 없는 개발 환경에서는 Authorization 헤더를 생략합니다.</p></article></div>';
      body+='<details class="pa-doc-workflow"><summary>사용 흐름 · 제출 → 상태 → 복구 → 결과</summary><ol>'+docs.workflow.map(step=>'<li><b>'+esc(step.step)+'</b> · <code>'+esc(step.group+'.'+step.action)+'</code><p>'+esc(step.note)+'</p></li>').join('')+'</ol></details>';
    }
  }
  return heading('RESOURCES','Loom Use','그룹별 사용 명세와 호출 예제')+'<button class="pa-button" type="button" data-loom-mcp-refresh>새로고침</button><span class="pa-doc-copy-status" role="status" aria-live="polite"></span><section class="pa-loom-mcp" aria-label="MCP 사용 명세">'+body+'</section>';
}
function loomDocRefresh() {
  for (const panel of root.querySelectorAll('.pa-loom-mcp')) {
    const template=document.createElement('template');template.innerHTML=loomMcpView();
    patchResourceNode(panel,template.content.querySelector('.pa-loom-mcp'),true);
  }
}
root.addEventListener('click',event=>{
  const group=event.target.closest('[data-doc-group]'),action=event.target.closest('[data-doc-action]');
  if (group) {loomDocGroup=group.dataset.docGroup;loomDocAction='';loomDocRefresh();root.querySelector('[data-doc-group="'+CSS.escape(loomDocGroup)+'"]')?.focus();}
  if (action) {loomDocGroup=action.dataset.docParent;loomDocAction=action.dataset.docAction;loomDocRefresh();root.querySelector('[data-doc-action="'+CSS.escape(loomDocAction)+'"]')?.focus();}
  const copy=event.target.closest('[data-doc-copy]');
  if (copy) parent.postMessage({type:'loom-doc-copy',text:copy.closest('.pa-doc-code').querySelector('code').textContent},'*');
});
window.addEventListener('message',event=>{
  if (event.source!==parent || event.data?.type!=='loom-doc-copied') return;
  for (const status of root.querySelectorAll('.pa-doc-copy-status')) status.textContent=event.data.ok?'복사했습니다.':'복사하지 못했습니다. 코드 블록을 선택해 복사해 주세요.';
});
