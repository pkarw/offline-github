#!/usr/bin/env python3
"""Install project-scoped skills and the explicit ogh tracker; never alias gh."""
import argparse
import json
import shutil
import time
import uuid
from pathlib import Path


def configure(workspace, skills, source, repository='open-mercato/open-mercato'):
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
    instruction = '''
## Offline workspace transport (installed by ogh setup)

Read `.ai/trackers/offline-github.md` before tracker operations. Use `ogh`, never
`gh`, for this workspace's issue/PR/cache/queue operations, even when examples
below show `gh`. Use the descriptor's REST alternative for GraphQL commit counts.

Supported writes ARE available offline: queue comments with `ogh issue comment
NUMBER --body-file FILE` (or `ogh pr comment`); queue label changes with
`ogh issue edit NUMBER --add-label NAME --remove-label OTHER` (or `ogh pr edit`);
create repository labels with `ogh label create NAME --color aabbcc`. Issue/PR
creation, PR reviews and branch publication also have dedicated ogh commands.
Translate supported REST write examples into these high-level commands; `ogh api`
is read-only. Pass `-R owner/repo` explicitly. Do not say comments/labels are
impossible merely because this workspace uses the offline provider.

Queuing saves a durable local intention; it does not update GitHub immediately.
When online and publication is authorized, run `ogh outbox sync --dry-run`, inspect
the outgoing payloads, then `ogh outbox sync --yes --plan-token TOKEN` using the
returned plan_token. The UI calls this Push. `ogh sync` is incoming Pull only.
Use `ogh outbox report --format json` or `--format md` for recovery and audit.
Only an applied event with a GitHub receipt confirms publication. Preserve and
report pending/conflict/uncertain events; never blindly repost uncertain writes.

Do not fall back to real gh for unsupported operations. Missing or stale cached
data is a gap. A mandatory live claim, atomic lock, merge or permission gate still
requires live verification and must stop offline; queued comments/labels cannot
satisfy it. Native local Git branches, commits and explicitly requested local
preparation/coding/tests remain available.

'''
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
    metadata = {'repository': repository, 'skills': installed,
                'tracker': 'offline-github', 'managedBy': 'ogh-setup-v1'}
    write(workspace / '.ogh-workspace.json', json.dumps(metadata, indent=2) + '\n')
    print(f'Configured {len(installed)} project skills for ogh (Claude, Codex, pi).')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('workspace')
    parser.add_argument('skills')
    parser.add_argument('source')
    parser.add_argument('--repo', default='open-mercato/open-mercato')
    args = parser.parse_args()
    configure(args.workspace, args.skills, args.source, args.repo)
