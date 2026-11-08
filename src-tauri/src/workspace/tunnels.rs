//! Remote application tunnels owned by the workspace (F17/T035).
use super::*;
use atlas_core::{
    ssh::SshCheckState,
    tunnel::{MAX_TUNNELS, TUNNEL_LIFETIME, TunnelInfo, TunnelProcess, TunnelRequest},
};

pub(super) struct TunnelEntry {
    info: TunnelInfo,
    target: ServerTarget,
    process: TunnelProcess,
}

#[derive(Default)]
pub(super) struct Tunnels {
    entries: HashMap<String, TunnelEntry>,
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
        let process = TunnelProcess::start(&self.ssh, target.host(), request.remote_port)?;
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
        self.reap_tunnels();
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
        let candidates: Vec<(String, ServerId, ServerTarget)> = match self.lock_tunnels() {
            Ok(tunnels) => tunnels
                .entries
                .iter()
                .map(|(id, e)| (id.clone(), e.info.server_id.clone(), e.target.clone()))
                .collect(),
            Err(_) => return Vec::new(),
        };
        if candidates.is_empty() {
            return Vec::new();
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
        let ended: Vec<String> = tunnels
            .entries
            .iter_mut()
            .filter_map(|(id, entry)| {
                (revoked.contains(id)
                    || entry.process.age() >= TUNNEL_LIFETIME
                    || !entry.process.alive())
                .then(|| id.clone())
            })
            .collect();
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
