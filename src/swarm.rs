use blake3::Hasher;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cert::CertBuilder;
use crate::chain::DyoloChain;
use crate::error::A1Error;
use crate::identity::Signer;
use crate::identity::narrowing::NarrowingMatrix;
use crate::passport::DyoloPassport;

const DOMAIN_SWARM_BIND: &str = "a1::dyolo::swarm::bind::v2.8.0";
const DOMAIN_SWARM_ROLE: &str = "a1::dyolo::swarm::role::v2.8.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwarmRole {
    Orchestrator,
    Auditor,
    Worker { capabilities: Vec<String> },
    Supervisor { capabilities: Vec<String>, max_worker_ttl_secs: u64 },
    Custom { name: String, capabilities: Vec<String>, can_delegate: bool, max_delegation_depth: u8 },
}

impl SwarmRole {
    pub fn name(&self) -> &str {
        match self {
            Self::Orchestrator => "orchestrator",
            Self::Auditor => "auditor",
            Self::Worker { .. } => "worker",
            Self::Supervisor { .. } => "supervisor",
            Self::Custom { name, .. } => name,
        }
    }
    pub fn can_delegate(&self) -> bool {
        matches!(self, Self::Orchestrator | Self::Supervisor { .. } | Self::Custom { can_delegate: true, .. })
    }
    pub fn max_depth(&self) -> u8 {
        match self { Self::Orchestrator => 16, Self::Supervisor { .. } => 4, Self::Custom { max_delegation_depth, .. } => *max_delegation_depth, _ => 0 }
    }
    pub fn capabilities(&self) -> &[String] {
        match self { Self::Worker { capabilities } | Self::Supervisor { capabilities, .. } | Self::Custom { capabilities, .. } => capabilities, _ => &[] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMember {
    pub agent_did: String,
    pub agent_pk_hex: String,
    pub role: SwarmRole,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert_fingerprint_hex: Option<String>,
    #[serde(with = "hex_32")]
    pub role_commitment: [u8; 32],
}

impl SwarmMember {
    fn compute_role_commitment(role: &SwarmRole, agent_did: &str, issued_at: u64) -> [u8; 32] {
        let mut h = Hasher::new_derive_key(DOMAIN_SWARM_ROLE);
        h.update(role.name().as_bytes());
        for cap in role.capabilities() {
            h.update(&(cap.len() as u64).to_le_bytes());
            h.update(cap.as_bytes());
        }
        h.update(&[role.can_delegate() as u8]);
        h.update(&(role.max_depth() as u64).to_le_bytes());
        h.update(agent_did.as_bytes());
        h.update(&issued_at.to_le_bytes());
        h.finalize().into()
    }
    pub fn verify_role_commitment(&self) -> bool {
        Self::compute_role_commitment(&self.role, &self.agent_did, self.issued_at_unix) == self.role_commitment
    }
    pub fn is_expired(&self, now_unix: u64) -> bool { now_unix >= self.expires_at_unix }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmPassport {
    pub root_passport: DyoloPassport,
    pub swarm_name: String,
    #[serde(with = "hex_32")]
    pub swarm_id: [u8; 32],
    pub members: HashMap<String, SwarmMember>,
}

impl SwarmPassport {
    pub fn new(root_passport: DyoloPassport, swarm_name: impl Into<String>) -> Self {
        let name: String = swarm_name.into();
        let swarm_id = compute_swarm_id(&name, root_passport.capability_mask.as_bytes());
        Self { root_passport, swarm_name: name, swarm_id, members: HashMap::new() }
    }

    pub fn add_member(
        &mut self, agent_pk: VerifyingKey, role: SwarmRole, ttl_secs: u64,
        orchestrator: &dyn Signer, clock: &dyn crate::chain::Clock,
    ) -> Result<SwarmMember, A1Error> {
        let now = clock.unix_now();
        let agent_did = format!("did:a1:{}", hex::encode(agent_pk.as_bytes()));
        let cap_refs: Vec<&str> = role.capabilities().iter().map(String::as_str).collect();
        let role_mask = if cap_refs.is_empty() { NarrowingMatrix::EMPTY } else { NarrowingMatrix::from_capabilities(&cap_refs) };
        role_mask.enforce_narrowing(&self.root_passport.capability_mask)?;
        let scope_root = self.root_passport.scope_root()?;
        let cert = CertBuilder::new(agent_pk, scope_root, now, now + ttl_secs).max_depth(role.max_depth()).sign(orchestrator);
        let fingerprint_hex = cert.fingerprint_hex();
        let role_commitment = SwarmMember::compute_role_commitment(&role, &agent_did, now);
        let member = SwarmMember { agent_did: agent_did.clone(), agent_pk_hex: hex::encode(agent_pk.as_bytes()), role, issued_at_unix: now, expires_at_unix: now + ttl_secs, cert_fingerprint_hex: Some(fingerprint_hex), role_commitment };
        self.members.insert(agent_did, member.clone());
        Ok(member)
    }

    pub fn chain_for_member(&self, agent_pk: &VerifyingKey, orchestrator: &dyn Signer, clock: &dyn crate::chain::Clock, ttl_secs: u64) -> Result<DyoloChain, A1Error> {
        let agent_did = format!("did:a1:{}", hex::encode(agent_pk.as_bytes()));
        let member = self.members.get(&agent_did).ok_or_else(|| A1Error::WireFormatError(format!("agent {agent_did} is not a swarm member")))?;
        let now = clock.unix_now();
        let scope_root = self.root_passport.scope_root()?;
        let cert = CertBuilder::new(*agent_pk, scope_root, now, now + ttl_secs).max_depth(member.role.max_depth()).sign(orchestrator);
        let mut chain = DyoloChain::new(orchestrator.verifying_key(), scope_root);
        chain.push(cert);
        Ok(chain)
    }

    pub fn active_members(&self, now_unix: u64) -> Vec<&SwarmMember> {
        self.members.values().filter(|m| !m.is_expired(now_unix)).collect()
    }
    pub fn evict_expired(&mut self, now_unix: u64) -> usize {
        let before = self.members.len();
        self.members.retain(|_, m| !m.is_expired(now_unix));
        before - self.members.len()
    }
    pub fn remove_member(&mut self, did: &str) -> Option<SwarmMember> { self.members.remove(did) }
    pub fn member_count(&self) -> usize { self.members.len() }
    pub fn swarm_id_hex(&self) -> String { hex::encode(self.swarm_id) }
    pub fn has_role(&self, agent_did: &str, now_unix: u64, role_name: &str) -> bool {
        self.members.get(agent_did).map(|m| !m.is_expired(now_unix) && m.role.name() == role_name).unwrap_or(false)
    }
}

fn compute_swarm_id(name: &str, mask: &[u8; 32]) -> [u8; 32] {
    let mut h = Hasher::new_derive_key(DOMAIN_SWARM_BIND);
    h.update(&(name.len() as u64).to_le_bytes());
    h.update(name.as_bytes());
    h.update(mask);
    h.finalize().into()
}

mod hex_32 {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[u8; 32], s: S) -> Result<S::Ok, S::Error> { s.serialize_str(&hex::encode(v)) }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let h = String::deserialize(d)?;
        hex::decode(&h).map_err(serde::de::Error::custom)?.try_into().map_err(|_| serde::de::Error::custom("expected 32 bytes"))
    }
}