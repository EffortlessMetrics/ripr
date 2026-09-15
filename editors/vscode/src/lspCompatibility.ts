import * as cp from 'child_process';
import * as crypto from 'crypto';

const DEFAULT_PROBE_TIMEOUT_MS = 5000;
const MAX_MESSAGE_BYTES = 1024 * 1024;
const PROBE_EXIT_CODE_MARKER = '__RIPR_PROBE_EXIT_CODE__';
const probeExitCodes = new WeakMap<cp.ChildProcess, number>();

export type RequiredLspCapability =
  | 'textDocumentSync'
  | 'hover'
  | 'codeAction'
  | 'pullDiagnostics'
  | 'executeCommand'
  | 'workspaceFolders'
  | 'positionEncoding';
export type OptionalLspCapability = 'codeActionResolve' | 'workDoneProgress';

export const REQUIRED_SERVER_COMMANDS = [
  'ripr.refresh',
  'ripr.collectContext',
  'ripr.collectEvidenceContext',
  'ripr.collectWorkspaceStatus',
  'ripr.collectRepairPacket',
  'ripr.collectTopLimitation',
  'ripr.collectReceiptStatus'
] as const;

export interface LspCompatibilityEvidence {
  readonly status: 'compatible';
  readonly serverName?: string;
  readonly serverVersion?: string;
  readonly positionEncoding: 'utf-16';
  readonly required: Readonly<Record<RequiredLspCapability, true>>;
  readonly optional: Readonly<Record<OptionalLspCapability, boolean>>;
  readonly processResult: 'clean_exit';
}

export type LspProbeFailureKind =
  | 'spawn_failure'
  | 'framing_failure'
  | 'protocol_failure'
  | 'initialize_error'
  | 'initialize_timeout'
  | 'server_identity_mismatch'
  | 'missing_required_capability'
  | 'position_encoding_mismatch'
  | 'shutdown_failure'
  | 'process_failure';

export interface LspCompatibilityFailure {
  readonly status: 'incompatible';
  readonly kind: LspProbeFailureKind;
  readonly detail: string;
}

export type LspCompatibilityResult = LspCompatibilityEvidence | LspCompatibilityFailure;

interface JsonRpcMessage {
  readonly jsonrpc?: string;
  readonly id?: number | string | null;
  readonly method?: string;
  readonly result?: unknown;
  readonly error?: unknown;
}

interface JsonRpcError {
  readonly code: number;
  readonly message: string;
}

