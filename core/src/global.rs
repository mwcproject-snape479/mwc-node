// Copyright 2020 The Grin Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Values that should be shared across all modules, without necessarily
//! having to pass them all over the place, but aren't consensus values.
//! should be used sparingly.

use crate::consensus::{
	graph_weight, HeaderInfo, BASE_EDGE_BITS, BLOCK_KERNEL_WEIGHT, BLOCK_OUTPUT_WEIGHT,
	BLOCK_TIME_SEC, COINBASE_MATURITY, CUT_THROUGH_HORIZON, DAY_HEIGHT, DEFAULT_MIN_EDGE_BITS,
	DIFFICULTY_ADJUST_WINDOW, INITIAL_DIFFICULTY, MAX_BLOCK_WEIGHT, PROOFSIZE,
	SECOND_POW_EDGE_BITS, STATE_SYNC_THRESHOLD,
};
use crate::pow::{self, new_cuckarood_ctx, new_cuckatoo_ctx, PoWContext};
use crate::ser::ProtocolVersion;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use util::OneTime;

/// An enum collecting sets of parameters used throughout the
/// code wherever mining is needed. This should allow for
/// different sets of parameters for different purposes,
/// e.g. CI, User testing, production values
/// Define these here, as they should be developer-set, not really tweakable
/// by users

/// The default "local" protocol version for this node.
/// We negotiate compatible versions with each peer via Hand/Shake.
/// Note: We also use a specific (possible different) protocol version
/// for both the backend database and MMR data files.
/// NOTE, grin bump the protocol version to 1000, but in any case fo far 1,2,3 are supported.
pub const PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion(3);

/// Automated testing edge_bits
pub const AUTOMATED_TESTING_MIN_EDGE_BITS: u8 = 10;

/// Automated testing proof size
pub const AUTOMATED_TESTING_PROOF_SIZE: usize = 8;

/// User testing edge_bits
pub const USER_TESTING_MIN_EDGE_BITS: u8 = 15;

/// User testing proof size
pub const USER_TESTING_PROOF_SIZE: usize = 42;

/// Automated testing coinbase maturity
pub const AUTOMATED_TESTING_COINBASE_MATURITY: u64 = 3;

/// User testing coinbase maturity
pub const USER_TESTING_COINBASE_MATURITY: u64 = 3;

/// Testing cut through horizon in blocks
pub const AUTOMATED_TESTING_CUT_THROUGH_HORIZON: u32 = 20;

/// Testing cut through horizon in blocks
pub const USER_TESTING_CUT_THROUGH_HORIZON: u32 = 70;

/// Testing state sync threshold in blocks
pub const TESTING_STATE_SYNC_THRESHOLD: u32 = 20;

/// Testing initial graph weight
pub const TESTING_INITIAL_GRAPH_WEIGHT: u32 = 1;

/// Testing initial block difficulty
pub const TESTING_INITIAL_DIFFICULTY: u64 = 1;

/// Testing max_block_weight (artifically low, just enough to support a few txs).
pub const TESTING_MAX_BLOCK_WEIGHT: u64 = 250;

/// If a peer's last updated difficulty is 2 hours ago and its difficulty's lower than ours,
/// we're sure this peer is a stuck node, and we will kick out such kind of stuck peers.
pub const STUCK_PEER_KICK_TIME: i64 = 2 * 3600 * 1000;

/// If a peer's last seen time is 2 weeks ago we will forget such kind of defunct peers.
const PEER_EXPIRATION_DAYS: i64 = 7 * 2;

/// Constant that expresses defunct peer timeout in seconds to be used in checks.
pub const PEER_EXPIRATION_REMOVE_TIME: i64 = PEER_EXPIRATION_DAYS * 24 * 3600;

/// Trigger compaction check on average every day for all nodes.
/// Randomized per node - roll the dice on every block to decide.
/// Will compact the txhashset to remove pruned data.
/// Will also remove old blocks and associated data from the database.
/// For a node configured as "archival_mode = true" only the txhashset will be compacted.
pub const COMPACTION_CHECK: u64 = DAY_HEIGHT;

/// Number of blocks to reuse a txhashset zip for (automated testing and user testing).
pub const TESTING_TXHASHSET_ARCHIVE_INTERVAL: u64 = 10;

/// Number of blocks to reuse a txhashset zip for.
pub const TXHASHSET_ARCHIVE_INTERVAL: u64 = 12 * 60;

