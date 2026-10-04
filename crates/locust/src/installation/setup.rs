//! Persistent client setup owns one MCP entry, skill and bound CLI launcher.
//! A private journal makes interrupted writes resumable; client readiness is
//! independently observed.
use super::*;
use locust_adapter::config::{self, BridgePaths, StdioServer};
use std::io::Read;
use toml_edit::{DocumentMut, Item};
mod launcher;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Client {
    Codex,
    Claude,
    Pi,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetupSpec {
    pub prefix: PathBuf,
    pub client: Client,
    pub profile_home: PathBuf,
    pub workspace: PathBuf,
    pub executable: PathBuf,
    pub skill_source: PathBuf,
    pub daemon_home: PathBuf,
    pub credential: PathBuf,
    pub session: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Image {
    bytes: Option<Vec<u8>>,
    mode: Option<u32>,
}
impl Image {
    fn absent() -> Self {
        Self {
            bytes: None,
            mode: None,
        }
    }
    fn summary(&self) -> Value {
        json!({"sha256":self.bytes.as_ref().map(|b|package::sha256(b)),"mode":self.mode})
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Record {
    format: String,
    spec: SetupSpec,
    original: Image,
    config: Image,
    skill: Image,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launcher: Option<Image>,
    entry: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Change {
    path: PathBuf,
    before: Image,
    after: Image,
}
#[derive(Clone, Serialize, Deserialize)]
struct Transaction {
    spec_hash: String,
    remove: bool,
    plan: SetupPlan,
    changes: Vec<Change>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct SetupPlan {
    review: Value,
}
impl SetupPlan {
    pub fn digest(&self) -> Result<String, Failure> {
        Ok(package::sha256(&encode(&self.review)?))
    }
    pub fn json(&self) -> Result<Value, Failure> {
        Ok(json!({"plan":self.review,"plan_sha256":self.digest()?}))
    }
}
struct Paths {
    config: PathBuf,
    skill: PathBuf,
    launcher: PathBuf,
    record: PathBuf,
    intent: PathBuf,
}
fn normalize(spec: &SetupSpec) -> Result<SetupSpec, Failure> {
    let mut s = spec.clone();
    s.prefix = absolute(&s.prefix)?;
    s.profile_home = absolute(&s.profile_home)?;
    s.workspace = absolute(&s.workspace)?;
    // Preserve the current symlink in installed command and skill references.
    if s.executable != spec.prefix.join("current/locust")
        || s.skill_source != spec.prefix.join("current/skills/locust/SKILL.md")
    {
        return Err(Failure::invalid(
            "setup source must be the installed current executable and skill",
        ));
    }
    s.executable = s.prefix.join("current/locust");
    s.skill_source = s.prefix.join("current/skills/locust/SKILL.md");
    for path in [&s.daemon_home, &s.credential, &s.session] {
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(Failure::invalid(
                "setup binding paths must be absolute without traversal",
            ));
        }
    }
    Ok(s)
}
fn paths(s: &SetupSpec) -> Result<Paths, Failure> {
    let (config, skill) = match s.client {
        Client::Codex => (".codex/config.toml", ".agents/skills/locust/SKILL.md"),
        Client::Claude => (".claude.json", ".claude/skills/locust/SKILL.md"),
        Client::Pi => (".pi/agent/mcp.json", ".pi/agent/skills/locust/SKILL.md"),
    };
    let id = package::sha256(&encode(&(s.client, &s.profile_home))?);
    Ok(Paths {
        config: s.profile_home.join(config),
        skill: s.profile_home.join(skill),
        launcher: s.profile_home.join(skill).with_file_name("locust-cli"),
        record: s.prefix.join("setup").join(format!("{id}.json")),
        intent: s.prefix.join("setup").join(format!("{id}.intent.json")),
    })
}
/// Existing client directories keep their permissions; reject writable or redirected components.
fn dirs(base: &Path, path: &Path, create: bool) -> Result<(), Failure> {
    let relative = path
        .strip_prefix(base)
        .map_err(|_| Failure::invalid("setup path outside profile"))?;
    let mut current = base.to_path_buf();
    for component in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(c) = component {
            current.push(c.as_os_str());
        }
        if !exists(&current)? {
            if !create {
                continue;
            }
            // The selected profile's ancestor must already exist; never create arbitrary HOME trees.
            DirBuilder::new()
                .mode(0o700)
                .create(&current)
                .map_err(io_error)?;
            sync(
                current
                    .parent()
                    .ok_or_else(|| Failure::invalid("profile has no parent"))?,
            )?;
        }
        let m = fs::symlink_metadata(&current).map_err(io_error)?;
        if !m.is_dir()
            || m.file_type().is_symlink()
            || m.uid() != rustix::process::getuid().as_raw()
            || m.mode() & 0o022 != 0
        {
            return Err(Failure::invalid(
                "setup directory must be owned, plain and not writable by others",
            ));
        }
    }
    Ok(())
}
fn snapshot(path: &Path) -> Result<Image, Failure> {
    if !exists(path)? {
        return Ok(Image::absent());
    }
    let mut f = package::regular(path)?;
    let m = f.metadata().map_err(io_error)?;
    if m.uid() != rustix::process::getuid().as_raw() || m.mode() & 0o022 != 0 {
        return Err(Failure::invalid(
            "setup file must be owned and not writable by others",
        ));
    }
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).map_err(io_error)?;
    Ok(Image {
        bytes: Some(bytes),
        mode: Some(m.mode() & 0o7777),
    })
}
fn read_record(p: &Paths) -> Result<(Image, Option<Record>), Failure> {
    let image = snapshot(&p.record)?;
    let record = image
        .bytes
        .as_ref()
        .map(|bytes| {
            serde_json::from_slice::<Record>(bytes)
                .map_err(|_| corrupt("invalid setup ownership record"))
        })
        .transpose()?;
    if image.bytes.is_some() && image.mode != Some(0o600) {
        return Err(corrupt("setup ownership must be mode 0600"));
    }
    if record.as_ref().is_some_and(|r| !record_format(r)) {
        return Err(corrupt("unknown setup ownership format"));
    }
    Ok((image, record))
}
fn record_format(record: &Record) -> bool {
    matches!(
        (record.format.as_str(), &record.launcher),
        ("locust-setup-owner-v1", None) | ("locust-setup-owner-v2", Some(_))
    )
}
fn protected(path: &Path) -> Result<Value, Failure> {
    let file = package::regular(path)?;
    let m = file.metadata().map_err(io_error)?;
    if m.uid() != rustix::process::getuid().as_raw() || m.mode() & 0o7777 != 0o600 || m.len() != 32
    {
        return Err(Failure::invalid(
            "setup credential and session must be owned mode 0600 regular 32-byte files",
        ));
    }
    let mut bytes = Vec::new();
    file.take(33).read_to_end(&mut bytes).map_err(io_error)?;
    if bytes.len() != 32 {
        return Err(Failure::invalid("setup secret changed during validation"));
    }
    Ok(json!({"sha256":package::sha256(&bytes),"mode":0o600}))
}
fn desired(s: &SetupSpec) -> Result<String, Failure> {
    let server = StdioServer {
        executable: s.executable.clone(),
        arguments: vec!["mcp".into()],
        paths: BridgePaths {
            home: s.daemon_home.clone(),
            session: s.session.clone(),
            credential: s.credential.clone(),
        },
    };
    let result = match s.client {
        Client::Codex => config::mcp_arguments(config::Client::Codex, "locust", &server, &[])
            .map(|args| args[1].to_string_lossy().into_owned()),
        Client::Claude => config::mcp_arguments(config::Client::ClaudeCode, "locust", &server, &[])
            .map(|args| {
                args[0]
                    .to_string_lossy()
                    .strip_prefix("--mcp-config=")
                    .expect("adapter contract")
                    .to_owned()
            }),
        Client::Pi => {
            config::mcp_file_overlay(config::Client::Pi, "locust", &server, &[], &json!({}))
                .map(|p| p.document.to_string())
        }
    }
    .map_err(|e| Failure::invalid(e.to_string()))?;
    if s.client == Client::Codex {
        Ok(result)
    } else {
        let v: Value = serde_json::from_str(&result).map_err(|_| corrupt("adapter JSON"))?;
        Ok(v["mcpServers"]["locust"].to_string())
    }
}
fn document(image: &Image) -> Result<&str, Failure> {
    std::str::from_utf8(image.bytes.as_deref().unwrap_or(b""))
        .map_err(|_| Failure::invalid("client config is not UTF-8"))
}
fn parse_toml(image: &Image) -> Result<DocumentMut, Failure> {
    document(image)?
        .parse()
        .map_err(|_| Failure::invalid("invalid client TOML"))
}
fn parse_json(image: &Image) -> Result<Value, Failure> {
    let v = if image.bytes.is_none() {
        json!({})
    } else {
        serde_json::from_str(document(image)?)
            .map_err(|_| Failure::invalid("invalid client JSON"))?
    };
    if !v.is_object() {
        return Err(Failure::invalid("client JSON must be an object"));
    }
    Ok(v)
}
fn entry(client: Client, image: &Image) -> Result<Option<String>, Failure> {
    if client == Client::Codex {
        let d = parse_toml(image)?;
        Ok(d.get("mcp_servers")
            .and_then(|v| v.get("locust"))
            .map(ToString::to_string))
    } else {
        let v = parse_json(image)?;
        if v.get("mcpServers").is_some_and(|s| !s.is_object()) {
            return Err(Failure::invalid("mcpServers must be an object"));
        }
        Ok(v.get("mcpServers")
            .and_then(|s| s.get("locust"))
            .map(Value::to_string))
    }
}
fn merge(client: Client, before: &Image, desired: Option<&str>) -> Result<Image, Failure> {
    let bytes = if client == Client::Codex {
        let mut d = parse_toml(before)?;
        if let Some(value) = desired {
            let new: DocumentMut = value.parse().map_err(|_| corrupt("adapter TOML"))?;
            if !d.contains_key("mcp_servers") {
                d["mcp_servers"] = Item::Table(toml_edit::Table::new());
            }
            let table = d["mcp_servers"]
                .as_table_like_mut()
                .ok_or_else(|| Failure::invalid("mcp_servers must be a table"))?;
            table.insert("locust", new["mcp_servers"]["locust"].clone());
        } else if let Some(table) = d.get_mut("mcp_servers").and_then(Item::as_table_like_mut) {
            table.remove("locust");
        }
        d.to_string().into_bytes()
    } else {
        let mut v = parse_json(before)?;
        if let Some(value) = desired {
            let servers = v
                .as_object_mut()
                .expect("object")
                .entry("mcpServers")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| Failure::invalid("mcpServers must be an object"))?;
            servers.insert(
                "locust".into(),
                serde_json::from_str(value).map_err(|_| corrupt("adapter JSON"))?,
            );
        } else if let Some(servers) = v.get_mut("mcpServers").and_then(Value::as_object_mut) {
            servers.remove("locust");
        }
        let mut bytes =
            serde_json::to_vec_pretty(&v).map_err(|_| corrupt("client JSON encoding"))?;
        bytes.push(b'\n');
        bytes
    };
    Ok(Image {
        bytes: Some(bytes),
        mode: Some(before.mode.unwrap_or(0o600)),
    })
}
fn json_nested_collision(value: &Value, root: bool) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(k, v)| {
            (!root && (k == "mcpServers" || k == "mcp_servers") && v.get("locust").is_some())
                || json_nested_collision(v, false)
        }),
        Value::Array(a) => a.iter().any(|v| json_nested_collision(v, false)),
        _ => false,
    }
}
fn toml_nested_collision(item: &Item, root: bool) -> bool {
    item.as_table_like().is_some_and(|table| {
        table.iter().any(|(k, v)| {
            (!root && k == "mcp_servers" && v.get("locust").is_some())
                || toml_nested_collision(v, false)
        })
    })
}
fn collisions(s: &SetupSpec, p: &Paths, config: &Image) -> Result<Value, Failure> {
    let nested = if s.client == Client::Codex {
        toml_nested_collision(parse_toml(config)?.as_item(), true)
    } else {
        json_nested_collision(&parse_json(config)?, true)
    };
    if nested {
        return Err(conflict("another profile-scoped Locust MCP entry exists"));
    }
    let mut evidence = Vec::new();
    if s.client == Client::Codex {
        let legacy = s.profile_home.join(".codex/skills/locust");
        if exists(&legacy)? {
            return Err(conflict("another profile Locust skill already exists"));
        }
        evidence.push(json!({"path":legacy,"exists":false}));
    }
    for ancestor in s.workspace.ancestors() {
        let (configs, skills): (&[&str], &[&str]) = match s.client {
            Client::Codex => (
                &[".codex/config.toml"],
                &[".agents/skills/locust", ".codex/skills/locust"],
            ),
            Client::Claude => (&[".mcp.json"], &[".claude/skills/locust"]),
            Client::Pi => (&[".pi/mcp.json"], &[".pi/skills/locust"]),
        };
        for relative in configs {
            let path = ancestor.join(relative);
            if path == p.config {
                continue;
            } // Project aliases are rejected rather than followed.
            if let Some(parent) = path.parent() {
                for component in parent.ancestors().take_while(|a| *a != ancestor) {
                    if exists(component)?
                        && fs::symlink_metadata(component)
                            .map_err(io_error)?
                            .file_type()
                            .is_symlink()
                    {
                        return Err(conflict("project configuration directory is a symlink"));
                    }
                }
            }
            let image = snapshot(&path)?;
            if entry(s.client, &image)?.is_some() {
                return Err(conflict(
                    "project or ancestor Locust MCP entry already exists",
                ));
            }
            evidence.push(json!({"path":path,"file":image.summary()}));
        }
        for relative in skills {
            let path = ancestor.join(relative);
            if Some(path.as_path()) == p.skill.parent() {
                continue;
            }
            let present = exists(&path)?;
            if present {
                return Err(conflict("project or ancestor Locust skill already exists"));
            }
            evidence.push(json!({"path":path,"exists":false}));
        }
    }
    Ok(json!(evidence))
}
fn pending(s: &SetupSpec, p: &Paths, remove: bool) -> Result<Option<Transaction>, Failure> {
    let image = snapshot(&p.intent)?;
    let Some(bytes) = image.bytes else {
        return Ok(None);
    };
    if image.mode != Some(0o600) {
        return Err(corrupt("setup journal must be mode 0600"));
    }
    let tx: Transaction =
        serde_json::from_slice(&bytes).map_err(|_| corrupt("invalid setup journal"))?;
    if tx.spec_hash != package::sha256(&encode(s)?) || (tx.remove && !remove) {
        return Err(conflict(
            "finish the pending setup operation with its original binding first",
        ));
    }
    let owned_paths = match tx.plan.review["format"].as_str() {
        Some("locust-setup-plan-v1") => vec![&p.config, &p.skill, &p.record],
        Some("locust-setup-plan-v2") => vec![&p.config, &p.skill, &p.launcher, &p.record],
        _ => return Err(corrupt("unknown setup journal plan format")),
    };
    if tx.changes.len() != owned_paths.len()
        || owned_paths.iter().any(|path| {
            tx.changes
                .iter()
                .filter(|change| &change.path == *path)
                .count()
                != 1
        })
    {
        return Err(corrupt(
            "setup journal must contain exactly its version's owned paths",
        ));
    }
    for change in &tx.changes {
        if !owned_paths.contains(&&change.path) {
            return Err(corrupt("unexpected setup journal path"));
        }
        let now = snapshot(&change.path)?;
        if now != change.before && now != change.after {
            return Err(conflict("a pending setup file was modified; preserving it"));
        }
    }
    if remove && !tx.remove {
        // This is a fresh reviewed cleanup, not resumption of the old apply.
        // All current bytes were checked above against the durable intent.
        // The intended ownership record includes any pre-apply unrelated edits.
        let owned = tx
            .changes
            .iter()
            .find(|c| c.path == p.record)
            .expect("validated paths");
        let record: Record = serde_json::from_slice(
            owned
                .after
                .bytes
                .as_deref()
                .ok_or_else(|| corrupt("pending apply has no intended ownership"))?,
        )
        .map_err(|_| corrupt("invalid pending setup ownership"))?;
        if !record_format(&record)
            || record.spec.client != s.client
            || record.spec.profile_home != s.profile_home
            || tx
                .changes
                .iter()
                .find(|c| c.path == p.config)
                .expect("validated paths")
                .after
                != record.config
            || tx
                .changes
                .iter()
                .find(|c| c.path == p.skill)
                .expect("validated paths")
                .after
                != record.skill
            || tx
                .changes
                .iter()
                .find(|c| c.path == p.launcher)
                .map(|c| &c.after)
                != record.launcher.as_ref()
        {
            return Err(corrupt("pending apply ownership disagrees with journal"));
        }
        let mut changes = vec![
            Change {
                path: p.config.clone(),
                before: snapshot(&p.config)?,
                after: record.original,
            },
            Change {
                path: p.skill.clone(),
                before: snapshot(&p.skill)?,
                after: Image::absent(),
            },
            Change {
                path: p.record.clone(),
                before: snapshot(&p.record)?,
                after: Image::absent(),
            },
        ];
        if record.launcher.is_some() {
            changes.insert(
                2,
                Change {
                    path: p.launcher.clone(),
                    before: snapshot(&p.launcher)?,
                    after: Image::absent(),
                },
            );
        }
        let review = json!({"format":tx.plan.review["format"],"action":"remove","spec":s,
            "cleanup_pending_apply_sha256":tx.plan.digest()?,
            "files":changes.iter().map(|c|json!({"path":c.path,"before":c.before.summary(),"after":c.after.summary()})).collect::<Vec<_>>(),
            "reload_required":true,"discovered":false,"api_ready":false,"binding_scope":"explicit profile/session"});
        return Ok(Some(Transaction {
            spec_hash: tx.spec_hash,
            remove: true,
            plan: SetupPlan { review },
            changes,
        }));
    }
    if !remove {
        let installed = super::status(&s.prefix)?;
        let binding =
            json!({"credential":protected(&s.credential)?,"session":protected(&s.session)?});
        let evidence = collisions(s, p, &snapshot(&p.config)?)?;
        if installed["installed"] != true
            || installed["withdrawn"] != false
            || installed["manifest_sha256"] != tx.plan.review["source_manifest_sha256"]
            || binding != tx.plan.review["binding_fingerprints"]
            || evidence != tx.plan.review["collision_inputs"]
        {
            return Err(conflict(
                "pending setup dependencies changed; preserving the journal and client files",
            ));
        }
    }
    Ok(Some(tx))
}
fn prepare(s: &SetupSpec, remove: bool) -> Result<Transaction, Failure> {
    private_dir(&s.prefix, false)?;
    private_dir(&s.prefix.join("setup"), false)?;
    let p = paths(s)?;
    for path in [&p.config, &p.skill, &p.launcher] {
        dirs(&s.profile_home, path.parent().expect("file parent"), false)?;
    }
    if let Some(tx) = pending(s, &p, remove)? {
        return Ok(tx);
    }
    let config = snapshot(&p.config)?;
    let skill = snapshot(&p.skill)?;
    let launcher = snapshot(&p.launcher)?;
    let (record_image, record) = read_record(&p)?;
    if let Some(r) = &record {
        if r.spec.client != s.client || r.spec.profile_home != s.profile_home {
            return Err(corrupt("setup ownership target mismatch"));
        }
        if entry(s.client, &config)?.as_deref() != Some(&r.entry)
            || skill != r.skill
            || r.launcher.as_ref().is_some_and(|owned| &launcher != owned)
        {
            return Err(conflict(
                "owned Locust entry, skill or launcher changed; preserving user edits",
            ));
        }
    }
    let collision = if remove {
        Value::Null
    } else {
        collisions(s, &p, &config)?
    };
    let binding = if remove {
        Value::Null
    } else {
        json!({"credential":protected(&s.credential)?,"session":protected(&s.session)?})
    };
    let (after_config, after_skill, after_launcher, after_record, source) = if remove {
        if let Some(r) = record {
            let restored = if config == r.config {
                r.original
            } else {
                merge(s.client, &config, None)?
            };
            let after_launcher = if r.launcher.is_some() {
                Image::absent()
            } else {
                launcher.clone()
            };
            (
                restored,
                Image::absent(),
                after_launcher,
                Image::absent(),
                Value::Null,
            )
        } else {
            (
                config.clone(),
                skill.clone(),
                launcher.clone(),
                record_image.clone(),
                Value::Null,
            )
        }
    } else {
        if record.is_none()
            && (entry(s.client, &config)?.is_some()
                || exists(p.skill.parent().expect("skill parent"))?)
        {
            return Err(conflict(
                "Locust MCP entry or skill directory already exists and is not owned",
            ));
        }
        if record.as_ref().is_none_or(|r| r.launcher.is_none()) && launcher.bytes.is_some() {
            return Err(conflict(
                "Locust CLI launcher already exists and is not owned",
            ));
        }
        let installed = super::status(&s.prefix)?;
        if installed["installed"] != true || installed["withdrawn"] != false {
            return Err(Failure::invalid(
                "setup requires a verified installed non-withdrawn release",
            ));
        }
        let new_skill = Image {
            bytes: Some(launcher::skill(
                &package::read_regular(&s.skill_source)?,
                &p.launcher,
            )?),
            mode: Some(0o644),
        };
        let new_launcher = Image {
            bytes: Some(launcher::render(s)?),
            mode: Some(0o700),
        };
        let new_config = merge(s.client, &config, Some(&desired(s)?))?;
        let entry = entry(s.client, &new_config)?.ok_or_else(|| corrupt("setup entry missing"))?;
        let original = match record {
            Some(r) if config == r.config => r.original,
            Some(_) => merge(s.client, &config, None)?,
            None => config.clone(),
        };
        let r = Record {
            format: "locust-setup-owner-v2".into(),
            spec: s.clone(),
            original,
            config: new_config.clone(),
            skill: new_skill.clone(),
            launcher: Some(new_launcher.clone()),
            entry,
        };
        let new_record = Image {
            bytes: Some(encode(&r)?),
            mode: Some(0o600),
        };
        (
            new_config,
            new_skill,
            new_launcher,
            new_record,
            installed["manifest_sha256"].clone(),
        )
    };
    let changes = vec![
        Change {
            path: p.config,
            before: config,
            after: after_config,
        },
        Change {
            path: p.skill,
            before: skill,
            after: after_skill,
        },
        Change {
            path: p.launcher.clone(),
            before: launcher,
            after: after_launcher,
        },
        Change {
            path: p.record,
            before: record_image,
            after: after_record,
        },
    ];
    let review = json!({"format":"locust-setup-plan-v2","action":if remove{"remove"}else{"apply"},"spec":s,"source_manifest_sha256":source,"files":changes.iter().map(|c|json!({"path":c.path,"before":c.before.summary(),"after":c.after.summary()})).collect::<Vec<_>>(),"collision_inputs":collision,"binding_fingerprints":binding,"server":if remove{None}else{Some(desired(s)?)},"launcher":p.launcher,"reload_required":true,"discovered":false,"api_ready":false,"binding_scope":"explicit profile/session"});
    Ok(Transaction {
        spec_hash: package::sha256(&encode(s)?),
        remove,
        plan: SetupPlan { review },
        changes,
    })
}
pub fn plan(spec: &SetupSpec, remove: bool) -> Result<SetupPlan, Failure> {
    Ok(prepare(&normalize(spec)?, remove)?.plan)
}
fn write_change(s: &SetupSpec, change: &Change) -> Result<(), Failure> {
    let parent = change.path.parent().expect("file parent");
    if change.path.starts_with(&s.profile_home) {
        dirs(&s.profile_home, parent, false)?;
    } else {
        private_dir(parent, false)?;
    }
    let now = snapshot(&change.path)?;
    if now == change.after {
        return Ok(());
    }
    if now != change.before {
        return Err(conflict("setup file changed since review; preserving it"));
    }
    if let Some(bytes) = &change.after.bytes {
        if change.path.starts_with(&s.profile_home) {
            dirs(&s.profile_home, parent, true)?;
        } else {
            private_dir(parent, true)?;
        }
        let temporary = parent.join(format!(".locust-setup-{}", nonce()?));
        package::create_file(&temporary, bytes, change.after.mode.expect("file mode"))?;
        if snapshot(&change.path)? != now {
            let _ = fs::remove_file(temporary);
            return Err(conflict("setup file changed during write"));
        }
        if let Err(e) = fs::rename(&temporary, &change.path) {
            let _ = fs::remove_file(temporary);
            return Err(io_error(e));
        }
        sync(parent)?;
    } else if now.bytes.is_some() {
        fs::remove_file(&change.path).map_err(io_error)?;
        sync(parent)?;
    }
    Ok(())
}
fn execute(
    spec: &SetupSpec,
    expected: &str,
    remove: bool,
    mut after_write: impl FnMut(usize) -> Result<(), Failure>,
) -> Result<Value, Failure> {
    let s = normalize(spec)?;
    let reviewed = prepare(&s, remove)?;
    if reviewed.plan.digest()? != expected {
        return Err(conflict("setup plan changed; review a fresh plan"));
    }
    let _lock = lock(&s.prefix)?;
    let tx = prepare(&s, remove)?;
    if tx.plan.digest()? != expected {
        return Err(conflict("setup plan changed while acquiring lock"));
    }
    let p = paths(&s)?;
    private_dir(&s.prefix.join("setup"), true)?;
    let journal = encode(&tx)?;
    if snapshot(&p.intent)?.bytes.as_deref() != Some(journal.as_slice()) {
        // Also persist an explicitly reviewed apply-to-removal transition before
        // changing profile files, so cleanup itself is safely resumable.
        atomic(&p.intent, &journal)?;
    }
    for (i, change) in tx.changes.iter().enumerate() {
        write_change(&s, change)?;
        after_write(i)?;
    }
    fs::remove_file(&p.intent).map_err(io_error)?;
    sync(p.intent.parent().expect("journal parent"))?;
    if remove
        && tx
            .changes
            .iter()
            .any(|c| c.path == p.skill && c.before.bytes.is_some() && c.after.bytes.is_none())
    {
        let _ = fs::remove_dir(p.skill.parent().expect("skill directory"));
    }
    let launcher_ready = !remove
        && tx
            .changes
            .iter()
            .any(|c| c.path == p.launcher && c.after.bytes.is_some());
    Ok(
        json!({"changed":tx.changes.iter().any(|c|c.before!=c.after),"configured":launcher_ready,"removed":remove,"launcher":p.launcher,"launcher_ready":launcher_ready,"reapply_required":!remove&&!launcher_ready,"reload_required":true,"discovered":false,"api_ready":false,"plan_sha256":expected}),
    )
}
pub fn apply(spec: &SetupSpec, expected: &str) -> Result<Value, Failure> {
    execute(spec, expected, false, |_| Ok(()))
}
pub fn remove(spec: &SetupSpec, expected: &str) -> Result<Value, Failure> {
    execute(spec, expected, true, |_| Ok(()))
}
pub fn status(spec: &SetupSpec) -> Result<Value, Failure> {
    let s = normalize(spec)?;
    let p = paths(&s)?;
    private_dir(&s.prefix, false)?;
    private_dir(&s.prefix.join("setup"), false)?;
    for path in [&p.config, &p.skill, &p.launcher] {
        dirs(&s.profile_home, path.parent().expect("parent"), false)?;
    }
    let (_, r) = read_record(&p)?;
    let launcher_image = snapshot(&p.launcher)?;
    let launcher_ready = r
        .as_ref()
        .and_then(|r| r.launcher.as_ref())
        .is_some_and(|owned| owned.bytes.is_some() && *owned == launcher_image);
    let intact = if let Some(r) = &r {
        entry(s.client, &snapshot(&p.config)?)?.as_deref() == Some(&r.entry)
            && snapshot(&p.skill)? == r.skill
            && launcher_ready
    } else {
        false
    };
    let binding_matches = r.as_ref().is_some_and(|r| {
        r.spec.executable == s.executable
            && r.spec.daemon_home == s.daemon_home
            && r.spec.credential == s.credential
            && r.spec.session == s.session
            && r.spec.client == s.client
            && r.spec.profile_home == s.profile_home
    });
    Ok(
        json!({"owned":r.is_some(),"configured":intact,"binding_matches":binding_matches,"pending":exists(&p.intent)?,"client":s.client,"profile_home":s.profile_home,"launcher":p.launcher,"launcher_ready":launcher_ready,"reapply_required":r.as_ref().is_some_and(|r|r.launcher.is_none()),"reload_required":true,"discovered":false,"api_ready":false,"binding_scope":"explicit profile/session"}),
    )
}
#[cfg(test)]
mod tests;
