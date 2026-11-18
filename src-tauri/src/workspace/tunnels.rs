//! Remote application tunnels owned by the workspace (F17/T035).
use super::*;
use atlas_core::{
    ssh::SshCheckState,
    tunnel::{
        AppDiscovery, DiscoveredApp, MAX_TUNNELS, TUNNEL_LIFETIME, TunnelInfo, TunnelProcess,
        TunnelRequest,
    },
};

pub(super) struct TunnelEntry {
    info: TunnelInfo,
    target: ServerTarget,
    process: TunnelProcess,
}

#[derive(Default)]
pub(super) struct Tunnels {
    entries: HashMap<String, TunnelEntry>,
    /// Latest discovery per server and endpoint; authorises specific-address
    /// destinations. Cleared when the server's target changes.
    discovered: HashMap<ServerId, (ServerTarget, Vec<DiscoveredApp>)>,
    /// Tunnels opened for a plugin pack's application (F21/T044). They end
    /// when what plugins may do changes.
    from_packs: HashSet<String>,
    /// Tunnels ended by such a change, for the next reap to report so the
    /// shell closes their windows.
    ended: Vec<String>,
}

impl Workspace {
    /// Verify trusted SSH access, then start one loopback-only forward. The
    /// verification uses the same strict path as Settings and never changes
    /// OpenSSH trust; an unknown or changed key stops here.
    pub fn open_tunnel(
        self: &Arc<Self>,
        id: &ServerId,
        request: TunnelRequest,
    ) -> Result<TunnelInfo, AtlasError> {
        request.validate()?;
        self.broker.extensions().authorize(
            atlas_core::extensions::OPERATION_LOOPBACK_TUNNEL,
            atlas_core::extensions::Capability::LoopbackTunnel,
        )?;
        if let Some(host) = request.remote_host.as_deref() {
            // A non-loopback destination must be an address discovery found
            // this server listening on, for this exact port and endpoint.
            let target = self.ssh_target(id)?;
            let tunnels = self.lock_tunnels()?;
            let allowed = tunnels.discovered.get(id).is_some_and(|(seen, apps)| {
                *seen == target
                    && apps.iter().any(|app| {
                        app.port == request.remote_port && app.remote_host.as_deref() == Some(host)
                    })
            });
            if !allowed {
                return Err(AtlasError::new(ErrorCode::Denied));
            }
        }
        if self.lock_tunnels()?.entries.len() >= MAX_TUNNELS {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        let ticket = self.prepare_ssh(id)?;
        let verification = self.verify_ssh(id, &ticket.operation_id)?;
        match verification.state {
            SshCheckState::Verified => {}
            SshCheckState::UnknownHost | SshCheckState::ChangedHost => {
                return Err(AtlasError::new(ErrorCode::HostTrustFailure));
            }
            SshCheckState::AuthenticationFailed => {
                return Err(AtlasError::new(ErrorCode::Unauthenticated));
            }
            _ => return Err(AtlasError::new(ErrorCode::IoFailure)),
        }
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let name = self
            .with_store(|store| store.snapshot())?
            .servers
            .into_iter()
            .find(|server| server.id == *id)
            .map(|server| server.name)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        let process = TunnelProcess::start(
            &self.ssh,
            target.host(),
            &request.destination(),
            request.remote_port,
        )?;
        // A policy or endpoint edit during start-up invalidates this forward.
        if self.ssh_target(id)? != target {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let now = chrono::Utc::now();
        let info = TunnelInfo {
            contract_version: CONTRACT_VERSION,
            tunnel_id: OperationId::generate().as_str().to_owned(),
            server_id: id.clone(),
            server_name: name,
            remote_port: request.remote_port,
            local_port: process.local_port(),
            url: format!("http://127.0.0.1:{}{}", process.local_port(), request.path),
            opened_at: now.to_rfc3339(),
            expires_at: (now
                + chrono::Duration::from_std(TUNNEL_LIFETIME).map_err(|_| failure())?)
            .to_rfc3339(),
        };
        let mut tunnels = self.lock_tunnels()?;
        if tunnels.entries.len() >= MAX_TUNNELS {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        tunnels.entries.insert(
            info.tunnel_id.clone(),
            TunnelEntry {
                info: info.clone(),
                target,
                process,
            },
        );
        Ok(info)
    }

    /// Read-only discovery of listening applications through a short-lived
    /// verified session (the same path as inventory refresh). Nothing is
    /// persisted beyond this run's in-memory cache.
    pub fn discover_applications(
        self: &Arc<Self>,
        id: &ServerId,
    ) -> Result<AppDiscovery, AtlasError> {
        let _admission = self.begin()?;
        // Live contact is serialised only while the session opens, as for
        // file sessions. The slow remote listing then runs without it, so
        // opening a tunnel from the cached list need not wait; a target
        // change meanwhile discards the result below.
        let (target, session) = {
            let _contact = self.live_contact_guard()?;
            let target = self.ssh_target(id)?;
            let ticket = self.files.prepare(&target)?;
            let session_admission = self.begin()?;
            let session = self.files.open(
                &target,
                &ticket.operation_id,
                Box::new(session_admission),
                None,
            )?;
            (target, session)
        };
        let found = self
            .files
            .discover_applications(&target, &session.session_id);
        let closed = self.files.close(&session.session_id);
        let (complete, apps) = found?;
        closed?;
        if self.ssh_target(id)? != target {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        self.lock_tunnels()?
            .discovered
            .insert(id.clone(), (target, apps.clone()));
        Ok(AppDiscovery {
            contract_version: CONTRACT_VERSION,
            server_id: id.clone(),
            discovered_at: chrono::Utc::now().to_rfc3339(),
            complete,
            apps,
        })
    }

    /// Marks a tunnel as opened for a pack application.
    pub(super) fn mark_pack_tunnel(&self, tunnel_id: &str) -> Result<(), AtlasError> {
        self.lock_tunnels()?.from_packs.insert(tunnel_id.to_owned());
        Ok(())
    }

    /// Ends every tunnel opened for a pack application. Their windows are
    /// closed by the next reap.
    pub(super) fn close_pack_tunnels(&self) {
        let Ok(mut tunnels) = self.lock_tunnels() else {
            return;
        };
        let ids: Vec<String> = tunnels.from_packs.drain().collect();
        let removed: Vec<_> = ids
            .iter()
            .filter_map(|id| tunnels.entries.remove(id))
            .collect();
        tunnels.ended.extend(ids);
        drop(tunnels);
        // Dropping the entries kills and reaps each OpenSSH child.
        drop(removed);
    }

    pub fn close_tunnel(&self, tunnel_id: &str) -> Result<(), AtlasError> {
        // Dropping the entry kills and reaps the OpenSSH child.
        let removed = self.lock_tunnels()?.entries.remove(tunnel_id);
        drop(removed);
        Ok(())
    }

    pub fn close_all_tunnels(&self) -> Vec<String> {
        let Ok(mut tunnels) = self.lock_tunnels() else {
            return Vec::new();
        };
        let removed: Vec<_> = tunnels.entries.drain().collect();
        drop(tunnels);
        removed.into_iter().map(|(id, _)| id).collect()
    }

    pub fn tunnels(&self) -> Result<Vec<TunnelInfo>, AtlasError> {
        // Tunnels a plugin change ended stay reported for the background
        // reap, which closes their windows.
        self.reap(false);
        let mut list: Vec<_> = self
            .lock_tunnels()?
            .entries
            .values()
            .map(|entry| entry.info.clone())
            .collect();
        list.sort_by(|a, b| a.opened_at.cmp(&b.opened_at));
        Ok(list)
    }

    /// Remove tunnels whose child exited, whose lifetime ended, or whose
    /// server is no longer active/contactable or changed endpoint. Returns the
    /// removed IDs so the shell can close their windows.
    pub fn reap_tunnels(&self) -> Vec<String> {
        self.reap(true)
    }

    fn reap(&self, take_ended: bool) -> Vec<String> {
        let candidates: Vec<(String, ServerId, ServerTarget)> = match self.lock_tunnels() {
            Ok(tunnels) => tunnels
                .entries
                .iter()
                .map(|(id, e)| (id.clone(), e.info.server_id.clone(), e.target.clone()))
                .collect(),
            Err(_) => return Vec::new(),
        };
        if candidates.is_empty() {
            return match self.lock_tunnels() {
                Ok(mut tunnels) if take_ended => std::mem::take(&mut tunnels.ended),
                _ => Vec::new(),
            };
        }
        // Policy is read without holding the tunnel lock.
        let revoked: HashSet<String> = candidates
            .into_iter()
            .filter(|(_, server, target)| self.ssh_target(server).ok().as_ref() != Some(target))
            .map(|(id, _, _)| id)
            .collect();
        let Ok(mut tunnels) = self.lock_tunnels() else {
            return Vec::new();
        };
        let mut ended: Vec<String> = tunnels
            .entries
            .iter_mut()
            .filter_map(|(id, entry)| {
                (revoked.contains(id)
                    || entry.process.age() >= TUNNEL_LIFETIME
                    || !entry.process.alive())
                .then(|| id.clone())
            })
            .collect();
        for id in &ended {
            tunnels.from_packs.remove(id);
        }
        if take_ended {
            let closed_for_plugins = std::mem::take(&mut tunnels.ended);
            ended.extend(closed_for_plugins);
        }
        let removed: Vec<_> = ended
            .iter()
            .filter_map(|id| tunnels.entries.remove(id))
            .collect();
        drop(tunnels);
        drop(removed);
        ended
    }

    fn lock_tunnels(&self) -> Result<std::sync::MutexGuard<'_, Tunnels>, AtlasError> {
        self.tunnels.lock().map_err(|_| failure())
    }
}

use std::collections::HashSet;
