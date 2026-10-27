// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from `tpt-anima-zkpa` to Thought's identity and permission checks (spec section 40;
//! todo.md Phase 6).
//!
//! Two mappings are provided:
//!
//! - Sessions: a task-scoped, expiring credential for an agent, with a compute budget. Thought
//!   opens one per task and revokes it when the task ends. Sessions expire by time as well.
//! - Delegation: a parent agent grants child agents named permissions, and may only grant what it
//!   holds itself. Revoking a delegation takes effect at once.
//! - Attestation: hardware evidence from Anima's platform probes. Anima's `verify_attestation` only
//!   checks the shape of the result (a 32-byte digest and the `verified` flag). It does not check a
//!   platform signature, so [`AnimaIdentity::check_attestation`] names that in its marker.
//! - Proofs: Groth16 proofs of authorization. [`AnimaIdentity::verify_proof`] runs Anima's real
//!   Groth16 check, but against the verifying key embedded in the proof. Anima does not expose a
//!   trusted key, so a valid result shows internal consistency, not a trusted setup.
//!
//! Attestation and proof results are returned as strings for the provenance `verification` field,
//! so Thought's provenance types do not depend on this module.
//!
//! Ownership states and the agency ledger are Thought's, not Anima's. The pinned `tpt-anima-zkpa`
//! has no ownership model: its delegation grants permissions, not responsibility for a thought. So
//! [`AnimaIdentity::transfer_ownership`] takes its authority from the delegations above, and records
//! the change in Thought's [`AgencyLedger`] with the ZKPA marker as the basis. A delegation with the
//! [`TRANSFER_PERMISSION`] lets its holder move a thought the owner has not handed over.

use std::time::Duration;

use tpt_anima_zkpa::{
    AttestationPlatform, AttestationService, DelegationManager, HardwareAttestation,
    ProofGenerator, ProofSystem, SessionManager, ZKCircuit, ZKProof, ZkpaError,
};
use tpt_thought_core::{
    AgencyEntry, AgencyLedger, LedgerError, ThoughtId, ThoughtOwnership, Timestamp,
};
use uuid::Uuid;

/// The permission an owner grants to let another agent move a thought's ownership state.
pub const TRANSFER_PERMISSION: &str = "thought:transfer";

/// Why an identity operation was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum IdentityError {
    /// The compute budget is not a finite, non-negative number.
    InvalidBudget,
    /// The session lifetime is zero, so the session would be expired on creation.
    ZeroLifetime,
    /// Anima refused the operation, such as an empty or over-reaching delegation. Holds Anima's
    /// message, since `ZkpaError` does not implement `PartialEq`.
    Refused(String),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBudget => write!(f, "compute budget must be finite and non-negative"),
            Self::ZeroLifetime => write!(f, "session lifetime must be greater than zero"),
            Self::Refused(reason) => write!(f, "{reason}"),
        }
    }
}

fn refused(error: ZkpaError) -> IdentityError {
    IdentityError::Refused(error.to_string())
}

impl std::error::Error for IdentityError {}

/// Task sessions and permission delegations, held by Anima's ZKPA managers.
#[derive(Default)]
pub struct AnimaIdentity {
    sessions: SessionManager,
    delegations: DelegationManager,
}

impl AnimaIdentity {
    /// No sessions and no delegations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a session for `agent`, scoped to `task`, with `budget` compute units, valid for `ttl`.
    /// Returns the session's identifier.
    pub fn open_session(
        &mut self,
        agent: Uuid,
        task: &str,
        budget: f64,
        ttl: Duration,
    ) -> Result<Uuid, IdentityError> {
        if !budget.is_finite() || budget < 0.0 {
            return Err(IdentityError::InvalidBudget);
        }
        if ttl.is_zero() {
            return Err(IdentityError::ZeroLifetime);
        }
        let session = self
            .sessions
            .create_session(agent, task.to_string(), budget, ttl, None);
        Ok(session.id)
    }

    /// Whether a session exists and has not expired.
    pub fn session_is_valid(&self, session: Uuid) -> bool {
        self.sessions.validate_session(session).is_ok()
    }

    /// Revoke a session. Returns `false` if it was not held.
    pub fn close_session(&mut self, session: Uuid) -> bool {
        self.sessions.revoke_session(session).is_ok()
    }

