/** Join class names, skipping falsy parts. */
export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

/**
 * Focus ring shared by every interactive element (DESIGN.md §2):
 * 2 px `accent/default` ring with a 2 px offset, keyboard focus only.
 */
export const focusRing =
  "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-solid focus-visible:outline-accent-default";
