# Controls

Use the left movement pad and drag the right viewing area to move the camera. On-screen buttons cover fire, aim, reload, jump, crouch, interaction, sprint, map and equipment selection.

Switch between infantry and vehicle profiles manually. Vehicle buttons support holding and dragging across controls. Automatic vehicle detection is not implemented.

Open the layout editor from the launcher or in-game options. Drag buttons to reposition them, adjust size and opacity, then save. Infantry and vehicle layouts are stored separately. Sensitivity and other control preferences are available from the launcher.

The game menus use pointer-style taps. Some interactions require a brief hold rather than a very short tap. Full vehicle/aircraft gameplay and every menu/equipment combination have not been exhaustively tested.

## Physical inputs (0.21)

USB/Bluetooth devices use Android input events and the existing keyboard/mouse game ABI. This is controller-to-keyboard/mouse emulation, not a new Unity gamepad backend. F12 or gamepad Select toggles game input versus menu cursor mode. In menu mode, left stick/D-pad moves a visible cursor and A clicks. Start/Escape opens the native pause panel. Physical mouse movement in gameplay uses pointer capture; switching modes or losing focus releases capture.

| Device | Mapping |
| --- | --- |
| Keyboard/mouse | WASD; mouse look; left/right fire/aim; R reload; Space jump; Ctrl crouch; Shift sprint; F use; Enter loadout; 1–5 equipment; wheel cycles equipment |
| Gamepad sticks | Left: digital WASD movement; right: timed camera motion; L3 sprint; R3 use |
| Gamepad buttons | RT fire; LT aim; A jump; B crouch; X reload; Y loadout; LB/RB cycle equipment |
| Gamepad D-pad in gameplay | Up map; down X/prone binding; left Q/right E bindings |

Touch and physical input ownership is combined so releasing one source does not release an action still held by another. Device removal and focus loss clear held inputs. The published 0.21 implementation uses fixed physical mappings. Full vehicle/aircraft coverage and physical controller-model coverage require separate testing. Equipment cycling tracks a local 1–5 selection; choosing equipment through another input can make the next wheel/bumper step differ from the game's current slot.

## Custom mappings (0.22)

The launcher and in-game pause panel offer **Physical input mapping**. Select a connected controller or alphabetic keyboard, choose a game action, then press and release the physical key. Settings save immediately for that Android device descriptor, stored under its SHA-256 identifier. Reset clears only that device profile. Device identities can differ between USB and Bluetooth, so those may need separate profiles.

Controller motion roles can select another reported axis, invert it or disable it. Moving a stick on the mapping screen displays Android axis names/values. Dead zone and look sensitivity are adjustable per device. Use movement/look role selection to correct swapped sticks; the existing menu/gameplay distinction still applies. Analog-only triggers can be assigned to the aim/fire axis roles; arbitrary key-action capture requires a KeyEvent. Mouse buttons/wheel are unchanged. Left-stick movement still emulates digital WASD.

In the touch editor, **+ Add button** selects another supported guest keyboard/mouse action. Up to 32 extra buttons per infantry/vehicle profile are supported; duplicate actions are not added. Extra buttons can be moved/resized/faded and removed from their settings dialog. Save commits both drafts; Cancel discards edits. Existing default button layouts are preserved unless explicitly reset.

## Launcher settings (0.21)

Render resolution offers 640×360, 960×540 and 1280×720 (default), saved for the next game session. The touch editor retains its logical 1280×720 coordinates. Menu pointer positions are scaled to the actual render surface. Lower resolution helps only when reduced graphics load addresses the bottleneck; no universal FPS improvement is claimed.

The official-distribution notice appears on a fresh launcher creation and can be reopened using the top-right `?` button. Turkish resources supply Turkish text; other locales fall back to English. The GitHub button opens the official repository in an external browser.
