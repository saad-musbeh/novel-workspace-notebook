export interface DocxExportReceipt {
  cancelled: boolean;
  name?: string;
}

export type DocxExportInvoke = (command: string, args: { projectId: string; suggestedName: string; profileRevisionId:string|null }) => Promise<DocxExportReceipt>;

export interface WorkspaceExportSavePayload {
  requestId: number;
  projectId: string;
}

export interface WorkspaceExportSaveAck {
  requestId: number;
  error: string | null;
}

/** Save every matching local tab and report this window's result to native coordination. */
export async function acknowledgeWorkspaceExportSave(
  payload: WorkspaceExportSavePayload,
  tabs: { id: string }[],
  options: {
    busy: boolean;
    isIncoming: (tabId: string) => boolean;
    drain: (tabId: string) => Promise<void>;
    acknowledge: (result: WorkspaceExportSaveAck) => Promise<void>;
  },
): Promise<void> {
  let error: string | null = null;
  try {
    if (options.busy) throw new Error('This window is busy with another workspace operation.');
    if (!tabs.length) throw new Error('This window has no ready tab for the requested project.');
    for (const tab of tabs) {
      if (options.isIncoming(tab.id)) throw new Error('A project tab is still transferring into this window.');
      await options.drain(tab.id);
    }
  } catch (reason) {
    error = String(reason);
  }
  await options.acknowledge({ requestId: payload.requestId, error });
}

/** Drain other open project windows, then ask the native exporter to save a DOCX. */
export async function exportDocx(
  projectId: string,
  invokeCommand: DocxExportInvoke,
  beforeProjectRecover?: (projectId: string) => Promise<void>,
  profileRevisionId:string|null=null,
): Promise<string | null> {
  if (beforeProjectRecover) await beforeProjectRecover(projectId);
  const receipt = await invokeCommand('export_docx', { projectId, suggestedName: 'manuscript.docx',profileRevisionId });
  if (receipt.cancelled) return null;
  const name = receipt.name ? ` Saved as ${receipt.name}.` : ' The file was saved.';
  return `Word manuscript exported.${name} DOCX includes manuscript text and formatting. Story planning, revision history, and custom font assets remain in the project archive.`;
}
