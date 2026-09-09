//! Le démarrage avec Windows (spec §9.2).
//!
//! Responsabilité unique : lire, poser et retirer **une** valeur de registre.
//!
//! La clé est `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, celle de
//! l'utilisateur courant. **Pas `HKLM`** : celle-là est globale à la machine,
//! exige des droits administrateur, et un desktop pet n'a aucune raison d'en
//! demander.
//!
//! Spec §9.2 : « au démarrage automatique, aucune fenêtre, aucune
//! notification, aucun vol de focus ». C'est déjà vrai **par construction** —
//! la fenêtre est non focalisable, sans bordure et hors taskbar depuis
//! l'étape 1a, et porte `WS_EX_NOACTIVATE` — donc il n'y a rien de
//! particulier à faire ici.
//!
//! Écart assumé par rapport à l'annexe A du plan de l'étape 0, qui plaçait
//! ceci dans `main.rs` : c'est du win32 avec des chaînes larges et des
//! `unsafe`, et le mélanger à l'amorçage rendrait les deux moins lisibles.
//! Ce n'est pas non plus une *sonde* — `probe/` ne fait que lire, jamais
//! écrire.
//!
//! Signatures vérifiées dans windows 0.61.3, `Win32/System/Registry/mod.rs` :
//!   RegOpenKeyExW    :376
//!   RegSetValueExW   :626
//!   RegQueryValueExW :489
//!   RegDeleteValueW  :211

use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SAM_FLAGS, REG_SZ,
};

/// Le nom de la valeur dans la clé `Run`. C'est ce qui apparaît dans le
/// gestionnaire des tâches, onglet « Démarrage ».
const NOM_VALEUR: &str = "shimeji-desktop";

const CHEMIN_RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Convertit une chaîne Rust en chaîne large terminée par un zéro.
///
/// Les API `…W` de Windows attendent de l'UTF-16 avec un terminateur nul.
/// Rust stocke ses `str` en UTF-8 sans terminateur, donc **les deux
/// conversions sont nécessaires**.
///
/// Le `Vec` rendu doit rester vivant aussi longtemps que le pointeur qu'on en
/// tire — d'où le fait qu'on le nomme systématiquement dans les appelants au
/// lieu de l'utiliser en ligne. `PCWSTR(large("x").as_ptr())` compilerait et
/// serait un pointeur pendant.
fn large(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Ouvre la clé `Run`. Le `HKEY` rendu doit être fermé par l'appelant.
///
/// Fonction privée : c'est un détail d'implémentation, et exposer un handle
/// brut inviterait à oublier de le fermer.
fn ouvrir_run(droits: REG_SAM_FLAGS) -> Result<HKEY, String> {
    let chemin = large(CHEMIN_RUN);
    let mut cle = HKEY::default();

    // SÉCURITÉ : `chemin` vit jusqu'à la fin de la fonction, et
    // `RegOpenKeyExW` ne conserve pas le pointeur au-delà de l'appel.
    let code = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(chemin.as_ptr()),
            None,
            droits,
            &mut cle,
        )
    };

    if code != ERROR_SUCCESS {
        return Err(format!("ouverture de HKCU\\{CHEMIN_RUN} : code {}", code.0));
    }
    Ok(cle)
}

/// Le démarrage automatique est-il actif ?
///
/// **Interroge le registre, pas la config.** Les deux peuvent diverger dès
/// que l'utilisateur retire l'entrée à la main ou par le gestionnaire des
/// tâches, et c'est le registre qui dit la vérité. C'est lui qui initialise
/// la case du tray.
pub fn est_actif() -> bool {
    // `let Ok(...) else` : la clé `Run` existe toujours en pratique, mais si
    // on ne peut pas l'ouvrir, « pas actif » est la réponse sûre — elle ne
    // fait rien croire à l'utilisateur.
    let Ok(cle) = ouvrir_run(KEY_READ) else {
        return false;
    };

    let nom = large(NOM_VALEUR);

    // On ne veut pas la valeur, seulement savoir si elle existe : tous les
    // paramètres de sortie sont donc `None`.
    let code = unsafe { RegQueryValueExW(cle, PCWSTR(nom.as_ptr()), None, None, None, None) };

    unsafe {
        let _ = RegCloseKey(cle);
    }

    code == ERROR_SUCCESS
}

/// Inscrit l'exécutable courant dans la clé `Run`.
pub fn activer() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("chemin de l'exécutable introuvable : {e}"))?;

    // Les guillemets sont **obligatoires** : sans eux, un chemin contenant
    // une espace (« C:\Program Files\… ») serait coupé par Windows au
    // premier blanc, et il tenterait de lancer « C:\Program ».
    let commande = format!("\"{}\"", exe.display());

    let cle = ouvrir_run(KEY_WRITE)?;
    let nom = large(NOM_VALEUR);
    let valeur = large(&commande);

    // `RegSetValueExW` attend des OCTETS, pas des `u16` : on réinterprète le
    // tableau, d'où la longueur doublée.
    //
    // SÉCURITÉ : passer de `*const u16` à `*const u8` **relâche**
    // l'alignement au lieu de le resserrer, ce qui est toujours valide. Et
    // `valeur` vit jusqu'à la fin de la fonction.
    let octets: &[u8] =
        unsafe { std::slice::from_raw_parts(valeur.as_ptr() as *const u8, valeur.len() * 2) };

    let code = unsafe { RegSetValueExW(cle, PCWSTR(nom.as_ptr()), None, REG_SZ, Some(octets)) };

    unsafe {
        let _ = RegCloseKey(cle);
    }

    if code != ERROR_SUCCESS {
        return Err(format!("écriture de la valeur : code {}", code.0));
    }
    Ok(())
}

/// Retire l'entrée de la clé `Run`.
///
/// Une valeur déjà absente n'est **pas** une erreur : le résultat voulu est
/// « elle n'y est plus », et il est atteint.
pub fn desactiver() -> Result<(), String> {
    let cle = ouvrir_run(KEY_WRITE)?;
    let nom = large(NOM_VALEUR);

    let code = unsafe { RegDeleteValueW(cle, PCWSTR(nom.as_ptr())) };

    unsafe {
        let _ = RegCloseKey(cle);
    }

    if code != ERROR_SUCCESS && code != ERROR_FILE_NOT_FOUND {
        return Err(format!("suppression de la valeur : code {}", code.0));
    }
    Ok(())
}

// Pas de tests unitaires, et c'est délibéré : ces trois fonctions ne
// contiennent aucune logique, seulement des appels au registre. Un test y
// affirmerait que Windows fonctionne — et surtout, il **modifierait le
// registre de la machine qui exécute la suite**, ce qui est inacceptable pour
// un `cargo test`.
//
// La vérification est manuelle, une fois, et consignée dans le plan.
