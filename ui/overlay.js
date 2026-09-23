// Afficheur de l'overlay : N sprites dans la fenêtre d'un écran.
//
// Il ne calcule RIEN — ni trajectoire, ni décision, et il ne renvoie jamais
// rien à Rust. Il fait deux choses :
//   1. placer chaque sprite là où Rust le dit (15 fois par seconde) ;
//   2. GLISSER entre deux positions reçues, pour dessiner à 60 images/s.
//
// Le point 2 est du LISSAGE, pas de la logique (conception §2.2) : le facteur
// d'interpolation est borné à 1, donc on n'invente jamais une position que
// Rust n'a pas calculée. Si l'envoi suivant tarde, le sprite s'arrête sur la
// dernière position connue au lieu de la dépasser.
//
// ⚠️ Le format d'un sprite — [id, x, y, w, h, image, flip] — est le MÊME que
// celui que fabrique `overlay::js_de` côté Rust. Les deux se corrompent en
// silence s'ils divergent : un `w` lu comme un `y` ne produit aucune erreur,
// juste un sprite au mauvais endroit. Les modifier ensemble, toujours.

// ── La table des personnages ────────────────────────────────────────────
//
// `id -> nom de pack`, publiée par Rust à chaque changement de roster.
//
// Elle ne peut PAS venir du fragment d'URL comme dans l'ancien `pet.js` :
// une fenêtre d'écran porte des personnages de packs différents. C'est la
// seule information non numérique qui traverse, et elle ne change qu'au
// changement de roster — jamais dans la boucle.
let packs = {};

// Version du contenu, changée à chaque rechargement à chaud. Les images sont
// servies avec `Cache-Control: max-age=3600` : sans ce paramètre, une image
// modifiée sur le disque ne serait jamais relue. On change donc l'URL plutôt
// que le cache.
let version = 0;

// Les URL déjà construites, pour ne pas reformer la même chaîne à chaque
// changement d'image.
const urls = new Map();

function urlDe(id, image) {
  const cle = version + '/' + id + '/' + image;
  let u = urls.get(cle);
  if (u === undefined) {
    // Les images viennent d'un schéma URI servi par Rust : les PNG sont des
    // fichiers EXTERNES au binaire (spec §8.1), donc aucun chemin relatif ne
    // peut les atteindre. Sur Windows, Tauri sert les schémas custom sous
    // http://<scheme>.localhost.
    //
    // Le gestionnaire lit `uri().path()`, qui IGNORE la requête : `?v=3` ne
    // change donc rien côté Rust, seulement la clé de cache du webview.
    u = 'http://shime.localhost/' + packs[id] + '/' + image + '?v=' + version;
    urls.set(cle, u);
  }
  return u;
}

// ── Les sprites vivants ─────────────────────────────────────────────────
//
// `id -> { el, dx, dy, ax, ay, t, w, h, image, flip, px, py, … }`
//   d*  départ du segment courant     a*  arrivée      t  instant de départ
//   p*  ce qui est réellement POSÉ dans le DOM, pour ne rien réécrire
//       d'identique (conception §5.3)
const sprites = new Map();

// L'intervalle entre deux envois, MESURÉ et non supposé : Rust vise 15 Hz et
// dérive. Une durée codée en dur ferait arriver l'interpolation trop tôt
// (saccade) ou trop tard (glissement). 66 ms est la valeur de départ,
// remplacée dès le deuxième envoi.
let intervalle = 66;
let dernierEnvoi = 0;

function spriteDe(id) {
  let s = sprites.get(id);
  if (s === undefined) {
    const el = document.createElement('img');
    el.className = 'perso';
    document.body.appendChild(el);
    // `px/py/…` à `null` et non à 0 : 0 est une position valide, et le
    // premier dessin doit s'écrire même si le sprite est en (0, 0).
    s = {
      el: el,
      px: null, py: null, pimage: null, pflip: null, pw: null, ph: null
    };
    sprites.set(id, s);
  }
  return s;
}

function retirer(s, id) {
  s.el.remove();
  sprites.delete(id);
}

// ── Ce que Rust appelle ─────────────────────────────────────────────────

// Publie la table `id -> pack`. Appelée à chaque changement de roster, et une
// fois juste après la création de la fenêtre.
//
// Les sprites dont l'id a disparu sont retirés du DOM : sans ça, un
// personnage supprimé resterait affiché pour toujours, figé sur sa dernière
// position — et aucune erreur ne le signalerait.
window.declarer = function (table, v) {
  packs = table;
  version = v;
  // Les URL portent la version ET le pack : les deux ayant pu changer, on
  // repart de zéro plutôt que d'invalider entrée par entrée.
  urls.clear();

  sprites.forEach(function (s, id) {
    if (!(id in packs)) {
      retirer(s, id);
    } else {
      // Le pack de cet id a pu changer (rechargement à chaud) : on force la
      // réécriture du `src` au prochain dessin. Sans ça on garderait un
      // personnage parfaitement animé… avec le mauvais dessin, exactement le
      // bug décrit dans `main.rs` à propos du fragment `#blob`.
      s.pimage = null;
    }
  });
};

