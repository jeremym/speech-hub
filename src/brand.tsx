import { Logo as KitLogo } from "@overtone-foundry/app-kit";

/* Speech Hub's own mark: a speaker with two sound waves, on the kit's 24 × 24 glyph grid. The kit
   ships no mark; this is the one thing about the look that is Speech Hub's alone. */
export const MARK = [
  "M4 9.5h3.5L12 5.5v13l-4.5-4H4z",
  "M15.5 9.5a3.5 3.5 0 0 1 0 5",
  "M18.5 7a7 7 0 0 1 0 10",
] as const;

export function Logo({ size = 28, word = true }: { size?: number; word?: boolean }) {
  return <KitLogo mark={MARK} name="Speech Hub" size={size} word={word} />;
}
