$ErrorActionPreference = "Stop"

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$compiler = "C:\msys64\ucrt64\bin\g++.exe"
$windresCandidates = @(
    "C:\msys64\ucrt64\bin\windres.exe",
    "C:\msys64\ucrt64\bin\x86_64-w64-mingw32-windres.exe"
)
$vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
$vs2022Paths = @()
if (Test-Path $vswhere) {
    $vs2022Json = & $vswhere -products * -format json | ConvertFrom-Json
    $vs2022Paths = $vs2022Json |
        Where-Object {
            $_.catalog.productLineVersion -eq "2022" -or $_.displayName -like "*2022*"
        } |
        ForEach-Object { $_.installationPath }
}
$vs2022DevCmds = $vs2022Paths | ForEach-Object { Join-Path $_ "Common7\Tools\VsDevCmd.bat" }
$vs2022Vcvars = $vs2022Paths | ForEach-Object { Join-Path $_ "VC\Auxiliary\Build\vcvars64.bat" }
$msvcEnvCandidates = @(
    $env:NU_MSVC_ENV_BAT
    $vs2022DevCmds
    $env:NU_MSVC_VCVARS64
    $vs2022Vcvars
) | Where-Object { $_ }
$source = Join-Path $PSScriptRoot "minecraft_block.cpp"
$output = Join-Path $PSScriptRoot "minecraft_block.exe"
$resourceScript = Join-Path $PSScriptRoot "minecraft_block.rc"
$resourceObject = Join-Path $PSScriptRoot "minecraft_block_res.o"
$iconPng = Join-Path $repo "assets\branding\nu-icon-white-512.png"
$iconIco = Join-Path $PSScriptRoot "nu-icon.ico"
$includeDir = Join-Path $repo "include"
$rustProfile = "release"
$rustTarget = Join-Path $repo "target\$rustProfile"
$importLib = Join-Path $rustTarget "nu.dll.lib"
$runtimeDll = Join-Path $rustTarget "nu.dll"
$localImportLib = Join-Path $PSScriptRoot "nu.dll.lib"
$localRuntimeDll = Join-Path $PSScriptRoot "nu.dll"
$ucrtDlls = @(
    "C:\msys64\ucrt64\bin\libstdc++-6.dll",
    "C:\msys64\ucrt64\bin\libgcc_s_seh-1.dll",
    "C:\msys64\ucrt64\bin\libwinpthread-1.dll"
)

function New-IcoFromPng {
    param(
        [Parameter(Mandatory = $true)][string]$PngPath,
        [Parameter(Mandatory = $true)][string]$IcoPath
    )

    Add-Type -AssemblyName System.Drawing
    $sourceImage = [System.Drawing.Image]::FromFile($PngPath)
    $bitmap = New-Object System.Drawing.Bitmap 256, 256
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.Clear([System.Drawing.Color]::Transparent)
    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.DrawImage($sourceImage, 0, 0, 256, 256)
    $pngStream = New-Object System.IO.MemoryStream
    $bitmap.Save($pngStream, [System.Drawing.Imaging.ImageFormat]::Png)
    $pngBytes = $pngStream.ToArray()
    $memory = New-Object System.IO.MemoryStream
    $writer = New-Object System.IO.BinaryWriter($memory)

    $writer.Write([UInt16]0)
    $writer.Write([UInt16]1)
    $writer.Write([UInt16]1)
    $writer.Write([byte]0)
    $writer.Write([byte]0)
    $writer.Write([byte]0)
    $writer.Write([byte]0)
    $writer.Write([UInt16]1)
    $writer.Write([UInt16]32)
    $writer.Write([UInt32]$pngBytes.Length)
    $writer.Write([UInt32]22)
    $writer.Write($pngBytes)
    $writer.Flush()
    [System.IO.File]::WriteAllBytes($IcoPath, $memory.ToArray())
    $writer.Dispose()
    $memory.Dispose()
    $pngStream.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
    $sourceImage.Dispose()
}

