import { invoke } from "@tauri-apps/api/core";
import { CONTRACT_VERSION, type FoundationStatus } from "./contracts.gen.ts";

export interface FoundationClient { status(): Promise<FoundationStatus> }
type StatusTransport = () => Promise<FoundationStatus>;

function client(transport: StatusTransport): FoundationClient {
  return { async status() {
    const response = await transport();
    if (response.contract_version !== CONTRACT_VERSION) {
      throw new Error("Atlas contract version is incompatible.");
    }
    return response;
  } };
}

export function createNativeClient(transport: StatusTransport = () => invoke<FoundationStatus>("foundation_status")): FoundationClient {
  return client(transport);
}

// F05 supplies the authenticated coordinator transport. This adapter defines
// its typed boundary; it does not start a listener or implement authentication.
export function createCoordinatorClient(authenticatedTransport: StatusTransport): FoundationClient {
  return client(authenticatedTransport);
}
