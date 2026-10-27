---
name: Chatnip
description: One warm, keyboard first inbox for mail, Teams and Slack across every tenant
colors:
  paper: "#faf8f4"
  paper-2: "#f3f0ea"
  paper-3: "#ebe7e0"
  ink: "#1d1a17"
  ink-2: "rgb(29 26 23 / 0.72)"
  ink-3: "rgb(29 26 23 / 0.5)"
  line: "rgb(29 26 23 / 0.08)"
  line-strong: "rgb(29 26 23 / 0.16)"
  accent: "#5a57d6"
  accent-ink: "#ffffff"
  accent-soft: "rgb(90 87 214 / 0.12)"
  signal: "#d9803b"
  glass: "rgb(250 248 244 / 0.78)"
  night-paper: "#211e1b"
  night-paper-2: "#2a2623"
  night-paper-3: "#342f2b"
  night-ink: "#f4f1ec"
  night-ink-2: "rgb(244 241 236 / 0.74)"
  night-ink-3: "rgb(244 241 236 / 0.52)"
  night-line: "rgb(244 241 236 / 0.08)"
  night-line-strong: "rgb(244 241 236 / 0.16)"
  night-accent: "#8d89f0"
  night-accent-ink: "#15132e"
  night-accent-soft: "rgb(141 137 240 / 0.16)"
  night-signal: "#e9a35f"
  night-glass: "rgb(33 30 27 / 0.72)"
  error: "#d14a33"
  warn: "#c98a12"
  success: "#378b4f"
  series-1: "#5a57d6"
  series-2: "#eb6834"
  series-3: "#1baf7a"
  series-4: "#eda100"
  series-5: "#e87ba4"
  series-6: "#008300"
  night-series-1: "#9085e9"
  night-series-2: "#d95926"
  night-series-3: "#199e70"
  night-series-4: "#c98500"
  night-series-5: "#d55181"
  night-series-6: "#008300"
typography:
  headline:
    fontFamily: "Schibsted Grotesk, ui-sans-serif, system-ui, sans-serif"
    fontSize: "17px"
    fontWeight: 600
    lineHeight: 1.4
    letterSpacing: "-0.025em"
  lead:
    fontFamily: "Schibsted Grotesk, ui-sans-serif, system-ui, sans-serif"
    fontSize: "17px"
    fontWeight: 400
    lineHeight: "28px"
    letterSpacing: "normal"
  title:
    fontFamily: "Schibsted Grotesk, ui-sans-serif, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 600
    lineHeight: "20px"
    letterSpacing: "-0.025em"
  body:
    fontFamily: "Schibsted Grotesk, ui-sans-serif, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  menu:
    fontFamily: "Schibsted Grotesk, ui-sans-serif, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  secondary:
    fontFamily: "Schibsted Grotesk, ui-sans-serif, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: "20px"
    letterSpacing: "normal"
  group-header:
    fontFamily: "Schibsted Grotesk, ui-sans-serif, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 500
    lineHeight: "20px"
    letterSpacing: "normal"
  caption:
    fontFamily: "Schibsted Grotesk, ui-sans-serif, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  lead-figure:
    fontFamily: "Martian Mono, ui-monospace, SFMono-Regular, Menlo, monospace"
    fontSize: "16px"
    fontWeight: 500
    lineHeight: 1.5
    letterSpacing: "normal"
  mono:
    fontFamily: "Martian Mono, ui-monospace, SFMono-Regular, Menlo, monospace"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: "16px"
    letterSpacing: "normal"
  caption-label:
    fontFamily: "Martian Mono, ui-monospace, SFMono-Regular, Menlo, monospace"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: "16px"
    letterSpacing: "0.05em"
rounded:
  control: "0.375rem"
  row: "0.625rem"
  card: "0.875rem"
  sheet: "1.125rem"
  pill: "999px"
