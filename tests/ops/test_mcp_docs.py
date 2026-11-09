import json
from jsonschema import Draft202012Validator
import ops,mcp_docs,mcp_groups
from test_support import IsolatedOps

class McpDocumentationTests(IsolatedOps):
    def reference(self,actor=None,**args):
        return ops.operate('system',{'action':'describe','arguments':args},actor or self.owner,self.config)
    def test_all_visible_actions_documented_and_examples_match_schema(self):
        docs=self.reference()
        expected={(t['name'],a) for t in mcp_groups.tools(self.owner,ops.TOOLS) for a in t['inputSchema']['properties']['action']['enum']}
        actual=set()
        for group in docs['groups']:
            for action in group['actions']:
                actual.add((group['name'],action['action']))
                Draft202012Validator.check_schema(action['schema'])
                Draft202012Validator(action['schema']).validate(action['example'])
        self.assertEqual(actual,expected);self.assertEqual(len(docs['groups']),6)
        self.assertNotIn(self.owner['token_sha256'],json.dumps(docs))
    def test_reader_only_sees_authorized_actions(self):
        docs=self.reference(self.reader)
        pairs={(g['name'],a['action']) for g in docs['groups'] for a in g['actions']}
        self.assertIn(('jobs','get'),pairs);self.assertNotIn(('jobs','submit'),pairs)
        self.assertNotIn(('components','register'),pairs);self.assertNotIn(('recovery','claim'),pairs)
        with self.assertRaises(PermissionError):self.reference(self.reader,group='jobs',action='submit')
    def test_grouped_recovery_and_components_have_precise_inputs(self):
        schema=self.reference(group='recovery',action='resume')['schema']
        self.assertNotIn('action',schema['properties']);self.assertIn('lease',schema['required'])
        schema=self.reference(group='components',action='get')['schema']
        self.assertEqual(set(schema['properties']),{'id','owner','revision'})
        self.assertIn('default',self.reference(group='events',action='list')['schema']['properties']['kind'])
        self.assertIn('group',self.reference(group='system',action='describe')['schema']['properties'])
    def test_submit_schema_stays_bound_to_api_contract(self):
        actual=self.reference(group='jobs',action='submit')['schema']
        expected=ops.tool_schema('submit_job')
        self.assertEqual(actual['required'],expected['required'])
        self.assertEqual(actual['properties']['execution'],{**expected['properties']['execution'],'description':mcp_docs.FIELD_HELP['execution']})
        self.assertEqual(self.reference(group='jobs')['group'],'jobs')
        with self.assertRaises(ValueError):self.reference(action='list')

    def test_public_result_operations_have_grouped_discovery_and_bundle_reference(self):
        # This list is the HTTP result contract, independent of GROUPS. A missing
        # mapping must fail rather than disappear from both generated expectations.
        operations={'list_registry','get_entity','reindex_job','reconnect_location',
                    'annotate_job','list_history','get_history','assign_entity_owner'}
        self.assertEqual(set(mcp_groups.GROUPS['results']['actions'].values()),operations)
        self.assertIn('results.reconnect',mcp_docs.ACTIONS)
        schema=self.reference(group='results',action='reconnect')['schema']
        self.assertEqual(set(schema['required']),{'id','revision','component','ref'})
        self.assertEqual(schema['properties']['storage']['default'],'data/0')
        with self.assertRaises(PermissionError):self.reference(self.reader,group='results',action='reconnect')

    def test_group_index_is_compact_and_scope_filtered(self):
        value=self.reference(self.reader,group='components')
        names={a['action'] for a in value['actions']}
        self.assertIn('list',names);self.assertNotIn('register',names)
        self.assertTrue(all('schema' not in a for a in value['actions']))
        with self.assertRaises(ValueError):self.reference(group='unknown')
