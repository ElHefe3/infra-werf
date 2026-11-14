import type { Contribution, ExtensionCatalogue, ExtensionPoint } from "./contracts.gen.ts";

// Extension points the interface renders (S12/F18). The Rust core owns the
// catalogue: IDs, titles, order, owners and declared capabilities. The
// interface only supplies a renderer for each contribution it is given.
export type UiPoint = Extract<ExtensionPoint, "workspace_tab" | "server_badge" | "settings_section" | "app_settings_section">;

export type Bound<R> = { contribution: Contribution; renderer: R };

export class ExtensionBindingError extends Error {
  constructor(message: string) { super(message); this.name = "ExtensionBindingError"; }
}

/** Contributions at one point in display order (order, then ID). */
export function contributionsAt(catalogue: ExtensionCatalogue, point: ExtensionPoint): Contribution[] {
  return catalogue.contributions.filter(entry => entry.point === point)
    .sort((a, b) => a.order - b.order || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
}

/**
 * Pairs each registered contribution at a point with its renderer.
 * Nothing is rendered that the core did not register, and a registered
 * contribution without a renderer is an error rather than a silent gap.
 */
export function bind<R>(catalogue: ExtensionCatalogue, point: UiPoint, renderers: Readonly<Record<string, R>>): Bound<R>[] {
  const entries = contributionsAt(catalogue, point);
  const seen = new Set<string>();
  for (const entry of entries) {
    if (seen.has(entry.id)) throw new ExtensionBindingError(`Extension ${entry.id} is registered twice.`);
    seen.add(entry.id);
    if (!entry.capabilities.length) throw new ExtensionBindingError(`Extension ${entry.id} declares no capabilities.`);
    if (!Object.hasOwn(renderers, entry.id)) throw new ExtensionBindingError(`Extension ${entry.id} has no interface renderer.`);
  }
  for (const id of Object.keys(renderers)) {
    if (!seen.has(id)) throw new ExtensionBindingError(`Renderer ${id} is not a registered ${point} extension.`);
  }
  return entries.map(contribution => ({ contribution, renderer: renderers[contribution.id] }));
}
