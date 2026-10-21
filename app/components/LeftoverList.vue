<script setup lang="ts">
import type { LeftoverItem } from '~/composables/useUninstallerBackend';
import { confidenceLabel, formatBytes, isRiskyLeftover } from '~/composables/useUninstallerBackend';

const props = defineProps<{
	leftovers: LeftoverItem[];
	selectedIds: Set<string>;
	isScanning: boolean;
	hasScanned: boolean;
	isDeleting: boolean;
}>();

const emit = defineEmits<{
	(e: 'toggle', id: string): void;
	(e: 'select-all', value: boolean): void;
	(e: 'request-delete'): void;
	(e: 'rescan'): void;
}>();

const kindIcon: Record<LeftoverItem['kind'], string> = {
	folder: 'i-lucide-folder',
	file: 'i-lucide-file',
	registry: 'i-lucide-database'
};

const selectedBytes = computed(() => props.leftovers
	.filter(item => props.selectedIds.has(item.id))
	.reduce((sum, item) => sum + item.sizeBytes, 0));

const allState = computed<boolean | 'indeterminate'>(() => {
	if (props.selectedIds.size === 0) {
		return false;
	}
	return props.selectedIds.size === props.leftovers.length ? true : 'indeterminate';
});

const isRisky = isRiskyLeftover;
</script>

<template>
	<UCard :ui="{ header: 'px-4 py-3 sm:px-4', body: 'p-0 sm:p-0', footer: 'px-4 py-3 sm:px-4' }">
		<template #header>
			<div class="flex items-center justify-between gap-2">
				<div class="flex items-center gap-2">
					<h3 class="text-sm font-semibold text-highlighted">
						Leftovers
					</h3>
					<UBadge
						v-if="hasScanned"
						:label="String(leftovers.length)"
						color="neutral"
						variant="subtle"
					/>
				</div>
				<UButton
					label="Scan again"
					icon="i-lucide-refresh-cw"
					color="neutral"
					variant="ghost"
					size="sm"
					:loading="isScanning"
					:disabled="isDeleting"
					@click="emit('rescan')"
				/>
			</div>
		</template>

		<UEmpty
			v-if="isScanning"
			loading
			variant="naked"
			size="sm"
			title="Scanning for leftovers"
			description="Checking AppData, ProgramData, the Start Menu and the Software registry keys."
		/>

		<UEmpty
			v-else-if="hasScanned && leftovers.length === 0"
			icon="i-lucide-circle-check"
			variant="naked"
			size="sm"
			title="Nothing left behind"
			description="The uninstaller cleaned up after itself."
		/>

		<ul
			v-else-if="leftovers.length > 0"
			class="max-h-80 overflow-y-auto divide-y divide-default"
		>
			<li
				v-for="item in leftovers"
				:key="item.id"
				class="flex items-start gap-3 px-4 py-3"
			>
				<UCheckbox
					:model-value="selectedIds.has(item.id)"
					:aria-label="`Select ${item.path}`"
					class="mt-0.5"
					@update:model-value="emit('toggle', item.id)"
				/>
				<UIcon
					:name="kindIcon[item.kind]"
					class="size-4 mt-0.5 shrink-0 text-muted"
				/>
				<div class="min-w-0 flex-1 space-y-1">
					<p class="font-mono text-xs text-highlighted break-all">
						{{ item.path }}
					</p>
					<p class="text-xs text-muted">
						{{ item.reason }}
					</p>
					<div class="flex flex-wrap items-center gap-1.5">
						<UBadge
							:label="confidenceLabel[item.confidence]"
							:color="isRisky(item) ? 'warning' : 'neutral'"
							:icon="isRisky(item) ? 'i-lucide-triangle-alert' : undefined"
							variant="subtle"
							size="sm"
						/>
						<span
							v-if="item.kind !== 'registry'"
							class="text-xs text-muted tabular-nums"
						>
							{{ formatBytes(item.sizeBytes) }}
						</span>
					</div>
				</div>
			</li>
		</ul>

		<template
			v-if="!isScanning && leftovers.length > 0"
			#footer
		>
			<div class="flex flex-wrap items-center justify-between gap-2">
				<UCheckbox
					:model-value="allState"
					:label="`${selectedIds.size} of ${leftovers.length} selected, ${formatBytes(selectedBytes)}`"
					@update:model-value="(v) => emit('select-all', v === true)"
				/>
				<UButton
					label="Delete selected"
					icon="i-lucide-trash-2"
					color="error"
					:disabled="selectedIds.size === 0"
					:loading="isDeleting"
					@click="emit('request-delete')"
				/>
			</div>
		</template>
	</UCard>
</template>
