//! Afficher le menu du clic droit **sur un thread à lui**, en Win32 pur.
//!
//! Responsabilité unique : montrer une liste de `menu_perso::Ligne` au
//! curseur et rendre l'identifiant choisi. Ce que le menu contient est décidé
//! par `menu_perso::lignes`, ce que fait le choix par `actions::executer`.
//!
//! # Pourquoi pas le menu de Tauri (2026-09-23)
//!
//! Le menu Tauri (`muda`) s'exécute **sur le thread principal**, dans un
//! callback de la boucle d'événements de `tao`. Or `tao` met en file tout
//! événement reçu pendant qu'un callback tourne (`should_buffer`,
//! `tao-0.35.3/src/platform_impl/windows/event_loop/runner.rs:143`) — y
//! compris les `eval` qui portent les positions des personnages vers les
//! webviews. Tant que le menu était ouvert, **tous** les personnages se
//! figeaient donc, et pas seulement celui qu'on avait cliqué ; les ouvrir
//! depuis un autre thread n'y changeait rien, l'affichage restant bloqué.
//!
//! Un menu natif, sur un thread qui possède sa propre fenêtre (invisible),
//! a sa boucle modale à lui : le thread principal continue de livrer les
//! `eval`, et les autres personnages vivent pendant qu'on choisit.

use crate::menu_perso::Ligne;

/// Le nom de la classe de fenêtre propriétaire du menu. Enregistrée une fois
/// par processus ; les appels suivants de `RegisterClassW` échouent en
/// silence, ce qui est sans conséquence (la classe existe déjà).
const CLASSE: windows::core::PCWSTR = windows::core::w!("shimeji-desktop-menu");

/// La procédure de fenêtre : ne rien faire de plus que Windows par défaut.
///
/// Une fonction à nous et non `DefWindowProcW` directement : le binding de
/// la crate `windows` est une fonction Rust ordinaire, alors que
/// `lpfnWndProc` exige une fonction à la convention d'appel `"system"`.
unsafe extern "system" fn procedure(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    // Le corps d'une `unsafe fn` est déjà un contexte `unsafe` en édition
    // 2021 : l'appel FFI n'a pas besoin d'un bloc de plus.
    windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Affiche le menu au curseur et **bloque** jusqu'à sa fermeture.
///
/// Rend l'identifiant de l'entrée choisie, ou `None` si l'utilisateur a
/// cliqué ailleurs (ou si Windows a refusé de créer le menu — un menu qui ne
/// s'ouvre pas n'empêche pas le personnage de vivre).
///
/// ⚠️ **À appeler depuis un thread dédié**, jamais depuis la boucle 60 Hz ni
/// depuis le thread principal : c'est tout l'objet du module. La fenêtre
/// propriétaire est créée ET détruite ici, sur ce thread — Windows exige que
/// `TrackPopupMenu` soit appelé par le thread qui possède la fenêtre.
pub fn choisir(lignes: &[Ligne]) -> Option<&'static str> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DestroyMenu, DestroyWindow, GetCursorPos,
        RegisterClassW, TrackPopupMenu, MF_SEPARATOR, MF_STRING, TPM_LEFTALIGN, TPM_RETURNCMD,
        TPM_RIGHTBUTTON, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
    };

    // `unsafe` : tout ce qui suit franchit la frontière FFI. Les handles
    // passés à Windows sortent tous d'un appel de ce même bloc, et sont
    // détruits avant d'en sortir.
    unsafe {
        // ── La fenêtre propriétaire, invisible ──────────────────────────
        //
        // `WS_EX_TOOLWINDOW` : ni barre des tâches, ni Alt+Tab, même le
        // temps d'un menu. Jamais `ShowWindow` : elle n'a pas à être vue,
        // seulement à pouvoir prendre le premier plan (voir plus bas).
        let instance = GetModuleHandleW(None).ok()?;
        let classe = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance.into(),
            lpszClassName: CLASSE,
            // `..Default::default()` : tous les autres champs à zéro — pas
            // d'icône, pas de curseur, pas de fond, rien à dessiner.
            ..Default::default()
        };
        let _ = RegisterClassW(&classe);

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASSE,
            windows::core::w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .ok()?;

        // ── Le menu ─────────────────────────────────────────────────────
        //
        // Windows n'identifie une entrée que par un entier. On prend son
        // rang dans `lignes` **plus un** : `TrackPopupMenu` rend 0 quand
        // rien n'est choisi, donc 0 ne peut pas désigner une entrée.
        let Ok(menu) = CreatePopupMenu() else {
            let _ = DestroyWindow(hwnd);
            return None;
        };
        for (rang, ligne) in lignes.iter().enumerate() {
            match ligne {
                Ligne::Entree { libelle, .. } => {
                    // `HSTRING` : la chaîne UTF-16 terminée par un zéro que
                    // Windows attend. Les accents passent tels quels.
                    let texte = windows::core::HSTRING::from(*libelle);
                    let _ = AppendMenuW(menu, MF_STRING, rang + 1, &texte);
                }
                Ligne::Separateur => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
                }
            }
        }

        // ── L'affichage ─────────────────────────────────────────────────
        //
        // À qui rendre le focus après le menu — retenu AVANT de l'avoir
        // pris. Sans cette restitution, un clic droit sur le personnage
        // laisserait l'éditeur muet (voir `fenetre_au_premier_plan`).
        let precedente = crate::render::fenetre_au_premier_plan();

        // Puis on prend RÉELLEMENT le premier plan, et on vérifie que Windows
        // a accepté : un refus donne précisément le menu qui ne se referme
        // pas quand on clique ailleurs. Tout le raisonnement est dans
        // `prendre_le_premier_plan`.
        let devant = crate::render::prendre_le_premier_plan(hwnd);
        if !devant && std::env::var_os("SHIMEJI_MENU").is_some() {
            eprintln!(
                "menu : Windows a refusé le premier plan — le menu risque de ne pas se refermer au clic"
            );
        }

        let mut curseur = POINT::default();
        let _ = GetCursorPos(&mut curseur);

        // `TPM_RETURNCMD` : le choix est RENDU par l'appel, au lieu d'être
        // posté en `WM_COMMAND` à une fenêtre qui n'a personne pour
        // l'écouter. `TPM_RIGHTBUTTON` : l'entrée se choisit aussi au bouton
        // droit, comme dans l'explorateur.
        let choix = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            curseur.x,
            curseur.y,
            None,
            hwnd,
            None,
        );

        // La seconde moitié de la recette de KB135788 : sans ce message vide,
        // c'est le menu SUIVANT qui se comporte mal. Voir `reveiller_la_file`.
        crate::render::reveiller_la_file(hwnd);

        let _ = DestroyMenu(menu);
        let _ = DestroyWindow(hwnd);

        // Puis on rend le focus, la fenêtre du menu n'existant plus pour le
        // reprendre. `if let Some` : il n'y avait pas forcément de premier
        // plan à l'ouverture (bureau sécurisé).
        if let Some(h) = precedente {
            crate::render::rendre_le_premier_plan(h);
        }

        // `choix.0` : le `BOOL` rendu porte en fait l'identifiant choisi
        // quand `TPM_RETURNCMD` est posé — 0 si rien ne l'a été.
        let rang = usize::try_from(choix.0).ok()?.checked_sub(1)?;
        match lignes.get(rang) {
            Some(Ligne::Entree { id, .. }) => Some(*id),
            _ => None,
        }
    }
}
