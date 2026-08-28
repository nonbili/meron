// Tiny className joiner, mirroring the `clsx` helper in ~/repo/nora's lib/utils.
// Drops falsy parts and space-joins the rest. We have no tailwind-merge, so a
// consumer's `className` is appended last and is meant to be additive (layout,
// positioning) — pick a variant rather than overriding colors.
export const clsx = (...classes: Array<string | false | null | undefined>): string => classes.filter(Boolean).join(' ')
