"""Derive the Loom workspace from the sanitized synthetic workspace template."""
import argparse
import hashlib
import html
from pathlib import Path
import re
from lazy_logs import patch_logs

HERE = Path(__file__).resolve().parent
SHA = "81edb77dad1269315cbff745751b380d46f2d8ef0a3aa4f3045e8189c696a946"


def replace_once(value, old, new):
    if value.count(old) != 1:
        raise ValueError(f"Expected one occurrence of {old[:100]!r}")
    return value.replace(old, new, 1)


def build(original):
    raw = original.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SHA:
        raise ValueError("Synthetic template does not match its pinned SHA-256")
    outer = raw.decode()
    match = re.search(r'data-srcdoc="([^"]*)"', outer)
    inner = html.unescape(match[1])
    inner = patch_logs(inner)
    inner = inner.replace("['component','컴포넌트']", "['component','컴포넌트'],['tool','Tool']")
    inner = inner.replace("component:'컴포넌트'}", "component:'컴포넌트',tool:'Tool'}")
    inner = inner.replace("if(e.kind==='component')return catComponent(e);", "if(e.loom_extension)return loomExtensionDetail(e);if(e.kind==='component')return catComponent(e);")
    inner = replace_once(inner, "['mcp','MCP','plug']", "['mcp','Loom Use','plug']")
    inner = replace_once(inner, '<div class="pa-activity-track"><div class="pa-activity-months">',
                         '<div class="pa-activity-track">${activityYears(weeks)}<div class="pa-activity-months">')
    inner = replace_once(inner, "catSyncControl();resourceInterest();", "catSyncControl();resourceInterest();scheduleActivityLayout();")
    begin = inner.index("function initActivityScroll(){")
    end = inner.index("function activityView(", begin)
    inner = inner[:begin] + (HERE / "activity.js").read_text() + "\n" + inner[end:]
    inner = replace_once(inner, "if(e.target.classList?.contains('pa-pane-body'))catalogLastScroll=performance.now();",
                         "if(e.target.matches?.('.pa-pane-body,.pa-activity-graph,.pa-activity-list'))catalogLastScroll=performance.now();")
    inner = inner.replace('lucide.createIcons(', 'loomCreateIcons(')
    inner = inner.replace('</head>', '<script>'+(HERE/'icons.js').read_text()+'</script></head>', 1)
    inner = replace_once(inner, 'const icon=n=>`<i data-lucide="${n}" aria-hidden="true"></i>`;', 'const icon=n=>n=== "tailscale"?loomTailscaleIcon():`<i data-lucide="${n}" aria-hidden="true"></i>`;')
    inner = replace_once(inner, " target.innerHTML=markup;target.open=!!opened;", " const next=target.cloneNode(false);next.innerHTML=markup;patchResourceNode(target,next,true);target.open=!!opened;")
    inner = replace_once(inner, "function patchResourceNode(current,next,keyed=false){", """function patchResourceNode(current,next,keyed=false){
 if(current.nodeType===1&&next.nodeType===1){
  if(current.localName==='svg'&&next.localName==='i'&&current.getAttribute('data-lucide')&&current.getAttribute('data-lucide')===next.getAttribute('data-lucide'))return;
  if(current.hasAttribute('data-loom-media-display')&&current.getAttribute('data-loom-media-display')===next.getAttribute('data-loom-media-display')&&current.querySelector('video,img,.pa-readable-file'))return;
  if(keyed&&current.hasAttribute('data-container-row')&&current.dataset.containerRow===next.dataset.containerRow){next=next.cloneNode(true);next.setAttribute('aria-expanded',current.getAttribute('aria-expanded'));}
  if(keyed&&current.hasAttribute('data-container-extra')&&current.dataset.containerExtra===next.dataset.containerExtra){next=next.cloneNode(true);next.hidden=current.hidden;}
 }""")
    inner = replace_once(inner, " if(n.dataset.modelFilter)return", " if(n.hasAttribute('data-container-row'))return 'container:'+n.dataset.containerRow;\n if(n.hasAttribute('data-container-extra'))return 'container-detail:'+n.dataset.containerExtra;\n if(n.dataset.modelFilter)return")

    # Decorative skins are client-owned. Telemetry/catalog reconciliation must
    # retain them, otherwise each sample removes and re-adds the card background.
    inner = replace_once(inner, " const keepOpen=keyed&&current.nodeName==='DETAILS';", """ const skin=current.querySelector(':scope > .loom-surface-skin');
 if(skin&&[...current.classList].some(c=>c!=='loom-surface'&&next.classList.contains(c))){
  next.classList.add('loom-surface');
  if(!next.querySelector(':scope > .loom-surface-skin'))next.append(skin.cloneNode(true));
 }
 const keepOpen=keyed&&current.nodeName==='DETAILS';""")

    inner = replace_once(inner, 'const rows=workItems(),current=', 'const rows=loomFilteredWorkItems(),current=')
    inner = replace_once(inner, "current=rows.filter(e=>!queueState(e).info[3]).sort((a,b)=>(queueTimes(b).created??0)-(queueTimes(a).created??0))", "current=rows.filter(e=>!queueState(e).info[3]).sort(loomQueueCompare)")
    inner = replace_once(inner, 'summary+`<div class="pa-queue-caption">사용자 목표', 'summary+loomJobFilters()+loomQueueFeedback()+`<div class="pa-queue-caption">사용자 목표')
    inner = replace_once(inner, 'table(shown,false)',
        'table(shown.slice(0,state.queueHistoryLimits?.[historyFilter]||10),false)+loomHistoryMore(shown.length,historyFilter)')
    inner = replace_once(inner, '+workLogCard(rows);', ';')
    inner = replace_once(inner,
        "if(v.page!=='queue'||v.entity)openView({page:'queue',workLogId:id,openWorkGroups:[id]});else{v.workLogId=id;v.openWorkGroups=[...new Set([...(v.openWorkGroups||[]),id])];render();persist();}",
        "openView({page:'queue',entity:id});")
    inner = inner.replace("<b>큐 예약</b> ${reservations}", "<b>이 환경의 실행 슬롯</b> ${reservations}${l?` · 대기 ${l.queued_jobs}건 · 조회 권한 내 작업`:''}")
    inner = inner.replace("'등록된 작업의 예약 없음'", "'현재 할당 없음'")
    inner = replace_once(inner, "resCard(v.path,s.storage,", "resCard(v.path,{...s.storage,status:v.status||s.storage?.status},")
    inner = inner.replace("[{path:'data/0'},{path:'data/1'}]", "[{path:'Storage'}]")
    inner = inner.replace("s?.status==='stale'?'이전 값':'수집 불가'", "s?.status==='stale'?'이전 값':s?.status==='warming'?'샘플 준비 중':'수집 불가'")
    inner = replace_once(inner, '<!-- LOOM_BRAND_LOGO -->', (HERE.parent/'branding/logo.svg').read_text().strip())
    inner = replace_once(inner, "const mode=['light','dark','system'].includes(ws.theme)?ws.theme:'system';", "const mode=window.loomThemePreference||'system';")
    inner = replace_once(inner, "ws.theme=b.dataset.theme;applyTheme();persist();", "window.loomThemePreference=b.dataset.theme;ws.theme=b.dataset.theme;applyTheme();parent.postMessage({type:'loom-theme-choice',mode:b.dataset.theme},'*');persist();")
    inner, replaced = re.subn(
        r"let catalogData=catTitleSnapshot\(\{.*?\}\);\nlet catalogConnection=",
        "let catalogData=catTitleSnapshot({entities:[],sources:[],revision:null});\nlet catalogConnection=",
        inner, count=1, flags=re.DOTALL,
    )
    if replaced != 1:
        raise ValueError("Original embedded catalog seed was not found")
    inner = inner.replace("https://dashboard.example.invalid/", "")
    inner = inner.replace("실시간 연결", "최근 조회 성공").replace("스냅샷 · 서버 연결 중", "카탈로그 연결 중")
    inner = re.sub(r"connect-src [^;]+;", "connect-src 'none';", inner, count=1)
    inner = inner.replace(
        "['agent','Agent','bot']]", "['tools','Tools','wrench'],['docker','Docker','boxes'],['mcp','MCP','plug'],['access','Tailscale','tailscale']]",
    ).replace(
        "files};", "files,tools:loomToolsView,docker:loomDockerView,mcp:loomMcpView,access:loomAccessView};",
    ).replace(
        "issues:'issues',agent:'agent'}", "issues:'issues',agent:'queue',tools:'tools',docker:'docker',mcp:'mcp',tailnet:'access',tailscale:'access'}",
    )
    if "['tools','Tools','wrench']" not in inner:
        inner=replace_once(inner,"['docker','Docker','boxes']","['tools','Tools','wrench'],['docker','Docker','boxes']")
    inner=replace_once(inner,"['tools','Tools','wrench']","['tools','Extensions','puzzle']")
    inner=replace_once(inner,"return {page:e?.kind==='model'?", "return {page:['tool','component'].includes(e?.kind)?'tools':e?.kind==='model'?")
    inner=replace_once(inner,"const catEntity=id=>catalogData.entities.find(e=>e.id===id)||(typeof workStepRecord==='function'?workStepRecord(id):null);",
        "const catEntity=id=>catalogData.entities.find(e=>e.id===id)||(typeof workStepRecord==='function'?workStepRecord(id):null)||loomHistoricalExtensionEntity(id);")
    marker = "catalogRefreshViews();parent.postMessage({type:'pai-catalog-ready'},'*');"
    inner_additions = "\n".join((HERE / name).read_text() for name in ("mcp.js", "frame.js", "extensions.js", "details.js", "queue.js", "text-preview.js", "media.js", "explorer.js", "surfaces.js"))
    inner = replace_once(inner, marker, inner_additions + "\n" + (HERE.parent/"branding/brand.js").read_text() + "\n" + marker)
    inner = inner.replace("</head>", "<style>" + (HERE / "frame.css").read_text() + "</style></head>", 1)
    style = (HERE / "operations-desk.css").read_text() + (HERE.parent/"branding/brand.css").read_text()
    inner = replace_once(inner, "</body>", '<style id="pa-operations-desk">' + style + "</style></body>")
    outer = outer[:match.start(1)] + html.escape(inner, quote=True) + outer[match.end(1):]
    begin = outer.index("// Parent owns transport. Sandboxed opaque iframe gets data, never credentials.")
    end = outer.index("})();", begin) + len("})();")
    outer = outer[:begin] + (HERE.parent / "theme/theme.js").read_text() + "\n" + (HERE / "projection.js").read_text() + "\n" + (HERE / "parent.js").read_text() + outer[end:]
    # Per-browser-tab history survives reload without sharing documents across
    # tabs or persisting credentials/catalog data in localStorage.
    outer = outer.replace("[key]:{epoch:current?.epoch,index:current?.index}",
                          "[key]:{epoch:current?.epoch,index:current?.index,workspace:current?.workspace}")
    outer = replace_once(outer, " const routeURL=()=>paiRouteHash(selectedRoute(current?.workspace));", """ const savedWorkspace=history.state?.[key]?.workspace;
 if(validWorkspace(savedWorkspace)&&JSON.stringify(savedWorkspace).length<=262144&&
    (!pendingRoute||paiRouteHash(pendingRoute)===paiRouteHash(selectedRoute(savedWorkspace)))){
  current={epoch,index:0,workspace:savedWorkspace};pendingRoute=null;
 }
 const routeURL=()=>paiRouteHash(selectedRoute(current?.workspace));""")
    outer = outer.replace(
        "connect-src blob: data: https://loom-machine.example.ts.net wss://loom-machine.example.ts.net;",
        "connect-src 'self' blob: data:;",
    )
    outer = replace_once(outer, '<iframe id="codex-visualization"', '<iframe hidden id="codex-visualization"')
    outer = replace_once(outer, "</head>", "<style>" + (HERE / "parent.css").read_text() + (HERE.parent/"branding/brand.css").read_text() + "</style></head>")
    outer = replace_once(outer, "</iframe>", "</iframe>" + (HERE / "access.html").read_text().replace("<!-- LOOM_BRAND_LOGO -->", (HERE.parent/"branding/logo.svg").read_text()))
    if any(host in outer for host in ("loom-machine.example.ts.net", "dashboard.example.invalid")):
        raise ValueError("The Example Target workspace must not reference the previous service origins")
    return outer


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("original", type=Path)
    cli.add_argument("--output", type=Path, default=HERE.parent / "public" / "workspace.html")
    args = cli.parse_args()
    args.output.write_text(build(args.original))
