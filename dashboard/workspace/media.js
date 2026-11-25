const loomMediaTypes = new Set([...loomTextTypes,'image/png','image/jpeg','image/webp','video/mp4','video/webm']);
let loomMediaDisplay = null;

function loomMediaPanel(entity, items = entity?.loom_media) {
  if (!Array.isArray(items)) return '';
  if (!items.length) return panel('파일 미리보기', '<p>미리보기 가능한 등록 파일이 없습니다.</p>');
  const rows = items.map(media => {
    const id = esc(media.id);
    const label = esc(media.label || media.id);
    if (media.status !== 'available') {
      return '<p>' + label + ' · ' + esc(loomPayloadLabels[media.status] || (media.status === 'too_large' ? '미리보기 크기 한도 초과' : media.status) || '미확인') + '</p>';
    }
    if (!loomMediaTypes.has(media.mime)) return '<p>' + label + ' · 미리보기 형식 미지원</p>';
    return '<div class="pa-row"><span>' + label + '</span><button class="pa-button" type="button"'
      + ' data-loom-media="' + id + '" data-loom-media-entity="' + esc(entity.id) + '">미리보기</button></div>';
  });
  return panel('파일 미리보기', rows.join('') + '<div data-loom-media-display="' + esc(entity.id) + '" aria-live="polite"></div>');
}

function loomMediaElement(type, url, label) {
  const media = document.createElement(type.startsWith('video/') ? 'video' : 'img');
  media.className = 'pa-loom-preview';
  if (media.tagName === 'VIDEO') {
    media.controls = true;
    media.preload = 'metadata';
    media.playsInline = true;
    media.setAttribute('aria-label', label);
  } else {
    media.alt = label;
  }
  media.src = url;
  return media;
}

function loomCloseMedia() {
  if (loomMediaDisplay?.url) URL.revokeObjectURL(loomMediaDisplay.url);
  loomMediaDisplay = null;
}

const loomOutputFilePages=new Map();
function loomOutputFileSection(entity){
 const key=entity.item_revision_id;
 return `<details data-output-files="${esc(entity.id)}"${state.outputFilePanels?.includes(key)?' open':''}>
  <summary>등록 파일</summary><div data-output-file-body></div></details>`;
}
function loomLoadOutputFiles(section,more=false){
 const entity=catEntity(section.dataset.outputFiles);
 if(!section.open||!entity?.item_id)return;
 const key=JSON.stringify([entity.item_revision_id,loomCatalogStamp('outputs')]);
 const body=section.querySelector('[data-output-file-body]');
 const draw=page=>{
  if(!section.isConnected)return;
  const rows=page.items||[],media=page.media||[];
  body.innerHTML=rows.length?catTable(['File','Location','Status'],rows.map(f=>[esc(f.label),esc(f.path),esc(loomFileStatus(f.status))]))
   :page.pending?'<span role="status">Loading…</span>':'';
  if(media.length)body.insertAdjacentHTML('beforeend',loomMediaPanel(entity,media));
  if(page.error)body.insertAdjacentHTML('beforeend','<button data-output-files-retry>Retry</button>');
  else if(page.next!=null)body.insertAdjacentHTML('beforeend','<button data-output-files-more'+(page.pending?' disabled':'')+'>More</button>');
  else if(!page.pending&&!rows.length)body.textContent='등록된 파일이 없습니다.';
 };
 const previous=loomOutputFilePages.get(key);
 if(previous&&!more&&!previous.error){draw(previous);previous.promise?.then(draw);return;}
 if(more&&(!previous||previous.pending||previous.next==null))return;
 const page={items:more?previous.items:[],media:more?previous.media:[],next:more?previous.next:null,pending:true};
 loomOutputFilePages.set(key,page);draw(page);
 page.promise=loomSearchAction('system','dashboard_entity',{item_id:entity.item_id,revision_id:entity.item_revision_id,section:'files',
   offset:more?previous.next:0,limit:50}).then(result=>{
  page.items=[...page.items,...result.items];page.media=[...page.media,...(result.media||[])];page.next=result.next_offset;return page;
 }).catch(()=>{page.error=true;return page;}).finally(()=>{
  page.pending=false;draw(page);
  while(loomOutputFilePages.size>32)loomOutputFilePages.delete(loomOutputFilePages.keys().next().value);
 });
}
root.addEventListener('toggle',event=>{
 const details=event.target;
 if(!details.matches?.('[data-output-files]'))return;
 const entity=catEntity(details.dataset.outputFiles),pane=ws.panes.find(p=>p.id===details.closest('[data-pane]')?.dataset.pane);
 if(!entity||!pane)return;
 const view=activeTab(pane).view,open=new Set(view.outputFilePanels||[]);
 if(details.open){open.add(entity.item_revision_id);loomLoadOutputFiles(details);}
 else open.delete(entity.item_revision_id);
 view.outputFilePanels=[...open];
},true);
root.addEventListener('click',event=>{
 const button=event.target.closest('[data-output-files-more],[data-output-files-retry]');
 if(button)loomLoadOutputFiles(button.closest('[data-output-files]'),button.hasAttribute('data-output-files-more'));
});
const loomDetailWithLocation = catalogDetail;
catalogDetail = function(id) {
  const entity=catEntity(id);
  return loomDetailWithLocation(id)+(entity?.deferred_output_sections?loomOutputFileSection(entity):loomMediaPanel(entity));
};

