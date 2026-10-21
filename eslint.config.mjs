import withNuxt from './.nuxt/eslint.config.mjs';

export default withNuxt(
	{
		rules: {
			'vue/multi-word-component-names': 'off',
			'no-console': 'off',
			// Typed `defineEmits` overloads read better one event per line.
			'@typescript-eslint/unified-signatures': 'off'
		}
	},
	{
		// Dashboard pages render several panels and modals side by side, as in the Nuxt UI dashboard template.
		files: ['app/pages/**/*.vue'],
		rules: {
			'vue/no-multiple-template-root': 'off'
		}
	}
);
