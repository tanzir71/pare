# Design spec

Figma: https://www.figma.com/design/f6552vSJ1ur7qFrxHArKZV/File-Manager. Links below use `?node-id=`.

**Direction:** calm, minimal and trustworthy. The *Files by Google* interaction model (suggestion cards, review lists, one big "free up" action) with an enterprise finish: a deep cobalt-navy accent, slate neutrals, Geist type, 8–12 px radii, thin borders, no gradients in the UI (the logo is the only one), and almost no shadow outside floating surfaces.

## 0. Brand

| | |
|---|---|
| Name | **Pare**, as in "pare down" (see RESEARCH §6 for the shortlist and conflicts) |
| Mark | `brand/mark` `6-198`. **Approved option E**: a storage disk with one slice pared away, on a cobalt gradient tile (`#3352CC → #21368C`, 135°). **Only the top-left corner is curved** (radius = 47% of tile size, so 30 at 64 px and 15 at 32 px); the other three corners are square. Slice `#9DB0FF`, disk white. All options are kept for reference in the "Logo options" board `24-633` |
| Lockup | `brand/lockup` `6-206`: 32 px mark, 10 px gap, wordmark "Pare" in Geist SemiBold 24, tracking −3% |
| Clear space / min size | 25% of mark size / 16 px (title bar) |
| Don't | Recolor the tile with category colors, round all four corners, add a drop shadow, use lowercase "pare" |

Exports needed (M1/M2): `favicon.svg`, `favicon.ico` (16/32/48), `icon.png` 512 and 1024, Windows `icon.ico` (16, 24, 32, 48, 64, 128, 256), Store tiles (44, 150, 310 and wide 310×150), OG image 1200×630.

## 1. Tokens

Source: `design/tokens.json`, which mirrors Figma variable collection **Pare** (modes Light/Dark). CSS variable names are `--pare-<path>`, for example `--pare-accent-default`.

### Color (Light / Dark)

| Token | Light | Dark | Use |
|---|---|---|---|
| `bg/app` | `#F6F7F9` | `#0A0F1A` | Window and page background |
| `bg/surface` | `#FFFFFF` | `#111827` | Cards, lists, panels |
| `bg/subtle` | `#F0F2F5` | `#1A2232` | Group headers, table headers, tracks |
| `bg/inverse` | `#0B1220` | `#F6F7F9` | Bulk bar, dark landing band |
| `border/default` | `#E4E7EC` | `#222B3A` | 1 px borders and dividers |
| `border/strong` | `#CBD2DC` | `#344054` | Outline buttons, checkbox border |
| `text/primary` | `#0B1220` | `#F2F4F7` | |
| `text/secondary` | `#475467` | `#A9B3C3` | Descriptions |
| `text/tertiary` | `#8A94A6` | `#6B778A` | Meta, placeholders, paths |
| `accent/default` | `#2541B2` | `#6F86E8` | Primary buttons, selection, links |
| `accent/hover` | `#1D348F` | `#8A9DEE` | |
| `accent/subtle` | `#EEF1FA` | `#18224A` | Tonal buttons, active nav, selected row |
| `accent/on-subtle` | `#1D348F` | `#C7D1FA` | Text and icons on `accent/subtle` |
| `on-inverse/accent` | `#9DB0FF` | `#2541B2` | Overlines and icons on `bg/inverse` |
| `on-inverse/text` | `#F6F7F9` | `#0B1220` | Headings on `bg/inverse` |
| `on-inverse/text-muted` | `#C3CAD6` | `#475467` | Body text on `bg/inverse` |
| `safe/fg` · `safe/bg` | `#067647` · `#ECFDF3` | `#47CD89` · `#0B2A1C` | Safe tier |
| `review/fg` · `review/bg` | `#B54708` · `#FFFAEB` | `#FDB022` · `#2E2208` | Review tier |
| `keep/fg` · `keep/bg` | `#B42318` · `#FEF3F2` | `#F97066` · `#2D1414` | Protected tier ("Keep", "In use") |
| `info/fg` · `info/bg` | `#175CD3` · `#EFF8FF` | `#53B1FD` · `#0B2340` | Informational |
| `cat/<hue>/fg` · `bg` | see tokens.json | | Category icon tiles: violet (AI tools), orange (projects), teal (dev caches), pink (large files), blue (downloads, screenshots), slate (Recycle Bin, protected), amber (duplicates), green (junk) |