// Reçoit toute la charge de CET écran, environ 15 fois par seconde.
window.poserTous = function (liste) {
  const maintenant = performance.now();
  if (dernierEnvoi !== 0) {
    intervalle = maintenant - dernierEnvoi;
  }
  dernierEnvoi = maintenant;

  // Qui a été vu dans cet envoi. Un sprite absent a quitté l'écran — il est
  // passé sur le voisin, ou il est parti.
  const vus = new Set();

  for (let i = 0; i < liste.length; i++) {
    const l = liste[i];
    const id = l[0];
    vus.add(id);
    const s = spriteDe(id);

    if (s.ax === undefined) {
      // Première position connue : on part d'elle. Sinon le sprite
      // traverserait l'écran en glissant depuis (0, 0) à son apparition.
      s.dx = l[1];
      s.dy = l[2];
    } else {
      // Le départ du nouveau segment est l'ARRIVÉE du précédent, jamais la
      // position dessinée : partir du dessin accumulerait l'erreur d'arrondi
      // de chaque image.
      s.dx = s.ax;
      s.dy = s.ay;
    }
    s.ax = l[1];
    s.ay = l[2];
    s.w = l[3];
    s.h = l[4];
    s.image = l[5];
    s.flip = l[6];
    s.t = maintenant;
  }

  sprites.forEach(function (s, id) {
    if (!vus.has(id)) {
      retirer(s, id);
    }
  });

  // La boucle de dessin s'était peut-être endormie faute de sprite (voir
  // `dessiner`). Des personnages viennent d'arriver : on la réveille.
  if (sprites.size > 0 && enPause) {
    enPause = false;
    requestAnimationFrame(dessiner);
  }
};

// ── La boucle de dessin, à la cadence de l'écran ────────────────────────

// Vrai quand la boucle de dessin est suspendue, faute de sprite.
let enPause = false;

function dessiner() {
  // ⚠️ **Plus aucun sprite : on s'endort au lieu de tourner à vide.**
  //
  // Depuis le 2026-09-23, la fenêtre d'un écran reste ouverte même quand
  // personne n'y est — la fermer puis la rouvrir faisait geler le thread
  // principal un instant. Une fenêtre vide doit alors coûter le moins
  // possible : une boucle `requestAnimationFrame` qui tourne à 60 Hz pour ne
  // rien dessiner garderait le compositeur éveillé pour rien.
  //
  // C'est `poserTous` qui la réveille, dès qu'un sprite arrive.
  if (sprites.size === 0) {
    enPause = true;
    return;
  }

  const maintenant = performance.now();

  sprites.forEach(function (s, id) {
    // Déclaré mais pas encore positionné : rien à dessiner.
    if (s.ax === undefined) {
      return;
    }

    // `alpha` va de 0 (on vient de recevoir) à 1 (on a rejoint la cible).
    // Borné à 1 : on ne DEVINE jamais une position que Rust n'a pas calculée
    // (conception §2.2). Extrapoler donnerait un sprite parfois faux, et
    // ferait de ce fichier une seconde source de vérité.
    let alpha = (maintenant - s.t) / intervalle;
    if (alpha > 1) {
      alpha = 1;
    }

    // Arrondi : le pixel-art doit tomber sur des pixels entiers, sinon le
    // compositeur le lisse et la netteté promise par `image-rendering:
    // pixelated` est perdue.
    const x = Math.round(s.dx + (s.ax - s.dx) * alpha);
    const y = Math.round(s.dy + (s.ay - s.dy) * alpha);

    // ⚠️ **Ne rien écrire quand rien n'a changé** (conception §5.3). Le péage
    // de ~34 % par fenêtre est payé quand le CONTENU change : un personnage
    // endormi doit être gratuit. Mesuré au spike : 24 % contre 101 %.
    if (s.px !== x || s.py !== y || s.pflip !== s.flip) {
      s.px = x;
      s.py = y;
      s.pflip = s.flip;
      // Miroir horizontal : l'orientation n'a pas de frames dédiées
      // (spec §8.5).
      s.el.style.transform =
        'translate(' + x + 'px,' + y + 'px)' + (s.flip ? ' scaleX(-1)' : '');
    }

    if (s.pimage !== s.image) {
      s.pimage = s.image;
      s.el.src = urlDe(id, s.image);
    }

    // La taille dépend du manifeste, de l'échelle de l'écran et de l'IMAGE
    // affichée — les frames d'un pack tiers n'ayant pas toutes la même
    // taille. Elle change donc parfois en cours d'animation.
    if (s.pw !== s.w || s.ph !== s.h) {
      s.pw = s.w;
      s.ph = s.h;
      s.el.style.width = s.w + 'px';
      s.el.style.height = s.h + 'px';
    }
  });

  requestAnimationFrame(dessiner);
}

requestAnimationFrame(dessiner);
