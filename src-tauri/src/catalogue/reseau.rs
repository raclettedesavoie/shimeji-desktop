//! La SEULE pièce du catalogue qui touche au réseau (spec §4, §11).
//!
//! Responsabilité unique : rendre les octets d'une URL. Aucune connaissance
//! des PNG, des manifestes ni des slugs.
//!
//! # Pourquoi un trait
//!
//! Pour que l'installation entière se teste **sans sortir de la machine** —
//! y compris le seul cas qui compte vraiment, la **coupure à mi-parcours**.
//! C'est le même motif que `probe::SystemProbe`, qui sépare déjà « ce qu'on
//! a besoin de savoir » de « comment on l'apprend ».
//!
//! # Pourquoi WinHttp et pas une crate HTTP
//!
//! **Aucune dépendance nouvelle** — la crate `windows` est déjà épinglée en
//! 0.61 — et **aucune pile TLS embarquée** dans un exe de 2,7 Mo qui doit le
//! rester (spec §4). C'est aussi la logique du reste du projet : l'API
//! Windows typée plutôt qu'un portage.

/// Ce que rend une requête.
///
/// **Trois issues et non deux**, et la distinction porte tout le reste :
///   · `Ok(Some(octets))` — la ressource existe ;
///   · `Ok(None)` — 404. Une absence **normale**, pas une erreur : c'est
///     ainsi qu'on découvre qu'un pack n'a que 30 frames ;
///   · `Err(message)` — une panne, qui doit interrompre l'installation.
///
/// Confondre les deux dernières ferait qu'une coupure réseau serait prise
/// pour « le pack s'arrête ici », et produirait un personnage **amputé
/// présenté comme complet** — un défaut silencieux, donc le pire.
pub type Resultat = Result<Option<Vec<u8>>, String>;

pub trait Reseau {
    fn get(&self, url: &str) -> Resultat;
}

// ── Le réseau des tests ─────────────────────────────────────────────────

use std::cell::Cell;

/// Une table d'URL connues, et une panne déclenchable à volonté.
pub struct ReseauFake {
    reponses: Vec<(String, Option<Vec<u8>>)>,

    /// `Cell` pour la même raison que dans `FakeProbe` : le trait expose
    /// `&self`, donc un test qui n'a qu'une référence partagée doit pouvoir
    /// faire avancer le compteur. `usize` est `Copy`, ce que `Cell` exige.
    servies: Cell<usize>,
    echec_apres: Cell<Option<usize>>,
}

impl ReseauFake {
    pub fn new(reponses: Vec<(String, Option<Vec<u8>>)>) -> Self {
        ReseauFake {
            reponses,
            servies: Cell::new(0),
            echec_apres: Cell::new(None),
        }
    }

    /// Après `n` requêtes servies, toutes les suivantes échouent.
    ///
    /// Existe pour LE cas de test qui compte : la coupure à mi-installation.
    /// Sans lui, on ne testerait que le chemin heureux — qui n'est pas celui
    /// qui casse.
    pub fn echouer_apres(&self, n: usize) {
        self.echec_apres.set(Some(n));
    }
}

impl Reseau for ReseauFake {
    fn get(&self, url: &str) -> Resultat {
        if let Some(seuil) = self.echec_apres.get() {
            if self.servies.get() >= seuil {
                return Err(format!("panne simulée sur {url}"));
            }
        }
        self.servies.set(self.servies.get() + 1);

        // Une URL non déclarée est traitée comme un 404 : c'est ce qui rend
        // les tests courts, puisqu'on ne déclare que ce qui existe.
        match self.reponses.iter().find(|(u, _)| u == url) {
            Some((_, corps)) => Ok(corps.clone()),
            None => Ok(None),
        }
    }
}

// ── Le vrai réseau : WinHttp ────────────────────────────────────────────

use std::ffi::c_void;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Networking::WinHttp::*;

pub struct ReseauWinHttp;

impl ReseauWinHttp {
    pub fn new() -> Self {
        ReseauWinHttp
    }
}

/// Un handle WinHttp qui se ferme tout seul.
///
/// `Drop` plutôt qu'un `WinHttpCloseHandle` écrit à la main : `get` a une
/// demi-douzaine de chemins de sortie, et en oublier un fuirait un handle à
/// chaque image manquante. C'est exactement ce à quoi sert `Drop` en Rust —
/// le destructeur est appelé quoi qu'il arrive, `return` anticipé compris.
struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // `unsafe` : on rend un handle au système. L'échec éventuel n'a
            // aucun recours utile, d'où le `let _`.
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

