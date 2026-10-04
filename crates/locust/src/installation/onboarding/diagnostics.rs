//! Read-only inspection of the selected onboarding journal and agent binding.
use super::*;

pub struct Selected {
    pub spec: Spec,
    pub binding: SetupSpec,
    record: Record,
}

/// The journal is selected only by daemon home, client and explicit profile.
/// Other selections come from its validated record, never from another profile.
pub fn select(home: &Path, client: Client, profile: &Path) -> Result<Selected, Failure> {
    let home = absolute(home)?;
    let profile = absolute(profile)?;
    private_dir(&home, false)?;
    private_dir(&home.join("onboarding"), false)?;
    let id = package::sha256(&encode(&(client, &profile))?);
    let directory = home.join("onboarding").join(id);
    private_dir(&directory, false)?;
    let bytes = protected_bytes(&directory.join("state.json"))?;
    let candidate: Record =
        serde_json::from_slice(&bytes).map_err(|_| corrupt("invalid onboarding journal"))?;
    if candidate.spec.client != client
        || candidate.spec.profile_home != profile
        || candidate.spec.daemon_home != home
    {
        return Err(conflict(
            "onboarding journal does not match the selected profile",
        ));
    }
    let (record, _) = read_record(&candidate.spec)?
        .ok_or_else(|| corrupt("selected onboarding journal disappeared"))?;
    Ok(Selected {
        binding: setup_spec(&record.spec)?,
        spec: record.spec.clone(),
        record,
    })
}

impl Selected {
    pub fn completed(&self) -> bool {
        self.record.configured
    }

    /// Checks both protected files and saved fingerprints before authenticating.
    /// No enrollment, token creation or identity replacement is performed.
    pub fn identity(&self) -> Result<Value, Failure> {
        let record = &self.record;
        let principal = record
            .principal
            .ok_or_else(|| Failure::unavailable("onboarding enrollment is incomplete"))?;
        let owner = protected_secret(&local::owner_credential_path(&self.spec.daemon_home))?;
        if package::sha256(&owner) != record.owner_sha256 {
            return Err(conflict(
                "daemon owner differs from the saved onboarding identity",
            ));
        }
        verify_secret(&self.binding.credential, &record.credential_sha256)?;
        verify_secret(&self.binding.session, &record.session_sha256)?;
        let credential = Credential(protected_secret(&self.binding.credential)?);
        let session = SessionSecret(protected_secret(&self.binding.session)?);
        if Some(session.instance()) != record.instance {
            return Err(conflict(
                "onboarding session differs from the saved instance",
            ));
        }
        let mut agent = open(&self.spec.daemon_home, credential, Some(session))?;
        if agent.caller() != Caller::Agent(principal) {
            return Err(conflict(
                "onboarding credential authenticates as another identity",
            ));
        }
        let observed = status(&mut agent, &self.spec.daemon_home)?;
        if !observed.agents.iter().any(|agent| {
            agent.agent == principal
                && Some(&agent.name) == self.spec.name.as_ref()
                && !agent.revoked
        }) {
            return Err(conflict("saved agent is missing, renamed or revoked"));
        }
        Ok(
            json!({"name":self.spec.name,"principal":principal,"instance":session.instance(),"authenticated":true}),
        )
    }
}
