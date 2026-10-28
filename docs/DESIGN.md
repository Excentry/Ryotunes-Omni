# Ryotunes design language

> **Inherited design reference.** Written for Ryotunes on Ryoku. Ryotune-Omni keeps this design as its default; the Windows comparison baseline is [W26](windows/W26-VISUAL-BASELINE.md). Ryoku-specific integration does not apply to Windows.

Ryotunes should look like it belongs to Ryoku before a user reads the title.

This document exists to keep visual polish, shell integration and performance moving in the same direction.

## Product character

Ryotunes is:

- architectural rather than decorative;
- warm rather than sterile;
- dense enough to feel like a desktop tool, never cramped;
- expressive through artwork and Ryoku colour, not through permanent animation;
- precise about spacing, alignment and one-pixel structure.

It is not a generic streaming-service clone and it should not become a rainbow dashboard.

## Colour

### Follow System

Follow System is the primary Ryoku-native mode.

Rust resolves Ryoku Material roles and exposes semantic tokens to the frontend. Named Ryoku palettes and wallpaper-derived palettes therefore remain authoritative.

Do not introduce a second independent "Ryotunes system palette" that competes with Ryoku.

### Local overrides

Explicit Light and Dark modes remain local Ryotunes choices.

Light mode uses a restrained material family:

- warm parchment content surfaces;
- sage navigation;
- blue-grey structural surfaces;
- clay / terracotta playback emphasis;
- muted gold only as a supporting detail.

Dark mode stays ink-like and lets artwork or the live Ryoku palette provide most of the colour.

## Geometry

Ryotunes follows the structural language already used across Ryoku:

- one-pixel hairlines;
- deliberate card boundaries;
- compact radii rather than inflated pill geometry;
- clear separation between navigation, content and transport;
- ornaments only in real dead space.

The compositor owns initial main-window sizing and centring on Ryoku.

The mini-player is a separate surface and must never inherit the main-window exact-title rule.

## Typography

Use the existing application type system.

Hierarchy should come from:

1. scale,
2. weight,
3. spacing,
4. semantic colour.

Do not solve hierarchy by adding more colours.

Metadata should remain quiet enough that titles, current playback and primary actions lead the eye.

## Motion

Motion is feedback, not ambient decoration.

Good reasons to animate:

- state changed;
- content entered or left;
- selection moved;
- the user directly manipulated something.

Bad reasons to animate:

- "the screen looks empty";
- a visualizer can run forever;
- a background gradient can drift forever;
- the frontend needs a permanent frame loop for decoration.

Ryoku live motion settings and reduced-motion preferences take precedence.

## Artwork

Artwork is allowed to be colourful. Chrome does not need to compete with it.

Large artwork swaps should:

- reuse already available thumbnails where possible;
- prepare the next image before replacing the current one;
- reject stale asynchronous requests;
- keep decode/cache work bounded.

## Home

Home deliberately keeps a stable DOM.

Do not reintroduce physical section virtualization that mounts and unmounts large portions of the page during scrolling. Previous versions proved that this creates visible jumps and unstable browsing.

Use progressive loading, containment and caching instead.

## Playback surfaces

The bottom player, Now Playing, queue, lyrics and mini-player should feel like one system.

A playback action should not change visual language just because the user moved to another surface.

Transport controls need breathing room around structural dividers. Hover and pressed states must preserve contrast instead of collapsing into black-on-black or white-on-white states.

## Performance budget

A visual change is incomplete until its steady-state cost is considered.

Avoid:

- permanent high-frequency frontend clocks;
- always-on requestAnimationFrame loops;
- decorative FFT work when no visualizer is visible;
- hidden WebKit work that survives after the main surface is hibernated.

The UI should be rich while visible and quiet while absent.

## Review questions

Before merging a UI change, ask:

- Does this still look like Ryoku?
- Does Follow System remain semantically complete?
- Is the Home DOM still stable?
- Does the mini-player remain independent?
- Did this add a new timer, animation loop or hidden renderer wakeup?
- Does the same state look coherent in Home, Queue, Lyrics, Now Playing and Mini?
- Does Light mode remain restrained rather than colourful for its own sake?
