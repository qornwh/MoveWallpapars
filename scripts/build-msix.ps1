# Packs the release build into an MSIX for Microsoft Store submission.
#
# Run `npm run tauri build` first. The Identity values must match the ones shown
# in Partner Center > Product identity; the defaults are the values assigned to
# this app (Store ID 9NZ11XWF7VB7).
#
#   powershell -File scripts/build-msix.ps1 `
#     -IdentityName "12345Publisher.MoveWallpapers" `
#     -Publisher "CN=XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX" `
#     -PublisherDisplayName "Publisher"

param(
    [string]$IdentityName = "coffeehanaro.MoveWallpapers",
    [string]$Publisher = "CN=C5B5FEA3-813E-4922-BB30-CD2ABC4F9DE2",
    [string]$PublisherDisplayName = "coffeehanaro",
    [string]$DisplayName = "MoveWallpapers"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$tauriDir = Join-Path $root "src-tauri"
$exe = Join-Path $tauriDir "target\release\move-wallpapers.exe"
$icons = Join-Path $tauriDir "icons"
$outDir = Join-Path $tauriDir "target\msix"
$layout = Join-Path $outDir "layout"

if (-not (Test-Path $exe)) { throw "Release exe not found. Run 'npm run tauri build' first: $exe" }

# MSIX versions are four-part and the Store requires the last part to be 0.
$conf = Get-Content (Join-Path $tauriDir "tauri.conf.json") -Raw | ConvertFrom-Json
$version = "$($conf.version).0"

# Newest Windows SDK that has the packaging tools.
$sdkBin = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Directory |
    Where-Object { Test-Path (Join-Path $_.FullName "x64\makeappx.exe") } |
    Sort-Object Name -Descending | Select-Object -First 1
if (-not $sdkBin) { throw "Windows SDK (makeappx.exe) not found." }
$makeappx = Join-Path $sdkBin.FullName "x64\makeappx.exe"
$makepri = Join-Path $sdkBin.FullName "x64\makepri.exe"

if (Test-Path $outDir) { Remove-Item -Recurse -Force $outDir }
$assets = Join-Path $layout "Assets"
New-Item -ItemType Directory -Force $assets | Out-Null

Copy-Item $exe $layout

# Tile and Store logos come from the `tauri icon` output.
foreach ($name in "StoreLogo.png", "Square44x44Logo.png", "Square71x71Logo.png",
                  "Square150x150Logo.png") {
    Copy-Item (Join-Path $icons $name) $assets
}
# Unplated variants so the taskbar and Start list show the icon without a tile behind it.
Copy-Item (Join-Path $icons "32x32.png") (Join-Path $assets "Square44x44Logo.targetsize-32_altform-unplated.png")
Copy-Item (Join-Path $icons "64x64.png") (Join-Path $assets "Square44x44Logo.targetsize-64_altform-unplated.png")
Copy-Item (Join-Path $icons "128x128@2x.png") (Join-Path $assets "Square44x44Logo.targetsize-256_altform-unplated.png")

$manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap rescap">
  <Identity Name="$IdentityName" Publisher="$Publisher" Version="$version" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>$DisplayName</DisplayName>
    <PublisherDisplayName>$PublisherDisplayName</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Resources>
    <Resource Language="ko-kr" />
    <Resource Language="en-us" />
  </Resources>
  <Applications>
    <Application Id="MoveWallpapers" Executable="move-wallpapers.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="$DisplayName"
        Description="Photo and GIF slideshow behind your desktop icons"
        BackgroundColor="transparent"
        Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png">
        <uap:DefaultTile Square71x71Logo="Assets\Square71x71Logo.png" />
      </uap:VisualElements>
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"@
[IO.File]::WriteAllText((Join-Path $layout "AppxManifest.xml"), $manifest, (New-Object Text.UTF8Encoding $false))

# resources.pri is what makes the qualified (targetsize/unplated) asset names resolve.
$priConfig = Join-Path $outDir "priconfig.xml"
& $makepri createconfig /cf $priConfig /dq en-US /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri createconfig failed" }
& $makepri new /pr $layout /cf $priConfig /mn (Join-Path $layout "AppxManifest.xml") /of (Join-Path $layout "resources.pri") /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri new failed" }

$msix = Join-Path $outDir "MoveWallpapers_${version}_x64.msix"
& $makeappx pack /d $layout /p $msix /o
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed" }

Write-Host ""
Write-Host "MSIX: $msix"
Write-Host "Identity: Name=$IdentityName  Publisher=$Publisher  Version=$version"
