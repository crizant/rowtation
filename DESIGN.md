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
center, with a compact toolbar for opening another document. Keep headers and
selected-cell identity stable under scrolling. Sort, find/replace and edit controls stay beside the document. Export
belongs to a later milestone.

Provide file selection and paste as alternatives to drag and drop. Keep the
format, name and row/column counts visible. Show parsing errors near their source;
failed operations preserve the current document. Support visible keyboard focus,
cell navigation and touch access to essential commands. Adapt toolbars to narrow
browsers while preserving actions. Style semantic NSH controls in the shared layer.


On pointer desktops the import home is centered: app mark, title, a rounded drop
card with visible Paste (⌘V/Ctrl+V also works) and Choose file buttons, and CSV/TSV
chips. Narrow and touch browsers use a left-aligned title, a pressable import card
and a grouped action list. Show only CSV/TSV as supported; there is no recent
history. The privacy note is concise and mentions large-file confirmation.

The document toolbar contains a labeled back action, filename, a "CSV · UTF-8"
badge and, on wide screens, Import…. Below 900 points, Column info and Import…
sit in an action bar below the grid, never floating over rows.
The table owns its scrolling region. Keep the status bar outside it and prevent
horizontal page overflow. On desktop, an independent scrolling inspector sits in
a stationary rounded frame. Below 900 points it opens contextually; never squeeze
the permanent sidebar beside a phone-sized grid. Use 44-point rows and targets.

App Dialog centers explicitly on desktop and sits at the bottom below 640 points.
The import review uses radio tiles for the separator, a checkbox for the header
row (it applies on import, not immediately), a measured size summary and, for
large input, a warning with an "Import anyway" action.
Its body scrolls when short viewports or large text need more room. Keep the
heading and import actions outside that scroll area. Fixed-height paste fields
clip their text overflow, use a monospace font and a slim app-styled scrollbar,
and leave room for focus rings and button shadows. All dialogs support
Escape/backdrop dismissal and semantic NSH focus behavior. Mobile padding
includes a base 20-point inset plus the browser safe area.

Show implemented search, replace, sorting, editing, undo and modification counts.
Do not show export until it exists, and do not draw fake window controls. CSV/TSV values get Text indicators; numeric/date inference would
misrepresent the engine. An inspector distinguishes empty text from missing fields.

Clickable buttons and checkboxes use a pointer cursor; disabled controls use a
not-allowed cursor and suppress interaction.

Editing uses an accessible dialog with a touch-sized Save cell action. Find and
replace is an expandable toolbar with explicit scope and wrapping compact controls.
Matching cells use primary-token tint; active selection retains its dark border.
Sort direction appears in the header and reset is visible while sorted.
