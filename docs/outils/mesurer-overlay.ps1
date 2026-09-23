# Mesure l'application overlay : CPU du processus ET de son arbre WebView2,
# plus la latence de la file du thread principal.
#
# ⚠️ `mesurer-roster.ps1` ne relève que le processus principal. Depuis la
# migration du 2026-09-23, ça ne suffit plus : l'essentiel du coût est dans les
# renderers, qui sont des processus séparés.
#
# Usage :
#   .\docs\outils\mesurer-overlay.ps1 -Roster "blob,blob" -Secondes 60
#   .\docs\outils\mesurer-overlay.ps1 -N 15 -Cache

param(
  [string]$Roster = "",
  [int]$N = 0,
  [int]$Secondes = 60,
  [switch]$Cache
)

if ($N -gt 0) { $Roster = (@("blob") * $N) -join "," }
if (-not $Roster) { throw "donner -Roster ou -N" }

$exe = (Resolve-Path (Join-Path $PSScriptRoot '..\..\src-tauri\target\release\shimeji-desktop.exe')).Path

$sortie = Join-Path $env:TEMP "overlay-mesure.txt"
$erreur = Join-Path $env:TEMP "overlay-mesure.err.txt"

$env:SHIMEJI_PERSONNAGES = $Roster
$env:SHIMEJI_SIGNAUX = "1"
if ($Cache) { $env:SHIMEJI_CACHE = "1" } else { Remove-Item Env:\SHIMEJI_CACHE -ErrorAction SilentlyContinue }

$nb = ($Roster -split ",").Count
Write-Host "=== $nb personnage(s) · $Secondes s · $(if ($Cache) { 'CACHE' } else { 'visible' }) ===" -ForegroundColor Cyan

Get-Process shimeji-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

$p = Start-Process -FilePath $exe -PassThru -RedirectStandardOutput $sortie -RedirectStandardError $erreur

# Laisser les fenêtres et les webviews se créer : leur démarrage est un pic
# qui n'a rien à voir avec le régime permanent qu'on mesure.
Start-Sleep -Seconds 8
if ($p.HasExited) { Write-Host "arret au demarrage" -ForegroundColor Red; Get-Content $erreur; exit 1 }

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

Start-Sleep -Seconds $Secondes

$dt = ((Get-Date) - $t0).TotalSeconds
$sommeWv = 0
foreach ($i in $wv) {
  $pr = Get-Process -Id $i -ErrorAction SilentlyContinue
  if ($pr -and $snap.ContainsKey($i)) { $sommeWv += ($pr.CPU - $snap[$i]) }
}
$p.Refresh()
$cpuProc = (($p.CPU - $c0) / $dt) * 100
$cpuWv = ($sommeWv / $dt) * 100

Write-Host ""
Write-Host ("processus webview : {0}" -f $wv.Count)
Write-Host ("shimeji-desktop   : {0} %" -f [math]::Round($cpuProc, 1))
Write-Host ("ses webviews      : {0} %" -f [math]::Round($cpuWv, 1))
Write-Host ("TOTAL             : {0} %" -f [math]::Round($cpuProc + $cpuWv, 1)) -ForegroundColor Yellow
Write-Host ("repond            : {0}" -f $p.Responding)

# La latence de la file : SHIMEJI_SIGNAUX=1 l'imprime deux fois par seconde.
$lat = Get-Content $sortie -Encoding UTF8 |
       Select-String -Pattern 'latence[^0-9]*([0-9]+)' -AllMatches |
       ForEach-Object { [int]$_.Matches[0].Groups[1].Value }
if ($lat) {
  $tri = $lat | Sort-Object
  Write-Host ""
  Write-Host ("latence mediane   : {0} ms" -f $tri[[int]($tri.Count / 2)]) -ForegroundColor Yellow
  Write-Host ("latence max       : {0} ms" -f ($tri | Select-Object -Last 1)) -ForegroundColor Yellow
  Write-Host ("releves           : {0}" -f $lat.Count)
} else {
  Write-Host "aucune latence relevee (SHIMEJI_SIGNAUX a-t-il ete lu ?)" -ForegroundColor DarkGray
}

Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