Rule: text on a colored surface always uses that surface's paired `on-*` or `/fg` token. Never put `cat/*/fg` or `accent/default` text on `bg/inverse`; that was the bug in the first landing draft.

### Type (Geist / Geist Mono)

| Style | Weight | Size/Line | Tracking | Use |
|---|---|---|---|---|
| Display/XL | 600 | 64/68 | −3.5% | Landing H1 |
| Display/L | 600 | 48/54 | −3% | Landing section titles, big numbers |
| Heading/H1 | 600 | 30/38 | −2% | Screen titles |
| Heading/H2 | 600 | 24/32 | −1.5% | Card sizes, sub-screen titles |
| Heading/H3 | 600 | 19/26 | −1% | Section headers |
| Title/M | 600 | 15/22 | −0.5% | Card titles |
| Body/L | 400 | 18/28 | −0.5% | Landing body |
| Body/M (Strong 500) | 400 | 14/20 | −0.2% | Default UI text |
| Body/S | 400 | 13/18 | 0 | Descriptions |
| Label/M | 500 | 14/20 | −0.2% | Buttons, chips |
| Label/S | 500 | 12/16 | 0 | Badges |
| Caption | 400 | 12/16 | 0 | Meta |
| Overline | 500 | 11/16 | +8%, uppercase | Eyebrows |
| Mono/S | Geist Mono 400 | 12/16 | 0 | Paths |

Numbers in sizes use `font-variant-numeric: tabular-nums`. Sizes are formatted with one decimal under 100 (`12.8 GB`) and none above (`412 GB`), using decimal units (GB = 10⁹), matching what Explorer users expect. Tooltips show exact bytes.

### Radius, spacing, elevation

- Radius: `sm 6` (checkbox-ish), `md 8` (buttons, chips, nav items, inputs, small tiles), `lg 12` (cards, lists), `xl 16` (summary card, landing cards), `full` (badges only). Windows: 8 px corners (Windows 11 standard).
- Spacing scale: 4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 96. Screen padding is 16 left / 32 right inside main; the gap between sections is 24.
- Elevation: flat by default. Shadows only on the window (`shadow.window`), the floating bulk bar (`shadow.floating`) and the landing hero mock.

### Iconography

`lucide-react`, 2 px stroke; 18 px in controls, 20 in nav, 22–24 in tiles. Mapping:

| Figma | lucide | Figma | lucide |
|---|---|---|---|
| sparkles | `Sparkles` | git-branch | `GitBranch` |
| package | `Package` | terminal | `Terminal` |
| file | `File` | download | `Download` |
| trash | `Trash2` | copy | `Copy` |
| archive | `Archive` | image | `Image` |
| shield-check | `ShieldCheck` | lock | `Lock` |
| history | `History` | refresh | `RefreshCw` |
| wand | `WandSparkles` | sliders | `SlidersHorizontal` |
| hard-drive | `HardDrive` | monitor | `Monitor` |
| folder / folder-open | `Folder` / `FolderOpen` | zap | `Zap` |
| info | `Info` | chevron-* | `Chevron*` |

Never use third-party product logos (Claude, Codex, Cursor, Docker…). Tools are named in text only.

## 2. Components (Foundations `4-2`)

| Component | Node | Props / variants | Notes |
|---|---|---|---|
| Button | `7-66` | Type: Primary, Tonal, Secondary, Ghost, Danger · Size: M (40), L (48) · Label · Show icon · Icon | 8 px radius. Danger only for "Delete permanently" |
| Badge | `7-82` | Tone: Safe, Review, Protected, Neutral, Accent · Label | Pill, 6 px dot. Protected shows "Keep" / "In use" / "Uncommitted" |
| Checkbox | `7-91` | Unchecked, Checked, Indeterminate, Disabled | Disabled = protected item |
| Chip | `7-100` | Default, Selected · Label · Icon · Show icon | Scope and filter chips |
| NavItem | `7-113` | Default, Active · Label · Icon | 208 × 40 |
| SuggestionCard | `9-46` | Title, Description, Size, Meta, Icon; nested Badge and CTA | Tile color = category hue |
| FileRow | `9-68` | Name, Path, Age, Size, Icon; nested Checkbox and Badge | Name and path truncate to one line |
| ProjectRow | `9-86` | Name, Path, Last commit, Branch, Build files, Size; nested Checkbox and Badge | |
| FeatureCard | `9-109` | Title, Body, Icon | Landing |
| Window/TitleBar | `9-117` | none | Custom caption buttons 46 × 40 |
| Sidebar | `9-134` | nested NavItems | 240 wide; drive meter pinned bottom |

