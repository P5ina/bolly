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
	const ext = name.split(".").pop()?.toLowerCase() ?? "";
	if (IMAGE_EXTS.includes(ext)) return "image";
	if (VIDEO_EXTS.includes(ext)) return "video";
	if (AUDIO_EXTS.includes(ext)) return "audio";
	if (PDF_EXTS.includes(ext)) return "pdf";
	return "other";
}

let current = $state<ViewerFile | null>(null);

export function openFile(url: string, name: string) {
	current = { url, name, type: detectFileType(name) };
}

export function closeFile() {
	current = null;
}

export function getViewerFile(): ViewerFile | null {
	return current;
}
