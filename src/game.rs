use anyhow::{anyhow, Result};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryInto;

use crate::load_map;
use crate::types::main_message::Body;
use crate::types::{
    Ability, GameState, GameStatus, Heister, HeisterColor, Internal, MainMessage, MapPosition,
    Move, MoveDirection, PlaceTile, Player, PlayerName, Square, SquareType, Tile, WallType,
    DOOR_TYPES,
};
use crate::utils::get_current_time_secs;

use log::{info, trace};

const MAX_PLAYERS: u32 = 8;
const TIMER_DURATION_SECS: u64 = 5 * 60;

#[derive(Debug)]
pub struct Game {
    pub game_handle: GameHandle,
    pub game_state: GameState,
    pub tile_deck: Vec<Tile>,
    pub game_created: u64,
    revealed_teleporters: HashMap<HeisterColor, Vec<MapPosition>>,
}
#[derive(Clone, Default, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct GameHandle(pub String);

pub struct GameOptions {
    pub shuffle_tiles: bool,
}

impl Default for GameOptions {
    fn default() -> Self {
        GameOptions {
            shuffle_tiles: true,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum MoveValidity {
    Valid,
    Invalid(String),
}

impl Game {
    pub fn new(game_handle: GameHandle, game_options: GameOptions) -> Game {
        let game_state = GameState::new(game_handle.clone());
        let mut tile_deck: Vec<Tile> = load_map::load_tiles_from_json();
        if game_options.shuffle_tiles {
            let mut rng = thread_rng();
            tile_deck.shuffle(&mut rng);
        }
        let game_created = get_current_time_secs();
        // NOTE: Assumption: All games start with only one tile revealed
        let mut revealed_teleporters: HashMap<HeisterColor, Vec<MapPosition>> = HashMap::new();
        Self::update_revealed_teleporters(
            &mut revealed_teleporters,
            game_state.tiles.first().unwrap(),
        );
        Game {
            game_handle,
            game_state,
            tile_deck,
            game_created,
            revealed_teleporters,
        }
    }

    fn draw_tile(&mut self) -> Option<Tile> {
        let tile = self.tile_deck.pop();
        self.game_state.remaining_tiles = self.tile_deck.len().try_into().unwrap();
        tile
    }

    pub fn add_player(&mut self, name: String) -> Result<()> {
        if self.game_state.game_status != GameStatus::Staging {
            // If the game is already in progress, don't actually register the player.
            return Ok(());
        }
        self.game_state.players.push(Player {
            name,
            abilities: vec![],
        });
        Ok(())
    }

    pub fn start_game(&mut self) -> MoveValidity {
        // When we start the game, we can figure out how to break up the abilities.
        let player_abilities: Vec<Vec<Ability>> =
            get_player_abilities(self.game_state.players.len());
        for (i, player) in self.game_state.players.iter_mut().enumerate() {
            player.abilities = player_abilities[i].clone();
        }

        // Kick off the timer.
        let now = get_current_time_secs();
        self.game_state.game_started = now;
        self.game_state.timer_runs_out = now + TIMER_DURATION_SECS;

        // Set the game status to ONGOING.
        self.game_state.game_status = GameStatus::Ongoing;

        // TODO Add this later, it would be too annoying for now for testing.
        /*
        if self.game_state.players.len() < 2 {
            MoveValidity::Invalid("There must be at least 2 players".to_string())
        } else {
            MoveValidity::Valid
        }
        */
        MoveValidity::Valid
    }

    fn rotate_abilities(&mut self) {
        let mut player_abilities: Vec<Vec<Ability>> = self
            .game_state
            .players
            .iter()
            .map(|p| p.abilities.clone())
            .collect();
        player_abilities.rotate_right(1);
        for (i, player) in self.game_state.players.iter_mut().enumerate() {
            player.abilities = player_abilities[i].clone();
        }
    }

    fn player_has_ability(&self, player_name: &PlayerName, ability: &Ability) -> bool {
        let player = self
            .game_state
            .players
            .iter()
            .find(|p| p.name == player_name.0)
            .expect("Tried to check for ability for player not in game");
        player.abilities.contains(ability)
    }

    pub fn has_player(&self, name: &str) -> bool {
        for p in self.game_state.players.iter() {
            if p.name == name {
                return true;
            }
        }
        false
    }

    pub fn get_game_state(&self) -> GameState {
        self.game_state.clone()
    }

    pub fn get_absolute_grid(&self) -> HashMap<MapPosition, Square> {
        let mut grid: HashMap<MapPosition, Square> = HashMap::new();
        for tile in self.game_state.tiles.iter() {
            // this is the top position for the tile - we can assign positions for this
            let tile_pos = &tile.position;
            for (i, square) in tile.squares.iter().enumerate() {
                let sq_x = (i % 4) as i32;
                let sq_y = (i / 4) as i32;
                let grid_x = tile_pos.x + sq_x;
                let grid_y = tile_pos.y + sq_y;
                trace!(
                    "{}: {:?} {:?} {:?} {:?}, {:?}",
                    i,
                    square.north_wall,
                    square.west_wall,
                    square.south_wall,
                    square.east_wall,
                    square.square_type
                );
                let mp = MapPosition {
                    x: grid_x,
                    y: grid_y,
                };
                grid.insert(mp, square.clone());
            }
        }
        grid
    }

    // NOTE: Would be nice if self.game_state.heisters was a map<color, heister>
    // or even <color, pos>
    fn get_mut_heister_from_vec(&mut self, hc: HeisterColor) -> Option<&mut Heister> {
        for h in self.game_state.heisters.iter_mut() {
            if h.heister_color == hc {
                return Some(h);
            }
        }
        return None;
    }

    fn get_heister_from_vec(&self, hc: HeisterColor) -> Option<&Heister> {
        for h in self.game_state.heisters.iter() {
            if h.heister_color == hc {
                return Some(h);
            }
        }
        return None;
    }

    fn move_blocked_by_wall(
        &self,
        grid: &HashMap<MapPosition, Square>,
        heister_pos: &MapPosition,
        dest_pos: &MapPosition,
    ) -> MoveValidity {
        // Assumes that heister_pos & dest_pos are adjacent
        let heister_square = match grid.get(&heister_pos) {
            Some(s) => s,
            None => {
                return MoveValidity::Invalid(format!(
                    "Heister square {:?} doesn't exist",
                    heister_pos
                ))
            }
        };
        let blocking_wall = match heister_pos.adjacent_move_direction(dest_pos) {
            MoveDirection::North => heister_square.north_wall,
            MoveDirection::East => heister_square.east_wall,
            MoveDirection::South => heister_square.south_wall,
            MoveDirection::West => heister_square.west_wall,
        };

        let invalid_msg = format!("Wall {:?} cannot be passed through", blocking_wall);
        match blocking_wall {
            WallType::Clear => MoveValidity::Valid,
            WallType::Impassable => {
                MoveValidity::Invalid("Can't pass through impassable wall".to_string())
            }
            // Wildcard matches each tile-discovery type (one per color)
            _wildcard => MoveValidity::Invalid(invalid_msg),
        }
    }

    fn position_is_occupied(&self, position: &MapPosition) -> MoveValidity {
        for h in &self.game_state.heisters {
            match &h.map_position == position {
                true => {
                    let msg = format!("Heister {:?} is on {:?}", h.heister_color, position);
                    return MoveValidity::Invalid(msg);
                }
                false => {}
            }
        }
        return MoveValidity::Valid;
    }

    fn teleport_matches_color(teleport_type: SquareType, color: HeisterColor) -> bool {
        match teleport_type {
            SquareType::PurpleTeleportPad => color == HeisterColor::Purple,
            SquareType::OrangeTeleportPad => color == HeisterColor::Orange,
            SquareType::GreenTeleportPad => color == HeisterColor::Green,
            SquareType::YellowTeleportPad => color == HeisterColor::Yellow,
            _wildcard => false,
        }
    }

    fn position_squaretype(
        grid: &HashMap<MapPosition, Square>,
        pos: &MapPosition,
    ) -> Result<SquareType> {
        let square = match grid.get(&pos) {
            Some(s) => s,
            None => {
                return Err(anyhow!("No square at pos {:?}", pos));
            }
        };
        Ok(square.square_type)
    }

    fn position_is_escalator(
        grid: &HashMap<MapPosition, Square>,
        pos: &MapPosition,
    ) -> MoveValidity {
        match grid.get(&pos) {
            Some(square) => match square.square_type {
                SquareType::Escalator => MoveValidity::Valid,
                _wildcard => {
                    let msg = format!("Square at {:?} is not escalator", pos);
                    MoveValidity::Invalid(msg)
                }
            },
            None => {
                let msg = format!("Position {:?} not found in grid", pos);
                MoveValidity::Invalid(msg)
            }
        }
    }

    fn get_tile_with_index(&self, position: &MapPosition) -> Option<(usize, Tile)> {
        for (i, t) in self.game_state.tiles.iter().enumerate() {
            let x_distance = position.x - t.position.x;
            let x_distance_within_tile = x_distance >= 0 && x_distance < 4;
            match x_distance_within_tile {
                true => {
                    let y_distance = position.y - t.position.y;
                    let y_distance_within_tile = y_distance >= 0 && y_distance < 4;
                    match y_distance_within_tile {
                        true => return Some((i, t.clone())),
                        false => continue,
                    }
                }
                false => continue,
            }
        }
        None
    }

    /// To validate an escalator move, we need to do a few checks:
    /// 1. is the dest position on an escalator square?
    /// 2. is the heister on an escalator square?
    /// 3. is the heister in the same tile as the dest escalator?
    fn validate_escalator_move(
        &self,
        grid: &HashMap<MapPosition, Square>,
        heister_pos: &MapPosition,
        dest_pos: &MapPosition,
    ) -> MoveValidity {
        match Self::position_is_escalator(grid, dest_pos) {
            MoveValidity::Valid => {
                match grid.get(&heister_pos).unwrap().square_type {
                    SquareType::Escalator => {
                        // last check: is the heister & dest on the same tile?
                        let ht = self.get_tile_with_index(heister_pos).unwrap().1;
                        let dt = self.get_tile_with_index(dest_pos).unwrap().1;
                        match ht == dt {
                            true => MoveValidity::Valid,
                            false => MoveValidity::Invalid(
                                "Heister and dest escalator are on different tiles".to_string(),
                            ),
                        }
                    }
                    _wildcard => {
                        MoveValidity::Invalid("Heister is not on an escalator".to_string())
                    }
                }
            }
            _invalid => _invalid,
        }
    }

    /// To validate a teleporter move, we need to do a few checks:
    /// 1. is the dest position on a teleporter square?
    /// 2. is the source position on a teleporter square matching its color?
    /// 3. is the heister color matching the teleporter color?
    fn validate_teleport(
        &self,
        grid: &HashMap<MapPosition, Square>,
        heister: &Heister,
        dest_pos: &MapPosition,
    ) -> MoveValidity {
        let heister_color = heister.heister_color;
        let heister_pos = &heister.map_position;
        let heister_square_type = Self::position_squaretype(grid, &heister_pos).unwrap();
        let dest_square_type_maybe = Self::position_squaretype(grid, &dest_pos);
        match dest_square_type_maybe {
            Ok(dest_square_type) => {
                if !Self::teleport_matches_color(heister_square_type, heister_color) {
                    let msg = "Heister and teleporter color do not match";
                    return MoveValidity::Invalid(msg.to_string());
                }
                match heister_square_type == dest_square_type {
                    true => MoveValidity::Valid,
                    false => {
                        let msg = "Source and Dest teleporter colors do not match";
                        MoveValidity::Invalid(msg.to_string())
                    }
                }
            }
            Err(e) => {
                let msg = format!("Destination is not on the grid: {:?}", e);
                MoveValidity::Invalid(msg.to_string())
            }
        }
    }

    fn validate_adjacent_move(
        &self,
        grid: &HashMap<MapPosition, Square>,
        heister_pos: &MapPosition,
        dest_pos: &MapPosition,
    ) -> MoveValidity {
        let validity = self.move_blocked_by_wall(&grid, &heister_pos, &dest_pos);
        match validity {
            MoveValidity::Invalid(_) => return validity,
            _ => (),
        }
        self.position_is_occupied(&dest_pos)
    }

    fn get_door_wall(square: &Square) -> Option<WallType> {
        // Return the square's (exit) door, if it has one
        let walls = square.get_walls();
        let door = walls
            .values()
            .find(|&wt| DOOR_TYPES.iter().any(|&dt| wt == dt));
        match door {
            Some(d) => {
                let ret = d.clone();
                Some(ret.clone())
            }
            None => None,
        }
    }

    fn get_door_direction(square: &Square) -> Option<MoveDirection> {
        // Return the direction of the square's (exit) door, if it has one
        match Self::get_door_wall(square) {
            Some(_) => (),
            None => return None,
        };
        for (dir, wall) in square.get_walls().iter() {
            if DOOR_TYPES.contains(&wall) {
                return Some(dir.clone());
            }
        }
        None
    }

    fn heister_tile_placement_positions(
        &self,
        grid: &HashMap<MapPosition, Square>,
    ) -> Vec<MapPosition> {
        // Return the places from which you could draw a tile
        // AKA - squares where a matching heister is on a square with a HeisterColor door
        let mut placement_locations: Vec<MapPosition> = Vec::new();
        for heister in &self.game_state.heisters {
            let color = heister.heister_color;
            let square = grid
                .get(&heister.map_position)
                .expect("Heister on invalid square");
            let maybe_door = Self::get_door_wall(square);
            let door = match maybe_door {
                Some(d) => d,
                None => continue,
            };
            // TODO: put this in a helper?
            match door {
                WallType::PurpleDoor => {
                    if color == HeisterColor::Purple {
                        placement_locations.push(heister.map_position.clone());
                    }
                }
                WallType::OrangeDoor => {
                    if color == HeisterColor::Orange {
                        placement_locations.push(heister.map_position.clone());
                    }
                }
                WallType::GreenDoor => {
                    if color == HeisterColor::Green {
                        placement_locations.push(heister.map_position.clone());
                    }
                }
                WallType::YellowDoor => {
                    if color == HeisterColor::Yellow {
                        placement_locations.push(heister.map_position.clone());
                    }
                }
                _wildcard => (),
            }
        }
        placement_locations
    }

    fn heister_to_tile_entrance_positions(
        &self,
        grid: &HashMap<MapPosition, Square>,
    ) -> HashMap<MapPosition, MapPosition> {
        // Returns a map from current heister positions to their prospective tile_entrance
        // positions, one tile away (if there are any such locations)
        let heister_door_positions = self.heister_tile_placement_positions(&grid);
        let mut tile_entrance_positions: HashMap<MapPosition, MapPosition> = HashMap::new();
        for heister_pos in heister_door_positions {
            let square = grid
                .get(&heister_pos)
                .expect("Heister must be on a valid square");
            let dir = &Self::get_door_direction(square)
                .expect("Square must have a door on it to be entered through");
            tile_entrance_positions.insert(heister_pos.clone(), heister_pos.move_in_direction(dir));
        }
        tile_entrance_positions
    }

    fn update_auxiliary_state(&mut self) -> () {
        let grid = self.get_absolute_grid();
        self.update_possible_placements(&grid);
        self.update_possible_escalators(&grid);
        self.update_possible_teleports(&grid);
    }

    /// Possible placements for new tiles that Heisters can discover
    fn update_possible_placements(&mut self, grid: &HashMap<MapPosition, Square>) -> () {
        let heister_to_tile_entrance_locs = self.heister_to_tile_entrance_positions(&grid);

        let mut v = Vec::new();
        for val in heister_to_tile_entrance_locs.values() {
            v.push(val.clone());
        }
        self.game_state.possible_placements = v;
    }

    /// Possible escalator destinations that a Heister can reach with an Escalator move
    fn update_possible_escalators(&mut self, grid: &HashMap<MapPosition, Square>) -> () {
        let mut m: HashMap<HeisterColor, MapPosition> = HashMap::new();
        for heister in &self.game_state.heisters {
            let color = heister.heister_color;
            let pos = &heister.map_position;

            let square = grid.get(&pos).unwrap();
            if square.square_type == SquareType::Escalator {
                let (_idx, tile) = self.get_tile_with_index(&pos).unwrap();
                let dest_pos = tile.get_escalator_dest(&pos).unwrap();
                m.insert(color, dest_pos);
            }
        }
        self.game_state.possible_escalators = m;
    }

    /// Possible teleport destinations that a Heister can reach with a Teleport move
    fn update_possible_teleports(&mut self, grid: &HashMap<MapPosition, Square>) -> () {
        let mut m: HashMap<HeisterColor, Vec<MapPosition>> = HashMap::new();
        for heister in &self.game_state.heisters {
            let color = heister.heister_color;
            let pos = &heister.map_position;
            let square = grid.get(&pos).unwrap();
            if square.is_teleport() {
                match self.revealed_teleporters.get(&color) {
                    Some(list) => {
                        m.insert(color, list.to_vec());
                    }
                    None => {}
                }
            }
        }
        self.game_state.possible_teleports = m;
    }

    /// Return all revealed teleporters, called after adding new tiles
    fn update_revealed_teleporters(
        already_revealed: &mut HashMap<HeisterColor, Vec<MapPosition>>,
        new_tile: &Tile,
    ) -> () {
        for (color, teleporter_pos) in new_tile.get_teleporters() {
            match already_revealed.get_mut(&color) {
                Some(already_revealed_list) => {
                    // append the new list onto the end
                    already_revealed_list.push(teleporter_pos);
                }
                None => {
                    // set the new list from teleporter list
                    let teleporter_list: Vec<MapPosition> = vec![teleporter_pos];
                    already_revealed.insert(color, teleporter_list);
                }
            }
        }
    }

    /// From a tile entrance and move direction of the tile's orientation,
    /// return the MapPosition for that new tile to place it in the absolute grid
    /// This is doable since every tile has an entry square in some rotation of
    /// (1, 3) - except for starting tiles
    fn new_tile_position(position: &MapPosition, dir: &MoveDirection) -> MapPosition {
        match dir {
            MoveDirection::North => MapPosition {
                x: position.x - 1,
                y: position.y - 3,
            },
            MoveDirection::East => MapPosition {
                x: position.x,
                y: position.y - 1,
            },
            MoveDirection::South => MapPosition {
                x: position.x - 2,
                y: position.y,
            },
            MoveDirection::West => MapPosition {
                x: position.x - 3,
                y: position.y - 2,
            },
        }
    }

    /// From a tile exit square (one from which a player might initiate a PlaceTile move),
    /// figure out the MapPosition of the tile that the heister is on.
    /// (Useful for looking up which tile a heister might currently be on)
    /// * You might notice - this is the same as new_tile_position, but with opposite
    /// directions swapped. That's true! That's the magic of the game.
    fn current_tile_position(position: &MapPosition, dir: &MoveDirection) -> MapPosition {
        match dir {
            MoveDirection::North => MapPosition {
                x: position.x - 2,
                y: position.y,
            },
            MoveDirection::West => MapPosition {
                x: position.x,
                y: position.y - 1,
            },
            MoveDirection::South => MapPosition {
                x: position.x - 1,
                y: position.y - 3,
            },
            MoveDirection::East => MapPosition {
                x: position.x - 3,
                y: position.y - 2,
            },
        }
    }

    fn validate_player_has_move_direction_ability(
        &self,
        current_pos: &MapPosition,
        dest_pos: &MapPosition,
        player_name: &PlayerName,
    ) -> MoveValidity {
        let direction = current_pos.adjacent_move_direction(&dest_pos);
        match direction {
            MoveDirection::North => {
                if !self.player_has_ability(&player_name, &Ability::MoveNorth) {
                    return MoveValidity::Invalid("You cannot move heisters North".to_string());
                }
            }
            MoveDirection::East => {
                if !self.player_has_ability(&player_name, &Ability::MoveEast) {
                    return MoveValidity::Invalid("You cannot move heisters East".to_string());
                }
            }
            MoveDirection::South => {
                if !self.player_has_ability(&player_name, &Ability::MoveSouth) {
                    return MoveValidity::Invalid("You cannot move heisters South".to_string());
                }
            }
            MoveDirection::West => {
                if !self.player_has_ability(&player_name, &Ability::MoveWest) {
                    return MoveValidity::Invalid("You cannot move heisters West".to_string());
                }
            }
        }
        MoveValidity::Valid
    }

    fn process_move(&mut self, m: Move, player_name: &PlayerName) -> MoveValidity {
        let heister_color = m.heister_color;
        let heister = self.get_heister_from_vec(heister_color).unwrap();
        let heister_pos = &heister.map_position;
        let dest_pos = m.position;

        let grid = self.get_absolute_grid();
        if heister_pos.is_adjacent(&dest_pos) {
            let ability_validity = self.validate_player_has_move_direction_ability(
                &heister_pos,
                &dest_pos,
                &player_name,
            );
            if let MoveValidity::Invalid(_) = ability_validity {
                return ability_validity;
            }
            let validity = self.validate_adjacent_move(&grid, heister_pos, &dest_pos);
            if validity == MoveValidity::Valid {
                let heister = self
                    .get_mut_heister_from_vec(heister_color.clone())
                    .unwrap();
                heister.map_position = dest_pos;
            }
            return validity;
        }
        match Self::position_squaretype(&grid, heister_pos) {
            // Handle escalator move
            Ok(SquareType::Escalator) => {
                if !self.player_has_ability(&player_name, &Ability::UseEscalator) {
                    return MoveValidity::Invalid("You cannot use escalators".to_string());
                }
                let validity = self.validate_escalator_move(&grid, heister_pos, &dest_pos);
                if validity == MoveValidity::Valid {
                    let mut heister = self
                        .get_mut_heister_from_vec(heister_color.clone())
                        .unwrap();
                    heister.map_position = dest_pos;
                }
                validity
            }
            // Handle teleport move
            Ok(SquareType::OrangeTeleportPad)
            | Ok(SquareType::YellowTeleportPad)
            | Ok(SquareType::PurpleTeleportPad)
            | Ok(SquareType::GreenTeleportPad) => {
                if !self.player_has_ability(&player_name, &Ability::UseEscalator) {
                    return MoveValidity::Invalid("You cannot use teleporters".to_string());
                }
                let validity = self.validate_teleport(&grid, heister, &dest_pos);
                if validity == MoveValidity::Valid {
                    let heister = self
                        .get_mut_heister_from_vec(heister_color.clone())
                        .unwrap();
                    heister.map_position = dest_pos;
                }
                validity
            }
            _wildcard => {
                let msg = format!(
                    "Invalid move for heister {:?} at {:?} to position {:?}",
                    heister_color, heister_pos, dest_pos
                );
                MoveValidity::Invalid(msg.to_string())
            }
        }
    }

    fn place_tile(&mut self, position: &MapPosition, direction: &MoveDirection) -> MoveValidity {
        let tile = self.draw_tile();
        match tile {
            Some(t) => {
                let new_pos = Self::new_tile_position(position, direction);
                let num_rotations = match direction {
                    MoveDirection::North => 0,
                    MoveDirection::East => 1,
                    MoveDirection::South => 2,
                    MoveDirection::West => 3,
                };
                let mut m: Vec<Vec<Square>> = t.to_matrix();
                for _ in 0..num_rotations {
                    m = Tile::rotate_matrix_clockwise(&m);
                }
                let rotated_tile = Tile::from_matrix(m, t.name.clone(), new_pos, num_rotations);
                info!(
                    "Added Tile {} at {:?} to Game map",
                    rotated_tile.name, rotated_tile.position
                );

                // Add the tile before opening doors on it, that way helpers that
                // rely on the tile's presence in game.tiles work correctly
                let new_tile_idx = self.game_state.tiles.len();
                Self::update_revealed_teleporters(&mut self.revealed_teleporters, &rotated_tile);
                self.game_state.tiles.push(rotated_tile);
                self.update_tile_doors(new_tile_idx); // Must be called _after_ push
                MoveValidity::Valid
            }
            None => MoveValidity::Invalid("No tiles left in deck to draw".to_string()),
        }
    }

    /// in order to update the door to be a clear wall, we need a few things:
    /// 1. we need a reference to the tile in self.tiles that contains the heister_square
    /// 2. we need to be able to know which wall on which square  to update
    /// 3. we need to replace that square wth one who has a clear wall instead of a door
    fn open_door(
        &mut self,
        door_pos: MapPosition,
        src_square: Square,
        dir: &MoveDirection,
    ) -> Result<()> {
        let current_tile_position = Self::current_tile_position(&door_pos, &dir);
        let mut tile = &mut Tile::default();
        for t in &mut self.game_state.tiles {
            if t.position == current_tile_position {
                tile = t;
                break;
            }
        }
        if tile.squares.len() == 0 {
            return Err(anyhow!("No tile found at pos {:?}", door_pos));
        }

        // TODO: the helper i am writing will change THIS iterator in open_door
        // helper = something like "get tile door squares"
        for mut square in &mut tile.squares {
            if square == &src_square {
                // Open The Door
                match dir {
                    MoveDirection::North => {
                        square.north_wall = WallType::Clear;
                    }
                    MoveDirection::East => {
                        square.east_wall = WallType::Clear;
                    }
                    MoveDirection::South => {
                        square.south_wall = WallType::Clear;
                    }
                    MoveDirection::West => {
                        square.west_wall = WallType::Clear;
                    }
                }
                return Ok(());
            }
        }
        return Err(anyhow!(
            "When opening a door, we expect the square to have a door to open"
        ));
    }

    /// In addition to rotating a new tile, we also need to open/close any doors
    /// it has that align with existing doors.
    /// * This takes an index into self.tiles, because it can only operate on a
    /// tile that has already been added to self.tiles
    fn update_tile_doors(&mut self, tile_idx: usize) -> () {
        let grid = self.get_absolute_grid();
        let tile = &self.game_state.tiles[tile_idx];

        for (dir, position) in tile.adjacent_entrances() {
            match grid.get(&position) {
                Some(neighbor_square) => {
                    let my_door_pos = position.move_in_direction(&dir.opposite());
                    let my_square = grid.get(&my_door_pos).unwrap();

                    let mut_tile = &mut self.game_state.tiles[tile_idx];
                    if my_square.has_door() {
                        if neighbor_square.has_door() {
                            mut_tile.open_door_in_dir(dir);
                            let (idx, mut neighbor_tile) =
                                self.get_tile_with_index(&position).unwrap();
                            neighbor_tile.open_door_in_dir(dir.opposite());
                            self.game_state.tiles[idx] = neighbor_tile;
                        } else {
                            // If there isn't a door on the other side, close door
                            // that way, we know it won't be a possible_placement
                            mut_tile.close_door_in_dir(dir);
                        }
                    } else {
                        // If my square does NOT have a door, but neighbor does
                        if neighbor_square.has_door() {
                            let (idx, mut neighbor_tile) =
                                self.get_tile_with_index(&position).unwrap();
                            neighbor_tile.close_door_in_dir(dir.opposite());
                            self.game_state.tiles[idx] = neighbor_tile;
                        }
                    }
                }
                None => {}
            }
        }
    }

    fn process_tile_placement(&mut self, pt: PlaceTile, player_name: &PlayerName) -> MoveValidity {
        if !self.player_has_ability(player_name, &Ability::RevealTiles) {
            return MoveValidity::Invalid("You cannot reveal tiles".to_string());
        }
        let grid = self.get_absolute_grid();
        let heister_to_tile_entrance_locs = self.heister_to_tile_entrance_positions(&grid);
        let maybe_heister_pos_tuple = heister_to_tile_entrance_locs
            .iter()
            .find(|&(_, te)| te == &pt.tile_entrance);
        let heister_pos = match maybe_heister_pos_tuple {
            Some(pos_tuple) => pos_tuple.0,
            None => {
                return MoveValidity::Invalid(
                    "When placing a tile, the heister must be at a tile-reveal door".to_string(),
                );
            }
        };

        let heister_square = grid
            .get(heister_pos)
            .expect("Heister must be on valid square");
        let dir = &Self::get_door_direction(heister_square)
            .expect("Heister must be on a square with a door");

        match self.open_door(heister_pos.clone(), *heister_square, dir) {
            Ok(_) => self.place_tile(&pt.tile_entrance, dir),
            Err(e) => {
                let msg = format!("Couldn't open door for newly placed tile: {}", e);
                MoveValidity::Invalid(msg.to_string())
            }
        }
    }

    pub fn handle_message(
        &mut self,
        message: MainMessage,
        player_name: &PlayerName,
    ) -> MoveValidity {
        // If we receive GameState or InvalidRequest at this endpoint, panic, it should never happen.
        info!("Received message: {:?}", message);
        let body = message.body.unwrap();
        let validity = match body {
            Body::StartGame(_) => self.start_game(),
            Body::Move(m) => self.process_move(Move::from_proto(m), &player_name),
            Body::PlaceTile(pt) => {
                self.process_tile_placement(PlaceTile::from_proto(pt), &player_name)
            }
            Body::GameState(_gs) => {
                MoveValidity::Invalid("GameState Message is invalid from players".to_string())
            }
            Body::InvalidRequest(_ir) => {
                MoveValidity::Invalid("InvalidRequest Message is invalid from players".to_string())
            }
            Body::Chat(_c) => MoveValidity::Valid,
        };
        self.update_auxiliary_state();
        validity
    }
}

// TODO When we make it that games can't start until 2 - 8 players have joined,
// remove the matches on 0 and 1.
fn get_player_abilities(num_players: usize) -> Vec<Vec<Ability>> {
    let mut player_abilities = match num_players {
        0 | 1 => vec![vec![
            Ability::MoveNorth,
            Ability::MoveEast,
            Ability::MoveSouth,
            Ability::MoveWest,
            Ability::Teleport,
            Ability::RevealTiles,
            Ability::UseEscalator,
        ]],
        2 => vec![
            vec![Ability::MoveNorth, Ability::MoveEast, Ability::Teleport],
            vec![
                Ability::MoveSouth,
                Ability::MoveWest,
                Ability::RevealTiles,
                Ability::UseEscalator,
            ],
        ],
        3 => vec![
            vec![Ability::MoveNorth, Ability::MoveEast],
            vec![
                Ability::MoveSouth,
                Ability::RevealTiles,
                Ability::UseEscalator,
            ],
            vec![Ability::MoveWest, Ability::Teleport],
        ],
        4 => vec![
            vec![Ability::MoveNorth],
            vec![Ability::MoveEast, Ability::UseEscalator],
            vec![Ability::MoveSouth, Ability::RevealTiles],
            vec![Ability::MoveWest, Ability::Teleport],
        ],
        5 => vec![
            vec![Ability::MoveNorth],
            vec![Ability::MoveEast, Ability::UseEscalator],
            vec![Ability::MoveSouth, Ability::RevealTiles],
            vec![Ability::MoveWest],
            vec![Ability::MoveWest, Ability::Teleport],
        ],
        6 => vec![
            vec![Ability::MoveNorth],
            vec![Ability::MoveEast],
            vec![Ability::MoveEast, Ability::UseEscalator],
            vec![Ability::MoveSouth, Ability::RevealTiles],
            vec![Ability::MoveWest],
            vec![Ability::MoveWest, Ability::Teleport],
        ],
        7 => vec![
            vec![Ability::MoveNorth],
            vec![Ability::MoveEast],
            vec![Ability::MoveEast, Ability::UseEscalator],
            vec![Ability::MoveSouth],
            vec![Ability::MoveSouth, Ability::RevealTiles],
            vec![Ability::MoveWest],
            vec![Ability::MoveWest, Ability::Teleport],
        ],
        8 => vec![
            vec![Ability::MoveNorth],
            vec![Ability::MoveNorth],
            vec![Ability::MoveEast],
            vec![Ability::MoveEast, Ability::UseEscalator],
            vec![Ability::MoveSouth],
            vec![Ability::MoveSouth, Ability::RevealTiles],
            vec![Ability::MoveWest],
            vec![Ability::MoveWest, Ability::Teleport],
        ],
        wildcard => panic!("Invalid number of players somehow: {}", wildcard),
    };
    let mut rng = thread_rng();
    player_abilities.shuffle(&mut rng);
    player_abilities
}

#[cfg(test)]
#[allow(dead_code, unused_imports)]
pub mod tests {
    use super::{Game, GameHandle, GameOptions, MoveValidity};
    use crate::types::{
        Heister, HeisterColor, Internal, MainMessage, MapPosition, Move, MoveDirection, Player,
        PlayerName, Square, WallType, HEISTER_COLORS,
    };
    use log::{info, warn};
    use std::collections::HashMap;

    lazy_static! {
        static ref FAKE_PLAYER_NAME: PlayerName = PlayerName("fake name".to_string());
    }

    fn setup_game(handle: String) -> Game {
        let _ = env_logger::builder().is_test(true).try_init();
        let game_handle = GameHandle(handle);
        let game_options = GameOptions::default();
        let mut game = super::Game::new(game_handle, game_options);
        game.add_player(FAKE_PLAYER_NAME.0.clone()).unwrap();
        game.start_game();
        game
    }

    /// In-place movement for heisters, to cause game state to update
    fn move_heister_in_place(game: &mut Game, heister_color: HeisterColor) -> MoveValidity {
        let heister_pos = &game
            .get_heister_from_vec(heister_color)
            .unwrap()
            .map_position;
        let test_move = super::Move {
            heister_color,
            position: heister_pos.clone(),
        };
        let message = MainMessage {
            body: Some(super::Body::Move(test_move.to_proto())),
        };
        let validity = game.handle_message(message, &FAKE_PLAYER_NAME);
        assert_eq!(validity, MoveValidity::Valid);
        validity
    }

    /// Adjacent square movement for heisters, to make testing easier
    /// Asserts that move was valid & that position is correct for valid move
    fn move_heister_in_dir(
        game: &mut Game,
        heister_color: HeisterColor,
        dir: MoveDirection,
        expected_validity: MoveValidity,
    ) -> MoveValidity {
        let heister_pos = &game
            .get_heister_from_vec(heister_color)
            .unwrap()
            .map_position;
        let position = match dir {
            MoveDirection::North => MapPosition {
                x: heister_pos.x,
                y: heister_pos.y - 1,
            },
            MoveDirection::East => MapPosition {
                x: heister_pos.x + 1,
                y: heister_pos.y,
            },
            MoveDirection::South => MapPosition {
                x: heister_pos.x,
                y: heister_pos.y + 1,
            },
            MoveDirection::West => MapPosition {
                x: heister_pos.x - 1,
                y: heister_pos.y,
            },
        };
        let test_move = super::Move {
            heister_color,
            position: position.clone(),
        };
        let message = MainMessage {
            body: Some(super::Body::Move(test_move.to_proto())),
        };
        let validity = game.handle_message(message, &FAKE_PLAYER_NAME);
        assert_eq!(validity, expected_validity);
        match validity.clone() {
            MoveValidity::Valid => {
                let curr_heister_pos = &game
                    .get_heister_from_vec(heister_color)
                    .unwrap()
                    .map_position;
                assert_eq!(curr_heister_pos, &position);
            }
            _invalid => {}
        }
        validity
    }

    /// TODO: must be generalized for any tile placement
    /// currently only works for initial second tile Orange North tile 1a placement
    fn place_first_tile_for_color(
        game: &mut Game,
        _heister_color: HeisterColor,
        tile_entrance: MapPosition,
        expected_validity: MoveValidity,
    ) -> MoveValidity {
        // needs to assert that heister color is correct, etc. or not! i don't care
        let tile_placement = super::PlaceTile { tile_entrance };
        let message = MainMessage {
            body: Some(super::Body::PlaceTile(tile_placement.to_proto())),
        };
        let validity = game.handle_message(message, &FAKE_PLAYER_NAME);
        assert_eq!(validity, expected_validity);

        for tile in &game.game_state.tiles {
            if tile.name == "1a".to_string() {
                let mp_00 = MapPosition { x: 0, y: 0 };
                assert_eq!(tile.position, mp_00);
            } else {
                // No matter the tile name, if we use this path to draw it, its
                // position should be here.
                let mp_1neg3 = MapPosition { x: 1, y: -4 };
                assert_eq!(tile.position, mp_1neg3);
            }
        }
        validity
    }

    /// Assuming that Yellow starts at 1, 1
    /// This test tries to move it up (safe),
    /// Then back down to its starting square
    /// Checks that the moves are accepted as valid
    #[test]
    pub fn test_can_move_to_free_square() -> () {
        let handle = "test can move to free square".to_string();
        let mut game = setup_game(handle);
        let _ = env_logger::builder().is_test(true).try_init();

        // Confirm yellow heister is where we expect it to be to begin with.
        let heister_color = super::HeisterColor::Purple;
        let heister_pos = &game
            .get_heister_from_vec(heister_color)
            .unwrap()
            .map_position;
        assert_eq!(heister_pos.x, 1);
        assert_eq!(heister_pos.y, 1);

        // Move Yellow Up into a free space
        let validity = move_heister_in_dir(
            &mut game,
            heister_color,
            MoveDirection::North,
            MoveValidity::Valid,
        );
        assert_eq!(validity, super::MoveValidity::Valid);

        // Move Yellow back down into the space it occupied - that should be safe
        let validity = move_heister_in_dir(
            &mut game,
            heister_color,
            MoveDirection::South,
            MoveValidity::Valid,
        );
        assert_eq!(validity, super::MoveValidity::Valid);
    }

    #[test]
    pub fn heister_collision_is_invalid() -> () {
        let handle = "heister collision is invalid".to_string();
        let mut game = setup_game(handle);
        // Assuming that Green starts at 1, 1 and Orange at 2, 1
        // This test tries to move it up and expects an invalid move
        // because Orange is there

        // Confirm green heister is where we expect it to be to begin with.
        let src_position = super::MapPosition { x: 2, y: 2 };
        let heister_color = super::HeisterColor::Green;
        let heister_pos = &game
            .get_heister_from_vec(heister_color)
            .unwrap()
            .map_position;
        assert_eq!(heister_pos.x, src_position.x);
        assert_eq!(heister_pos.y, src_position.y);

        let dest_position = super::MapPosition { x: 2, y: 1 };
        let expected_msg = format!(
            "Heister {:?} is on {:?}",
            HeisterColor::Orange,
            dest_position
        );
        let expected_validity = super::MoveValidity::Invalid(expected_msg);
        move_heister_in_dir(
            &mut game,
            HeisterColor::Green,
            MoveDirection::North,
            expected_validity,
        );
        let curr_green_pos = game
            .get_heister_from_vec(super::HeisterColor::Green)
            .unwrap();
        assert_eq!(&curr_green_pos.map_position, &src_position);
    }

    #[test]
    pub fn grid_walls_align() -> () {
        let handle = "grid walls align".to_string();
        let game = setup_game(handle);
        let grid: HashMap<MapPosition, Square> = game.get_absolute_grid();

        for (mp, square) in grid.iter() {
            // Check left wall lines up.
            if mp.x > 0 {
                let index = MapPosition {
                    x: mp.x - 1,
                    y: mp.y,
                };
                let msg = format!("Map tile {},{} not found", &mp.x, &mp.y);
                let left = grid.get(&index).expect(&msg);
                assert_eq!(square.west_wall, left.east_wall);
            }
            // Check right wall lines up.
            if mp.x < 3 {
                let index = MapPosition {
                    x: mp.x + 1,
                    y: mp.y,
                };
                let msg = format!("Map tile {},{} not found", &mp.x, &mp.y);
                let right = grid.get(&index).expect(&msg);
                assert_eq!(square.east_wall, right.west_wall);
            }
            // Check top wall lines up.
            if mp.y > 0 {
                let index = MapPosition {
                    x: mp.x,
                    y: mp.y - 1,
                };
                let msg = format!("Map tile {},{} not found", &mp.x, &mp.y);
                let above = grid.get(&index).expect(&msg);
                assert_eq!(square.north_wall, above.south_wall);
            }
            // Check bottom wall lines up.
            if mp.y < 3 {
                let index = MapPosition {
                    x: mp.x,
                    y: mp.y + 1,
                };
                let msg = format!("Map tile {},{} not found", &mp.x, &mp.y);
                let below = grid.get(&index).expect(&msg);
                assert_eq!(square.south_wall, below.north_wall);
            }
        }
        info!("All walls line up");
    }

    /// We test with initial game state (1a), move Orange one square north,
    /// and then send a drawTile message.
    #[test]
    pub fn test_tile_draw() -> () {
        let handle = "grid walls align".to_string();
        let mut game = setup_game(handle);
        let first_tile_entrance = MapPosition { x: 2, y: -1 };

        move_heister_in_dir(
            &mut game,
            HeisterColor::Orange,
            MoveDirection::North,
            MoveValidity::Valid,
        );
        place_first_tile_for_color(
            &mut game,
            HeisterColor::Orange,
            first_tile_entrance,
            MoveValidity::Valid,
        );
    }

    /// Ensure that we generate possible placements that are correct for the color
    /// of heister & door.
    #[test]
    pub fn possible_placements_no_mismatched_results() -> () {
        let handle = "possible placements no mismatched results".to_string();
        let mut game = setup_game(handle);
        // Set up the game such that many heisters are at matching doors

        // Set up correct, happy, matching case first:
        let orange_door_pos = MapPosition { x: 2, y: 0 };
        let purple_door_pos = MapPosition { x: 0, y: 1 };
        let yellow_door_pos = MapPosition { x: 1, y: 3 };
        let green_door_pos = MapPosition { x: 3, y: 2 };
        let mut color_to_door_pos: HashMap<HeisterColor, MapPosition> = HashMap::new();
        color_to_door_pos.insert(HeisterColor::Orange, orange_door_pos);
        color_to_door_pos.insert(HeisterColor::Purple, purple_door_pos);
        color_to_door_pos.insert(HeisterColor::Yellow, yellow_door_pos);
        color_to_door_pos.insert(HeisterColor::Green, green_door_pos);

        let mut heisters: Vec<Heister> = Vec::new();
        for hc in HEISTER_COLORS.iter() {
            let mut h = Heister::default();
            h.heister_color = *hc.clone();
            h.map_position = color_to_door_pos.get(hc).unwrap().clone();
            heisters.push(h);
        }

        // Move heister in place
        game.game_state.heisters = heisters;
        let dest_position = super::MapPosition { x: 2, y: 0 };
        let test_move = super::Move {
            heister_color: HeisterColor::Orange,
            position: dest_position,
        };
        let message = MainMessage {
            body: Some(super::Body::Move(test_move.to_proto())),
        };
        game.handle_message(message, &FAKE_PLAYER_NAME); // don't care if this move is valid

        let pp = game.game_state.possible_placements;
        assert_eq!(pp.len(), 4);
        // TODO: assert the positions in PP are as expected, this is annoying
        // because PP is the tile entrance, not the heister pos.
        // could short circuit it by directly calling the functioning returning the
        // dict?
    }

    /// We test with initial game state (1a), move Orange one square north,
    /// and then send a drawTile message.
    #[test]
    pub fn test_new_tile_crossing() -> () {
        let handle = "new tile crossing".to_string();
        let mut game = setup_game(handle);
        let first_tile_entrance = MapPosition { x: 2, y: -1 };

        move_heister_in_dir(
            &mut game,
            HeisterColor::Orange,
            MoveDirection::North,
            MoveValidity::Valid,
        );
        place_first_tile_for_color(
            &mut game,
            HeisterColor::Orange,
            first_tile_entrance,
            MoveValidity::Valid,
        );

        // Next, we want to move orange UP, then down.
        move_heister_in_dir(
            &mut game,
            HeisterColor::Orange,
            MoveDirection::North,
            MoveValidity::Valid,
        );
        move_heister_in_dir(
            &mut game,
            HeisterColor::Orange,
            MoveDirection::South,
            MoveValidity::Valid,
        );
    }

    // Ensure that a player with no abilities can't do anything.
    #[test]
    pub fn test_ability_check() -> () {
        let handle = "new tile crossing".to_string();
        let mut game = setup_game(handle);
        game.game_state.players[0].abilities = vec![];
        let first_tile_entrance = MapPosition { x: 2, y: -1 };

        move_heister_in_dir(
            &mut game,
            HeisterColor::Orange,
            MoveDirection::North,
            MoveValidity::Invalid("You cannot move heisters North".to_string()),
        );
        place_first_tile_for_color(
            &mut game,
            HeisterColor::Orange,
            first_tile_entrance,
            MoveValidity::Invalid("You cannot reveal tiles".to_string()),
        );
        move_heister_in_dir(
            &mut game,
            HeisterColor::Orange,
            MoveDirection::North,
            MoveValidity::Invalid("You cannot move heisters North".to_string()),
        );
        move_heister_in_dir(
            &mut game,
            HeisterColor::Orange,
            MoveDirection::South,
            MoveValidity::Invalid("You cannot move heisters South".to_string()),
        );
    }
}
