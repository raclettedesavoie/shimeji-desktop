// Déclenche la génération de code de Tauri (contexte, capacités, ressource
// Windows). Deux exigences découvertes à l'étape 0 : `icons/icon.ico` doit
// exister, et `frontendDist` est résolu relativement au tauri.conf.json.
fn main() {
    tauri_build::build()
}
