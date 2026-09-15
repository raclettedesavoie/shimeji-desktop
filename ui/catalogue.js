// La fenêtre du catalogue (spec §9).
//
// Deux écrans, parce que DEUX GESTES : le catalogue installe, la
// bibliothèque choisit qui s'affiche. Personne ne doit pouvoir confondre
// « je le télécharge » et « je l'affiche ».
//
// ⚠️ Cette page est la SEULE du projet à utiliser l'IPC de Tauri, et la
// seule couverte par `capabilities/catalogue.json`. Le personnage, lui,
// reçoit tout par `eval` et n'a besoin d'aucune permission.

const CDN = 'https://sprites.shimejis.xyz/directory';

// À partir de combien d'exemplaires on avertit. **La même valeur que
// `commandes::SEUIL_AVERTISSEMENT` côté Rust** — les deux doivent dire la
// même chose, la ligne imprimée sur la sortie standard étant l'équivalent
// scriptable du bandeau (design §2).
const SEUIL_AVERTISSEMENT = 10;

const contenu = document.getElementById('contenu');
const etat = document.getElementById('etat');
const recherche = document.getElementById('recherche');
const ongletCatalogue = document.getElementById('onglet-catalogue');
const ongletBiblio = document.getElementById('onglet-biblio');

let packs = [];        // l'index, lu une fois
let installes = new Set();
let ecran = 'catalogue';

function dire(message) {
  etat.textContent = message || '';
}

// ── Le catalogue ────────────────────────────────────────────────────────
//
// Les vignettes SONT les shime1.png, tirées du CDN. La CSP est nulle, rien
// ne s'y oppose (spec §3).
//
// `loading="lazy"` : WebView2 est Chromium, il ne charge que ce qui approche
// du viewport. Zéro ligne de défilement virtuel à écrire (spec §9).
function carteCatalogue(pack) {
  const bouton = document.createElement('button');
  bouton.className = 'carte';

  const img = document.createElement('img');
  img.loading = 'lazy';
  img.src = `${CDN}/${pack.slug}/img/shime1.png`;
  img.alt = '';

  const nom = document.createElement('span');
  const deja = installes.has(pack.slug);
  nom.textContent = deja ? `✓ ${pack.nom}` : pack.nom;
  if (deja) {
    nom.className = 'pastille';
    bouton.disabled = true;
  }

  bouton.append(img, nom);
  bouton.addEventListener('click', () => installer(pack));
  return bouton;
}

async function installer(pack) {
  dire(`installation de ${pack.nom}…`);
  try {
    await window.__TAURI__.core.invoke('installer', { slug: pack.slug });
    installes.add(pack.slug);
    dire(`${pack.nom} installé.`);
    rendre();
  } catch (e) {
    // Bruyant : une installation silencieusement ratée laisserait croire
    // que le clic n'a rien fait.
    dire(`échec : ${e}`);
  }
}

