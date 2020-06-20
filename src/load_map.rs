// Load the map from data/tiles/*.json

use crate::types::{MapPosition, SerializableTile, Square, SquareType, Tile, WallType};
use log::info;
use serde_json::json;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn load_tiles_from_json() -> HashMap<String, Tile> {
    // TODO
    // Ideally takes a path (like data/tiles/), and returns a hashmap of Tiles
    let tile_map: HashMap<String, Tile> = HashMap::new();
    tile_map
}

pub fn load_tile_json_from_path(s: String) -> Tile {
    let p = Path::new(&s);
    let file = File::open(p).expect("Path should exist");
    let reader = BufReader::new(file);
    let st: SerializableTile =
        serde_json::from_reader(reader).expect("Path should be valid serde JSON of a Tile");
    let t = Tile::from(st);
    t
}

pub fn print_tile_json() -> () {
    // Function for helping when we write new tiles, and want to print them to stdout
    // on a one-off basis to store them in files.
    info!("\n{}\n", json!(SerializableTile::from(tile_2())));
}

pub fn tile_1a() -> Tile {
    // Generate the object for Tile 1a
    let mut my_squares: Vec<Square> = Vec::new();
    let sq00 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Clear,
        south_wall: WallType::Impassable,
        west_wall: WallType::Impassable,
        square_type: SquareType::TimerFlip,
    };
    my_squares.push(sq00);

    let sq01 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Clear,
        south_wall: WallType::Clear,
        west_wall: WallType::Clear,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq01);

    let sq02 = Square {
        north_wall: WallType::OrangeDoor,
        east_wall: WallType::Clear,
        south_wall: WallType::Clear,
        west_wall: WallType::Clear,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq02);

    let sq03 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Impassable,
        south_wall: WallType::Impassable,
        west_wall: WallType::Clear,
        square_type: SquareType::PurpleTeleportPad,
    };
    my_squares.push(sq03);

    let sq10 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Clear,
        south_wall: WallType::Impassable,
        west_wall: WallType::PurpleDoor,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq10);

    let sq11 = Square {
        north_wall: WallType::Clear,
        east_wall: WallType::Clear,
        south_wall: WallType::Clear,
        west_wall: WallType::Clear,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq11.clone());
    // Square 12 is the same as square 11
    my_squares.push(sq11.clone());

    let sq13 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Impassable,
        south_wall: WallType::Impassable,
        west_wall: WallType::Clear,
        square_type: SquareType::YellowTeleportPad,
    };
    my_squares.push(sq13);

    let sq20 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Clear,
        south_wall: WallType::Impassable,
        west_wall: WallType::Impassable,
        square_type: SquareType::OrangeTeleportPad,
    };
    my_squares.push(sq20);
    // Square 21 is the same as square 11
    my_squares.push(sq11.clone());

    let sq22 = Square {
        north_wall: WallType::Clear,
        east_wall: WallType::Impassable,
        south_wall: WallType::Clear,
        west_wall: WallType::Clear,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq22);

    let sq23 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::GreenDoor,
        south_wall: WallType::Impassable,
        west_wall: WallType::Impassable,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq23);

    let sq30 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Clear,
        south_wall: WallType::Impassable,
        west_wall: WallType::Impassable,
        square_type: SquareType::GreenTeleportPad,
    };
    my_squares.push(sq30);

    let sq31 = Square {
        north_wall: WallType::Clear,
        east_wall: WallType::Clear,
        south_wall: WallType::YellowDoor,
        west_wall: WallType::Clear,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq31);

    let sq32 = Square {
        north_wall: WallType::Clear,
        east_wall: WallType::Impassable,
        south_wall: WallType::Impassable,
        west_wall: WallType::Clear,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq32);

    let sq33 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Impassable,
        south_wall: WallType::Impassable,
        west_wall: WallType::Impassable,
        square_type: SquareType::Filled,
    };
    my_squares.push(sq33);
    let my_pos = MapPosition { x: 0, y: 0 };
    Tile {
        squares: my_squares,
        position: my_pos,
        name: "1a".to_string(),
    }
}

pub fn tile_2() -> Tile {
    // Generate the object for Tile 1a
    let mut my_squares: Vec<Square> = Vec::new();
    let sq00 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Impassable,
        south_wall: WallType::Clear,
        west_wall: WallType::Impassable,
        square_type: SquareType::PurpleEscape,
    };
    my_squares.push(sq00);

    let blocked_square = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Impassable,
        south_wall: WallType::Impassable,
        west_wall: WallType::Impassable,
        square_type: SquareType::Filled,
    };
    my_squares.push(blocked_square);

    // square 02 is also blocked
    my_squares.push(blocked_square.clone());
    // square 03 is also blocked
    my_squares.push(blocked_square.clone());

    let sq10 = Square {
        north_wall: WallType::Clear,
        east_wall: WallType::Impassable,
        south_wall: WallType::Impassable,
        west_wall: WallType::Impassable,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq10);

    // square 11 and 12 are also blocked
    my_squares.push(blocked_square.clone());
    my_squares.push(blocked_square.clone());

    let sq13 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Impassable,
        south_wall: WallType::Clear,
        west_wall: WallType::Impassable,
        square_type: SquareType::PurpleTeleportPad,
    };
    my_squares.push(sq13);

    // square 20, 21 are both blocked
    my_squares.push(blocked_square.clone());
    my_squares.push(blocked_square.clone());

    let sq22 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Clear,
        south_wall: WallType::Clear,
        west_wall: WallType::Impassable,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq22);

    let sq23 = Square {
        north_wall: WallType::Clear,
        east_wall: WallType::Clear,
        south_wall: WallType::Impassable,
        west_wall: WallType::Clear,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq23);

    // square 30 is blocked
    my_squares.push(blocked_square.clone());

    let sq31 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Clear,
        south_wall: WallType::OrangeDoor,
        west_wall: WallType::Impassable,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq31);

    let sq32 = Square {
        north_wall: WallType::Clear,
        east_wall: WallType::Clear,
        south_wall: WallType::Impassable,
        west_wall: WallType::Clear,
        square_type: SquareType::Normal,
    };
    my_squares.push(sq32);

    let sq33 = Square {
        north_wall: WallType::Impassable,
        east_wall: WallType::Impassable,
        south_wall: WallType::Impassable,
        west_wall: WallType::Clear,
        square_type: SquareType::GreenTeleportPad,
    };
    my_squares.push(sq33);
    let my_pos = MapPosition { x: 0, y: 0 };
    Tile {
        squares: my_squares,
        position: my_pos,
        name: "2".to_string(),
    }
}

#[allow(dead_code, unused_imports)]
mod tests {
    #[test]
    pub fn test_1a_and_json_match() {
        let code1a = super::tile_1a();
        let json1a = super::load_tile_json_from_path("data/tiles/1a.json".to_string());
        assert_eq!(code1a, json1a)
    }
}
