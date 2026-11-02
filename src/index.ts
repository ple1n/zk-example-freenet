import {
  ContractKey,
  DeltaUpdate,
  DisconnectRequest,
  FreenetWsApi,
  GetRequest,
  UpdateData,
  UpdateRequest,
} from "@freenetorg/freenet-stdlib/websocket-interface";
import { UpdateDataType } from "@freenetorg/freenet-stdlib/common";

type Candidate = {
  counter: number;
  text: string;
  proof: string;
};

const form = document.querySelector<HTMLFormElement>("#submit-form")!;
const contractInput = document.querySelector<HTMLInputElement>("#contract-id")!;
const counterInput = document.querySelector<HTMLInputElement>("#counter")!;
const textInput = document.querySelector<HTMLTextAreaElement>("#text")!;
const proofInput = document.querySelector<HTMLTextAreaElement>("#proof")!;
const candidateFile = document.querySelector<HTMLInputElement>("#candidate-file")!;
const status = document.querySelector<HTMLElement>("#status")!;
const current = document.querySelector<HTMLElement>("#current-note")!;

let api: FreenetWsApi | undefined;
let connectedContractId: string | undefined;

function showStatus(message: string): void {
  status.textContent = message;
}

function parseCandidate(value: unknown): Candidate {
  if (
    typeof value !== "object" ||
    value === null ||
    !("counter" in value) ||
    !("text" in value) ||
    !("proof" in value) ||
    typeof value.counter !== "number" ||
    !Number.isSafeInteger(value.counter) ||
    value.counter < 0 ||
    typeof value.text !== "string" ||
    typeof value.proof !== "string" ||
    !/^(?:[0-9a-f]{2})+$/i.test(value.proof)
  ) {
    throw new Error("Candidate JSON must contain a nonnegative counter, text, and hex proof.");
  }
  return {
    counter: value.counter,
    text: value.text,
    proof: value.proof,
  };
}

function connect(contractId: string): Promise<FreenetWsApi> {
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  const socketUrl = new URL(`${protocol}//${location.host}/contract/command/`);
  const key = ContractKey.fromInstanceId(contractId);

  return new Promise((resolve, reject) => {
    let client: FreenetWsApi;
    let isConnected = false;
    const timeout = window.setTimeout(
      () => reject(new Error("Timed out connecting to the Freenet node.")),
      10_000,
    );
    client = new FreenetWsApi(socketUrl, {
      onContractPut: () => undefined,
      onContractGet: (response) => {
        if (response.key.encode() !== key.encode()) return;
        if (response.state.length === 0) {
          current.textContent = "No note is stored yet.";
          return;
        }
        try {
          const note = JSON.parse(new TextDecoder().decode(new Uint8Array(response.state))) as {
            counter: number;
            text: string;
          };
          current.textContent = `Revision ${note.counter}: ${note.text}`;
        } catch {
          current.textContent = "The contract returned state that this demo could not parse.";
        }
      },
      onContractUpdate: () => showStatus("The node accepted the update request for processing."),
      onContractUpdateNotification: (notification) => {
        if (notification.key.encode() === key.encode()) {
          showStatus("A state update notification was received.");
          void client.get(new GetRequest(key, false));
        }
      },
      onDelegateResponse: () => undefined,
      onErr: (error) => {
        showStatus(`Node error: ${error.cause}`);
        if (!isConnected) {
          window.clearTimeout(timeout);
          reject(new Error(error.cause));
        }
      },
      onOpen: () => {
        isConnected = true;
        window.clearTimeout(timeout);
        showStatus("Connected. Fetching the current note.");
        void client.get(new GetRequest(key, false));
        resolve(client);
      },
    });
  });
}

candidateFile.addEventListener("change", async () => {
  const file = candidateFile.files?.[0];
  if (!file) return;
  try {
    const candidate = parseCandidate(JSON.parse(await file.text()));
    counterInput.value = String(candidate.counter);
    textInput.value = candidate.text;
    proofInput.value = candidate.proof;
    showStatus("Candidate loaded. Submitting will ask the contract to verify it.");
  } catch (error) {
    showStatus(error instanceof Error ? error.message : "Could not read candidate file.");
  }
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    const contractId = contractInput.value.trim();
    const candidate = parseCandidate({
      counter: Number(counterInput.value),
      text: textInput.value,
      proof: proofInput.value.trim(),
    });
    if (!contractId) throw new Error("Enter the Freenet contract instance ID.");

    if (connectedContractId !== contractId) {
      if (api) void api.disconnect(new DisconnectRequest());
      api = await connect(contractId);
      connectedContractId = contractId;
    }

    const delta = new TextEncoder().encode(JSON.stringify(candidate));
    const updateData = new UpdateData(
      UpdateDataType.DeltaUpdate,
      new DeltaUpdate(Array.from(delta)),
    );
    await api!.update(new UpdateRequest(ContractKey.fromInstanceId(contractId), updateData));
    showStatus("Submitted. The contract will verify the proof and apply it only if its counter wins.");
  } catch (error) {
    showStatus(error instanceof Error ? error.message : "Could not submit the update.");
  }
});