/// Types of chain a server can run with, dictates the genesis block and
/// and mining parameters used.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ChainTypes {
	/// For CI testing
	AutomatedTesting,
	/// For Perf Testing
	PerfTesting,
	/// For User testing
	UserTesting,
	/// Protocol testing network
	Floonet,
	/// Main production network
	Mainnet,
}

impl ChainTypes {
	/// Short name representing the chain type ("floo", "main", etc.)
	pub fn shortname(&self) -> String {
		match *self {
			ChainTypes::AutomatedTesting => "auto".to_owned(),
			ChainTypes::PerfTesting => "perf".to_owned(),
			ChainTypes::UserTesting => "user".to_owned(),
			ChainTypes::Floonet => "floo".to_owned(),
			ChainTypes::Mainnet => "main".to_owned(),
		}
	}
}

impl Default for ChainTypes {
	fn default() -> ChainTypes {
		ChainTypes::Mainnet
	}
}

lazy_static! {
	/// Global chain_type that must be initialized once on node startup.
	/// This is accessed via get_chain_type() which allows the global value
	/// to be overridden on a per-thread basis (for testing).
	pub static ref GLOBAL_CHAIN_TYPE: OneTime<ChainTypes> = OneTime::new();

	/// Global feature flag for NRD kernel support.
	/// If enabled NRD kernels are treated as valid after HF3 (based on header version).
	/// If disabled NRD kernels are invalid regardless of header version or block height.
	pub static ref GLOBAL_NRD_FEATURE_ENABLED: OneTime<bool> = OneTime::new();

	/// Running flag for MWC node.
	pub static ref SERVER_RUNNING: Arc<AtomicBool> =
			Arc::new(AtomicBool::new(true));
}

thread_local! {
	/// Mainnet|Floonet|UserTesting|AutomatedTesting
	pub static CHAIN_TYPE: Cell<Option<ChainTypes>> = Cell::new(None);

	/// Local feature flag for NRD kernel support.
	pub static NRD_FEATURE_ENABLED: Cell<Option<bool>> = Cell::new(None);
}

/// Set the chain type on a per-thread basis via thread_local storage.
pub fn set_local_chain_type(new_type: ChainTypes) {
	CHAIN_TYPE.with(|chain_type| chain_type.set(Some(new_type)))
}

/// Get the chain type via thread_local, fallback to global chain_type.
pub fn get_chain_type() -> ChainTypes {
	CHAIN_TYPE.with(|chain_type| match chain_type.get() {
		None => {
			if GLOBAL_CHAIN_TYPE.is_init() {
				let chain_type = GLOBAL_CHAIN_TYPE.borrow();
				set_local_chain_type(chain_type);
				chain_type
			} else {
				panic!("GLOBAL_CHAIN_TYPE and CHAIN_TYPE unset. Consider set_local_chain_type() in tests.");
			}
		}
		Some(chain_type) => chain_type,
	})
}

/// One time initialization of the global chain_type.
/// Will panic if we attempt to re-initialize this (via OneTime).
pub fn init_global_chain_type(new_type: ChainTypes) {
	GLOBAL_CHAIN_TYPE.init(new_type)
}

/// One time initialization of the global chain_type.
/// Will panic if we attempt to re-initialize this (via OneTime).
pub fn init_global_nrd_enabled(enabled: bool) {
	GLOBAL_NRD_FEATURE_ENABLED.init(enabled)
}

/// Explicitly enable the NRD global feature flag.
pub fn set_local_nrd_enabled(enabled: bool) {
	NRD_FEATURE_ENABLED.with(|flag| flag.set(Some(enabled)))
}

/// Is the NRD feature flag enabled?
/// Look at thread local config first. If not set fallback to global config.
/// Default to false if global config unset.
pub fn is_nrd_enabled() -> bool {
	NRD_FEATURE_ENABLED.with(|flag| match flag.get() {
		None => {
			if GLOBAL_NRD_FEATURE_ENABLED.is_init() {
				let global_flag = GLOBAL_NRD_FEATURE_ENABLED.borrow();
				flag.set(Some(global_flag));
				global_flag
			} else {
				// Global config unset, default to false.
				false
			}
		}
		Some(flag) => flag,
	})
}

