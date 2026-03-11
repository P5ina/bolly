type ToastKind = "error" | "info" | "success";

interface Toast {
	id: number;
	kind: ToastKind;
	message: string;
}

let nextId = 0;
let toasts = $state<Toast[]>([]);

export function getToasts() {
	return {
		get list() {
			return toasts;
		},
		push(kind: ToastKind, message: string, duration = 4000) {
			const id = nextId++;
			toasts = [...toasts, { id, kind, message }];
			setTimeout(() => {
				toasts = toasts.filter((t) => t.id !== id);
			}, duration);
		},
		error(message: string) {
			this.push("error", message, 5000);
		},
		info(message: string) {
			this.push("info", message);
		},
		success(message: string) {
			this.push("success", message, 3000);
		},
		dismiss(id: number) {
			toasts = toasts.filter((t) => t.id !== id);
		},
	};
}
