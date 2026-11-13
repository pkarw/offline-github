# Microsandbox alternative

[← Getting started](../README.md) · [All Docs](README.md)

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/pkarw/offline-github/main/scripts/sandbox.sh)
```

📂 **Before installing anything**, the launcher shows the guest project folder,
persistent disk, host helper and browser URL. It prompts for a folder name;
press **Enter** to keep `open-mercato-offline`, or type your own.

```bash
# Choose the guest folder directly (no folder prompt):
bash <(curl -fsSL https://raw.githubusercontent.com/pkarw/offline-github/main/scripts/sandbox.sh) --folder "My Mercato"

# Preview paths only — no downloads, installs or VM changes:
bash <(curl -fsSL https://raw.githubusercontent.com/pkarw/offline-github/main/scripts/sandbox.sh) --folder "My Mercato" --plan
```

Folders live at `/root/Projects/NAME` **inside the VM**. Names may contain
letters, digits, spaces, dots, dashes and underscores, and must start with a
letter or digit. `-d` is shorthand for `--folder`; `OGH_SANDBOX_FOLDER` sets the
default. Use `--no-prompt` for automation or `--interactive` to force the prompt.
The generated `ogh-sandbox` helper remembers the selected guest folder.

This installs Microsandbox if absent, checks its host prerequisites, creates a
persistent **local** microVM, installs the same ogh/Open Mercato/skills setup
inside it, and opens the project shell. Sign into GitHub **inside the guest**
when asked. Install/sign into your preferred agent there too.

- Browser app: http://127.0.0.1:4183
- Return later: `~/.local/bin/ogh-sandbox`
- Stop: `msb stop ogh-mercato`
- Project inside the guest: `/root/Projects/open-mercato-offline`
- Persistent disk volume: `ogh-mercato-home`, mounted at `/root` (40 GB)
- VM: 4 CPUs, 8 GB RAM, 32 GB root disk; Node 22 Debian base image.

The host home, GitHub credentials and agent credentials are **not mounted or
forwarded**. Guest login is separate. Public internet access is enabled for
installation and explicit sync; this is isolation, not a network air gap. The
public port binds host **127.0.0.1** only. A guest TCP relay forwards port 4173
to ogh's protected loopback listener on 4174, preserving HTTP Host and Origin
checks. `ogh` itself keeps its localhost-only binding.

Stop/start preserves the VM filesystem; named volumes persist independently.
**Do not remove the volume**: it contains repositories, unpublished branches,
credentials and the outgoing ledger. Volume persistence is not a backup. Never
attach the same writable disk to a second VM simultaneously. The launcher does
not use replace/remove flags. Reentry restarts the app/relay after a VM restart.

Microsandbox must support local virtualization on your host (`msb doctor`);
macOS needs Apple Silicon. First setup downloads the image, installs packages,
builds ogh and syncs GitHub, so allow time and disk space. Running the actual
Open Mercato ERP/database stack requires its own setup and extra port mappings;
this launcher exposes only the Offline GitHub browser app. Agent API access
still needs internet even when repository data is cached.

Use `OGH_SANDBOX_NAME` and `OGH_SANDBOX_PORT` to select a different VM/host port
on first setup. Existing unrecognized VMs or changed port mappings are preserved;
choose a new name instead. Reuse the generated helper for that instance. Stop the VM when
finished; leaving its shell does not discard or stop it.

Official references: [Microsandbox](https://microsandbox.dev/),
[CLI sandbox commands](https://docs.microsandbox.dev/cli/sandbox-commands),
[persistent volumes](https://docs.microsandbox.dev/sandboxes/volumes).