const WINDOWS_PROBE_JOB_WRAPPER = String.raw`
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading;

public static class RiprProbeJob {
    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct IO_COUNTERS {
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr CreateJobObject(IntPtr attributes, string name);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(
        IntPtr job,
        int informationClass,
        ref JOBOBJECT_EXTENDED_LIMIT_INFORMATION information,
        uint informationLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

    [DllImport("kernel32.dll")]
    private static extern bool CloseHandle(IntPtr handle);

    public static IntPtr CreateAndAssignCurrentProcess() {
        IntPtr job = CreateJobObject(IntPtr.Zero, null);
        if (job == IntPtr.Zero) {
            throw new Win32Exception(Marshal.GetLastWin32Error(), "CreateJobObject failed");
        }
        var information = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
        information.BasicLimitInformation.LimitFlags = 0x00002000;
        if (!SetInformationJobObject(job, 9, ref information, (uint)Marshal.SizeOf(information))) {
            int error = Marshal.GetLastWin32Error();
            CloseHandle(job);
            throw new Win32Exception(error, "SetInformationJobObject failed");
        }
        if (!AssignProcessToJobObject(job, Process.GetCurrentProcess().Handle)) {
            int error = Marshal.GetLastWin32Error();
            CloseHandle(job);
            throw new Win32Exception(error, "AssignProcessToJobObject failed");
        }
        return job;
    }

    public static void Close(IntPtr job) {
        if (job != IntPtr.Zero) {
            CloseHandle(job);
        }
    }

    public static int Run(string command, string arguments, bool useShell) {
        IntPtr job = CreateAndAssignCurrentProcess();
        try {
            var start = new ProcessStartInfo {
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardInput = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true
            };
            if (useShell) {
                start.FileName = Environment.GetEnvironmentVariable("ComSpec") ?? "cmd.exe";
                start.Arguments = "/d /s /c \"\"" + command.Replace("\"", "\"\"") + "\" " + arguments + "\"";
            } else {
                start.FileName = command;
                start.Arguments = arguments;
            }
            string exitNonce = Environment.GetEnvironmentVariable("RIPR_PROBE_EXIT_NONCE") ?? "";
            start.EnvironmentVariables.Remove("RIPR_PROBE_EXIT_NONCE");
            using (var process = new Process { StartInfo = start }) {
                if (!process.Start()) {
                    throw new InvalidOperationException("Probe process did not start.");
                }
                var input = StartPump(Console.OpenStandardInput(), process.StandardInput.BaseStream, true);
                var output = StartPump(process.StandardOutput.BaseStream, Console.OpenStandardOutput(), false);
                var error = StartPump(process.StandardError.BaseStream, Console.OpenStandardError(), false);
                process.WaitForExit();
                int exitCode = process.ExitCode;
                Console.Error.WriteLine("__RIPR_PROBE_EXIT_CODE__" + exitNonce + "=" + exitCode);
                Console.Error.Flush();
                Close(job);
                job = IntPtr.Zero;
                try { process.StandardInput.Close(); } catch {}
                output.Join(1000);
                error.Join(1000);
                return exitCode;
            }
        } finally {
            Close(job);
        }
    }

    private static Thread StartPump(System.IO.Stream source, System.IO.Stream destination, bool closeDestination) {
        var thread = new Thread(() => {
            try {
                var buffer = new byte[8192];
                int count;
                while ((count = source.Read(buffer, 0, buffer.Length)) > 0) {
                    destination.Write(buffer, 0, count);
                    destination.Flush();
                }
            } catch (IOException) {
                // The owner closes pipes during bounded process-tree cleanup.
            } catch (ObjectDisposedException) {
                // The owner closes pipes during bounded process-tree cleanup.
            } finally {
                if (closeDestination) {
                    try { destination.Close(); } catch {}
                }
            }
        });
        thread.IsBackground = true;
        thread.Start();
        return thread;
    }
}
'@ | Out-Null

[Environment]::Exit([RiprProbeJob]::Run(
    $env:RIPR_PROBE_COMMAND,
    $env:RIPR_PROBE_ARGUMENTS,
    $env:RIPR_PROBE_USE_SHELL -eq '1'))
`;

