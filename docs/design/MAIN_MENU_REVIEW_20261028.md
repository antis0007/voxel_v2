# Main menu design review

The previous menu gave Continue and Quit the same visual weight, concealed the
recent world in a tooltip, used a generic Play heading, and left the action
stack near the top of large windows. Its cold white accents also competed with
the warmer item artwork.

The revision uses the existing game title, Voxel Workshop, and one primary
action. Continue includes the actual recent world name; a fresh library
promotes New World instead of offering a disabled Continue. Worlds and New
World share a secondary row. Multiplayer remains a separate destination, while
Settings and Quit form a quiet utility row. A fresh library pairs Worlds and
Multiplayer beneath New World without duplicating it.

The 408-point column is centred vertically as a group. Secondary actions stack
below 340 points of available width, and the existing scroll container handles
short windows. Icons and labels share a left alignment. World names truncate
within the primary action and retain a full-name tooltip.

The default palette retains its saved ID, with charcoal surfaces, subdued
green-grey edges, warm ivory text and a mint accent. Shared palette methods
derive text and secondary text colours, avoiding a separate white menu palette.
The primary action uses height, fill and an accent edge; utilities use lower
contrast and no idle border. Focus and press states retain clear feedback.
Pixel shading conveys depth; magic is attached to the focused control's icon.
No ambient particles, decorative frame lines or descriptive slogans were added.

Capture cases include desktop, 640×480, a narrow window, a fresh library,
a long world name, library loading, focus, reduced motion and alternate palettes.

## Validation

The combined native executable built successfully. The frozen candidate
`target/multiplayer-cave-visual.exe`, SHA-256
`B74AAE2A5D4C261A717EED779D6A90609165157AF80E0B933004EE7E1F69D5B7`,
exported all 26 production wgpu/egui views successfully into
`target/ui-main-menu-final-review/`. Desktop, compact, narrow, long-name,
busy and alternate-palette images were inspected after the final caption and
centring corrections. Reduced-motion images at two times are byte-identical.
This confirms rendering and layout; no new tests or hosted CI were run.

The menu source is stable for the coordinated gameplay build. The earlier local
final-build failures were unrelated shared integration errors; the combined
candidate above contains the completed menu changes.
