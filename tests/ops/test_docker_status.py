"""Image removal is a row condition; daemon failures remain collection errors."""
import json
import subprocess
import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch
import docker_status


class ImageMetadataTests(unittest.TestCase):
    def test_missing_image_keeps_successful_batch_records(self):
        error = subprocess.CalledProcessError(1, ['docker'],
            output=json.dumps({'id':'sha256:present', 'digests':['repo@sha256:abc']})+'\n',
            stderr='Error response from daemon: No such image: sha256:gone\n')
        with patch.object(docker_status, 'docker', side_effect=error):
            values, missing = docker_status.image_metadata([], ['sha256:present','sha256:gone'])
        self.assertEqual(values, {'sha256:present':['repo@sha256:abc']})
        self.assertEqual(missing, {'sha256:gone'})

    def test_local_build_without_registry_digest_is_available(self):
        with patch.object(docker_status, 'docker', return_value='{"id":"local","digests":null}\n'):
            self.assertEqual(docker_status.image_metadata([], ['local']), ({'local':[]},set()))

    def test_daemon_failure_is_not_reported_as_image_removed(self):
        error = subprocess.CalledProcessError(1, ['docker'], output='',stderr='permission denied')
        with patch.object(docker_status, 'docker', side_effect=error):
            with self.assertRaises(subprocess.CalledProcessError):
                docker_status.image_metadata([], ['local'])

class InstanceVisibilityTests(unittest.TestCase):
    def test_default_hides_development_even_from_host_reader_snapshot(self):
        raw = {'containers':[{'name':'runtime','compose_project':'loom'},
            {'name':'dev','compose_project':'loom-dev-build'},
            {'name':'custom','compose_project':'test','deployment_channel':'dev-build'},
            {'name':'demo','compose_project':None}]}
        with tempfile.TemporaryDirectory() as directory:
            result = docker_status.for_instance(raw, Path(directory))
        self.assertEqual([r['name'] for r in result['containers']], ['runtime','demo'])
        self.assertEqual(len(raw['containers']), 4)
        self.assertFalse(result['visibility']['show_dev_builds'])

    def test_development_requires_explicit_boolean_flag(self):
        raw = {'containers':[{'compose_project':'loom-dev-build'}]}
        with tempfile.TemporaryDirectory() as directory:
            node = Path(directory); (node/'config').mkdir()
            path = node/'config/docker-observations.json'
            path.write_text('{"show_dev_builds":true}')
            self.assertEqual(len(docker_status.for_instance(raw,node)['containers']),1)
            path.write_text('{"show_dev_builds":"false"}')
            self.assertEqual(docker_status.for_instance(raw,node)['containers'],[])

if __name__ == '__main__':
    unittest.main()

class ObservationPathTests(unittest.TestCase):
    def test_protected_mount_overrides_mutable_instance_observations(self):
        import os
        from unittest.mock import patch
        from pathlib import Path
        with patch.dict(os.environ,{'LOOM_OBSERVATIONS':'/run/loom-observations'}):
            self.assertEqual(docker_status.observation_path(Path('/loom/ops'),'docker-status.json'),Path('/run/loom-observations/docker-status.json'))
        with patch.dict(os.environ,{'LOOM_OBSERVATIONS':''}):
            self.assertEqual(docker_status.observation_path(Path('/loom/ops'),'docker-status.json'),Path('/loom/ops/docker-status.json'))
