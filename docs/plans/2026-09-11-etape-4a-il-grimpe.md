# Étape 4a — il grimpe les bords de l'écran — plan d'implémentation

> **Pour un exécutant agentique :** SOUS-COMPÉTENCE REQUISE — utiliser
> `superpowers:subagent-driven-development` (recommandé) ou
> `superpowers:executing-plans` pour exécuter ce plan tâche par tâche. Les étapes
> utilisent la syntaxe `- [ ]` pour le suivi.

**But :** le personnage grimpe les murs et le plafond de chaque écran, de lui-même ou
parce qu'on l'a jeté contre un bord.

**Architecture :** `World::from_screens` expose désormais quatre plateformes par écran
(sol, deux murs, plafond) au lieu d'une, chacune étant un rectangle fin dont la face
pointe vers l'intérieur de l'écran — ce qui laisse `geom.rs` et `Attachment` totalement
inchangés. La détection d'atterrissage devient une détection de **contact** qui rend une
`Face`, et une nouvelle intention `Grimper` possède tout le monde vertical, `Flaner`
continuant de ne connaître que le sol.

**Pile :** Rust, `cargo test`, aucune dépendance nouvelle.

**Spec :** [`docs/specs/2026-09-11-etape-4a-il-grimpe-design.md`](../specs/2026-09-11-etape-4a-il-grimpe-design.md)
— le plan argumente depuis elle ; lire les deux.

---

## Contraintes globales

Elles s'appliquent implicitement à **toutes** les tâches.

- **Compiler depuis PowerShell, jamais depuis Git Bash** — piège n° 3 de `CLAUDE.md` :
  dans un shell Git Bash, rustc peut pêcher le `link.exe` de Git for Windows au lieu du
  linker MSVC, et rendre une erreur `extra operand` opaque.
- **Tout le code est abondamment commenté, en français.** C'est une exigence explicite de
  l'auteur, qui apprend Rust sur ce projet — pas une préférence de style. Commenter le
  *pourquoi*, citer la décision ou la section de spec appliquée, et expliquer toute
  construction Rust non élémentaire (`let … else`, combinateurs, emprunts).
