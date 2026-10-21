<script setup lang="ts">
const { app } = useRuntimeConfig()

const shots = [
  { file: '01-installed-programs.png', title: 'Installed Programs', description: 'Every program Windows knows about, with size, architecture and install date.' },
  { file: '02-program-details.png', title: 'Program details', description: 'Install folder, registry key and the uninstall options for the selected program.' },
  { file: '03-forced-uninstall.png', title: 'Forced Uninstall', description: 'Clean up programs whose uninstaller is broken or already gone.' },
  { file: '04-windows-apps.png', title: 'Windows Apps', description: 'Remove Microsoft Store apps; protected system packages stay hidden.' },
  { file: '05-startup-manager.png', title: 'Startup Manager', description: 'Switch startup items on and off the same way Task Manager does.' },
  { file: '06-junk-files.png', title: 'Junk Files', description: 'Temporary files and browser caches, with a warning when a browser is still open.' },
  { file: '07-hunter-mode.png', title: 'Hunter Mode', description: 'See which installed program each running process belongs to.' },
  { file: '08-installed-programs-light.png', title: 'Light mode', description: 'Every page also comes in a light theme.' }
].map(shot => ({ ...shot, src: `${app.baseURL}screenshots/${shot.file}` }))
</script>

<template>
  <UCarousel
    v-slot="{ item }"
    :items="shots"
    loop
    arrows
    dots
    :autoplay="{ delay: 4500, stopOnMouseEnter: true, stopOnInteraction: false }"
    :prev="{ color: 'neutral', variant: 'subtle' }"
    :next="{ color: 'neutral', variant: 'subtle' }"
    :ui="{ prev: 'sm:-start-12', next: 'sm:-end-12', dots: '-bottom-8' }"
    class="mx-auto mb-12 w-full max-w-5xl"
  >
    <figure class="space-y-3">
      <img
        :src="item.src"
        :alt="`Tuntas ${item.title} screen`"
        width="1600"
        height="1000"
        loading="lazy"
        class="w-full rounded-xl shadow-lg ring ring-default"
      >
      <figcaption class="text-center">
        <p class="font-semibold text-highlighted">
          {{ item.title }}
        </p>
        <p class="text-sm text-muted">
          {{ item.description }}
        </p>
      </figcaption>
    </figure>
  </UCarousel>
</template>