    /// Grant `child` the named `permissions` from `parent`. Anima refuses an empty list, and a
    /// parent that does not hold a permission cannot delegate it.
    pub fn delegate(
        &mut self,
        parent: Uuid,
        child: Uuid,
        permissions: &[&str],
    ) -> Result<(), IdentityError> {
        let permissions = permissions.iter().map(|p| (*p).to_string()).collect();
        self.delegations
            .delegate(parent, child, permissions, None)
            .map(|_| ())
            .map_err(refused)
    }

    /// Whether `child` currently holds `permission` from `parent`. Revoked delegations do not count.
    pub fn is_authorized(&self, parent: Uuid, child: Uuid, permission: &str) -> bool {
        self.delegations.is_authorized(parent, child, permission)
    }

    /// Revoke the delegation from `parent` to `child`.
    pub fn revoke_delegation(&mut self, parent: Uuid, child: Uuid) -> Result<(), IdentityError> {
        self.delegations.revoke(parent, child).map_err(refused)
    }

    /// Collect hardware evidence for `platform` over `evidence`. Anima probes the platform first, so
    /// success means the probe passed on this machine. Platforms Anima has no backend for (AMD
    /// Secure, AWS Nitro, Azure Confidential) are refused.
    pub fn attest(
        &self,
        platform: AttestationPlatform,
        evidence: &[u8],
    ) -> Result<HardwareAttestation, IdentityError> {
        let service = AttestationService::new();
        let result = match platform {
            AttestationPlatform::TPM => service.attest_tpm(evidence),
            AttestationPlatform::TrustZone => service.attest_trustzone(evidence),
            AttestationPlatform::SGX => service.attest_sgx(evidence),
            AttestationPlatform::NVIDIACC => service.attest_nvidia_cc(evidence),
            AttestationPlatform::AppleSE => service.attest_apple_se(evidence),
            AttestationPlatform::AMDSecure
            | AttestationPlatform::AWSNitro
            | AttestationPlatform::AzureConfidential => {
                return Err(IdentityError::Refused(format!(
                    "tpt-anima-zkpa has no {platform:?} attestation backend"
                )));
            }
        };
        result.map_err(refused)
    }

    /// Check an attestation with Anima's `verify_attestation` and return its provenance marker. The
    /// check is structural only, so the marker says so. It does not prove the platform signed the
    /// evidence.
    pub fn check_attestation(
        &self,
        attestation: &HardwareAttestation,
    ) -> Result<String, IdentityError> {
        AttestationService::new()
            .verify_attestation(attestation)
            .map_err(refused)?;
        Ok(format!(
            "zkpa:attestation:{:?}:structural",
            attestation.platform
        ))
    }

    /// Prove, with a Groth16 proof from Anima, that `agent` holds `permissions`. The proof commits to
    /// the inputs with MiMC, so a verifier learns that a preimage exists but not what it is.
    pub fn prove_authorization(
        &self,
        agent: Uuid,
        permissions: &[&str],
    ) -> Result<ZKProof, IdentityError> {
        let permissions: Vec<String> = permissions.iter().map(|p| (*p).to_string()).collect();
        let circuit = ZKCircuit {
            circuit_type: "authorization".to_string(),
            proof_system: ProofSystem::Groth16,
        };
        ProofGenerator::new()
            .generate_authorization_proof(agent.as_bytes(), &permissions, &circuit)
            .map_err(refused)
    }

    /// Verify a proof with Anima's `ProofGenerator::verify` and return its provenance marker. The
    /// check uses the verifying key and public inputs inside the proof. It does not check the proof
    /// against a key Thought trusts, and it does not bind the proof to a particular agent.
    pub fn verify_proof(&self, proof: &ZKProof) -> Result<String, IdentityError> {
        ProofGenerator::new().verify(proof).map_err(refused)?;
        Ok(format!("zkpa:proof:{}:groth16-verified", proof.proof_type))
    }

    /// Open `thought` in `ledger` with `owner` as its creating agent. The thought starts private.
    pub fn open_ownership(
        &self,
        ledger: &mut AgencyLedger,
        thought: ThoughtId,
        owner: Uuid,
    ) -> Result<(), IdentityError> {
        ledger
            .open(thought, owner.to_string())
            .map_err(ledger_refused)
    }

