#!/usr/bin/env python3
"""Regression checks for preserving local work and the offline skills contract."""
import importlib.util
import json
import os
import subprocess
import tempfile
import unittest
from unittest.mock import patch, Mock
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('configure', ROOT / 'scripts/setup/configure.py')
configure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(configure)

spec = importlib.util.spec_from_file_location('finish', ROOT / 'scripts/setup/finish.py')
finish = importlib.util.module_from_spec(spec)
spec.loader.exec_module(finish)


class SetupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='ogh-setup-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.work = self.root / 'project with spaces'
        self.work.mkdir()
        self.skills = self.root / 'upstream'
        skill = self.skills / 'skills/om-example'
        skill.mkdir(parents=True)
        (skill / 'SKILL.md').write_text('---\nname: om-example\ndescription: Example\n---\n\nOriginal instructions: gh issue list\n')
        (skill / 'references').mkdir()
        (skill / 'references/details.md').write_text('Keep references intact')

    def run_config(self):
        configure.configure(self.work, self.skills, ROOT)

    def test_project_discovery_and_transport(self):
        self.run_config()
        for folder in ('.agents', '.claude', '.codex', '.pi'):
            link = self.work / folder / 'skills/om-example'
            self.assertTrue(link.is_symlink())
            text = (link / 'SKILL.md').read_text()
            self.assertTrue(text.startswith('---\nname: om-example'))
            self.assertLess(text.index('Offline workspace transport'), text.index('Original instructions'))
            self.assertIn('Use `ogh`, never', text)
            self.assertIn('mandatory live claim', text)
            self.assertTrue((link / 'references/details.md').exists())
        config = json.loads((self.work / '.ai/agentic.config.json').read_text())
        self.assertEqual(config['tracker'], 'offline-github')
        self.assertTrue((self.work / '.ai/trackers/offline-github.md').exists())

    def test_config_and_project_overrides_preserved(self):
        ai = self.work / '.ai'
        ai.mkdir()
        path = ai / 'agentic.config.json'
        original = {'tracker': 'github', 'custom': {'precious': 12}, 'validation': {'commands': ['original check']}}
        path.write_text(json.dumps(original))
        custom = self.work / '.claude/skills/om-example'
        custom.mkdir(parents=True)
        (custom / 'SKILL.md').write_text('project override')
        self.run_config()
        self.assertEqual(json.loads(path.read_text())['custom'], original['custom'])
        self.assertEqual((custom / 'SKILL.md').read_text(), 'project override')
        backups = list(ai.glob('agentic.config.json.ogh-backup-*'))
        self.assertEqual(json.loads(backups[0].read_text()), original)
        installed = ai / 'ogh-skills/om-example/SKILL.md'
        installed.write_text('user change')
        self.run_config()
        self.assertTrue(any((p / 'SKILL.md').read_text() == 'user change' for p in (ai / 'ogh-skills').glob('*.ogh-backup-*')))

    def test_helper_preserves_files_and_symlink_targets(self):
        helper = self.root / 'helper'
        helper.write_text('precious original')
        finish.install_helper(helper, 'new helper')
        self.assertEqual(next(self.root.glob('helper.ogh-backup-*')).read_text(), 'precious original')
        target = self.root / 'target'
        target.write_text('precious target')
        helper.unlink()
        helper.symlink_to(target)
        finish.install_helper(helper, 'replacement')
        self.assertEqual(target.read_text(), 'precious target')
        self.assertFalse(helper.is_symlink())
        self.assertTrue(any(p.is_symlink() for p in self.root.glob('helper.ogh-backup-*')))

    def test_sandbox_helper_preserves_symlink_target(self):
        script = (ROOT / 'scripts/sandbox.sh').read_text()
        block = script[script.index('mkdir -p "$HOME/.local/bin"'):script.index('echo "✅ Ready:')]
        folder = self.root / '.local/bin'
        folder.mkdir(parents=True)
        helper = folder / 'ogh-sandbox'
        target = self.root / 'precious'
        target.write_text('keep me')
        helper.symlink_to(target)
        subprocess.run(['bash', '-eu', '-c', block], check=True, timeout=10,
                       env=dict(os.environ, HOME=str(self.root), OGH_SANDBOX_NAME='test', OGH_SANDBOX_PORT='4183', OGH_GUEST_WORKSPACE='/root/Projects/My Mercato'))
        self.assertEqual(target.read_text(), 'keep me')
        self.assertFalse(helper.is_symlink())
        self.assertTrue(next(folder.glob('ogh-sandbox.backup.*/ogh-sandbox')).is_symlink())

    def test_custom_cache_reentry_and_same_cache_reused(self):
        import io
        cache = str(self.root / 'custom cache')
        def response(data_dir):
            return io.BytesIO(json.dumps({'status': 'ok', 'outgoing_queue': True, 'data_dir': data_dir}).encode())
        with patch.dict(os.environ, {'HOME': str(self.root), 'OGH_DATA_DIR': cache}):
            with patch('urllib.request.urlopen', side_effect=lambda url, **kw: response(cache) if url.endswith('/api/health') else io.BytesIO(b'{"tools":[],"workspaces":[]}')):
                finish.finish(self.work, self.root / 'state', 4181)
            helper = (self.root / '.local/bin/ogh-work').read_text()
            self.assertIn("export OGH_DATA_DIR='" + cache + "'", helper)
            self.assertEqual((self.root / 'state/server.url').read_text(), 'http://127.0.0.1:4181')
            self.assertFalse((self.root / 'state/server.pid').exists())

    def test_rapid_reconfiguration_backups_never_collide(self):
        with patch.object(configure.time, 'strftime', return_value='fixed-clock'):
            for _ in range(4):
                self.run_config()
        self.assertEqual(len(list((self.work / '.ai/ogh-skills').glob('*.ogh-backup-*'))), 3)

    def test_installer_folder_plans_and_interactive_edit(self):
        for script in ('setup.sh', 'sandbox.sh'):
            path = str(ROOT / 'scripts' / script)
            plan_home = self.root / ('plan-' + script)
            plan_home.mkdir()
            result = subprocess.run(['bash', path, '--folder', 'My Mercato', '--plan'],
                                    capture_output=True, text=True, check=True, timeout=10,
                                    env=dict(os.environ, HOME=str(plan_home)))
            self.assertEqual(list(plan_home.iterdir()), [])
            self.assertIn('/Projects/My Mercato', result.stdout)
            self.assertIn('📂', result.stdout)
            result = subprocess.run(['bash', path, '--interactive', '--plan'], input='\nFlight Work\n',
                                    capture_output=True, text=True, check=True, timeout=10)
            self.assertIn('/Projects/Flight Work', result.stdout)
            result = subprocess.run(['bash', path, '--interactive', '--plan'], input='',
                                    capture_output=True, text=True, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('cancelled before installation', result.stderr)
            for args in (['--folder'], ['--folder', '--plan']):
                result = subprocess.run(['bash', path, *args], capture_output=True, text=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('Missing --folder', result.stderr)
        invalid = subprocess.run(['bash', str(ROOT / 'scripts/sandbox.sh'), '--folder', '../outside', '--plan'],
                                 capture_output=True, text=True, timeout=10)
        self.assertNotEqual(invalid.returncode, 0)

    def test_repository_prompt_default_url_and_validation(self):
        for script in ('setup.sh', 'sandbox.sh'):
            path = str(ROOT / 'scripts' / script)
            for value in ('acme/widget', 'https://github.com/acme/widget.git'):
                result = subprocess.run(['bash', path, '--repo', value, '--no-prompt', '--plan'],
                                        capture_output=True, text=True, check=True, timeout=10)
                self.assertIn('Selected repository: acme/widget', result.stdout)
                self.assertIn('/Projects/widget-offline', result.stdout)
            for reply, expected in (('\n\n', 'open-mercato/open-mercato'), ('acme/widget\n\n', 'acme/widget')):
                result = subprocess.run(['bash', path, '--interactive', '--plan'], input=reply,
                                        capture_output=True, text=True, check=True, timeout=10)
                self.assertIn('GitHub repository (Enter', result.stdout)
                self.assertIn('Selected repository: ' + expected, result.stdout)
            for args in (['--repo'], ['--repo', '--plan'], ['--repo', 'bad;repo', '--plan']):
                result = subprocess.run(['bash', path, *args], capture_output=True, text=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)

    def test_selected_repository_is_retained_in_manifest_and_helper(self):
        configure.configure(self.work, self.skills, ROOT, 'acme/widget')
        self.assertEqual(json.loads((self.work / '.ogh-workspace.json').read_text())['repository'], 'acme/widget')
        cache = str(self.root / 'cache')
        import io
        response = io.BytesIO(json.dumps({'status': 'ok', 'outgoing_queue': True, 'data_dir': cache}).encode())
        with patch.dict(os.environ, {'HOME': str(self.root), 'OGH_DATA_DIR': cache}):
            with patch('urllib.request.urlopen', return_value=response):
                finish.finish(self.work, self.root / 'state', 4181)
        self.assertIn('export OGH_REPO=acme/widget', (self.root / '.local/bin/ogh-work').read_text())

    def test_sandbox_folder_reaches_bootstrap_and_reentry(self):
        # Exercise the whole launcher without virtualization, downloads or host writes.
        bindir = self.root / '.microsandbox/bin'
        bindir.mkdir(parents=True)
        log = self.root / 'msb.jsonl'
        msb = bindir / 'msb'
        msb.write_text('#!' + os.sys.executable + '\nimport json,sys\n'
                       + 'with open(' + repr(str(log)) + ', "a") as f: f.write(json.dumps(sys.argv[1:]) + "\\n")\n'
                       + 'sys.exit(1 if sys.argv[1] == "inspect" else 0)\n')
        msb.chmod(0o755)
        opener = bindir / 'open'
        opener.write_text('#!/bin/sh\nexit 0\n')
        opener.chmod(0o755)
        subprocess.run(['bash', str(ROOT / 'scripts/sandbox.sh'), '--repo', 'acme/widget', '--folder', 'My Mercato'],
                       env=dict(os.environ, HOME=str(self.root)), capture_output=True, text=True, check=True, timeout=15)
        calls = [json.loads(line) for line in log.read_text().splitlines()]
        bootstrap = next(call for call in calls if '--folder' in call)
        self.assertEqual(bootstrap[bootstrap.index('--folder') + 1], '/root/Projects/My Mercato')
        self.assertIn('--no-prompt', bootstrap)
        self.assertEqual(bootstrap[bootstrap.index('--repo') + 1], 'acme/widget')
        helper = (self.root / '.local/bin/ogh-sandbox').read_text()
        self.assertIn("--env 'OGH_WORKSPACE=/root/Projects/My Mercato'", helper)
        self.assertIn('finish.py "$OGH_WORKSPACE"', helper)
        subprocess.run(['bash', '-n', str(self.root / '.local/bin/ogh-sandbox')], check=True, timeout=10)

    def test_shell_syntax(self):
        for script in ('setup.sh', 'sandbox.sh'):
            subprocess.run(['bash', '-n', str(ROOT / 'scripts' / script)], check=True, timeout=10)
        text = (ROOT / 'scripts/sandbox.sh').read_text()
        self.assertIn('127.0.0.1:$OGH_SANDBOX_PORT:4173', text)
        self.assertIn('server.url', text)
        self.assertNotIn('--replace', text)
        self.assertNotIn('$HOME:/root', text)

    def test_foreign_listener_falls_back_to_actual_url(self):
        import socket
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0)); sock.listen()
            port = sock.getsockname()[1]
            cache = self.root / 'cache'
            env = dict(os.environ, HOME=str(self.root), OGH_DATA_DIR=str(cache),
                       PATH=str(ROOT / 'target/debug') + os.pathsep + os.environ['PATH'],
                       OGH_WEB_DIR=str(ROOT / 'web/dist'))
            state = self.root / 'state'
            try:
                result = subprocess.run(['python3', str(ROOT / 'scripts/setup/finish.py'),
                                         str(self.work), str(state), str(port)],
                                        env=env, capture_output=True, text=True, timeout=20)
                self.assertEqual(result.returncode, 0, result.stderr)
                url = (state / 'server.url').read_text()
                self.assertNotEqual(url, f'http://127.0.0.1:{port}')
                with finish.urllib.request.urlopen(url + '/api/health', timeout=2) as response:
                    self.assertEqual(json.load(response)['data_dir'], str(cache))
            finally:
                if (state / 'server.pid').exists():
                    os.kill(int((state / 'server.pid').read_text()), 15)


if __name__ == '__main__':
    unittest.main()
