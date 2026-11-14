# UI pixel magic

The effect communicates interaction state. It is not ambient decoration.

- Idle menus stay still. A hovered or keyboard-focused menu action gains a small
  pixel flame and falling motes behind its existing icon, clipped to that control.
  Labels and hit targets retain their existing positions and dimensions.
- Selected slots use the same two-pixel, dithered rainbow border throughout the
  UI. Eight falling sparks occupy the side gutters, leaving item centres and
  stack counts clear. Hue and motion advance in discrete steps.
- Ordinary panels use flat palette colours, a stepped upper lip and dark lower
  bevel. The previous smooth rim halo and blurred popup shadows are removed.
- Tabs, slot trim and book fittings follow the selected palette. Item artwork
  and semantic warning colours retain their meanings.
- Reduced motion freezes both effects. No extra player-facing controls or copy
  were added.

The flame takes inspiration from Noita's crisp magical pixel effects, rather
than copying its assets. It uses the existing bounded UI GPU uniform pool and
analytic particle trajectories; no particle state, physics, texture downloads or
blur passes are required. Only the focused menu control requests the effect.

Production files: `egui_ui/prism.rs`, `shaders/ui_prism.wgsl`,
`egui_ui/pixel_art.rs`, `egui_ui/grimoire.rs`, `egui_ui/themes.rs` and
`egui_ui/main_menu.rs`. The opt-in menu exporter includes focused, idle,
reduced-motion, compact-window, palette and inventory views for visual review.

## Render review

`target/ui-pixel-magic-final.exe` built successfully with `release-fast`.
The production wgpu/egui exporter completed 22 captures in
`target/ui-pixel-magic-final-preview/`. Focused menu, 640×480 idle menu,
inventory and Amethyst palette renders were inspected. Focused frames at two
times differ; reduced-motion captures at 8 and 24 seconds are byte-identical.
The screenshots establish GPU rendering and layout, not a subjective motion
audition or a performance benchmark. No tests or hosted CI were added or run.