States to implement that aren't drawn: hover (surface → `bg/subtle`; primary → `accent/hover`), pressed (−2% lightness), focus (2 px `accent/default` ring, 2 px offset, on every interactive element), disabled (40% opacity, except protected rows, which keep full contrast plus a lock icon and tooltip).

## 3. App screens (`4-3`)

Window 1280 × 800 by default; minimum 960 × 640. Sidebar is fixed; main scrolls. Below 1100 px wide, cards go to 2 columns and the details panel becomes an overlay sheet.

### 01 Clean — Home `11-2`
1. Header: H1 "Clean", search field (300, filters the current scope), Secondary "Scan again".
2. **Storage summary** card: overline with scan time and drives; big number "40.6 GB can be freed"; segmented bar (Other used = accent, Safe = safe/fg, Review = review/fg, Free = bg/subtle) with legend; right column: Primary L "Free up 28.0 GB" (Safe only), caption "Safe items only · your own files stay restorable", Ghost "Review everything first".
3. Scope chips: This PC, Desktop, Downloads, Documents, custom folders, "Add folder".
4. Suggestions grid: 3 columns, 16 gap, cards sorted by reclaimable bytes. Badge shows the dominant tier ("Mostly safe" when mixed).
5. "Kept safe automatically" note, with a link to a sheet that lists protected items and why.

The Primary action asks for confirmation once per session: a sheet listing categories and bytes, with "Free up 28.0 GB" and "Cancel". It never asks again for Safe-only runs in the same session.

