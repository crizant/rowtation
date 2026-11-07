# UI standards

The current target is web, on desktop and mobile browsers. Native desktop, mobile
and TUI builds are outside this scaffold's scope.

The current screen is a placeholder while import and table UI are planned.
The root fills its surface and respects device safe areas. The shared semantic
button supports keyboard, touch and pointer input.

Use NSH's default semantic theme in light and dark appearances: `background` and
`foreground` for the surface, `primary` and `primary-foreground` for the main action,
and `ring` for visible keyboard focus. Add future app-specific colors through a
theme file configured in `nsh.json`, keeping literal colors out of presentation code.

Use the Tailwind spacing and typography scales: a 4-unit content gap, 2xl page title,
lg supporting text and button labels. Controls use the theme radius and a minimum
44-point height. Keep the layout centered and avoid fixed viewport widths.

Build shared controls in `components/` on top of `nsh/base`. Button hover and pressed
states use primary-token opacity; focus uses the ring token; disabled controls
reduce opacity and suppress interaction. No motion is needed for this starter.


## Converter direction (planned)

Make import → table → export the primary workflow. Put the virtualized table at
the center, with a compact toolbar for search, sort, find/replace and export.
Keep headers and selected-cell identity stable under scrolling and filtering.

Provide Open and paste as alternatives to drag and drop. Keep the format, name,
row count, search scope and pending edits visible. Show parsing errors near their
source and explain lossy exports before download. Failed operations must preserve
the current document.

Support visible keyboard focus, keyboard cell navigation and editing, and touch
access to essential commands. Adapt toolbars to narrow browsers while preserving
all actions. Style semantic NSH controls in the shared component layer.
