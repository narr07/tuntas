---
seo:
  title: Tuntas, the Windows uninstaller that finishes the job
  description: Tuntas runs a program's own uninstaller, then finds and removes the folders, shortcuts and registry keys it left behind. Free and open source.
---

::u-page-hero{class="dark:bg-gradient-to-b from-neutral-900 to-neutral-950"}
---
orientation: horizontal
---
#top
:hero-background

#headline
  :::u-badge
  ---
  label: Free and open source for Windows
  variant: subtle
  ---
  :::

#title
Uninstall programs. [Completely.]{.text-primary}

#description
*Tuntas* is Indonesian for "done, to the very end". It runs a program's own uninstaller, waits until it has really finished, then shows you the folders, shortcuts and registry keys it left behind. You decide what goes, and everything it removes can be brought back.

#links
  :::u-button
  ---
  to: '#download'
  size: xl
  icon: i-lucide-download
  ---
  Download for Windows
  :::

  :::u-button
  ---
  to: /getting-started
  size: xl
  color: neutral
  variant: outline
  trailing-icon: i-lucide-arrow-right
  ---
  Read the guide
  :::

#default
  :download-card
::

::u-page-section{class="dark:bg-neutral-950"}
#title
See it in action

#description
Real screens from Tuntas on a Windows 11 PC.

#default
  :screenshot-carousel
::

::u-page-section{class="dark:bg-neutral-950"}
#title
Cleans up properly, without breaking anything

#description
Most uninstallers leave something behind. Aggressive cleaners delete too much. Tuntas is built to be thorough and careful at the same time.

#features
  :::u-page-feature
  ---
  icon: i-lucide-scan-search
  to: /features/leftovers
  ---
  #title
  Leftover scan

  #description
  Finds folders in AppData, ProgramData, Program Files and the Start Menu, plus registry keys, that carry the program's exact name.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-shield-check
  to: /safety/how-tuntas-protects-you
  ---
  #title
  Deletion guard

  #description
  Windows, Program Files, your user folders and shared folders such as AppData\Microsoft can never be deleted, on any drive.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-undo-2
  to: /safety/undo
  ---
  #title
  Everything is reversible

  #description
  Files go to the Recycle Bin, registry keys are saved as .reg files first, and a restore point can be made before each uninstall.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-file-lock
  to: /features/files-in-use
  ---
  #title
  Files in use, handled

  #description
  If another program keeps a file open, Tuntas tells you which one and offers to close it, or to delete the file at the next restart.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-hourglass
  to: /features/uninstall
  ---
  #title
  Waits for the real uninstaller

  #description
  Many uninstallers hand over to a hidden copy of themselves. Tuntas waits for that copy too, and checks the program is gone before cleaning.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-layout-grid
  to: /features/windows-apps
  ---
  #title
  More than uninstalling

  #description
  Remove Store apps, control what starts with Windows, clear temporary files and browser caches, and find which program a process belongs to.
  :::
::

::u-page-section{class="dark:bg-gradient-to-b from-neutral-950 to-neutral-900"}
  :::u-page-c-t-a
  ---
  links:
    - label: Download Tuntas
      to: '#download'
      icon: i-lucide-download
    - label: View the source
      to: 'https://github.com/narr07/tuntas'
      target: _blank
      variant: subtle
      icon: i-simple-icons-github
  title: Your PC, without the clutter
  description: Tuntas is free, has no ads and sends nothing anywhere. The full source code is on GitHub under the MIT license.
  class: dark:bg-neutral-950
  ---

  :stars-bg
  :::
::
