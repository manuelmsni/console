<#
.SYNOPSIS
Publica una versión nueva de Console.

.DESCRIPTION
1. Comprueba que no hay cambios sin confirmar y que la versión es la de Cargo.toml (si aún
   no se ha publicado) o una mayor.
2. Pasa los tests de la versión normal y de la portable.
3. Pone la versión en Cargo.toml, crea el commit y la etiqueta vX.Y.Z (con las novedades
   como mensaje) y lo sube a GitHub.
4. Al llegar la etiqueta, GitHub Actions (.github/workflows/release.yml) compila todos los
   tipos de programa y publica la release. Las apps instaladas la verán solas.

No guarda ninguna credencial: la subida usa tu acceso normal a git. La primera vez,
Git Credential Manager te pedirá iniciar sesión en GitHub (en el navegador) y lo recordará.

.EXAMPLE
./scripts/release.ps1 1.2.0 -Notes "Ctrl + clic abre los enlaces"
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    # Lo que ha cambiado. Si no se da, se pregunta.
    [string]$Notes
)

# Cada paso se comprueba por su código de salida (Invoke-Checked). Con 'Stop', Windows
# PowerShell 5.1 cortaría el script con cualquier línea que git escriba en stderr.
$ErrorActionPreference = 'Continue'
Set-Location (Split-Path -Parent $PSScriptRoot)

function Stop-Release([string]$Reason) {
    Write-Host "No se publica: $Reason" -ForegroundColor Red
    exit 1
}

function Invoke-Checked([string]$Description, [scriptblock]$Command) {
    Write-Host "==> $Description" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) { Stop-Release "falló: $Description" }
}

# --- Comprobaciones -----------------------------------------------------------
if ($Version -notmatch '^\d+\.\d+\.\d+$') { Stop-Release "la versión debe ser X.Y.Z (por ejemplo 1.2.0)" }
$tag = "v$Version"

git rev-parse --is-inside-work-tree 2> $null | Out-Null
if ($LASTEXITCODE -ne 0) { Stop-Release "esta carpeta no es un repositorio git" }
if (-not (git remote get-url origin 2> $null)) { Stop-Release "no hay remoto 'origin' (git remote add origin https://github.com/<usuario>/<repo>.git)" }
if (git status --porcelain) { Stop-Release "hay cambios sin confirmar; haz commit o descártalos antes" }
if (git tag --list $tag) { Stop-Release "la etiqueta $tag ya existe" }

$cargoToml = Get-Content Cargo.toml -Raw
$current = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"(\d+\.\d+\.\d+)"').Groups[1].Value
# Se puede publicar la versión que ya tiene Cargo.toml (por ejemplo, la primera) o una mayor.
if ([version]$Version -lt [version]$current) { Stop-Release "$Version es menor que la versión actual ($current)" }
$versionChanges = [version]$Version -gt [version]$current

if (-not $Notes) {
    $Notes = Read-Host "Novedades de la $tag (una línea; Enter para dejarlo vacío)"
}
if (-not $Notes) { $Notes = "Versión $Version" }

# --- Tests ----------------------------------------------------------------------
Invoke-Checked "Tests (versión normal)" { cargo test --quiet }
Invoke-Checked "Tests (versión portable)" { cargo test --quiet --features portable }

# --- Versión, commit, etiqueta y subida -----------------------------------------
if ($versionChanges) {
    Write-Host "==> Versión $current -> $Version" -ForegroundColor Cyan
    $updated = [regex]::Replace($cargoToml, '(?m)^version\s*=\s*"\d+\.\d+\.\d+"', "version = `"$Version`"", 1)
    [System.IO.File]::WriteAllText((Resolve-Path Cargo.toml), $updated, (New-Object System.Text.UTF8Encoding $false))
    Invoke-Checked "Actualizar Cargo.lock" { cargo check --quiet }
    Invoke-Checked "Commit" { git add Cargo.toml Cargo.lock; git commit -m "Versión $Version" }
} else {
    Write-Host "==> Se publica la versión que ya tiene Cargo.toml ($Version)" -ForegroundColor Cyan
}
Invoke-Checked "Etiqueta $tag" { git tag -a $tag -m $Notes }
Invoke-Checked "Subir la rama" { git push origin HEAD }
Invoke-Checked "Subir la etiqueta" { git push origin $tag }

$repository = (git remote get-url origin) -replace '^.*github\.com[:/]', '' -replace '\.git$', ''
Write-Host ""
Write-Host "Hecho. GitHub está compilando y publicando la ${tag}:" -ForegroundColor Green
Write-Host "  https://github.com/$repository/actions"
Write-Host "  https://github.com/$repository/releases/tag/$tag"