spacing:
  hair: "2px"
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
  xxl: "24px"
  rail: "3.5rem"
  row: "3.5rem"
  bar: "3rem"
  sidebar-row: "2rem"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-ink}"
    typography: "{typography.secondary}"
    rounded: "{rounded.row}"
    padding: "6px 12px"
    height: "32px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    typography: "{typography.secondary}"
    rounded: "{rounded.row}"
    padding: "6px 12px"
    height: "32px"
  button-ghost-hover:
    backgroundColor: "{colors.paper-3}"
    textColor: "{colors.ink}"
  button-icon:
    backgroundColor: "transparent"
    textColor: "{colors.ink-3}"
    rounded: "{rounded.control}"
    size: "32px"
  button-icon-hover:
    backgroundColor: "{colors.paper-3}"
    textColor: "{colors.ink}"
  button-icon-active:
    backgroundColor: "{colors.accent-soft}"
    textColor: "{colors.accent}"
  input:
    backgroundColor: "{colors.paper-2}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.row}"
    padding: "8px 12px"
  inbox-row:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.row}"
    padding: "0 12px"
    height: "{spacing.row}"
  inbox-row-hover:
    backgroundColor: "{colors.paper-2}"
  inbox-row-focused:
    backgroundColor: "{colors.paper-3}"
  inbox-row-active:
    backgroundColor: "{colors.accent-soft}"
  avatar:
    backgroundColor: "{colors.paper-3}"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.pill}"
    size: "36px"
  card:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "20px"
  card-raised:
    backgroundColor: "{colors.paper-2}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "16px"
  sheet:
    backgroundColor: "{colors.glass}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sheet}"
    padding: "8px"
  chip:
    backgroundColor: "{colors.paper-3}"
    textColor: "{colors.ink-2}"
    typography: "{typography.mono}"
    rounded: "{rounded.pill}"
    padding: "0 6px"
    height: "16px"
  count-badge:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-ink}"
    typography: "{typography.mono}"
    rounded: "{rounded.pill}"
    padding: "0 6px"
    height: "16px"
  kbd:
    backgroundColor: "{colors.paper-3}"
    textColor: "{colors.ink-2}"
    typography: "{typography.mono}"
    rounded: "0.35em"
    padding: "0.1em 0.45em"
  sidebar-row:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    typography: "{typography.menu}"
    rounded: "{rounded.row}"
    padding: "0 8px"
    height: "{spacing.sidebar-row}"
  sidebar-row-hover:
    backgroundColor: "{colors.paper-3}"
    textColor: "{colors.ink}"
  sidebar-row-active:
    backgroundColor: "{colors.accent-soft}"
    textColor: "{colors.accent}"
  split-tab:
    backgroundColor: "transparent"
    textColor: "{colors.ink-3}"
    typography: "{typography.secondary}"
    padding: "6px 10px 8px"
  split-tab-active:
    textColor: "{colors.ink}"
  table-cell:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    typography: "{typography.secondary}"
    padding: "6px 8px"
---

# Design System: Chatnip

## Overview

**Creative North Star: "The Warm Desk"**

Chatnip is one inbox for every conversation Milo is part of, and it is built to be read for hours at a desk by day and in a dim room at night. The surface is warm paper, not white, and warm charcoal, not black; everything on it is ink at full or reduced opacity, so the page reads as one material with words laid on it. Nothing is boxed for its own sake. Panes meet at hairlines, rows are flat until the keyboard lands on them, and the only object that floats is a sheet of frosted glass: the command palette, the snooze and label sheets, menus, pickers and drill downs.

One blue violet accent does all the pointing. It marks the open conversation, the active sidebar row, the primary action, links, unread times and counts, and the leader in a ranking. Where a conversation lives is a grey line glyph, never a colour: the provider badge on an avatar and the glyph row after a name are `ink-3` and desaturated. Split and label colours exist only as 8px dots and tinted pills, so the list stays quiet and the sorting stays legible. The statistics surface adds a six step series palette, and it stays there.

Density is Superhuman's: 56px rows, 32px sidebar rows, 48px bars, 15px body with 17px for the one large line per pane, and Martian Mono for every time, count, address, figure and key. Mail and chat never look the same. A mail message is a paper card with a hairline, a subject and a header; a chat message is a bare run of text with hover actions and no card at all.