// ── La bibliothèque : le SECOND geste, devenu un compteur ───────────────
//
// « Choisir » ne REMPLACE plus : il ajoute et il retire. Une seule commande
// sert les quatre gestes (design §4) :
//
//   · le fond de la carte  → definirCompte(n + 1)
//   · le bouton −          → definirCompte(n − 1)
//   · le bouton +          → definirCompte(n + 1)
//   · l'interrupteur       → definirCompte(0) ou definirCompte(1)
//
// L'interrupteur n'est que le REFLET de `compte > 0`. Éteindre trois blob
// puis rallumer en ramène UN : il n'y a aucun compte « en sommeil » stocké
// quelque part, donc aucune seconde vérité à tenir d'accord avec la liste.
function carteBibliotheque(pack) {
  const carte = document.createElement('div');
  carte.className = 'carte carte-biblio';

  // Le fond de la carte ajoute un exemplaire. Un `div` et non un `button` :
  // il contient maintenant d'autres boutons, et un bouton dans un bouton est
  // du HTML invalide que les navigateurs réparent chacun à leur façon.
  carte.addEventListener('click', () => definirCompte(pack, pack.compte + 1));
  carte.title = `Cliquer pour ajouter un ${pack.nom}`;

  const img = document.createElement('img');
  img.loading = 'lazy';
  // Le schéma servi par Rust : les images sont des fichiers EXTERNES au
  // binaire, aucun chemin relatif ne peut les atteindre.
  img.src = `http://shime.localhost/${pack.nom}/1`;
  img.alt = '';

  const nom = document.createElement('span');
  nom.textContent = pack.nom;
  if (pack.compte > 0) nom.className = 'pastille';

  // ── La rangée du bas : − compteur + · interrupteur ────────────────────
  const rangee = document.createElement('div');
  rangee.className = 'rangee';

  // `stopPropagation` sur CHAQUE bouton : sans lui, le clic remonterait au
  // fond de la carte et ajouterait AUSSI un exemplaire. C'est le défaut le
  // plus facile à introduire ici, et le plus déroutant à l'usage — « je
  // clique sur moins et il en arrive un ».
  const moins = bouton('−', 'Retirer un exemplaire', (e) => {
    e.stopPropagation();
    definirCompte(pack, Math.max(0, pack.compte - 1));
  });
  // Rien à retirer : le bouton existe mais n'agit pas, plutôt que
  // d'apparaître et disparaître au fil des clics.
  moins.disabled = pack.compte === 0;

  const compteur = document.createElement('span');
  compteur.className = 'compteur';
  // N'afficher le nombre que s'il y en a : une bibliothèque de 40 packs dont
  // 2 sont actifs ne doit pas être un mur de zéros.
  compteur.textContent = pack.compte > 0 ? String(pack.compte) : '';

  const plus = bouton('+', 'Ajouter un exemplaire', (e) => {
    e.stopPropagation();
    definirCompte(pack, pack.compte + 1);
  });

  const interrupteur = document.createElement('button');
  interrupteur.className = 'interrupteur' + (pack.compte > 0 ? ' allume' : '');
  interrupteur.setAttribute('role', 'switch');
  interrupteur.setAttribute('aria-checked', pack.compte > 0 ? 'true' : 'false');
  interrupteur.title = pack.compte > 0
    ? 'Retirer tous les exemplaires'
    : 'Faire apparaître ce personnage';
  interrupteur.addEventListener('click', (e) => {
    e.stopPropagation();
    definirCompte(pack, pack.compte > 0 ? 0 : 1);
  });

  rangee.append(moins, compteur, plus, interrupteur);

  // ── La poubelle ───────────────────────────────────────────────────────
  //
  // Désactivée pour un pack livré avec l'application : la règle est générale
  // — supprimable si et seulement si le dossier résout dans la bibliothèque
  // %APPDATA% — et pas un cas particulier nommé « blob » (design §7).
  const poubelle = bouton('🗑', '', (e) => {
    e.stopPropagation();
    supprimer(pack);
  });
  poubelle.className = 'poubelle';
  poubelle.disabled = !pack.supprimable;
  poubelle.title = pack.supprimable
    ? `Supprimer « ${pack.nom} » du disque, définitivement`
    : `« ${pack.nom} » est livré avec l'application : il ne peut pas être supprimé`;

  carte.append(poubelle, img, nom, rangee);
  return carte;
}

// Un bouton de carte. Trois lignes recopiées quatre fois valent une
// fonction — et surtout, le `type="button"` oublié une fois sur quatre
// soumettrait un formulaire qui n'existe pas.
function bouton(texte, titre, action) {
  const b = document.createElement('button');
  b.type = 'button';
  b.textContent = texte;
  if (titre) b.title = titre;
  b.addEventListener('click', action);
  return b;
}

// Le SEUL appel qui change ce qui vit à l'écran. Les quatre gestes y
// arrivent tous, avec un nombre différent.
async function definirCompte(pack, combien) {
  try {
    await window.__TAURI__.core.invoke('definir_compte', {
      nom: pack.nom,
      combien,
    });
    rendre();
  } catch (e) {
    // Bruyant : un clic silencieusement raté laisserait croire que
    // l'interface est cassée.
    dire(`échec : ${e}`);
  }
}

// La suppression est DÉFINITIVE et sans corbeille : c'est le seul geste de
// toute l'application qui détruise quelque chose, et le seul qui demande
// confirmation.
async function supprimer(pack) {
  if (!confirm(`Supprimer définitivement « ${pack.nom} » du disque ?\n\nCette action est irréversible.`)) {
    return;
  }
  dire(`suppression de ${pack.nom}…`);
  try {
    await window.__TAURI__.core.invoke('supprimer', { nom: pack.nom });
    // La grille du catalogue doit oublier qu'il était installé, sinon sa
    // carte resterait grisée avec sa pastille.
    installes.delete(pack.nom);
    dire(`${pack.nom} supprimé.`);
    rendre();
  } catch (e) {
    // Bruyant, et c'est essentiel : une suppression silencieusement ratée
    // laisserait croire que le pack est parti alors qu'il reviendra au
    // prochain démarrage.
    dire(`échec : ${e}`);
  }
}

