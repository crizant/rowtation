# UI standards

The current target is web, on desktop and mobile browsers. Native desktop, mobile
and TUI builds are outside this scaffold's scope.

The home/import workflow and read-only virtualized inspection grid are implemented.
The root fills its surface and respects device safe areas. The shared semantic
button supports keyboard, touch and pointer input.

Use NSH's default semantic theme in light and dark appearances: `background` and
`foreground` for the surface, `primary` and `primary-foreground` for the main action,
and `ring` for visible keyboard focus. Add future app-specific colors through a
theme file configured in `nsh.json`, keeping literal colors out of presentation code.

Use the Tailwind spacing and typography scales: a 4-unit content gap, 2xl page title,
lg supporting text; compact controls use smaller labels. Controls use restrained rounded corners and a minimum
44-point height. Keep the layout centered and avoid fixed viewport widths.

Build shared controls in `components/` on top of `nsh/base`. Button hover and pressed
states use primary-token opacity; focus uses the ring token; disabled controls
reduce opacity and suppress interaction. No motion is needed for this starter.


## Inspection workflow

Make import → inspect the primary workflow. Keep the virtualized table at the
center, with a compact toolbar for opening another document. Keep headers and
selected-cell identity stable under scrolling. Search, sort, editing and export
belong to later milestones.

Provide file selection and paste as alternatives to drag and drop. Keep the
format, name and row/column counts visible. Show parsing errors near their source;
failed operations preserve the current document. Support visible keyboard focus,
cell navigation and touch access to essential commands. Adapt toolbars to narrow
browsers while preserving actions. Style semantic NSH controls in the shared layer.


Keep the import home centered on desktop and stacked on narrow browsers. Show only
CSV/TSV as supported. Use a quiet background, rounded import surface and neutral
format badges; omit session history. The privacy/UTF-8/size note is concise.

The document toolbar contains a labeled back action, filename, format and Import.
The table owns its scrolling region. Keep the status bar outside it and prevent
horizontal page overflow. On desktop, an independent scrolling inspector sits in
a stationary rounded frame. Below 900 points it opens contextually; never squeeze
the permanent sidebar beside a phone-sized grid. Use 44-point rows and targets.

App Dialog centers explicitly on desktop and sits at the bottom below 640 points.
Its content scrolls when short viewports or large text need more room. All dialogs
support Escape/backdrop dismissal and semantic NSH focus behavior. Mobile padding
includes a base 20-point inset plus the browser safe area.

Do not show search, replace, sorting, editing, undo or export controls until those
workflows exist. CSV/TSV values get Text indicators; numeric/date inference would
misrepresent the engine. An inspector distinguishes empty text from missing fields.

Clickable buttons and checkboxes use a pointer cursor; disabled controls use a
not-allowed cursor and suppress interaction.
