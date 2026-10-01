<# Prépare SignTool + le client Microsoft Artifact Signing ; aucune clé privée n'est exportée. #>
param(
    [Parameter(Mandatory)][string]$Endpoint,
    [Parameter(Mandatory)][string]$AccountName,
    [Parameter(Mandatory)][string]$CertificateProfileName,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$endpointUri = [uri]$Endpoint
if ($endpointUri.Scheme -ne "https" -or $endpointUri.Host -notmatch '^[a-z0-9]+\.codesigning\.azure\.net$' -or
    $endpointUri.UserInfo -or $endpointUri.Query -or $endpointUri.Fragment -or $endpointUri.AbsolutePath -ne "/") {
    throw "Endpoint Artifact Signing invalide : utilisez l'URL HTTPS de la région du compte Azure."
}
if ([string]::IsNullOrWhiteSpace($AccountName) -or [string]::IsNullOrWhiteSpace($CertificateProfileName)) {
    throw "Le compte et le profil de certificat Public Trust sont requis."
}

$sdkDirectory = Join-Path ${env:ProgramFiles(x86)} "Windows Kits/10/bin"
$signTool = Get-ChildItem -LiteralPath $sdkDirectory -Directory |
    Where-Object { $_.Name -match '^10\.0\.[0-9]+\.[0-9]+$' -and [version]$_.Name -ge [version]"10.0.22621.0" } |
    Sort-Object { [version]$_.Name } -Descending |
    ForEach-Object { Join-Path $_.FullName "x64/signtool.exe" } |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
    Select-Object -First 1
if (-not $signTool) { throw "SignTool x64 du Windows SDK 10.0.22621 ou supérieur est requis." }

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$package = Join-Path $OutputDirectory "microsoft.artifactsigning.client.1.0.128.nupkg"
Invoke-WebRequest -Uri "https://api.nuget.org/v3-flatcontainer/microsoft.artifactsigning.client/1.0.128/microsoft.artifactsigning.client.1.0.128.nupkg" -OutFile $package
# Refuse un package dont la signature NuGet n'est pas vérifiable avant de charger son DLL.
dotnet nuget verify --all $package
$clientDirectory = Join-Path $OutputDirectory "client"
[System.IO.Compression.ZipFile]::ExtractToDirectory($package, $clientDirectory, $true)
$dlib = Join-Path $clientDirectory "bin/x64/Azure.CodeSigning.Dlib.dll"
if (-not (Test-Path -LiteralPath $dlib -PathType Leaf)) { throw "dlib Artifact Signing x64 absent du package Microsoft." }

$metadata = Join-Path $OutputDirectory "metadata.json"
@{
    Endpoint = $endpointUri.AbsoluteUri
    CodeSigningAccountName = $AccountName
    CertificateProfileName = $CertificateProfileName
    # azure/login utilise OIDC pour authentifier la CLI ; aucune connexion interactive ni secret.
    ExcludeCredentials = @(
        "EnvironmentCredential", "ManagedIdentityCredential", "WorkloadIdentityCredential",
        "SharedTokenCacheCredential", "VisualStudioCredential", "VisualStudioCodeCredential",
        "AzurePowerShellCredential", "AzureDeveloperCliCredential", "InteractiveBrowserCredential"
    )
} | ConvertTo-Json | Set-Content -LiteralPath $metadata -Encoding utf8NoBOM

if ($env:GITHUB_OUTPUT) {
    "signtool=$signTool" >> $env:GITHUB_OUTPUT
    "dlib=$dlib" >> $env:GITHUB_OUTPUT
    "metadata=$metadata" >> $env:GITHUB_OUTPUT
}
Write-Host "==> Outils de signature prêts ; compte $AccountName, profil $CertificateProfileName."
