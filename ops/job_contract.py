"""Job display metadata, independent of execution profiles and verification."""
REVISION = 'loom-job/v1'
API_REVISION = 'loom-ops/v37'
CATEGORIES = {
    'sdg': 'SDG', 'data_preparation': 'Data Preparation',
    'fine_tuning': 'Fine-tuning', 'evaluation': 'Evaluation',
    'pipeline': 'Pipeline', 'other': 'Other',
}
MODES = {'full': 'Full', 'pilot': 'Pilot-test', 'smoke': 'Smoke-test'}
FIELDS = {
    'category': {'type': 'string', 'enum': list(CATEGORIES), 'default': 'other',
                 'description': 'Job purpose; does not select an execution runner.'},
    'execution_mode': {'type': ['string', 'null'], 'enum': [*MODES, None], 'default': None,
                       'description': 'Full run, reduced pilot, or short smoke check. Null means unrecorded.'},
}


def values(spec, *, strict=False):
    category, mode = spec.get('category', 'other'), spec.get('execution_mode')
    if not isinstance(category, str) or category not in CATEGORIES:
        if strict: raise ValueError('Invalid Job category')
        category = 'other'
    if mode is not None and (not isinstance(mode, str) or mode not in MODES):
        if strict: raise ValueError('Invalid Job execution_mode')
        mode = None
    return {'category': category, 'execution_mode': mode}


def contract():
    return {'revision': REVISION, 'fields': FIELDS, 'categories': CATEGORIES,
            'execution_modes': MODES,
            'semantics': 'Classification does not change kind, params, task level, verification, or dispatch.'}


def select(items, args):
    for field, choices in [('category', CATEGORIES), ('execution_mode', MODES)]:
        if field in args and (not isinstance(args[field], str) or args[field] not in choices):
            raise ValueError('Invalid Job filter: ' + field)
    offset, limit = args.get('offset', 0), args.get('limit', 250)
    if type(offset) is not int or offset < 0 or type(limit) is not int or not 1 <= limit <= 1000:
        raise ValueError('Invalid Job cursor or limit')
    rows = [item for item in items if all(item.get(f) == args[f] for f in FIELDS if f in args)]
    rows.sort(key=lambda item: (item.get('created') or 0, item['id']), reverse=True)
    return rows[offset:offset + limit]
