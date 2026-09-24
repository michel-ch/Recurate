# Claude Design — Integration Spec

A drop-in design language for projects that should feel like Claude: warm, editorial, literary, quietly confident. The browser is treated as a printed page, not a dashboard.

This document works two ways:

- **As a project spec.** Commit it to the repo (e.g. `docs/claude-design.md`), wire up the tokens in [Integration](#integration), and treat the token reference as the single source of truth.
- **As an LLM prompt.** Paste it alongside the UI you want restyled — HTML, JSX, screenshots, or a design file — and the model returns a restyled version. See [Restyling an existing UI](#restyling-an-existing-ui).

When acting as the restyler, you are a senior product designer at Anthropic. Preserve structure, features, and information hierarchy. Change the *feel*, not the *function*. Make judgment calls — don't ask the reader to choose between two reasonable defaults; pick one, note it, move on.

> **Recurate additions.** Two derived tokens are used by the desktop handoff (`design_handoff_recurate/README.md`) and stay inside the warm spectrum: `--caution` (`#7A5A1E` light / `#D9B26A` dark) for "attention, not danger", and `--critical-soft` (`#F2DFD9` light / `rgba(208,122,102,.15)` dark) as the wash for rows about to be deleted. The app's row lists are data-dense and run at 11–13px sans per the Pragmatics clause below; prose returns to 14–16px.

---

## The feel

Imagine a well-printed book that happens to be interactive. The room is warm, the paper is cream, the ink is sepia rather than black. Somewhere on the shelf there's a single piece of terracotta pottery — that's the only saturated color in the room. Everything else recedes so the words can do the work.

Think editorial magazine, literary journal, museum wall text. Not dashboard, not SaaS, not neon.

---

## Principles

1. **Editorial over technical.** The reader should feel invited to read, not to click. Serif for what matters. Generous margins. Confident typography.
2. **Warm neutrals only.** No cool greys, no pure white, no pure black. Everything lives on a spectrum of cream, sand, clay, and ink. State this once and let the tokens enforce it.
3. **One accent, used like punctuation.** A single warm accent in the terracotta / burnt-sienna family marks the one thing you want the reader to do. If more than ~5% of the screen is accent-colored, it's too much.
4. **Breathing room is the design.** Whitespace isn't empty — it's structural. When in doubt, more. Trust the air.
5. **Hierarchy from weight and size, not decoration.** No heavy borders, no synthetic shadows, no gradient fills carrying meaning. A larger serif heading beats a smaller bold one. A hairline beats a 2px divider.
6. **Honest surfaces.** No glassmorphism, neumorphism, or fake depth. If there's a shadow, it's so subtle you'd miss it.
7. **Motion is a sigh, not a gesture.** Subtle, quick, easing out. Nothing bounces, spins, or flashes. UI acknowledges interaction without performing it.
8. **Quiet confidence.** Nothing on screen is trying to convince you it's exciting. The design trusts the content.

**Pragmatics.** This is a brand language, not a straitjacket. Genuinely data-dense surfaces — tables, logs, dense settings — may tighten spacing below the comfortable default and lean harder on the sans face. Bend deliberately, document the exception, and never reach for a cool grey or a heavy shadow to do it.

---

## Design tokens

These are the canonical values. Use them as written unless the project already has equivalents; if you adjust, stay inside the verbal descriptions above and never drift cool. Everything downstream references these tokens — components should contain no raw hex.

### Color — light

| Token | Value | Role |
|---|---|---|
| `--paper` | `#F6F3EC` | Page background — uncoated cream, never white |
| `--surface` | `#EFEADF` | Cards, panels — one half-shade deeper, a second sheet of paper |
| `--surface-sunk` | `#E7E1D4` | Wells, inputs, code blocks, track backgrounds |
| `--ink` | `#33302A` | Primary text — warm espresso, not black |
| `--ink-2` | `#6B6356` | Secondary text |
| `--ink-3` | `#978C7B` | Tertiary text, placeholders |
| `--line` | `#E2DBCE` | Hairline border — disappears on casual viewing |
| `--line-strong` | `#D6CDBC` | Border on hover, dividers |
| `--accent` | `#B3502F` | Links, focus ring, small icons, the anchor color |
| `--accent-fill` | `#A8492B` | Solid button background |
| `--accent-hover` | `#8F3D22` | Accent darkened on hover |
| `--accent-ink` | `#FBF8F1` | Paper-colored text on an accent fill |
| `--accent-soft` | `#F1E3D8` | Wash for chips, tags, active nav, user chat bubble |
| `--accent-soft-ink` | `#8A3C22` | Text on the soft wash |
| `--positive` | `#5C6B3F` | Success — warm olive, icons and small text only |
| `--critical` | `#9C3B2E` | Destructive — warm brick, icons and small text only |
| `--shadow` | `0 1px 2px rgba(54,46,36,.06)` | The only elevation, and even this is optional |

### Color — dark

Dark mode inverts the metaphor without losing the warmth: cream becomes warm charcoal, ink becomes bone. Still no cool greys, still no true black.

| Token | Value | Role |
|---|---|---|
| `--paper` | `#22201B` | Warm charcoal page |
| `--surface` | `#2A2823` | Cards, panels |
| `--surface-sunk` | `#1C1A16` | Wells, inputs, code |
| `--ink` | `#EAE4D6` | Primary text — bone |
| `--ink-2` | `#B3AB9A` | Secondary |
| `--ink-3` | `#847C6D` | Tertiary, placeholders |
| `--line` | `#38352D` | Hairline border |
| `--line-strong` | `#46423A` | Hover border, dividers |
| `--accent` | `#D98A66` | Links, focus, icons — lifts to glow on charcoal |
| `--accent-fill` | `#AE4F30` | Solid button background |
| `--accent-hover` | `#984229` | Hover |
| `--accent-ink` | `#FBF6EE` | Text on accent fill |
| `--accent-soft` | `rgba(205,117,81,.15)` | Wash for chips, active nav, user bubble |
| `--accent-soft-ink` | `#E7A98A` | Text on the soft wash |
| `--positive` | `#8A9A66` | Success |
| `--critical` | `#D07A66` | Destructive |
| `--shadow` | `0 1px 2px rgba(0,0,0,.25)` | Optional elevation |

> Terracotta is a mid-luminance hue, so it fights AA contrast with *both* black and white text. The fill tokens above are tuned so paper-colored text clears 4.5:1; don't lighten a text-bearing fill without re-checking. Use the brighter `--accent` for links, icons, and rings — not as a button background behind small text.

### Type

| Token | Value |
|---|---|
| `--font-serif` | `"Newsreader", Georgia, "Times New Roman", serif` |
| `--font-sans` | `"Inter", -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif` |
| `--font-mono` | `"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace` |
| `--display` | `44px` serif — hero |
| `--h1` | `34px` serif |
| `--h2` | `26px` serif |
| `--h3` | `21px` serif |
| `--text-lg` | `18px` sans |
| `--text-base` | `16px` sans — body |
| `--text-sm` | `14px` sans |
| `--text-xs` | `13px` sans — meta |
| `--leading-tight` | `1.15` — headings |
| `--leading-body` | `1.65` — prose |
| `--leading-snug` | `1.4` — UI labels |

Serif for display and major headings; humanist sans for body, UI controls, and anything small; mono only for code. The serif never goes bold and never goes below `--h3` — it carries weight through size and optical sizing, not stroke. Recommended pairing: **Newsreader** (transitional, optical sizes, a printed-book feel) or **Fraunces** (more ink-trap character) for display; **Inter** or another humanist sans for body. Swap freely; if nothing loads, fall back to Georgia and a humanist system sans.

### Spacing — 8pt grid, fine control at 4

| Token | Value |
|---|---|
| `--space-1` | `4px` |
| `--space-2` | `8px` |
| `--space-3` | `12px` |
| `--space-4` | `16px` |
| `--space-5` | `24px` |
| `--space-6` | `32px` |
| `--space-7` | `48px` |
| `--space-8` | `64px` |
| `--space-9` | `96px` |

Avoid 10, 14, 18, 22 and other off-grid values. Default to more space than feels necessary.

### Shape & motion

| Token | Value |
|---|---|
| `--radius-sm` | `6px` |
| `--radius` | `10px` |
| `--radius-lg` | `14px` |
| `--radius-pill` | `999px` |
| `--border` | `1px` |
| `--ease` | `cubic-bezier(.2,0,0,1)` — ease-out, no overshoot |
| `--dur-fast` | `150ms` |
| `--dur` | `200ms` |
| `--dur-slow` | `250ms` |

### Layout widths

| Token | Value | Role |
|---|---|---|
| `--measure` | `68ch` | Prose column (caps line length at ~65–75 chars) |
| `--w-app` | `1120px` | App / settings max width — a wide paragraph, not the viewport |
| `--w-read` | `720px` | Reading layouts — a book column |
| `--w-form` | `460px` | Forms — narrower still |

Content always has a maximum width. Large screens never sprawl full-bleed; page gutters scale with the viewport but stay generous so mobile is never cramped.

---

## Integration

### CSS custom properties (source of truth)

Drop this into the global stylesheet. Toggle dark mode by setting `data-theme="dark"` on `<html>` (or `:root`), driven by a stored preference that defaults to the OS setting.

```css
:root {
  color-scheme: light;

  --paper:#F6F3EC; --surface:#EFEADF; --surface-sunk:#E7E1D4;
  --ink:#33302A; --ink-2:#6B6356; --ink-3:#978C7B;
  --line:#E2DBCE; --line-strong:#D6CDBC;
  --accent:#B3502F; --accent-fill:#A8492B; --accent-hover:#8F3D22;
  --accent-ink:#FBF8F1; --accent-soft:#F1E3D8; --accent-soft-ink:#8A3C22;
  --positive:#5C6B3F; --critical:#9C3B2E;
  --shadow:0 1px 2px rgba(54,46,36,.06);

  --font-serif:"Newsreader",Georgia,"Times New Roman",serif;
  --font-sans:"Inter",-apple-system,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;
  --font-mono:"JetBrains Mono",ui-monospace,"SF Mono",Menlo,monospace;

  --display:44px; --h1:34px; --h2:26px; --h3:21px;
  --text-lg:18px; --text-base:16px; --text-sm:14px; --text-xs:13px;
  --leading-tight:1.15; --leading-body:1.65; --leading-snug:1.4;

  --space-1:4px; --space-2:8px; --space-3:12px; --space-4:16px; --space-5:24px;
  --space-6:32px; --space-7:48px; --space-8:64px; --space-9:96px;

  --radius-sm:6px; --radius:10px; --radius-lg:14px; --radius-pill:999px;
  --border:1px;
  --ease:cubic-bezier(.2,0,0,1); --dur-fast:150ms; --dur:200ms; --dur-slow:250ms;

  --measure:68ch; --w-app:1120px; --w-read:720px; --w-form:460px;
}

:root[data-theme="dark"] {
  color-scheme: dark;

  --paper:#22201B; --surface:#2A2823; --surface-sunk:#1C1A16;
  --ink:#EAE4D6; --ink-2:#B3AB9A; --ink-3:#847C6D;
  --line:#38352D; --line-strong:#46423A;
  --accent:#D98A66; --accent-fill:#AE4F30; --accent-hover:#984229;
  --accent-ink:#FBF6EE; --accent-soft:rgba(205,117,81,.15); --accent-soft-ink:#E7A98A;
  --positive:#8A9A66; --critical:#D07A66;
  --shadow:0 1px 2px rgba(0,0,0,.25);
}

body { background: var(--paper); color: var(--ink); font-family: var(--font-sans);
       font-size: var(--text-base); line-height: var(--leading-body); }
h1,h2,h3 { font-family: var(--font-serif); font-weight: 400; line-height: var(--leading-tight); }

@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation: none !important; transition: none !important; }
}
```

### Fonts

Load the pairing, or self-host for production. The serif is requested only up to weight 500 so it can't accidentally be set bold.

```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Newsreader:ital,opsz,wght@0,6..72,400;0,6..72,500;1,6..72,400&family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet">
```

### Tailwind

If the project uses Tailwind v4, map the CSS variables in `@theme` so utilities resolve to tokens rather than the default palette:

```css
@import "tailwindcss";

@theme {
  --color-paper: #F6F3EC;
  --color-surface: #EFEADF;
  --color-ink: #33302A;
  --color-ink-2: #6B6356;
  --color-line: #E2DBCE;
  --color-accent: #B3502F;
  --color-accent-soft: #F1E3D8;
  --font-serif: "Newsreader", Georgia, serif;
  --font-sans: "Inter", system-ui, sans-serif;
  --radius-md: 10px;
}
```

For Tailwind v3, set the same values under `theme.extend.colors`, `fontFamily`, `borderRadius`, and `spacing` in `tailwind.config.js`, and **disable or override** the default `indigo / sky / rose / slate` palettes so nobody can reach a cool color by accident.

---

## Components

Describe, don't decorate. All states resolve from tokens.

**Primary button.** Sans label. `--accent-fill` background, `--accent-ink` text, `--radius`, no shadow. Hover darkens the fill to `--accent-hover` — no lift, no scale. Active darkens slightly further. Disabled drops to ~45% opacity, no pointer.

**Secondary button.** Transparent, hairline `--line` border, `--ink` text. Hover fills with `--surface` and the border firms to `--line-strong`. Same radius and states.

**Text link.** `--accent`, no underline at rest, underline on hover. Nothing more.

**Input.** Sits on `--surface-sunk`, hairline border, `--radius-sm`. Focus shows a thin `--accent` outline with a paper-colored inset halo — never the heavy blue browser default. Error state tints the border toward `--critical` and adds a small message in the same color; never a filled red banner.

**Card.** `--surface` background, hairline `--line` border, generous padding (`--space-5`+), no shadow by default.

**Tag / chip.** `--accent-soft` wash, `--accent-soft-ink` text, sans, small and tight, `--radius-pill`.

**Navigation.** Quiet sidebar on desktop. The active item is distinguished by a soft `--accent-soft` wash behind the text — not a colored vertical bar, not an underline.

**Chat bubble.** The user message sits on an `--accent-soft` wash. The assistant reply has no bubble at all — it's just type on the page, like a letter.

**Icons.** Outlined, thin consistent stroke, single size per context. Never filled, never multicolor, never decorative.

---

## Motion

Quick, gentle, easing out — `--dur-fast` to `--dur-slow` for most transitions, `--ease` for all of them. No bounces, overshoots, or springs. No staggered entrances on load; content appears, it doesn't perform. Loading states are a single subtle pulse or shimmer, never a spinning beachball. Hover changes are minimal — a darkening, a border appearing — no scale, rotate, or shadow bloom. Always honor `prefers-reduced-motion`.

---

## Accessibility

A warm, low-contrast, sepia-on-cream palette is exactly the kind that quietly fails WCAG. Treat contrast as a gate, not an afterthought.

- Body text meets **4.5:1**; large text (≥24px, or ≥18.66px bold) and UI components/icons/borders meet **3:1**. The token pairs above are tuned to clear these — verify in context after any change.
- The terracotta fills are calibrated so paper-colored button labels pass 4.5:1. Don't lighten a text-bearing fill without re-checking, and don't put small text on the brighter `--accent`.
- Never signal state with color alone. Pair `--critical` / `--positive` with an icon or text label.
- Every interactive element gets a visible `:focus-visible` ring — the thin accent outline, not an outline you've removed for looks.
- Keep the prose measure at ~65–75 characters; long warm-grey lines are a readability tax.
- Respect `prefers-reduced-motion` and `prefers-color-scheme` (use the OS setting as the dark-mode default).

---

## Do not

- Introduce blue, violet, teal, emerald, or any cool saturated color.
- Use Tailwind defaults (indigo, sky, rose, slate, etc.) — override or replace them with tokens.
- Fill backgrounds with gradients. A hero *may* carry a near-imperceptible warm gradient; nothing else.
- Use glassmorphism, neumorphism, or any synthetic-depth effect.
- Add emoji to UI chrome. Emoji is fine in user content, never in navigation or labels.
- Bold a serif, or shrink it below `--h3`. Size does that job.
- Stack more than two elevation levels.
- Add badges, ribbons, or "NEW" pills unless genuinely load-bearing.
- Hardcode a hex value in a component. Everything routes through a token.

---

## Restyling an existing UI

1. **Audit.** List everything off-brand in the source: cool colors, heavy shadows, playful iconography, Tailwind defaults, cramped or off-grid spacing, display type in bold sans.
2. **Adopt tokens.** Use the [Design tokens](#design-tokens) as-is, or derive equivalents that stay inside the verbal descriptions. This is the source of truth; don't reinvent per component.
3. **Replace globally.** Swap color, font, radius, and shadow references across the codebase before touching structure.
4. **Normalize spacing.** Snap every off-grid value to the nearest `--space-*`.
5. **Rewrite typography.** Serif display, sans body. Remove extra serif weights. Tune tracking and line-height to feel editorial.
6. **Strip chrome.** Remove shadows, gradients, and decorative strokes. Replace with hairlines or nothing.
7. **Rebuild components.** Button, input, card, nav, chip — to the specs above, with all states.
8. **Check at multiple widths.** Mobile, tablet, small desktop, large desktop. Confirm breathing room holds and large screens don't sprawl.

---

## Definition of done

- [ ] No hardcoded colors, fonts, radii, or shadows in components — only tokens.
- [ ] No cool greys, pure white, pure black, or Tailwind-default colors anywhere.
- [ ] Light and dark modes both implemented, with full token parity.
- [ ] All text and UI contrast passes its WCAG target; nothing relies on color alone.
- [ ] Every interactive element has hover, active, focus-visible, and disabled states.
- [ ] All spacing snaps to the 8pt grid (fine control at 4).
- [ ] Serif used only for display/headings, never bold, never below `--h3`.
- [ ] Content respects max-width tokens; prose stays at ~65–75 chars.
- [ ] Motion is ≤250ms, ease-out, no spring; `prefers-reduced-motion` honored.
- [ ] Fonts loaded with sensible fallbacks (Georgia + humanist system sans).

---

## Deliverables

When restyling, respond with:

1. A short **audit** (~5–10 bullets) of what was off-brand in the source.
2. The **tokens** used (note any that diverge from the canonical set and why).
3. The **restyled code** — complete files unless diffs are requested.
4. A short list of **follow-ups** needing human judgment: a different asset, photograph, or copy edit, rather than a style change.

---

## One-line summary

A well-printed book that happens to be interactive: cream paper, sepia ink, one piece of terracotta pottery on the shelf, and enough air to let every element breathe.
