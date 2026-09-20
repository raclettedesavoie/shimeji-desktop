// L'assistant de première configuration (spec §3).
//
// Délibérément bête, comme `pet.js` : il navigue entre trois sections et fait
// deux appels. Toute la logique — registre, configuration, écran, toast — est
// en Rust, dans `commandes::onboarding_terminer`.
//
// `window.__TAURI__` est disponible parce que `withGlobalTauri` est à `true`
// dans `tauri.conf.json` : aucun import, aucun bundler.

const invoke = window.__TAURI__.core.invoke;

const ecrans = [
  document.getElementById("ecran-1"),
  document.getElementById("ecran-2"),
  document.getElementById("ecran-3"),
];
const precedent = document.getElementById("precedent");
const suivant = document.getElementById("suivant");
const erreur = document.getElementById("erreur");
const caseDemarrage = document.getElementById("demarrage-auto");

let courant = 0;

function afficher() {
  ecrans.forEach((s, i) => { s.hidden = i !== courant; });
  precedent.hidden = courant === 0;
  suivant.textContent = courant === ecrans.length - 1 ? "Terminer" : "Suivant";
}

// L'état initial de la case vient du REGISTRE, pas de la configuration :
// quelqu'un qui réinstalle par-dessus une version où il avait activé le
// démarrage automatique doit retrouver sa case cochée.
//
// Tauri sérialise les champs en camelCase, donc `demarrage_auto` se lit
// `demarrageAuto` ici. Une faute donnerait `undefined` — silencieusement.
invoke("onboarding_etat")
  .then((etat) => { caseDemarrage.checked = etat.demarrageAuto; })
  .catch((e) => { console.error("onboarding_etat :", e); });

precedent.addEventListener("click", () => {
  if (courant > 0) { courant -= 1; afficher(); }
});

suivant.addEventListener("click", () => {
  if (courant < ecrans.length - 1) { courant += 1; afficher(); return; }

  // Dernier écran : on valide. Le bouton est désactivé pendant l'appel — un
  // double clic lancerait deux fois l'écriture et deux toasts.
  suivant.disabled = true;
  erreur.textContent = "";

  const ecranChoisi = document.querySelector('input[name="ecran"]:checked').value;

  invoke("onboarding_terminer", {
    demarrageAuto: caseDemarrage.checked,
    ecran: ecranChoisi,
  }).catch((e) => {
    // La fenêtre est normalement déjà fermée quand une erreur arrive : le
    // seul cas où elle ne l'est pas est un échec d'écriture de la
    // configuration, qui laisse tout ouvert.
    //
    // On DÉCOCHE la case si c'est le registre qui a refusé — afficher une
    // case cochée alors que rien n'a été écrit serait un mensonge, et c'est
    // exactement ce que l'entrée du tray s'interdit déjà.
    caseDemarrage.checked = false;
    erreur.textContent = String(e);
    suivant.disabled = false;
    courant = 1;
    afficher();
  });
});

afficher();
