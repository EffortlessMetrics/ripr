import { LspCompatibilityEvidence } from '../../src/lspCompatibility';

export const compatibleLspEvidence: LspCompatibilityEvidence = {
  status: 'compatible',
  positionEncoding: 'utf-16',
  required: {
    textDocumentSync: true,
    hover: true,
    codeAction: true,
    pullDiagnostics: true,
    executeCommand: true,
    workspaceFolders: true,
    positionEncoding: true
  },
  optional: {
    codeActionResolve: true,
    workDoneProgress: false
  },
  processResult: 'clean_exit'
};