export async function probeStandardLspCompatibility(
  command: string,
  useShell = false,
  timeoutMs = DEFAULT_PROBE_TIMEOUT_MS
): Promise<LspCompatibilityResult> {
  return new Promise((resolve) => {
    let settled = false;
    let phase: 'initialize' | 'shutdown' | 'exit' = 'initialize';
    let evidence: Omit<LspCompatibilityEvidence, 'processResult'> | undefined;
    let stdout: Buffer<ArrayBufferLike> = Buffer.alloc(0);
    const child = spawnProbeProcess(command, ['lsp', '--stdio'], useShell);

    const finish = (result: LspCompatibilityResult): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      terminateProbeProcessTree(child);
      resolve(result);
    };

    const fail = (kind: LspProbeFailureKind, detail: string): void => {
      finish({ status: 'incompatible', kind, detail });
    };

    const timer = setTimeout(() => {
      const kind = phase === 'initialize' ? 'initialize_timeout' : 'shutdown_failure';
      fail(kind, `Timed out after ${timeoutMs}ms during the LSP ${phase} phase.`);
    }, timeoutMs);

    child.once('error', (error) => fail('spawn_failure', error.message));
    child.stdin?.once('error', (error) => fail('process_failure', `LSP probe stdin failed: ${error.message}`));
    child.once('exit', (wrapperCode, signal) => {
      const code = probeProcessExitCode(child, wrapperCode);
      if (settled) {
        return;
      }
      if (phase === 'exit' && code === 0 && evidence) {
        finish({ ...evidence, processResult: 'clean_exit' });
        return;
      }
      fail(
        'process_failure',
        `LSP probe process exited during ${phase} with code ${String(code)} and signal ${String(signal)}.`
      );
    });

    child.stdout.on('data', (chunk: Buffer) => {
      if (settled) {
        return;
      }
      stdout = Buffer.concat([stdout, chunk]);
      try {
        stdout = consumeMessages(stdout, onMessage);
      } catch (error) {
        fail('framing_failure', error instanceof Error ? error.message : String(error));
      }
    });

    const onMessage = (message: JsonRpcMessage): void => {
      if (message.jsonrpc !== '2.0') {
        fail('protocol_failure', 'LSP response omitted the required JSON-RPC 2.0 envelope.');
        return;
      }
      if (message.method && message.id !== undefined) {
        writeMessage(child, { jsonrpc: '2.0', id: message.id, result: null });
        return;
      }
      const expectedResponseId = phase === 'initialize' ? 1 : phase === 'shutdown' ? 2 : undefined;
      if (hasResponseMember(message) && message.id !== expectedResponseId) {
        fail('protocol_failure', `LSP response used unexpected id ${String(message.id)} during ${phase}.`);
        return;
      }
      if (phase === 'initialize' && message.id === 1) {
        const response = validateJsonRpcResponse(message);
        if (response.status === 'invalid') {
          fail('protocol_failure', `Initialize response ${response.detail}`);
          return;
        }
        if (response.status === 'error') {
          fail('initialize_error', response.error.message || `Initialize failed with code ${String(response.error.code)}.`);
          return;
        }
        const checked = validateInitializeResult(response.result);
        if (checked.status === 'incompatible') {
          finish(checked);
          return;
        }
        evidence = checked;
        phase = 'shutdown';
        writeMessage(child, { jsonrpc: '2.0', method: 'initialized', params: {} });
        writeMessage(child, { jsonrpc: '2.0', id: 2, method: 'shutdown', params: null });
        return;
      }
      if (phase === 'shutdown' && message.id === 2) {
        const response = validateJsonRpcResponse(message);
        if (response.status === 'invalid') {
          fail('protocol_failure', `Shutdown response ${response.detail}`);
          return;
        }
        if (response.status === 'error') {
          fail('shutdown_failure', response.error.message || 'The server rejected shutdown.');
          return;
        }
        if (response.result !== null) {
          fail('protocol_failure', 'Shutdown response result must be null.');
          return;
        }
        phase = 'exit';
        writeMessage(child, { jsonrpc: '2.0', method: 'exit', params: null });
      }
    };

    writeMessage(child, {
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        processId: process.pid,
        clientInfo: { name: 'ripr-vscode-compatibility-probe' },
        rootUri: null,
        capabilities: {
          general: { positionEncodings: ['utf-16'] },
          workspace: { configuration: true, workspaceFolders: true },
          textDocument: {
            hover: { contentFormat: ['markdown', 'plaintext'] },
            codeAction: { resolveSupport: { properties: ['edit', 'command'] } },
            diagnostic: {},
            publishDiagnostics: {}
          },
          window: { workDoneProgress: true }
        },
        workspaceFolders: null
      }
    });
  });
}

function consumeMessages(buffer: Buffer, consume: (message: JsonRpcMessage) => void): Buffer {
  let remaining = buffer;
  while (remaining.length > 0) {
    const headerEnd = remaining.indexOf('\r\n\r\n');
    if (headerEnd < 0) {
      if (remaining.length > 8192) {
        throw new Error('LSP response headers exceeded 8192 bytes or were not CRLF framed.');
      }
      return remaining;
    }
    const header = remaining.subarray(0, headerEnd).toString('ascii');
    const lengthMatch = /^content-length:\s*(\d+)\s*$/im.exec(header);
    if (!lengthMatch) {
      throw new Error('LSP response omitted a valid Content-Length header.');
    }
    const length = Number(lengthMatch[1]);
    if (!Number.isSafeInteger(length) || length > MAX_MESSAGE_BYTES) {
      throw new Error(`LSP response declared an invalid Content-Length of ${lengthMatch[1]}.`);
    }
    const bodyStart = headerEnd + 4;
    if (remaining.length < bodyStart + length) {
      return remaining;
    }
    const body = remaining.subarray(bodyStart, bodyStart + length).toString('utf8');
    let parsed: unknown;
    try {
      parsed = JSON.parse(body);
    } catch {
      throw new Error('LSP response body was not valid JSON.');
    }
    if (!isObject(parsed)) {
      throw new Error('LSP response body was not a JSON-RPC object.');
    }
    consume(parsed as JsonRpcMessage);
    remaining = remaining.subarray(bodyStart + length);
  }
  return remaining;
}

