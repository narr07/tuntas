# Tuntas design notes

## Product

Tuntas removes Windows programs completely: it runs the program's own uninstaller, then finds what was left behind. The people using it are not always technical, and a wrong deletion can break their PC, so every design decision leans toward **showing evidence and asking** over acting silently.

## Principles

1. **Nothing is deleted without a reviewed list.** Every destructive action ends in a modal that lists exactly what will go.
2. **Default selections are conservative.** Only `exact-path` items are ticked automatically. Anything matched by name is shown with a warning badge and left unticked.
3. **Every deletion is reversible.** Files go to the Recycle Bin, registry keys are exported to `.reg` first, and a restore point can be created before uninstalling.
4. **Say what really happened.** Reports count removed, skipped and locked items separately. No decorative status cards.

## UI

- Built entirely from Nuxt UI components (`UDashboardGroup`, `UDashboardPanel`, `UTable`, `UModal`, `UAlert`, ...). Native elements are used only for layout wrappers or when Nuxt UI has no equivalent.
- Colours come from `app.config.ts`: `primary: emerald`, `neutral: mist`. Red (`error`) is reserved for destructive actions, amber (`warning`) for items that need a second look.
- Paths, registry keys and PIDs use the monospace font so they can be read character by character.
- Dark mode is the default; light mode is fully supported through Nuxt UI tokens.

## Confidence levels

| Level | Meaning | Ticked by default |
| --- | --- | --- |
| `exact-path` | The registered install folder (holding the uninstaller or named after the program) or the program's own uninstall key | Yes |
| `name-match` | A folder or key whose name *equals* the program name | Folders yes, registry no |
| `publisher-match` | `Publisher\Program` inside a shared root | Folders yes, registry no |
| `registered-path` | The registered install folder, but its name does not fit the program | No |

## Architecture

- `app/`: Nuxt 4 SPA (`ssr: false`), statically generated and loaded by Tauri.
- `app/composables/useUninstallerBackend.ts`: the only place that calls `invoke`.
- `src-tauri/src/commands.rs`: Tauri commands. All run off the main thread (`#[tauri::command(async)]`).
- `src-tauri/src/safety.rs`: the deletion guard. Every filesystem delete goes through `check_deletable`.
- `src-tauri/src/win.rs`: Win32 wrappers (processes, elevation, Recycle Bin, Restart Manager, delete-on-reboot). All `unsafe` code lives here.