**Key Characteristics:**
- Warm paper by day, warm charcoal by night, chosen by the OS colour scheme
- One accent, blue violet, for focus, primary action and links; dots for split colours
- Provider identity as a grey glyph, never a colour
- Flat panes at hairlines; depth only by tone, glass only on floating sheets
- Schibsted Grotesk ranked by weight 400, 500, 600 and by ink opacity; Martian Mono for figures and keys
- Lucide line icons at 1.5px stroke on a 14 / 16 / 18 / 20 / 22px scale
- Every action has a key and the key is shown beside it as a kbd

## Colors

A two tone paper with three steps of elevation, one ink at three opacities, one accent, one warm signal, and a chart series palette confined to the statistics page.

### Primary
- **Blue Violet** (`accent`, day `#5a57d6`, night `#8d89f0`): the open conversation row, the active sidebar row and settings tab, the primary button, links in mail, the underline under the active split tab, unread times and counts, the leader bar in a ranking, the drag handle on hover. Text on it is `accent-ink`.
- **Blue Violet Wash** (`accent-soft`, 12% day, 16% night): the fill of the selected or active row, the selection highlight, the composer focus ring, hover on menu and sheet rows, the brush on the timeline.

### Secondary
- **Series** (`series-1` to `series-6`, day `#5a57d6` `#eb6834` `#1baf7a` `#eda100` `#e87ba4` `#008300`, night `#9085e9` `#d95926` `#199e70` `#c98500` `#d55181` `#008300`): declared on `.stats` in src/routes/main/stats/+page.svelte and consumed through `SERIES` in stats.ts. `series-1` is the accent and carries the single series: timeline area, people table bars, globe dots; `series-2` onward appear only when a comparison adds a second line. Never used outside the statistics surface.

### Tertiary
- **Amber Signal** (`signal`, day `#d9803b`, night `#e9a35f`): the filled star on a pinned conversation and the now line in the calendar. Nowhere else.

### Neutral
- **Paper** (`paper`, day `#faf8f4`, night `#211e1b`): the app ground, the reading pane, mail cards, the inbox list, the statistics page, the globe sea.
- **Paper 2** (`paper-2`, day `#f3f0ea`, night `#2a2623`): the sidebar, the settings nav, raised cards, inputs, the composer field, row hover.
- **Paper 3** (`paper-3`, day `#ebe7e0`, night `#342f2b`): the keyboard focused row, control hover, kbd and chip backgrounds, initials avatars, globe land, the trailing bars in a ranking.
- **Ink** (`ink`, day `#1d1a17`, night `#f4f1ec`): primary text, unread sender, active tab, figures in tables.
- **Ink 2** (`ink-2`, 72% day, 74% night): read sender, sidebar rows, avatar initials, ghost button text, quoted mail, numeric table cells.
- **Ink 3** (`ink-3`, 50% day, 52% night): previews, times, counts, captions, placeholders, idle icons, provider glyphs, table headers.
- **Line** (`line`, 8%): pane borders, dividers, table and list row separators, the hairline on cards, chart gridlines.
- **Line Strong** (`line-strong`, 16%): the window frame inset, kbd and glass rims, scrollbar thumb, quoted mail rule, globe graticule.
- **Glass** (`glass`, paper at 78% day, 72% night): the sole floating material, always with blur.

### Status
- **Error** (`error-500` `#d14a33`), **Warn** (`warn-500` `#c98a12`), **Success** (`success-500` `#378b4f`): failure text, the connection dot on provider avatars, nothing structural.

### Named Rules
**The One Ink Rule.** Secondary and tertiary text are the same ink at lower opacity, never a second grey. New text picks `ink`, `ink-2` or `ink-3` and nothing else.

**The Dot Rule.** Split and label colours appear only as an 8px dot beside the name or as an 18% tint behind a mono pill. They never fill a row, a tab or a button.

**The Single Accent Rule.** Blue violet is the only hue that says "here" or "do this". Amber is reserved for the star and the present moment; status colours are reserved for failure and connection state.

**The Grey Glyph Rule.** A provider is shown by its Lucide glyph in `ink-3` with `grayscale`, on the avatar badge, after a name, in a table cell or a "via" line. `providerColor()` colours the title bar badge alone and falls back to `ink-3`. No provider colour reaches a row, an avatar or text.

