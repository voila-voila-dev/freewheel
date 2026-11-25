// What every page of freewheel.voila.dev shares: the links, the footer, the call to action.
import type { Link } from "@voila.dev/site/lib/types";
import { repo } from "./bench";

export const siteName = "freewheel";

export const links: Link[] = [
	{ href: "/#why", label: "Why" },
	{ href: "/docs", label: "Docs" },
	{ href: "/demos", label: "Demos" },
	{ href: "/benchmark", label: "Benchmark" },
	{ href: "/#roadmap", label: "Roadmap" },
];

export const footer = {
	note: "freewheel · MIT · by",
	by: { href: "https://voila.dev", label: "VOILA DEVELOPMENT" },
	email: "emilien@voila.dev",
	links: [
		{ href: "/docs", label: "Docs" },
		{ href: "/demos", label: "Demos" },
		{ href: "/benchmark", label: "Benchmark" },
		{ href: repo, label: "GitHub" },
		{ href: "https://voila.dev/legal", label: "Legal" },
	],
};

export const cta = {
	text: "Stream IRL? Tell us where it drops.",
	buttons: [
		{
			href: "mailto:emilien@voila.dev?subject=freewheel",
			label: "Get in touch",
		},
		{ href: repo, label: "Star on GitHub ↗" },
	],
};
