import { fetchInstances, AuthError } from "$lib/api/client.js";
import type { InstanceSummary } from "$lib/api/types.js";

let instances = $state<InstanceSummary[]>([]);
let loading = $state(true);

export function getInstances() {
	return {
		get list() {
			return instances;
		},
		get loading() {
			return loading;
		},
		async refresh() {
			loading = true;
			try {
				instances = await fetchInstances();
			} catch (e) {
				instances = [];
				if (e instanceof AuthError) throw e;
			} finally {
				loading = false;
			}
		},
		upsert(instance: InstanceSummary) {
			const idx = instances.findIndex((i) => i.slug === instance.slug);
			if (idx >= 0) {
				instances[idx] = instance;
			} else {
				instances = [...instances, instance];
			}
		},
	};
}
