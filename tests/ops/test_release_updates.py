import io
import json
from pathlib import Path
import tempfile
import threading
import time
import unittest
from unittest.mock import Mock
from urllib.error import HTTPError, URLError
import release_updates as releases


def release(tag, **extra):
    return {'tag_name': tag, 'draft': False, 'published_at': '2026-11-08T00:00:00Z', **extra}


class ReleaseChecks(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(); self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name); self.now = 1000
        self.config = releases.settings({'build_id':'0.1.0-alpha.9'}, {})
        self.fetch = Mock(return_value={'etag':'one','rows':[release('v0.1.0-alpha.10')],'next':False})
        self.checker = releases.Checker(self.config, self.root/'cache.json', self.root/'token', self.fetch, lambda:self.now)
        self.addCleanup(self.checker.close)

    def test_semver_prereleases_and_stable_channels(self):
        rows = [release('v0.1.0-alpha.9'),release('0.1.0-alpha.10'),release('0.1.0-beta.1'),
                release('0.1.0-rc.1'),release('0.1.0'),release('9.0.0',draft=True),release('10.0.0',published_at=None)]
        self.assertEqual(releases.select_release(rows,'alpha','org/repo')['tag'],'0.1.0')
        self.assertIsNone(releases.select_release(rows[:4],'stable','org/repo'))
        self.assertEqual(releases.select_release(rows[:2],'alpha','org/repo')['tag'],'0.1.0-alpha.10')
        self.assertIsNone(releases.select_release([release('1.0.0',prerelease=True)],'stable','org/repo'))
        for invalid in ['dev-build.123','latest','01.0.0','0.1.0-alpha.01',None]:self.assertIsNone(releases.version(invalid))

    def test_discovery_preserves_private_credential_and_compares_current_build(self):
        (self.root/'token').write_text('fixture-secret')
        self.checker.check(); result=self.checker.status()
        self.assertEqual(result['status'],'update_available');self.assertTrue(result['update_available'])
        self.assertFalse(result['automatic_install']);self.assertNotIn('fixture-secret',json.dumps(result))
        self.assertNotIn('fixture-secret',(self.root/'cache.json').read_text())
        self.assertEqual(self.fetch.call_args.args[2],'fixture-secret')
        self.assertEqual((self.root/'cache.json').stat().st_mode & 0o777,0o600)
        self.config['installed']='0.1.0';self.assertEqual(self.checker.status()['status'],'up_to_date')
        self.config['installed']='dev-build.123';self.assertEqual(self.checker.status()['status'],'unversioned')
        self.assertIsNone(self.checker.status()['update_available'])

    def test_failure_never_means_up_to_date_and_keeps_last_success(self):
        self.checker.check();self.now+=100
        self.fetch.side_effect=releases.CheckError('repository_unavailable')
        self.checker.check();value=self.checker.status()
        self.assertEqual(value['status'],'error');self.assertTrue(value['stale']);self.assertEqual(value['last_success_at'],1000)
        self.assertEqual(value['latest']['tag'],'v0.1.0-alpha.10');self.assertEqual(value['checked_at'],1100)
        # Fresh failure cannot claim there are no releases.
        self.checker.state['last_success_at']=None;self.checker.state['latest']=None
        self.assertIsNone(self.checker.status()['update_available'])

    def test_pagination_including_cached_pages_and_restart(self):
        self.fetch.side_effect=[{'rows':[release('0.1.0-alpha.9')],'etag':'1','next':True},
                                {'rows':[release('0.2.0-alpha.1')],'etag':'2','next':False}]
        self.checker.check();self.assertEqual(self.checker.status()['latest']['tag'],'0.2.0-alpha.1')
        restored=releases.Checker(self.config,self.root/'cache.json',self.root/'token',self.fetch,lambda:self.now)
        self.assertEqual(restored.pages['2']['etag'],'2');self.assertEqual(restored.status()['last_success_at'],1000)
        changed=releases.Checker({**self.config,'repository':'other/repo'},self.root/'cache.json')
        self.assertIsNone(changed.status()['last_success_at'])

    def test_incomplete_list_does_not_publish_partial_success(self):
        self.fetch.return_value={'rows':[release('9.0.0')],'etag':'1','next':True}
        self.checker.check();self.assertEqual(self.fetch.call_count,10)
        self.assertEqual(self.checker.status()['error'],'release_list_incomplete');self.assertIsNone(self.checker.status()['latest'])

    def test_disabled_and_manual_cooldown_and_rate_limit(self):
        self.config['enabled']=False;self.checker.start()
        self.assertEqual(self.checker.request()['reason'],'disabled');self.fetch.assert_not_called()
        self.config['enabled']=True
        self.assertEqual(self.checker.request()['reason'],'not_running')
        self.checker.thread=Mock()  # Deterministic scheduler receipt test.
        self.assertTrue(self.checker.request()['accepted']);self.assertEqual(self.checker.request()['reason'],'checking')
        self.checker.check();self.assertEqual(self.checker.request()['reason'],'cooldown')
        self.now+=61;self.assertTrue(self.checker.request()['accepted'])
        self.fetch.side_effect=releases.CheckError('rate_limited',9000);self.checker.check()
        self.now=8999;self.assertEqual(self.checker.request()['reason'],'cooldown')
        self.assertGreaterEqual(self.checker.status()['next_check_at'],9000)

    def test_slow_network_does_not_block_status_or_duplicate_requests(self):
        entered=threading.Event();leave=threading.Event();self.addCleanup(leave.set)
        def slow(*args):
            entered.set();leave.wait(3);return {'rows':[],'etag':'1','next':False}
        self.fetch.side_effect=slow;self.checker.start();self.assertTrue(entered.wait(2))
        start=time.monotonic()
        self.assertTrue(self.checker.status()['checking']);self.assertEqual(self.checker.request()['reason'],'checking')
        self.assertLess(time.monotonic()-start,.2);self.assertEqual(self.fetch.call_count,1)
        leave.set();self.checker.close()

    def test_invalid_token_and_bad_cache_are_safe(self):
        (self.root/'token').write_text('invalid\ncredential');self.checker.check()
        self.assertEqual(self.checker.status()['error'],'credential_invalid');self.fetch.assert_not_called()
        (self.root/'cache.json').write_text('{not json')
        self.assertIsNone(releases.Checker(self.config,self.root/'cache.json').status()['last_success_at'])

    def test_no_release_is_distinct_from_no_check(self):
        self.assertEqual(self.checker.status()['status'],'not_checked')
        self.fetch.return_value={'rows':[],'etag':'1','next':False};self.checker.check()
        self.assertEqual(self.checker.status()['status'],'no_release');self.assertIsNone(self.checker.status()['update_available'])

    def test_config_rejects_urls_and_invalid_flags(self):
        for value in [{'LOOM_RELEASE_REPOSITORY':'https://evil.test/repo'}, {'LOOM_RELEASE_CHANNEL':'edge'},
                      {'LOOM_RELEASE_INTERVAL':'0'}, {'LOOM_RELEASE_CHECK_ENABLED':'yes'}]:
            with self.assertRaises(ValueError):releases.settings({},value)
        self.assertEqual(releases.settings({'build_id':'1.0.0'}, {})['channel'],'stable')


