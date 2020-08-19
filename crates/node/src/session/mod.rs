mod types;
pub use types::*;

use lazy_static::lazy_static;
use parking_lot::RwLockWriteGuard;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use flo_net::packet::{FloPacket, Frame, OptionalFieldExt};
use flo_net::proto::flo_node::{
  ClientConnectRejectReason, ControllerCreateGameRejectReason, Game, PacketClientConnect,
  PacketClientConnectAccept, PacketClientConnectReject, PacketControllerConnectReject,
  PacketControllerCreateGame, PacketControllerCreateGameAccept, PacketControllerCreateGameReject,
};

use crate::error::*;
use crate::metrics;

#[derive(Debug)]
pub struct SessionStore {
  pending_players: PendingPlayerRegistry,
  connected_players: RwLock<HashMap<i32, ConnectedPlayer>>,
  games: GameRegistry,
}

impl SessionStore {
  pub fn get() -> &'static SessionStore {
    lazy_static! {
      static ref INSTANCE: SessionStore = SessionStore::new();
    }

    &INSTANCE
  }

  fn new() -> Self {
    SessionStore {
      pending_players: PendingPlayerRegistry::new(),
      connected_players: RwLock::new(HashMap::new()),
      games: GameRegistry::new(),
    }
  }

  pub fn handle_controller_create_game(&self, packet: PacketControllerCreateGame) -> Result<Frame> {
    let game = packet.game.extract()?;
    let game_id = game.id;
    let player_ids: Vec<i32> = game
      .slots
      .iter()
      .filter_map(|s| s.player.as_ref().map(|p| p.player_id))
      .collect();

    if player_ids.is_empty() {
      return Err(Error::NoPlayer);
    }

    {
      let connected_players_guard = self.connected_players.read();
      for id in &player_ids {
        if connected_players_guard.contains_key(id) {
          return Err(Error::PlayerBusy(*id));
        }
      }
    }

    let mut g_guard = self.games.lock();

    if let Err(err) = g_guard.pre_check(game_id) {
      let reason = match err {
        Error::GameExists => ControllerCreateGameRejectReason::GameExists,
        Error::PlayerBusy(_) => ControllerCreateGameRejectReason::PlayerBusy,
        err => return Err(err),
      };
      return Ok(
        PacketControllerCreateGameReject {
          game_id,
          reason: reason.into(),
        }
        .encode_as_frame()?,
      );
    }

    let pending: Vec<(PlayerToken, PendingPlayer)> = {
      let players: Vec<_> = game
        .slots
        .iter()
        .filter_map(|s| s.player.as_ref())
        .collect();
      players
        .iter()
        .map(|p| {
          (
            PlayerToken::new(),
            PendingPlayer {
              player_id: p.player_id,
              game_id,
            },
          )
        })
        .collect()
    };

    let player_tokens: Vec<_> = pending
      .iter()
      .map(|(token, player)| flo_net::proto::flo_node::PlayerToken {
        player_id: player.player_id,
        token: token.to_vec(),
      })
      .collect();

    let stale_pending_players = self.pending_players.register(pending);
    if !stale_pending_players.is_empty() {
      for player in stale_pending_players {
        tracing::warn!(
          "stale player: player_id = {}, game_id = {}",
          player.player_id,
          player.game_id
        );
      }
    }

    g_guard.register(game)?;

    metrics::GAME_SESSIONS.inc();
    metrics::PENDING_PLAYER_TOKENS.add(player_tokens.len() as i64);

    Ok(
      PacketControllerCreateGameAccept {
        game_id,
        player_tokens,
      }
      .encode_as_frame()?,
    )
  }

  pub fn handle_client_connect() {}
}

#[derive(Debug)]
struct PendingPlayerRegistry {
  map: Arc<RwLock<HashMap<PlayerToken, PendingPlayer>>>,
  player_token: Mutex<HashMap<i32, PlayerToken>>,
}

impl PendingPlayerRegistry {
  fn new() -> Self {
    PendingPlayerRegistry {
      map: Arc::new(RwLock::new(HashMap::new())),
      player_token: Mutex::new(HashMap::new()),
    }
  }

  // for controller
  fn register(&self, pairs: Vec<(PlayerToken, PendingPlayer)>) -> Vec<PendingPlayer> {
    use std::collections::hash_map::Entry;

    let mut player_token_guard = self.player_token.lock();
    let mut map_guard = self.map.write();

    let mut stale_players = vec![];

    for (token, player) in pairs {
      let player_id = player.player_id;
      // remove stale player
      let stale_player = if player_token_guard.contains_key(&player_id) {
        match player_token_guard.entry(player_id) {
          // replace existing token
          Entry::Occupied(mut e) => {
            let r = e.get_mut();
            let prev_token = std::mem::replace(r, token.clone());
            map_guard.remove(&prev_token)
          }
          // add token
          Entry::Vacant(e) => {
            e.insert(token.clone());
            metrics::PENDING_PLAYER_TOKENS.inc();
            None
          }
        }
      } else {
        None
      };

      map_guard.insert(token.clone(), player);

      if let Some(stale_player) = stale_player {
        stale_players.push(stale_player)
      }
    }

    stale_players
  }
}

#[derive(Debug)]
struct GameRegistry {
  map: RwLock<HashMap<i32, Arc<RwLock<GameSession>>>>,
}

impl GameRegistry {
  fn new() -> Self {
    GameRegistry {
      map: RwLock::new(HashMap::new()),
    }
  }

  fn lock(&self) -> GameRegistryGuard {
    GameRegistryGuard {
      guard: self.map.write(),
    }
  }
}

#[derive(Debug)]
struct GameRegistryGuard<'a> {
  guard: RwLockWriteGuard<'a, HashMap<i32, Arc<RwLock<GameSession>>>>,
}

impl<'a> GameRegistryGuard<'a> {
  // for controller
  fn pre_check(&mut self, game_id: i32) -> Result<()> {
    if let Some(game) = self.guard.get(&game_id) {
      if game.read().status != GameStatus::Created {
        return Err(Error::GameExists);
      }
    }

    Ok(())
  }

  // for controller
  fn register(&mut self, game: Game) -> Result<()> {
    let game_id = game.id;

    let session = Arc::new(RwLock::new(GameSession::new(game)?));

    if self.guard.insert(game_id, session).is_none() {
      metrics::GAME_SESSIONS.inc();
    }

    Ok(())
  }
}