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
A simple object that exists in the World and can be picked up, previewed, and dropped onto the Placement Grid.
_Avoid_: Item, prop

**Object Preview**:
The temporary visual representation of a Placeable Object at the mouse’s snapped Placement Cell. It is green when the drop is valid and red when it is invalid.
_Avoid_: ghost object

## Actors and View

**Player**:
The single controllable capsule-shaped actor in the World.
_Avoid_: Character, avatar

**Top-down Camera**:
The elevated view that follows the Player at a constant elevation and distance, with eight selectable directions around the Player.
_Avoid_: Isometric camera, fixed camera
