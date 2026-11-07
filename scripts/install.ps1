# Installs the markshot release binary for this Windows machine (x86_64 or arm64) and checks it
# against the release's SHA256SUMS.
#
#   powershell -ExecutionPolicy Bypass -c "irm https://github.com/hypn4/markshot/releases/latest/download/install.ps1 | iex"
#
# Environment:
#   MARKSHOT_VERSION      release tag to install, e.g. v0.1.0 (default: the latest release)
#   MARKSHOT_INSTALL_DIR  where to put markshot.exe (default: %LOCALAPPDATA%\Programs\markshot),
#                         added to the user PATH when missing
#   MARKSHOT_DOWNLOAD_URL base URL, or local directory, holding the assets, overriding the GitHub
#                         release (for tests)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repo = 'hypn4/markshot'
$version = if ($env:MARKSHOT_VERSION) { $env:MARKSHOT_VERSION } else { 'latest' }
$dir = if ($env:MARKSHOT_INSTALL_DIR) {
    $env:MARKSHOT_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA 'Programs\markshot'
}

# The OS architecture, not the process's: an x64 PowerShell on Arm64 Windows still gets the
# native binary.
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
$target = switch ($arch) {
    'X64' { 'x86_64-pc-windows-msvc' }
    'Arm64' { 'aarch64-pc-windows-msvc' }
    default { throw "markshot install: no release binary for Windows on $arch" }
}

$base = if ($env:MARKSHOT_DOWNLOAD_URL) {
    $env:MARKSHOT_DOWNLOAD_URL
} elseif ($version -eq 'latest') {
    "https://github.com/$repo/releases/latest/download"
} else {
    "https://github.com/$repo/releases/download/$version"
}

# Copies `$name` from the release (or the local directory standing in for it) to `$out`.
function Get-Asset($name, $out) {
    if (Test-Path -LiteralPath $base -PathType Container) {
        Copy-Item -LiteralPath (Join-Path $base $name) -Destination $out
    } else {
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$name" -OutFile $out
    }
}

$asset = "markshot-$target.exe"
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("markshot-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Write-Output "Downloading $asset ($version)"
    Get-Asset $asset (Join-Path $tmp $asset)
    Get-Asset 'SHA256SUMS' (Join-Path $tmp 'SHA256SUMS')

    $want = $null
    foreach ($line in Get-Content (Join-Path $tmp 'SHA256SUMS')) {
        $fields = $line -split '\s+', 2
        if ($fields.Count -eq 2 -and $fields[1].TrimStart('*') -eq $asset) { $want = $fields[0] }
    }
    if (-not $want) { throw "markshot install: SHA256SUMS has no entry for $asset" }
    $got = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $asset)).Hash.ToLowerInvariant()
    if ($got -ne $want.ToLowerInvariant()) {
        throw "markshot install: checksum mismatch for ${asset}: expected $want, got $got"
    }

    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $exe = Join-Path $dir 'markshot.exe'
    Move-Item -Force (Join-Path $tmp $asset) $exe
    Write-Output "Installed $(& $exe --version) to $exe"
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$entries = if ($userPath) { $userPath -split ';' } else { @() }
if ($entries -notcontains $dir) {
    $newPath = if ($userPath) { "$userPath;$dir" } else { $dir }
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    $env:Path = "$env:Path;$dir"
    Write-Output "Added $dir to your user PATH; open a new terminal to use markshot."
}
