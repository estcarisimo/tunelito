# Platform support

| | Debian | macOS | Windows 10 1809+ / 11 |
| --- | --- | --- | --- |
| What they open | `.deb` or AppImage | drag the app out of the `.dmg` | `tunelito.exe` |
| First launch | one admin prompt | one admin prompt | one UAC prompt |
| Executed in CI | file layout and command text. The Linux job does not install packages. | no | `windows-latest` builds the exe and runs setup, then deletes the account |
| Installer | `.deb` | `.dmg` | optional NSIS. The exe is the file to download. |

Windows on ARM has no pinned `cloudflared`. macOS Remote Login is boot-persistent in the plan and is not executed. Signing secrets are optional; empty means unsigned. See [SIGNING.md](SIGNING.md).