/// Return either a cuckoo context or a cuckatoo context
/// Single change point
/// MWC: We modify this to launch with cuckarood only on both floonet and mainnet
pub fn create_pow_context<T>(
	_height: u64,
	edge_bits: u8,
	proof_size: usize,
	max_sols: u32,
) -> Result<Box<dyn PoWContext>, pow::Error> {
	let chain_type = get_chain_type();
	match chain_type {
		// Mainnet has Cuckaroo(d)29 for AR and Cuckatoo31+ for AF
		ChainTypes::Mainnet if edge_bits > 29 => new_cuckatoo_ctx(edge_bits, proof_size, max_sols),
		ChainTypes::Mainnet => new_cuckarood_ctx(edge_bits, proof_size),

		// Same for Floonet
		ChainTypes::Floonet if edge_bits > 29 => new_cuckatoo_ctx(edge_bits, proof_size, max_sols),
		ChainTypes::Floonet => new_cuckarood_ctx(edge_bits, proof_size),

		// Everything else is Cuckatoo only
		_ => new_cuckatoo_ctx(edge_bits, proof_size, max_sols),
	}
}

/// The minimum acceptable edge_bits
pub fn min_edge_bits() -> u8 {
	match get_chain_type() {
		ChainTypes::AutomatedTesting | ChainTypes::PerfTesting => AUTOMATED_TESTING_MIN_EDGE_BITS,
		ChainTypes::UserTesting => USER_TESTING_MIN_EDGE_BITS,
		_ => DEFAULT_MIN_EDGE_BITS,
	}
}

/// Reference edge_bits used to compute factor on higher Cuck(at)oo graph sizes,
/// while the min_edge_bits can be changed on a soft fork, changing
/// base_edge_bits is a hard fork.
pub fn base_edge_bits() -> u8 {
	match get_chain_type() {
		ChainTypes::AutomatedTesting | ChainTypes::PerfTesting => AUTOMATED_TESTING_MIN_EDGE_BITS,
		ChainTypes::UserTesting => USER_TESTING_MIN_EDGE_BITS,
		_ => BASE_EDGE_BITS,
	}
}

/// The proofsize
pub fn proofsize() -> usize {
	match get_chain_type() {
		ChainTypes::AutomatedTesting | ChainTypes::PerfTesting => AUTOMATED_TESTING_PROOF_SIZE,
		ChainTypes::UserTesting => USER_TESTING_PROOF_SIZE,
		_ => PROOFSIZE,
	}
}

/// Coinbase maturity for coinbases to be spent
pub fn coinbase_maturity() -> u64 {
	match get_chain_type() {
		ChainTypes::AutomatedTesting | ChainTypes::PerfTesting => {
			AUTOMATED_TESTING_COINBASE_MATURITY
		}
		ChainTypes::UserTesting => USER_TESTING_COINBASE_MATURITY,
		_ => COINBASE_MATURITY,
	}
}

/// Initial mining difficulty
pub fn initial_block_difficulty() -> u64 {
	match get_chain_type() {
		ChainTypes::AutomatedTesting | ChainTypes::PerfTesting => TESTING_INITIAL_DIFFICULTY,
		ChainTypes::UserTesting => TESTING_INITIAL_DIFFICULTY,
		ChainTypes::Floonet => INITIAL_DIFFICULTY,
		ChainTypes::Mainnet => INITIAL_DIFFICULTY,
	}
}
/// Initial mining secondary scale
pub fn initial_graph_weight() -> u32 {
	match get_chain_type() {
		ChainTypes::AutomatedTesting | ChainTypes::PerfTesting => TESTING_INITIAL_GRAPH_WEIGHT,
		ChainTypes::UserTesting => TESTING_INITIAL_GRAPH_WEIGHT,
		ChainTypes::Floonet => graph_weight(0, SECOND_POW_EDGE_BITS) as u32,
		ChainTypes::Mainnet => graph_weight(0, SECOND_POW_EDGE_BITS) as u32,
	}
}

/// Maximum allowed block weight.
pub fn max_block_weight() -> u64 {
	match get_chain_type() {
		ChainTypes::AutomatedTesting => TESTING_MAX_BLOCK_WEIGHT,
		ChainTypes::PerfTesting => MAX_BLOCK_WEIGHT,
		ChainTypes::UserTesting => TESTING_MAX_BLOCK_WEIGHT,
		ChainTypes::Floonet => MAX_BLOCK_WEIGHT,
		ChainTypes::Mainnet => MAX_BLOCK_WEIGHT,
	}
}

/// Maximum allowed transaction weight (1 weight unit ~= 32 bytes)
pub fn max_tx_weight() -> u64 {
	let coinbase_weight = BLOCK_OUTPUT_WEIGHT + BLOCK_KERNEL_WEIGHT;
	max_block_weight().saturating_sub(coinbase_weight) as u64
}

