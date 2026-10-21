<script setup lang="ts">
interface ReleaseAsset {
  name: string
  browser_download_url: string
  size: number
}

interface Release {
  tag_name: string
  html_url: string
  published_at: string
  assets: ReleaseAsset[]
}

const { repo } = useAppConfig()
const apiUrl = repo.replace('https://github.com/', 'https://api.github.com/repos/') + '/releases/latest'

// Fetched in the browser so the page always offers the newest build without a redeploy.
const { data: release, status } = useFetch<Release>(apiUrl, {
  server: false,
  lazy: true,
  headers: { Accept: 'application/vnd.github+json' }
})

const installer = computed(() => release.value?.assets.find(a => /setup\.exe$/i.test(a.name)) ?? release.value?.assets.find(a => /\.exe$/i.test(a.name)))
const msi = computed(() => release.value?.assets.find(a => /\.msi$/i.test(a.name)))
const published = computed(() => release.value ? new Date(release.value.published_at).toLocaleDateString(undefined, { dateStyle: 'medium' }) : '')

function size(bytes: number) {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}
</script>

<template>
  <UCard
    id="download"
    class="w-full scroll-mt-24"
    :ui="{ body: 'space-y-5 p-6 sm:p-8' }"
  >
    <div class="flex items-center gap-4">
      <AppLogo class="h-12" />
    </div>

    <div
      v-if="status === 'pending' || status === 'idle'"
      class="space-y-3"
    >
      <USkeleton class="h-5 w-40" />
      <USkeleton class="h-12 w-full" />
      <USkeleton class="h-10 w-2/3" />
    </div>

    <template v-else-if="release && installer">
      <div class="flex flex-wrap items-center gap-2 text-sm text-muted">
        <UBadge
          :label="release.tag_name"
          color="primary"
          variant="subtle"
        />
        <span>Released {{ published }}</span>
      </div>

      <div class="flex flex-col gap-3">
        <UButton
          :to="installer.browser_download_url"
          icon="i-lucide-download"
          size="xl"
          block
          :label="`Download for Windows (${size(installer.size)})`"
        />
        <div class="flex flex-wrap gap-2">
          <UButton
            v-if="msi"
            :to="msi.browser_download_url"
            icon="i-lucide-package"
            color="neutral"
            variant="outline"
            :label="`MSI package (${size(msi.size)})`"
          />
          <UButton
            :to="release.html_url"
            target="_blank"
            color="neutral"
            variant="ghost"
            trailing-icon="i-lucide-arrow-up-right"
            label="Release notes"
          />
        </div>
      </div>

      <p class="text-xs text-muted">
        Windows 10 and 11, 64-bit. The installer is not code-signed yet, so SmartScreen may ask you to confirm.
        <ULink
          to="/getting-started/installation"
          class="text-primary"
        >
          Why?
        </ULink>
      </p>
    </template>

    <template v-else>
      <UAlert
        color="neutral"
        variant="subtle"
        icon="i-lucide-hourglass"
        title="The first release is on its way"
        description="No installer has been published yet. Watch the repository to get notified, or build Tuntas from source today."
      />
      <div class="flex flex-wrap gap-2">
        <UButton
          :to="`${repo}/releases`"
          target="_blank"
          icon="i-simple-icons-github"
          label="Releases on GitHub"
        />
        <UButton
          to="/development/build-from-source"
          color="neutral"
          variant="outline"
          icon="i-lucide-hammer"
          label="Build from source"
        />
      </div>
    </template>
  </UCard>
</template>
