//! Windows est-il en mode « Ne pas déranger » ?
//!
//! Responsabilité unique : répondre à cette question. Ce qu'on en fait —
//! remplacer le toast par une notification maison — est dans `toast.rs`.
//!
//! # Pourquoi une API non documentée
//!
//! Mesuré le 2026-09-24 sur la machine de l'auteur, mode désactivé puis
//! activé :
//!
//! | Source | désactivé | activé |
//! |---|---|---|
//! | `SHQueryUserNotificationState` (documentée) | 2 | **2** |
//! | état WNF `WNF_SHEL_QUIETHOURS_ACTIVE_PROFILE_CHANGED` | `00` | **`01`** |
//!
//! La seule API documentée ne voit pas le mode. L'état WNF (Windows
//! Notification Facility) est ce que lisent les outils tiers qui le gèrent.
//! Non documenté veut dire qu'une version de Windows peut le changer : la
//! lecture est donc **best-effort** — en cas d'échec on répond « non », et
//! l'on retombe sur le toast ordinaire, exactement le comportement d'avant.

/// Le nom de l'état WNF des « heures calmes » (le profil actif).
const WNF_SHEL_QUIETHOURS_ACTIVE_PROFILE_CHANGED: u64 = 0x0d83_063e_a3bf_1c75;

// La fonction de `ntdll.dll` qui lit un état WNF. Absente de la crate
// `windows` (non documentée), d'où la déclaration à la main.
//
// `extern "system"` : la convention d'appel de l'API Windows. `#[link]` :
// demande à l'éditeur de liens de la trouver dans `ntdll.lib`, livrée avec
// le SDK Windows — rien à installer.
#[cfg(windows)]
#[link(name = "ntdll")]
extern "system" {
    fn NtQueryWnfStateData(
        state_name: *const u64,
        type_id: *const core::ffi::c_void,
        explicit_scope: *const core::ffi::c_void,
        change_stamp: *mut u32,
        buffer: *mut u8,
        buffer_size: *mut u32,
    ) -> i32;
}

/// Vrai si le mode « Ne pas déranger » est actif. `SHIMEJI_NE_PAS_DERANGER=1`
/// le force — l'équivalent scriptable d'aller le cocher dans Windows.
pub fn actif() -> bool {
    if std::env::var_os("SHIMEJI_NE_PAS_DERANGER").is_some() {
        return true;
    }
    lire_etat().map(|o| interpreter(&o)).unwrap_or(false)
}

/// Les octets bruts de l'état, ou `None` si la lecture échoue.
#[cfg(windows)]
fn lire_etat() -> Option<Vec<u8>> {
    let nom = WNF_SHEL_QUIETHOURS_ACTIVE_PROFILE_CHANGED;
    let mut octets = [0u8; 16];
    let mut taille: u32 = octets.len() as u32;
    let mut tampon_de_version: u32 = 0;
    // `unsafe` : un appel vers du code C, dont Rust ne peut pas vérifier
    // les pointeurs. Ils sont tous valides ici : des variables locales qui
    // vivent jusqu'à la fin de l'appel, et `taille` dit bien la place du
    // tampon.
    let statut = unsafe {
        NtQueryWnfStateData(
            &nom,
            core::ptr::null(),
            core::ptr::null(),
            &mut tampon_de_version,
            octets.as_mut_ptr(),
            &mut taille,
        )
    };
    // Un `NTSTATUS` négatif est une erreur.
    if statut < 0 {
        return None;
    }
    let taille = (taille as usize).min(octets.len());
    Some(octets[..taille].to_vec())
}

#[cfg(not(windows))]
fn lire_etat() -> Option<Vec<u8>> {
    None
}

/// Les quatre premiers octets sont le profil actif, en petit-boutiste : 0
/// quand le mode est désactivé, autre chose sinon (mesure ci-dessus).
fn interpreter(octets: &[u8]) -> bool {
    // `get(0..4)` rend `None` s'il y a moins de quatre octets, plutôt que de
    // paniquer comme `octets[0..4]` le ferait.
    match octets.get(0..4) {
        Some(q) => u32::from_le_bytes([q[0], q[1], q[2], q[3]]) != 0,
        None => false,
    }
}

#[cfg(test)]
#[path = "ne_pas_deranger_tests.rs"]
mod tests;