class GitHubBoundary(unittest.TestCase):
    def fetch(self, error, cached=None):
        opener=Mock();opener.open.side_effect=error
        return releases.fetch_page('LoomOps/loom',1,'secret',cached or {},1000,opener)

    def test_etag_304_requires_previous_page_and_does_not_leak_token(self):
        old={'rows':[release('1.0.0')],'etag':'etag','next':False}
        self.assertEqual(self.fetch(HTTPError('https://api.github.com',304,'',{},None),old),old)
        with self.assertRaises(releases.CheckError):self.fetch(HTTPError('https://api.github.com',304,'',{},None))

    def test_http_error_mapping_and_retry_after(self):
        for status, expected in [(401,'credential_rejected'),(403,'access_denied'),(404,'repository_unavailable'),(500,'github_unavailable'),(302,'repository_moved')]:
            with self.subTest(status=status),self.assertRaises(releases.CheckError) as caught:
                self.fetch(HTTPError('secret',status,'secret',{},None))
            self.assertEqual(caught.exception.code,expected);self.assertNotIn('secret',str(caught.exception))
        with self.assertRaises(releases.CheckError) as caught:
            self.fetch(HTTPError('url',429,'',{'Retry-After':'120','X-RateLimit-Reset':'1500'},None))
        self.assertEqual(caught.exception.retry_at,1500)
        with self.assertRaises(releases.CheckError) as caught:self.fetch(URLError('secret'))
        self.assertEqual(caught.exception.code,'network_unavailable')

    def test_response_bound_and_conditional_request_headers(self):
        response=Mock();response.__enter__=Mock(return_value=response);response.__exit__=Mock(return_value=False);response.headers={'ETag':'v1','Link':'<https://api.github.com/repos/LoomOps/loom/releases?page=2>; rel="next"'}
        response.read.return_value=json.dumps([release('1.0.0')]).encode();opener=Mock();opener.open.return_value=response
        result=releases.fetch_page('LoomOps/loom',1,'secret',{'etag':'old'},1000,opener)
        self.assertTrue(result['next']);self.assertEqual(result['etag'],'v1')
        request=opener.open.call_args.args[0];self.assertEqual(request.get_header('If-none-match'),'old')
        self.assertEqual(request.get_header('Authorization'),'Bearer secret')
        response.read.return_value=b'x'*(releases.MAX_BYTES+1)
        with self.assertRaises(releases.CheckError):releases.fetch_page('LoomOps/loom',1,'',{},1000,opener)

    def test_redirect_handler_never_forwards_credential(self):
        self.assertIsNone(releases.NoRedirect().redirect_request(None,None,302,'',{},'https://other.test'))
