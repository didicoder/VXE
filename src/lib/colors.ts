import type { ColorKey } from '../types/battery';

/** 六色强调色板（暗色/gnome；light 变体见 UI-SPEC §1.5）。 */
export const COLOR_HEX: Record<ColorKey, string> = {
  blue: 'var(--pal-blue)',
  red: 'var(--pal-red)',
  green: 'var(--pal-green)',
  purple: 'var(--pal-purple)',
  amber: 'var(--pal-amber)',
  cyan: 'var(--pal-cyan)',
};

export function colorHex(key: string | null | undefined, fallback = 'var(--accent)'): string {
  if (key && key in COLOR_HEX) return COLOR_HEX[key as ColorKey];
  return fallback;
}
