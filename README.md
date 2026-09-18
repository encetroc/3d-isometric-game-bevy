# Capsule Wanderer

A minimal Bevy 3D top-down movement sandbox. The player is a capsule in an otherwise empty world with a checker pattern for visible movement.

## Run

```bash
cargo run
```

## Controls

- `WASD` or arrow keys: move relative to the camera (up moves toward the top of the view)
- Diagonals have the same ground speed as cardinal movement.
- `Q` / `E`: smoothly orbit the camera in opposite directions, 45° per press (holding does not repeat).
- Click a colored object to grab it, move the cursor, then click again to drop it.
  - Green preview: the placement is inside the World and does not overlap another object.
  - Red preview: clicking to drop returns the object to its previous cell.

The World is divided into four-by-four Placement Cells per Checker Tile. The orthographic camera follows the player at a constant elevation and distance inspired by `top-down-camera.webp`. There are eight orbit positions; movement follows the selected view.