**The Series Stays Home Rule.** `series-1` to `series-6` exist inside `.stats`. A chart elsewhere uses `accent`, `accent-soft`, `paper-3` and `line`.

## Typography

**Display Font:** none; the largest text in the app is 17px
**Body Font:** Schibsted Grotesk (variable 400 to 900, self hosted), with ui-sans-serif, system-ui fallback
**Label/Mono Font:** Martian Mono (variable 100 to 800, self hosted), with ui-monospace fallback

**Character:** A sturdy grotesk at a generous 15px, ranked by three weights and three ink opacities rather than by size; a mono for anything that is a figure, an address or a key, so numbers line up and keys read as keys. The root font size is 15px and scales with the UI scale setting (90, 100, 110, 125%), so rem based widths grow with it while px type stays fixed.

### Hierarchy
- **Headline** (600, 17px, tracking tight): the pane title "Inbox", the mail subject at the top of a card, the person title, settings section titles, the sender sheet title, the empty state line. One per pane.
- **Lead** (400, 17px, 28px line, 68ch): the opening paragraph of the statistics page, prose with figures set inline as Lead Figure.
- **Title** (600, 15px, 20px line, tracking tight): the conversation title in the title bar; the unread sender in a row; chart section titles.
- **Body** (400, 15px, 1.5): mail prose (`prose-mail` at 1.6), the composer, inputs, the row's first line when read (500 at `ink-2`).
- **Menu** (400, 14px): sidebar rows (500 when active), settings nav rows, palette and sheet result rows, menu items, the sender name in a mail card header (600).
- **Secondary** (400, 13px, 20px line): row previews, split tab names (500), button labels (500), descriptions under settings fields, hints, table text, chart titles (500 `ink`).
- **Group Header** (500, 13px, 20px line, `ink-2`): inbox group headers ("People", "Mail", "Open threads") and sheet labels ("Snooze until", "Labels", "Sections"), followed by a mono 11px `ink-3` count. Body size, never uppercase.
- **Caption** (400, 12px, `ink-3`): the second line under a palette result, the people line under a mail sender, topic and meta lines, the tab counter in the sidebar collapsed state, right aligned table figures in mono.
- **Lead Figure** (mono 500, 16px): a number inside the lead paragraph on the statistics page; `tabular-nums` throughout.
- **Mono** (400, 11px): times, counts, addresses, meta under a title, palette footer hints, chart axis labels (10px). 10px at 600 inside count pills and chips.
- **Caption Label** (mono 400, 11px, uppercase, 0.05em, `ink-3`): palette group headers ("Recent", "This conversation"), date separators in a thread, calendar day names, "via" lines in a person thread, details panel field labels, compose field labels (to, cc, subject). A label over a block of data, never a heading and never above a title.

### Icon Scale
Lucide through `@iconify/svelte` at a 1.5px stroke, sized by font size: 14px for inline markers and chevrons, 16px in menus and sheets, 18px in 32px controls and sidebar rows, 20px in the title bar and settings nav, 22px in the palette search and the rail's collapsed state. 11 to 13px only for the provider badge glyph. 28 to 40px only as the single empty state icon.

### Named Rules
**The Weight Rank Rule.** Unread is 600 `ink`; read is 500 `ink-2`; preview is 400 `ink-3`. Hierarchy within a row is weight and opacity, never size.

**The Mono Figure Rule.** Every time, count, email address, statistic and keyboard key is set in Martian Mono with tabular figures. A number in the grotesk is a defect.

**The 15 and 17 Rule.** Body is 15px and the one large line per pane is 17px. Nothing in the app is larger than 17px except the empty state icon.

## Layout

The window is a three column grid, `var(--sidebar-width) | var(--list-width) | minmax(0, 1fr)`: a sidebar at `clamp(11rem, 16%, 14rem)` on `paper-2` that collapses to 3.5rem (⌘⇧|, stored on the profile), an inbox list at `clamp(18rem, 32%, 28rem)` that hides with ⌘⇧L, and the reading pane taking the rest. Both widths are dragged on a 1px `line` handle that turns `accent` on hover, clamped 9rem to 24rem for the sidebar, and persisted in localStorage as `chatnip.sidebar-width` and `chatnip.list-width`. The conversation page splits the reading pane again into `minmax(0, 1fr) | var(--thread-width) | var(--details-width)` for the thread and details panels. Settings is a 13rem nav plus a scrolling column capped at 52rem; the reading pane caps mail at 68ch, a person thread at 80ch, the composer at 46rem, the statistics page at 72rem with a two column grid of charts, rankings in one to three CSS columns.

