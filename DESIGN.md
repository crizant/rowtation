# UI standards

The current target is web, on desktop and mobile browsers. Native desktop, mobile
and TUI builds are outside this scaffold's scope.

The home/import workflow and editable virtualized inspection grid are implemented.
The root fills its surface and respects device safe areas. The shared semantic
button supports keyboard, touch and pointer input.

Use NSH's default semantic theme in light and dark appearances: `background` and
`foreground` for the surface, `primary` and `primary-foreground` for the main action,
and `ring` for visible keyboard focus. Add future app-specific colors through a
theme file configured in `nsh.json`, keeping literal colors out of presentation code.

Use the Tailwind spacing and typography scales. The visual language is quiet and
monochrome: white surfaces on a light muted background, hairline borders, soft
shadows on cards, rounded-full buttons and the dark primary for the main action.
Visible text is at least 12 points (decorative format marks excepted). Touch
layouts keep 44-point controls and rows; pointer desktops use 36-point compact
controls and 34-point grid rows. Avoid fixed viewport widths.

Build shared controls in `components/` on top of `nsh/base`. Button hover and pressed
states use primary-token opacity; focus uses the ring token; disabled controls
reduce opacity and suppress interaction. No motion is needed for this starter.


## Inspection workflow

Make import → inspect the primary workflow. Keep the virtualized table at the
center. Back in the document toolbar returns home so another file can be imported.
Keep headers and selected-cell identity stable under scrolling. Sort from the column header.
Find, undo, and redo stay in the document toolbar. Export… opens a compact five-format dialog. Closing or replacing a modified
document still asks before the changes are dropped, even after export.

Provide file selection and paste as alternatives to drag and drop. Keep the
format, name and row/column counts visible. Show parsing errors near their source;
failed operations preserve the current document. Support visible keyboard focus,
cell navigation and touch access to essential commands. Adapt toolbars to narrow
browsers while preserving actions. Style semantic NSH controls in the shared layer.


On pointer desktops the import home is centered: app mark, the 30-point semibold
name grouped with the 16-point medium tagline “View, Edit & Convert Your Data.”,
then, with a little more space, the 13-point muted line “CSV, TSV, JSON, NDJSON
& XML — all in your browser, without uploading a thing.”, a rounded drop
card with visible Paste (⌘V/Ctrl+V also works) and Choose file buttons, and supported-format
chips. Narrow and touch browsers use that same type, left-aligned, a pressable import card
and a grouped action list. Show CSV, TSV, JSON, NDJSON and XML as supported; there is no recent
history. The home description says data stays in the browser. Large-file confirmation
stays in the import review.

The document toolbar contains a labeled back action, the filename, and a
"CSV · UTF-8" badge, then Find, Undo, Redo, and Export… on desktop. Below 900 points
the format sits on a second line under the filename so the name keeps the
width. Use Back to close the document and return home to import another file.
Column info and Export… sit in an action bar below the grid, never floating over rows.
The table owns its scrolling region. Keep the status bar outside it and prevent
horizontal page overflow. On desktop, an independent scrolling inspector sits in
a stationary rounded frame. Below 900 points it opens contextually; never squeeze
the permanent sidebar beside a phone-sized grid. Use 44-point rows and targets.

App Dialog centers explicitly on desktop and sits at the bottom below 640 points.
The import review offers a format selector. Delimited formats use radio tiles for the separator and a checkbox for the header
row (it applies on import, not immediately), a measured size summary and, for
large input, a warning with an "Import anyway" action.
Its body scrolls when short viewports or large text need more room. Keep the
heading and import actions outside that scroll area. Fixed-height paste fields
clip their text overflow, use a monospace font and a slim app-styled scrollbar,
and leave room for focus rings and button shadows. All dialogs support
Escape/backdrop dismissal and semantic NSH focus behavior. Mobile padding
includes a base 20-point inset plus the browser safe area.

Show implemented search, replace, sorting, editing, undo and modification counts.
Use the shared dialog for export, with format cards, contextual options, mapping
warnings, with Copy at the far left of the footer and Cancel and Download at the right. Keep the body scrollable and the
actions visible on mobile. Do not draw fake window controls. CSV/TSV values get Text indicators; numeric/date inference would
misrepresent the engine. An inspector distinguishes empty text, Null and Missing and reports canonical type counts. Structured format settings follow docs/structured-imports.md.

Clickable buttons and checkboxes use a pointer cursor; disabled controls use a
not-allowed cursor and suppress interaction.

Editing uses an accessible dialog with a touch-sized Save cell action. Open it
with a double-press, F2, or Edit cell in the cell menu. Clear cell is in that
menu. Find opens a compact bar: the query and match navigation come first.
Case, regex, and all-columns or the selected column are compact toggles on
that bar, and replacement stays closed until asked. Below 900 points the query
keeps its own row so those controls do not shrink it. An empty query shows no
count; a search in flight, no hits, and errors each have their own status.
Matching cells use a primary-token tint, and the active occurrence is marked
in the preview or named in the bar when it lies past that preview. Active
selection retains its dark border. Sort direction is
the table header's own indicator; press it again to reverse, and once more to
restore imported order.
