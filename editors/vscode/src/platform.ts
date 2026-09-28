export interface RiprPlatform {
  readonly target: string;
  readonly executableName: string;
  readonly archiveExtension: 'zip' | 'tar.gz';
  readonly displayName: string;
}

export function currentRiprPlatform(): RiprPlatform | undefined {
  return riprPlatformFor(process.platform, process.arch);
}

/**
 * Pure mapping from a Node `(platform, arch)` pair to the published release
 * target. Kept separate from `process` so the mapping is unit-testable on any
 * host.
 */
export function riprPlatformFor(platform: string, arch: string): RiprPlatform | undefined {
  // Windows on ARM64 has no native release target. Windows 11 on ARM runs
  // x64 binaries through built-in emulation, so the x64 MSVC artifact is the
  // working managed-server download there rather than offering none.
  if (platform === 'win32' && (arch === 'x64' || arch === 'arm64')) {
    return {
      target: 'x86_64-pc-windows-msvc',
      executableName: 'ripr.exe',
      archiveExtension: 'zip',
      displayName: 'Windows x64'
    };
  }

  if (platform === 'darwin' && arch === 'x64') {
    return {
      target: 'x86_64-apple-darwin',
      executableName: 'ripr',
      archiveExtension: 'tar.gz',
      displayName: 'macOS x64'
    };
  }

  if (platform === 'darwin' && arch === 'arm64') {
    return {
      target: 'aarch64-apple-darwin',
      executableName: 'ripr',
      archiveExtension: 'tar.gz',
      displayName: 'macOS arm64'
    };
  }

  if (platform === 'linux' && arch === 'x64') {
    return {
      target: 'x86_64-unknown-linux-gnu',
      executableName: 'ripr',
      archiveExtension: 'tar.gz',
      displayName: 'Linux x64 GNU'
    };
  }

  if (platform === 'linux' && arch === 'arm64') {
    return {
      target: 'aarch64-unknown-linux-gnu',
      executableName: 'ripr',
      archiveExtension: 'tar.gz',
      displayName: 'Linux arm64 GNU'
    };
  }

  return undefined;
}

