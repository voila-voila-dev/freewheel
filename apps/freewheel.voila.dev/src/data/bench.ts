// Numbers from the freewheel repo, bench/README.md and docs/technical/decisions.md.
// Update them when a new CI bench run lands.

export const repo = "https://github.com/voila-voila-dev/freewheel";
export const benchUrl = `${repo}/blob/main/bench/README.md`;

export type Sender = { name: string; detail: string; ours?: boolean };

/** `trace-drive`: three recorded LTE uplinks from a moving car, ~2,700 frames, 3.4 Mbit/s stream. */
export const traceDrive = {
	totalFrames: 2700,
	senders: [
		{
			name: "freewheel",
			detail: "own transport",
			ours: true,
			framesLost: 532,
			deliveredMbps: 2.9,
		},
		{
			name: "SRTLA",
			detail: "upstream BELABOX",
			framesLost: 1754,
			deliveredMbps: 1.8,
		},
		{
			name: "irlserver",
			detail: "srtla_send, Rust",
			framesLost: 1956,
			deliveredMbps: 1.7,
		},
	],
};

/** Share of traffic a restored link gets back 15–25 s after it returns (even split = 50 %). */
export const restoredLink = [
	{ name: "freewheel", value: "48–50 %", ours: true },
	{ name: "irlserver", value: "47 %" },
	{ name: "SRTLA", value: "12 %" },
];

/** Compatibility mode against upstream (M1, 2026-11-12). */
export const compatibility = [
	["Delivered at 50 Mbit/s", "100 %", "100 %"],
	["12 Mbit/s over 2 × 8 Mbit/s", "100 %", "100 %"],
	["Sender CPU, 50 Mbit/s for 20 s", "1210 ms", "1090 ms"],
	["Sender peak memory", "1.8 MB", "1.55 MB"],
	["Characterisation tests passed", "40 / 40", "40 / 40"],
];

/** Lab scenarios: what each one emulates. */
export const scenarios = [
	[
		"trace-drive",
		"Three real LTE uplinks recorded in a car. Where transports differ.",
	],
	["real-ride", "A ride: links fade, drop and come back."],
	["trace-city", "Real LTE uplinks recorded in a city."],
	["all-links-drop", "Every link dies for 6 s at once."],
	["link-drop", "One link dies, then comes back."],
	["burst-loss", "Bursts of loss on one link."],
	["varying-bandwidth", "Capacity swings up and down."],
];

export type Status = "done" | "now" | "next";
/** Plain-language roadmap for the home page; technical milestones live in the freewheel repo. */
export const roadmap: {
	id: string;
	name: string;
	text: string;
	status: Status;
}[] = [
	{
		id: "01",
		name: "Works with BELABOX",
		text: "Same protocol, same speed.",
		status: "done",
	},
	{
		id: "02",
		name: "Fewer drops",
		text: "Already 3× less video lost.",
		status: "now",
	},
	{
		id: "03",
		name: "Hosted server",
		text: "Nothing to install.",
		status: "next",
	},
	{
		id: "04",
		name: "First real demos",
		text: "On the road, raw.",
		status: "next",
	},
	{
		id: "05",
		name: "Full-quality recording",
		text: "Uploaded in the background.",
		status: "next",
	},
	{
		id: "06",
		name: "Phone app",
		text: "Networks, data, live.",
		status: "next",
	},
	{ id: "07", name: "The box", text: "Rain, heat, bumps.", status: "next" },
];