    /// Move `thought` to `to` on behalf of `actor`, and record the change in `ledger`.
    ///
    /// Authority comes from the delegations held by this adapter:
    ///
    /// - the agent holding the thought may move it: the owner, or the delegate named by a
    ///   `Delegated` state (basis `zkpa:holder`);
    /// - the owner may always move it, which lets it reclaim a delegated thought (`zkpa:owner`);
    /// - any other agent needs a live [`TRANSFER_PERMISSION`] from the owner
    ///   (`zkpa:delegation:thought:transfer`). Revoking it takes effect on the next call.
    ///
    /// A `Delegated` target must be an agent identity (a UUID). The ledger checks the state
    /// transition itself, so a published thought cannot be moved back to private.
    pub fn transfer_ownership(
        &self,
        ledger: &mut AgencyLedger,
        thought: ThoughtId,
        actor: Uuid,
        to: ThoughtOwnership,
        at: Timestamp,
    ) -> Result<(), IdentityError> {
        let owner = ledger
            .owner(thought)
            .and_then(|owner| Uuid::parse_str(owner).ok())
            .ok_or_else(|| {
                IdentityError::Refused(format!("thought {thought:?} has no owner in the ledger"))
            })?;
        // An opened thought always has a current state, so the owner check above covers it.
        let current = ledger.current(thought).unwrap_or_default();
        let holder = match &current {
            ThoughtOwnership::Delegated { to } => Uuid::parse_str(to).map_err(|_| {
                IdentityError::Refused(format!("delegate `{to}` is not an agent identity"))
            })?,
            _ => owner,
        };

        let basis = if actor == holder {
            "zkpa:holder"
        } else if actor == owner {
            "zkpa:owner"
        } else if self
            .delegations
            .is_authorized(owner, actor, TRANSFER_PERMISSION)
        {
            "zkpa:delegation:thought:transfer"
        } else {
            return Err(IdentityError::Refused(format!(
                "agent {actor} does not hold thought {thought:?}"
            )));
        };

        if let ThoughtOwnership::Delegated { to: delegate } = &to {
            Uuid::parse_str(delegate).map_err(|_| {
                IdentityError::Refused(format!("delegate `{delegate}` is not an agent identity"))
            })?;
        }

        ledger
            .record(AgencyEntry {
                thought,
                from: current,
                to,
                actor: actor.to_string(),
                basis: basis.to_string(),
                at,
            })
            .map_err(ledger_refused)
    }
}