### 02 Review — category `12-377`
- Header: back, title, subtitle (bytes, items, sources), Ghost "Select all safe", Secondary "Sort: Size" (Size, Age, Name, Tier).
- Info banner (accent/subtle): the safety guarantees for this category.
- List grouped by source (tool or folder) with tri-state group checkbox and group total. Rows are FileRow.
- **Selection model:** click the row to open details; click the checkbox, or press Space, to toggle; Shift-click selects a range; Ctrl+A selects all *selectable* rows; protected rows are skipped by every bulk selection.
- **Details panel** (320): identity, size and tier badge, "Why Pare suggests this", "If you remove it", a facts table (Created by, Created, Last modified, Last opened, Comes back?, Location in mono, plus Git when relevant), and actions (Primary = the rule's disposal, "Open in Explorer", Ghost "Always keep this", which adds an exclusion).
- **Floating bulk bar** (bg/inverse, radius lg): "6 selected · 8.8 GB", a split line "6.7 GB removed now · 2.1 GB to Recycle Bin", Ghost "Clear", Primary "Free up 8.8 GB".

### 03 Projects `13-647`
Filters: All, Idle 90+ days, Unpushed, Uncommitted, No remote. A summary card with a bulk action. Table columns: checkbox, tile, Project (name and path), Last commit (relative date, plus branch and count), Git status badge, Build files, Size. Protected rows use a slate tile and a disabled checkbox.

### 04 Scanning `14-900`
168 px progress ring (12 px stroke, round caps, accent on a `border/default` track), percentage, phase line, fast-mode note, stats strip (files read, found so far, elapsed), detector checklist (done / running / waiting), Secondary "Cancel scan". Category cards stream into Home as soon as they're ready; the user can leave this screen.

### 05 Cleanup complete `14-1051`
Success mark (72 px solid `safe/fg` circle, white check, soft green shadow), "You freed 25.9 GB", explanation of what went to the Recycle Bin, before/after bars, per-category breakdown, then Ghost "View in History", Secondary "Empty 2.1 GB now", Primary "Done". If some items failed: an inline review/bg banner, "12 items couldn't be removed because they're in use", plus "Retry" and "Show items".

### Not drawn yet (build from these patterns)
- **History**: a list of operations (date, bytes, items, disposal mix) → detail with per-item status, "Restore" for recycled items, and "Empty Pare's items now".
- **Settings**: Scan locations, Exclusions, Thresholds, Disposal ("Send caches to the Recycle Bin too"), Appearance, Updates, About (version, licenses, "Open log folder").
- **Browse (M6)**: treemap plus type categories.
- **Empty states**: first run ("Pick what to scan" with scope chips and a big Primary "Scan this PC"); nothing found ("Nothing worth cleaning here. Your PC is in good shape.").
- **Errors**: access denied (offer "Restart as administrator" only for system targets), drive not ready, scan cancelled.

## 4. Motion

`base` 180 ms with `cubic-bezier(0.2, 0, 0, 1)`. Cards fade and rise 4 px as they stream in (stagger 40 ms, max 8). The progress ring animates smoothly; numbers count up only on the Done screen. Respect `prefers-reduced-motion`: no stagger, no count-up.

## 5. Accessibility

- All text pairs meet WCAG AA, including the badge fg/bg pairs and `on-inverse/*` on `bg/inverse`.
- Every icon-only control gets an `aria-label`; badges are announced ("Safe: rebuilds automatically").
- Keyboard: Tab order is sidebar → header → content → bulk bar. Lists use the roving tabindex pattern. Esc closes the details panel and sheets.
- Never rely on color alone: each tier has a text label.

## 6. Landing page (`16-2`, 1440 desktop)

### 6.1 Sections

1. **Nav**: lockup · Features · How it works · Safety · FAQ · GitHub · Primary M "Download".
2. **Hero**: announcement pill (accent/subtle, shield icon) "Open source · Runs locally · No telemetry"; H1 "Make room on your PC. / Keep what matters."; sub (Body/L, 640 max): "Pare finds caches, idle build folders, AI-tool logs and forgotten downloads — and shows you what's safe to remove before anything is deleted."; CTAs: Primary L "Download for Windows", Secondary L "Star on GitHub"; meta caption "Free and open source (MIT) · Windows 10 & 11 · Nothing leaves your PC"; product mock 1120 wide: a complete mini window (title bar, sidebar, storage summary, scope chips and the first row of 3 cards) built from HTML components. No fade or crop; the window's own rounded corners and shadow finish it.
3. **Trust bar**: "Open-source rules you can audit" · "Your files go to the Recycle Bin first" · "Local-only · no account, no telemetry" · "Every action logged on your PC" (accent icons, 1 px dividers).
4. **Features** (bg/app): overline "WHY PARE"; title "Built for how you actually use your PC"; 3 × 2 FeatureCards (AI tools, git-aware, safety you can read, bulk actions, dates, whole drives in seconds).
5. **How it works**: "Three steps. No surprises." Scan, Review, Free up.
6. **Tools** (bg/app): "The tools that quietly fill your drive"; text-only chips; note "Rules are open-source TOML files — add your own tool in a pull request."
7. **Safety** (bg/inverse): "Built to never delete the wrong thing", 4 guarantees, plus an example list card showing Protected / Review / Review / Safe rows.
8. **FAQ**: 6 Q&As in a 2-column grid (use `<details>` below 768).
9. **Final CTA**: accent band, "Get your space back.", Secondary L "Download for Windows".
10. **Footer**: lockup and tagline; columns Product / Project / Legal (Security, Privacy, MIT License); non-affiliation line.

### 6.2 Copy rules
No superlatives we can't prove ("fastest", "best"). Numbers on the landing page must come from the shipped app (download size, scan speed). Until then use the qualitative copy above. Keep the non-affiliation line in the footer.

### 6.3 Responsive
- **1440**: as drawn; content max-width 1200.
- **1024**: side padding 48; features 2 columns; safety band stacks (text above the card).
- **768**: H1 48/54; trust bar becomes a 2 × 2 grid; steps stack; FAQ 1 column.
- **390**: side padding 20; H1 40/44; CTAs full width and stacked; nav collapses to a menu button; the product mock shows the storage summary and one card only.
