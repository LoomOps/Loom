"""Verify the active build paths without building images or touching instances."""
import ast
import importlib.util
from pathlib import Path
import shlex
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('loom_snapshot_test',ROOT/'deploy/snapshot.py')
snapshot=importlib.util.module_from_spec(spec);spec.loader.exec_module(snapshot)


def copied_python(dockerfile):
    result=set()
    for line in dockerfile.read_text().splitlines():
        parts=shlex.split(line)
        if not parts or parts[0]!='COPY' or any(p.startswith('--from=') for p in parts):continue
        result.update(Path(p).name for p in parts[1:-1] if p.endswith('.py'))
    return result


class PackagingTests(unittest.TestCase):
    def test_installations_with_same_compose_name_have_distinct_identities(self):
        import os,json,sys
        sys.path.insert(0,str(ROOT/'deploy'))
        spec=importlib.util.spec_from_file_location('fresh_loom_install',ROOT/'deploy/install.py')
        installer=importlib.util.module_from_spec(spec);spec.loader.exec_module(installer)
        with tempfile.TemporaryDirectory() as tmp:
            configs=[]
            for label in ('first','second'):
                root=Path(tmp)/label/'ops';data=Path(tmp)/label/'data'
                installer.install(root,data,18080,18081,18082,'loom','ops:test','dashboard:test',os.getuid(),os.getgid())
                configs.append(json.loads((root/'config/api.json').read_text()))
            self.assertNotEqual(configs[0]['machine_id'],configs[1]['machine_id'])
            self.assertNotEqual(configs[0]['installation_id'],configs[1]['installation_id'])
            self.assertTrue(all(c['machine_name']=='loom' for c in configs))

    def test_existing_data_contents_and_ownership_are_preserved(self):
        import os,sys
        sys.path.insert(0,str(ROOT/'deploy'))
        spec=importlib.util.spec_from_file_location('existing_data_install',ROOT/'deploy/install.py')
        installer=importlib.util.module_from_spec(spec);spec.loader.exec_module(installer)
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)/'ops';data=Path(tmp)/'data';data.mkdir()
            file=data/'existing.bin';file.write_bytes(b'preserved');before=file.stat()
            original=installer.own_tree
            with patch.object(installer,'own_tree',wraps=original) as own:
                installer.install(root,data,18080,18081,18082,'loom','ops:test','dashboard:test',os.getuid(),os.getgid())
            self.assertEqual(file.read_bytes(),b'preserved')
            self.assertEqual((file.stat().st_uid,file.stat().st_gid),(before.st_uid,before.st_gid))
            self.assertNotIn(data,[call.args[0] for call in own.call_args_list])
            with self.assertRaises(ValueError):
                installer.install(Path(tmp)/'second',data,18080,18081,18082,'loom','ops:test','dashboard:test',os.getuid(),os.getgid())

    def test_host_executor_and_installer_parse(self):
        for path in [ROOT/'executor/server.py',ROOT/'deploy/install.py',ROOT/'deploy/executor-attach.py']:
            ast.parse(path.read_text(),filename=str(path))

    def test_local_api_imports_are_packaged_in_active_image(self):
        local={p.stem for p in (ROOT/'ops').glob('*.py')}
        for name in ['Dockerfile.loom']:
            included=copied_python(ROOT/'ops'/name)
            pending=['ops.py'];seen=set()
            while pending:
                module=pending.pop()
                if module in seen:continue
                seen.add(module)
                self.assertIn(module,included, f'{name} omits runtime dependency {module}')
                tree=ast.parse((ROOT/'ops'/module).read_text())
                imports=[]
                for node in ast.walk(tree):
                    if isinstance(node,ast.Import):imports.extend(a.name.split('.')[0] for a in node.names)
                    if isinstance(node,ast.ImportFrom) and node.module:imports.append(node.module.split('.')[0])
                pending.extend(m+'.py' for m in imports if m in local)
    def test_snapshot_excludes_runtime_secrets_and_includes_tests(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            for name in ['ops/app.py','tests/ops/test_app.py','INSTALL.md','tests/evidence/report.json','ops/live.sqlite3','ops/secrets/key','deploy/evidence/a.json','dashboard/workspace/frame.js','components/shared.py','legacy/installation/prepare_storage.py','legacy/research-docs/metadata-contract.md']:
                p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text('fixture')
            with patch.object(snapshot,'ROOT',root):names={str(p.relative_to(root)) for p in snapshot.sources()}
            self.assertEqual(names,{'ops/app.py','tests/ops/test_app.py','INSTALL.md','dashboard/workspace/frame.js','components/shared.py'})
    def test_snapshot_rejects_source_link_outside_checkout(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/'source';(root/'ops').mkdir(parents=True)
            outside=Path(temp)/'external';outside.write_text('fixture')
            (root/'ops/link.py').symlink_to(outside)
            with patch.object(snapshot,'ROOT',root),self.assertRaises(ValueError):list(snapshot.sources())

    def test_usage_tools_are_in_snapshot_and_ops_build_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            for ref in ['ops/app.py','loom_use/README.md','loom_use/client-release.json','README.md','.dockerignore']:
                p=root/ref;p.parent.mkdir(parents=True,exist_ok=True);p.write_text('fixture')
            with patch.object(snapshot,'ROOT',root):
                files=list(snapshot.sources());hashes={str(p.relative_to(root)):'digest' for p in files}
                local=snapshot.component_files('ops',files,hashes)
            self.assertIn('../loom_use/client-release.json',local)
            self.assertIn('README.md',hashes);self.assertIn('.dockerignore',hashes)

    def test_live_build_detects_usage_tool_edits(self):
        import json,hashlib,os
        spec=importlib.util.spec_from_file_location('ops_build_fixture',ROOT/'ops/build_version.py')
        version=importlib.util.module_from_spec(spec);spec.loader.exec_module(version)
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'ops').mkdir();(root/'loom_use').mkdir()
            guide=root/'loom_use/entry.md';guide.write_text('original')
            info={'build_id':'fixture','channel':'dev-build','created_at':'fixture','source_sha256':'fixture','git_commit':None,'component':'ops','files':{'../loom_use/entry.md':hashlib.sha256(guide.read_bytes()).hexdigest()}}
            (root/'ops/build-info.json').write_text(json.dumps(info))
            with patch.object(version,'__file__',str(root/'ops/build_version.py')),patch.dict(os.environ,{'LOOM_LIVE_SOURCE':'1'}):
                self.assertFalse(version.status()['source_modified'])
                guide.write_text('updated shared usage tools')
                self.assertTrue(version.status()['source_modified'])

class DashboardBuildContextTests(unittest.TestCase):
    def test_every_dashboard_copy_input_is_in_the_build_context(self):
        import shlex
        root=Path(__file__).resolve().parents[2]/'dashboard'
        allowed=set((root/'.dockerignore').read_text().splitlines())
        for line in (root/'Dockerfile').read_text().splitlines():
            if not line.startswith('COPY '):continue
            for source in shlex.split(line)[1:-1]:
                with self.subTest(source=source):
                    name=source.rstrip('/')
                    self.assertIn('!'+name,allowed)
                    self.assertTrue((root/name).exists())
                    if (root/name).is_dir():self.assertIn('!'+name+'/**',allowed)

    def test_explicit_dashboard_context_includes_native_identity_runtime(self):
        root=Path(__file__).resolve().parents[2]
        allowed=set((root/'dashboard/.dockerignore').read_text().splitlines())
        for name in ['server.py','sso.py','requirements.txt','build_version.py','build-info.json','admission.py','sharing.py','managed_resources.py','admission_http.py','gateway.py','gateway.js','theme','theme/**']:
            self.assertIn('!'+name,allowed)
        self.assertNotIn('PyJWT',(root/'dashboard/requirements.txt').read_text())
