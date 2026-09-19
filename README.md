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
- Mouse wheel: zoom in and out.
- Click a colored object to grab it, move the cursor, then click again to drop it.
  - Green preview: the placement is inside the World and does not overlap another object or the Workbench.
  - Red preview: clicking to drop returns the object to its previous cell.
- Right-click the brown Workbench to enter Creation Mode.
  - Left-click the floor to add a 0.25-unit Voxel Cube; click a cube face to attach another cube there.
  - The Workbench is a 1×1 world-tile surface with a visible 4×4 placement grid.
  - The builder height is capped at 50 Voxel Cubes.
  - Right-click a cube to remove it, while preserving face-connected structures.
  - Press `Q` / `E` to orbit around the Workbench while building.
  - Crafted-object bounds are hidden while building and shown as an opaque wireframe only while the finished object is hovered (or being dragged).
  - Press `Enter` to finish and `Escape` to cancel. The camera returns to the World view.

The World is divided into four-by-four Placement Cells per Checker Tile. The orthographic camera follows the player at a constant elevation and distance inspired by `top-down-camera.webp`. There are eight orbit positions; movement follows the selected view.
