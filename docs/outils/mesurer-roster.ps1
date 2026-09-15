# Mesure le CPU d'un roster donne, dans UN SEUL processus.
#
# Remplace `mesurer-n.ps1`, qui lancait N instances parce que N personnages
# n'existaient pas encore. Depuis l'etape « plusieurs personnages », une seule
# instance porte tout le roster : la mesure est enfin celle du vrai programme,
# et non d'une approximation par N processus.
#
# ⚠️ Protocole de CLAUDE.md : 40 a 60 s, JAMAIS 10. Et le chiffre « en marche »
# ne se compare pas d'une version a l'autre — il mesure le COMPORTEMENT, pas le
# code. Le releve qui se compare est celui en mode cache (-Cache), ou le taux de
# deplacement de SHIMEJI_CADENCE=1 qui accompagne le chiffre brut.

param(
  # Ex. : "blob", "blob,blob,blob", ou 10 fois blob.
  [string]$Roster = "blob",
  [switch]$Cache,
  [switch]$Cadence,
  [int]$Secondes = 60
)

# Trois niveaux : le script est dans <racine>\docs\outils\.
$racine = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSCommandPath))
$exe = Join-Path $racine "src-tauri\target\release\shimeji-desktop.exe"
$sortie = Join-Path $env:TEMP "shimeji-mesure-roster.txt"

Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

$env:SHIMEJI_PERSONNAGES = $Roster
$env:SHIMEJI_QUITTER_APRES = "$($Secondes + 25)"
if ($Cache) { $env:SHIMEJI_CACHE = "1" } else { Remove-Item Env:\SHIMEJI_CACHE -ErrorAction SilentlyContinue }
if ($Cadence) { $env:SHIMEJI_CADENCE = "1" } else { Remove-Item Env:\SHIMEJI_CADENCE -ErrorAction SilentlyContinue }

Start-Process -FilePath $exe -RedirectStandardOutput $sortie -WindowStyle Hidden

# Laisser les fenetres se creer, les personnages TOMBER et atterrir avant de
# compter : la chute initiale deplace a chaque image, et fausserait une mesure
# qui commencerait trop tot.
Start-Sleep -Seconds 10

$p = Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue
if (-not $p) { Write-Output "le processus n'a pas demarre"; return }

$avant = $p.CPU
Start-Sleep -Seconds $Secondes
$p.Refresh()
$cpu = (($p.CPU - $avant) / $Secondes) * 100

$n = ($Roster -split ',').Count
$mode = if ($Cache) { "cache" } else { "en marche" }
Write-Output "roster de $n ($mode) : $([math]::Round($cpu,1)) % d'un coeur, soit $([math]::Round($cpu / $n, 2)) % par personnage"

if ($Cadence) {
  # Le taux de deplacement, sans lequel le chiffre « en marche » ne veut rien
  # dire : c'est lui qui explique tout (voir docs/specs/2026-09-09-mesure-cpu.md).
  $placements = 0; $images = 0
  foreach ($ligne in (Get-Content $sortie -ErrorAction SilentlyContinue)) {
    if ($ligne -match '(\d+) placements sur (\d+) images') {
      $placements += [int]$Matches[1]; $images += [int]$Matches[2]
    }
  }
  if ($images -gt 0) {
    Write-Output "      $placements placements sur $images images ($([math]::Round($placements * 100.0 / $images, 0)) %), soit $([math]::Round($placements / $Secondes, 0)) placements/s"
  }
}

Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
