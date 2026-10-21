// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  modules: [
    '@nuxt/eslint',
    '@nuxt/image',
    '@nuxt/ui',
    '@nuxt/content',
    'nuxt-llms'
  ],

  devtools: {
    enabled: true
  },

  app: {
    // GitHub Pages serves the site from https://narr07.github.io/tuntas/.
    // Nuxt lets NUXT_APP_BASE_URL=/ override this to preview it at the root.
    baseURL: '/tuntas/'
  },

  css: ['~/assets/css/main.css'],

  content: {
    build: {
      markdown: {
        toc: {
          searchDepth: 1
        }
      }
    },
    experimental: {
      sqliteConnector: 'native'
    }
  },

  experimental: {
    asyncContext: true
  },

  compatibilityDate: '2026-06-30',

  nitro: {
    prerender: {
      routes: ['/'],
      crawlLinks: true
    }
  },

  eslint: {
    config: {
      stylistic: {
        commaDangle: 'never',
        braceStyle: '1tbs'
      }
    }
  },

  llms: {
    domain: 'https://narr07.github.io/tuntas/',
    title: 'Tuntas',
    description: 'Open-source Windows uninstaller that removes programs completely, with leftover scanning, backups and a deletion guard.',
    sections: [
      {
        title: 'Getting Started',
        contentCollection: 'docs',
        contentFilters: [
          { field: 'path', operator: 'LIKE', value: '/getting-started%' }
        ]
      },
      {
        title: 'Features',
        contentCollection: 'docs',
        contentFilters: [
          { field: 'path', operator: 'LIKE', value: '/features%' }
        ]
      },
      {
        title: 'Safety',
        contentCollection: 'docs',
        contentFilters: [
          { field: 'path', operator: 'LIKE', value: '/safety%' }
        ]
      }
    ]
  }
})
