const repo = 'https://github.com/narr07/tuntas'

export default defineAppConfig({
  ui: {
    colors: {
      primary: 'emerald',
      neutral: 'slate'
    },
    footer: {
      slots: {
        root: 'border-t border-default',
        left: 'text-sm text-muted'
      }
    }
  },
  repo,
  seo: {
    siteName: 'Tuntas'
  },
  header: {
    title: '',
    to: '/',
    logo: {
      alt: '',
      light: '',
      dark: ''
    },
    search: true,
    colorMode: true,
    links: [{
      'icon': 'i-simple-icons-github',
      'to': repo,
      'target': '_blank',
      'aria-label': 'Tuntas on GitHub'
    }]
  },
  footer: {
    credits: `Tuntas is MIT licensed • © ${new Date().getFullYear()} narr07`,
    colorMode: false,
    links: [{
      'icon': 'i-simple-icons-github',
      'to': repo,
      'target': '_blank',
      'aria-label': 'Tuntas on GitHub'
    }]
  },
  toc: {
    title: 'On this page',
    bottom: {
      title: 'Project',
      edit: `${repo}/edit/main/docs/content`,
      links: [{
        icon: 'i-lucide-download',
        label: 'Latest release',
        to: `${repo}/releases/latest`,
        target: '_blank'
      }, {
        icon: 'i-lucide-bug',
        label: 'Report a problem',
        to: `${repo}/issues/new`,
        target: '_blank'
      }]
    }
  }
})