The sidebar is Superhuman's: a 28px avatar row, then Inbox with its unread count, the user's groups as a tree indented 0.75rem per depth with drag to reorder, Starred, Snoozed, Done, a `line` rule, Calendar, Stats, and at the foot connections, theme and Settings. Rows are 32px, 14px text, 18px icon, 10px radius, `ink-2` at rest, `paper-3` on hover, `accent-soft` and `accent` when current, counts in mono 11px right aligned. Collapsed, the same rows are icon only, centred.

Inside a split the list is grouped in a fixed order: Starred, the user's own sections, People (one row per person across providers), Teams and channels, Services (bots and mail without a person), Mail, Direct messages (fallback for what is left), Open threads. Each group has a 32px header row with a chevron, the name at 13px 500 `ink-2` and a mono count, collapses on click, and sits on a 4px gap from the next.

Vertical rhythm is in 8px steps with 4px half steps: bars are 48px, rows are 56px, sidebar and group header rows 32px, the selection bar 40px, controls 32px, avatars 36px (28px in the sidebar, 32px in a mail card). Gutters are 16px at pane edges, 20px in the reading pane and mail cards, 24px on the statistics and person pages, 12px inside rows, 8px around lists. Rows sit on a 2px inner inset so their rounded fill never touches the pane edge.

The whole window is a 14px rounded sheet on a transparent Tauri window with a 1px `line-strong` inset frame; the top 36 to 40px of each pane is a drag region. UI scale (90, 100, 110, 125%) sets the root font size from 15px, so every rem based pane width and radius scales with it.

## Elevation & Depth

Depth is tonal. Three paper steps carry rest, raised and focused; panes meet at 1px `line` borders and cards carry a 1px `line` hairline as an inset box shadow, never a drop shadow. The one exception is glass: floating sheets (command palette, snooze and label sheet, menus, pickers, context menu, modals, the sender drill down on statistics) use `.glass`, which is paper at 72 to 78% with 22px blur and 140% saturation, rimmed with a 1px `line-strong` inset and lifted by one soft shadow. The palette sits over a 20% ink scrim. The globe is flat paper with `paper-3` land and a `line-strong` graticule, no lighting.

Motion is one gesture. CSS transitions are off globally (`transition: none !important`, because the WKWebView leaves colour transitions stale), and the `transition-*` utilities still present in markup are inert. The only movement is the `rise` keyframe, 320ms on `--ease-out` (`cubic-bezier(0.16, 1, 0.3, 1)`), on every sheet, panel and page that enters; it is removed under `prefers-reduced-motion`.

### Shadow Vocabulary
- **Hairline** (`box-shadow: inset 0 0 0 1px var(--line)`): cards, inputs, avatar rims, hover action clusters.
- **Rim** (`box-shadow: inset 0 0 0 1px var(--line-strong)`): kbd, the window frame, the glass edge.
- **Glass lift** (`box-shadow: inset 0 0 0 1px var(--line-strong), 0 24px 60px -24px rgb(0 0 0 / 0.45)`): only on `.glass`.

### Named Rules
**The No Shadow On Panes Rule.** Sidebar, inbox, reading pane, title bar, cards, tables, charts and the composer are flat. A drop shadow belongs only to a floating glass sheet.

**The Glass Floats Rule.** Glass is for things that sit over content and go away: palette, sheets, menus, modals, drill downs. A pane is never glass.

**The One Motion Rule.** Entry is `rise`; everything else snaps. No transition on colour, size or position.

## Shapes

