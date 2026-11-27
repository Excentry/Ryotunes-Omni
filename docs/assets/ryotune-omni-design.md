# Sound Script

Sound Script places a genuine written character beside a universal sign of melody. Meaning comes first: the Japanese kanji 音 denotes sound, while a single eighth note makes the musical intention immediately legible. Meticulously crafted spacing keeps the two forms related without altering the character into a fabricated glyph.

Warm ivory sits against dark ink, interrupted once by a copper-orange note. The mark uses flat color; a shallow gradient on the desktop tile gives it depth without competing with the glyph. The same ivory and copper carry through to the solid-background variants.

Rhythm emerges through the character's horizontal structure and the note's rising stem. Broad, precisely rendered strokes carry the main visual weight; the smaller musical accent adds motion. Painstaking attention to scale keeps this relationship clear without crowding the character or borrowing another project's emblem.

The composition remains compact, with margins tuned to the surface and an optically balanced center. The musical accent uses a reinforced stem, an inclined oval head and a curved flag; its lighter weight provides a deliberate counterpoint to the broad character. Master-level execution means refining the relationship between typography, accent and breathing room until the identity feels independent, musical and quietly forceful.

## Asset formats

The two paths in `ryotune-omni-windows.svg` define the shared mark. The compact SVG (`ryotune-omni-small.svg`), web favicon (`ui/src/lib/assets/favicon.svg`), desktop and Store PNGs, ICO and ICNS all use its borderless rounded tile and geometry.

- **ICO:** Rasterize separately at 16, 20, 24, 28, 30, 32, 36, 40, 44, 48, 60, 64, 72, 80, 96, 128 and 256 pixels (`cargo tauri icon --png`). Pack the 32-bit frames with 48 px first. Tauri takes the first frame for its runtime window and tray icon; Windows can also select the matching size from the executable's icon resource.
- **ICNS and PNG:** Rasterize each native size directly from the vector, including the 2× ICNS entries. Keep existing dimensions and file names.
- **Logo and symbol:** `ryotune-omni-logo.svg` and `ryotune-omni-symbol.svg` use identical, slightly inset geometry. The logo has an opaque background and the symbol has a transparent background. Export both at 2048 px.
- **iOS:** Use inset artwork on a full-bleed opaque ink square so the system applies its own mask. Export as RGB PNGs without an alpha channel.
- **Android:** The normal and circular legacy icons use the same artwork placement with different backgrounds. Adaptive icons use a separate, transparent 108 dp foreground over `#171310`; keep all visible foreground pixels inside the centered 66 dp circle. The drawable and density-specific PNG foregrounds use the same geometry and transform.

The README selects `ryotune-omni-banner-mobile.svg` at viewport widths up to 720 pixels and `ryotune-omni-banner.svg` above that. The compact composition enlarges the title, tagline and experimental-platform notice instead of shrinking the wide banner. Both preserve the same product status. The desktop banner also uses larger secondary lettering and a two-line status notice.

All SVGs contain vector paths, with no embedded bitmaps, external fonts or scripts. Banner lettering is outlined to avoid installed-font differences.

The note intentionally makes the composition asymmetric. Preserve optical balance rather than mirroring the character. Essential status information must also remain available as README text, independently of the banner. Exporting assets does not validate native builds or platform support.
