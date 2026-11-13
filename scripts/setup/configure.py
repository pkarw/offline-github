#!/usr/bin/env python3
"""Install project-scoped skills and the explicit ogh tracker; never alias gh."""
import argparse
import json
import shutil
import time
import uuid
from pathlib import Path


def configure(workspace, skills, source):
    workspace, skills, source = map(Path, (workspace, skills, source))
    ai = workspace / '.ai'
    ai.mkdir(exist_ok=True)
    stamp = time.strftime('%Y%m%d-%H%M%S') + '-' + uuid.uuid4().hex[:8]

    def write(path, text):
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists() and path.read_text() != text:
            shutil.copy2(path, path.with_name(path.name + '.ogh-backup-' + stamp))
        path.write_text(text)

    config_path = ai / 'agentic.config.json'
    config = json.loads(config_path.read_text()) if config_path.exists() else {
        'version': 1, 'baseBranch': 'auto', 'validation': {'commands': ['yarn typecheck', 'yarn test', 'yarn build']},
        'labels': {'enabled': False}, 'qaGate': True,
        'paths': {'runs': '.ai/runs', 'analysis': '.ai/analysis', 'specs': '.ai/specs',
                  'dataModel': '.ai/data-model', 'prototypes': '.ai/prototypes', 'scripts': '.ai/scripts', 'qa': '.ai/qa'},
    }
    config['tracker'] = 'offline-github'
    write(config_path, json.dumps(config, indent=2) + '\n')
    write(ai / 'trackers/offline-github.md', (source / 'docs/trackers/offline-github.md').read_text())
    instruction = '''\n## Offline workspace transport (installed by ogh setup)\n\nRead `.ai/trackers/offline-github.md` before tracker operations. Use `ogh`, never\n`gh`, for this workspace's issue/PR/cache/queue operations, even when examples\nbelow show `gh`. Use the descriptor's REST alternative for GraphQL commit counts.\nDo not fall back to online GitHub calls for unsupported operations. Missing or\nstale cached data is a gap; queued writes are intentions, not live receipts.\nA mandatory live claim, merge, permission or publication gate must stop offline.\nContinue only the explicitly requested local preparation/coding/test work.\nNative local Git is available. Outgoing publication requires reviewed Sync.\n\n'''
    installed = []
    for skill in sorted((skills / 'skills').iterdir()):
        if not (skill / 'SKILL.md').is_file():
            continue
        destination = ai / 'ogh-skills' / skill.name
        if destination.exists():
            # These are our managed copies, but retain any user edits on refresh.
            backup = destination.with_name(destination.name + '.ogh-backup-' + stamp)
            destination.rename(backup)
        shutil.copytree(skill, destination)
        text = (destination / 'SKILL.md').read_text()
        if text.startswith('---\n'):
            end = text.find('\n---', 4)
            if end < 0:
                raise ValueError(f'Malformed skill frontmatter: {skill.name}')
            end = text.index('\n', end + 1) + 1
            text = text[:end] + instruction + text[end:]
        else:
            text = instruction + text
        (destination / 'SKILL.md').write_text(text)
        for directory in ('.agents/skills', '.claude/skills', '.codex/skills', '.pi/skills'):
            link = workspace / directory / skill.name
            link.parent.mkdir(parents=True, exist_ok=True)
            if link.is_symlink() and link.resolve() == destination.resolve():
                continue
            if link.exists() or link.is_symlink():
                # Preserve repo-provided skills/overrides; configure their tracker centrally.
                continue
            link.symlink_to(destination.resolve(), target_is_directory=True)
        installed.append(skill.name)
    metadata = {'repository': 'open-mercato/open-mercato', 'skills': installed,
                'tracker': 'offline-github', 'managedBy': 'ogh-setup-v1'}
    write(workspace / '.ogh-workspace.json', json.dumps(metadata, indent=2) + '\n')
    print(f'Configured {len(installed)} project skills for ogh (Claude, Codex, pi).')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('workspace')
    parser.add_argument('skills')
    parser.add_argument('source')
    args = parser.parse_args()
    configure(args.workspace, args.skills, args.source)
