# Sonde Win32 — interroge Windows sur la fenêtre du spike PENDANT qu'il tourne.
#
# Répond objectivement aux propriétés 3 (hors Alt+Tab), 4 (clics traversants) et
# 5 (pas de vol de focus) de l'étape 0 : un style de fenêtre se demande au système,
# il ne se constate pas à la souris. Voir ../specs/2026-09-08-spike-0-resultat.md.
#
# Conservée — contrairement au reste du spike — parce qu'elle reste utile deux fois :
#   · comparer avec l'application si la transparence ou le premier plan se dérèglent ;
#   · fermer la seule inconnue de l'étape 0, le multi-DPI, sur une machine dont les
#     écrans n'ont pas la même échelle (les deux d'ici sont à l'échelle 1).
#
# Usage : lancer le spike (`cargo run`), puis ce script dans un second terminal.

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class Probe {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)]
    public struct POINT { public int X, Y; }

    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int i);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int max);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int max);

    public static string Describe(IntPtr h) {
        var cls = new System.Text.StringBuilder(256);
        GetClassName(h, cls, 256);
        var txt = new System.Text.StringBuilder(256);
        GetWindowText(h, txt, 256);
        return "classe=" + cls.ToString() + " titre='" + txt.ToString() + "'";
    }
}
'@

# La fenêtre du spike, retrouvée par le titre posé dans main.rs.
$proc = Get-Process -Name spike-overlay -ErrorAction SilentlyContinue
if (-not $proc) { Write-Output "spike-overlay ne tourne pas"; exit 1 }

# Le processus a plusieurs fenêtres (WebView2) ; on veut celle qui a un titre.
$hwnd = [IntPtr]::Zero
foreach ($p in $proc) {
    if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $hwnd = $p.MainWindowHandle }
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Output "aucune fenetre principale trouvee"; exit 1 }

Write-Output "hwnd du spike : $hwnd  ($([Probe]::Describe($hwnd)))"

# --- Styles etendus (GWL_EXSTYLE = -20) ---------------------------------
$ex = [Probe]::GetWindowLong($hwnd, -20)
Write-Output ("exstyle brut : 0x{0:X8}" -f $ex)

$flags = @{
    'WS_EX_TRANSPARENT (clics traversants)' = 0x00000020
    'WS_EX_LAYERED     (composition alpha)' = 0x00080000
    'WS_EX_TOOLWINDOW  (hors Alt+Tab)'      = 0x00000080
    'WS_EX_NOACTIVATE  (pas de vol focus)'  = 0x08000000
    'WS_EX_TOPMOST     (premier plan)'      = 0x00000008
}
foreach ($k in $flags.Keys | Sort-Object) {
    $present = ($ex -band $flags[$k]) -ne 0
    $mark = if ($present) { 'OUI' } else { 'non' }
    Write-Output ("  {0,-40} {1}" -f $k, $mark)
}

# --- WindowFromPoint au centre du personnage ----------------------------
# La fenetre se deplace a 300 px/s ; on echantillonne plusieurs fois pour
# que le resultat ne depende pas d'un instant particulier.
Write-Output ""
Write-Output "WindowFromPoint au centre de la fenetre, 5 echantillons :"
for ($i = 0; $i -lt 5; $i++) {
    $r = New-Object Probe+RECT
    [void][Probe]::GetWindowRect($hwnd, [ref]$r)
    $pt = New-Object Probe+POINT
    $pt.X = [int](($r.Left + $r.Right) / 2)
    $pt.Y = [int](($r.Top + $r.Bottom) / 2)
    $under = [Probe]::WindowFromPoint($pt)
    $verdict = if ($under -eq $hwnd) { 'BLOQUE (la fenetre capture le clic)' } else { 'TRAVERSE' }
    Write-Output ("  ({0,5},{1,5}) -> hwnd {2,-10} {3}  [{4}]" -f $pt.X, $pt.Y, $under, $verdict, [Probe]::Describe($under))
    Start-Sleep -Milliseconds 200
}
