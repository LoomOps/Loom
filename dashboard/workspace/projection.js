// Adapt scoped Ops records to the original workspace's presentation fields.
function loomHost(host) {
  return typeof host === 'string' ? {name:host, hostname:host} : host;
}

function loomCatalogProjection(catalog) {
  const work = new Map(catalog.entities
    .filter(entity => entity.loom_history?.source === 'queue' && entity.loom_history.kind === 'work')
    .map(entity => [entity.loom_history.id, entity]));
  const entities = catalog.entities.flatMap(entity => {
    if (entity.loom_history?.source === 'queue' && entity.loom_history.kind !== 'job_view') return [];
    if (entity.loom_registry) {
      const kind = entity.kind === 'experiment' ? 'run' : entity.kind;
      const properties = {...entity.properties};
      const metrics = (entity.metrics || []).map(metric => Array.isArray(metric.observations) ? metric : ({
        ...metric,
        id:metric.name,
        kind:'series',
        label:metric.name,
        aggregation:metric.denominator,
        x_label:'step',
        observations:(metric.series || []).map(point => ({x:point.step, value:point.value})),
      }));
      return [{...entity, kind, label:entity.label || entity.id, properties, metrics,
        loom_job_id:entity.loom_job?.id}];
    }
    if (entity.loom_job && entity.kind === 'job') {
      return [{...entity, label:entity.label || entity.id,
        properties:{...entity.properties, state:entity.state, created_at:entity.created}}];
    }
    const originalWork = entity.properties?.record_role === 'work'
      ? work.get(entity.properties.id) || work.get(entity.id.replace(/^work:/, '')) : null;
    const displayRecord = originalWork?.properties ? {...entity, ...originalWork} : entity;
    return [{...displayRecord, revision:entity.revision ?? entity.loom_history?.revision,
      ...(originalWork ? {loom_history:originalWork.loom_history} : {})}];
  });
  return {...catalog, entities, host:loomHost(catalog.host),
    sources:(catalog.sources || []).filter(source => source && typeof source === 'object')};
}