Four radii and a pill. Controls and icon buttons are 6px (`rounded-md`); rows, inputs, sidebar rows and ghost and primary buttons are 10px (`--radius-row`); cards, the composer and settings groups are 14px (`--radius-card`); glass sheets, the palette and menus are 18px (`--radius-sheet`). Avatars, dots, count pills, chips and the scrollbar thumb are full pills; bars in tables and rankings are 1px to 1.5px tall with a 2px right radius. Borders are 1px at 8% or 16% ink, drawn as inset shadows so they never add to layout. The window itself is a 14px rounded sheet. Focus is a 2px accent outline with 2px offset at 6px radius.

## Components

### Buttons
- **Shape:** rounded 10px (`--radius-row`) for labelled buttons; 6px (`rounded-md`) for 32px icon buttons.
- **Primary:** accent fill, `accent-ink` text, 13px at 500, 6px by 12px padding, 32px tall; a kbd inside it uses `accent-ink` at 15% with no rim. Disabled is 40% opacity.
- **Ghost:** transparent, `ink-2` text, same metrics; hover is `paper-3` fill and `ink` text.
- **Icon:** 32px square, `ink-3` glyph at 18 to 20px; hover `paper-3` and `ink`; pressed or on state is `accent-soft` fill with `accent` glyph.
- **Segmented:** a hairline rimmed pill of 10px radius; the on segment is accent fill with `accent-ink`, the rest `paper` with `ink-2`. Used for theme, time format and UI scale.

### Chips
- **Style:** full pill, mono 10px, `ink-2` text, 16px tall, 6px side padding; background is the label colour mixed to 18% in oklab.
- **Count badge:** same geometry, accent fill, `accent-ink`, 600 weight.

### Cards / Containers
- **Corner Style:** 14px (`--radius-card`).
- **Background:** `paper` for a mail card on the reading pane; `paper-2` for a raised card on `paper` (settings groups, composer, thread reply fields, chart frames).
- **Shadow Strategy:** hairline inset only.
- **Border:** none beyond the hairline; settings lists and tables divide rows with `line`.
- **Internal Padding:** 20px on mail cards, 16px on settings cards, 12px by 16px on settings rows, 24px on the sender sheet.

### Inputs / Fields
- **Style:** `paper-2` fill (or `paper` when sitting on `paper-2`), hairline, 10px radius, 15px text, 8px by 12px padding, placeholder `ink-3`. The palette's search field is bare at 17px; the GIF picker search is 13px with a leading icon. Query builder and rule editor fields are bare 13 to 14px inputs inside a hairline row.
- **Focus:** the composer gains a 2px `accent-soft` ring; standalone inputs a 2px accent ring or the global 2px accent outline.
- **Error / Disabled:** error text 12px `error-500` below the field; disabled is 40% opacity.

### Navigation
- **Sidebar:** `clamp(11rem, 16%, 14rem)` of `paper-2`, 32px rows at 14px with an 18px icon and a mono count; idle `ink-2`, hover `paper-3` and `ink`, current `accent-soft`, `accent`, 500. Groups nest as a tree with a chevron and a 6px colour dot. Collapses to 3.5rem of centred icons.
- **Split tabs:** 13px at 500, 8px colour dot, mono count; idle `ink-3`, hover `ink-2`, active `ink` with a 2px accent underline inset 8px.
- **Settings nav:** 13rem `paper-2` column, 14px rows of 10px radius; tabs General, Connections, Organise, Intelligence, Calendar, Media, Shortcuts; active `accent-soft`, `accent`, 500.
- **Pane handles:** a 1px `line` strip on the pane edge with a 12px hit area, `accent` on hover and while dragging.

### Avatar
36px pill of `paper-3` with initials at 12px 600 `ink-2`, or the person's photo; a 28px variant in the sidebar and a 32px one in mail cards. A single source shows as an 18px `paper` badge at the bottom right with the provider glyph at 11px in `ink-3` grayscale; several sources show as a 12px glyph row after the name. Multi select turns the avatar into an accent circle with a check.

### Inbox Row
56px grid of 36px avatar, flexible text, right column; 12px side padding, 12px gaps, 10px radius. Line one: name at 15px (600 `ink` unread, 500 `ink-2` read), for mail the latest sender beside it in 400, then the provider glyph row, member count in mono and the amber star. Line two: preview at 13px `ink-2` or `ink-3`, label chips. Right: mono 11px time (`accent` when unread), accent count pill. States: hover `paper-2`, keyboard focus `paper-3`, open `accent-soft`. A hairline action cluster (done, snooze, star) rises on hover at the right edge.

