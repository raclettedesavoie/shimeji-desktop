# Mesure le cout CPU de N personnages, AVANT de les avoir implementes.
#
# Le principe : N instances du binaire release = N fenetres en couche
# deplacees independamment a 60 Hz. C'est exactement la charge que produira
# une boucle unique pilotant N personnages, PLUS (N-1) fois notre calcul
# partage (0,9 %) que le vrai design paiera une seule fois. La mesure est
# donc une BORNE HAUTE, et l'ecart est connu et soustrayable.
#
# Protocole de CLAUDE.md : 60 s, jamais 10.

param(
  [int]$N = 1,
  [switch]$Cache,
  [int]$Secondes = 60
)

$exe = "C:\Users\alri\Documents\shimeji-desktop\src-tauri\target\release\shimeji-desktop.exe"

# On part d'une table rase : une instance oubliee d'une mesure precedente
# fausserait tout, et c'est le genre d'erreur qu'on ne voit pas.
Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

if ($Cache) { $env:SHIMEJI_CACHE = "1" } else { Remove-Item Env:\SHIMEJI_CACHE -ErrorAction SilentlyContinue }
# Chaque instance se termine seule : pas de processus orphelin si la mesure
# est interrompue.
$env:SHIMEJI_QUITTER_APRES = "$($Secondes + 25)"

for ($i = 0; $i -lt $N; $i++) {
  Start-Process -FilePath $exe -WindowStyle Hidden
  Start-Sleep -Milliseconds 400
}

# Laisser les fenetres se creer et le comportement demarrer avant de compter.
Start-Sleep -Seconds 8

$procs = @(Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue)
if ($procs.Count -ne $N) {
  Write-Output "ATTENTION : $($procs.Count) processus au lieu de $N"
}

$avant = ($procs | Measure-Object -Property CPU -Sum).Sum
Start-Sleep -Seconds $Secondes
foreach ($p in $procs) { $p.Refresh() }
$apres = ($procs | Measure-Object -Property CPU -Sum).Sum

$pourcent = [math]::Round((($apres - $avant) / $Secondes) * 100, 1)
$mode = if ($Cache) { "cache" } else { "en marche" }
Write-Output "N=$N $mode : $pourcent % d'un coeur au total, soit $([math]::Round($pourcent / $N, 1)) % par personnage"

Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
