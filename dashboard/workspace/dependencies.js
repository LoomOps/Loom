const loomDependencyCategories={'languages':'Languages','simulation':'Simulation','machine-learning':'Machine Learning','gpu':'GPU & Drivers','libraries':'Libraries & Tools'};
function loomDependencyRows(){return (catalogData.all_entities||catalogData.entities).filter(e=>e.loom_dependency);}
function loomDependencyRuntimes(refs){return refs.map(r=>catLink(r.entity,r.label+(r.revision?' · r'+r.revision:''))).join(' · ');}
function loomDependencyOrigin(origin){return origin.path?`<code>${esc(origin.path)}</code>`:origin.image?`<span>Image</span> <code title="${esc(origin.image)}">${esc(origin.image.slice(0,19))}…</code>`:esc(origin.type==='unrecorded'?'—':origin.type);}
function loomDependenciesView(){
 const rows=loomDependencyRows(),query=(state.dependencyQuery||'').toLowerCase(),category=state.dependencyCategory||'';
 const filtered=rows.filter(e=>(!category||e.loom_dependency.category===category)&&[e.label,e.loom_dependency.name,...e.loom_dependency.versions.map(v=>v.version)].some(s=>s.toLowerCase().includes(query)));
 const groups=Object.entries(loomDependencyCategories).map(([key,label])=>{const matches=filtered.filter(e=>e.loom_dependency.category===key);if(!matches.length)return '';return `<section class="pa-dependency-group"><h2>${esc(label)} <small>${matches.length}</small></h2><div class="pa-extension-grid">${matches.map(e=>{const d=e.loom_dependency;return `<article class="pa-extension-card" data-dependency-card="${esc(d.id)}"><h3>${catLink(e.id)}</h3><div class="pa-dependency-versions">${d.versions.map(v=>`<span>${esc(v.version)}</span>`).join('')}</div><div class="pa-dependency-links">${loomDependencyRuntimes(d.runtimes)}</div></article>`;}).join('')}</div></section>`;}).join('');
 return loomHubTabs()+heading('RUNTIMES','Dependencies','')+catStatus()+`<div class="pa-extension-toolbar"><label class="pa-extension-search">${icon('search')}<input type="search" data-dependency-query placeholder="Search" aria-label="Dependency search" value="${esc(state.dependencyQuery||'')}"></label></div><nav class="pa-dependency-categories" aria-label="Dependency categories"><button data-dependency-category="" aria-pressed="${!category}">All <b>${rows.length}</b></button>${Object.entries(loomDependencyCategories).map(([key,label])=>`<button data-dependency-category="${key}" aria-pressed="${category===key}">${esc(label)} <b>${rows.filter(e=>e.loom_dependency.category===key).length}</b></button>`).join('')}</nav>`+(catalogData.runtime_catalog?.status==='unavailable'?'<div class="pa-extension-empty" role="alert">Runtime catalog unavailable</div>':groups||'<div class="pa-extension-empty">No dependencies</div>');
}
function loomDependencyDetail(entity){
 const d=entity.loom_dependency,profiles=[...new Set(d.runtimes.flatMap(r=>catEntity(r.entity)?.loom_runtime?.used_by_profiles?.map(p=>p.kind)||[]))];
 return loomHubTabs()+heading('DEPENDENCY',d.label,'',`<button class="pa-button" data-dependency-home>Dependencies</button>`)+catStatus()+`<div class="pa-software-tags"><span>${esc(loomDependencyCategories[d.category]||d.category)}</span></div>`+panel('Versions',d.versions.map(v=>`<section class="pa-dependency-version"><h3>${esc(v.version)}</h3>${v.installations.map(i=>`<div class="pa-dependency-installation"><div>${loomDependencyOrigin(i.origin)}</div><div class="pa-dependency-links">${loomDependencyRuntimes(i.runtimes)}</div></div>`).join('')}</section>`).join(''))+(profiles.length?panel('Profiles',profiles.map(kind=>catLink('profile:'+kind,loomProfileName(catEntity('profile:'+kind)?.loom_execution_profile||{kind}))).join(' · ')):'');
}
root.addEventListener('click',event=>{
 const button=event.target.closest('[data-dependency-category],[data-dependency-home]');if(!button)return;
 loomExtensionView(button);
 if(button.hasAttribute('data-dependency-home'))return loomHubNavigate({page:'dependencies',entity:null});
 state.dependencyCategory=button.dataset.dependencyCategory;render();persist();
});
root.addEventListener('input',event=>{
 if(!event.target.matches('[data-dependency-query]')||event.isComposing)return;
 const input=event.target,start=input.selectionStart;loomExtensionView(input);state.dependencyQuery=input.value;render();persist();
 const next=root.querySelector('.pa-document-pane.is-active [data-dependency-query]');next?.focus();next?.setSelectionRange(start,start);
});
