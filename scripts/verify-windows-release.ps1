<# Vérifie ce qui est distribué : installeur, payload complet et empreintes du flux Velopack. #>
param(
    [Parameter(Mandatory)][string]$ReleaseDirectory,
    [Parameter(Mandatory)][ValidateSet("win-x64", "win-arm64")][string]$Rid,
    [Parameter(Mandatory)][string]$SignToolPath
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$ReleaseDirectory = (Resolve-Path -LiteralPath $ReleaseDirectory).Path
$SignToolPath = (Resolve-Path -LiteralPath $SignToolPath).Path
$setup = Join-Path $ReleaseDirectory "DmxMoney-$Rid-Setup.exe"
$feedPath = Join-Path $ReleaseDirectory "releases.$Rid.json"
if (-not (Test-Path -LiteralPath $setup -PathType Leaf)) { throw "Installeur $Rid absent." }

function Assert-TrustedSignature([string]$Path) {
    # /pa applique la politique Authenticode ; /all vérifie les signatures ; /tw exige un timestamp.
    # SignTool renvoie 2 pour un avertissement : lui aussi fait échouer la release.
    & $SignToolPath verify /pa /all /tw $Path
}

Assert-TrustedSignature $setup
$feed = Get-Content -LiteralPath $feedPath -Raw | ConvertFrom-Json
$fullPackages = @($feed.Assets | Where-Object { $_.Type -eq "Full" })
if ($fullPackages.Count -ne 1) { throw "Le flux $Rid doit contenir exactement un paquet complet." }
foreach ($asset in $feed.Assets) {
    if (-not $asset.FileName -or [System.IO.Path]::GetFileName($asset.FileName) -ne $asset.FileName -or
        $asset.FileName -match '[/\\:]' -or $asset.FileName -in @(".", "..")) {
        throw "Nom de paquet invalide dans le flux $Rid."
    }
    $packagePath = Join-Path $ReleaseDirectory $asset.FileName
    $stream = [System.IO.File]::OpenRead($packagePath)
    try {
        $digest = [Convert]::ToBase64String([System.Security.Cryptography.SHA256]::HashData($stream))
        if ($stream.Length -ne $asset.Size -or $digest -cne $asset.SHA256) {
            throw "Taille ou empreinte SHA-256 incorrecte : $($asset.FileName)."
        }
    }
    finally {
        $stream.Dispose()
    }
}

$extraction = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString("N"))
try {
    [System.IO.Compression.ZipFile]::ExtractToDirectory((Join-Path $ReleaseDirectory $fullPackages[0].FileName), $extraction)
    $binaries = @(Get-ChildItem -LiteralPath $extraction -File -Recurse |
        Where-Object { $_.Extension -in @(".exe", ".dll") })
    if (-not ($binaries | Where-Object Name -eq "DmxMoney.exe") -or
        -not ($binaries | Where-Object Name -eq "dmx_ffi.dll") -or
        -not ($binaries | Where-Object Name -eq "Squirrel.exe")) {
        throw "Paquet incomplet : application, noyau ou moteur de mise à jour absent."
    }
    foreach ($binary in $binaries) { Assert-TrustedSignature $binary.FullName }
    Write-Host "==> $Rid : installeur et $($binaries.Count) binaires signés, horodatés et vérifiés ; SHA-256 valide."
}
finally {
    if (Test-Path -LiteralPath $extraction) { Remove-Item -LiteralPath $extraction -Recurse -Force }
}
