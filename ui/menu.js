// Le menu du clic droit : dessine les lignes reçues de Rust, se mesure, et
// renvoie le choix. Aucune décision ici (spec « menu sur mesure » §4).

const invoke = window.__TAURI__.core.invoke;
const menu = document.getElementById('menu');

// Appelée par Rust (menu_fenetre::ouvrir) avec { lignes, hauteurMax }.
window.afficherMenu = function (charge) {
  menu.replaceChildren();
  menu.style.maxHeight = charge.hauteurMax + 'px';

  for (const ligne of charge.lignes) {
    if (ligne.type === 'Separateur') {
      menu.appendChild(document.createElement('hr'));
    } else if (ligne.type === 'Titre') {
      const t = document.createElement('div');
      t.className = 'titre';
      t.textContent = ligne.texte;
      menu.appendChild(t);
    } else {
      const b = document.createElement('button');
      b.className = ligne.coche ? 'entree coche' : 'entree';
      b.textContent = ligne.libelle;
      b.addEventListener('click', () => invoke('choisir_entree_menu', { id: ligne.id }));
      menu.appendChild(b);
    }
  }
  menu.scrollTop = 0;

  // getBoundingClientRect force la mise en page : la mesure est juste même
  // dans une fenêtre encore cachée, où requestAnimationFrame peut ne jamais
  // se déclencher.
  const r = menu.getBoundingClientRect();
  invoke('placer_menu', { largeur: r.width, hauteur: r.height });
};

// Un clic ailleurs retire le focus à la fenêtre : le menu se ferme.
window.addEventListener('blur', () => invoke('fermer_menu'));
window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') invoke('fermer_menu');
});
