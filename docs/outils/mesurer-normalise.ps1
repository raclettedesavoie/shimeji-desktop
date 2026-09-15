# Mesure NORMALISEE : le cout d'UN deplacement, pas le cout d'une tranche.
#
# Le dossier CPU etablit que le CPU « en marche » ne se compare pas d'une
# mesure a l'autre : il mesure le COMPORTEMENT (combien de fois le personnage
# a bouge), pas le code. La mesure qui se compare est donc :
#
#     (cpu_en_marche - cpu_cache) / placements_par_seconde
#
# SHIMEJI_CADENCE=1 donne les placements. Il n'ecrit que sur la console, que
# le build release n'a pas : on mesure donc en debug, ce que le dossier CPU
# autorise (12,3 % debug contre 12 % release — l'ecart est dans le bruit).

param([int]$N = 1, [int]$Secondes = 60)

$exe = "C:\Users\alri\Documents\shimeji-desktop\src-tauri\target\debug\shimeji-desktop.exe"
$sortie = "C:\Users\alri\AppData\Local\Temp\claude\c--Users-alri-Documents-shimeji-desktop\8b32700f-19f0-44d2-b0d6-72237bfb422d\scratchpad"

Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

$env:SHIMEJI_CADENCE = "1"
$env:SHIMEJI_QUITTER_APRES = "$($Secondes + 25)"
Remove-Item Env:\SHIMEJI_CACHE -ErrorAction SilentlyContinue

for ($i = 0; $i -lt $N; $i++) {
  Start-Process -FilePath $exe -RedirectStandardOutput "$sortie\cadence-$N-$i.txt" -WindowStyle Hidden
  Start-Sleep -Milliseconds 400
}
Start-Sleep -Seconds 8

$procs = @(Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue)
$avant = ($procs | Measure-Object -Property CPU -Sum).Sum
Start-Sleep -Seconds $Secondes
foreach ($p in $procs) { $p.Refresh() }
$apres = ($procs | Measure-Object -Property CPU -Sum).Sum
$cpu = (($apres - $avant) / $Secondes) * 100

Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

# Sommer les placements de toutes les instances. Chaque ligne de trace dit
# « X placements sur Y images » pour une tranche de quelques secondes ; on
# additionne tout, sur tous les processus.
$placements = 0
$images = 0
for ($i = 0; $i -lt $N; $i++) {
  foreach ($ligne in (Get-Content "$sortie\cadence-$N-$i.txt" -ErrorAction SilentlyContinue)) {
    if ($ligne -match '(\d+) placements sur (\d+) images') {
      $placements += [int]$Matches[1]
      $images     += [int]$Matches[2]
    }
  }
}

$parSeconde = [math]::Round($placements / $Secondes, 0)
Write-Output "N=$N : CPU $([math]::Round($cpu,1)) %, $placements placements ($parSeconde/s), $images images, taux $([math]::Round($placements * 100.0 / [math]::Max($images,1),0)) %"
if ($placements -gt 0) {
  # 0,8 % par processus = notre calcul, mesure en mode cache.
  $moves = $cpu - (0.8 * $N)
  Write-Output "      -> deplacements seuls : $([math]::Round($moves,1)) % pour $parSeconde placements/s, soit $([math]::Round($moves / [math]::Max($parSeconde,1) * 1000, 1)) millipoints de CPU par placement"
}
