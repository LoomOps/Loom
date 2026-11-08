const loomTextTypes = new Set(['text/markdown','text/plain','application/json','application/x-ndjson']);

// Preserve numeric tokens exactly (JSON.stringify would round large integer IDs).
function loomPrettyJSON(source) {
  JSON.parse(source);
  let quoted=false, escaped=false, depth=0, result='';
  const newline=()=> '\n'+'  '.repeat(Math.min(depth,32));
  for (const char of source) {
    if (quoted) {
      result+=char;
      if (escaped) escaped=false;
      else if (char==='\\') escaped=true;
      else if (char==='"') quoted=false;
    } else if (char==='"') { quoted=true; result+=char; }
    else if (char==='{' || char==='[') { depth++; result+=char+newline(); }
    else if (char==='}' || char===']') { depth--; result+=newline()+char; }
    else if (char===',') result+=char+newline();
    else if (char===':') result+=': ';
    else if (!/\s/.test(char)) result+=char;
  }
  return result;
}

function loomInlineText(element, text) {
  const expression=/(`[^`\n]+`|\*\*[^*\n]+\*\*|\[[^\]\n]+\]\(https?:\/\/[^\s)]+\))/g;
  let previous=0;
  for (const match of text.matchAll(expression)) {
    element.append(document.createTextNode(text.slice(previous,match.index)));
    const token=match[0]; let node;
    if (token[0]==='`') {node=document.createElement('code');node.textContent=token.slice(1,-1);}
    else if (token.startsWith('**')) {node=document.createElement('strong');node.textContent=token.slice(2,-2);}
    else {
      const parts=/^\[([^\]]+)\]\((.+)\)$/.exec(token);
      node=document.createElement('a');node.textContent=parts[1];node.href=parts[2];node.target='_blank';node.rel='noopener noreferrer';
    }
    element.append(node);previous=match.index+token.length;
  }
  element.append(document.createTextNode(text.slice(previous)));
}

function loomMarkdown(source) {
  const documentView=document.createElement('article');documentView.className='pa-readable-markdown';
  const lines=source.replace(/\r\n/g,'\n').split('\n');let list=null;
  for(let i=0;i<lines.length;i++) {
    const line=lines[i];let item;
    if (/^\s*```/.test(line)) {
      const code=[];while(++i<lines.length && !/^\s*```/.test(lines[i])) code.push(lines[i]);
      item=document.createElement('pre');item.textContent=code.join('\n');list=null;
    } else if (/^#{1,6}\s/.test(line)) {
      const match=/^(#{1,6})\s+(.*)/.exec(line);item=document.createElement('h'+Math.min(match[1].length+1,6));loomInlineText(item,match[2]);list=null;
    } else if (/^\s*([-*+] |\d+\. )/.test(line)) {
      const ordered=/^\s*\d+\. /.test(line);const tag=ordered?'OL':'UL';
      if (!list || list.tagName!==tag) {list=document.createElement(tag);documentView.append(list);}
      const entry=document.createElement('li');loomInlineText(entry,line.replace(/^\s*(?:[-*+] |\d+\. )/,''));list.append(entry);continue;
    } else if (line.includes('|') && /^\s*\|?\s*:?-{3,}/.test(lines[i+1]||'')) {
      item=document.createElement('table');
      const row=(text,tag)=>{const tr=document.createElement('tr');for(const value of text.trim().replace(/^\||\|$/g,'').split('|')){const cell=document.createElement(tag);loomInlineText(cell,value.trim());tr.append(cell);}item.append(tr);};
      row(line,'th');i++;while(i+1<lines.length && lines[i+1].includes('|')) row(lines[++i],'td');list=null;
    } else if (line.startsWith('>')) {item=document.createElement('blockquote');loomInlineText(item,line.replace(/^>\s?/,''));list=null;}
    else if (/^\s*([-*_])\1{2,}\s*$/.test(line)) {item=document.createElement('hr');list=null;}
    else if (!line.trim()) {list=null;continue;}
    else {item=document.createElement('p');loomInlineText(item,line);list=null;}
    documentView.append(item);
  }
  return documentView;
}

async function loomTextElement(blob, label) {
  const source=new TextDecoder('utf-8',{fatal:true}).decode(await blob.arrayBuffer());
  if (source.includes('\0')) throw new Error('Binary content');
  const panel=document.createElement('section');panel.className='pa-readable-file';
  const bar=document.createElement('div');bar.className='pa-readable-toolbar';
  const title=document.createElement('span');title.textContent=label+' · '+blob.size.toLocaleString()+' bytes';
  const toggle=document.createElement('button');toggle.type='button';toggle.className='pa-button';
  const body=document.createElement('div');body.className='pa-readable-body';let raw=false;
  const render=()=>{
    toggle.textContent=raw?'읽기 보기':'원문';
    if (!raw && blob.type==='text/markdown') body.replaceChildren(loomMarkdown(source));
    else {
      const pre=document.createElement('pre');let value=source;
      if (!raw && blob.type==='application/json') {try{value=loomPrettyJSON(source);}catch{title.textContent=label+' · JSON 문법 오류 · 원문 표시';}}
      if (!raw && blob.type==='application/x-ndjson') {try{value=source.split('\n').filter(line=>line.trim()).map(loomPrettyJSON).join('\n\n');}catch{title.textContent=label+' · JSON Lines 문법 오류 · 원문 표시';}}
      pre.textContent=value||'(빈 파일)';body.replaceChildren(pre);
    }
  };
  toggle.addEventListener('click',()=>{raw=!raw;render();});bar.append(title,toggle);panel.append(bar,body);render();return panel;
}
