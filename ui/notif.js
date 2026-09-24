// La notification maison : pose le titre et le texte que Rust envoie.
// `textContent` et non `innerHTML` : le texte est affiché tel quel, jamais
// interprété comme du HTML.
window.afficherNotif = function (n) {
  document.getElementById("titre").textContent = n.titre;
  document.getElementById("corps").textContent = n.corps;
  // Rejoue l'animation d'entrée à chaque notification : la retirer puis la
  // remettre, avec une lecture de mise en page entre les deux.
  const boite = document.getElementById("notif");
  boite.style.animation = "none";
  void boite.offsetWidth;
  boite.style.animation = "";
};