impl Reseau for ReseauWinHttp {
    fn get(&self, url: &str) -> Resultat {
        // ── Découper l'URL ──────────────────────────────────────────────
        // On n'accepte QUE https, et on découpe à la main plutôt que
        // d'ajouter une crate d'URL pour deux `split`.
        let Some(reste) = url.strip_prefix("https://") else {
            return Err(format!("URL non https : {url}"));
        };
        let (hote, chemin) = match reste.find('/') {
            Some(i) => (&reste[..i], &reste[i..]),
            None => (reste, "/"),
        };

        // `HSTRING` convertit en UTF-16 terminé par un nul, ce qu'attendent
        // toutes les API « W » de Windows. La variable doit VIVRE aussi
        // longtemps que le `PCWSTR` qui la pointe — d'où des `let` séparés
        // plutôt que des temporaires au milieu de l'appel, qui seraient
        // libérés avant que Windows ne les lise.
        let hote_w = HSTRING::from(hote);
        let chemin_w = HSTRING::from(chemin);
        let agent_w = HSTRING::from("shimeji-desktop/0.1");

        unsafe {
            let session = Handle(WinHttpOpen(
                PCWSTR(agent_w.as_ptr()),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ));
            if session.0.is_null() {
                return Err("WinHttpOpen a échoué".to_string());
            }

            let connexion = Handle(WinHttpConnect(
                session.0,
                PCWSTR(hote_w.as_ptr()),
                INTERNET_DEFAULT_HTTPS_PORT,
                0,
            ));
            if connexion.0.is_null() {
                return Err(format!("connexion à {hote} impossible"));
            }

            let requete = Handle(WinHttpOpenRequest(
                connexion.0,
                PCWSTR::null(), // verbe nul = GET
                PCWSTR(chemin_w.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null_mut(),
                WINHTTP_FLAG_SECURE,
            ));
            if requete.0.is_null() {
                return Err(format!("requête sur {chemin} impossible"));
            }

            // Six arguments et non sept : dans `windows` 0.61, les en-têtes
            // sont un `Option<&[u16]>` dont la longueur est DÉDUITE, au lieu
            // du couple pointeur + longueur du C. `None` = aucun en-tête
            // ajouté, `None` = aucun corps de requête.
            WinHttpSendRequest(requete.0, None, None, 0, 0, 0)
                .map_err(|e| format!("envoi : {e}"))?;
            WinHttpReceiveResponse(requete.0, std::ptr::null_mut())
                .map_err(|e| format!("réponse : {e}"))?;

            // ── Le code de statut ───────────────────────────────────────
            // 404 n'est PAS une erreur : c'est ainsi qu'on découvre la fin
            // d'un pack. Tout autre code hors 200 en est une.
            let mut statut: u32 = 0;
            let mut taille = std::mem::size_of::<u32>() as u32;
            WinHttpQueryHeaders(
                requete.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some(&mut statut as *mut u32 as *mut c_void),
                &mut taille,
                std::ptr::null_mut(),
            )
            .map_err(|e| format!("statut : {e}"))?;

            if statut == 404 {
                return Ok(None);
            }
            if statut != 200 {
                return Err(format!("statut {statut} sur {url}"));
            }

            // ── Lire le corps ───────────────────────────────────────────
            // Boucle en deux temps, imposée par l'API : demander combien
            // d'octets sont disponibles, puis les lire.
            let mut corps: Vec<u8> = Vec::new();
            loop {
                let mut dispo: u32 = 0;
                WinHttpQueryDataAvailable(requete.0, &mut dispo)
                    .map_err(|e| format!("lecture : {e}"))?;
                if dispo == 0 {
                    break;
                }

                let debut = corps.len();
                corps.resize(debut + dispo as usize, 0);
                let mut lus: u32 = 0;
                WinHttpReadData(
                    requete.0,
                    corps[debut..].as_mut_ptr() as *mut c_void,
                    dispo,
                    &mut lus,
                )
                .map_err(|e| format!("lecture : {e}"))?;

                // On tronque à ce qui a RÉELLEMENT été lu : `dispo` est un
                // majorant annoncé, pas une promesse tenue.
                corps.truncate(debut + lus as usize);
            }

            Ok(Some(corps))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le faux réseau rend ce qu'on lui a dit, et sait tomber en panne.
    #[test]
    fn le_faux_reseau_rend_et_echoue_a_la_demande() {
        let r = ReseauFake::new(vec![
            ("https://x/1".to_string(), Some(vec![1, 2, 3])),
            ("https://x/2".to_string(), None), // 404 : absence NORMALE
        ]);

        assert_eq!(r.get("https://x/1"), Ok(Some(vec![1, 2, 3])));
        assert_eq!(r.get("https://x/2"), Ok(None), "404 n'est pas une panne");
        assert_eq!(r.get("https://x/inconnue"), Ok(None), "non déclarée = absente");

        // Après une requête servie, toutes les suivantes tombent.
        let r2 = ReseauFake::new(vec![("https://x/1".to_string(), Some(vec![9]))]);
        r2.echouer_apres(1);
        assert_eq!(r2.get("https://x/1"), Ok(Some(vec![9])));
        assert!(r2.get("https://x/1").is_err(), "la 2e requête doit tomber");
    }
}
