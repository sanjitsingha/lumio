<#
.SYNOPSIS
  Builds Lummo's Microsoft Store package (MSIX) from the release build.

.DESCRIPTION
  npm run build:msix                   # build Lummo, then the package
  npm run build:msix -- -SkipBuild     # package the lummo.exe built last
  npm run build:msix -- -Test          # placeholder identity: checks packaging only, not for the Store

  The package lands in src-tauri\target\msix\. The Store signs it, so it is
  uploaded unsigned. See docs\MICROSOFT_STORE.md.
#>
param([switch]$SkipBuild, [switch]$Test)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$tauri = Join-Path $root 'src-tauri'
$msix = Join-Path $tauri 'msix'
$out = Join-Path $tauri 'target\msix'
$layout = Join-Path $out 'layout'

function Fail($message) {
  Write-Host ''
  Write-Host "x $message" -ForegroundColor Red
  Write-Host ''
  exit 1
}

# The newest x64 build of a Windows SDK tool.
function Find-SdkTool($name) {
  $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
  $tool = Get-ChildItem $kits -Recurse -Filter $name -ErrorAction SilentlyContinue |
    Where-Object { $_.Directory.Name -eq 'x64' } |
    Sort-Object { try { [version]$_.Directory.Parent.Name } catch { [version]'0.0' } } -Descending |
    Select-Object -First 1
  if (-not $tool) { Fail "$name was not found. Install the Windows SDK: winget install Microsoft.WindowsSDK.10.0.26100" }
  $tool.FullName
}

# Partner Center gives these when the name is reserved (Product identity).
if ($Test) {
  $identity = [pscustomobject]@{ name = 'Lummo.PackagingTest'; publisher = 'CN=Lummo Packaging Test'; publisherDisplayName = 'Lummo' }
} else {
  $identity = Get-Content (Join-Path $msix 'identity.json') -Raw | ConvertFrom-Json
  if (-not $identity.name -or -not $identity.publisher -or -not $identity.publisherDisplayName) {
    Fail 'Fill in src-tauri\msix\identity.json with the three values of Partner Center > Lummo > Product identity (docs\MICROSOFT_STORE.md).'
  }
}

$cargo = Get-Content (Join-Path $tauri 'Cargo.toml') -Raw
if ($cargo -notmatch '(?m)^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"') { Fail 'No version found in src-tauri\Cargo.toml.' }
# the Store keeps the fourth part for itself; it must be 0
$version = "$($Matches[1]).$($Matches[2]).$($Matches[3]).0"

if (-not $SkipBuild) {
  Push-Location $root
  try {
    npx tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) { Fail 'The release build failed (its messages are above).' }
  } finally {
    Pop-Location
  }
}
$exe = Join-Path $tauri 'target\release\lummo.exe'
if (-not (Test-Path $exe)) { Fail "There is no release build at $exe; run without -SkipBuild." }

if (Test-Path $out) { Remove-Item $out -Recurse -Force }
New-Item -ItemType Directory -Force (Join-Path $layout 'Assets') | Out-Null
Copy-Item $exe $layout

# Tiles and the taskbar icon, from the app icon. The "unplated" sizes keep
# the taskbar and Start from drawing it small on a backplate.
Add-Type -AssemblyName System.Drawing
$source = [System.Drawing.Image]::FromFile((Join-Path $tauri 'icons\icon.png'))
$edges = New-Object System.Drawing.Imaging.ImageAttributes
$edges.SetWrapMode([System.Drawing.Drawing2D.WrapMode]::TileFlipXY)
function Save-Icon($size, $name) {
  $bitmap = New-Object System.Drawing.Bitmap $size, $size
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $graphics.InterpolationMode = 'HighQualityBicubic'
  $graphics.PixelOffsetMode = 'HighQuality'
  $graphics.CompositingQuality = 'HighQuality'
  $graphics.SmoothingMode = 'HighQuality'
  $target = New-Object System.Drawing.Rectangle 0, 0, $size, $size
  $graphics.DrawImage($source, $target, 0, 0, $source.Width, $source.Height, [System.Drawing.GraphicsUnit]::Pixel, $edges)
  $graphics.Dispose()
  $bitmap.Save((Join-Path $layout "Assets\$name"), [System.Drawing.Imaging.ImageFormat]::Png)
  $bitmap.Dispose()
}
Save-Icon 44 'Square44x44Logo.png'
foreach ($size in 16, 24, 32, 48, 256) { Save-Icon $size "Square44x44Logo.targetsize-$($size)_altform-unplated.png" }
Save-Icon 71 'Square71x71Logo.png'
Save-Icon 150 'Square150x150Logo.png'
Save-Icon 50 'StoreLogo.png'
$source.Dispose()

function Escape-Xml($text) { [System.Security.SecurityElement]::Escape([string]$text) }
$manifest = (Get-Content (Join-Path $msix 'AppxManifest.xml') -Raw -Encoding UTF8).
  Replace('{{NAME}}', (Escape-Xml $identity.name)).
  Replace('{{PUBLISHER}}', (Escape-Xml $identity.publisher)).
  Replace('{{PUBLISHER_DISPLAY_NAME}}', (Escape-Xml $identity.publisherDisplayName)).
  Replace('{{VERSION}}', $version)
[System.IO.File]::WriteAllText((Join-Path $layout 'AppxManifest.xml'), $manifest, (New-Object System.Text.UTF8Encoding $false))

# resources.pri tells Windows which icon file is which size
$makepri = Find-SdkTool 'makepri.exe'
$priConfig = Join-Path $out 'priconfig.xml'
& $makepri createconfig /cf $priConfig /dq en-US /pv 10.0.0 /o | Out-Null
if ($LASTEXITCODE -ne 0) { Fail 'makepri could not create its configuration.' }
& $makepri new /pr $layout /cf $priConfig /mn (Join-Path $layout 'AppxManifest.xml') /of (Join-Path $layout 'resources.pri') /o | Out-Null
if ($LASTEXITCODE -ne 0) { Fail 'makepri could not index the icons.' }

$makeappx = Find-SdkTool 'makeappx.exe'
$package = Join-Path $out "Lummo_$($version)_x64.msix"
& $makeappx pack /o /h SHA256 /d $layout /p $package
if ($LASTEXITCODE -ne 0) { Fail 'makeappx could not build the package (its messages are above).' }

Write-Host ''
Write-Host "Package: $package" -ForegroundColor Green
Write-Host "Version: $version   Identity: $($identity.name)"
if ($Test) {
  Write-Host 'This was a packaging test with a placeholder identity: the Store will not accept it.' -ForegroundColor Yellow
} else {
  Write-Host 'Upload it in Partner Center > Lummo > Packages (docs\MICROSOFT_STORE.md).'
}
