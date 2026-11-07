import { invoke } from "@tauri-apps/api/core";
import type { MonitoringState, MonitoringPolicy } from "./contracts.gen";
import { CONTRACT_VERSION, type AddRelationship, type AddServer, type MapState, type UpdateServer, type WorkspaceStatus, type OperationTicket, type SshVerification, type FileSession, type FileAction, type FileResult, type FileChange, type FileMutationPreview, type FileMutationReceipt, type LocalSelection, type TransferView, type ElevationPreview, type ElevationReceipt, type ProtectedPlacementRequest, type ProtectedPlacementStage, type ProtectedPlacementReceipt, type HealthProbeConfig, type HealthSnapshot, type InventoryRefreshResult, type TerminalPoll, type WorkloadInspection, type WorkloadAction, type WorkloadControlPreview, type WorkloadControlReceipt } from "./contracts.gen.ts";

export type NativeTransport = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
export function createWorkspaceClient(transport: NativeTransport = invoke) {
  async function request<T extends { contract_version: number }>(command: string, args?: Record<string, unknown>): Promise<T> {
    const value = await transport<T>(command, args);
    if (value.contract_version !== CONTRACT_VERSION) throw new Error("InfraWerf contract version is incompatible.");
    return value;
  }
  const map = (command: string, args?: Record<string, unknown>) => request<MapState>(command, args);
  return {
    monitoring: () => request<MonitoringState>("monitoring_state"),
    acceptMonitoringIdentity:(serverId:string,previous:string,accepted:string)=>request<MonitoringState>("monitoring_accept_identity",{serverId,previous,accepted}),
    configureMonitoring: (serverId:string,policy:MonitoringPolicy) => request<MonitoringState>("monitoring_configure",{serverId,policy}),
    discoverServers: () => request<MonitoringState>("monitoring_discover"),
    historyLimit: (limit:number) => request<MonitoringState>("monitoring_history_limit",{limit}),
    status: () => request<WorkspaceStatus>("workspace_status"),
    load: () => map("map_state"),
    retry: () => map("retry_workspace"),
    add: (input: AddServer) => map("add_server", { input }),
    update: (id: string, input: UpdateServer) => map("update_server", { id, input }),
    remove: (id: string) => map("remove_server", { id }),
    connect: (input: AddRelationship) => map("add_relationship", { input }),
    disconnect: (id: string) => map("remove_relationship", { id }),
    importReports: () => map("import_reports"),
    prepareSsh: (serverId: string) => transport<OperationTicket>("ssh_prepare", { serverId }),
    verifySsh: (serverId: string, operationId: string) => request<SshVerification>("ssh_verify", { serverId, operationId }),
    cancelSsh: (operationId: string) => transport<void>("ssh_cancel", { operationId }),
    configureHealth: (serverId: string, config: HealthProbeConfig) => request<HealthSnapshot>("health_configure", { serverId, config }),
    healthStatus: (serverId: string) => request<HealthSnapshot>("health_status", { serverId }),
    cancelHealth: (serverId: string) => transport<void>("health_cancel", { serverId }),
    refreshInventory: (serverId: string) => request<InventoryRefreshResult>("inventory_refresh", { serverId }),
    prepareFiles: (serverId: string) => transport<OperationTicket>("files_prepare", { serverId }),
    openFiles: (serverId: string, operationId: string) => transport<FileSession>("files_open", { serverId, operationId }),
    readFiles: (serverId: string, sessionId: string, action: FileAction) => transport<FileResult>("files_read", { serverId, sessionId, action }),
    inspectWorkload: (serverId: string, sessionId: string, workloadId: string) => request<WorkloadInspection>("workload_inspect", { serverId, sessionId, workloadId }),
    previewWorkloadControl: (serverId: string, sessionId: string, workloadId: string, action: WorkloadAction) => request<WorkloadControlPreview>("workload_control_preview", { serverId, sessionId, workloadId, action }),
    confirmWorkloadControl: (serverId: string, sessionId: string, operationId: string, password?: string) => request<WorkloadControlReceipt>("workload_control_confirm", { serverId, sessionId, operationId, password }),
    lockFiles: (sessionId: string) => transport<void>("files_lock", { sessionId }),
    startTerminal: (serverId: string, sessionId: string, cols: number, rows: number) => transport<void>("files_terminal_start", { serverId, sessionId, cols, rows }),
    readTerminal: (serverId: string, sessionId: string) => transport<TerminalPoll>("files_terminal_poll", { serverId, sessionId }),
    writeTerminal: (serverId: string, sessionId: string, data: Uint8Array) => transport<void>("files_terminal_write", { serverId, sessionId, data: Array.from(data) }),
    resizeTerminal: (serverId: string, sessionId: string, cols: number, rows: number) => transport<void>("files_terminal_resize", { serverId, sessionId, cols, rows }),
    releaseFiles: (sessionId: string) => transport<void>("files_release", { sessionId }),
    pickUpload: (serverId: string, sessionId: string) => transport<LocalSelection | null>("files_pick_upload", { serverId, sessionId }),
    pickDownload: (serverId: string, sessionId: string, resourceId: string) => transport<LocalSelection | null>("files_pick_download", { serverId, sessionId, resourceId }),
    discardSelection: (grantId: string) => transport<void>("files_selection_discard", { grantId }),
    transfer: (serverId: string, sessionId: string, resourceId: string, grantId: string) => transport<TransferView>("files_transfer", { serverId, sessionId, resourceId, grantId }),
    transfers: () => transport<TransferView[]>("transfer_list"),
    cancelTransfer: (operationId: string) => transport<void>("transfer_cancel", { operationId }),
    previewElevation: (serverId: string, sessionId: string) => transport<ElevationPreview>("elevation_preview", { serverId, sessionId }),
    confirmElevation: (serverId: string, sessionId: string, operationId: string, password: string) => transport<ElevationReceipt>("elevation_confirm", { serverId, sessionId, operationId, password }),
    cancelElevation: (serverId: string, sessionId: string, operationId: string) => transport<void>("elevation_cancel", { serverId, sessionId, operationId }),
    stageProtectedPlacement: (serverId: string, sessionId: string, resourceId: string, grantId: string) => transport<ProtectedPlacementStage>("files_stage_protected", { serverId, sessionId, resourceId, grantId }),
    previewProtectedPlacement: (serverId: string, sessionId: string, request: ProtectedPlacementRequest) => transport<ElevationPreview>("protected_placement_preview", { serverId, sessionId, request }),
    confirmProtectedPlacement: (serverId: string, sessionId: string, operationId: string, password: string) => transport<ProtectedPlacementReceipt>("protected_placement_confirm", { serverId, sessionId, operationId, password }),
    cancelProtectedPlacement: (serverId: string, sessionId: string, resourceId: string) => transport<void>("protected_placement_cancel", { serverId, sessionId, resourceId }),
    previewChange: (serverId: string, sessionId: string, change: FileChange) => transport<FileMutationPreview>("files_preview", { serverId, sessionId, change }),
    commitChange: (serverId: string, sessionId: string, operationId: string) => transport<FileMutationReceipt>("files_commit", { serverId, sessionId, operationId }),
    discardChange: (serverId: string, sessionId: string, operationId: string) => transport<void>("files_discard", { serverId, sessionId, operationId }),
  };
}
export type WorkspaceClient = ReturnType<typeof createWorkspaceClient>;