// ── Le rendu ────────────────────────────────────────────────────────────
async function rendre() {
  contenu.textContent = '';
  const filtre = recherche.value.trim().toLowerCase();

  if (ecran === 'biblio') {
    const liste = await window.__TAURI__.core.invoke('bibliotheque');

    // Le catalogue doit connaître les packs installés pour les griser :
    // on profite de cet appel plutôt que d'en faire un second.
    installes = new Set(liste.map((p) => p.nom));

    // ── L'avertissement à 10 (design §2) ──────────────────────────────
    //
    // ⚠️ Il AVERTIT, il n'interdit pas : aucun bouton désactivé, aucune
    // confirmation, aucun plafond. C'est un panneau, pas une barrière —
    // décision de l'auteur, prise en connaissance de la mesure.
    //
    // Le total compte les EXEMPLAIRES et non les packs : dix blob coûtent
    // exactement ce que coûtent dix personnages différents, puisque le CPU
    // est proportionnel au nombre de fenêtres déplacées.
    const total = liste.reduce((n, p) => n + p.compte, 0);
    if (total >= SEUIL_AVERTISSEMENT) {
      const bandeau = document.createElement('p');
      bandeau.className = 'avertissement';
      bandeau.textContent =
        `⚠️ ${total} personnages à l'écran. Chacun qui marche consomme du ` +
        `processeur ; à ce nombre, la consommation peut devenir notable.`;
      contenu.append(bandeau);
    }

    const grille = document.createElement('div');
    grille.className = 'grille';
    for (const p of liste) {
      if (!filtre || p.nom.toLowerCase().includes(filtre)) {
        grille.append(carteBibliotheque(p));
      }
    }
    contenu.append(grille);
    return;
  }

  // Groupés par franchise, alphabétiquement (spec §9).
  const parFranchise = new Map();
  for (const p of packs) {
    if (filtre && !p.nom.toLowerCase().includes(filtre)
        && !p.franchise.toLowerCase().includes(filtre)) {
      continue;
    }
    if (!parFranchise.has(p.franchise)) parFranchise.set(p.franchise, []);
    parFranchise.get(p.franchise).push(p);
  }

  for (const franchise of [...parFranchise.keys()].sort()) {
    const titre = document.createElement('h2');
    titre.textContent = franchise;
    const grille = document.createElement('div');
    grille.className = 'grille';
    for (const p of parFranchise.get(franchise)) {
      grille.append(carteCatalogue(p));
    }
    contenu.append(titre, grille);
  }
}

// ── Démarrage ───────────────────────────────────────────────────────────
ongletCatalogue.addEventListener('click', () => {
  ecran = 'catalogue';
  ongletCatalogue.setAttribute('aria-selected', 'true');
  ongletBiblio.setAttribute('aria-selected', 'false');
  rendre();
});
ongletBiblio.addEventListener('click', () => {
  ecran = 'biblio';
  ongletCatalogue.setAttribute('aria-selected', 'false');
  ongletBiblio.setAttribute('aria-selected', 'true');
  rendre();
});
recherche.addEventListener('input', rendre);

// L'index est un fichier statique servi avec le reste du front : un `fetch`
// local suffit. Le faire transiter par l'IPC serait 463 Ko de données
// statiques passées par un pont fait pour des messages (spec §9).
(async function demarrer() {
  try {
    const reponse = await fetch('catalogue.json');
    const index = await reponse.json();
    packs = index.packs;
  } catch (e) {
    dire(`catalogue.json illisible : ${e}`);
    return;
  }

  // Les packs déjà sur le disque : le catalogue grise leurs cartes.
  // `rendre()` le recalcule à chaque passage sur « Ma bibliothèque », ce qui
  // suffit ensuite — celui-ci n'est là que pour le tout premier affichage,
  // qui est celui du catalogue.
  const liste = await window.__TAURI__.core.invoke('bibliotheque');
  installes = new Set(liste.map((p) => p.nom));

  window.__TAURI__.event.listen('installation', (evenement) => {
    const { slug, fait, total } = evenement.payload;
    dire(`${slug} : ${fait}/${total}`);
  });

  dire(`${packs.length} personnages disponibles.`);
  rendre();
})();
