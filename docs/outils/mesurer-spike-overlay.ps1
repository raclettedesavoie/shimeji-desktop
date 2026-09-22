# Mesure une phase du spike « une fenêtre par écran ».
#
# Relève le CPU du processus ET de son arbre WebView2 — les deux, parce que la
# mesure du 2026-09-22 a montré que nos renderers pesaient 31 % à eux seuls, une
# part que le dossier CPU du projet n'avait jamais isolée.
#
# Usage :
#   .\docs\outils\mesurer-spike-overlay.ps1 -Phase 0
#   .\docs\outils\mesurer-spike-overlay.ps1 -Phase 1
#   .\docs\outils\mesurer-spike-overlay.ps1 -Phase 2 -N 15 -Secondes 60

param(
  [Parameter(Mandatory = $true)][int]$Phase,
  [int]$N = 15,
  [int]$Secondes = 60
)

$exe = Join-Path $PSScriptRoot '..\..\src-tauri\target\release\shimeji-desktop.exe'
$exe = (Resolve-Path $exe).Path
if (-not (Test-Path $exe)) { throw "binaire absent : $exe" }

$sortie = Join-Path $env:TEMP "spike-overlay-phase$Phase.txt"
$erreur = Join-Path $env:TEMP "spike-overlay-phase$Phase.err.txt"

$env:SHIMEJI_SPIKE_OVERLAY = "$Phase"
$env:SHIMEJI_SPIKE_N = "$N"
$env:SHIMEJI_SPIKE_SECONDES = "$Secondes"

Write-Host "=== phase $Phase · $N figurants · $Secondes s ===" -ForegroundColor Cyan

$p = Start-Process -FilePath $exe -PassThru -RedirectStandardOutput $sortie -RedirectStandardError $erreur

# Laisser les fenêtres et les webviews se créer avant de commencer à compter :
# le démarrage de WebView2 est un pic qui n'a rien à voir avec le régime
# permanent qu'on mesure.
Start-Sleep -Seconds 5
if ($p.HasExited) { Write-Host "le processus s'est arrêté au démarrage" -ForegroundColor Red; Get-Content $erreur; exit 1 }

function Arbre-Webview($parentPid) {
  $racines = (Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
              Where-Object { $_.ParentProcessId -eq $parentPid }).ProcessId
  if (-not $racines) { return @() }
  $enfants = (Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
              Where-Object { $_.ParentProcessId -in $racines }).ProcessId
  return @($racines) + @($enfants)
}

$wv = Arbre-Webview $p.Id
$snap = @{}
foreach ($i in $wv) { $pr = Get-Process -Id $i -ErrorAction SilentlyContinue; if ($pr) { $snap[$i] = $pr.CPU } }
$p.Refresh(); $c0 = $p.CPU; $t0 = Get-Date

# On s'arrête un peu avant que l'application ne se ferme d'elle-même, pour lire
# les compteurs pendant qu'ils existent encore.
$attente = [Math]::Max(10, $Secondes - 8)
Start-Sleep -Seconds $attente

$dt = ((Get-Date) - $t0).TotalSeconds
$sommeWv = 0
foreach ($i in $wv) {
  $pr = Get-Process -Id $i -ErrorAction SilentlyContinue
  if ($pr -and $snap.ContainsKey($i)) { $sommeWv += ($pr.CPU - $snap[$i]) }
}
$cpuProc = 0
if (-not $p.HasExited) { $p.Refresh(); $cpuProc = (($p.CPU - $c0) / $dt) * 100 }
$cpuWv = ($sommeWv / $dt) * 100

Write-Host ""
Write-Host ("processus webview : {0}" -f $wv.Count)
Write-Host ("shimeji-desktop   : {0} %" -f [math]::Round($cpuProc, 1))
Write-Host ("ses webviews      : {0} %" -f [math]::Round($cpuWv, 1))
Write-Host ("TOTAL             : {0} %" -f [math]::Round($cpuProc + $cpuWv, 1)) -ForegroundColor Yellow

# Laisser le spike finir et imprimer son verdict.
$p.WaitForExit(30000) | Out-Null
Write-Host ""
Get-Content $sortie -Encoding UTF8 | Select-Object -Last 12
if (Test-Path $erreur) {
  $err = Get-Content $erreur -Encoding UTF8
  if ($err) { Write-Host "`n--- stderr ---" -ForegroundColor DarkGray; $err | Select-Object -Last 10 }
}
