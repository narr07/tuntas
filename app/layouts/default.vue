<script setup lang="ts">
import type { NavigationMenuItem } from '@nuxt/ui';

import { errorText, useElevation, useUninstallerBackend } from '~/composables/useUninstallerBackend';

const { isTauriRuntime, restartAsAdmin } = useUninstallerBackend();
const { elevated, refresh: refreshElevation } = useElevation();
const toast = useToast();
const route = useRoute();
const isRestarting = ref(false);

async function relaunchElevated() {
	isRestarting.value = true;
	try {
		await restartAsAdmin();
	}
	catch (err: unknown) {
		toast.add({ title: 'Still running without admin rights', description: errorText(err), color: 'warning' });
	}
	finally {
		isRestarting.value = false;
	}
}

onMounted(refreshElevation);
const sidebarOpen = ref(false);

const links = [
	[
		{ label: 'Uninstall', type: 'label' },
		{ label: 'Installed Programs', icon: 'i-lucide-package', to: '/' },
		{ label: 'Forced Uninstall', icon: 'i-lucide-shield-alert', to: '/forced' },
		{ label: 'Windows Apps', icon: 'i-lucide-layout-grid', to: '/windows-apps' }
	],
	[
		{ label: 'Clean up', type: 'label' },
		{ label: 'Startup Manager', icon: 'i-lucide-power', to: '/startup' },
		{ label: 'Junk Files', icon: 'i-lucide-trash-2', to: '/cleaner' }
	],
	[
		{ label: 'Inspect', type: 'label' },
		{ label: 'Hunter Mode', icon: 'i-lucide-crosshair', to: '/hunter' }
	]
] satisfies NavigationMenuItem[][];

const searchGroups = computed(() => [
	{
		id: 'pages',
		label: 'Go to',
		items: links.flat().filter(item => item.type !== 'label')
	}
]);

watch(() => route.fullPath, () => {
	sidebarOpen.value = false;
});
</script>

<template>
	<UDashboardGroup unit="rem">
		<UDashboardSidebar
			id="default"
			v-model:open="sidebarOpen"
			collapsible
			resizable
			:default-size="16"
			:min-size="14"
			:max-size="20"
			class="bg-elevated/25"
			:ui="{ footer: 'lg:border-t lg:border-default' }"
		>
			<template #header="{ collapsed }">
				<NuxtLink
					to="/"
					class="flex items-center gap-2 min-w-0"
					aria-label="Tuntas home"
				>
					<span class="size-7 shrink-0">
						<AppLogo />
					</span>
					<span
						v-if="!collapsed"
						class="text-sm font-semibold text-highlighted truncate"
					>
						Tuntas
					</span>
				</NuxtLink>
			</template>

			<template #default="{ collapsed }">
				<UDashboardSearchButton
					:collapsed="collapsed"
					class="bg-transparent ring-default"
				/>

				<UNavigationMenu
					v-for="(group, index) in links"
					:key="index"
					:collapsed="collapsed"
					:items="group"
					orientation="vertical"
					tooltip
				/>
			</template>

			<template #footer="{ collapsed }">
				<div class="flex w-full flex-col gap-2">
					<UAlert
						v-if="isTauriRuntime && elevated === false && !collapsed"
						color="warning"
						variant="subtle"
						icon="i-lucide-shield-alert"
						title="Limited mode"
						description="Machine-wide programs, startup items and system folders need administrator rights."
						:actions="[{ label: 'Restart as administrator', color: 'warning', variant: 'solid', size: 'xs', loading: isRestarting, onClick: relaunchElevated }]"
						:ui="{ title: 'text-xs', description: 'text-xs' }"
					/>
					<UTooltip
						v-else-if="isTauriRuntime && elevated === false"
						text="Restart as administrator"
					>
						<UButton
							icon="i-lucide-shield-alert"
							color="warning"
							variant="subtle"
							aria-label="Restart as administrator"
							:loading="isRestarting"
							@click="relaunchElevated"
						/>
					</UTooltip>
					<div class="flex w-full items-center justify-between gap-2">
						<UTooltip
							:text="!isTauriRuntime ? 'Running in a browser tab: system actions are disabled' : (elevated ? 'Running as administrator' : 'Running without administrator rights')"
						>
							<UBadge
								:color="isTauriRuntime ? 'success' : 'warning'"
								variant="subtle"
								:icon="isTauriRuntime ? 'i-lucide-monitor-check' : 'i-lucide-globe'"
								:label="collapsed ? undefined : (!isTauriRuntime ? 'Browser preview' : (elevated ? 'Administrator' : 'Standard user'))"
							/>
						</UTooltip>
						<UColorModeButton
							v-if="!collapsed"
							color="neutral"
							variant="ghost"
						/>
					</div>
				</div>
			</template>
		</UDashboardSidebar>

		<UDashboardSearch :groups="searchGroups" />

		<slot />
	</UDashboardGroup>
</template>