if (-not (Test-Path $compiler)) {
    throw "compiler not found: $compiler"
}

if (-not (Test-Path $iconPng)) {
    throw "missing nu icon asset: $iconPng"
}

$windres = $windresCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $windres) {
    throw "windres not found in C:\msys64\ucrt64\bin"
}

$msvcEnv = $msvcEnvCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $msvcEnv) {
    throw "Visual Studio 2022 MSVC environment batch not found. Set NU_MSVC_ENV_BAT or NU_MSVC_VCVARS64."
}

$cargoScript = Join-Path $PSScriptRoot "build_nu_dll.cmd"
$envCall = if ([System.IO.Path]::GetFileName($msvcEnv) -ieq "VsDevCmd.bat") {
    "call `"$msvcEnv`" -arch=amd64 -host_arch=amd64"
} else {
    "call `"$msvcEnv`""
}
$cargoBody = @"
@echo off
${envCall}
if errorlevel 1 exit /b 1
set PATH=%VCToolsInstallDir%bin\Hostx64\x64;%PATH%
set CC=%VCToolsInstallDir%bin\Hostx64\x64\cl.exe
set CXX=%VCToolsInstallDir%bin\Hostx64\x64\cl.exe
set CMAKE_C_COMPILER=%VCToolsInstallDir%bin\Hostx64\x64\cl.exe
set CMAKE_CXX_COMPILER=%VCToolsInstallDir%bin\Hostx64\x64\cl.exe
set CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=%VCToolsInstallDir%bin\Hostx64\x64\link.exe
cd /d "$repo"
cargo build
"@
[System.IO.File]::WriteAllText($cargoScript, $cargoBody.Replace("cargo build", "cargo build --release"), [System.Text.Encoding]::ASCII)
try {
    & cmd /c $cargoScript
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
}
catch {
    if ((Test-Path $localImportLib) -and (Test-Path $localRuntimeDll)) {
        Write-Warning "Rust rebuild failed under MSVC. Reusing staged nu.dll and nu.dll.lib from examples/cpp."
        $importLib = $localImportLib
        $runtimeDll = $localRuntimeDll
    } else {
        throw
    }
}
finally {
    if (Test-Path $cargoScript) {
        Remove-Item $cargoScript -Force
    }
}

if (-not (Test-Path $importLib)) {
    throw "missing import library: $importLib"
}

if (-not (Test-Path $runtimeDll)) {
    throw "missing runtime dll: $runtimeDll"
}

$missingUcrt = $ucrtDlls | Where-Object { -not (Test-Path $_) }
if ($missingUcrt.Count -gt 0) {
    throw "missing UCRT runtime dlls: $($missingUcrt -join ', ')"
}

$env:PATH = "C:\msys64\ucrt64\bin;$env:PATH"

New-IcoFromPng -PngPath $iconPng -IcoPath $iconIco
@"
1 ICON "nu-icon.ico"
"@ | Set-Content -Path $resourceScript -Encoding ASCII

if ($importLib -ne $localImportLib) {
    Copy-Item $importLib $localImportLib -Force
}
if ($runtimeDll -ne $localRuntimeDll) {
    Copy-Item $runtimeDll $localRuntimeDll -Force
}
foreach ($dll in $ucrtDlls) {
    Copy-Item $dll (Join-Path $PSScriptRoot ([System.IO.Path]::GetFileName($dll))) -Force
}

& $windres `
    -i $resourceScript `
    -o $resourceObject

if ($LASTEXITCODE -ne 0) {
    throw "windres failed with exit code $LASTEXITCODE"
}

& $compiler `
    -std=c++17 `
    -Wall `
    -Wextra `
    -I $includeDir `
    -I $PSScriptRoot `
    $source `
    $resourceObject `
    $localImportLib `
    -o $output

if ($LASTEXITCODE -ne 0) {
    throw "g++ failed with exit code $LASTEXITCODE"
}

Write-Host "built $output"
