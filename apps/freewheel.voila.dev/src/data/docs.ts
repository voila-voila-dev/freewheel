// The docs' navigation, in reading order: previous and next follow it. Pages come from the
// repository's docs/ folder (src/lib/docs-loader.ts); a page missing here is still built.
import type { NavGroup } from "@voila.dev/site/lib/types";

export const docsNav: NavGroup[] = [
	{
		title: "Guide",
		items: [
			{ href: "/docs/guide", label: "Start here" },
			{
				href: "/docs/guide/combining-connections",
				label: "Combining connections",
			},
			{ href: "/docs/guide/quality-and-bitrate", label: "Quality and bitrate" },
			{ href: "/docs/guide/delay", label: "Delay" },
			{ href: "/docs/guide/losing-the-network", label: "Losing the network" },
			{
				href: "/docs/guide/full-quality-recording",
				label: "Full-quality recording",
			},
			{ href: "/docs/guide/data-plans", label: "Data plans" },
			{ href: "/docs/guide/glossary", label: "Glossary" },
		],
	},
	{
		title: "Build",
		items: [
			{ href: "/docs/technical", label: "Overview" },
			{ href: "/docs/technical/architecture", label: "Architecture" },
			{ href: "/docs/technical/code-style", label: "Code style" },
			{ href: "/docs/technical/testing", label: "Testing" },
			{ href: "/docs/technical/link-emulator", label: "Link emulator" },
			{ href: "/docs/technical/benchmarks", label: "Benchmarks" },
		],
	},
	{
		title: "Protocol",
		items: [
			{ href: "/docs/technical/protocol/srtla", label: "SRTLA v2" },
			{
				href: "/docs/technical/protocol/freewheel",
				label: "freewheel transport",
			},
		],
	},
	{
		title: "Project",
		items: [
			{ href: "/docs/technical/roadmap", label: "Roadmap" },
			{ href: "/docs/technical/decisions", label: "Decisions" },
		],
	},
	{
		title: "Research",
		items: [
			{ href: "/docs/technical/research/00-synthesis", label: "Synthesis" },
			{
				href: "/docs/technical/research/01-state-of-the-art",
				label: "State of the art",
			},
			{
				href: "/docs/technical/research/02-transport-mechanisms",
				label: "Transport",
			},
			{
				href: "/docs/technical/research/03-media-aware-priorities",
				label: "Media awareness",
			},
			{
				href: "/docs/technical/research/04-security-operations",
				label: "Security and operations",
			},
			{
				href: "/docs/technical/research/05-embedded-performance",
				label: "Embedded performance",
			},
		],
	},
];
