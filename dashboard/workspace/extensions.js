// Registry presentation only; no source execution or mutation from the browser.
function loomToolsView() { return catalogList('tool'); }
function loomExtensionDetail(entity) {
  const item=entity.loom_extension;
  const pins=item.manifest?.source_pins||{};
  const tools=item.manifest?.tools||{};
  return heading(entity.kind==='tool'?'TOOL':'COMPONENT',entity.label,'')+catStatus()
    +panel('Properties',catProperties({Owner:item.owner_name||item.owner,
      Revision:item.revision,Role:entity.kind==='tool'?'Research software':'Job lifecycle',
      SHA256:item.sha256,Description:item.manifest?.description}))
    +panel('Pinned Tools',catProperties(tools))
    +panel('Source files',catTable(['File','SHA256'],Object.entries(pins).map(row=>row.map(esc))))
    +panel('Source metadata',`<details><summary>Manifest</summary><pre>${esc(JSON.stringify(item.manifest,null,2))}</pre></details>`);
}
