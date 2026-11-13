#!/usr/bin/env python3
"""Regression checks for preserving local work and the offline skills contract."""
import importlib.util
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('configure', ROOT / 'scripts/setup/configure.py')
configure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(configure)


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

    def test_shell_syntax(self):
        for script in ('setup.sh', 'sandbox.sh'):
            subprocess.run(['bash', '-n', str(ROOT / 'scripts' / script)], check=True, timeout=10)
        text = (ROOT / 'scripts/sandbox.sh').read_text()
        self.assertIn('127.0.0.1:$OGH_SANDBOX_PORT:4173', text)
        self.assertIn('TCP:127.0.0.1:4174', text)
        self.assertNotIn('--replace', text)
        self.assertNotIn('$HOME:/root', text)

    def test_no_server_success_on_foreign_listener(self):
        # The launcher must not declare a foreign service to be an ogh server.
        import socket
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            sock.listen()
            port = sock.getsockname()[1]
            env = dict(os.environ, HOME=str(self.root))
            result = subprocess.run(['python3', str(ROOT / 'scripts/setup/finish.py'),
                                     str(self.work), str(self.root / 'state'), str(port)],
                                    env=env, capture_output=True, text=True, timeout=8)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Address already in use', result.stderr)
            self.assertFalse((self.root / 'state/server.pid').exists())


if __name__ == '__main__':
    unittest.main()
