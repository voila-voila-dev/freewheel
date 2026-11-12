import {
	createRootRoute,
	createRoute,
	createRouter,
	Outlet,
} from "@tanstack/react-router";

const rootRoute = createRootRoute({ component: () => <Outlet /> });

const statusRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/",
	component: () => <h1>freewheel</h1>,
});

export const router = createRouter({
	routeTree: rootRoute.addChildren([statusRoute]),
});

declare module "@tanstack/react-router" {
	interface Register {
		router: typeof router;
	}
}
