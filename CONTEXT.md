# Capsule Wanderer

A tiny 3D movement sandbox used to make player motion easy to see from an elevated top-down view.

## World

**World**:
The playable space in which the Player moves and Placeable Objects can be arranged.
_Avoid_: Level, map

**Checker Tile**:
A square of alternating light and dark ground that makes movement across the World visually legible. It is not a Placement Cell.
_Avoid_: Grid cell, terrain

**Placement Grid**:
The regular arrangement of locations used to position Placeable Objects while preserving the World’s existing extent. Each Checker Tile is divided into four-by-four Placement Cells.
_Avoid_: terrain grid

**Placement Cell**:
The smallest location unit on the Placement Grid. Its physical size is one quarter of a Checker Tile’s side, but it is the canonical one-unit measure for object footprints.
_Avoid_: Checker Tile, terrain tile

## Objects

**Placeable Object**:
A simple object that exists in the World and can be picked up, previewed, and dropped onto the Placement Grid. A Placeable Object may be stacked on another when its full footprint is supported; its base height is then the support's top surface.
_Avoid_: Item, prop

**Crafted Object**:
A Placeable Object assembled from Voxel Cubes in Creation Mode.
_Avoid_: Build, creation

**Voxel Cube**:
The smallest cubic unit from which a Crafted Object is assembled. Voxel Cubes occupy Builder Cells and may be attached to any face of another Voxel Cube.
_Avoid_: Block, brick

**Builder Cell**:
The cubic location used to assemble Voxel Cubes in Creation Mode. Each Builder Cell is 0.25 world units wide; the Workbench surface contains four Builder Cells across each axis, with height limited to fifty cells.
_Avoid_: Placement Cell, voxel

**Builder Volume**:
The bounded one-world-tile Workbench surface in which a Crafted Object is assembled. It is organized as a four-by-four grid of 0.25-unit Builder Cells.
_Avoid_: Build area, canvas

**Object Preview**:
The temporary visual representation of a Placeable Object at the mouse’s snapped Placement Cell. It is green when the drop is valid and red when it is invalid.
_Avoid_: ghost object

## Creation

**Workbench**:
A fixed object in the World that opens Creation Mode when activated.
_Avoid_: Crafting table, builder

**Creation Mode**:
The focused interaction state in which Voxel Cubes are assembled into a Crafted Object at the Workbench.
_Avoid_: Build mode, editor

## Actors and View

**Player**:
The single controllable capsule-shaped actor in the World.
_Avoid_: Character, avatar

**Top-down Camera**:
The elevated view that follows the Player at a constant elevation and distance, with eight selectable directions around the Player.
_Avoid_: Isometric camera, fixed camera