### Command Palette and Sheets
A 40rem glass sheet of 18px radius 18vh from the top over a 20% ink scrim, rising with `rise`. Search field at 17px, group labels as Caption Label, 14px result rows of 10px radius with `accent-soft` selection, kbd hints right, a mono 11px footer of keys. Snooze and label sheets are the same material pinned inside the inbox pane; each row leads with its kbd. Emoji and GIF pickers are glass sheets with a 13px search field and a three column grid at 6px gaps.

### Messages
- **Mail:** a `paper` card, hairline, 20px padding, 17px subject, 32px initials avatar, 14px sender at 600 with mono address and time, then `prose-mail` at 15px over 1.6 with accent underlined links and a `line-strong` left rule on quotes. Hover shows a hairline action cluster at the top right.
- **Chat:** no card. A run of text with a 10px radius hover fill of `paper-2`, grouped by sender, times in mono.
- **Person thread:** everything from one person across providers at 80ch, mail as cards and chat as runs, each run opened by a Caption Label "via <connection>" line with the grey provider glyph and a `line` rule either side.

### Statistics
A 72rem page on `paper`, opened by a 17px lead paragraph whose figures are mono 16px 500. Charts are inline SVG on paper: the timeline fills `series-1` under a line with an `accent-soft` brush; heatmap and distribution cells step `series-1` by opacity; bars and the people table draw 1.5px `series-1` bars with a 2px right radius; rankings run in CSS columns with a 1px bar under each entry, `accent` for the leader and `paper-3` for the rest; words are a wrapped list sized by count with the top three in `ink`; the globe is an orthographic d3-geo sphere in `paper-2` with `paper-3` land, `line-strong` graticule and `series-1` dots. Tables are `table-fixed` at 13px with 32px `ink-3` headers, 6px by 8px cells, numbers right aligned in mono `ink-2`, rows divided by `line`. A sender drill down opens as a 52rem glass sheet.

### Keys
`kbd` is mono at 0.72em and 500, `paper-3` fill, `ink-2` text, 0.35em radius, `line-strong` rim. It appears beside every action that has a key.

## Do's and Don'ts

### Do:
- **Do** use `paper`, `paper-2`, `paper-3` for every surface step and `ink`, `ink-2`, `ink-3` for every text step; both themes resolve from the same names.
- **Do** rank text by weight (400, 500, 600) and ink opacity at 15px; reserve 17px for the single large line of a pane.
- **Do** set times, counts, addresses, figures and keys in Martian Mono with `tabular-nums`, and show the key beside the action as a `kbd`.
- **Do** keep rows at 56px, sidebar and header rows at 32px, bars at 48px, controls at 32px and pane gutters at 16px.
- **Do** reach for `.hairline` for an edge and `.glass` with `--radius-sheet` for anything that floats; animate its entry with `.rise`.
- **Do** use Lucide line icons on the 14 / 16 / 18 / 20 / 22px scale with the global 1.5px stroke, and show a provider as its glyph in `ink-3` grayscale.
- **Do** size pane widths in rem and clamp them, so the UI scale setting carries them.

### Don't:
- **Don't** put a drop shadow on a pane, a card, a row, a table, a chart or the composer; shadows belong to glass only.
- **Don't** introduce a second grey or a cool neutral; every neutral is warm paper or warm ink at an opacity.
- **Don't** fill a row, tab, avatar or button with a split, label or provider colour; the dot, the 18% chip and the grey glyph are the whole budget.
- **Don't** use `series-*` outside `.stats`.
- **Don't** exceed 17px for text or use a display face; the system has no display tier.
- **Don't** set a group header or a heading in uppercase or mono; uppercase mono is a Caption Label over data, 11px, never above a title.
- **Don't** render chat as cards or mail as bubbles; mail is a paper card, chat is a bare run.
- **Don't** add a CSS transition; the WKWebView leaves them stale, transitions are disabled globally and the only motion is `rise`.