fn ledger_refused(error: LedgerError) -> IdentityError {
    IdentityError::Refused(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy, Timestamp};
    use tpt_thought_provenance::{ProvenanceBody, ProvenanceSource, ProvenanceStore};

    fn agent(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn sessions_are_valid_until_closed() {
        let mut identity = AnimaIdentity::new();
        let session = identity
            .open_session(agent(1), "summarise", 5.0, Duration::from_secs(60))
            .unwrap();
        assert!(identity.session_is_valid(session));
        assert!(identity.close_session(session));
        assert!(!identity.session_is_valid(session));
        assert!(!identity.close_session(session), "already closed");
    }

    #[test]
    fn sessions_with_bad_budgets_or_zero_lifetimes_are_refused() {
        let mut identity = AnimaIdentity::new();
        let ttl = Duration::from_secs(60);
        assert_eq!(
            identity.open_session(agent(1), "t", f64::NAN, ttl),
            Err(IdentityError::InvalidBudget)
        );
        assert_eq!(
            identity.open_session(agent(1), "t", -1.0, ttl),
            Err(IdentityError::InvalidBudget)
        );
        assert_eq!(
            identity.open_session(agent(1), "t", 1.0, Duration::ZERO),
            Err(IdentityError::ZeroLifetime)
        );
    }

    #[test]
    fn delegated_permissions_hold_until_revoked() {
        let mut identity = AnimaIdentity::new();
        let (root, child) = (agent(1), agent(2));
        identity.delegate(root, child, &["read", "write"]).unwrap();
        assert!(identity.is_authorized(root, child, "read"));
        assert!(!identity.is_authorized(root, child, "delete"));
        identity.revoke_delegation(root, child).unwrap();
        assert!(!identity.is_authorized(root, child, "read"));
    }

    #[test]
    fn attestation_without_an_anima_backend_is_refused() {
        let identity = AnimaIdentity::new();
        let err = identity
            .attest(AttestationPlatform::AMDSecure, b"report")
            .unwrap_err();
        assert!(matches!(err, IdentityError::Refused(reason) if reason.contains("AMDSecure")));
    }

    #[test]
    fn attestation_probes_are_passed_through_and_marked() {
        let identity = AnimaIdentity::new();
        let platforms = [
            AttestationPlatform::TPM,
            AttestationPlatform::TrustZone,
            AttestationPlatform::SGX,
            AttestationPlatform::NVIDIACC,
            AttestationPlatform::AppleSE,
        ];
        for platform in platforms {
            let label = format!("{platform:?}");
            // Each probe fails honestly on machines without the hardware, so both outcomes are
            // checked: a refusal is accepted, and a success must carry the structural marker.
            match identity.attest(platform, b"evidence") {
                Ok(attestation) => {
                    let marker = identity.check_attestation(&attestation).unwrap();
                    assert_eq!(marker, format!("zkpa:attestation:{label}:structural"));
                }
                Err(IdentityError::Refused(_)) => {}
                Err(other) => panic!("unexpected error for {label}: {other}"),
            }
        }
    }

    #[test]
    fn malformed_or_unverified_attestations_are_not_marked() {
        let identity = AnimaIdentity::new();
        let forged = HardwareAttestation {
            platform: AttestationPlatform::TPM,
            attestation_data: vec![1, 2, 3],
            timestamp: std::time::SystemTime::now(),
            verified: true,
        };
        assert!(matches!(
            identity.check_attestation(&forged),
            Err(IdentityError::Refused(_))
        ));
        let unflagged = HardwareAttestation {
            attestation_data: vec![0; 32],
            verified: false,
            ..forged
        };
        assert!(identity.check_attestation(&unflagged).is_err());
    }

    #[test]
    fn authorization_proofs_verify_and_are_marked() {
        let identity = AnimaIdentity::new();
        let proof = identity
            .prove_authorization(agent(1), &["read", "write"])
            .unwrap();
        assert_eq!(proof.proof_type, "authorization");
        assert_eq!(
            identity.verify_proof(&proof).unwrap(),
            "zkpa:proof:authorization:groth16-verified"
        );
    }

    #[test]
    fn tampered_proofs_are_not_marked() {
        let identity = AnimaIdentity::new();
        let mut proof = identity.prove_authorization(agent(1), &["read"]).unwrap();
        // The last public input byte is the commitment, so changing it breaks the Groth16 check.
        let last = proof.public_inputs.len() - 1;
        proof.public_inputs[last] ^= 0x01;
        assert!(matches!(
            identity.verify_proof(&proof),
            Err(IdentityError::Refused(_))
        ));
    }

    #[test]
    fn markers_fit_the_provenance_verification_field() {
        let identity = AnimaIdentity::new();
        let proof = identity.prove_authorization(agent(1), &["read"]).unwrap();
        let marker = identity.verify_proof(&proof).unwrap();

        let ids = IdGenerator::new(SeededEntropy::new(3));
        let mut store = ProvenanceStore::new();
        let body = ProvenanceBody {
            id: ids.provenance(),
            source: ProvenanceSource::Verification,
            thought: None,
            parents: vec![],
            evidence: vec![],
            memory: vec![],
            provider: Some("tpt-anima-zkpa".into()),
            model: None,
            version: None,
            reasoning_path: vec![],
            verification: Some(marker.clone()),
            timestamp: Timestamp(1),
        };
        let record = store.insert(body).unwrap();
        assert_eq!(record.body.verification.as_deref(), Some(marker.as_str()));
        assert!(store.verify().is_ok());
    }

    #[test]
    fn empty_delegations_are_refused() {
        let mut identity = AnimaIdentity::new();
        assert!(matches!(
            identity.delegate(agent(1), agent(2), &[]),
            Err(IdentityError::Refused(reason)) if reason.contains("empty permissions")
        ));
    }

    fn thought_id(seed: u64) -> ThoughtId {
        IdGenerator::new(SeededEntropy::new(seed)).thought()
    }

    #[test]
    fn owner_hands_a_thought_to_a_delegate_who_publishes_it() {
        let identity = AnimaIdentity::new();
        let mut ledger = AgencyLedger::new();
        let (owner, delegate) = (agent(1), agent(2));
        let id = thought_id(10);
        identity.open_ownership(&mut ledger, id, owner).unwrap();

        let handed = ThoughtOwnership::Delegated {
            to: delegate.to_string(),
        };
        identity
            .transfer_ownership(&mut ledger, id, owner, handed, Timestamp(1))
            .unwrap();
        identity
            .transfer_ownership(
                &mut ledger,
                id,
                delegate,
                ThoughtOwnership::Published,
                Timestamp(2),
            )
            .unwrap();

        let bases: Vec<&str> = ledger.history(id).map(|e| e.basis.as_str()).collect();
        // While private, the owner is also the holder, so the holder rule applies first.
        assert_eq!(bases, ["zkpa:holder", "zkpa:holder"]);
        assert_eq!(ledger.current(id), Some(ThoughtOwnership::Published));
    }

    #[test]
    fn a_stranger_cannot_move_a_thought() {
        let identity = AnimaIdentity::new();
        let mut ledger = AgencyLedger::new();
        let id = thought_id(11);
        identity.open_ownership(&mut ledger, id, agent(1)).unwrap();

        let result = identity.transfer_ownership(
            &mut ledger,
            id,
            agent(9),
            ThoughtOwnership::Shared,
            Timestamp(1),
        );
        assert!(
            matches!(result, Err(IdentityError::Refused(reason)) if reason.contains("does not hold"))
        );
        assert_eq!(ledger.current(id), Some(ThoughtOwnership::Private));
    }

    #[test]
    fn transfer_permission_can_be_granted_and_revoked() {
        let mut identity = AnimaIdentity::new();
        let mut ledger = AgencyLedger::new();
        let (owner, helper) = (agent(1), agent(2));
        let id = thought_id(12);
        identity.open_ownership(&mut ledger, id, owner).unwrap();
        identity
            .delegate(owner, helper, &[TRANSFER_PERMISSION])
            .unwrap();

        identity
            .transfer_ownership(
                &mut ledger,
                id,
                helper,
                ThoughtOwnership::Shared,
                Timestamp(1),
            )
            .unwrap();
        assert_eq!(
            ledger.last_entry(id).map(|e| e.basis.as_str()),
            Some("zkpa:delegation:thought:transfer")
        );

        identity.revoke_delegation(owner, helper).unwrap();
        let after_revoke = identity.transfer_ownership(
            &mut ledger,
            id,
            helper,
            ThoughtOwnership::Private,
            Timestamp(2),
        );
        assert!(after_revoke.is_err());
        assert_eq!(ledger.history(id).count(), 1);
    }

    #[test]
    fn the_owner_can_reclaim_a_delegated_thought() {
        let identity = AnimaIdentity::new();
        let mut ledger = AgencyLedger::new();
        let (owner, delegate) = (agent(1), agent(2));
        let id = thought_id(13);
        identity.open_ownership(&mut ledger, id, owner).unwrap();
        let handed = ThoughtOwnership::Delegated {
            to: delegate.to_string(),
        };
        identity
            .transfer_ownership(&mut ledger, id, owner, handed, Timestamp(1))
            .unwrap();

        identity
            .transfer_ownership(
                &mut ledger,
                id,
                owner,
                ThoughtOwnership::Private,
                Timestamp(2),
            )
            .unwrap();
        assert_eq!(ledger.current(id), Some(ThoughtOwnership::Private));
    }

    #[test]
    fn delegates_and_state_changes_are_checked() {
        let identity = AnimaIdentity::new();
        let mut ledger = AgencyLedger::new();
        let owner = agent(1);
        let id = thought_id(14);
        identity.open_ownership(&mut ledger, id, owner).unwrap();

        let not_an_agent = ThoughtOwnership::Delegated { to: "bob".into() };
        assert!(matches!(
            identity.transfer_ownership(&mut ledger, id, owner, not_an_agent, Timestamp(1)),
            Err(IdentityError::Refused(reason)) if reason.contains("not an agent identity")
        ));

        identity
            .transfer_ownership(
                &mut ledger,
                id,
                owner,
                ThoughtOwnership::Published,
                Timestamp(2),
            )
            .unwrap();
        assert!(matches!(
            identity.transfer_ownership(&mut ledger, id, owner, ThoughtOwnership::Private, Timestamp(3)),
            Err(IdentityError::Refused(reason)) if reason.contains("cannot move")
        ));
        assert!(identity.open_ownership(&mut ledger, id, owner).is_err());
    }

    #[test]
    fn a_child_cannot_pass_on_more_than_it_was_granted() {
        let mut identity = AnimaIdentity::new();
        let (root, middle, leaf) = (agent(1), agent(2), agent(3));
        identity.delegate(root, middle, &["read"]).unwrap();
        assert!(identity.delegate(middle, leaf, &["write"]).is_err());
        assert!(identity.delegate(middle, leaf, &["read"]).is_ok());
    }
}
