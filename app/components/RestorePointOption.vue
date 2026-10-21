<script setup lang="ts">
import { useElevation } from '~/composables/useUninstallerBackend';

const model = defineModel<boolean>({ default: true });
const { elevated } = useElevation();
</script>

<template>
	<UCheckbox
		:model-value="model && Boolean(elevated)"
		:disabled="!elevated"
		label="Create a System Restore point first"
		:description="elevated
			? 'Lets you roll Windows back if something goes wrong. Windows makes at most one per day.'
			: 'Needs administrator rights. Use Restart as administrator in the sidebar.'"
		@update:model-value="(value) => model = value === true"
	/>
</template>
