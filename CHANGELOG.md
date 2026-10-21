# Changelog

## 0.1.0 (unreleased)

First public preview.

### Safety
- Leftover folders and registry keys only match when their name *is* the program name (or `Publisher\Program`). Shared folders such as `AppData\Microsoft` or `AppData\Google` are never matched.
- Only leftovers that clearly belong to the program are ticked by default. Name matches and install folders whose name does not fit the program wait for the user.
- A deletion guard refuses drive roots, Windows, Program Files roots, every user profile and its known folders, and any parent of those, on every drive. Paths are normalised first, so `C:\Windows\..\Users` does not slip through.
- Registry keys are exported to `.reg` files in `%LOCALAPPDATA%\Tuntas\Backups` before they are deleted. Shared keys such as `SOFTWARE\Microsoft` are refused.
- Files and folders go to the Recycle Bin instead of being deleted outright.
- Optional System Restore point before uninstalling.

### Uninstalling
- Waits for the whole uninstaller, including the copies Inno Setup and NSIS start from `%TEMP%`, before looking for leftovers.
- Checks that the program is really gone. A cancelled uninstall no longer leads to its folder being removed.
- `MsiExec /I{...}` entries are turned into `/X{...}`, so MSI products uninstall instead of opening the repair wizard.
- Uninstall commands are started like Windows itself does: quoted and unquoted paths and `%VARIABLES%` all work.

### Files in use
- When a program holds a leftover open, Tuntas names it (via the Windows Restart Manager) and offers to close it and retry, force-close it, or delete the files at the next restart.

### Other
- Startup items are switched on and off the way Task Manager does it, so a disabled item can always be enabled again.
- Junk cleaner deletes file by file, reports how many files were in use, covers every browser profile and no longer empties Windows Prefetch.
- Store apps that Windows does not allow to be removed are marked as protected.
- All system calls run off the UI thread and no console windows flash.
- "Restart as administrator" button when Tuntas runs without admin rights.
