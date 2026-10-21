<p align="center">
  <img src="app-icon.svg" width="96" height="96" alt="Tuntas logo">
</p>

<h1 align="center">Tuntas</h1>

<p align="center">
  The Windows uninstaller that finishes the job.<br>
  <a href="https://narr07.github.io/tuntas/"><strong>Website and docs</strong></a> ·
  <a href="https://github.com/narr07/tuntas/releases/latest"><strong>Download</strong></a>
</p>

---

*Tuntas* is Indonesian for "done, to the very end". It runs a program's own uninstaller, waits until it has really finished, checks the program is gone, and then shows you the folders, shortcuts and registry keys it left behind. You decide what goes, and everything it removes can be brought back.

## Features

- **Installed Programs**: uninstall one program or several in a row, then clean up the leftovers.
- **Forced Uninstall**: clean up programs whose uninstaller is broken or already gone.
- **Windows Apps**: remove Microsoft Store apps.
- **Startup Manager**: switch startup items on and off, the same way Task Manager does.
- **Junk Files**: clear temporary files, crash dumps and browser caches.
- **Hunter Mode**: see which installed program a running process belongs to.

## Built to be safe

- Leftovers only match when their name *is* the program name. Shared folders such as `AppData\Microsoft` are never matched.
- Only items that clearly belong to the program are ticked by default.
- A deletion guard refuses drive roots, Windows, Program Files roots, user profiles and their known folders, and any parent of those.
- Files go to the Recycle Bin, registry keys are exported to `.reg` files first, and a System Restore point can be created before uninstalling.
- Files held open by another program are never forced. Tuntas names the program and offers to close it, or deletes the files at the next restart.

Read more in [How Tuntas protects you](https://narr07.github.io/tuntas/safety/how-tuntas-protects-you).

## Install

Download the latest `Tuntas_*_x64-setup.exe` from [Releases](https://github.com/narr07/tuntas/releases/latest). Windows 10 and 11, 64-bit.

The installer is not code-signed yet, so Windows SmartScreen may show "Windows protected your PC". Click **More info**, then **Run anyway**. Every release is built by [GitHub Actions](.github/workflows/release.yml) from this repository.

## Development

Requirements: Windows, [Bun](https://bun.sh), [Rust](https://rustup.rs) (MSVC) and the Microsoft C++ Build Tools.

```bash
bun install
bun run desktop:dev      # run the app
bun run lint             # ESLint
bun run typecheck        # vue-tsc
bun run test:rust        # Rust unit tests
bun run desktop:build    # build the installers
```

The app is built with [Tauri 2](https://tauri.app), [Nuxt 4](https://nuxt.com) and [Nuxt UI](https://ui.nuxt.com), starting from the [Nuxtor](https://github.com/NicolaSpadari/nuxtor) template. The documentation site in [`docs/`](docs) uses the [Nuxt UI Docs template](https://github.com/nuxt-ui-templates/docs).

```text
app/                 Nuxt app (UI)
src-tauri/src/
  commands.rs        Tauri commands
  safety.rs          deletion guard
  win.rs             Win32 wrappers
docs/                website and documentation (GitHub Pages)
```

To publish a release, bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, then push a tag such as `v0.1.0`.

## License

[MIT](LICENSE)
