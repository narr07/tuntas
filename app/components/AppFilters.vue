<script setup lang="ts">
import type { TabsItem } from '@nuxt/ui';

type ArchFilter = 'all' | 'x64' | 'x86' | 'User';
type SortKey = 'name' | 'size' | 'date';

const searchQuery = defineModel<string>('searchQuery', { required: true });
const archFilter = defineModel<ArchFilter>('archFilter', { required: true });
const sortBy = defineModel<SortKey>('sortBy', { required: true });
const showSystemComponents = defineModel<boolean>('showSystemComponents', { required: true });

const searchInput = useTemplateRef('searchInput');

defineShortcuts({
	meta_f: {
		usingInput: true,
		handler: () => searchInput.value?.inputRef?.focus()
	}
});

const archItems = [
	{ label: 'All', value: 'all' },
	{ label: 'x64', value: 'x64' },
	{ label: 'x86', value: 'x86' },
	{ label: 'Per-user', value: 'User' }
] satisfies TabsItem[];

const sortItems = [
	{ label: 'Name (A to Z)', value: 'name' },
	{ label: 'Largest first', value: 'size' },
	{ label: 'Newest first', value: 'date' }
];
</script>

<template>
	<div class="flex w-full flex-wrap items-center gap-2 py-2">
		<UInput
			ref="searchInput"
			v-model="searchQuery"
			icon="i-lucide-search"
			placeholder="Name, publisher or folder"
			aria-label="Search installed programs"
			class="w-full sm:w-64"
		>
			<template #trailing>
				<div class="flex items-center gap-0.5">
					<UKbd value="meta" />
					<UKbd value="f" />
				</div>
			</template>
		</UInput>

		<UTabs
			v-model="archFilter"
			:items="archItems"
			:content="false"
			size="xs"
			aria-label="Filter by architecture"
		/>

		<div class="flex items-center gap-3 sm:ms-auto">
			<USwitch
				v-model="showSystemComponents"
				label="System components"
				size="sm"
			/>
			<USelect
				v-model="sortBy"
				:items="sortItems"
				aria-label="Sort programs"
				class="w-36"
			/>
		</div>
	</div>
</template>
