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

const loomDetailWithLocation = catalogDetail;
catalogDetail = function(id) {
  return loomDetailWithLocation(id) + loomMediaPanel(catEntity(id));
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