- **Chaque module s'ouvre sur deux ou trois lignes** disant sa responsabilité unique.
- **Ne jamais inventer une constante d'animation ou de physique** : la chercher dans
  `C:\Users\alri\Downloads\shimejieesrc (2)\` (`conf/actions.xml`, `src/…/action/*.java`).
  Les valeurs déjà relevées sont dans le §5 du design.
- **Ne jamais ajouter de compensation de décalage dans `attach.rs`** — l'ancre du
  `mascot.json` existe pour rendre cela inutile ; son en-tête l'interdit explicitement.
- **`cargo test` ne reconstruit pas l'exe.** Après une correction, `cargo build` avant de
  relancer l'application, sinon on vérifie un binaire plus vieux que la source.
- **Semer l'aléatoire une seule fois** dans un test de distribution, et laisser l'état
  avancer — re-semer `XorShift32::seeded(n)` avec de petits entiers séquentiels biaise le
  premier tirage et produit des faux négatifs complets.
- Une tâche = un commit. Tous les tests passent à chaque commit.

**Valeurs exactes, reprises du design §5 :**

| Constante | Valeur | Source |
|---|---|---|
| vitesse d'escalade | `16.1` px/s | `ClimbWall` : 36 px sur 56 ticks de 40 ms |
| durée d'accroche | `[0.5, 1.5]` s | `HoldOntoWall` : `${500+Math.random()*1000}` |
| délai d'abandon de `Grimper` | `120` s | design §4.4 |
| tolérance de jonction | `8.0` px | valeur déjà utilisée par `face_voisine` |
| épaisseur d'une plateforme | `1.0` px | `EPAISSEUR_DU_SOL` existante |

---

## Structure des fichiers

| Fichier | Responsabilité, et ce que ce plan y change |
|---|---|
| `src-tauri/src/world.rs` | **modifié** — quatre plateformes par écran, `PlatformId::ecran`, `RoleEcran`, `premier_sol` |
| `src-tauri/src/geom.rs` | **inchangé** — c'est le but du §2.1 du design ; si ce fichier change, l'approche a dérivé |
| `src-tauri/src/character/physics.rs` | **modifié** — `contact()`, `VITESSE_ESCALADE`, `DUREE_ACCROCHE` |
| `src-tauri/src/character/attach.rs` | **inchangé** — `Attachment` décrit déjà un mur (design §3.1) |
| `src-tauri/src/behavior/reflex.rs` | **modifié** — le Réflexe 3 appelle `contact`, `Reflexe::Accroche` |
| `src-tauri/src/behavior/mod.rs` | **modifié** — la règle « fini sur une face verticale ⇒ il lâche » |
| `src-tauri/src/behavior/intention.rs` | **modifié** — `Intention::Grimper`, `PhaseGrimpe`, `grimper()`, délai par intention |
| `src-tauri/src/behavior/desire.rs` | **modifié** — une ligne de table |
| `src-tauri/src/menu_perso.rs` | **modifié** — une ligne dans `ENVIES` |
| `src-tauri/src/config.rs` | **modifié** — `Envies::grimper`, `Escalade`, `Reglages::vitesse_escalade` |
| `src-tauri/src/main.rs` | **modifié** — `premier_sol()` au placement initial |
| `src-tauri/src/sim.rs` | **modifié** — l'escalade dans la trace et l'invariant |
| `characters/blob/mascot.json` | **modifié en Tâche 7 seulement** — l'ancre mesurée |
| `CLAUDE.md` | **modifié en Tâche 7** — correction de la table des frames, état d'avancement |

---

## Ordre des tâches, et pourquoi celui-là

| | Tâche | Pourquoi ici |
|---|---|---|
| 1 | Les murs et le plafond dans le monde | tout en dépend, et rien ne change à l'écran |
| 2 | `contact()`, fonction pure | testable seule, non branchée : comportement inchangé |
| 3 | La règle « fini sur une face verticale ⇒ il lâche » | **avant** qu'on puisse arriver sur un mur, donc aucune fenêtre où il resterait coincé |
| 4 | L'intention `Grimper` (+ envie, menu, config) | la première fois qu'on le voit grimper |
| 5 | Le lancer qui s'accroche | branche la Tâche 2 sur la Tâche 4 |
| 6 | Le plafond | le dernier morceau du monde vertical |
| 7 | L'ancre mesurée, le CPU, la doc | ce qui ne se décide pas sur document |

---

### Tâche 1 : Les murs et le plafond dans le monde

**Fichiers :**
- Modifier : `src-tauri/src/world.rs`
- Modifier : `src-tauri/src/main.rs` (le placement initial)
- Test : `src-tauri/src/world.rs` (module `tests` en fin de fichier)

**Interfaces :**
- Consomme : `ScreenInfo { id: u64, work_area: Rect, scale: f32 }`, `Rect`, `Face` (existants)
- Produit :
  - `pub enum RoleEcran { Sol, MurGauche, MurDroit, Plafond }`
  - `impl PlatformId { pub fn ecran(monitor: u64, role: RoleEcran) -> PlatformId }`
  - `impl PlatformId { pub fn meme_ecran(&self, autre: PlatformId) -> bool }`
  - `impl World { pub fn premier_sol(&self) -> Option<&Platform> }`
  - `World::from_screens` rend jusqu'à 4 plateformes par écran, **le sol toujours en
    premier pour chaque écran**

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans le module `tests` de `src-tauri/src/world.rs`, **remplacer** les deux tests
`un_ecran_donne_une_plateforme_de_sol` et `deux_ecrans_donnent_deux_plateformes_distinctes`
(leurs assertions de longueur sont devenues fausses par construction) et **ajouter** les
suivants :

```rust
    /// Compte les plateformes qui exposent une face donnée. Rendue locale au
    /// module de test : c'est une question de test, pas une propriété du monde.
    fn combien_de_face(monde: &World, face: Face) -> usize {
        monde.platforms().iter().filter(|p| p.has_face(face)).count()
    }

    #[test]
    fn un_ecran_isole_donne_sol_plafond_et_deux_murs() {
        let monde = World::from_screens(&FakeProbe::un_ecran().screens());
        assert_eq!(monde.platforms().len(), 4);
        assert_eq!(combien_de_face(&monde, Face::Top), 1);
        assert_eq!(combien_de_face(&monde, Face::Bottom), 1);
        assert_eq!(combien_de_face(&monde, Face::Right), 1); // le mur GAUCHE
        assert_eq!(combien_de_face(&monde, Face::Left), 1);  // le mur DROIT
    }

    #[test]
    fn le_sol_est_toujours_la_premiere_plateforme_de_son_ecran() {
        // Plusieurs tests d'autres modules écrivent `platforms()[0]` en
        // voulant dire « le sol ». Cet ordre est donc un contrat, pas un
        // hasard — d'où ce test qui le fige.
        let monde = World::from_screens(&FakeProbe::un_ecran().screens());
        assert!(monde.platforms()[0].has_face(Face::Top));
        assert_eq!(monde.premier_sol().map(|p| p.id), Some(monde.platforms()[0].id));
    }

    #[test]
    fn le_mur_gauche_expose_sa_face_droite_sur_le_bord_de_la_zone_de_travail() {
        // La vérification la plus importante de la tâche : le POINT rendu
        // par point_on doit tomber exactement sur le bord, pas un pixel à
        // côté — c'est là que le personnage sera dessiné.
        let monde = World::from_screens(&FakeProbe::un_ecran().screens());
        let mur = monde
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Right))
            .expect("un écran isolé a un mur gauche");

        // FakeProbe::un_ecran : zone de travail (0, 0, 1920, 1032).
        assert_eq!(mur.rect.point_on(Face::Right, 0.0), Point::new(0.0, 0.0));
        assert_eq!(mur.rect.point_on(Face::Right, 100.0), Point::new(0.0, 100.0));
        assert_eq!(mur.rect.face_length(Face::Right), 1032.0);
    }

    #[test]
    fn le_mur_droit_et_le_plafond_tombent_aussi_sur_le_bord() {
        let monde = World::from_screens(&FakeProbe::un_ecran().screens());

        let droit = monde
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Left))
            .expect("un écran isolé a un mur droit");
        assert_eq!(droit.rect.point_on(Face::Left, 50.0), Point::new(1920.0, 50.0));

        let plafond = monde
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Bottom))
            .expect("tout écran a un plafond");
        assert_eq!(plafond.rect.point_on(Face::Bottom, 30.0), Point::new(30.0, 0.0));
    }

    #[test]
    fn deux_ecrans_cote_a_cote_ne_posent_que_les_deux_murs_exterieurs() {
        // Le point décidé avec l'auteur : vu de l'utilisateur, le bureau est
        // UNE boîte. Six plateformes : 2 sols + 2 plafonds + 2 murs.
        let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
        assert_eq!(monde.platforms().len(), 6);
        assert_eq!(combien_de_face(&monde, Face::Top), 2);
        assert_eq!(combien_de_face(&monde, Face::Bottom), 2);

        // Un seul mur gauche (celui de l'écran de gauche, à x = 0) et un seul
        // mur droit (celui de l'écran de droite, à x = 3840).
        let gauches: Vec<&Platform> = monde
            .platforms()
            .iter()
            .filter(|p| p.has_face(Face::Right))
            .collect();
        assert_eq!(gauches.len(), 1);
        assert_eq!(gauches[0].rect.point_on(Face::Right, 0.0).x, 0.0);

        let droits: Vec<&Platform> = monde
            .platforms()
            .iter()
            .filter(|p| p.has_face(Face::Left))
            .collect();
        assert_eq!(droits.len(), 1);
        assert_eq!(droits[0].rect.point_on(Face::Left, 0.0).x, 3840.0);
    }

    #[test]
    fn deux_ecrans_donnent_des_identites_toutes_distinctes() {
        // Remplace `deux_ecrans_donnent_deux_plateformes_distinctes`.
        let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
        let mut ids: Vec<u64> = monde.platforms().iter().map(|p| p.id.0).collect();
        ids.sort_unstable();
        let avant = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), avant, "deux plateformes partagent une identité");
    }

    #[test]
    fn meme_ecran_reconnait_les_quatre_plateformes_d_un_ecran() {
        let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
        let sol_a = monde.platforms()[0].id;

        let memes = monde
            .platforms()
            .iter()
            .filter(|p| p.id.meme_ecran(sol_a))
            .count();
        assert_eq!(memes, 3, "sol + plafond + un mur pour l'écran de gauche");
    }

    #[test]
    fn un_ecran_a_gauche_a_bien_son_mur_a_x_negatif() {
        // Contrainte globale du projet : ne jamais supposer x >= 0.
        let monde = World::from_screens(&FakeProbe::ecran_a_gauche_hidpi().screens());
        let mur = monde
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Right))
            .expect("le mur gauche de l'écran de gauche existe");
        assert!(mur.rect.point_on(Face::Right, 0.0).x < 0.0);
    }
```

Ajouter `Point` à l'import du module de test si nécessaire :
`use crate::geom::{Face, Point};`

- [ ] **Étape 2 : lancer les tests pour vérifier qu'ils échouent**

Depuis PowerShell :

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo test world::
```

Attendu : échec de compilation — `premier_sol` et `meme_ecran` n'existent pas.

- [ ] **Étape 3 : le rôle et l'identité**

Dans `src-tauri/src/world.rs`, après la définition de `PlatformId` :

```rust
/// Le rôle d'une plateforme issue d'un écran.
///
/// Sert **uniquement** à fabriquer quatre identités distinctes par écran :
/// ni la physique ni le comportement ne le consultent jamais, exactement
/// comme `PlatformKind`. Une plateforme se décrit par ses `faces`, pas par
/// son étiquette d'origine.
///
/// Les valeurs explicites (`= 0`, `= 1`…) ne sont pas décoratives : elles
/// entrent dans le calcul de `PlatformId::ecran`, et les changer changerait
/// toutes les identités.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleEcran {
    Sol = 0,
    MurGauche = 1,
    MurDroit = 2,
    Plafond = 3,
}

impl PlatformId {
    /// L'identité d'une des quatre plateformes d'un écran (design §2.2).
    ///
    /// `monitor << 2 | role` : les deux bits de poids faible portent le rôle,
    /// le reste porte la poignée du moniteur. `HMONITOR` et `HWND` sont des
    /// poignées en espace utilisateur, largement sous 2⁴⁷ sur Windows x64 —
    /// décaler de deux bits ne perd donc rien et ne peut pas collisionner.
    ///
    /// **L'identité reste indépendante de la géométrie** (spec §5.2), ce qui
    /// est la condition de la décision n° 1 : changer la résolution ne change
    /// pas la poignée du moniteur, donc pas l'identité.
    ///
    /// `role as u64` : un `enum` sans données et à valeurs explicites se
    /// convertit en entier par un simple `as`. C'est la seule conversion de
    /// ce genre du projet, et elle est sûre parce que les quatre valeurs
    /// tiennent sur deux bits.
    pub fn ecran(monitor: u64, role: RoleEcran) -> PlatformId {
        PlatformId(monitor << 2 | role as u64)
    }

    /// Ces deux plateformes viennent-elles du même écran ?
    ///
    /// On retire les deux bits de rôle et on compare le reste. Sert à
    /// l'intention `Grimper` (Tâche 4), qui cherche un mur **de l'écran où
    /// le personnage se trouve** — pas celui d'en face.
    pub fn meme_ecran(&self, autre: PlatformId) -> bool {
        self.0 >> 2 == autre.0 >> 2
    }
}
```

- [ ] **Étape 4 : construire les quatre plateformes**

Toujours dans `world.rs`, renommer la constante et réécrire `from_screens` :

```rust
/// Épaisseur donnée au rectangle d'une plateforme.
///
/// Aucune de ces plateformes n'a d'épaisseur réelle — un sol est une ligne,
/// un mur aussi — mais un `Rect` en demande une, et une épaisseur nulle
/// rendrait `contains` toujours faux. Un pixel suffit : seule la face
/// tournée vers l'intérieur de l'écran est exposée, donc cette épaisseur
/// n'est jamais parcourue.
const EPAISSEUR_PLATEFORME: f32 = 1.0;

/// Tolérance sur la jonction entre deux écrans, en pixels.
///
/// La **même valeur** que la tolérance de `face_voisine`, et pour la même
/// raison : deux écrans côte à côte se touchent exactement, mais des
/// résolutions ou des échelles différentes peuvent laisser quelques pixels
/// de jeu. On ne veut pas d'un mur fantôme pour 2 px.
const TOLERANCE_JONCTION: f32 = 8.0;
```

Puis :

```rust
    /// Construit le monde : **sol, deux murs et plafond par écran**
    /// (design §2).
    ///
    /// Un mur n'est posé que si aucun autre écran ne le touche : sans cette
    /// règle, deux écrans côte à côte donneraient un mur invisible en plein
    /// milieu du bureau, que le personnage escaladerait alors qu'il traverse
    /// déjà librement le sol au même endroit (design §2.3).
    ///
    /// Tout est pris sur la **zone de travail**, jamais sur l'écran complet :
    /// c'est le piège Windows n° 3 appliqué aux trois nouvelles faces — sur
    /// l'écran complet, il grimperait derrière la barre des tâches.
    ///
    /// ⚠️ **Le sol de chaque écran est poussé en premier**, et c'est un
    /// contrat : plusieurs tests d'autres modules écrivent `platforms()[0]`
    /// en voulant dire « le sol ». Le test
    /// `le_sol_est_toujours_la_premiere_plateforme_de_son_ecran` le fige.
    pub fn from_screens(screens: &[ScreenInfo]) -> World {
        let mut platforms = Vec::with_capacity(screens.len() * 4);

        for s in screens {
            let z = s.work_area;

            // ── Le sol ──────────────────────────────────────────────────
            // Son bord SUPÉRIEUR est à hauteur du bas de la zone de travail :
            // c'est la ligne sur laquelle on marche.
            platforms.push(Platform {
                id: PlatformId::ecran(s.id, RoleEcran::Sol),
                rect: Rect::new(z.left(), z.bottom(), z.w, EPAISSEUR_PLATEFORME),
                kind: PlatformKind::Screen,
                z: 0,
                faces: vec![Face::Top],
            });

            // ── Le plafond ──────────────────────────────────────────────
            // Posé JUSTE AU-DESSUS de la zone de travail, et c'est sa face
            // `Bottom` qui est exposée : on s'y suspend par en dessous. Le
            // `- EPAISSEUR` place le rectangle de sorte que `bottom()` tombe
            // exactement sur `z.top()`.
            //
            // Jamais supprimé, lui : il n'y a rien au-dessus du bureau.
            platforms.push(Platform {
                id: PlatformId::ecran(s.id, RoleEcran::Plafond),
                rect: Rect::new(
                    z.left(),
                    z.top() - EPAISSEUR_PLATEFORME,
                    z.w,
                    EPAISSEUR_PLATEFORME,
                ),
                kind: PlatformKind::Screen,
                z: 0,
                faces: vec![Face::Bottom],
            });

            // ── Les deux murs, si personne ne les touche ────────────────
            //
            // Le mur GAUCHE expose sa face `Right` : le personnage se tient
            // à sa droite, c'est-à-dire à l'intérieur de l'écran. C'est la
            // même logique que le sol, dont la face `Top` regarde vers le
            // haut, donc vers l'intérieur (design §2.1).
            if !ecran_adjacent(screens, s, false) {
                platforms.push(Platform {
                    id: PlatformId::ecran(s.id, RoleEcran::MurGauche),
                    rect: Rect::new(
                        z.left() - EPAISSEUR_PLATEFORME,
                        z.top(),
                        EPAISSEUR_PLATEFORME,
                        z.h,
                    ),
                    kind: PlatformKind::Screen,
                    z: 0,
                    faces: vec![Face::Right],
                });
            }

            if !ecran_adjacent(screens, s, true) {
                platforms.push(Platform {
                    id: PlatformId::ecran(s.id, RoleEcran::MurDroit),
                    rect: Rect::new(z.right(), z.top(), EPAISSEUR_PLATEFORME, z.h),
                    kind: PlatformKind::Screen,
                    z: 0,
                    faces: vec![Face::Left],
                });
            }
        }

        World { platforms }
    }
```

Et la fonction libre, placée juste après l'`impl World` :

```rust
/// Un autre écran touche-t-il `s` de ce côté ?
///
/// **Règle binaire assumée** (design §2.3) : on ne regarde pas *quelle
/// portion* du bord est partagée, seulement s'il y a contact. Une adjacence
/// partielle — deux écrans de hauteurs différentes — fait donc perdre le mur
/// entier plutôt que sa moitié libre. C'est conservateur : on ne crée jamais
/// un mur fantôme, on en perd parfois un vrai. Le traitement rigoureux est la
/// soustraction d'intervalles 1D de la décision n° 2, réservée à l'étape 4
/// complète.
///
/// Le recouvrement vertical est exigé en plus du contact horizontal : un
/// écran placé en diagonale peut toucher la même ligne `x` sans être en face,
/// et son bord ne devrait alors rien masquer.
fn ecran_adjacent(screens: &[ScreenInfo], s: &ScreenInfo, a_droite: bool) -> bool {
    for autre in screens {
        if autre.id == s.id {
            continue;
        }

        // Les deux écrans se croisent-ils verticalement, ne serait-ce qu'un
        // peu ? `max des tops < min des bottoms` est le test d'intersection
        // d'intervalles habituel.
        let haut = s.work_area.top().max(autre.work_area.top());
        let bas = s.work_area.bottom().min(autre.work_area.bottom());
        if haut >= bas {
            continue;
        }

        let colle = if a_droite {
            (autre.work_area.left() - s.work_area.right()).abs() <= TOLERANCE_JONCTION
        } else {
            (s.work_area.left() - autre.work_area.right()).abs() <= TOLERANCE_JONCTION
        };

        if colle {
            return true;
        }
    }

    false
}
```

- [ ] **Étape 5 : `premier_sol`**

Dans l'`impl World`, à côté de `nearest_floor` :

```rust
    /// Le premier sol du monde, s'il y en a un.
    ///
    /// Existe pour que les appelants qui veulent dire « le sol » cessent
    /// d'écrire `platforms()[0]`, qui n'est vrai que par convention d'ordre.
    /// Utilisé au placement initial du personnage (`main.rs`).
    ///
    /// `find` sur un `Vec` de quelques éléments : inutile d'indexer.
    pub fn premier_sol(&self) -> Option<&Platform> {
        self.platforms.iter().find(|p| p.has_face(Face::Top))
    }
```

Dans `src-tauri/src/main.rs`, au placement initial (autour de la ligne 246), remplacer
`let sol = &monde.platforms()[0];` par une lecture qui ne suppose plus l'ordre :

```rust
            // `let … else` : sans écran, il n'y a nulle part où poser le
            // personnage. On sort du bloc de placement plutôt que de paniquer
            // — un monde vide est un cas normal (session distante en cours
            // d'établissement), voir `World::from_screens`.
            let Some(sol) = monde.premier_sol() else {
                return;
            };
```

> Adapter la sortie (`return`, `continue`…) à ce que fait le bloc englobant : lire les dix
> lignes autour avant d'écrire, et **ne pas introduire de `unwrap`**.

- [ ] **Étape 6 : lancer toute la suite**

```powershell
cargo test
```

Attendu : **tout passe**. Les tests d'autres modules qui écrivent `platforms()[0]`
continuent de désigner le sol du premier écran, puisqu'il est poussé en premier.

En cas d'échec de `bounds_englobe_tous_les_ecrans` : le plafond est à `top - 1`, donc
`bounds().top()` a bougé d'un pixel. C'est correct — ajuster l'attendu du test, pas le
code.

- [ ] **Étape 7 : commit**

```powershell
git add src-tauri/src/world.rs src-tauri/src/main.rs
git commit -m "feat(monde): les deux murs et le plafond de chaque ecran

Quatre plateformes par ecran au lieu d'une : sol, mur gauche, mur droit,
plafond. Chacune est un rectangle fin dont la face pointe vers l'interieur
de l'ecran, ce qui laisse geom.rs inchange (design 2.1).

Un mur n'est pose que si aucun autre ecran ne le touche a 8 px pres : deux
ecrans cote a cote se comportent donc comme une seule boite, sans mur
fantome au milieu du bureau (design 2.3).

PlatformId::ecran(monitor, role) porte le role sur deux bits de poids
faible ; l'identite reste independante de la geometrie.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Tâche 2 : `contact()` — un mur arrête une chute

**Fichiers :**
- Modifier : `src-tauri/src/character/physics.rs`
- Test : `src-tauri/src/character/physics.rs` (module `tests`)

**Interfaces :**
- Consomme : `World`, `PlatformId`, `Face`, `Point`, et `atterrissage` (existante, conservée)
- Produit : `pub fn contact(world: &World, avant: Point, apres: Point) -> Option<(PlatformId, Face, f32)>`

> **Rien n'est branché dans cette tâche.** `contact` est écrite et testée, le Réflexe 3
> continue d'appeler `atterrissage`. Le comportement à l'écran est donc rigoureusement
> inchangé, et la tâche est reviewable seule.

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans le module `tests` de `src-tauri/src/character/physics.rs` :

```rust
    /// Un monde d'un seul écran isolé : sol à y = 1032, mur gauche à x = 0,
    /// mur droit à x = 1920, plafond à y = 0.
    fn monde_isole() -> World {
        World::from_screens(&crate::probe::fake::FakeProbe::un_ecran().screens())
    }

    #[test]
    fn contact_rend_le_sol_comme_avant() {
        // La première moitié de `contact` est l'ancien `atterrissage`, et
        // elle ne doit rien changer.
        let m = monde_isole();
        let (_, face, offset) =
            contact(&m, Point::new(500.0, 1020.0), Point::new(500.0, 1040.0))
                .expect("il traverse la ligne du sol");
        assert_eq!(face, Face::Top);
        assert_eq!(offset, 500.0);
    }

    #[test]
    fn un_lancer_vers_la_gauche_s_accroche_au_mur_gauche() {
        let m = monde_isole();
        let (_, face, offset) =
            contact(&m, Point::new(20.0, 400.0), Point::new(-10.0, 420.0))
                .expect("il traverse la ligne x = 0 vers la gauche");

        // Le mur GAUCHE de l'écran expose sa face `Right` : le personnage se
        // tient à sa droite, donc à l'intérieur de l'écran.
        assert_eq!(face, Face::Right);
        // L'offset compte vers le BAS depuis le haut de la zone de travail.
        assert_eq!(offset, 420.0);
    }

    #[test]
    fn un_lancer_vers_la_droite_s_accroche_au_mur_droit() {
        let m = monde_isole();
        let (_, face, offset) =
            contact(&m, Point::new(1900.0, 300.0), Point::new(1930.0, 310.0))
                .expect("il traverse la ligne x = 1920 vers la droite");
        assert_eq!(face, Face::Left);
        assert_eq!(offset, 310.0);
    }

    #[test]
    fn on_ne_s_accroche_pas_a_un_mur_qu_on_quitte() {
        // Le sens compte : partir du mur vers l'intérieur ne doit PAS
        // s'accrocher, sinon un personnage qui se lâche se rattraperait
        // aussitôt.
        let m = monde_isole();
        assert_eq!(contact(&m, Point::new(-10.0, 400.0), Point::new(20.0, 420.0)), None);
    }

    #[test]
    fn le_sol_gagne_sur_le_mur_dans_un_coin() {
        // Repris de `Fall.java`, dont la boucle de sous-pas fait `break
        // OUTER` sur le sol AVANT de tester le mur. Sans cette priorité, un
        // lancer dans le coin s'accrocherait au mur trois pixels au-dessus du
        // sol au lieu d'atterrir — visiblement bête.
        let m = monde_isole();
        let (_, face, _) = contact(&m, Point::new(20.0, 1020.0), Point::new(-10.0, 1040.0))
            .expect("il franchit le sol ET le mur dans le même pas");
        assert_eq!(face, Face::Top);
    }

    #[test]
    fn le_plafond_n_attrape_rien() {
        // `Fall.java::hasNext()` teste le sol et le mur, PAS le plafond.
        // Lancé vers le haut, il passe devant et retombe (design §3.2).
        let m = monde_isole();
        assert_eq!(contact(&m, Point::new(500.0, 20.0), Point::new(500.0, -10.0)), None);
    }

    #[test]
    fn un_mur_hors_de_la_hauteur_du_pas_n_attrape_pas() {
        // Franchir la ligne x = 0 SOUS le bas de la zone de travail ne doit
        // pas s'accrocher : il n'y a plus de mur à cette hauteur.
        let m = monde_isole();
        assert_eq!(
            contact(&m, Point::new(20.0, 2000.0), Point::new(-10.0, 2020.0)),
            None
        );
    }

    #[test]
    fn aucun_seuil_de_vitesse_pour_s_accrocher() {
        // `Fall.java` ne teste qu'un `isOn`, sans aucune condition de
        // vitesse : un contact d'un pixel suffit. Ce test fige cette absence
        // de seuil, pour qu'on ne la « corrige » pas plus tard.
        let m = monde_isole();
        assert!(contact(&m, Point::new(0.5, 400.0), Point::new(-0.5, 400.1)).is_some());
    }
```

- [ ] **Étape 2 : lancer pour vérifier l'échec**

```powershell
cargo test physics::
```

Attendu : échec de compilation, `contact` n'existe pas.

- [ ] **Étape 3 : écrire `contact`**

Dans `src-tauri/src/character/physics.rs`, juste après `atterrissage` :

```rust
/// Ce que le personnage a heurté pendant ce pas de chute — sol **ou** mur.
///
/// Généralise `atterrissage` aux faces verticales (design §3.2). Rend la
/// `Face` en plus de la plateforme, parce que l'appelant en a besoin pour
/// choisir la pose et l'orientation : on ne se pose pas sur un mur comme on
/// se pose sur un sol.
///
/// **Les trois règles viennent de `Fall.java`, pas d'une intuition :**
///   1. le sol d'abord — sa boucle fait `break OUTER` sur le sol avant de
///      tester le mur ;
///   2. puis les murs — `hasNext()` teste `floor.isOn(pos) || wall.isOn(pos)`,
///      donc un mur arrête une chute exactement comme un sol, et **sans
///      aucun seuil de vitesse** ;
///   3. le plafond n'attrape rien — il ne figure dans aucun de ces tests.
pub fn contact(world: &World, avant: Point, apres: Point) -> Option<(PlatformId, Face, f32)> {
    // Règle 1. `if let Some(…)` et non un `?` : si le sol n'attrape rien, on
    // veut continuer vers les murs, pas sortir.
    if let Some((id, offset)) = atterrissage(world, avant, apres) {
        return Some((id, Face::Top, offset));
    }

    contact_mur(world, avant, apres)
}

/// Règle 2 : a-t-on traversé la ligne verticale d'un mur, dans le bon sens ?
///
/// Séparée de `contact` pour que la priorité au sol se lise en une ligne
/// plutôt que d'être enfouie dans une boucle.
fn contact_mur(world: &World, avant: Point, apres: Point) -> Option<(PlatformId, Face, f32)> {
    // (id, face, offset, x de la face) — le `x` ne sert qu'à départager.
    let mut meilleur: Option<(PlatformId, Face, f32, f32)> = None;

    for plat in world.platforms() {
        // Les deux faces verticales, dans un tableau : écrire deux fois le
        // même corps de boucle finirait par diverger.
        for face in [Face::Left, Face::Right] {
            if !plat.has_face(face) {
                continue;
            }

            // `point_on(face, 0.0).x` plutôt que `rect.left()` / `rect.right()`
            // écrits à la main : c'est la MÊME fonction qui placera le
            // personnage, donc les deux ne peuvent pas se désaccorder.
            let x_face = plat.rect.point_on(face, 0.0).x;

            // Le bon sens, et c'est le cœur du test. Une face `Left` regarde
            // vers la gauche : on la heurte en allant vers la DROITE. Une
            // face `Right` regarde vers la droite : on la heurte en allant
            // vers la gauche. Sans cette condition, un personnage qui se
            // lâche se rattraperait à l'image suivante.
            let franchie = match face {
                Face::Left => avant.x <= x_face && apres.x >= x_face,
                Face::Right => avant.x >= x_face && apres.x <= x_face,
                // Les faces horizontales ne passent jamais par ici : le
                // tableau ci-dessus n'en contient pas. `false` est le repli
                // muet correct.
                Face::Top | Face::Bottom => false,
            };
            if !franchie {
                continue;
            }

            // Est-on à la hauteur du mur ? Même approximation volontaire que
            // dans `atterrissage` : on teste avec le point d'ARRIVÉE plutôt
            // que le croisement exact. À 15 px par image au maximum, l'écart
            // est invisible, et la navigation a le droit d'être imparfaite
            // (décision n° 4).
            if apres.y < plat.rect.top() || apres.y > plat.rect.bottom() {
                continue;
            }

            // L'offset d'une face verticale compte vers le BAS depuis le haut
            // du rectangle — c'est la convention de `Rect::point_on`.
            let offset = apres.y - plat.rect.top();

            // Départage : garder le mur rencontré le PLUS TÔT, c'est-à-dire
            // le plus proche du point de départ. Le cas ne se présente
            // qu'avec des écrans qui se recouvrent, mais laisser le choix au
            // hasard de l'ordre du `Vec` serait un bug dormant.
            let remplace = match meilleur {
                None => true,
                Some((_, _, _, x)) => (x_face - avant.x).abs() < (x - avant.x).abs(),
            };
            if remplace {
                meilleur = Some((plat.id, face, offset, x_face));
            }
        }
    }

    meilleur.map(|(id, face, offset, _)| (id, face, offset))
}
```

Vérifier que `Face` est importée en tête de `physics.rs` (`use crate::geom::{Face, Point, Vec2};`
ou équivalent) et que le module de test importe `World` et `FakeProbe`.

- [ ] **Étape 4 : lancer les tests**

```powershell
cargo test physics::
cargo test
```

Attendu : **tout passe**, y compris les tests existants d'`atterrissage`, qui n'a pas
bougé.

- [ ] **Étape 5 : commit**

```powershell
git add src-tauri/src/character/physics.rs
git commit -m "feat(physique): contact() — un mur arrete une chute, sans seuil

Generalise atterrissage aux faces verticales et rend la Face heurtee.
Trois regles relevees dans Fall.java : le sol gagne sur le mur dans un
coin (break OUTER avant le test du mur), un mur arrete une chute sans
aucun seuil de vitesse (hasNext ne teste qu'un isOn), et le plafond
n'attrape rien (il ne figure dans aucun des deux tests).

Non branchee : le Reflexe 3 appelle toujours atterrissage. Le
comportement a l'ecran est inchange.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Tâche 3 : Fini sur une face verticale ⇒ il lâche

**Fichiers :**
- Modifier : `src-tauri/src/behavior/mod.rs`
- Test : `src-tauri/src/behavior/mod.rs` (module `tests`)

**Interfaces :**
- Consomme : `Attachment`, `World`, `Face`, `attach::world_position`
- Produit : rien de public — une règle interne à `behavior::pas`, insérée **entre la
  couche 2 et la couche 3**

> **Pourquoi cette tâche vient avant qu'on puisse grimper.** C'est le filet, et le poser
> d'abord garantit qu'il n'existe à aucun moment une version du programme où le
> personnage peut rester accroché sans intention. Testable seule : il suffit de le poser
> de force sur un mur.

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans le module `tests` de `src-tauri/src/behavior/mod.rs` :

```rust
    /// Le mur gauche du monde de test.
    fn mur_gauche(m: &World) -> &crate::world::Platform {
        m.platforms()
            .iter()
            .find(|p| p.has_face(Face::Right))
            .expect("un écran isolé a un mur gauche")
    }

    #[test]
    fn une_intention_finie_sur_un_mur_le_fait_lacher() {
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);

        // Accroché à mi-hauteur, sans intention : la couche 2 rendra donc
        // `Finie` dès la première image.
        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        ch.intention = None;

        let mut rng = XorShift32::seeded(1);
        pas(
            &mut ch,
            &m,
            &Entrees::default(),
            &desire::TableEnvies::defaut(),
            &crate::config::Reglages::depuis(&crate::config::Config::default()),
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        // Il tombe, et il tombe DE LÀ OÙ IL ÉTAIT — pas d'un point recalculé
        // ailleurs.
        match ch.attachment {
            Attachment::Falling { pos, vel } => {
                assert_eq!(pos, Point::new(0.0, 400.0));
                assert_eq!(vel, crate::geom::Vec2::zero());
            }
            autre => panic!("il devrait tomber, il est {autre:?}"),
        }
    }

    #[test]
    fn une_intention_finie_sur_le_sol_ne_le_fait_pas_lacher() {
        // Le sol reste le seul endroit où l'on peut ne rien faire.
        let m = monde();
        let mut ch = perso(&m);
        ch.intention = None;

        let mut rng = XorShift32::seeded(1);
        pas(
            &mut ch,
            &m,
            &Entrees::default(),
            &desire::TableEnvies::defaut(),
            &crate::config::Reglages::depuis(&crate::config::Config::default()),
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        assert!(
            matches!(ch.attachment, Attachment::On { face: Face::Top, .. }),
            "il ne doit pas quitter le sol"
        );
        assert!(ch.intention.is_some(), "la couche 3 doit lui en tirer une");
    }

    #[test]
    fn lacher_un_mur_n_ecrase_pas_une_chute_deja_en_cours() {
        // Le cas du personnage qui s'est lâché lui-même à l'image
        // précédente : il est déjà `Falling` avec une vitesse. La règle ne
        // doit pas la remettre à zéro.
        let m = monde();
        let mut ch = perso(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(100.0, 200.0),
            vel: crate::geom::Vec2::new(0.0, 300.0),
        };
        ch.intention = None;

        let mut rng = XorShift32::seeded(1);
        pas(
            &mut ch,
            &m,
            &Entrees::default(),
            &desire::TableEnvies::defaut(),
            &crate::config::Reglages::depuis(&crate::config::Config::default()),
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        match ch.attachment {
            // Le Réflexe 3 l'a fait avancer : la vitesse a grandi, elle n'a
            // pas été effacée.
            Attachment::Falling { vel, .. } => assert!(vel.y > 300.0),
            autre => panic!("il devrait toujours tomber, il est {autre:?}"),
        }
    }
```

- [ ] **Étape 2 : lancer pour vérifier l'échec**

```powershell
cargo test behavior::tests::une_intention_finie_sur_un_mur
```

Attendu : ÉCHEC — il reste accroché au mur (`assert` sur `Falling` non tenu).

- [ ] **Étape 3 : insérer la règle**

Dans `src-tauri/src/behavior/mod.rs`, **entre** le `match intention::poursuivre(…)` et le
bloc « Couche 3 » :

```rust
    // ── La règle de sécurité du monde vertical ──────────────────────────
    //
    // > **Toute intention qui se termine — finie, échouée, ou expirée au
    // > délai d'abandon — alors que le personnage est accroché à une face
    // > `Left`, `Right` ou `Bottom` le fait se lâcher.** (design §4.5)
    //
    // Sans elle, le tirage de la couche 3, juste en dessous, peut sortir
    // `Flaner` — et `avancer` déplace l'offset LE LONG DE LA FACE COURANTE,
    // donc le personnage « marcherait » verticalement le long du mur, en pose
    // de marche, avec des demi-tours. C'est le seul trou structurel de
    // l'étape, et cette règle le ferme entièrement.
    //
    // Conséquence à retenir : **le sol est le seul endroit où l'on peut ne
    // rien faire.** C'est aussi ce qui rend le délai d'abandon lisible à
    // l'œil — au bout de deux minutes il en a marre, il lâche, il tombe.
    // C'est `FallFromWall` de Shimeji-ee.
    //
    // `if let Attachment::On { … }` : les états `Falling` et `Dragged` ne
    // sont pas concernés — on ne lâche pas ce qu'on ne tient pas, et écraser
    // une chute en cours remettrait sa vitesse à zéro.
    if let crate::character::attach::Attachment::On { platform, face, offset } = ch.attachment {
        if face != crate::geom::Face::Top {
            // On repart du rectangle COURANT pour savoir d'où il tombe
            // (décision n° 1). `if let Some(…)` : si la plateforme a disparu
            // dans le même souffle, le Réflexe 1 s'en occupera à l'image
            // suivante — il n'y a rien à faire ici.
            if let Some(plat) = world.get(platform) {
                ch.attachment = crate::character::attach::Attachment::Falling {
                    pos: plat.rect.point_on(face, offset),
                    vel: crate::geom::Vec2::zero(),
                };
                ch.intention = None;

                // On rend la main : la chute est un réflexe, et c'est lui qui
                // posera la pose `fall` à l'image suivante. Tirer une envie
                // maintenant la ferait s'appliquer à un personnage en l'air.
                return r;
            }
        }
    }
```

> **Note d'emprunt (Rust).** `ch.attachment` est `Copy`, donc le `if let` en prend une
> copie et l'on peut réassigner `ch.attachment` dans le corps sans conflit d'emprunt. Si
> `Attachment` cessait d'être `Copy`, ce bloc ne compilerait plus — ce serait le signe
> qu'une variante a gagné un champ possédé, ce que `attach.rs` déconseille par ailleurs.

- [ ] **Étape 4 : lancer les tests**

```powershell
cargo test behavior::
cargo test
```

Attendu : tout passe.

- [ ] **Étape 5 : commit**

```powershell
git add src-tauri/src/behavior/mod.rs
git commit -m "feat(comportement): fini sur une face verticale, il lache

Toute intention qui se termine — finie, echouee ou expiree — alors que le
personnage est accroche a une face Left, Right ou Bottom le fait tomber.

Sans cette regle, le tirage de la couche 3 peut sortir Flaner, et avancer
deplace l'offset le long de la face courante : le personnage « marcherait »
verticalement le long du mur. C'est le seul trou structurel de l'etape,
et il est ferme avant meme qu'on puisse grimper.

Le sol reste le seul endroit ou l'on peut ne rien faire.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Tâche 4 : L'intention `Grimper`

**Fichiers :**
- Modifier : `src-tauri/src/behavior/intention.rs` (l'essentiel)
- Modifier : `src-tauri/src/character/physics.rs` (deux constantes)
- Modifier : `src-tauri/src/config.rs` (`Envies::grimper`, `Escalade`, `Reglages`)
- Modifier : `src-tauri/src/behavior/desire.rs` (une ligne de table)
- Modifier : `src-tauri/src/menu_perso.rs` (une ligne dans `ENVIES`)
- Modifier : `src-tauri/src/character/manifest.rs` (deux constantes de pose)
- Test : `src-tauri/src/behavior/intention.rs`, `src-tauri/src/menu_perso.rs`

**Interfaces :**
- Consomme : `World`, `PlatformId::meme_ecran`, `Face`, `Rect::point_on`, `Rng`
- Produit :
  - `Intention::Grimper` (nouvelle variante)
  - `pub enum PhaseGrimpe { Choisir, Rejoindre { mur: PlatformId }, Paroi { cible: f32 }, Accroche }`
  - `EtatIntention::Grimpe { phase: PhaseGrimpe, jusqu_a: Duration }`
  - `pub const POSE_GRAB_WALL: &str = "grabWall";` et `POSE_CLIMB_WALL: &str = "climbWall";`
  - `pub const VITESSE_ESCALADE: f32` et `pub const DUREE_ACCROCHE: [f32; 2]`
  - `Reglages::vitesse_escalade: f32`, `Reglages::escalade: Escalade`
  - `pub fn delai_abandon(kind: Intention) -> Duration`

> ⚠️ **La ligne du menu contextuel fait partie de CETTE tâche, pas d'une suivante.**
> C'est la règle explicite de `CLAUDE.md` : son oubli ne casse aucun test et ne produit
> aucun message — l'intention existerait pour le tirage mais resterait à jamais hors de
> portée de l'utilisateur.

- [ ] **Étape 1 : les constantes, relevées dans le source**

Dans `src-tauri/src/character/manifest.rs`, à côté des autres `POSE_*` :

```rust
/// La pose d'accroche à une paroi verticale — `GrabWall`, frame **13**.
///
/// ⚠️ Ce sont bien les frames **12, 13, 14** qui font le mur. Les frames
/// 23, 24, 25 sont celles du **plafond** (`GrabCeiling` / `ClimbCeiling`) :
/// la table de `CLAUDE.md` les attribuait à tort à la paroi verticale.
/// `docs/specs/2026-09-09-frames-shimeji.md` fait foi.
pub const POSE_GRAB_WALL: &str = "grabWall";

/// L'escalade d'une paroi — `ClimbWall`, frames 14, 12, 13.
pub const POSE_CLIMB_WALL: &str = "climbWall";
```

Dans `src-tauri/src/character/physics.rs`, à côté de `VITESSE_MARCHE` :

```rust
/// Vitesse d'escalade, en px/s.
///
/// **Relevée dans `conf/actions.xml`, pas réglée à l'œil.** L'action
/// `ClimbWall` enchaîne huit poses de durées 16, 4, 4, 4, 16, 4, 4, 4 ticks,
/// de vitesses 0, −1, −1, −1, 0, −2, −2, −2 px/tick. Déplacement :
/// `3×4×1 + 3×4×2 = 36 px`. Durée : `56 × 40 ms = 2,24 s`. Soit **16,1 px/s**,
/// c'est-à-dire **trois fois plus lent que la marche** (50 px/s).
///
/// C'est cette lenteur qui donne le « il se hisse » plutôt que « il glisse ».
/// Ne pas l'accélérer pour rendre l'escalade « plus fluide » : on perdrait
/// exactement ce qui la rend jolie. Un mur de 1032 px prend donc 64 s, ce qui
/// est la raison du délai d'abandon à 120 s (design §4.4).
pub const VITESSE_ESCALADE: f32 = 16.1;

/// Bornes de la durée d'accroche à une paroi, en secondes.
///
/// `HoldOntoWall` de `conf/actions.xml` : `Duration="${500+Math.random()*1000}"`,
/// en millisecondes.
pub const DUREE_ACCROCHE: [f32; 2] = [0.5, 1.5];
```

- [ ] **Étape 2 : les réglages**

Dans `src-tauri/src/config.rs`, ajouter le champ à `Envies` **et sa valeur par défaut** :

```rust
    /// Le poids de l'envie de grimper (étape 4a).
    pub grimper: f32,
```

```rust
            grimper: 1.0,
```

Puis, à côté d'`Allures`, une structure de réglages pour l'escalade :

```rust
/// Ce qui décide s'il est casse-cou ou prudent sur un mur (décision n° 5).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Escalade {
    /// Poids du tirage de sortie, en fin d'accroche : se lâcher et tomber.
    pub poids_lacher: f32,

    /// Poids du tirage de sortie : redescendre tranquillement.
    ///
    /// Deux fois le poids de `poids_lacher` par défaut : un personnage qui se
    /// lâcherait une fois sur deux passerait son temps en l'air, et la chute
    /// perdrait sa valeur de surprise.
    pub poids_redescendre: f32,

    /// Bornes de la durée d'une accroche, en secondes.
    pub duree_accroche: [f32; 2],
}

impl Default for Escalade {
    fn default() -> Self {
        Escalade {
            poids_lacher: 1.0,
            poids_redescendre: 2.0,
            duree_accroche: crate::character::physics::DUREE_ACCROCHE,
        }
    }
}
```

Déclarer le champ `pub escalade: Escalade` dans `Config` (avec son `Default`), puis dans
`Reglages` :

```rust
    /// Vitesse d'escalade, déjà multipliée par le facteur de vitesse de
    /// l'utilisateur — comme la marche et la course.
    pub vitesse_escalade: f32,

    pub escalade: Escalade,
```

et dans `Reglages::depuis` :

```rust
            vitesse_escalade: VITESSE_ESCALADE * facteur,
            escalade: config.escalade,
```

en ajoutant `VITESSE_ESCALADE` à l'import `use crate::character::physics::{…};`.

- [ ] **Étape 3 : écrire les tests qui échouent**

Dans le module `tests` de `src-tauri/src/behavior/intention.rs` :

```rust
    /// Un monde d'un écran isolé — donc avec ses deux murs.
    fn monde_mure() -> World {
        World::from_screens(&crate::probe::fake::FakeProbe::un_ecran().screens())
    }

    /// Fait tourner l'intention jusqu'à ce que `condition` soit vraie, ou
    /// jusqu'à `max_s` secondes simulées. Rend le temps écoulé.
    ///
    /// Écrit une fois ici plutôt que recopié dans chaque test : les tests
    /// d'escalade durent des dizaines de secondes simulées, et la boucle est
    /// toujours la même.
    fn derouler(
        ch: &mut Character,
        world: &World,
        rng: &mut dyn Rng,
        max_s: f32,
        mut condition: impl FnMut(&Character) -> bool,
    ) -> f32 {
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut t = Duration::ZERO;
        let mut ecoule = 0.0;

        while ecoule < max_s {
            if condition(ch) {
                return ecoule;
            }
            poursuivre(ch, world, &Entrees::default(), &reglages, t, DT, rng);
            t += Duration::from_secs_f32(DT);
            ecoule += DT;
        }

        ecoule
    }

    #[test]
    fn grimper_rejoint_le_mur_puis_s_y_accroche() {
        let m = monde_mure();
        let mut ch = perso_sur_le_sol(&m);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(7);
        derouler(&mut ch, &m, &mut rng, 60.0, |c| {
            matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top)
        });

        match ch.attachment {
            Attachment::On { face, .. } => {
                assert!(face == Face::Left || face == Face::Right, "il est sur un mur");
            }
            autre => panic!("il devrait être accroché, il est {autre:?}"),
        }
    }

    #[test]
    fn grimper_monte_vraiment() {
        let m = monde_mure();
        let mut ch = perso_sur_le_sol(&m);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(7);

        // On le laisse rejoindre le mur et grimper un moment.
        derouler(&mut ch, &m, &mut rng, 90.0, |c| {
            matches!(c.attachment, Attachment::On { face, offset, .. }
                     if face != Face::Top && offset < 900.0)
        });

        match ch.attachment {
            Attachment::On { face, offset, .. } => {
                assert_ne!(face, Face::Top);
                // L'offset d'une face verticale compte vers le bas : plus
                // petit = plus haut. Le bas du mur est à 1032.
                assert!(offset < 1000.0, "il devrait avoir quitté le bas du mur");
            }
            autre => panic!("il devrait être sur le mur, il est {autre:?}"),
        }
    }

    #[test]
    fn grimper_pose_les_bonnes_animations() {
        let m = monde_mure();
        let mut ch = perso_sur_le_sol(&m);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(7);
        derouler(&mut ch, &m, &mut rng, 90.0, |c| c.pose == POSE_CLIMB_WALL);
        assert_eq!(ch.pose, POSE_CLIMB_WALL);
    }

    #[test]
    fn grimper_regarde_le_mur() {
        // Mur gauche → il regarde à gauche ; mur droit → à droite. Sans quoi
        // il grimperait dos à la paroi.
        let m = monde_mure();
        let mut ch = perso_sur_le_sol(&m);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(7);
        derouler(&mut ch, &m, &mut rng, 90.0, |c| {
            matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top)
        });

        match ch.attachment {
            Attachment::On { face: Face::Right, .. } => {
                assert_eq!(ch.facing, Facing::Left, "mur gauche → il regarde à gauche")
            }
            Attachment::On { face: Face::Left, .. } => {
                assert_eq!(ch.facing, Facing::Right, "mur droit → il regarde à droite")
            }
            autre => panic!("il devrait être sur un mur, il est {autre:?}"),
        }
    }

    #[test]
    fn grimper_echoue_immediatement_sans_mur() {
        // L'écran du milieu d'une rangée de trois n'a aucun mur. L'intention
        // doit échouer tout de suite pour qu'une autre soit tirée — et
        // surtout pas figer le personnage.
        let sans_mur = World::from_screens(&[
            ScreenInfo { id: 1, work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0), scale: 1.0 },
            ScreenInfo { id: 2, work_area: Rect::new(1920.0, 0.0, 1920.0, 1032.0), scale: 1.0 },
        ]);
        // Sur ce monde, l'écran 1 n'a pas de mur droit et l'écran 2 pas de
        // mur gauche ; chacun garde le sien. On prend donc un cas vraiment
        // sans mur : un monde d'un seul écran dont les deux murs sont
        // masqués par des voisins.
        let entoure = World::from_screens(&[
            ScreenInfo { id: 1, work_area: Rect::new(-1920.0, 0.0, 1920.0, 1032.0), scale: 1.0 },
            ScreenInfo { id: 2, work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0), scale: 1.0 },
            ScreenInfo { id: 3, work_area: Rect::new(1920.0, 0.0, 1920.0, 1032.0), scale: 1.0 },
        ]);
        let _ = sans_mur;

        // Le personnage est sur le sol de l'écran 2, celui du milieu.
        let sol_milieu = entoure
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Top) && p.rect.left() == 0.0)
            .expect("le sol du milieu");
        let mut ch = perso_sur(&entoure, sol_milieu.id, 500.0);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(3);
        let issue = poursuivre(
            &mut ch, &entoure, &Entrees::default(), &reglages, Duration::ZERO, DT, &mut rng,
        );

        assert_eq!(issue, Issue::Echouee);
        assert!(ch.intention.is_none());
    }

    #[test]
    fn grimper_a_un_delai_d_abandon_de_120_s() {
        assert_eq!(delai_abandon(Intention::Grimper), Duration::from_secs(120));
        assert_eq!(delai_abandon(Intention::Flaner), DELAI_ABANDON);
        assert_eq!(delai_abandon(Intention::SeReposer), DELAI_ABANDON);
    }

    #[test]
    fn une_escalade_complete_tient_dans_le_delai_d_abandon() {
        // Le calcul du design §4.4, vérifié plutôt que supposé : marcher
        // jusqu'au bord (960 px au pire, à 50 px/s) plus grimper toute la
        // hauteur (1032 px à 16,1 px/s) doit tenir sous 120 s.
        let marche = 960.0 / crate::character::physics::VITESSE_MARCHE;
        let montee = 1032.0 / crate::character::physics::VITESSE_ESCALADE;
        assert!(
            marche + montee < 120.0,
            "une escalade complète dure {}s, au-dessus du délai",
            marche + montee
        );
    }

    #[test]
    fn en_fin_d_accroche_il_lache_parfois_et_redescend_parfois() {
        // Décision n° 3 appliquée à la sortie de mur : les deux issues
        // doivent réellement sortir. Une seule graine, l'état qui avance —
        // re-semer par petits entiers biaiserait le premier tirage.
        let m = monde_mure();
        let mut rng = XorShift32::seeded(12345);
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());

        let mut laches = 0;
        let mut descentes = 0;

        for _ in 0..200 {
            let mur = m
                .platforms()
                .iter()
                .find(|p| p.has_face(Face::Right))
                .expect("mur gauche");
            let mut ch = perso_sur_le_sol(&m);
            ch.attachment = Attachment::On {
                platform: mur.id,
                face: Face::Right,
                offset: 300.0,
            };
            // Une accroche déjà expirée : la prochaine image tire la sortie.
            ch.intention = Some(ActiveIntention {
                kind: Intention::Grimper,
                depuis: Duration::ZERO,
                etat: EtatIntention::Grimpe {
                    phase: PhaseGrimpe::Accroche,
                    jusqu_a: Duration::ZERO,
                },
            });

            poursuivre(
                &mut ch, &m, &Entrees::default(), &reglages,
                Duration::from_secs(1), DT, &mut rng,
            );

            match ch.attachment {
                Attachment::Falling { .. } => laches += 1,
                Attachment::On { .. } => descentes += 1,
                _ => {}
            }
        }

        assert!(laches > 10, "il ne se lâche jamais ({laches} sur 200)");
        assert!(descentes > 10, "il ne redescend jamais ({descentes} sur 200)");
    }
```

> **Sur les aides `perso_sur_le_sol` et `perso_sur`** : le module de test d'`intention.rs`
> construit déjà des personnages posés sur `m.platforms()[0]`. Extraire ces deux aides à
> partir du code existant plutôt que d'en inventer de nouvelles, et les réutiliser.

Dans `src-tauri/src/menu_perso.rs`, ajouter un test qui fige la règle du menu :

```rust
    #[test]
    fn toute_intention_de_la_table_d_envies_est_proposee_par_le_menu() {
        // ⚠️ Ce test est le rattrapage de l'oubli que `CLAUDE.md` décrit :
        // une intention qui existe pour le tirage mais qu'aucune entrée de
        // menu ne propose est un manque SILENCIEUX. Il ne l'est plus.
        let table = TableEnvies::defaut();
        for entree in &table.entrees {
            assert!(
                ENVIES.iter().any(|(_, _, i)| *i == entree.intention),
                "{:?} est tirable mais absente du menu contextuel",
                entree.intention
            );
        }
    }
```

- [ ] **Étape 4 : lancer pour vérifier l'échec**

```powershell
cargo test intention::
```

Attendu : échec de compilation — `Intention::Grimper`, `PhaseGrimpe` et `delai_abandon`
n'existent pas.

- [ ] **Étape 5 : les types**

Dans `src-tauri/src/behavior/intention.rs` :

```rust
pub enum Intention {
    Flaner,
    SeReposer,
    Jouer(Jeu),
    /// Aller sur un mur et y monter (étape 4a).
    ///
    /// **Cette intention possède tout le monde vertical**, et `Flaner`
    /// continue de ne connaître que le sol. L'alternative — généraliser
    /// `Flaner` à n'importe quelle face — est un piège : `avancer` déplace
    /// l'offset *le long de la face courante*, donc un `Flaner` sur un mur
    /// ferait monter et descendre le personnage en pose de marche, allure et
    /// demi-tours compris. Séparer coûte une variante ; fondre coûterait une
    /// matrice pose × face (design §4.1).
    Grimper,
}
```

```rust
/// Où en est une escalade.
///
/// Même forme que `PhaseRepos`, et pour la même raison : ce ne sont pas des
/// choix distincts, c'est *la suite* d'une même intention. Les mettre dans la
/// table d'envies demanderait au tirage de savoir où le personnage est
/// accroché, ce qui n'a rien à y faire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhaseGrimpe {
    /// Première image : choisir le mur. Existe pour que
    /// `ActiveIntention::nouvelle` reste **sans `World` ni `Rng`** — c'est
    /// déjà le parti pris des autres intentions, dont l'état initial est
    /// délibérément périmé pour que la première image décide.
    Choisir,

    /// Marcher vers le mur retenu. On mémorise **son identité**, jamais sa
    /// position : le monde est reconstruit à 8 Hz, et une position serait
    /// périmée (décision n° 1).
    Rejoindre { mur: PlatformId },

    /// Se déplacer le long de la paroi vers `cible`.
    ///
    /// La phase s'appelle `Paroi` et non `Monter` parce qu'elle sert dans les
    /// **deux sens** : monter, c'est une cible plus petite que l'offset
    /// courant ; redescendre, une cible plus grande. C'est la structure de
    /// `ClimbWall` chez Shimeji-ee, dont les deux animations sont
    /// conditionnées par `TargetY < mascot.anchor.y`.
    Paroi { cible: f32 },

    /// Accroché, immobile, le temps tiré au sort.
    Accroche,
}
```

Ajouter la variante d'état :

```rust
    Grimpe {
        phase: PhaseGrimpe,
        jusqu_a: Duration,
    },
```

et le cas d'`ActiveIntention::nouvelle` :

```rust
            Intention::Grimper => EtatIntention::Grimpe {
                phase: PhaseGrimpe::Choisir,
                jusqu_a: Duration::ZERO,
            },
```

- [ ] **Étape 6 : le délai d'abandon par intention**

Remplacer l'usage direct de la constante dans `poursuivre` :

```rust
/// Délai d'abandon de 120 s pour l'escalade (design §4.4).
///
/// **Pourquoi pas 20 s comme le reste.** L'escalade va à 16,1 px/s : un mur
/// de 1032 px prend 64 s, et la marche jusqu'au bord en ajoute jusqu'à 19.
/// Avec le délai commun, il abandonnerait toujours au tiers du mur et
/// n'atteindrait jamais le plafond.
///
/// La décision n° 4 écrit « ~20 s » avec un tilde : c'est une règle de
/// sécurité anti-blocage, pas un trait de caractère, et elle n'a pas de
/// raison d'être identique pour une intention trois fois plus lente.
pub const DELAI_ABANDON_GRIMPE: Duration = Duration::from_secs(120);

/// Le délai d'abandon qui s'applique à cette intention-là.
///
/// Une fonction et non une méthode de `Intention` : le délai est une règle
/// du moteur de comportement, pas une propriété de l'étiquette — la même
/// raison qui a fait de `vitesse_de(allure)` une fonction libre.
pub fn delai_abandon(kind: Intention) -> Duration {
    match kind {
        Intention::Grimper => DELAI_ABANDON_GRIMPE,
        Intention::Flaner | Intention::SeReposer | Intention::Jouer(_) => DELAI_ABANDON,
    }
}
```

et dans `poursuivre` :

```rust
    if maintenant.saturating_sub(ai.depuis) > delai_abandon(ai.kind) {
```

- [ ] **Étape 7 : la fonction `grimper`**

Dans `src-tauri/src/behavior/intention.rs`, après `flaner` :

```rust
/// Grimper : rejoindre un mur, y monter, s'y accrocher, puis en sortir.
///
/// Les transitions de coin (sol → mur, mur → sol) vivent ici et non dans
/// `world.rs` parce que ce sont des **décisions de navigation**, pas des
/// propriétés du monde — la même raison qui place déjà `face_voisine` dans ce
/// fichier (design §4.2).
fn grimper(
    ch: &mut Character,
    world: &World,
    reglages: &Reglages,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    dt: f32,
    rng: &mut dyn Rng,
) -> Issue {
    let EtatIntention::Grimpe {
        mut phase,
        mut jusqu_a,
    } = ai.etat
    else {
        ch.intention = None;
        return Issue::Echouee;
    };

    match phase {
        // ── Choisir le mur ──────────────────────────────────────────────
        PhaseGrimpe::Choisir => {
            // `let … else` : s'il n'est pas posé quelque part, il n'y a pas
            // d'écran de référence. Les réflexes s'occupent de lui.
            let Attachment::On { platform, .. } = ch.attachment else {
                ch.intention = None;
                return Issue::Echouee;
            };

            let Some(mur) = mur_le_plus_proche(world, platform, ch, world) else {
                // Aucun mur sur cet écran — l'écran du milieu d'une rangée de
                // trois. L'intention échoue, la couche 3 en tire une autre.
                // **Aucun cas particulier ailleurs** : c'est le même esprit
                // que la couverture partielle (spec §8.6).
                ch.intention = None;
                return Issue::Echouee;
            };

            phase = PhaseGrimpe::Rejoindre { mur };
        }

        // ── Marcher jusqu'au pied du mur ────────────────────────────────
        PhaseGrimpe::Rejoindre { mur } => {
            let Some(plat_mur) = world.get(mur) else {
                // Écran débranché en cours de route.
                ch.intention = None;
                return Issue::Echouee;
            };

            // La face du mur est la seule de sa liste — un mur n'en expose
            // qu'une.
            let Some(face_mur) = plat_mur.faces.first().copied() else {
                ch.intention = None;
                return Issue::Echouee;
            };

            let x_mur = plat_mur.rect.point_on(face_mur, 0.0).x;
            let Some(pos) = position_actuelle(ch, world) else {
                ch.intention = None;
                return Issue::Echouee;
            };

            // Il regarde le mur, et il marche vers lui.
            ch.facing = if x_mur < pos.x { Facing::Left } else { Facing::Right };
            ch.set_pose(POSE_WALK, maintenant);

            let pas = reglages.vitesse_marche * dt;

            if (x_mur - pos.x).abs() <= pas {
                // Arrivé : on s'accroche au BAS du mur. L'offset d'une face
                // verticale compte vers le bas depuis le haut du rectangle,
                // donc le bas du mur est à `face_length`.
                let longueur = plat_mur.rect.face_length(face_mur);
                ch.attachment = Attachment::On {
                    platform: mur,
                    face: face_mur,
                    offset: longueur,
                };

                // Jusqu'où monter ? Deux comportements de Shimeji-ee :
                // `ClimbAlongWall` va jusqu'en haut, `ClimbHalfwayAlongWall`
                // s'arrête à une hauteur tirée. On tire entre les deux — la
                // marge est le produit (décision n° 3).
                let cible = if rng.unit_f32() < 0.5 {
                    0.0
                } else {
                    rng.range(0.0, longueur * 0.7)
                };
                phase = PhaseGrimpe::Paroi { cible };
            } else {
                avancer(ch, world, pas * ch.facing.signe());
            }
        }

        // ── Monter, ou redescendre ──────────────────────────────────────
        PhaseGrimpe::Paroi { cible } => {
            let Attachment::On { platform, face, offset } = ch.attachment else {
                // Il a été attrapé, ou il est tombé : les réflexes ont déjà
                // tranché, on ne discute pas.
                ch.intention = None;
                return Issue::Echouee;
            };

            let Some(plat) = world.get(platform) else {
                ch.intention = None;
                return Issue::Echouee;
            };

            ch.set_pose(POSE_CLIMB_WALL, maintenant);

            let pas = reglages.vitesse_escalade * dt;
            let reste = cible - offset;

            if reste.abs() <= pas {
                // Cible atteinte. Si c'était le bas du mur, l'escalade est
                // finie et il repasse sur le sol.
                let longueur = plat.rect.face_length(face);
                if cible >= longueur - 1.0 {
                    match sol_au_pied_du_mur(world, platform) {
                        Some((sol, offset_sol)) => {
                            ch.attachment = Attachment::On {
                                platform: sol,
                                face: Face::Top,
                                offset: offset_sol,
                            };
                            ch.set_pose(POSE_STAND, maintenant);
                            ch.intention = None;
                            return Issue::Finie;
                        }
                        None => {
                            // Pas de sol retrouvé : il se lâche. La règle de
                            // sécurité l'aurait fait de toute façon, mais le
                            // dire ici évite une image de flottement.
                            ch.intention = None;
                            return Issue::Echouee;
                        }
                    }
                }

                phase = PhaseGrimpe::Accroche;
                let d = reglages.escalade.duree_accroche;
                jusqu_a = maintenant + Duration::from_secs_f32(rng.range(d[0], d[1]));
            } else {
                // `signum` donne le sens : −1 vers le haut, +1 vers le bas.
                ch.attachment = Attachment::On {
                    platform,
                    face,
                    offset: offset + pas * reste.signum(),
                };
            }
        }

        // ── Accroché, puis la sortie tirée au sort ──────────────────────
        PhaseGrimpe::Accroche => {
            ch.set_pose(POSE_GRAB_WALL, maintenant);

            if maintenant < jusqu_a {
                ai.etat = EtatIntention::Grimpe { phase, jusqu_a };
                return Issue::EnCours;
            }

            let Attachment::On { platform, face, offset } = ch.attachment else {
                ch.intention = None;
                return Issue::Echouee;
            };
            let Some(plat) = world.get(platform) else {
                ch.intention = None;
                return Issue::Echouee;
            };

            // Décision n° 5 : les deux poids viennent de `config.json`, on
            // règle s'il est casse-cou ou prudent sans recompiler.
            let e = &reglages.escalade;
            let lache = match rng.weighted(&[e.poids_lacher, e.poids_redescendre]) {
                Some(0) => true,
                // `Some(1)` redescend, et `None` aussi — il n'arrive que si
                // les deux poids sont nuls, auquel cas redescendre est le
                // repli le moins surprenant.
                _ => false,
            };

            if lache {
                ch.attachment = Attachment::Falling {
                    pos: plat.rect.point_on(face, offset),
                    vel: crate::geom::Vec2::zero(),
                };
                ch.intention = None;
                return Issue::Finie;
            }

            phase = PhaseGrimpe::Paroi {
                cible: plat.rect.face_length(face),
            };
        }
    }

    ai.etat = EtatIntention::Grimpe { phase, jusqu_a };
    Issue::EnCours
}

/// Le mur de l'écran du personnage le plus proche de lui.
///
/// « De son écran » : c'est à cela que sert `PlatformId::meme_ecran`. Sans ce
/// filtre, un personnage sur l'écran de gauche pourrait viser le mur droit de
/// l'écran de droite, à 3 000 px — une marche de 60 s pour rien.
fn mur_le_plus_proche(
    world: &World,
    depuis: PlatformId,
    ch: &Character,
    monde: &World,
) -> Option<PlatformId> {
    let pos = position_actuelle(ch, monde)?;
    let mut meilleur: Option<(PlatformId, f32)> = None;

    for plat in world.platforms() {
        if !plat.id.meme_ecran(depuis) {
            continue;
        }
        // Un mur, c'est-à-dire une face verticale.
        let Some(face) = plat.faces.first().copied() else {
            continue;
        };
        if face != Face::Left && face != Face::Right {
            continue;
        }

        let d = (plat.rect.point_on(face, 0.0).x - pos.x).abs();
        let remplace = match meilleur {
            None => true,
            Some((_, best)) => d < best,
        };
        if remplace {
            meilleur = Some((plat.id, d));
        }
    }

    meilleur.map(|(id, _)| id)
}

/// Le sol sur lequel reposer en bas d'un mur, et l'offset où y arriver.
///
/// Le mur et le sol appartiennent au même écran, donc `meme_ecran` suffit —
/// inutile de chercher géométriquement.
fn sol_au_pied_du_mur(world: &World, mur: PlatformId) -> Option<(PlatformId, f32)> {
    let plat_mur = world.get(mur)?;
    let face_mur = plat_mur.faces.first().copied()?;
    let x = plat_mur.rect.point_on(face_mur, 0.0).x;

    for plat in world.platforms() {
        if !plat.id.meme_ecran(mur) || !plat.has_face(Face::Top) {
            continue;
        }
        // `clamp` : on rabat dans les bornes du sol, le mur étant exactement
        // sur son bord à un pixel près.
        let offset = (x - plat.rect.left()).clamp(0.0, plat.rect.face_length(Face::Top));
        return Some((plat.id, offset));
    }

    None
}

/// La position écran actuelle, quand elle existe.
///
/// Enveloppe `attach::world_position` avec un curseur factice : le personnage
/// n'est jamais `Dragged` quand cette fonction est appelée depuis une
/// intention — les réflexes ont la priorité sur le portage, et ils rendent la
/// main avant. Le point passé n'est donc jamais lu.
fn position_actuelle(ch: &Character, world: &World) -> Option<Point> {
    crate::character::attach::world_position(&ch.attachment, world, Point::new(0.0, 0.0))
}
```

Enfin, brancher le cas dans `poursuivre`, à côté des trois autres :

```rust
        Intention::Grimper => {
            let issue = grimper(ch, world, reglages, &mut ai, maintenant, dt, rng);
            if ch.intention.is_some() {
                ch.intention = Some(ai);
            }
            issue
        }
```

> **Note (Rust) :** `mur_le_plus_proche` reçoit `world` deux fois dans l'esquisse
> ci-dessus (`world` et `monde`). C'est une maladresse à corriger en écrivant : un seul
> paramètre `world: &World` suffit. Le signaler ici parce que le compilateur ne le dira
> pas — il accepterait les deux.

- [ ] **Étape 8 : l'envie, le menu, la config**

Dans `src-tauri/src/behavior/desire.rs`, une entrée de plus dans `depuis_config` :

```rust
                // L'escalade (étape 4a). Les deux poses suffisent : sans
                // `grabWall` il ne saurait pas tenir, sans `climbWall` il ne
                // saurait pas monter. Un pack qui n'a ni l'une ni l'autre ne
                // grimpera JAMAIS, et il n'y a aucun cas particulier ailleurs
                // (spec §8.6).
                //
                // `climbCeiling` n'y figure pas volontairement : un pack qui
                // sait grimper mais pas se suspendre grimpe quand même, et
                // s'arrête en haut du mur (Tâche 6).
                EntreeEnvie {
                    intention: Intention::Grimper,
                    base: config.envies.grimper,
                    poses_requises: &[POSE_GRAB_WALL, POSE_CLIMB_WALL],
                },
```

Dans `src-tauri/src/menu_perso.rs`, une ligne dans `ENVIES` :

```rust
    ("perso.grimper", "Grimper au mur", Intention::Grimper),
```

- [ ] **Étape 9 : lancer les tests**

```powershell
cargo test intention::
cargo test menu_perso::
cargo test
```

Attendu : tout passe. Si `en_fin_d_accroche_il_lache_parfois_et_redescend_parfois` rend
0 d'un côté, **ne pas re-semer par petits entiers** pour « mieux échantillonner » : c'est
le piège documenté dans `CLAUDE.md`, qui produit des faux négatifs complets. Une seule
graine, l'état qui avance.

- [ ] **Étape 10 : le regarder vraiment**

```powershell
cargo build
cargo run
```

Clic droit sur le personnage → **« Grimper au mur »**. Attendu : il marche vers le bord
le plus proche, s'y accroche, monte lentement (trois fois plus lent que sa marche : c'est
normal), s'arrête, puis tombe ou redescend.

À ce stade, l'ancre n'est pas encore mesurée : il est probablement à moitié hors écran.
**C'est attendu, et c'est la Tâche 7.**

- [ ] **Étape 11 : commit**

```powershell
git add src-tauri/src/behavior/intention.rs src-tauri/src/behavior/desire.rs src-tauri/src/menu_perso.rs src-tauri/src/config.rs src-tauri/src/character/physics.rs src-tauri/src/character/manifest.rs
git commit -m "feat(comportement): l'intention Grimper

Quatre phases — choisir le mur, le rejoindre, la paroi, l'accroche — sur
le modele de PhaseRepos. La phase Paroi sert dans les deux sens : monter,
c'est une cible plus petite que l'offset ; redescendre, une cible plus
grande (ClimbWall de Shimeji-ee a deux animations conditionnees par
TargetY < anchor.y).

Flaner continue de ne connaitre que le sol : avancer deplace l'offset le
long de la face courante, donc un Flaner sur un mur marcherait
verticalement. Separer coute une variante, fondre couterait une matrice
pose x face.

Le delai d'abandon devient une fonction de l'intention : 120 s pour
Grimper, 20 s pour le reste. A 16,1 px/s, un mur de 1032 px prend 64 s.

La ligne du menu contextuel est dans ce commit, comme l'exige CLAUDE.md,
et un test verifie desormais que toute intention tirable y figure.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Tâche 5 : Le lancer qui s'accroche

**Fichiers :**
- Modifier : `src-tauri/src/behavior/reflex.rs`
- Modifier : `src-tauri/src/behavior/intention.rs` (un constructeur)
- Test : `src-tauri/src/behavior/reflex.rs`

**Interfaces :**
- Consomme : `contact` (Tâche 2), `PhaseGrimpe::Accroche` (Tâche 4)
- Produit :
  - `Reflexe::Accroche` (nouvelle variante)
  - `impl ActiveIntention { pub fn accroche_au_mur(maintenant: Duration) -> Self }`

> **Le point non évident de cette tâche.** Poser la pose `grabWall` et laisser
> `ch.intention = None` ne suffirait pas : la couche 2 rendrait `Finie`, et la règle de
> sécurité de la Tâche 3 le ferait **tomber aussitôt**. Jeté contre un mur, il ne
> tiendrait qu'une image. Il faut donc lui **installer une intention `Grimper` déjà en
> phase `Accroche`** — exactement le motif d'`ActiveIntention::reveil`, qui est posée
> depuis `main.rs` au déverrouillage.

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans le module `tests` de `src-tauri/src/behavior/reflex.rs` :

```rust
    #[test]
    fn jete_contre_un_mur_il_s_y_accroche() {
        let m = World::from_screens(&FakeProbe::un_ecran().screens());
        let mut ch = perso(&m);

        // Lancé vers la gauche, à mi-hauteur.
        ch.attachment = Attachment::Falling {
            pos: Point::new(30.0, 400.0),
            vel: crate::geom::Vec2::new(-600.0, 0.0),
        };

        // Quelques images suffisent pour parcourir les 30 px.
        let mut accroche = false;
        for i in 0..20 {
            let r = appliquer(
                &mut ch, &m, &Entrees::default(),
                Duration::from_secs_f32(i as f32 * DT), DT,
            );
            if r == Reflexe::Accroche {
                accroche = true;
                break;
            }
        }

        assert!(accroche, "il devrait s'accrocher au mur gauche");
        assert!(matches!(ch.attachment, Attachment::On { face: Face::Right, .. }));
        assert_eq!(ch.pose, POSE_GRAB_WALL);
        assert_eq!(ch.facing, Facing::Left, "il regarde le mur");
    }

    #[test]
    fn accroche_par_un_lancer_il_ne_lache_pas_a_l_image_suivante() {
        // LE test de la tâche. Sans l'intention posée en phase `Accroche`,
        // la couche 2 rend `Finie` et la règle de sécurité le fait tomber :
        // jeté contre un mur, il ne tiendrait qu'une image.
        let m = World::from_screens(&FakeProbe::un_ecran().screens());
        let mut ch = perso(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(30.0, 400.0),
            vel: crate::geom::Vec2::new(-600.0, 0.0),
        };

        for i in 0..20 {
            let r = appliquer(
                &mut ch, &m, &Entrees::default(),
                Duration::from_secs_f32(i as f32 * DT), DT,
            );
            if r == Reflexe::Accroche {
                break;
            }
        }

        assert!(
            ch.intention.is_some(),
            "une intention doit avoir été posée, sinon la règle de sécurité le lâche"
        );

        // Et on le vérifie réellement, en faisant tourner le comportement
        // complet plusieurs images.
        let mut rng = crate::rng::XorShift32::seeded(4);
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        for i in 0..10 {
            crate::behavior::pas(
                &mut ch, &m, &Entrees::default(),
                &crate::behavior::desire::TableEnvies::defaut(), &reglages,
                Duration::from_secs_f32(1.0 + i as f32 * DT), DT, &mut rng,
            );
        }

        assert!(
            matches!(ch.attachment, Attachment::On { face: Face::Right, .. }),
            "il doit tenir le mur, il est {:?}",
            ch.attachment
        );
    }

    #[test]
    fn jete_dans_un_coin_il_atterrit_au_lieu_de_s_accrocher() {
        let m = World::from_screens(&FakeProbe::un_ecran().screens());
        let mut ch = perso(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(30.0, 1020.0),
            vel: crate::geom::Vec2::new(-600.0, 400.0),
        };

        for i in 0..20 {
            let r = appliquer(
                &mut ch, &m, &Entrees::default(),
                Duration::from_secs_f32(i as f32 * DT), DT,
            );
            if r == Reflexe::Atterrissage || r == Reflexe::Accroche {
                break;
            }
        }

        assert!(
            matches!(ch.attachment, Attachment::On { face: Face::Top, .. }),
            "le sol gagne sur le mur"
        );
    }
```

- [ ] **Étape 2 : lancer pour vérifier l'échec**

```powershell
cargo test reflex::tests::jete_contre_un_mur
```

Attendu : ÉCHEC — `Reflexe::Accroche` n'existe pas.

- [ ] **Étape 3 : le constructeur d'intention**

Dans `src-tauri/src/behavior/intention.rs`, à côté de `reveil` :

```rust
    /// L'intention posée quand un lancer vient de le coller à un mur.
    ///
    /// **Elle est indispensable, et sa raison n'est pas évidente.** Laisser
    /// `intention = None` ferait rendre `Finie` à la couche 2, et la règle de
    /// sécurité du monde vertical le ferait tomber à l'image suivante : jeté
    /// contre un mur, il ne tiendrait qu'une image.
    ///
    /// Même motif qu'`ActiveIntention::reveil` : l'état est POSÉ de
    /// l'extérieur, avec `jusqu_a` à zéro pour que la première image tire la
    /// durée — ce qui permet à `reflex.rs` de la construire **sans générateur
    /// aléatoire**, et garde toutes les durées dans ce fichier-ci.
    pub fn accroche_au_mur(maintenant: Duration) -> Self {
        ActiveIntention {
            kind: Intention::Grimper,
            depuis: maintenant,
            etat: EtatIntention::Grimpe {
                phase: PhaseGrimpe::Accroche,
                jusqu_a: Duration::ZERO,
            },
        }
    }
```

- [ ] **Étape 4 : brancher le Réflexe 3**

Dans `src-tauri/src/behavior/reflex.rs`, ajouter la variante :

```rust
    /// Il vient de s'accrocher à une paroi verticale — le lancer contre un
    /// mur (design §3.2). Distinct d'`Atterrissage` pour que la trace du mode
    /// simulation puisse les compter séparément.
    Accroche,
```

puis remplacer l'appel à `atterrissage` par `contact` :

```rust
        // A-t-on heurté quelque chose pendant ce pas ? `contact` rend la
        // FACE, parce qu'on ne se pose pas sur un mur comme sur un sol.
        if let Some((platform, face, offset)) = contact(world, pos, nouvelle_pos) {
            ch.attachment = Attachment::On { platform, face, offset };

            // `match` explicite plutôt qu'un `if face == Face::Top` : les
            // quatre cas se lisent d'un coup, et le compilateur exigera d'en
            // traiter un cinquième si `Face` en gagnait un.
            return match face {
                Face::Top => {
                    ch.set_pose(POSE_LAND, maintenant);
                    ch.intention = None;
                    Reflexe::Atterrissage
                }

                // Une face `Right` est celle d'un mur GAUCHE d'écran : le
                // personnage se tient à sa droite, donc il regarde à gauche
                // pour faire face à la paroi. Et symétriquement.
                Face::Right | Face::Left => {
                    ch.facing = if face == Face::Right {
                        Facing::Left
                    } else {
                        Facing::Right
                    };
                    ch.set_pose(POSE_GRAB_WALL, maintenant);

                    // ⚠️ Voir le commentaire d'`accroche_au_mur` : sans cette
                    // intention, la règle de sécurité le ferait tomber à
                    // l'image suivante.
                    ch.intention = Some(
                        crate::behavior::intention::ActiveIntention::accroche_au_mur(maintenant),
                    );
                    Reflexe::Accroche
                }

                // `contact` ne rend jamais `Bottom` : le plafond n'attrape
                // rien (design §3.2, d'après `Fall.java`). On ne panique pas
                // pour autant — on traite comme une chute qui continue.
                Face::Bottom => Reflexe::Chute,
            };
        }
```

- [ ] **Étape 5 : lancer les tests**

```powershell
cargo test reflex::
cargo test
```

Attendu : tout passe. Les tests existants d'atterrissage sur le sol continuent de rendre
`Reflexe::Atterrissage`.

- [ ] **Étape 6 : le regarder**

```powershell
cargo build
cargo run
```

Attraper le personnage à la souris et le **jeter vers le bord de l'écran**. Attendu : il
s'y colle au lieu de tomber, tient une seconde, puis lâche ou redescend.

- [ ] **Étape 7 : commit**

```powershell
git add src-tauri/src/behavior/reflex.rs src-tauri/src/behavior/intention.rs
git commit -m "feat(reflexe): jete contre un mur, il s'y accroche

Le Reflexe 3 appelle contact() au lieu d'atterrissage, et pose grabWall
en orientant le personnage vers la paroi. Nouvelle variante
Reflexe::Accroche, distincte d'Atterrissage pour la trace du mode
simulation.

Le point non evident : laisser intention = None ne suffirait pas. La
couche 2 rendrait Finie et la regle de securite du monde vertical le
ferait tomber a l'image suivante — jete contre un mur, il ne tiendrait
qu'une image. On lui pose donc une intention Grimper deja en phase
Accroche, sur le meme motif qu'ActiveIntention::reveil.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Tâche 6 : Le plafond

**Fichiers :**
- Modifier : `src-tauri/src/behavior/intention.rs` (la phase `Plafond`, `face_voisine`)
- Modifier : `src-tauri/src/character/manifest.rs` (deux constantes de pose)
- Test : `src-tauri/src/behavior/intention.rs`

**Interfaces :**
- Consomme : `PhaseGrimpe` (Tâche 4), `Face::Bottom`
- Produit :
  - `PhaseGrimpe::Plafond { cible: f32 }`
  - `pub const POSE_GRAB_CEILING: &str = "grabCeiling";` et `POSE_CLIMB_CEILING: &str = "climbCeiling";`
  - `face_voisine` généralisée aux faces `Bottom`

- [ ] **Étape 1 : écrire les tests qui échouent**

```rust
    #[test]
    fn arrive_en_haut_du_mur_il_peut_basculer_au_plafond() {
        let m = monde_mure();
        let mur = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Right))
            .expect("mur gauche");

        // Posé en haut du mur, accroche expirée : la prochaine image tire la
        // sortie, et le plafond doit en faire partie.
        let mut rng = XorShift32::seeded(999);
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());

        let mut vus_au_plafond = 0;
        for _ in 0..200 {
            let mut ch = perso_sur_le_sol(&m);
            ch.attachment = Attachment::On {
                platform: mur.id,
                face: Face::Right,
                offset: 0.0, // tout en haut
            };
            ch.intention = Some(ActiveIntention {
                kind: Intention::Grimper,
                depuis: Duration::ZERO,
                etat: EtatIntention::Grimpe {
                    phase: PhaseGrimpe::Accroche,
                    jusqu_a: Duration::ZERO,
                },
            });

            poursuivre(
                &mut ch, &m, &Entrees::default(), &reglages,
                Duration::from_secs(1), DT, &mut rng,
            );

            if matches!(ch.attachment, Attachment::On { face: Face::Bottom, .. }) {
                vus_au_plafond += 1;
            }
        }

        assert!(vus_au_plafond > 10, "il ne passe jamais au plafond ({vus_au_plafond} sur 200)");
    }

    #[test]
    fn au_plafond_il_traverse_avec_la_bonne_pose() {
        let m = monde_mure();
        let plafond = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Bottom))
            .expect("plafond");

        let mut ch = perso_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: plafond.id,
            face: Face::Bottom,
            offset: 200.0,
        };
        ch.intention = Some(ActiveIntention {
            kind: Intention::Grimper,
            depuis: Duration::ZERO,
            etat: EtatIntention::Grimpe {
                phase: PhaseGrimpe::Plafond { cible: 800.0 },
                jusqu_a: Duration::ZERO,
            },
        });

        let mut rng = XorShift32::seeded(5);
        derouler(&mut ch, &m, &mut rng, 10.0, |c| c.pose == POSE_CLIMB_CEILING);
        assert_eq!(ch.pose, POSE_CLIMB_CEILING);

        match ch.attachment {
            Attachment::On { face: Face::Bottom, offset, .. } => {
                assert!(offset > 200.0, "il doit avoir avancé vers sa cible");
            }
            autre => panic!("il devrait être au plafond, il est {autre:?}"),
        }
    }

    #[test]
    fn le_plafond_d_un_ecran_prolonge_celui_du_voisin() {
        // La généralisation de `face_voisine` aux faces `Bottom` : au bout du
        // plafond de A, il passe sur celui de B, comme il le fait déjà au sol.
        let m = World::from_screens(&crate::probe::fake::FakeProbe::deux_ecrans().screens());
        let plafond_a = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Bottom) && p.rect.left() == 0.0)
            .expect("plafond de gauche");

        let voisin = face_voisine(&m, plafond_a.id, Face::Bottom, true);
        assert!(voisin.is_some(), "le plafond de droite doit être trouvé");
        let (id, offset) = voisin.unwrap();
        assert_ne!(id, plafond_a.id);
        assert_eq!(offset, 0.0, "on y entre par son bord gauche");
    }

    #[test]
    fn un_pack_sans_pose_de_plafond_grimpe_quand_meme() {
        // Couverture partielle (spec §8.6) : `climbCeiling` n'est PAS dans
        // les poses requises de `Grimper`. Un pack qui ne l'a pas doit
        // pouvoir grimper au mur, et simplement ne jamais passer au plafond.
        let table = crate::behavior::desire::TableEnvies::defaut();
        let sans_plafond = manifeste_sans(&[POSE_CLIMB_CEILING, POSE_GRAB_CEILING]);
        assert!(table.jouable(&sans_plafond, Intention::Grimper));
    }
```

> `manifeste_sans(&[…])` : une aide locale au module de test qui charge `blob` puis
> retire les poses nommées de la `BTreeMap`. Si une aide équivalente existe déjà dans ce
> module, la réutiliser plutôt que d'en créer une seconde.

- [ ] **Étape 2 : lancer pour vérifier l'échec**

```powershell
cargo test intention::tests::arrive_en_haut_du_mur
```

Attendu : ÉCHEC — `PhaseGrimpe::Plafond` n'existe pas.

- [ ] **Étape 3 : les constantes de pose**

Dans `src-tauri/src/character/manifest.rs` :

```rust
/// La pose de suspension au plafond — `GrabCeiling`, frame **23**, ancre
/// `64,48`.
pub const POSE_GRAB_CEILING: &str = "grabCeiling";

/// Le déplacement au plafond — `ClimbCeiling`, frames 23, 24, 25.
pub const POSE_CLIMB_CEILING: &str = "climbCeiling";
```

- [ ] **Étape 4 : généraliser `face_voisine`**

Dans `src-tauri/src/behavior/intention.rs`, donner une `face` en paramètre et remplacer
les `Face::Top` codés en dur :

```rust
/// Cherche une plateforme adjacente à celle de `depuis`, exposant la MÊME
/// face, du côté demandé et à peu près à la même hauteur.
///
/// Vit ici et non dans `world.rs` parce que c'est une **décision de
/// navigation**, pas une propriété du monde : « ce sol en prolonge-t-il un
/// autre ? » n'a de sens que pour quelqu'un qui marche dessus.
///
/// Généralisée aux faces `Bottom` à l'étape 4a : le plafond d'un écran
/// prolonge celui du voisin exactement comme le sol prolonge le sol. **Un
/// seul chemin de code pour les deux** — en écrire un second finirait par
/// diverger.
fn face_voisine(
    world: &World,
    depuis: PlatformId,
    face: Face,
    vers_la_droite: bool,
) -> Option<(PlatformId, f32)> {
    const TOLERANCE: f32 = 8.0;

    let source = world.get(depuis)?;
    // La ligne de référence : le haut du rectangle pour un sol, le bas pour
    // un plafond. C'est `point_on(face, 0.0).y` qui la donne, donc la MÊME
    // fonction que celle qui place le personnage.
    let hauteur = source.rect.point_on(face, 0.0).y;

    for plat in world.platforms() {
        if plat.id == depuis || !plat.has_face(face) {
            continue;
        }

        if (plat.rect.point_on(face, 0.0).y - hauteur).abs() > TOLERANCE {
            continue;
        }

        if vers_la_droite {
            if (plat.rect.left() - source.rect.right()).abs() <= TOLERANCE {
                return Some((plat.id, 0.0));
            }
        } else if (source.rect.left() - plat.rect.right()).abs() <= TOLERANCE {
            return Some((plat.id, plat.rect.face_length(face)));
        }
    }

    None
}
```

Mettre à jour l'unique appelant existant, dans `avancer` :

```rust
    if let Some((voisine, offset_entree)) = face_voisine(world, platform, face, vers_la_droite)
    {
        ch.attachment = Attachment::On {
            platform: voisine,
            face,
            offset: offset_entree,
        };
        return;
    }
```

> **Attention :** l'ancienne version passait `plat.rect.top()` et réattachait toujours à
> `Face::Top`. La nouvelle réattache à la **même face**, ce qui est ce qu'on veut au sol
> comme au plafond. Vérifier que le test existant
> `flaner_finit_par_faire_avancer_le_personnage` et ses voisins passent toujours.

- [ ] **Étape 5 : la phase `Plafond`**

Ajouter la variante :

```rust
    /// Se déplacer le long du plafond vers `cible`, un offset horizontal.
    ///
    /// Une phase distincte de `Paroi` et non un paramètre de face : les deux
    /// n'ont ni la même pose, ni le même axe, ni la même sortie. Les fondre
    /// demanderait un `match` sur la face dans chaque ligne du corps.
    Plafond { cible: f32 },
```

Dans `grimper`, ajouter le bras et étendre la sortie d'`Accroche` :

```rust
        // ── Traverser le plafond ────────────────────────────────────────
        PhaseGrimpe::Plafond { cible } => {
            let Attachment::On { platform, face, offset } = ch.attachment else {
                ch.intention = None;
                return Issue::Echouee;
            };
            let Some(plat) = world.get(platform) else {
                ch.intention = None;
                return Issue::Echouee;
            };

            ch.set_pose(POSE_CLIMB_CEILING, maintenant);

            let pas = reglages.vitesse_escalade * dt;
            let reste = cible - offset;

            // Au plafond, l'orientation suit le SENS DU DÉPLACEMENT, comme au
            // sol — et non « il regarde la surface », qui n'a pas de sens à
            // l'horizontale (design §3.4).
            ch.facing = if reste < 0.0 { Facing::Left } else { Facing::Right };

            if reste.abs() <= pas {
                phase = PhaseGrimpe::Accroche;
                let d = reglages.escalade.duree_accroche;
                jusqu_a = maintenant + Duration::from_secs_f32(rng.range(d[0], d[1]));
            } else {
                let nouveau = offset + pas * reste.signum();
                let longueur = plat.rect.face_length(face);

                // Au bout du plafond : le plafond du voisin le prolonge-t-il ?
                // C'est le MÊME mécanisme qu'au sol, et c'est pour cela que
                // `face_voisine` a été généralisée.
                if nouveau < 0.0 || nouveau > longueur {
                    match face_voisine(world, platform, face, reste > 0.0) {
                        Some((voisine, entree)) => {
                            ch.attachment = Attachment::On {
                                platform: voisine,
                                face,
                                offset: entree,
                            };
                            // La cible appartenait à l'ancien plafond : on
                            // s'arrête là et on s'accroche, plutôt que de
                            // traduire un offset d'une plateforme à l'autre.
                            phase = PhaseGrimpe::Accroche;
                            let d = reglages.escalade.duree_accroche;
                            jusqu_a = maintenant + Duration::from_secs_f32(rng.range(d[0], d[1]));
                        }
                        None => {
                            // Bout du monde : on s'accroche sur place.
                            ch.attachment = Attachment::On {
                                platform,
                                face,
                                offset: nouveau.clamp(0.0, longueur),
                            };
                            phase = PhaseGrimpe::Accroche;
                            let d = reglages.escalade.duree_accroche;
                            jusqu_a = maintenant + Duration::from_secs_f32(rng.range(d[0], d[1]));
                        }
                    }
                } else {
                    ch.attachment = Attachment::On { platform, face, offset: nouveau };
                }
            }
        }
```

Dans le bras `Accroche`, la pose et la sortie doivent distinguer le plafond du mur :

```rust
            // La pose dépend de la face, pas de la phase : accroché à un mur
            // ou suspendu au plafond, ce ne sont pas les mêmes frames.
            let pose = match ch.attachment {
                Attachment::On { face: Face::Bottom, .. } => POSE_GRAB_CEILING,
                _ => POSE_GRAB_WALL,
            };
            ch.set_pose(pose, maintenant);
```

et, après le tirage `lache` :

```rust
            // Troisième issue, réservée au HAUT d'un mur : passer au plafond.
            // Le test `offset <= pas_d_une_image` plutôt que `== 0.0` : on ne
            // compare jamais deux flottants pour l'égalité après une
            // accumulation.
            let en_haut = face != Face::Bottom && offset <= reglages.vitesse_escalade * dt;
            if en_haut && ch.manifest.has_pose(POSE_CLIMB_CEILING) {
                // Couverture partielle (spec §8.6) : un pack sans pose de
                // plafond grimpe quand même, il s'arrête simplement en haut
                // du mur. C'est exactement pourquoi `climbCeiling` n'est PAS
                // dans les `poses_requises` de `Grimper`.
                if let Some((plafond, entree)) = plafond_au_sommet(world, platform, plat, face) {
                    ch.attachment = Attachment::On {
                        platform: plafond,
                        face: Face::Bottom,
                        offset: entree,
                    };
                    let longueur = world
                        .get(plafond)
                        .map(|p| p.rect.face_length(Face::Bottom))
                        .unwrap_or(0.0);
                    phase = PhaseGrimpe::Plafond {
                        cible: rng.range(0.0, longueur),
                    };
                    ai.etat = EtatIntention::Grimpe { phase, jusqu_a };
                    return Issue::EnCours;
                }
            }
```

et la fonction d'aide :

```rust
/// Le plafond de l'écran de ce mur, et l'offset où y entrer.
///
/// L'offset d'entrée est l'abscisse du mur ramenée dans les bornes du
/// plafond : on arrive au plafond juste au-dessus de l'endroit où l'on
/// tenait la paroi.
fn plafond_au_sommet(
    world: &World,
    mur: PlatformId,
    plat_mur: &crate::world::Platform,
    face_mur: Face,
) -> Option<(PlatformId, f32)> {
    let x = plat_mur.rect.point_on(face_mur, 0.0).x;

    for plat in world.platforms() {
        if !plat.id.meme_ecran(mur) || !plat.has_face(Face::Bottom) {
            continue;
        }
        let offset = (x - plat.rect.left()).clamp(0.0, plat.rect.face_length(Face::Bottom));
        return Some((plat.id, offset));
    }

    None
}
```

- [ ] **Étape 6 : lancer les tests**

```powershell
cargo test intention::
cargo test
```

- [ ] **Étape 7 : le regarder**

```powershell
cargo build
cargo run
```

Clic droit → « Grimper au mur », plusieurs fois. Attendu : il lui arrive de monter
jusqu'en haut, de basculer au plafond, de le traverser, puis de se lâcher ou de repartir.

- [ ] **Étape 8 : commit**

```powershell
git add src-tauri/src/behavior/intention.rs src-tauri/src/character/manifest.rs
git commit -m "feat(comportement): le plafond — il bascule, traverse, et redescend

Phase Plafond, et face_voisine generalisee aux faces Bottom : le plafond
d'un ecran prolonge celui du voisin exactement comme le sol prolonge le
sol, par un seul chemin de code.

Au plafond l'orientation suit le sens du deplacement, comme au sol — et
non « il regarde la surface », qui n'a pas de sens a l'horizontale.

climbCeiling n'est volontairement PAS dans les poses requises de Grimper :
un pack qui ne l'a pas grimpe quand meme, il s'arrete en haut du mur
(couverture partielle, spec 8.6).

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Tâche 7 : L'ancre mesurée, la journée simulée, le CPU, la doc

**Fichiers :**
- Modifier : `characters/blob/mascot.json` (les ancres, **après mesure**)
- Modifier : `src-tauri/src/sim.rs` (la trace et l'invariant)
- Modifier : `CLAUDE.md` (la table des frames, l'état d'avancement)

**Interfaces :**
- Consomme : tout ce qui précède
- Produit : aucun type nouveau — une mesure, un invariant, et la documentation

- [ ] **Étape 1 : l'invariant du monde vertical dans la simulation**

Dans `src-tauri/src/sim.rs`, compter les phases et **vérifier à chaque image** qu'aucune
pose de sol ne s'affiche sur une face verticale :

```rust
        // ── L'invariant du monde vertical (étape 4a) ────────────────────
        //
        // Le filet de la règle de sécurité, vérifié 60 fois par seconde
        // simulée : si le personnage est accroché à une face verticale ou au
        // plafond, sa pose est forcément une pose d'escalade. C'est ce qui
        // attrape la régression « il marche sur un mur », qu'aucun test
        // unitaire ne verrait parce qu'elle demande la conjonction d'un
        // tirage et d'une transition.
        if let Attachment::On { face, .. } = ch.attachment {
            if face != Face::Top {
                let attendue = matches!(
                    ch.pose.as_str(),
                    POSE_GRAB_WALL | POSE_CLIMB_WALL | POSE_GRAB_CEILING | POSE_CLIMB_CEILING
                );
                assert!(
                    attendue,
                    "pose « {} » sur une face {:?} à t = {:?}",
                    ch.pose, face, t
                );
            }
        }
```

Ajouter au récapitulatif imprimé par `--sim` deux compteurs : le temps passé accroché, et
le nombre de fois qu'il a atteint le plafond.

- [ ] **Étape 2 : le test de la journée**

Étendre `sim::tests::une_journee_entiere_dort_au_bon_moment` **sans en ajouter une
seconde** — le test de 24 h coûte déjà ~7 s, et le lancer deux fois multiplierait ce coût
par deux pour rien :

```rust
        // Étape 4a : sur 24 h, il doit avoir grimpé au moins une fois, et
        // n'avoir jamais tenu une pose de sol sur une face verticale
        // (l'invariant, vérifié dans la boucle elle-même).
        assert!(
            resume.temps_accroche > Duration::ZERO,
            "en 24 h il n'a jamais grimpé une seule fois"
        );
```

- [ ] **Étape 3 : lancer la simulation**

```powershell
cargo test
cargo build
cargo run -- --sim 1440
```

Attendu : l'histogramme du sommeil est inchangé (l'escalade ne doit pas manger le repos
au point de le faire disparaître), et les deux nouveaux compteurs sont non nuls.

Si le sommeil s'est effondré, c'est que `envies.grimper = 1.0` est trop haut face à
`se_reposer = 1.0` : le corriger **dans la valeur par défaut de la config**, pas dans le
code de l'intention.

- [ ] **Étape 4 : mesurer l'ancre — la seule chose qui ne se décide pas**

```powershell
cargo build
cargo run
```

Clic droit → « Grimper au mur ». **Regarder** le personnage accroché à la paroi.

`Wall.java` place le mur exactement sur `workArea.left`, et l'ancre `64,128` de
`grabWall` tombe donc pile sur le bord : la moitié du sprite est hors écran. Si le rendu
ne convient pas, ajuster **dans `characters/blob/mascot.json`** — et nulle part ailleurs :

```json
    "grabWall":     { "frames": [13], "anchor": [<mesuré>, 128] },
    "climbWall":    { "frames": [14, 12, 13], "frameMs": 160, "loop": true, "anchor": [<mesuré>, 128] },
```

> ⚠️ **Ne jamais compenser dans `attach.rs`.** L'en-tête de `window_top_left` l'interdit
> explicitement : « si le personnage est mal posé, c'est l'ancre du manifeste qu'il faut
> corriger — c'est de la donnée, elle se règle sans recompiler ». Le prototype VSCode
> contenait un tel bricolage ; il ne revient pas.
>
> Consigner la valeur retenue et **la raison** dans le champ `_hitbox` ou un champ
> `_ancres` du `mascot.json`, comme l'a fait `luffy`.

Vérifier aussi le plafond : `grabCeiling` a l'ancre `64,48` (la tête en bas, les mains en
haut) — c'est la valeur de Shimeji-ee, mais elle n'a jamais été éprouvée chez nous.

- [ ] **Étape 5 : mesurer le CPU, correctement**

**La seule mesure comparable est celle du mode caché** (`CLAUDE.md`, quatrième
hypothèse) : la boucle tourne entièrement, mais la fenêtre n'est jamais déplacée, donc la
charge ne dépend pas de ce que le personnage est en train de faire.

```powershell
cargo build --release
$env:SHIMEJI_CACHE=1
Start-Process .\target\release\shimeji-desktop.exe
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU; Start-Sleep -Seconds 60; $p.Refresh()
"$([math]::Round((($p.CPU - $c) / 60) * 100, 1)) % d'un coeur"
```

Référence à battre : **0,9 %** (étapes 1b et 2). Un écart au-dessus de ~1,2 % demande une
explication avant de commiter.

> Ne **pas** conclure depuis une mesure « en marche » : l'escalade change la charge de
> travail — un personnage qui grimpe déplace sa fenêtre à 16 px/s au lieu de 50, donc
> beaucoup moins souvent. Le chiffre baisserait sans rien prouver. C'est exactement la
> quatrième hypothèse de `CLAUDE.md`, et elle a déjà trompé une fois.

- [ ] **Étape 6 : la documentation**

Dans `CLAUDE.md` :

1. **Corriger la table des frames de `blob`** — la ligne « 23, 24, 25 | agripper une
   paroi verticale (**escalade**) » est fausse :

```markdown
| 12, 13, 14 | agripper et escalader une **paroi verticale** |
| 23, 24, 25 | se suspendre et se déplacer au **plafond** |
| 34, 35, 36 | se hisser par-dessus un bord |
```

2. **Mettre à jour « L'ordre de construction »** : marquer l'étape 4a faite, et préciser
   que l'étape 4 restante ne porte plus que sur les **fenêtres**.
3. **Mettre à jour « État actuel »** avec le nombre de tests, la taille de l'exe et le
   chiffre CPU en mode caché mesuré à l'étape 5.
4. **Ajouter une ligne au tableau des documents** pour le design et le plan de l'étape 4a.
5. **Mettre à jour « La prochaine action »** : l'étape 3 (un deuxième personnage) est mise
   de côté ; la suite est l'étape 4 complète (les fenêtres) puis l'étape 5.

- [ ] **Étape 7 : commit**

```powershell
git add CLAUDE.md characters/blob/mascot.json src-tauri/src/sim.rs
git commit -m "docs(etape-4a): l'ancre mesuree, l'invariant du monde vertical, l'etat

L'ancre de grabWall et climbWall est MESUREE, pas recopiee : Wall.java
pose le mur exactement sur workArea.left, donc l'ancre 64,128 de
Shimeji-ee laisse la moitie du sprite hors ecran. Corrigee dans le
mascot.json, jamais dans attach.rs.

--sim verifie desormais a chaque image qu'une pose de sol ne s'affiche
jamais sur une face verticale : c'est le filet qui attrape « il marche
sur un mur », qu'aucun test unitaire ne verrait.

CLAUDE.md : la table des frames attribuait 23/24/25 a la paroi verticale,
ce sont celles du plafond ; le mur c'est 12/13/14.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Auto-relecture du plan

**Couverture du design, section par section :**

| Section du design | Tâche |
|---|---|
| §2 le monde, §2.1 rectangles fins, §2.2 identités, §2.3 bords partagés, §2.4 gratuit | 1 |
| §3.1 `Attachment` inchangé | vérifié par l'absence de `attach.rs` dans les fichiers modifiés |
| §3.2 `contact` et ses trois règles | 2 |
| §3.3 aucun nouveau réflexe, §3.4 orientation, §3.5 se lâcher | 5 |
| §4.1 une intention, §4.2 transitions de coin, §4.3 sortie tirée | 4 et 6 |
| §4.4 délai 120 s | 4 |
| §4.5 fini sur un mur ⇒ il lâche | 3 |
| §4.6 desire, config, menu, sim | 4 (les trois premiers) et 7 (sim) |
| §4.7 écarté : la flânerie qui grimpe | rien à faire — `flaner` n'est pas touché |
| §5 constantes et leur source | 4 |
| §6 l'ancre mesurée | 7 |
| §7 vérification sans écran | 1, 2, 6 (tests) et 7 (`--sim`, CPU) |

**Cohérence des types**, vérifiée d'un bout à l'autre : `contact` rend
`Option<(PlatformId, Face, f32)>` en Tâche 2 et est consommée sous cette forme en
Tâche 5 ; `PhaseGrimpe` a quatre variantes en Tâche 4 et en gagne une cinquième
(`Plafond`) en Tâche 6, la Tâche 6 étant seule à l'ajouter ; `delai_abandon(Intention)`
est définie et testée en Tâche 4 ; `POSE_GRAB_WALL` / `POSE_CLIMB_WALL` naissent en
Tâche 4 et `POSE_GRAB_CEILING` / `POSE_CLIMB_CEILING` en Tâche 6, chacune avant son
premier usage.

**Deux maladresses signalées dans le corps du plan plutôt que corrigées en silence**, pour
que l'exécutant les voie : le double paramètre `World` de `mur_le_plus_proche` (Tâche 4,
étape 7), et le test `grimper_echoue_immediatement_sans_mur` dont la première construction
de monde est inutile et doit être supprimée en écrivant.
