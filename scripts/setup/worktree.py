#!/usr/bin/env python3
"""Create a checkout, replacing only its own missing, unlocked registration."""
import os
from pathlib import Path
import subprocess
import sys


def create(mirror, destination, revision):
    destination = Path(destination).absolute()
    if os.path.lexists(destination):
        raise RuntimeError(f'Preserving existing destination: {destination}; choose another --folder')
    command = ['git', '--git-dir=' + str(mirror), 'worktree']
    listing = subprocess.check_output(command + ['list', '--porcelain', '-z'], timeout=30).decode()
    stale = False
    for record in listing.split('\0\0'):
        fields = record.split('\0')
        if not fields[0].startswith('worktree '):
            continue
        registered = fields[0][len('worktree '):]
        if os.path.realpath(registered) != os.path.realpath(destination):
            continue
        if any(field == 'locked' or field.startswith('locked ') for field in fields):
            raise RuntimeError(f'Worktree registration is locked: {destination}. Restore its folder/mount or explicitly unlock it before retrying.')
        stale = True
    destination.parent.mkdir(parents=True, exist_ok=True)
    args = command + ['add', '--detach']
    if stale:
        # One force replaces a missing registration, but Git still refuses a locked one.
        # Never prune unrelated worktrees or delete checkout files.
        print(f'🔧 Recovering missing worktree registration: {destination}', flush=True)
        args += ['--force']
    # Recheck immediately before invoking Git; Git also refuses existing nonempty folders.
    if os.path.lexists(destination):
        raise RuntimeError(f'Destination appeared during setup; preserving it: {destination}')
    subprocess.run(args + ['--', str(destination), revision], check=True, timeout=300)


if __name__ == '__main__':
    try:
        create(*sys.argv[1:])
    except (RuntimeError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error))
