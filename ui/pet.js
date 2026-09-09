// Afficheur de sprite. Délibérément bête : il reçoit { image, flip } et
// n'en fait rien d'autre que l'afficher (spec §3.1).
//
// Toute la logique — physique, comportement, position de la fenêtre — vit en
// Rust. Faire calculer quoi que ce soit ici coûterait un aller-retour IPC
// 60 fois par seconde et par personnage.

const pet = document.getElementById('pet');

// Le personnage à afficher est passé dans le fragment de l'URL par Rust, à
// la création de la fenêtre : #blob. Un fragment plutôt qu'un paramètre de
// requête, pour ne pas interférer avec la résolution du fichier.
const personnage = window.location.hash.slice(1) || 'blob';

// Les images viennent d'un schéma URI servi par Rust : les PNG sont des
// fichiers EXTERNES au binaire (spec §8.1), donc aucun chemin relatif ne
// peut les atteindre. Sur Windows, Tauri sert les schémas custom sous
// http://<scheme>.localhost.
const BASE = `http://shime.localhost/${personnage}/`;

// On précharge chaque image à sa première utilisation : sans ça, la
// première apparition d'une pose clignote le temps du chargement. Les
// images restent ensuite dans le cache du webview.
const cache = new Map();
function urlDe(n) {
  if (!cache.has(n)) {
    const img = new Image();
    img.src = BASE + n;
    cache.set(n, img.src);
  }
  return cache.get(n);
}

let derniere = null;

function poser(image, flip) {
  // Ne toucher au DOM que si quelque chose a changé. Rust n'émet déjà que
  // sur changement, mais il pousse aussi sans condition pendant la première
  // seconde (course au démarrage) : ces deux comparaisons évitent alors
  // 60 écritures inutiles par seconde.
  if (derniere === null || derniere.image !== image) {
    pet.src = urlDe(image);
  }
  if (derniere === null || derniere.flip !== flip) {
    // Miroir horizontal : l'orientation n'a pas de frames dédiées
    // (spec §8.5).
    pet.style.transform = flip ? 'scaleX(-1)' : 'none';
  }
  derniere = { image, flip };
}

// Exposée pour que Rust puisse l'appeler par `eval` — c'est la voie de
// secours si le pont d'événements ne fonctionne pas.
window.poser = poser;

// ── Affichage immédiat, sans attendre Rust ──────────────────────────────
//
// Deux raisons, et la seconde est un outil de diagnostic :
//
// 1. Il y a une COURSE AU DÉMARRAGE : tout message émis par Rust avant que
//    l'écouteur ci-dessous n'existe est perdu. Si le personnage était
//    immobile pendant ses premières secondes, rien ne s'afficherait.
// 2. Cet appel ne dépend QUE du schéma URI, pas du pont d'événements. Si
//    la silhouette apparaît mais reste figée, le schéma marche et c'est le
//    pont qui est muet. Si rien n'apparaît, c'est le schéma.
poser(1, false);

// `window.__TAURI__` est l'API PUBLIQUE de Tauri, disponible parce que
// `withGlobalTauri` est activé dans tauri.conf.json.
//
// On a écarté `window.__TAURI_INTERNALS__.invoke('plugin:event|listen', …)`,
// qui évite l'injection du script d'API : c'est une interface INTERNE, dont
// le nom peut changer d'une version de Tauri à l'autre.
//
// `if` de garde : si l'API n'est pas là, on ne veut pas d'une exception qui
// interromprait le script — l'affichage initial ci-dessus a déjà eu lieu, et
// Rust peut encore passer par `eval`.
if (window.__TAURI__ && window.__TAURI__.event) {
  window.__TAURI__.event.listen('frame', (event) => {
    poser(event.payload.image, event.payload.flip);
  });
} else {
  console.error('API Tauri absente : withGlobalTauri est-il activé ?');
}
