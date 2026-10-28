# Sound Script

Sound Script places a genuine written character beside a universal sign of melody. Meaning comes first: the Japanese kanji 音 denotes sound, while a single eighth note makes the musical intention immediately legible. Meticulously crafted spacing keeps the two forms related without altering the character into a fabricated glyph.

Warm bone sits against near-black, interrupted once by oxide orange. Flat color preserves the clarity of printed ink without metallic shine or ambient glow. Patient calibration and deep expertise should be visible in the balance of luminous strokes, compact counters and dark space.

Rhythm emerges through the character's horizontal structure and the note's rising stem. Broad, precisely rendered strokes carry the main visual weight; the smaller musical accent adds motion. Painstaking attention to scale keeps this relationship clear without crowding the character or borrowing another project's emblem.

The composition remains compact, with generous external margins and an optically balanced center. The musical accent uses a slender stem, an inclined oval head and a curved flag; its lighter weight provides a deliberate counterpoint to the broad character. Master-level execution means refining the relationship between typography, accent and breathing room until the identity feels independent, musical and quietly forceful.

## Asset formats

Use `ryotune-omni-logo.svg` for the opaque logo and `ryotune-omni-symbol.svg` for its transparent counterpart. They share smooth contours, aligned horizontal axes and consistent lower counters. Their transparent and opaque PNG exports use the same geometry.

Use `ryotune-omni-small.svg` for compact icons. Its more open stroke spacing, reinforced note and slightly larger artwork compensate for rasterization at small sizes. The web favicon and generic desktop PNG exports up to 64 pixels use this variant. iOS 20- and 29-point exports use it at every density; Android launcher exports use it at every density as well. Larger desktop PNG exports use the standard logo. ICO and ICNS containers include size-specific optical variants.

The README selects `ryotune-omni-banner-mobile.svg` at viewport widths up to 720 pixels and `ryotune-omni-banner.svg` above that. The compact composition enlarges the title, tagline and experimental-platform notice instead of shrinking the wide banner. Both preserve the same product status. The desktop banner also uses larger secondary lettering and a two-line status notice.

All SVGs contain vector paths, with no embedded bitmaps, external fonts or scripts. Banner lettering is outlined to avoid installed-font differences. PNG, ICO and ICNS files are regenerated native exports, not separate designs; preserve their existing platform dimensions. The web UI uses `ui/src/lib/assets/favicon.svg`; its PNG counterpart shares the compact geometry.

Android normal and circular legacy icons share identical foreground placement and scale. Adaptive icons use an unmasked, transparent foreground over the brand background, `#050505`. The vector drawable and density-specific PNG layers share the same transform; the artwork remains inside the central 66-dp safe area of the 108-dp layer. Keep legacy masks separate from the adaptive foreground.

The note intentionally makes the composition asymmetric. Preserve optical balance rather than mirroring the character. Essential status information must also remain available as README text, independently of the banner. Exporting assets does not validate native builds or platform support.
