const IMAGE_EXTS = ["jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "ico"];
const VIDEO_EXTS = ["mp4", "webm", "mov", "ogg"];
const AUDIO_EXTS = ["mp3", "wav", "ogg", "m4a", "flac", "aac"];
const PDF_EXTS = ["pdf"];

export type FileType = "image" | "video" | "audio" | "pdf" | "other";

export interface ViewerFile {
	url: string;
	name: string;
	type: FileType;
}

export function detectFileType(name: string): FileType {
	// Strip query params and hash before extracting extension
	const clean = name.split("?")[0].split("#")[0];
	const ext = clean.split(".").pop()?.toLowerCase() ?? "";
	if (IMAGE_EXTS.includes(ext)) return "image";
	if (VIDEO_EXTS.includes(ext)) return "video";
	if (AUDIO_EXTS.includes(ext)) return "audio";
	if (PDF_EXTS.includes(ext)) return "pdf";
	return "other";
}

function detectFileTypeFromMime(mime: string): FileType {
	if (mime.startsWith("image/")) return "image";
	if (mime.startsWith("video/")) return "video";
	if (mime.startsWith("audio/")) return "audio";
	if (mime === "application/pdf") return "pdf";
	return "other";
}

let current = $state<ViewerFile | null>(null);

export async function openFile(url: string, name: string) {
	let type = detectFileType(name);

	// If extension-based detection fails, probe Content-Type via HEAD
	if (type === "other") {
		try {
			const res = await fetch(url, { method: "HEAD" });
			const ct = res.headers.get("content-type")?.split(";")[0]?.trim() ?? "";
			type = detectFileTypeFromMime(ct);
		} catch { /* fall through */ }
	}

	current = { url, name, type };
}

export function closeFile() {
	current = null;
}

export function getViewerFile(): ViewerFile | null {
	return current;
}
