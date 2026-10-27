# Gamepad input through gilrs

Date: 2026-10-27.

Dependency added: `gilrs` 0.11 (default `wgi` backend on Windows) for gamepad
discovery, hotplug and button/axis events. winit 0.30 has no gamepad API, and a
hand-written XInput or Windows.Gaming.Input layer would duplicate gilrs's
per-platform mappings. A failed backend leaves the game as if no pad were present.

Pad buttons are a third `Binding` kind, so they share the action table, the user
file and the controls screen with keys and mouse buttons. The left stick's four
directions count as buttons with hysteresis, so movement stays digital; analogue
speed would need an `Input` field and is not part of this change. The held-action
mask stays a `u32` at two slots per action, so pad defaults use the free second
slots only. The pad's Menu button is fixed to the pause menu, like Escape.
Right-stick look scales with the mouse sensitivity and the zoom lens.
