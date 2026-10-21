<script setup lang="ts">
import { formatKb } from '~/composables/useUninstallerBackend';

const props = defineProps<{
	totalApps: number;
	visibleApps: number;
	totalDiskSizeKb: number;
	x64AppsCount: number;
}>();

const stats = computed(() => [
	{
		label: 'Programs',
		icon: 'i-lucide-package',
		value: String(props.totalApps),
		hint: props.visibleApps === props.totalApps ? 'All shown' : `${props.visibleApps} match the filter`
	},
	{
		label: 'Disk use',
		icon: 'i-lucide-hard-drive',
		value: formatKb(props.totalDiskSizeKb),
		hint: 'Reported by installers'
	},
	{
		label: '64-bit',
		icon: 'i-lucide-cpu',
		value: String(props.x64AppsCount),
		hint: `${props.totalApps - props.x64AppsCount} other`
	}
]);
</script>

<template>
	<div class="grid grid-cols-1 sm:grid-cols-3 gap-3 sm:gap-px">
		<UPageCard
			v-for="stat in stats"
			:key="stat.label"
			:icon="stat.icon"
			:title="stat.label"
			variant="subtle"
			:ui="{
				container: 'gap-y-1 p-3 sm:p-4',
				wrapper: 'items-start',
				leading: 'p-2 rounded-full bg-primary/10 ring ring-inset ring-primary/25',
				leadingIcon: 'size-4 text-primary',
				title: 'font-normal text-muted text-xs uppercase'
			}"
			class="sm:rounded-none sm:first:rounded-s-lg sm:last:rounded-e-lg"
		>
			<p class="text-xl font-semibold text-highlighted tabular-nums">
				{{ stat.value }}
			</p>
			<p class="text-xs text-muted">
				{{ stat.hint }}
			</p>
		</UPageCard>
	</div>
</template>
