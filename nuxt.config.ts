export default defineNuxtConfig({
	modules: [
		'@nuxt/ui',
		'@vueuse/nuxt',
		'@nuxt/eslint'
	],
	ssr: false,
	devtools: {
		enabled: false
	},
	app: {
		head: {
			title: 'Tuntas',
			charset: 'utf-8',
			viewport: 'width=device-width, initial-scale=1',
			meta: [
				{
					name: 'description',
					content: 'Uninstall Windows programs completely: native uninstall, leftover scan, Store apps, startup manager and junk cleaner.'
				},
				{
					name: 'format-detection',
					content: 'no'
				}
			]
		}
	},
	css: [
		'@/assets/css/main.css'
	],
	colorMode: {
		preference: 'dark',
		fallback: 'dark',
		classSuffix: ''
	},
	experimental: {
		typedPages: true
	},
	compatibilityDate: '2025-07-01',
	vite: {
		clearScreen: false,
		envPrefix: ['VITE_', 'TAURI_'],
		server: {
			strictPort: true,
			hmr: {
				protocol: 'ws',
				host: '0.0.0.0',
				port: 3001
			},
			watch: {
				ignored: ['**/src-tauri/**']
			}
		}
	},
	telemetry: false,
	eslint: {
		config: {
			stylistic: {
				indent: 'tab',
				quotes: 'single',
				semi: true,
				commaDangle: 'never'
			}
		}
	},
	icon: {
		// The desktop app runs offline: bundle every icon into the client instead of fetching from the Iconify API.
		provider: 'none',
		fallbackToApi: false,
		clientBundle: {
			scan: true,
			sizeLimitKb: 512
		}
	}
});