root.addEventListener('click', event => {
  const button = event.target.closest('[data-loom-media]');
  if (!button) return;
  const entity = button.dataset.loomMediaEntity;
  const id = button.dataset.loomMedia;
  const destination = [...button.closest('.pa-panel').querySelectorAll('[data-loom-media-display]')]
    .find(item => item.dataset.loomMediaDisplay === entity);
  if (!destination) return;
  loomCloseMedia();
  loomMediaDisplay = {entity, id, destination};
  destination.textContent = '파일을 읽는 중입니다.';
  parent.postMessage({type:'loom-media-request', entity, id}, '*');
});

window.addEventListener('message', async event => {
  const message = event.data;
  const display = loomMediaDisplay;
  if (event.source !== parent || message?.type !== 'loom-media-response'
      || !display || display.id !== message.id || display.entity !== message.entity) return;
  if (!display.destination.isConnected) {
    loomCloseMedia();
    return;
  }
  const {blob, label, error} = message;
  if (!(blob instanceof Blob) || !loomMediaTypes.has(blob.type)) {
    display.destination.textContent = error === 403 ? '미디어 조회 권한이 없습니다.'
      : error === 404 ? '미디어 파일을 찾을 수 없습니다.'
      : error === 410 ? '이 미디어는 이관 대상에서 제외됐습니다.'
      : error === 413 ? '미리보기 크기 한도를 초과했습니다.'
      : '미디어를 조회할 수 없습니다.';
    return;
  }
  if (loomTextTypes.has(blob.type)) {
    try {
      const content=await loomTextElement(blob,label||'등록 파일');
      if (display===loomMediaDisplay && display.destination.isConnected) display.destination.replaceChildren(content);
    } catch {display.destination.textContent='UTF-8 텍스트로 읽을 수 없는 파일입니다.';}
    return;
  }
  display.url = URL.createObjectURL(blob);
  const media = loomMediaElement(blob.type, display.url, label || '기록된 미디어');
  if (media.tagName === 'VIDEO') {
    media.addEventListener('error', () => {
      const status = document.createElement('p');
      status.textContent = media.error?.code === 4
        ? '이 브라우저에서 영상 형식을 재생할 수 없습니다.' : '영상을 재생할 수 없습니다.';
      const download = document.createElement('button');
      download.type = 'button';
      download.className = 'pa-button';
      download.dataset.loomMediaDownload = '';
      download.textContent = '원본 영상 다운로드';
      download.addEventListener('click', () => parent.postMessage({
        type:'loom-media-download', entity:display.entity, id:display.id
      }, '*'));
      display.destination.append(status, download);
    }, {once:true});
  }
  display.destination.replaceChildren(media);
});

window.addEventListener('pagehide', loomCloseMedia);
