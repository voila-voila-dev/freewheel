import { defineCollection, z } from "astro:content";
import { docsLoader } from "./lib/docs-loader";

export const collections = {
	docs: defineCollection({
		loader: docsLoader(),
		schema: z.object({
			title: z.string(),
			description: z.string(),
			source: z.string(),
		}),
	}),
};
