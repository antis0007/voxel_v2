# Teardown: destruction and presentation

Reviewed 2026-10-24; B02/B06/B07/C16/R01. Dennis Gustafsson's [developer blog](https://blog.voxagon.se/) links voxel-space ray tracing, material palettes, fully dynamic quicksaves and design notes. These primary notes replace the earlier reliance on an interview. Specific debris/sleep budgets and an alleged GDC 2022/2023 talk remain unverified.

## What we adopt

Propose material-owned breakage and explicit body/cell extraction, fracture and fusion boundaries. Cache inactive geometry and wake only affected regions. Treat saving a changing world as complete state ownership, including controllers and pending work rather than terrain alone.

## What we reject

Do not infer exact physics integration or cross-platform determinism from rendered destruction. Its rendering strategy does not settle S3, and float simulation restrictions do not apply automatically to presentation.

## What we improve

Specify conserved transferred mass/energy, fixed processing order and bounded fragment admission. S3 compares rendering candidates; body SDDs measure fracture bursts and settled sleep before claiming quality or laptop budgets.