function validateInitializeResult(result: unknown): Omit<LspCompatibilityEvidence, 'processResult'> | LspCompatibilityFailure {
  if (!isObject(result) || !isObject(result.capabilities)) {
    return { status: 'incompatible', kind: 'initialize_error', detail: 'Initialize result omitted server capabilities.' };
  }
  const capabilities = result.capabilities;
  const missing: string[] = [];
  if (!supportedTextDocumentSync(capabilities.textDocumentSync)) missing.push('textDocumentSync');
  if (!providerEnabled(capabilities.hoverProvider)) missing.push('hoverProvider');
  if (!providerEnabled(capabilities.codeActionProvider)) missing.push('codeActionProvider');
  if (!providerEnabled(capabilities.diagnosticProvider)) missing.push('diagnosticProvider');
  const executeCommand = isObject(capabilities.executeCommandProvider)
    ? capabilities.executeCommandProvider
    : undefined;
  const commands = Array.isArray(executeCommand?.commands)
    ? executeCommand.commands.filter((command): command is string => typeof command === 'string')
    : [];
  const missingCommands = REQUIRED_SERVER_COMMANDS.filter((command) => !commands.includes(command));
  if (missingCommands.length > 0) missing.push(`executeCommandProvider.commands (${missingCommands.join(', ')})`);
  const workspace = isObject(capabilities.workspace) ? capabilities.workspace : undefined;
  const workspaceFolders = workspace && isObject(workspace.workspaceFolders) ? workspace.workspaceFolders : undefined;
  if (workspaceFolders?.supported !== true) missing.push('workspace.workspaceFolders.supported');
  if (missing.length > 0) {
    return {
      status: 'incompatible',
      kind: 'missing_required_capability',
      detail: `Initialize result omitted required capabilities: ${missing.join(', ')}.`
    };
  }
  const positionEncoding = capabilities.positionEncoding ?? 'utf-16';
  if (positionEncoding !== 'utf-16') {
    return {
      status: 'incompatible',
      kind: 'position_encoding_mismatch',
      detail: `Server selected ${String(positionEncoding)}; the extension requires utf-16.`
    };
  }
  const serverInfo = isObject(result.serverInfo) ? result.serverInfo : undefined;
  if (serverInfo?.name !== 'ripr' || typeof serverInfo.version !== 'string' || serverInfo.version.length === 0) {
    return {
      status: 'incompatible',
      kind: 'server_identity_mismatch',
      detail: 'Initialize result did not identify a versioned ripr server.'
    };
  }
  const codeAction = capabilities.codeActionProvider;
  return {
    status: 'compatible',
    serverName: serverInfo.name,
    serverVersion: serverInfo.version,
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
      codeActionResolve: isObject(codeAction) && codeAction.resolveProvider === true,
      workDoneProgress:
        (isObject(capabilities.hoverProvider) && capabilities.hoverProvider.workDoneProgress === true) ||
        (isObject(codeAction) && codeAction.workDoneProgress === true)
    }
  };
}

function supportedTextDocumentSync(value: unknown): boolean {
  if (value === 1) {
    return true;
  }
  if (!isObject(value) || value.change !== 1) {
    return false;
  }
  return value.save === true || (isObject(value.save) && value.save.includeText !== false);
}

type CheckedJsonRpcResponse =
  | { readonly status: 'result'; readonly result: unknown }
  | { readonly status: 'error'; readonly error: JsonRpcError }
  | { readonly status: 'invalid'; readonly detail: string };

