use crate::config::Eth2Config;
use crate::libp2p::types::{EnrBitfield, GossipEncoding, GossipKind, GossipTopic};
use crate::libp2p::NetworkConfig;
use crate::ssz::types::BitVector;
use crate::ssz::{Decode, Encode};
use crate::testnet::config::Eth2TestnetConfig;
use crate::types::{ChainSpec, EnrForkId, EthSpec, Hash256, MainnetEthSpec, Slot};
use libp2p_core::{identity::Keypair, identity::PublicKey, multiaddr::Protocol, Multiaddr, PeerId};
#[cfg(not(feature = "local"))]
use discv5::enr::{CombinedKey, CombinedPublicKey, Enr};
#[cfg(feature = "local")]
use discv5_local::enr::{CombinedKey, CombinedPublicKey, Enr};

use std::path::PathBuf;

pub fn load_testnet_config<E: EthSpec>(testnet_dir: PathBuf) -> Eth2TestnetConfig<E> {
    Eth2TestnetConfig::load(testnet_dir).unwrap()
}

pub fn get_eth2_config() -> Eth2Config {
    Eth2Config::mainnet()
}

pub fn get_chain_spec() -> ChainSpec {
    ChainSpec::mainnet()
}

pub fn get_default_fork_id() -> EnrForkId {
    EnrForkId {
        fork_digest: [0; 4],
        next_fork_version: [0; 4],                //genesis_fork_version,
        next_fork_epoch: u64::max_value().into(), //far_future_epoch,
    }
}

pub fn get_fork_id(
    fork_digest: Vec<u8>,
    next_fork_version: Vec<u8>,
    next_fork_epoch: u64,
) -> EnrForkId {
    EnrForkId {
        fork_digest: [
            fork_digest[0],
            fork_digest[1],
            fork_digest[2],
            fork_digest[3],
        ],
        next_fork_version: [
            next_fork_version[0],
            next_fork_version[1],
            next_fork_version[2],
            next_fork_version[3],
        ], //genesis_fork_version,
        next_fork_epoch: next_fork_epoch.into(), //far_future_epoch,
    }
}

pub fn get_fork_id_from_dir(dir: Option<PathBuf>) -> Option<EnrForkId> {
    if let Some(value) = dir {
        let config = load_testnet_config::<MainnetEthSpec>(value);
        let state = config.genesis_state.unwrap();
        let spec = get_chain_spec();
        Some(spec.enr_fork_id(state.slot, state.genesis_validators_root))
    } else {
        None
    }
}

pub fn get_fork_id_from_enr(enr: &Enr<CombinedKey>) -> Option<EnrForkId> {
    match enr.get("eth2") {
        Some(enr_fork_id) => match EnrForkId::from_ssz_bytes(enr_fork_id) {
            Ok(enr_fork_id) => Some(enr_fork_id),
            Err(_e) => None,
        },
        None => None,
    }
}

pub fn get_attnets_from_enr(enr: &Enr<CombinedKey>) -> Vec<u64> {
    let mut attnets = vec![];

    if let Ok(bitfield) = get_bitfield_from_enr(enr) {
        if bitfield.len() > 0 {
            let subnet_count = get_chain_spec().attestation_subnet_count as usize;
            for i in 0..=subnet_count {
                match bitfield.get(i) {
                    Ok(true) => attnets.push(i as u64),
                    _ => (),
                }
            }
        }
    }
    return attnets;
}

pub fn get_bitfield_from_enr(
    enr: &Enr<CombinedKey>,
) -> Result<EnrBitfield<MainnetEthSpec>, &'static str> {
    let bitfield_bytes = enr
        .get("attnets")
        .ok_or_else(|| "ENR bitfield non-existent")?;

    BitVector::<<MainnetEthSpec as EthSpec>::SubnetBitfieldLength>::from_ssz_bytes(bitfield_bytes)
        .map_err(|_| "Could not decode the ENR SSZ bitfield")
}

pub fn get_enr_from_string(enr: String) -> Option<Enr<CombinedKey>> {
    match enr.parse::<Enr<CombinedKey>>() {
        Ok(enr) => Some(enr),
        Err(_e) => None,
    }
}

pub fn get_fork_id_from_string(enr: String) -> Option<EnrForkId> {
    match enr.parse::<Enr<CombinedKey>>() {
        Ok(enr) => get_fork_id_from_enr(&enr),
        Err(_e) => None,
    }
}

pub fn create_topic_ids(enr_fork_id: EnrForkId) -> Vec<String> {
    let network_config = NetworkConfig::default();
    let topic_kinds = network_config.topics; //type GossipKind
    let mut topic_ids: Vec<String> = vec![];
    for kind in topic_kinds {
        let topic_id = GossipTopic::new(kind, GossipEncoding::default(), enr_fork_id.fork_digest);
        topic_ids.push(topic_id.into());
    }
    topic_ids
}

pub fn get_gossip_topic_id(kind: GossipKind, enr_fork_id: EnrForkId) -> String {
    GossipTopic::new(kind, GossipEncoding::default(), enr_fork_id.fork_digest).into()
}


///
/// The following code was "borrowed from "https://github.com/AgeManning/enr-cli/blob/master/src/enr_ext.rs"
/// 

/// Extend ENR for libp2p types.
pub trait EnrExt {
    /// The libp2p `PeerId` for the record.
    fn peer_id(&self) -> PeerId;
}

/// Extend ENR CombinedPublicKey for libp2p types.
pub trait CombinedKeyPublicExt {
    /// Converts the publickey into a peer id, without consuming the key.
    fn into_peer_id(&self) -> PeerId;
}

impl EnrExt for Enr<CombinedKey> {
    /// The libp2p `PeerId` for the record.
    fn peer_id(&self) -> PeerId {
        self.public_key().into_peer_id()
    }
}

impl CombinedKeyPublicExt for CombinedPublicKey {
    /// Converts the publickey into a peer id, without consuming the key.
    ///
    /// This is only available with the `libp2p` feature flag.
    fn into_peer_id(&self) -> PeerId {
        match self {
            Self::Secp256k1(pk) => {
                let pk_bytes = pk.serialize_compressed();
                let libp2p_pk = PublicKey::Secp256k1(
                    libp2p_core::identity::secp256k1::PublicKey::decode(&pk_bytes)
                        .expect("valid public key"),
                );
                PeerId::from_public_key(libp2p_pk)
            }
            Self::Ed25519(pk) => {
                let pk_bytes = pk.to_bytes();
                let libp2p_pk = PublicKey::Ed25519(
                    libp2p_core::identity::ed25519::PublicKey::decode(&pk_bytes)
                        .expect("valid public key"),
                );
                PeerId::from_public_key(libp2p_pk)
            }
        }
    }
}