/// Horizon at which we can cut-through and do full local pruning
pub fn cut_through_horizon() -> u32 {
	match get_chain_type() {
		ChainTypes::AutomatedTesting | ChainTypes::PerfTesting => {
			AUTOMATED_TESTING_CUT_THROUGH_HORIZON
		}
		ChainTypes::UserTesting => USER_TESTING_CUT_THROUGH_HORIZON,
		_ => CUT_THROUGH_HORIZON,
	}
}

/// Threshold at which we can request a txhashset (and full blocks from)
pub fn state_sync_threshold() -> u32 {
	match get_chain_type() {
		ChainTypes::AutomatedTesting | ChainTypes::PerfTesting => TESTING_STATE_SYNC_THRESHOLD,
		ChainTypes::UserTesting => TESTING_STATE_SYNC_THRESHOLD,
		_ => STATE_SYNC_THRESHOLD,
	}
}

/// Number of blocks to reuse a txhashset zip for.
pub fn txhashset_archive_interval() -> u64 {
	match get_chain_type() {
		ChainTypes::AutomatedTesting | ChainTypes::PerfTesting => {
			TESTING_TXHASHSET_ARCHIVE_INTERVAL
		}
		ChainTypes::UserTesting => TESTING_TXHASHSET_ARCHIVE_INTERVAL,
		_ => TXHASHSET_ARCHIVE_INTERVAL,
	}
}

/// Are we in production mode?
/// Production defined as a live public network, testnet[n] or mainnet.
pub fn is_production_mode() -> bool {
	match get_chain_type() {
		ChainTypes::Floonet => true,
		ChainTypes::Mainnet => true,
		_ => false,
	}
}

/// Are we in floonet?
/// Note: We do not have a corresponding is_mainnet() as we want any tests to be as close
/// as possible to "mainnet" configuration as possible.
/// We want to avoid missing any mainnet only code paths.
pub fn is_floonet() -> bool {
	match get_chain_type() {
		ChainTypes::Floonet => true,
		_ => false,
	}
}

/// Are we for real?
pub fn is_mainnet() -> bool {
	match get_chain_type() {
		ChainTypes::Mainnet => true,
		_ => false,
	}
}

/// Get a network name
pub fn get_network_name() -> String {
	let name = match get_chain_type() {
		ChainTypes::AutomatedTesting => "automatedtests",
		ChainTypes::PerfTesting => "perftests",
		ChainTypes::UserTesting => "usertestnet",
		ChainTypes::Floonet => "floonet",
		ChainTypes::Mainnet => "mainnet",
	};
	name.to_string()
}

/// Converts an iterator of block difficulty data to more a more manageable
/// vector and pads if needed (which will) only be needed for the first few
/// blocks after genesis

pub fn difficulty_data_to_vector<T>(cursor: T) -> Vec<HeaderInfo>
where
	T: IntoIterator<Item = HeaderInfo>,
{
	// Convert iterator to vector, so we can append to it if necessary
	let needed_block_count = DIFFICULTY_ADJUST_WINDOW as usize + 1;
	let mut last_n: Vec<HeaderInfo> = cursor.into_iter().take(needed_block_count).collect();

	// Only needed just after blockchain launch... basically ensures there's
	// always enough data by simulating perfectly timed pre-genesis
	// blocks at the genesis difficulty as needed.
	let n = last_n.len();
	if needed_block_count > n {
		let last_ts_delta = if n > 1 {
			last_n[0].timestamp - last_n[1].timestamp
		} else {
			BLOCK_TIME_SEC
		};
		let last_diff = last_n[0].difficulty;

		// fill in simulated blocks with values from the previous real block
		let mut last_ts = last_n.last().unwrap().timestamp;
		for _ in n..needed_block_count {
			last_ts = last_ts.saturating_sub(last_ts_delta);
			last_n.push(HeaderInfo::from_ts_diff(last_ts, last_diff));
		}
	}
	last_n.reverse();
	last_n
}

/// Checking running status if the server
pub fn is_server_running() -> bool {
	SERVER_RUNNING.load(Ordering::SeqCst)
}

/// Request for server stopping
pub fn request_server_stop() {
	SERVER_RUNNING.store(false, Ordering::SeqCst)
}

/// Get an access to the the flag responsible for stopping the server
pub fn get_server_running_controller() -> Arc<AtomicBool> {
	SERVER_RUNNING.clone()
}
