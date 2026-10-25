# UI fonts

The in-game Font panel selects Body, Headings and Numbers independently.
Monocraft is the default. Tiny5 and PixelCode are alternate pixel faces;
Smooth restores egui's bundled proportional/monospace fallbacks.
Missing glyphs retain egui's fallback chain.

| Font | Source | License |
|---|---|---|
| Monocraft | [IdreesInc/Monocraft](https://github.com/IdreesInc/Monocraft) | Monocraft-LICENSE.txt |
| Tiny5 | [Gissio/font_tiny5](https://github.com/Gissio/font_tiny5) | OFL.txt |
| PixelCode | [qwerasd205/PixelCode](https://github.com/qwerasd205/PixelCode) | PixelCode-LICENSE.txt |

Monocraft.ttf and its license were downloaded together from upstream commit
`e498bf70aeb25b4bdcff1e44d878fb2cb4f7c2a9`.
Font SHA-256: `262E31822E0A75BE567B02320BFA092143D871E8A273E529D1C5AB0C04CD215F`.
This font emulates Minecraft lettering without bundling Mojang game assets.

Pixel mode thresholds a copy of font-atlas coverage and uses nearest sampling.
Weight adjusts that coverage threshold. Smooth mode restores original coverage
and linear sampling. This affects UI text, not scene resolution or sprite filters.
Font settings apply at the next UI frame boundary and use preferences version 9;
versions 1 through 8 retain their existing settings and receive font defaults.

Validation: local application check and executable build passed. No tests or game launch;
font appearance, DPI/layout fit, switching and preference round-trip still need
runtime validation.
