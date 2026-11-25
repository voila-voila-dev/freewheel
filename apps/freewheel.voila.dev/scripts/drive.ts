// The drive demo's data: the three recorded LTE uplinks the `trace-drive` bench replays
// (labs/srtla-capture/scenarios/trace-drive.sh), as kbit/s per second. The traces are Mahimahi's
// (GPL-3.0): downloaded here, never committed, like the lab does. Output: src/data/drive.json.
import { mkdir } from "node:fs/promises";

const SOURCE =
	"https://raw.githubusercontent.com/ravinet/mahimahi/master/traces";
const CACHE = new URL("../node_modules/.cache/traces/", import.meta.url);
const OUT = new URL("../src/data/drive.json", import.meta.url);
/** One delivery opportunity is one 1500-byte packet. */
const KBIT_PER_OPPORTUNITY = 12;
const SECONDS = 100;

/** Same files and windows as the bench scenario. */
const LINKS = [
	{ name: "Verizon", trace: "Verizon-LTE-driving.up", from: 240 },
	{ name: "AT&T", trace: "ATT-LTE-driving.up", from: 210 },
	{ name: "AT&T 2016", trace: "ATT-LTE-driving-2016.up", from: 0 },
];

async function trace(file: string): Promise<string> {
	const cached = Bun.file(new URL(file, CACHE));
	if (await cached.exists()) return cached.text();
	const response = await fetch(`${SOURCE}/${file}`);
	if (!response.ok) throw new Error(`${file}: HTTP ${response.status}`);
	const text = await response.text();
	await mkdir(CACHE, { recursive: true });
	await Bun.write(cached, text);
	return text;
}

/** Capacity per second, in kbit/s, from the second `from` on. */
function perSecond(text: string, from: number): number[] {
	const counts = new Array<number>(SECONDS).fill(0);
	for (const line of text.split("\n")) {
		const second = Math.floor(Number(line) / 1000) - from;
		if (line && second >= 0 && second < SECONDS) counts[second]++;
	}
	return counts.map((n) => n * KBIT_PER_OPPORTUNITY);
}

const links = await Promise.all(
	LINKS.map(async (l) => ({
		name: l.name,
		kbps: perSecond(await trace(l.trace), l.from),
	})),
);
await Bun.write(OUT, `${JSON.stringify({ seconds: SECONDS, links })}\n`);
console.log(
	`drive: ${links.map((l) => `${l.name} ${Math.round(l.kbps.reduce((a, b) => a + b, 0) / SECONDS)} kbit/s`).join(", ")}`,
);