function validateJsonRpcResponse(message: JsonRpcMessage): CheckedJsonRpcResponse {
  if (message.method !== undefined) {
    return { status: 'invalid', detail: 'must not include a method member.' };
  }
  const hasResult = Object.prototype.hasOwnProperty.call(message, 'result');
  const hasError = Object.prototype.hasOwnProperty.call(message, 'error');
  if (hasResult === hasError) {
    return { status: 'invalid', detail: 'must include exactly one of result or error.' };
  }
  if (hasResult) {
    return { status: 'result', result: message.result };
  }
  if (!isObject(message.error) || !Number.isInteger(message.error.code) || typeof message.error.message !== 'string') {
    return { status: 'invalid', detail: 'included a malformed error object.' };
  }
  return {
    status: 'error',
    error: { code: message.error.code as number, message: message.error.message }
  };
}

function hasResponseMember(message: JsonRpcMessage): boolean {
  return (
    Object.prototype.hasOwnProperty.call(message, 'result') ||
    Object.prototype.hasOwnProperty.call(message, 'error')
  );
}

function providerEnabled(value: unknown): boolean {
  return value === true || isObject(value);
}

function writeMessage(child: cp.ChildProcess, message: object): void {
  const body = Buffer.from(JSON.stringify(message), 'utf8');
  child.stdin?.write(`Content-Length: ${body.length}\r\n\r\n`);
  child.stdin?.write(body);
}

export function terminateProbeProcessTree(child: cp.ChildProcess): void {
  child.stdin?.destroy();
  if (child.pid && process.platform === 'win32') {
    // The wrapper's kill-on-close Job Object already owns descendants after
    // wrapper exit. Address taskkill only to a still-current wrapper PID;
    // using an exited PID would introduce a replacement-process deletion race.
    if (child.exitCode === null && child.signalCode === null) {
      cp.spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'], {
        stdio: 'ignore',
        windowsHide: true
      });
    }
    return;
  }
  if (child.pid) {
    try {
      process.kill(-child.pid, 'SIGKILL');
      return;
    } catch {
      // Fall through when the process exited between the currentness check and termination.
    }
  }
  child.kill('SIGKILL');
}

export function spawnProbeProcess(
  command: string,
  args: readonly string[],
  useShell: boolean
): cp.ChildProcessWithoutNullStreams {
  if (process.platform === 'win32') {
    const encodedWrapper = Buffer.from(WINDOWS_PROBE_JOB_WRAPPER, 'utf16le').toString('base64');
    const exitNonce = crypto.randomBytes(16).toString('hex');
    const child = cp.spawn(
      'powershell.exe',
      [
        '-NoProfile',
        '-NonInteractive',
        '-InputFormat',
        'Text',
        '-OutputFormat',
        'Text',
        '-ExecutionPolicy',
        'Bypass',
        '-EncodedCommand',
        encodedWrapper
      ],
      {
        shell: false,
        windowsHide: true,
        stdio: ['pipe', 'pipe', 'pipe'],
        env: {
          ...process.env,
          RIPR_PROBE_COMMAND: command,
          RIPR_PROBE_ARGUMENTS: args.join(' '),
          RIPR_PROBE_EXIT_NONCE: exitNonce,
          RIPR_PROBE_USE_SHELL: useShell ? '1' : '0'
        }
      }
    );
    let stderr = '';
    child.stderr.on('data', (chunk: Buffer) => {
      stderr += chunk.toString('utf8');
      const match = new RegExp(`${PROBE_EXIT_CODE_MARKER}${exitNonce}=(-?\\d+)`).exec(stderr);
      if (match) {
        probeExitCodes.set(child, Number(match[1]));
      }
      if (stderr.length > 4096) {
        stderr = stderr.slice(-4096);
      }
    });
    return child;
  }
  return cp.spawn(command, args, {
    shell: useShell,
    detached: true,
    stdio: ['pipe', 'pipe', 'pipe']
  });
}

export function probeProcessExitCode(child: cp.ChildProcess, wrapperCode: number | null): number | null {
  return probeExitCodes.get(child) ?? wrapperCode;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
