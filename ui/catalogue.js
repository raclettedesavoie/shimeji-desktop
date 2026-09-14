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

// ── La bibliothèque : le SECOND geste ───────────────────────────────────
function carteBibliotheque(pack) {
  const bouton = document.createElement('button');
  bouton.className = 'carte';

  const img = document.createElement('img');
  img.loading = 'lazy';
  // Le schéma servi par Rust : les images sont des fichiers EXTERNES au
  // binaire, aucun chemin relatif ne peut les atteindre.
  img.src = `http://shime.localhost/${pack.nom}/1`;
  img.alt = '';

  const nom = document.createElement('span');
  nom.textContent = pack.actif ? `● ${pack.nom}` : pack.nom;
  if (pack.actif) nom.className = 'pastille';

  bouton.append(img, nom);
  bouton.addEventListener('click', async () => {
    dire(`affichage de ${pack.nom}…`);
    try {
      await window.__TAURI__.core.invoke('choisir', { nom: pack.nom });
      dire(`${pack.nom} s'affiche.`);
      rendre();
    } catch (e) {
      dire(`échec : ${e}`);
    }
  });
  return bouton;
}

// ── Le rendu ────────────────────────────────────────────────────────────
async function rendre() {
  contenu.textContent = '';
  const filtre = recherche.value.trim().toLowerCase();

  if (ecran === 'biblio') {
    const liste = await window.__TAURI__.core.invoke('bibliotheque');
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

  const liste = await window.__TAURI__.core.invoke('bibliotheque');
  installes = new Set(liste.map((p) => p.nom));

  window.__TAURI__.event.listen('installation', (evenement) => {
    const { slug, fait, total } = evenement.payload;
    dire(`${slug} : ${fait}/${total}`);
  });

  dire(`${packs.length} personnages disponibles.`);
  rendre();
})();
